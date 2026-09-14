//! OAuth redirect_uri hardening (OWASP: open redirect / callback injection).

/// Returns true if `uri` is an absolute http(s) URL we allow registering/using.
///
/// Allowed:
/// - `https://…` (any host)
/// - `http://localhost` / `http://127.0.0.1` / `http://[::1]` (with optional port)
///
/// Rejected: relative paths, `javascript:`, `data:`, `file:`, empty, credentials in URL, fragments-only, etc.
pub fn is_safe_redirect_uri(uri: &str) -> bool {
    let uri = uri.trim();
    if uri.is_empty() || uri.len() > 2048 {
        return false;
    }
    // Block common smuggling / weird whitespace before parse.
    if uri.contains(|c: char| c.is_control()) || uri.contains('\\') {
        return false;
    }

    let Ok(parsed) = url::Url::parse(uri) else {
        return false;
    };

    // Must be absolute with a host (url crate treats some schemes specially).
    if parsed.cannot_be_a_base() {
        return false;
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return false;
    }

    match parsed.scheme() {
        "https" => parsed.host_str().is_some_and(|h| !h.is_empty()),
        "http" => matches!(
            parsed.host_str().map(str::to_ascii_lowercase).as_deref(),
            Some("localhost") | Some("127.0.0.1") | Some("::1")
        ),
        _ => false,
    }
}

/// Normalize + validate a list of redirect URIs for create/update.
/// Fails closed: empty list or any invalid entry → error message.
pub fn validate_redirect_uris(uris: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(uris.len());
    for raw in uris {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if !is_safe_redirect_uri(&trimmed) {
            return Err(format!(
                "invalid redirect_uri `{trimmed}` (use https://… or http://localhost)"
            ));
        }
        if !out.contains(&trimmed) {
            out.push(trimmed);
        }
    }
    if out.is_empty() {
        return Err("at least one redirect_uri is required".into());
    }
    Ok(out)
}

/// Authorize/token callback: require non-empty allowlist and exact match.
pub fn assert_redirect_uri_allowed(registered: &[String], requested: &str) -> Result<(), String> {
    if registered.is_empty() {
        return Err("application has no registered redirect_uris".into());
    }
    if !is_safe_redirect_uri(requested) {
        return Err("redirect_uri is not a safe absolute URL".into());
    }
    if !registered.iter().any(|u| u == requested) {
        return Err("redirect_uri not registered for this app".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_and_localhost_http() {
        assert!(is_safe_redirect_uri("https://blockminer.space/api/auth/satspay/callback"));
        assert!(is_safe_redirect_uri("https://www.blockminer.space/api/auth/satspay/callback"));
        assert!(is_safe_redirect_uri("http://localhost:5173/callback"));
        assert!(is_safe_redirect_uri("http://127.0.0.1:3000/cb"));
    }

    #[test]
    fn rejects_open_redirect_and_dangerous_schemes() {
        assert!(!is_safe_redirect_uri(""));
        assert!(!is_safe_redirect_uri("/relative"));
        assert!(!is_safe_redirect_uri("//evil.com"));
        assert!(!is_safe_redirect_uri("javascript:alert(1)"));
        assert!(!is_safe_redirect_uri("data:text/html,hi"));
        assert!(!is_safe_redirect_uri("http://evil.com/cb"));
        assert!(!is_safe_redirect_uri("https://user:pass@evil.com/"));
        assert!(!is_safe_redirect_uri("ftp://files.example/x"));
    }

    #[test]
    fn validate_list_requires_at_least_one_safe_uri() {
        assert!(validate_redirect_uris(&[]).is_err());
        assert!(validate_redirect_uris(&["".into()]).is_err());
        assert!(validate_redirect_uris(&["http://evil.com".into()]).is_err());
        let ok = validate_redirect_uris(&[
            "https://a.example/cb".into(),
            "https://a.example/cb".into(), // dedupe
            "http://localhost/cb".into(),
        ])
        .unwrap();
        assert_eq!(ok.len(), 2);
    }

    #[test]
    fn rejects_control_chars_backslash_and_overlong() {
        assert!(!is_safe_redirect_uri("https://a.example/c\nb"));
        assert!(!is_safe_redirect_uri("https://a.example\\cb"));
        let huge = format!("https://x.example/{}", "a".repeat(2100));
        assert!(!is_safe_redirect_uri(&huge));
        assert!(is_safe_redirect_uri("http://127.0.0.1/cb"));
    }

    #[test]
    fn assert_allowed_rejects_unsafe_requested_uri() {
        assert!(assert_redirect_uri_allowed(
            &["https://a.example/cb".into()],
            "javascript:alert(1)"
        )
        .is_err());
        assert!(assert_redirect_uri_allowed(&[], "https://a.example/cb").is_err());
    }
}
