//! LGPD export / erase + rolling PII backfill. Ledger rows are never deleted (art. 16).

use crate::house::HOUSE_EMAIL;
use crate::ledger::get_wallet_balance;
use crypto::{SecretsService, PII_PREFIX};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

pub const KIND_USER_EMAIL: &str = "user.email";
pub const KIND_MERCHANT_NAME: &str = "user.merchant.business_name";
pub const KIND_MERCHANT_WEB: &str = "user.merchant.website";
pub const KIND_MERCHANT_DESC: &str = "user.merchant.description";
pub const KIND_TICKET_SUBJECT: &str = "ticket.subject";
pub const KIND_MSG_BODY: &str = "ticket.body";
pub const KIND_WD_TO: &str = "withdrawal.to";
pub const KIND_KEY_LABEL: &str = "api_key.label";

/// IPv4/IPv6 still in the clear (HMAC hex never contains `.` or `:`).
pub fn looks_like_ip(value: &str) -> bool {
    let v = value.trim();
    !v.is_empty() && (v.contains('.') || v.contains(':')) && !v.starts_with(PII_PREFIX)
}

pub fn store_ip(secrets: Option<&SecretsService>, ip: &str) -> String {
    match secrets {
        Some(s) if looks_like_ip(ip) => s.ip_fingerprint(ip),
        _ => ip.to_string(),
    }
}

pub fn seal_opt(secrets: Option<&SecretsService>, kind: &str, row_key: &str, plaintext: &str) -> String {
    match secrets {
        Some(s) => s.seal_pii(kind, row_key, plaintext),
        None => plaintext.to_string(),
    }
}

pub fn open_opt(secrets: Option<&SecretsService>, kind: &str, row_key: &str, stored: &str) -> String {
    match secrets {
        Some(s) => s.open_pii(kind, row_key, stored),
        None => stored.to_string(),
    }
}

pub fn open_opt_option(secrets: Option<&SecretsService>, kind: &str, row_key: &str, stored: Option<&str>) -> Option<String> {
    stored.map(|s| open_opt(secrets, kind, row_key, s))
}

pub fn reveal_stored_email(secrets: Option<&SecretsService>, user_id: Uuid, email: &str, email_enc: Option<&str>) -> String {
    if email.eq_ignore_ascii_case(HOUSE_EMAIL) {
        return email.to_string();
    }
    let Some(s) = secrets else {
        return email.to_string();
    };
    if let Some(enc) = email_enc {
        if enc.starts_with(PII_PREFIX) {
            return s.open_pii(KIND_USER_EMAIL, &user_id.to_string(), enc);
        }
    }
    if email.starts_with(PII_PREFIX) {
        return s.open_pii(KIND_USER_EMAIL, &user_id.to_string(), email);
    }
    email.to_string()
}

fn truncate_ua(ua: &str) -> String {
    ua.chars().take(80).collect()
}

#[derive(Debug, thiserror::Error)]
pub enum PrivacyError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("user not found")]
    NotFound,
    #[error("already erased")]
    AlreadyErased,
}

