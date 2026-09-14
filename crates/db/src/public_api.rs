//! Port of legacy `apps/api/src/modules/public-api/{services,repositories,utils}/*.ts`.
//! Owner-alert email on send is best-effort from the HTTP layer
//! (`api-http::public_api`) after a successful commit — this module stays
//! free of SMTP/`EmailSender` deps.

use crate::ledger::{apply_ledger_entry, lock_wallets, LedgerCreditInput};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use crypto::SecretsService;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use shared::Coin;
use sqlx::{PgPool, Row};
use std::time::Duration;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PublicApiError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("API key not found")]
    KeyNotFound,
    #[error("invalid API key")]
    InvalidKey,
    #[error("API key expired")]
    KeyExpired,
    #[error("source IP not allowed for this API key")]
    IpNotAllowed,
    #[error("this API key requires signed requests")]
    RequiresSignature,
    #[error("missing required scope: {0}")]
    MissingScope(String),
    #[error("transfer target is not eligible")]
    IneligibleTarget,
    #[error("cannot send to self")]
    SendToSelf,
    #[error("wallet not found")]
    WalletNotFound,
    #[error("daily send limit reached for this API key")]
    DailyLimitReached,
    #[error("bad timestamp")]
    BadTimestamp,
    #[error("timestamp outside the accepted signing window")]
    TimestampOutOfWindow,
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("signature already used (replay)")]
    SignatureReplay,
}

