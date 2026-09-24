//! One structured access-log line per HTTP request + a correlation id.
//!
//! Before this, neither the API nor Caddy logged requests, so an incident
//! could only be reconstructed from the ledger and the 5xx table. Every
//! request now gets:
//!
//! - a `request_id` — the caller's `X-Request-Id` when it is short and safe,
//!   otherwise a fresh `req_<uuid>`; echoed back in the `X-Request-Id` header;
//! - a tracing span carrying that id, so every log emitted while handling the
//!   request (including `http_error::internal_error`) is correlated;
//! - one `http_access` event: method, route template, status, duration.
//!
//! What is deliberately **not** logged: query strings (OAuth `code`/`state`,
//! e-mails, API keys in callbacks), bodies, headers, raw paths with ids (the
//! route template `/v1/wallet/:coin` is used instead), and raw IPs.

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use tracing::Instrument;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Requests slower than this are logged at WARN (`PERFORMANCE_DEGRADATION`).
const SLOW_REQUEST_MS: u128 = 2_000;

/// Probes and scrapes: logged at DEBUG so they do not drown the access log.
const QUIET_PATHS: &[&str] = &["/healthz", "/metrics"];

tokio::task_local! {
    static REQUEST_ID: String;
}

/// Correlation id of the request being handled, if called inside one.
pub fn current_request_id() -> Option<String> {
    REQUEST_ID.try_with(Clone::clone).ok()
}

/// Accept a caller-supplied id only if it cannot forge log lines or blow up
/// cardinality: 8–64 chars of `[A-Za-z0-9_-]`.
fn sanitize_incoming(v: &HeaderValue) -> Option<String> {
    let s = v.to_str().ok()?.trim();
    let ok = (8..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    ok.then(|| s.to_string())
}

fn new_request_id() -> String {
    format!("req_{}", uuid::Uuid::new_v4().simple())
}

/// Route template when axum matched one, else a fixed bucket — never the raw
/// path, which carries ids and would be attacker-controlled for 404s.
fn route_label(req: &Request) -> String {
    req.extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| {
            let p = req.uri().path();
            if QUIET_PATHS.contains(&p) {
                p.to_string()
            } else {
                "<unmatched>".to_string()
            }
        })
}

pub async fn layer(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(sanitize_incoming)
        .unwrap_or_else(new_request_id);
    if let Ok(v) = HeaderValue::from_str(&request_id) {
        request.headers_mut().insert(REQUEST_ID_HEADER, v);
    }

    let method = request.method().clone();
    let route = route_label(&request);
    let quiet = QUIET_PATHS.contains(&route.as_str());
    let span =
        tracing::info_span!("http", request_id = %request_id, method = %method, route = %route);
    let started = Instant::now();

    let mut response = REQUEST_ID
        .scope(
            request_id.clone(),
            next.run(request).instrument(span.clone()),
        )
        .await;

    let status = response.status().as_u16();
    let duration_ms = started.elapsed().as_millis();
    if !quiet {
        let module = route
            .split('/')
            .nth(2)
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown");
        static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let version = VERSION
            .get_or_init(|| std::env::var("APP_VERSION").unwrap_or_else(|_| "unknown".into()));
        axum_prometheus::metrics::counter!("satspay_http_requests_total", "module" => module.to_owned(), "version" => version.clone(), "status_class" => format!("{}xx", status / 100)).increment(1);
    }
    if let Ok(v) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, v);
    }

    let _enter = span.enter();
    if quiet {
        tracing::debug!(target: "http_access", status, duration_ms, "request");
    } else if status >= 500 {
        tracing::error!(target: "http_access", status, duration_ms, "request");
    } else if duration_ms >= SLOW_REQUEST_MS {
        tracing::warn!(target: "http_access", status, duration_ms, code = "PERFORMANCE_DEGRADATION", "slow request");
    } else {
        tracing::info!(target: "http_access", status, duration_ms, "request");
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn app() -> Router {
        Router::new()
            .route(
                "/v1/items/:id",
                get(|| async { current_request_id().unwrap_or_default() }),
            )
            .route(
                "/boom",
                get(|| async { crate::http_error::internal_error("db down") }),
            )
            .layer(axum::middleware::from_fn(layer))
    }

    async fn send(uri: &str, rid: Option<&str>) -> Response {
        let mut b = Request::builder().uri(uri);
        if let Some(r) = rid {
            b = b.header("x-request-id", r);
        }
        app().oneshot(b.body(Body::empty()).unwrap()).await.unwrap()
    }

    #[tokio::test]
    async fn generates_request_id_and_exposes_it_to_handlers() {
        let res = send("/v1/items/42?code=secret", None).await;
        assert_eq!(res.status(), StatusCode::OK);
        let header = res
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(header.starts_with("req_"));
        let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
        assert_eq!(std::str::from_utf8(&body).unwrap(), header);
    }

    #[tokio::test]
    async fn keeps_safe_incoming_id_and_replaces_unsafe_one() {
        let res = send("/v1/items/1", Some("abc-123_XYZ")).await;
        assert_eq!(res.headers()["x-request-id"], "abc-123_XYZ");
        // CRLF / spaces / too short → replaced, never echoed.
        for bad in ["short", "has space here!!", &"x".repeat(65)] {
            let res = send("/v1/items/1", Some(bad)).await;
            let v = res.headers()["x-request-id"].to_str().unwrap();
            assert!(v.starts_with("req_"), "{bad} -> {v}");
        }
    }

    #[tokio::test]
    async fn internal_error_body_carries_the_same_request_id() {
        let res = send("/boom", Some("corr-00000001")).await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["requestId"], "corr-00000001");
        assert_eq!(v["code"], "INTERNAL");
    }

    #[test]
    fn sanitize_rejects_header_injection() {
        assert!(sanitize_incoming(&HeaderValue::from_static("ok-request-id")).is_some());
        assert!(sanitize_incoming(&HeaderValue::from_static("bad\tvalue-xxxx")).is_none());
        assert!(sanitize_incoming(&HeaderValue::from_static("../../etc/passwd")).is_none());
    }
}
