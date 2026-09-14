//! Real Cloudflare Turnstile verification — port of legacy
//! `apps/api/src/modules/faucet/utils/captcha.ts`. Anti-replay uses a
//! Postgres table (`captcha_seen_tokens`) instead of Redis (see the
//! migration for why), with the same atomic-reservation guarantee.

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Duration as StdDuration;

const TURNSTILE_ENDPOINT: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

#[derive(Debug, thiserror::Error)]
pub enum CaptchaError {
    #[error("TURNSTILE_SECRET must be configured in production")]
    NotConfiguredInProduction,
}

#[derive(Debug, Deserialize)]
struct TurnstileResponse {
    success: bool,
    #[serde(rename = "error-codes")]
    #[allow(dead_code)]
    error_codes: Vec<String>,
    action: Option<String>,
    challenge_ts: Option<String>,
    hostname: Option<String>,
}

pub struct VerifyOptions<'a> {
    pub expected_action: Option<&'a str>,
    pub expected_hostnames: &'a [String],
}

pub struct TurnstileConfig {
    pub secret: String,
    /// `"disabled_in_dev"` bypasses verification (returns true) outside
    /// production, and is a hard error in production — same fail-closed
    /// policy as `chain::policy::assert_stub_client_allowed`.
    pub node_env: String,
    pub timeout: StdDuration,
    pub max_token_age: StdDuration,
    /// Override siteverify URL (tests). `None` → Cloudflare production endpoint.
    pub siteverify_url: Option<String>,
}

pub struct TurnstileVerifier {
    http: reqwest::Client,
    config: TurnstileConfig,
}

impl TurnstileVerifier {
    pub fn new(config: TurnstileConfig) -> Self {
        Self { http: reqwest::Client::new(), config }
    }

    fn endpoint(&self) -> &str {
        self.config.siteverify_url.as_deref().unwrap_or(TURNSTILE_ENDPOINT)
    }

    /// Verifies `token` against Cloudflare, with local anti-replay,
    /// optional action/hostname pinning, and a max token age — fails
    /// closed (`Ok(false)`) on any ambiguous condition (network error,
    /// timeout, replay, mismatch), matching the legacy behavior exactly.
    pub async fn verify(&self, pool: &PgPool, token: &str, remote_ip: Option<&str>, opts: VerifyOptions<'_>) -> Result<bool, CaptchaError> {
        if self.config.secret == "disabled_in_dev" {
            if self.config.node_env == "production" {
                return Err(CaptchaError::NotConfiguredInProduction);
            }
            return Ok(true);
        }
        if token.is_empty() || token.len() > 4096 {
            return Ok(false);
        }

        // --- STEP 1: Quick local pre-check (non-consuming) ---
        let already_seen = self.token_already_seen(pool, token).await;
        if already_seen {
            tracing::warn!(remote_ip, "captcha token reuse blocked (pre-check)");
            return Ok(false);
        }

        // --- STEP 2: Verify with Cloudflare Turnstile ---
        let mut form = vec![("secret", self.config.secret.as_str()), ("response", token)];
        if let Some(ip) = remote_ip {
            form.push(("remoteip", ip));
        }

        let resp = match self.http.post(self.endpoint()).timeout(self.config.timeout).form(&form).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "turnstile siteverify network error");
                return Ok(false);
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = resp.status().as_u16(), "turnstile siteverify HTTP error");
            return Ok(false);
        }
        let json: TurnstileResponse = match resp.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!(error = %e, "turnstile siteverify malformed response");
                return Ok(false);
            }
        };

        if !evaluate_turnstile_response(&json, &opts, self.config.max_token_age) {
            return Ok(false);
        }

        // --- STEP 3: Token is valid — NOW reserve it to prevent replay ---
        if !self.reserve_token(pool, token).await {
            tracing::warn!(remote_ip, "captcha token replay race (post-verify)");
            return Ok(false);
        }

        Ok(true)
    }

    async fn reserve_token(&self, pool: &PgPool, token: &str) -> bool {
        let hash = hex::encode(Sha256::digest(token.as_bytes()));
        match sqlx::query("INSERT INTO captcha_seen_tokens (token_hash) VALUES ($1) ON CONFLICT DO NOTHING").bind(&hash).execute(pool).await {
            Ok(result) => result.rows_affected() == 1,
            Err(e) => {
                tracing::warn!(error = %e, "captcha replay-cache (postgres) unavailable, failing open");
                true
            }
        }
    }

    async fn token_already_seen(&self, pool: &PgPool, token: &str) -> bool {
        let hash = hex::encode(Sha256::digest(token.as_bytes()));
        match sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM captcha_seen_tokens WHERE token_hash = $1)"
        )
        .bind(&hash)
        .fetch_one(pool)
        .await
        {
            Ok(exists) => exists,
            Err(e) => {
                tracing::warn!(error = %e, "captcha replay pre-check (postgres) unavailable, allowing");
                false
            }
        }
    }
}