#[derive(Debug, Clone)]
pub struct ApiKeyRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub scopes: Vec<String>,
    pub allowed_ips: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub disabled_at: Option<DateTime<Utc>>,
    pub require_signature: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeySummary {
    pub id: Uuid,
    pub label: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub allowed_ips: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub require_signature: bool,
    pub created_at: DateTime<Utc>,
    pub disabled_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub async fn list_api_keys_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<ApiKeySummary>, PublicApiError> {
    let rows = sqlx::query(
        "SELECT id, label, key_prefix, scopes, allowed_ips, expires_at, require_signature, created_at, disabled_at, last_used_at \
         FROM api_keys WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let keys = rows
        .into_iter()
        .map(|r| ApiKeySummary {
            id: r.get("id"),
            label: r.get("label"),
            key_prefix: r.get("key_prefix"),
            scopes: r.get("scopes"),
            allowed_ips: r.get("allowed_ips"),
            expires_at: r.get("expires_at"),
            require_signature: r.get("require_signature"),
            created_at: r.get("created_at"),
            disabled_at: r.get("disabled_at"),
            last_used_at: r.get("last_used_at"),
        })
        .collect();

    Ok(keys)
}

#[derive(Debug, serde::Serialize)]
pub struct IssuedKey {
    pub id: Uuid,
    pub key: String,
    pub prefix: String,
}

/// Mints a new API key (raw secret returned once — only its HMAC hash and an
/// AES-GCM-encrypted copy, for future signed-request verification, are
/// persisted).
#[allow(clippy::too_many_arguments)]
pub async fn issue_api_key(pool: &PgPool, secrets: &SecretsService, user_id: Uuid, label: &str, scopes: &[&str], allowed_ips: &[&str], expires_in_days: Option<i64>, require_signature: bool) -> Result<IssuedKey, PublicApiError> {
    let raw = crypto::random_token(32);
    let key_hash = secrets.hmac_hex(&raw);
    let key_prefix = raw[..8].to_string();
    let key_enc = secrets.encrypt(&raw);
    let expires_at = expires_in_days.map(|d| Utc::now() + chrono::Duration::days(d));

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO api_keys (user_id, label, key_hash, key_prefix, key_enc, scopes, allowed_ips, expires_at, require_signature) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(user_id)
    .bind(label)
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(&key_enc)
    .bind(scopes)
    .bind(allowed_ips)
    .bind(expires_at)
    .bind(require_signature)
    .fetch_one(pool)
    .await?;

    Ok(IssuedKey { id, key: raw, prefix: key_prefix })
}

/// Rotates a key in place: new secret, same id/scopes/IP-allowlist/expiry/signing-policy.
pub async fn rotate_api_key(pool: &PgPool, secrets: &SecretsService, user_id: Uuid, id: Uuid) -> Result<IssuedKey, PublicApiError> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM api_keys WHERE id = $1").bind(id).fetch_optional(pool).await?;
    if owner != Some(user_id) {
        return Err(PublicApiError::KeyNotFound);
    }
    let raw = crypto::random_token(32);
    let key_hash = secrets.hmac_hex(&raw);
    let key_prefix = raw[..8].to_string();
    let key_enc = secrets.encrypt(&raw);
    sqlx::query("UPDATE api_keys SET key_hash = $2, key_prefix = $3, key_enc = $4 WHERE id = $1").bind(id).bind(&key_hash).bind(&key_prefix).bind(&key_enc).execute(pool).await?;
    Ok(IssuedKey { id, key: raw, prefix: key_prefix })
}

pub async fn disable_api_key(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<(), PublicApiError> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM api_keys WHERE id = $1").bind(id).fetch_optional(pool).await?;
    if owner != Some(user_id) {
        return Err(PublicApiError::KeyNotFound);
    }
    sqlx::query("UPDATE api_keys SET disabled_at = now() WHERE id = $1").bind(id).execute(pool).await?;
    Ok(())
}

fn row_to_record(row: sqlx::postgres::PgRow) -> ApiKeyRecord {
    ApiKeyRecord {
        id: row.get("id"),
        user_id: row.get("user_id"),
        scopes: row.get("scopes"),
        allowed_ips: row.get("allowed_ips"),
        expires_at: row.get("expires_at"),
        disabled_at: row.get("disabled_at"),
        require_signature: row.get("require_signature"),
    }
}

/// Shared usability gate — disabled/expired/IP-restricted — applied by both
/// auth paths (`x-api-key` and HMAC-signed).
fn check_usable(record: &ApiKeyRecord, source_ip: &str) -> Result<(), PublicApiError> {
    if record.disabled_at.is_some() {
        return Err(PublicApiError::InvalidKey);
    }
    if let Some(exp) = record.expires_at {
        if exp <= Utc::now() {
            return Err(PublicApiError::KeyExpired);
        }
    }
    if !record.allowed_ips.is_empty() && !record.allowed_ips.iter().any(|ip| ip == source_ip) {
        return Err(PublicApiError::IpNotAllowed);
    }
    Ok(())
}

/// Looks up a key by its HMAC hash (the `x-api-key` auth path) and applies
/// the shared usability gate — disabled/expired/IP-restricted/requires-signature.
pub async fn authenticate_by_hash(pool: &PgPool, secrets: &SecretsService, raw_key: &str, source_ip: &str) -> Result<ApiKeyRecord, PublicApiError> {
    let hash = secrets.hmac_hex(raw_key);
    let row = sqlx::query("SELECT id, user_id, scopes, allowed_ips, expires_at, disabled_at, require_signature FROM api_keys WHERE key_hash = $1").bind(&hash).fetch_optional(pool).await?;
    let row = row.ok_or(PublicApiError::InvalidKey)?;
    let record = row_to_record(row);

    check_usable(&record, source_ip)?;
    if record.require_signature {
        return Err(PublicApiError::RequiresSignature);
    }

    sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1").bind(record.id).execute(pool).await.ok();
    Ok(record)
}

/// Canonical string covered by the HMAC signature: timestamp, method, path
/// (including query string, so query params can't be tampered with in
/// transit), and a hash of the raw body — same shape as legacy's
/// `canonicalString`.
fn canonical_string(timestamp: &str, method: &str, path: &str, raw_body: &[u8]) -> String {
    let body_hash = hex::encode(Sha256::digest(raw_body));
    format!("{timestamp}\n{}\n{path}\n{body_hash}", method.to_uppercase())
}

/// Reference implementation of the client signature (handy for tests/SDKs too).
pub fn sign_request(secret: &str, timestamp: &str, method: &str, path: &str, raw_body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(canonical_string(timestamp, method, path, raw_body).as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub struct VerifySignedRequestInput<'a> {
    pub key_id: Uuid,
    pub timestamp: &'a str,
    pub signature: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub raw_body: &'a [u8],
    pub source_ip: &'a str,
}

/// Verifies an HMAC-signed public API request: looks up the key by id,
/// decrypts its secret (never the raw key over the wire), recomputes the
/// signature, checks the timestamp is inside `max_skew`, and reserves the
/// signature as a one-shot nonce in Postgres so it can never be replayed —
/// fails closed on every step, same posture as `captcha::TurnstileVerifier`.
pub async fn verify_signed_request(pool: &PgPool, secrets: &SecretsService, input: VerifySignedRequestInput<'_>, max_skew: Duration) -> Result<ApiKeyRecord, PublicApiError> {
    let ts_num: i64 = input.timestamp.parse().map_err(|_| PublicApiError::BadTimestamp)?;
    // Accept seconds or milliseconds, same heuristic as legacy.
    let ts_ms = if ts_num < 1_000_000_000_000 { ts_num * 1000 } else { ts_num };
    let now_ms = Utc::now().timestamp_millis();
    if (now_ms - ts_ms).unsigned_abs() > max_skew.as_millis() as u64 {
        return Err(PublicApiError::TimestampOutOfWindow);
    }

    let row = sqlx::query("SELECT id, user_id, scopes, allowed_ips, expires_at, disabled_at, require_signature, key_enc FROM api_keys WHERE id = $1").bind(input.key_id).fetch_optional(pool).await?;
    let row = row.ok_or(PublicApiError::KeyNotFound)?;
    let key_enc: String = row.get("key_enc");
    let record = row_to_record(row);
    check_usable(&record, input.source_ip)?;

    let raw_key = secrets.decrypt(&key_enc).map_err(|_| PublicApiError::InvalidKey)?;
    let expected = sign_request(&raw_key, input.timestamp, input.method, input.path, input.raw_body);
    if expected.len() != input.signature.len() || expected.as_bytes().ct_eq(input.signature.as_bytes()).unwrap_u8() != 1 {
        return Err(PublicApiError::SignatureMismatch);
    }

    // One-shot nonce reservation — same signature can't be replayed inside
    // the window. Opportunistically prune this key's expired nonces first so
    // the table doesn't grow unbounded.
    let cutoff = Utc::now() - chrono::Duration::from_std(max_skew).unwrap_or_default();
    sqlx::query("DELETE FROM public_api_signature_nonces WHERE api_key_id = $1 AND seen_at < $2").bind(record.id).bind(cutoff).execute(pool).await.ok();
    let reserved = sqlx::query("INSERT INTO public_api_signature_nonces (api_key_id, signature) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(record.id).bind(input.signature).execute(pool).await?;
    if reserved.rows_affected() == 0 {
        return Err(PublicApiError::SignatureReplay);
    }

    sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1").bind(record.id).execute(pool).await.ok();
    Ok(record)
}

pub fn require_scope(record: &ApiKeyRecord, scope: &str) -> Result<(), PublicApiError> {
    if record.scopes.iter().any(|s| s == scope || s == "*") {
        Ok(())
    } else {
        Err(PublicApiError::MissingScope(scope.to_string()))
    }
}

/// Internal transfer from an API-key owner's DEVELOPER wallet to another
/// user's PERSONAL wallet by email. Idempotent per `idempotency_key`, scoped
/// to the specific API key (finer than per-user, and a guessed key from
/// another credential can never turn a legit send into a no-op).
#[allow(clippy::too_many_arguments)]
pub async fn send_to_user(pool: &PgPool, from_user_id: Uuid, api_key_id: Uuid, coin: Coin, to_email: &str, amount: BigDecimal, idempotency_key: &str, daily_send_limit: i32) -> Result<String, PublicApiError> {
    let recipient_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1").bind(to_email).fetch_optional(pool).await?;
    let recipient_id = recipient_id.ok_or(PublicApiError::IneligibleTarget)?;
    if recipient_id == from_user_id {
        return Err(PublicApiError::SendToSelf);
    }

    let ref_key = format!("pubapi:{api_key_id}:{idempotency_key}");

    let mut tx = pool.begin().await?;

    let from_wallet: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'DEVELOPER'").bind(from_user_id).bind(coin.as_str()).fetch_optional(&mut *tx).await?;
    let to_wallet: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'").bind(recipient_id).bind(coin.as_str()).fetch_optional(&mut *tx).await?;
    let (Some(from_wallet), Some(to_wallet)) = (from_wallet, to_wallet) else { return Err(PublicApiError::WalletNotFound) };

    lock_wallets(&mut tx, &[from_wallet, to_wallet]).await.map_err(|e| PublicApiError::Db(sqlx::Error::Protocol(e.to_string())))?;

    let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM ledger_entries WHERE reference_type = 'PublicApiTransfer' AND reference_key = $1 LIMIT 1").bind(&ref_key).fetch_optional(&mut *tx).await?;
    if existing.is_some() {
        tx.commit().await?;
        return Ok(ref_key);
    }

    // Reservation precedes ledger mutations and rolls back with them on failure.
    let today: NaiveDate = Utc::now().date_naive();
    let reserved: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO api_daily_send_quotas (api_key_id, day, count) VALUES ($1, $2, 1) \
         ON CONFLICT (api_key_id, day) DO UPDATE SET count = api_daily_send_quotas.count + 1 \
         WHERE api_daily_send_quotas.count < $3 \
         RETURNING api_key_id",
    )
    .bind(api_key_id)
    .bind(today)
    .bind(daily_send_limit)
    .fetch_optional(&mut *tx)
    .await?;
    if reserved.is_none() {
        return Err(PublicApiError::DailyLimitReached);
    }

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: Some(&ref_key), wallet_id: from_wallet, amount: -amount.clone(), ledger_type: "TRANSFER_OUT", reference_id: None, reference_type: Some("PublicApiTransfer"), memo: Some(&format!("Send to {to_email}")) },
    )
    .await
    .map_err(|e| PublicApiError::Db(sqlx::Error::Protocol(e.to_string())))?;
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: Some(&ref_key), wallet_id: to_wallet, amount, ledger_type: "TRANSFER_IN", reference_id: None, reference_type: Some("PublicApiTransfer"), memo: Some("Received from public API") },
    )
    .await
    .map_err(|e| PublicApiError::Db(sqlx::Error::Protocol(e.to_string())))?;

    tx.commit().await?;
    // Owner notification email is sent by the HTTP layer after success (best-effort).
    Ok(ref_key)
}

