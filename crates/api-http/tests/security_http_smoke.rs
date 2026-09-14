//! Axum oneshot smoke — OAuth redirect hardening + PKCE unit wiring.
//! These do not need a live DB; they exercise pure validators used by handlers.

#[cfg(test)]
mod security_http_smoke {
    use api_http::oauth_pkce::{normalize_challenge, verify_s256};
    use api_http::oauth_redirect::{assert_redirect_uri_allowed, is_safe_redirect_uri, validate_redirect_uris};

    #[test]
    fn oauth_redirect_fail_closed_empty_allowlist() {
        assert!(assert_redirect_uri_allowed(&[], "https://a.example/cb").is_err());
    }

    #[test]
    fn oauth_redirect_rejects_javascript_scheme() {
        assert!(!is_safe_redirect_uri("javascript:alert(1)"));
        assert!(validate_redirect_uris(&["javascript:alert(1)".into()]).is_err());
    }

    #[test]
    fn oauth_redirect_accepts_https_merchant() {
        let uris = validate_redirect_uris(&[
            "https://blockminer.space/api/auth/satspay/callback".into(),
        ])
        .unwrap();
        assert_eq!(uris.len(), 1);
        assert!(assert_redirect_uri_allowed(&uris, &uris[0]).is_ok());
    }

    #[test]
    fn pkce_s256_required_shape() {
        let ch = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        let ok = normalize_challenge(Some(ch), Some("S256")).unwrap();
        assert_eq!(ok.1, "S256");
        assert!(normalize_challenge(None, Some("S256")).is_err());
        assert!(normalize_challenge(Some(ch), Some("plain")).is_err());
    }

    #[test]
    fn pkce_verify_rejects_wrong_verifier() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        use base64::Engine;
        use sha2::{Digest, Sha256};
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(verifier.as_bytes()));
        assert!(verify_s256(verifier, &challenge));
        assert!(!verify_s256("totally-wrong-verifier-value-pad-pad-pad", &challenge));
    }
}