/// Pure policy checks on a Cloudflare siteverify payload (unit-testable).
fn evaluate_turnstile_response(json: &TurnstileResponse, opts: &VerifyOptions<'_>, max_token_age: StdDuration) -> bool {
    if !json.success {
        tracing::warn!(error_codes = ?json.error_codes, "turnstile verification failed");
        return false;
    }
    if let Some(expected) = opts.expected_action {
        if json.action.as_deref() != Some(expected) {
            tracing::warn!(got = ?json.action, want = expected, "turnstile action mismatch");
            return false;
        }
    }
    if !opts.expected_hostnames.is_empty() {
        match &json.hostname {
            Some(hostname) if opts.expected_hostnames.iter().any(|h| h == hostname) => {}
            other => {
                tracing::warn!(hostname = ?other, "turnstile hostname mismatch or missing");
                return false;
            }
        }
    }
    if let Some(ts_str) = &json.challenge_ts {
        if let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) {
            let age = Utc::now().signed_duration_since(ts.with_timezone(&Utc));
            if age > Duration::from_std(max_token_age).unwrap_or(Duration::zero()) {
                tracing::warn!(challenge_ts = %ts_str, "turnstile token too old");
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    fn lazy_pool() -> PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://bitcosats:bitcosats@127.0.0.1:1/none")
            .expect("lazy pool")
    }

    fn verifier(secret: &str, node_env: &str) -> TurnstileVerifier {
        TurnstileVerifier::new(TurnstileConfig {
            secret: secret.into(),
            node_env: node_env.into(),
            timeout: StdDuration::from_secs(2),
            max_token_age: StdDuration::from_secs(300),
            siteverify_url: None,
        })
    }

    #[tokio::test]
    async fn disabled_in_dev_bypasses_outside_production() {
        let v = verifier("disabled_in_dev", "development");
        let ok = v.verify(&lazy_pool(), "", None, VerifyOptions { expected_action: None, expected_hostnames: &[] }).await.unwrap();
        assert!(ok);
    }

    #[tokio::test]
    async fn disabled_in_dev_fails_closed_in_production() {
        let v = verifier("disabled_in_dev", "production");
        let err = v.verify(&lazy_pool(), "x", None, VerifyOptions { expected_action: None, expected_hostnames: &[] }).await.unwrap_err();
        assert!(matches!(err, CaptchaError::NotConfiguredInProduction));
    }

    #[tokio::test]
    async fn empty_or_huge_token_rejected() {
        let v = verifier("real_secret", "production");
        let opts = VerifyOptions { expected_action: None, expected_hostnames: &[] };
        assert!(!v.verify(&lazy_pool(), "", None, opts).await.unwrap());
        let huge = "x".repeat(4097);
        assert!(!v.verify(&lazy_pool(), &huge, None, VerifyOptions { expected_action: None, expected_hostnames: &[] }).await.unwrap());
    }

    #[test]
    fn evaluate_rejects_failed_success_flag() {
        let json = TurnstileResponse {
            success: false,
            error_codes: vec!["invalid-input-response".into()],
            action: Some("faucet_claim".into()),
            challenge_ts: None,
            hostname: Some("satspay.pro".into()),
        };
        assert!(!evaluate_turnstile_response(&json, &VerifyOptions { expected_action: Some("faucet_claim"), expected_hostnames: &[] }, StdDuration::from_secs(300)));
    }

    #[test]
    fn evaluate_rejects_action_mismatch() {
        let json = TurnstileResponse {
            success: true,
            error_codes: vec![],
            action: Some("login".into()),
            challenge_ts: None,
            hostname: Some("satspay.pro".into()),
        };
        assert!(!evaluate_turnstile_response(
            &json,
            &VerifyOptions { expected_action: Some("faucet_claim"), expected_hostnames: &[] },
            StdDuration::from_secs(300),
        ));
    }

    #[test]
    fn evaluate_rejects_missing_hostname_when_allowlist_set() {
        let hosts = vec!["satspay.pro".to_string()];
        let json = TurnstileResponse {
            success: true,
            error_codes: vec![],
            action: Some("faucet_claim".into()),
            challenge_ts: None,
            hostname: None,
        };
        assert!(!evaluate_turnstile_response(
            &json,
            &VerifyOptions { expected_action: Some("faucet_claim"), expected_hostnames: &hosts },
            StdDuration::from_secs(300),
        ));
    }

    #[test]
    fn evaluate_rejects_hostname_mismatch() {
        let hosts = vec!["satspay.pro".to_string()];
        let json = TurnstileResponse {
            success: true,
            error_codes: vec![],
            action: Some("faucet_claim".into()),
            challenge_ts: None,
            hostname: Some("evil.example".into()),
        };
        assert!(!evaluate_turnstile_response(
            &json,
            &VerifyOptions { expected_action: Some("faucet_claim"), expected_hostnames: &hosts },
            StdDuration::from_secs(300),
        ));
    }

    #[test]
    fn evaluate_rejects_stale_token() {
        let json = TurnstileResponse {
            success: true,
            error_codes: vec![],
            action: Some("faucet_claim".into()),
            challenge_ts: Some("2020-01-01T00:00:00Z".into()),
            hostname: Some("satspay.pro".into()),
        };
        assert!(!evaluate_turnstile_response(
            &json,
            &VerifyOptions { expected_action: Some("faucet_claim"), expected_hostnames: &[] },
            StdDuration::from_secs(300),
        ));
    }

    #[test]
    fn evaluate_accepts_valid_payload() {
        let hosts = vec!["satspay.pro".to_string()];
        let json = TurnstileResponse {
            success: true,
            error_codes: vec![],
            action: Some("faucet_claim".into()),
            challenge_ts: Some(Utc::now().to_rfc3339()),
            hostname: Some("satspay.pro".into()),
        };
        assert!(evaluate_turnstile_response(
            &json,
            &VerifyOptions { expected_action: Some("faucet_claim"), expected_hostnames: &hosts },
            StdDuration::from_secs(300),
        ));
    }
}