pub async fn get_balance_for_api_key(pool: &PgPool, user_id: Uuid) -> Result<Vec<(Coin, BigDecimal)>, PublicApiError> {
    let rows = sqlx::query(
        "SELECT w.coin::text as coin, COALESCE(SUM(l.amount), 0) as total FROM wallets w \
         LEFT JOIN ledger_entries l ON l.wallet_id = w.id WHERE w.user_id = $1 AND w.kind = 'DEVELOPER' GROUP BY w.coin",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(|r| { let c: String = r.get("coin"); c.parse::<Coin>().ok().map(|coin| (coin, r.get("total"))) }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sample_key(allowed_ips: Vec<String>, disabled: bool, expired: bool) -> ApiKeyRecord {
        ApiKeyRecord {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            scopes: vec!["wallet:read".into(), "send".into()],
            allowed_ips,
            expires_at: if expired { Some(Utc::now() - Duration::hours(1)) } else { Some(Utc::now() + Duration::hours(1)) },
            disabled_at: if disabled { Some(Utc::now()) } else { None },
            require_signature: false,
        }
    }

    #[test]
    fn check_usable_rejects_disabled_expired_and_ip() {
        assert!(matches!(check_usable(&sample_key(vec![], true, false), "1.2.3.4"), Err(PublicApiError::InvalidKey)));
        assert!(matches!(check_usable(&sample_key(vec![], false, true), "1.2.3.4"), Err(PublicApiError::KeyExpired)));
        assert!(matches!(
            check_usable(&sample_key(vec!["10.0.0.1".into()], false, false), "1.2.3.4"),
            Err(PublicApiError::IpNotAllowed)
        ));
        assert!(check_usable(&sample_key(vec!["1.2.3.4".into()], false, false), "1.2.3.4").is_ok());
        assert!(check_usable(&sample_key(vec![], false, false), "9.9.9.9").is_ok());
    }

    #[test]
    fn require_scope_supports_wildcard() {
        let mut rec = sample_key(vec![], false, false);
        assert!(require_scope(&rec, "send").is_ok());
        assert!(matches!(require_scope(&rec, "admin"), Err(PublicApiError::MissingScope(_))));
        rec.scopes = vec!["*".into()];
        assert!(require_scope(&rec, "anything").is_ok());
    }

    #[test]
    fn sign_request_is_deterministic_and_body_sensitive() {
        let a = sign_request("sekrit", "1700000000", "POST", "/v1/public/send", br#"{"a":1}"#);
        let b = sign_request("sekrit", "1700000000", "POST", "/v1/public/send", br#"{"a":1}"#);
        let c = sign_request("sekrit", "1700000000", "POST", "/v1/public/send", br#"{"a":2}"#);
        let d = sign_request("sekrit", "1700000000", "get", "/v1/public/send", br#"{"a":1}"#);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d); // method is part of the canonical string
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn canonical_string_includes_body_hash() {
        let s = canonical_string("1", "post", "/x", b"hi");
        assert!(s.starts_with("1\nPOST\n/x\n"));
        assert_eq!(s.lines().count(), 4);
    }
}