pub async fn export_user_data(pool: &PgPool, secrets: &SecretsService, user_id: Uuid) -> Result<Value, PrivacyError> {
    let user = sqlx::query_as::<_, (Uuid, String, Option<String>, String, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, email, email_enc, username, merchant_status::text, created_at, erased_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(PrivacyError::NotFound)?;

    let wallets = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, coin::text, kind::text FROM wallets WHERE user_id = $1 ORDER BY coin, kind",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut wallet_out = Vec::new();
    let mut tx = pool.begin().await?;
    for (wid, coin, kind) in wallets {
        let bal = get_wallet_balance(&mut tx, wid).await.unwrap_or_else(|_| 0.into());
        wallet_out.push(json!({
            "coin": coin,
            "kind": kind,
            "balance": bal.to_string(),
        }));
    }
    tx.commit().await?;

    let invoices = sqlx::query_as::<_, (Uuid, String, String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, order_id, coin::text, status::text, created_at \
         FROM merchant_deposit_invoices WHERE merchant_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let invoice_out: Vec<Value> = invoices
        .into_iter()
        .map(|(id, order_id, coin, status, created_at)| {
            json!({
                "id": id,
                "orderId": order_id,
                "coin": coin,
                "status": status,
                "createdAt": created_at,
            })
        })
        .collect();

    let keys = sqlx::query_as::<_, (Uuid, String, String, Vec<String>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, label, key_prefix, scopes, disabled_at FROM api_keys WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let ticket_rows = sqlx::query_as::<_, (Uuid, String, String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, topic, subject, status::text, created_at FROM support_tickets WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut tickets_out = Vec::new();
    for (tid, topic, subject, status, created_at) in ticket_rows {
        let msgs = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, body, created_at FROM support_messages WHERE ticket_id = $1 ORDER BY created_at ASC",
        )
        .bind(tid)
        .fetch_all(pool)
        .await?;
        tickets_out.push(json!({
            "id": tid,
            "topic": topic,
            "subject": secrets.open_pii(KIND_TICKET_SUBJECT, &tid.to_string(), &subject),
            "status": status,
            "createdAt": created_at,
            "messages": msgs.into_iter().map(|(mid, body, at)| json!({
                "id": mid,
                "body": secrets.open_pii(KIND_MSG_BODY, &mid.to_string(), &body),
                "createdAt": at,
            })).collect::<Vec<_>>(),
        }));
    }

    let email = reveal_stored_email(Some(secrets), user.0, &user.1, user.2.as_deref());

    Ok(json!({
        "exportedAt": chrono::Utc::now(),
        "controller": "SatsPay",
        "legalBasis": ["contract", "legal_obligation", "fraud_prevention"],
        "user": {
            "id": user.0,
            "email": email,
            "username": user.3,
            "merchantStatus": user.4,
            "createdAt": user.5,
            "erasedAt": user.6,
        },
        "wallets": wallet_out,
        "invoices": invoice_out,
        "supportTickets": tickets_out,
        "apiKeys": keys.into_iter().map(|(id, label, prefix, scopes, disabled)| json!({
            "id": id,
            "label": secrets.open_pii(KIND_KEY_LABEL, &id.to_string(), &label),
            "prefix": prefix,
            "scopes": scopes,
            "disabled": disabled.is_some(),
        })).collect::<Vec<_>>(),
    }))
}

/// Anonymize profile PII, revoke sessions and keys. Does not touch ledger_entries.
pub async fn erase_user(pool: &PgPool, secrets: &SecretsService, user_id: Uuid) -> Result<(), PrivacyError> {
    let erased_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT erased_at FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten();
    if erased_at.is_some() {
        return Err(PrivacyError::AlreadyErased);
    }
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(PrivacyError::NotFound);
    }

    let tombstone = format!("erased+{user_id}@invalid.local");
    let hmac = secrets.email_index(&tombstone);
    let unusable = format!("!erased!{}", crypto::random_token(16));

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE users SET \
            email = $2, email_hmac = $3, email_enc = NULL, \
            username = $4, \
            merchant_business_name = NULL, merchant_website = NULL, merchant_description = NULL, \
            password_hash = $5, two_factor_enabled = false, totp_secret_enc = NULL, \
            erased_at = now(), updated_at = now() \
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(&tombstone)
    .bind(&hmac)
    .bind(format!("d_{}", &user_id.as_simple().to_string()[..22]))
    .bind(&unusable)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE api_keys SET disabled_at = now() WHERE user_id = $1 AND disabled_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn seal_user_email(pool: &PgPool, secrets: &SecretsService, user_id: Uuid, email: &str) -> Result<(), PrivacyError> {
    let hmac = secrets.email_index(email);
    let enc = secrets.seal_pii("user.email", &user_id.to_string(), &SecretsService::normalize_email(email));
    sqlx::query("UPDATE users SET email_hmac = $2, email_enc = $3, updated_at = now() WHERE id = $1 AND erased_at IS NULL")
        .bind(user_id)
        .bind(&hmac)
        .bind(&enc)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn find_user_id_by_email(pool: &PgPool, secrets: &SecretsService, email: &str) -> Result<Option<Uuid>, PrivacyError> {
    let hmac = secrets.email_index(email);
    let id = sqlx::query_scalar(
        "SELECT id FROM users WHERE email_hmac = $1 OR lower(email) = lower($2) LIMIT 1",
    )
    .bind(&hmac)
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BackfillReport {
    pub users: u64,
    pub merchants: u64,
    pub invoices: u64,
    pub tickets: u64,
    pub messages: u64,
    pub withdrawals: u64,
    pub api_key_labels: u64,
    pub ips: u64,
}

/// Idempotent: only seals rows that are still plaintext.
pub async fn backfill_pii(pool: &PgPool, secrets: &SecretsService) -> Result<BackfillReport, PrivacyError> {
    let mut report = BackfillReport::default();
    report.users = backfill_users(pool, secrets).await?;
    report.merchants = backfill_merchant_profile(pool, secrets).await?;
    report.invoices = backfill_invoices(pool, secrets).await?;
    report.tickets = backfill_tickets(pool, secrets).await?;
    report.messages = backfill_messages(pool, secrets).await?;
    report.withdrawals = backfill_withdrawals(pool, secrets).await?;
    report.api_key_labels = backfill_api_key_labels(pool, secrets).await?;
    report.ips = backfill_ips(pool, secrets).await?;
    Ok(report)
}

async fn backfill_users(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>)>(
        "SELECT id, email, email_hmac, email_enc FROM users \
         WHERE erased_at IS NULL AND lower(email) <> lower($1) \
           AND email NOT LIKE 'erased+%' AND email NOT LIKE 'sealed+%' \
           AND (email_hmac IS NULL OR email_enc IS NULL OR email_enc NOT LIKE 'enc:v1:%')",
    )
    .bind(HOUSE_EMAIL)
    .fetch_all(pool)
    .await?;
    let n = rows.len() as u64;
    for (id, email, _, _) in rows {
        seal_user_email(pool, secrets, id, &email).await?;
    }
    Ok(n)
}

async fn backfill_merchant_profile(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, merchant_business_name, merchant_website, merchant_description FROM users \
         WHERE erased_at IS NULL AND ( \
            (merchant_business_name IS NOT NULL AND merchant_business_name NOT LIKE 'enc:v1:%') \
            OR (merchant_website IS NOT NULL AND merchant_website NOT LIKE 'enc:v1:%') \
            OR (merchant_description IS NOT NULL AND merchant_description NOT LIKE 'enc:v1:%') \
         )",
    )
    .fetch_all(pool)
    .await?;
    let n = rows.len() as u64;
    for (id, name, web, desc) in rows {
        let key = id.to_string();
        let name = name.map(|v| secrets.seal_pii(KIND_MERCHANT_NAME, &key, &v));
        let web = web.map(|v| secrets.seal_pii(KIND_MERCHANT_WEB, &key, &v));
        let desc = desc.map(|v| secrets.seal_pii(KIND_MERCHANT_DESC, &key, &v));
        sqlx::query(
            "UPDATE users SET merchant_business_name = COALESCE($2, merchant_business_name), \
             merchant_website = COALESCE($3, merchant_website), \
             merchant_description = COALESCE($4, merchant_description) WHERE id = $1",
        )
        .bind(id)
        .bind(name.as_deref())
        .bind(web.as_deref())
        .bind(desc.as_deref())
        .execute(pool)
        .await?;
    }
    Ok(n)
}

async fn backfill_invoices(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, merchant_id, order_id, callback_url, success_url, cancel_url, customer_email, customer_name, site_user_id \
         FROM merchant_deposit_invoices \
         WHERE callback_url NOT LIKE 'enc:v1:%' \
            OR (success_url IS NOT NULL AND success_url NOT LIKE 'enc:v1:%') \
            OR (cancel_url IS NOT NULL AND cancel_url NOT LIKE 'enc:v1:%') \
            OR (customer_email IS NOT NULL AND customer_email NOT LIKE 'enc:v1:%') \
            OR (customer_name IS NOT NULL AND customer_name NOT LIKE 'enc:v1:%') \
            OR (site_user_id IS NOT NULL AND site_user_id NOT LIKE 'enc:v1:%')",
    )
    .fetch_all(pool)
    .await?;
    let n = rows.len() as u64;
    for (id, merchant_id, order_id, callback, success, cancel, cemail, cname, site_user) in rows {
        let key = SecretsService::invoice_row_key(&merchant_id.to_string(), &order_id);
        sqlx::query(
            "UPDATE merchant_deposit_invoices SET callback_url = $2, success_url = $3, cancel_url = $4, \
             customer_email = $5, customer_name = $6, site_user_id = $7 WHERE id = $1",
        )
        .bind(id)
        .bind(secrets.seal_pii("invoice.callback", &key, &callback))
        .bind(success.as_deref().map(|v| secrets.seal_pii("invoice.success", &key, v)))
        .bind(cancel.as_deref().map(|v| secrets.seal_pii("invoice.cancel", &key, v)))
        .bind(cemail.as_deref().map(|v| secrets.seal_pii("invoice.customer_email", &key, v)))
        .bind(cname.as_deref().map(|v| secrets.seal_pii("invoice.customer_name", &key, v)))
        .bind(site_user.as_deref().map(|v| secrets.seal_pii("invoice.site_user", &key, v)))
        .execute(pool)
        .await?;
    }
    Ok(n)
}

async fn backfill_tickets(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, subject FROM support_tickets WHERE subject NOT LIKE 'enc:v1:%'",
    )
    .fetch_all(pool)
    .await?;
    let n = rows.len() as u64;
    for (id, subject) in rows {
        let sealed = secrets.seal_pii(KIND_TICKET_SUBJECT, &id.to_string(), &subject);
        sqlx::query("UPDATE support_tickets SET subject = $2 WHERE id = $1")
            .bind(id)
            .bind(&sealed)
            .execute(pool)
            .await?;
    }
    Ok(n)
}

async fn backfill_messages(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, body FROM support_messages WHERE body NOT LIKE 'enc:v1:%'",
    )
    .fetch_all(pool)
    .await?;
    let n = rows.len() as u64;
    for (id, body) in rows {
        let sealed = secrets.seal_pii(KIND_MSG_BODY, &id.to_string(), &body);
        sqlx::query("UPDATE support_messages SET body = $2 WHERE id = $1")
            .bind(id)
            .bind(&sealed)
            .execute(pool)
            .await?;
    }
    Ok(n)
}

