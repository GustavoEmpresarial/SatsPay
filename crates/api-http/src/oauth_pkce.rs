//! OAuth PKCE helpers (RFC 7636) — S256 only; challenge is mandatory (OAuth 2.1).

use base64::Engine;
use sha2::{Digest, Sha256};

/// Validate and normalize a required code_challenge for storage.
pub fn normalize_challenge(challenge: Option<&str>, method: Option<&str>) -> Result<(String, String), String> {
    let Some(ch) = challenge.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err("code_challenge is required (PKCE S256)".into());
    };
    if ch.len() < 43 || ch.len() > 128 {
        return Err("code_challenge length must be 43..128".into());
    }
    if !ch.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~') {
        return Err("code_challenge has invalid characters".into());
    }
    let method = method.unwrap_or("S256").trim().to_ascii_uppercase();
    if method != "S256" {
        return Err("only code_challenge_method=S256 is supported".into());
    }
    Ok((ch.to_string(), method))
}

use crypto::ct_eq;

/// Verify code_verifier against stored S256 challenge.
pub fn verify_s256(verifier: &str, challenge: &str) -> bool {
    let v = verifier.trim();
    if v.len() < 43 || v.len() > 128 {
        return false;
    }
    let digest = Sha256::digest(v.as_bytes());
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    ct_eq(encoded.as_bytes(), challenge.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_roundtrip() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = {
            let digest = Sha256::digest(verifier.as_bytes());
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
        };
        assert!(verify_s256(verifier, &challenge));
        assert!(!verify_s256("wrong-verifier-wrong-verifier-wrong-xx", &challenge));
    }

    #[test]
    fn normalize_rejects_bad_length_and_chars() {
        assert!(normalize_challenge(Some(&"a".repeat(42)), Some("S256")).is_err());
        assert!(normalize_challenge(Some(&"a".repeat(129)), Some("S256")).is_err());
        assert!(normalize_challenge(Some(&format!("{}!", "a".repeat(42))), Some("S256")).is_err());
        assert!(normalize_challenge(Some(&"a".repeat(43)), None).is_ok());
    }

    #[test]
    fn verify_s256_rejects_short_verifier() {
        assert!(!verify_s256("short", "challenge"));
        assert!(!verify_s256(&"x".repeat(43), &"y".repeat(43)));
    }
}
