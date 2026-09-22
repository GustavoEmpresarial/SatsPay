//! Searchable + sealed PII on top of [`SecretsService`].
//!
//! Ciphertexts are stored as `enc:v1:` + AES-GCM. Plaintext rows (no prefix)
//! still open so a rolling backfill does not break reads.

use super::secrets::SecretsService;

pub const PII_PREFIX: &str = "enc:v1:";

/// API-key scopes that actually gate a route. Anything else is rejected at issue.
pub const ALLOWED_API_SCOPES: &[&str] = &["deposits", "send", "balance", "*"];

impl SecretsService {
    pub fn normalize_email(email: &str) -> String {
        email.trim().to_ascii_lowercase()
    }

    /// Blind index for login / send-by-email. Not reversible.
    pub fn email_index(&self, email: &str) -> String {
        self.email_index_hmac(&Self::normalize_email(email))
    }

    /// HMAC of an IP for audit / telemetry equality without storing the raw address.
    pub fn ip_fingerprint(&self, ip: &str) -> String {
        self.ip_hmac(ip.trim())
    }

    pub fn seal_pii(&self, kind: &str, row_key: &str, plaintext: &str) -> String {
        if plaintext.is_empty() || plaintext.starts_with(PII_PREFIX) {
            return plaintext.to_string();
        }
        let aad = format!("{kind}:{row_key}");
        format!("{PII_PREFIX}{}", self.encrypt_with_aad(plaintext, aad.as_bytes()))
    }

    pub fn open_pii(&self, kind: &str, row_key: &str, stored: &str) -> String {
        let Some(ct) = stored.strip_prefix(PII_PREFIX) else {
            return stored.to_string();
        };
        let aad = format!("{kind}:{row_key}");
        self.decrypt_with_aad(ct, aad.as_bytes()).unwrap_or_else(|_| stored.to_string())
    }

    pub fn open_pii_opt(&self, kind: &str, row_key: &str, stored: Option<&str>) -> Option<String> {
        stored.map(|s| self.open_pii(kind, row_key, s))
    }

    pub fn invoice_row_key(merchant_id: &str, order_id: &str) -> String {
        format!("{merchant_id}:{order_id}")
    }
}

pub fn validate_api_scopes(scopes: &[String]) -> Result<(), String> {
    if scopes.is_empty() {
        return Err("scopes must not be empty".into());
    }
    for s in scopes {
        if !ALLOWED_API_SCOPES.contains(&s.as_str()) {
            return Err(format!("unknown scope: {s}"));
        }
    }
    Ok(())
}