async fn backfill_withdrawals(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, to_address, requested_ip FROM withdrawals \
         WHERE to_address NOT LIKE 'enc:v1:%' OR requested_ip IS NULL OR requested_ip LIKE '%.%' OR requested_ip LIKE '%:%'",
    )
    .fetch_all(pool)
    .await?;
    let n = rows.len() as u64;
    for (id, addr, ip) in rows {
        let sealed = secrets.seal_pii(KIND_WD_TO, &id.to_string(), &addr);
        let ip = ip.map(|v| store_ip(Some(secrets), &v));
        sqlx::query("UPDATE withdrawals SET to_address = $2, requested_ip = COALESCE($3, requested_ip) WHERE id = $1")
            .bind(id)
            .bind(&sealed)
            .bind(ip.as_deref())
            .execute(pool)
            .await?;
    }
    Ok(n)
}

async fn backfill_api_key_labels(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, label FROM api_keys WHERE label NOT LIKE 'enc:v1:%'",
    )
    .fetch_all(pool)
    .await?;
    let n = rows.len() as u64;
    for (id, label) in rows {
        let sealed = secrets.seal_pii(KIND_KEY_LABEL, &id.to_string(), &label);
        sqlx::query("UPDATE api_keys SET label = $2 WHERE id = $1")
            .bind(id)
            .bind(&sealed)
            .execute(pool)
            .await?;
    }
    Ok(n)
}

