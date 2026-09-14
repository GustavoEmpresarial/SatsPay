//! CSRF / cross-site request checks for cookie-authenticated endpoints.
//!
//! Refresh + logout rely on the HttpOnly refresh cookie. SameSite=Lax blocks
//! most cross-site POSTs; this Origin/Referer allowlist is defense-in-depth.

use axum::http::{HeaderMap, StatusCode};
use axum::Json;

/// Returns Ok(()) when the request looks same-site, or when no Origin/Referer
/// is present (non-browser clients / older fetches).
pub fn assert_browser_csrf(headers: &HeaderMap, allowed_hostnames: &[String]) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty());

    let referer = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty());

    let candidate = origin.or(referer);
    let Some(raw) = candidate else {
        // Native / curl / server-to-server: no Origin — allow (cookie still needed).
        return Ok(());
    };

    let host = url::Url::parse(raw)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase));

    let Some(host) = host else {
        return Err(reject("invalid origin"));
    };

    if allowed_hostnames.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return Ok(());
    }

    // Also allow bare localhost variants used in local SPA proxy.
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") {
        return Ok(());
    }

    Err(reject("cross-site request blocked"))
}

fn reject(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": msg, "code": "CSRF_BLOCKED" })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers_with(origin: Option<&str>, referer: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(o) = origin {
            h.insert(axum::http::header::ORIGIN, HeaderValue::from_str(o).unwrap());
        }
        if let Some(r) = referer {
            h.insert(axum::http::header::REFERER, HeaderValue::from_str(r).unwrap());
        }
        h
    }

    #[test]
    fn allows_missing_origin() {
        let allowed = vec!["www.satspay.pro".into()];
        assert!(assert_browser_csrf(&headers_with(None, None), &allowed).is_ok());
    }

    #[test]
    fn allows_listed_origin() {
        let allowed = vec!["www.satspay.pro".into()];
        assert!(assert_browser_csrf(
            &headers_with(Some("https://www.satspay.pro"), None),
            &allowed
        )
        .is_ok());
    }

    #[test]
    fn allows_localhost_without_allowlist() {
        let allowed: Vec<String> = vec![];
        assert!(assert_browser_csrf(
            &headers_with(Some("http://localhost:5173"), None),
            &allowed
        )
        .is_ok());
        assert!(assert_browser_csrf(
            &headers_with(Some("http://127.0.0.1:3000"), None),
            &allowed
        )
        .is_ok());
    }

    #[test]
    fn uses_referer_when_origin_absent() {
        let allowed = vec!["www.satspay.pro".into()];
        assert!(assert_browser_csrf(
            &headers_with(None, Some("https://www.satspay.pro/settings")),
            &allowed
        )
        .is_ok());
        assert!(assert_browser_csrf(
            &headers_with(None, Some("https://evil.example/x")),
            &allowed
        )
        .is_err());
    }

    #[test]
    fn rejects_unparseable_origin() {
        let allowed = vec!["www.satspay.pro".into()];
        assert!(assert_browser_csrf(&headers_with(Some("not-a-url"), None), &allowed).is_err());
    }

    #[test]
    fn rejects_cross_site_origin() {
        let allowed = vec!["www.satspay.pro".into()];
        let err = assert_browser_csrf(
            &headers_with(Some("https://evil.example"), None),
            &allowed,
        )
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert_eq!(err.1.0["code"], "CSRF_BLOCKED");
    }

    #[test]
    fn reject_helper_shape() {
        let (st, Json(body)) = reject("cross-site request blocked");
        assert_eq!(st, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], "cross-site request blocked");
        assert_eq!(body["code"], "CSRF_BLOCKED");
    }
}
