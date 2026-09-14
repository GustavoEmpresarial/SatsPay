//! Resolve the real client IP for audit / captcha / API-key IP allowlists.
//!
//! Trust order (nginx must **overwrite** forwarding headers — see `client/nginx.conf`):
//! 1. `X-Real-IP` (set by our reverse proxy to `$remote_addr`, or by the edge
//!    from Cloudflare's real client IP — never copied from a client header here)
//! 2. **Last** non-private hop of `X-Forwarded-For` (closest proxy)
//! 3. TCP peer from axum `ConnectInfo<SocketAddr>`
//!
//! We deliberately do **not** read `CF-Connecting-IP` / `True-Client-IP` in the
//! API: when the container is not behind Cloudflare those headers are
//! attacker-controlled and would bypass faucet per-IP Sybil cooldown, API-key
//! allowlists, and rate limits. The edge (Caddy/CF) must materialize the real
//! client into `X-Real-IP` before traffic reaches this process.

use axum::async_trait;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use std::net::SocketAddr;

/// Resolved client IP as a string (no invented placeholder like `0.0.0.0`).
#[derive(Debug, Clone)]
pub struct ClientIp(pub String);

fn is_internal_or_private(ip_str: &str) -> bool {
    if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            std::net::IpAddr::V6(v6) => v6.is_loopback(),
        }
    } else {
        false
    }
}

/// Pure lookup used by extractors and by handlers that already own `Parts`
/// / `Request` extensions (e.g. public-api body-consuming auth).
pub fn resolve_client_ip(parts: &Parts) -> Option<String> {
    if let Some(real) = parts.headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let trimmed = real.trim();
        if !trimmed.is_empty() && !is_internal_or_private(trimmed) {
            return Some(trimmed.to_string());
        }
    }
    if let Some(xff) = parts.headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        // Last hop = closest proxy. First hop is client-spoofable when the proxy appends.
        for hop in xff.split(',').map(str::trim).filter(|s| !s.is_empty()).rev() {
            if !is_internal_or_private(hop) {
                return Some(hop.to_string());
            }
        }
    }
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
}

#[async_trait]
impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        resolve_client_ip(parts)
            .map(ClientIp)
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "client IP unavailable"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, Request};

    fn parts_with(headers: HeaderMap) -> Parts {
        let mut req = Request::builder().body(()).unwrap();
        *req.headers_mut() = headers;
        let (parts, _) = req.into_parts();
        parts
    }

    fn hdr(map: &mut HeaderMap, name: &'static str, value: &str) {
        map.insert(name, HeaderValue::from_str(value).unwrap());
    }

    #[test]
    fn x_real_ip_wins_over_spoofed_xff_and_cf_headers() {
        let mut h = HeaderMap::new();
        hdr(&mut h, "cf-connecting-ip", "198.51.100.99");
        hdr(&mut h, "true-client-ip", "198.51.100.98");
        hdr(&mut h, "x-forwarded-for", "203.0.113.1, 198.51.100.50");
        hdr(&mut h, "x-real-ip", "203.0.113.77");
        assert_eq!(resolve_client_ip(&parts_with(h)).as_deref(), Some("203.0.113.77"));
    }

    #[test]
    fn xff_uses_last_non_private_hop() {
        let mut h = HeaderMap::new();
        hdr(&mut h, "x-forwarded-for", "203.0.113.1, 198.51.100.50");
        assert_eq!(resolve_client_ip(&parts_with(h)).as_deref(), Some("198.51.100.50"));
    }

    #[test]
    fn xff_skips_private_last_hops() {
        let mut h = HeaderMap::new();
        hdr(&mut h, "x-forwarded-for", "203.0.113.9, 10.0.0.1");
        assert_eq!(resolve_client_ip(&parts_with(h)).as_deref(), Some("203.0.113.9"));
    }

    #[test]
    fn private_x_real_ip_falls_through_to_xff_not_cf() {
        let mut h = HeaderMap::new();
        hdr(&mut h, "x-real-ip", "172.27.0.4");
        hdr(&mut h, "cf-connecting-ip", "198.51.100.99");
        hdr(&mut h, "x-forwarded-for", "203.0.113.77");
        assert_eq!(resolve_client_ip(&parts_with(h)).as_deref(), Some("203.0.113.77"));
    }

    #[test]
    fn cf_connecting_ip_is_never_trusted() {
        let mut h = HeaderMap::new();
        hdr(&mut h, "cf-connecting-ip", "198.51.100.99");
        assert_eq!(resolve_client_ip(&parts_with(h)), None);
    }

    #[test]
    fn private_x_real_ip_and_spoofed_cf_fall_to_connect_info() {
        let mut req = Request::builder().body(()).unwrap();
        hdr(req.headers_mut(), "x-real-ip", "172.27.0.4");
        hdr(req.headers_mut(), "cf-connecting-ip", "198.51.100.99");
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 44], 443))));
        let (parts, _) = req.into_parts();
        assert_eq!(resolve_client_ip(&parts).as_deref(), Some("203.0.113.44"));
    }

    #[test]
    fn connect_info_fallback() {
        let mut req = Request::builder().body(()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 44], 443))));
        let (parts, _) = req.into_parts();
        assert_eq!(resolve_client_ip(&parts).as_deref(), Some("203.0.113.44"));
    }
}