async fn backfill_ips(pool: &PgPool, secrets: &SecretsService) -> Result<u64, PrivacyError> {
    let mut n = 0u64;
    let audit = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, ip FROM audit_logs WHERE ip IS NOT NULL AND (ip LIKE '%.%' OR ip LIKE '%:%')",
    )
    .fetch_all(pool)
    .await?;
    n += audit.len() as u64;
    for (id, ip) in audit {
        sqlx::query("UPDATE audit_logs SET ip = $2 WHERE id = $1")
            .bind(id)
            .bind(secrets.ip_fingerprint(&ip))
            .execute(pool)
            .await?;
    }
    let faucet = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, ip FROM faucet_claims WHERE ip LIKE '%.%' OR ip LIKE '%:%'",
    )
    .fetch_all(pool)
    .await?;
    n += faucet.len() as u64;
    for (id, ip) in faucet {
        sqlx::query("UPDATE faucet_claims SET ip = $2 WHERE id = $1")
            .bind(id)
            .bind(secrets.ip_fingerprint(&ip))
            .execute(pool)
            .await?;
    }
    let tel = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>)>(
        "SELECT id, ip_address, user_agent FROM system_error_logs \
         WHERE (ip_address IS NOT NULL AND (ip_address LIKE '%.%' OR ip_address LIKE '%:%')) \
            OR (user_agent IS NOT NULL AND char_length(user_agent) > 80)",
    )
    .fetch_all(pool)
    .await?;
    n += tel.len() as u64;
    for (id, ip, ua) in tel {
        let ip = ip.map(|v| store_ip(Some(secrets), &v));
        let ua = ua.map(|v| truncate_ua(&v));
        sqlx::query("UPDATE system_error_logs SET ip_address = COALESCE($2, ip_address), user_agent = COALESCE($3, user_agent) WHERE id = $1")
            .bind(id)
            .bind(ip.as_deref())
            .bind(ua.as_deref())
            .execute(pool)
            .await?;
    }
    Ok(n)
}

/// Deploy 4 — only after HMAC lookups are live. HOUSE and already-erased rows stay.
pub async fn blank_plaintext_emails(pool: &PgPool) -> Result<u64, PrivacyError> {
    let res = sqlx::query(
        "UPDATE users SET email = 'sealed+' || id::text || '@invalid.local', updated_at = now() \
         WHERE email_hmac IS NOT NULL AND email_enc IS NOT NULL AND email_enc LIKE 'enc:v1:%' \
           AND erased_at IS NULL \
           AND email NOT LIKE 'erased+%' AND email NOT LIKE 'sealed+%' \
           AND lower(email) <> lower($1)",
    )
    .bind(HOUSE_EMAIL)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RetentionReport {
    pub otps: u64,
    pub captcha: u64,
    pub refresh: u64,
    pub telemetry: u64,
}

/// OTP > 7d, captcha > 7d, revoked/expired refresh > 30d, telemetry > 90d. Never ledger.
pub async fn retain_expired_pii(pool: &PgPool) -> Result<RetentionReport, PrivacyError> {
    let otps = sqlx::query("DELETE FROM email_otps WHERE expires_at < now() - interval '7 days'")
        .execute(pool)
        .await?
        .rows_affected();
    let captcha = sqlx::query("DELETE FROM captcha_seen_tokens WHERE seen_at < now() - interval '7 days'")
        .execute(pool)
        .await?
        .rows_affected();
    let refresh = sqlx::query(
        "DELETE FROM refresh_tokens WHERE \
            (revoked_at IS NOT NULL AND revoked_at < now() - interval '30 days') \
            OR expires_at < now() - interval '30 days'",
    )
    .execute(pool)
    .await?
    .rows_affected();
    let telemetry = sqlx::query("DELETE FROM system_error_logs WHERE last_seen_at < now() - interval '90 days'")
        .execute(pool)
        .await?
        .rows_affected();
    Ok(RetentionReport {
        otps,
        captcha,
        refresh,
        telemetry,
    })
}

/// Boot hook: backfill always; blank only with `PII_BLANK_EMAIL=true`.
pub async fn run_boot_privacy_jobs(pool: &PgPool, secrets: &SecretsService) -> Result<BackfillReport, PrivacyError> {
    let report = backfill_pii(pool, secrets).await?;
    if std::env::var("PII_BLANK_EMAIL").ok().as_deref() == Some("true") {
        let n = blank_plaintext_emails(pool).await?;
        tracing::info!(blanked = n, "pii email column blanked");
    }
    Ok(report)
}
