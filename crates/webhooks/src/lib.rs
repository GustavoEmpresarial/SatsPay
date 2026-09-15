//! Signed merchant webhooks for the SatsPay deposit gateway.
//!
//! Lives in its own crate because both sides of the gateway dispatch them:
//! `api-http` (checkout paid with an internal balance, plus the dashboard's
//! "send test webhook") and `worker` (invoice confirmed on-chain, plus
//! redelivery). `worker` does not depend on `api-http`.
//!
//! Contract — this is what `/docs` documents, and the two must not drift:
//!
//! ```text
//! POST <callback_url>
//! Content-Type: application/json
//! X-SatsPay-Signature: sha256=<hex HMAC-SHA256 of the raw body>
//! X-SatsPay-Event: deposit.confirmed
//! X-SatsPay-Timestamp: <unix seconds>      (also inside the signed body)
//! X-SatsPay-Delivery: <uuid, unique per attempt>
//! ```
//!
//! The signature covers the raw body, and the body carries `timestamp`, so a
//! receiver gets replay protection without a second signing scheme: reject
//! bodies whose `timestamp` is more than `MAX_WEBHOOK_AGE` old.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use crypto::SecretsService;
use db::merchant_deposits::{record_webhook_delivery, schedule_webhook_retry, MerchantDepositInvoice};
use serde_json::json;
use std::net::IpAddr;
use std::time::Duration;
use uuid::Uuid;

/// The only event the gateway emits today.
pub const EVENT_DEPOSIT_CONFIRMED: &str = "deposit.confirmed";

/// Attempts (including the first) before an undelivered webhook is parked.
pub const MAX_WEBHOOK_ATTEMPTS: i32 = 8;

/// How stale a signed body may be before a receiver should reject it.
/// Documented on `/docs`; kept here so the number has one home.
pub const MAX_WEBHOOK_AGE: Duration = Duration::from_secs(300);

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackUrlError {
    NotAUrl,
    NotHttps,
    NoHost,
    PrivateHost,
}

impl std::fmt::Display for CallbackUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            CallbackUrlError::NotAUrl => "callbackUrl is not a valid URL",
            CallbackUrlError::NotHttps => "callbackUrl must use https",
            CallbackUrlError::NoHost => "callbackUrl has no host",
            CallbackUrlError::PrivateHost => "callbackUrl must point at a public host",
        };
        f.write_str(msg)
    }
}

/// Outcome of one delivery attempt.
#[derive(Debug, Clone)]
pub struct DeliveryOutcome {
    pub delivered: bool,
    pub status_code: Option<i32>,
    pub error: Option<String>,
}

/// True when the address must never be dialled from the server: loopback,
/// RFC1918, link-local (incl. 169.254.169.254 cloud metadata), CGNAT,
/// multicast, unspecified, and the IPv6 equivalents.
///
/// Stricter than `api-http`'s inbound `is_internal_or_private`, which only
/// has to decide whether a *claimed* client IP is plausible; here a wrong
/// answer means the server makes the request an attacker asked for (SSRF).
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || v4.is_unspecified()
                // 100.64.0.0/10 carrier-grade NAT
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
                // 0.0.0.0/8 "this network"
                || v4.octets()[0] == 0
                // 192.0.0.0/24 IETF protocol assignments
                || (v4.octets()[0] == 192 && v4.octets()[1] == 0 && v4.octets()[2] == 0)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 unique local
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // fe80::/10 link local
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped — judge by the embedded v4 address
                || v6.to_ipv4_mapped().map(|v4| is_blocked_ip(IpAddr::V4(v4))).unwrap_or(false)
        }
    }
}

/// Syntactic half of the SSRF gate: https only, real host, and no literal
/// private/loopback address. Pure, so it can run at invoice-creation time to
/// reject a bad `callbackUrl` before an invoice exists.
pub fn validate_callback_url(raw: &str) -> Result<(), CallbackUrlError> {
    let url = url::Url::parse(raw).map_err(|_| CallbackUrlError::NotAUrl)?;
    if url.scheme() != "https" {
        return Err(CallbackUrlError::NotHttps);
    }
    let host = url.host_str().ok_or(CallbackUrlError::NoHost)?;
    if host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".localhost") {
        return Err(CallbackUrlError::PrivateHost);
    }
    if let Ok(ip) = host.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>() {
        if is_blocked_ip(ip) {
            return Err(CallbackUrlError::PrivateHost);
        }
    }
    Ok(())
}

/// Resolution half of the SSRF gate: every address the host resolves to must
/// be publicly routable. Run immediately before dialling.
async fn resolve_is_public(url: &url::Url) -> Result<(), CallbackUrlError> {
    let host = url.host_str().ok_or(CallbackUrlError::NoHost)?;
    if host.parse::<IpAddr>().is_ok() {
        return Ok(()); // already checked syntactically
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let mut resolved = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| CallbackUrlError::NoHost)?
        .peekable();
    let mut any = false;
    for addr in resolved.by_ref() {
        any = true;
        if is_blocked_ip(addr.ip()) {
            return Err(CallbackUrlError::PrivateHost);
        }
    }
    if any {
        Ok(())
    } else {
        Err(CallbackUrlError::NoHost)
    }
}

/// Exponential backoff with a 1h ceiling: 30s, 1m, 2m, 4m … per attempt.
pub fn next_retry_delay(attempts_so_far: i32) -> Duration {
    let exp = attempts_so_far.clamp(0, 8) as u32;
    Duration::from_secs((30u64 << exp).min(3600))
}

/// The signed body. Pure so tests (and the docs page) can assert its exact
/// shape without a database or a network.
pub fn invoice_payload(inv: &MerchantDepositInvoice, attempt: i32, now: DateTime<Utc>) -> serde_json::Value {
    json!({
        "event": EVENT_DEPOSIT_CONFIRMED,
        "invoiceId": inv.id,
        "orderId": inv.order_id,
        "siteUserId": inv.site_user_id,
        "coin": inv.coin,
        "amount": inv.amount.to_string(),
        "fee": inv.fee_amount.to_string(),
        "netAmount": inv.net_amount.to_string(),
        "txHash": inv.tx_hash.as_deref().unwrap_or("internal_satspay"),
        "status": inv.status,
        "paidAt": inv.paid_at.unwrap_or(now),
        "customerEmail": inv.customer_email,
        // Replay protection: signed, and the receiver compares it to its own
        // clock (see MAX_WEBHOOK_AGE).
        "timestamp": now.timestamp(),
        "attempt": attempt,
    })
}

/// Signs and POSTs one delivery attempt, records the outcome, and schedules
/// the next retry (or clears the schedule on success / exhaustion).
pub async fn dispatch_invoice_webhook(
    pool: &sqlx::PgPool,
    inv: &MerchantDepositInvoice,
    secrets: &SecretsService,
) -> DeliveryOutcome {
    let attempt = inv.webhook_attempts + 1;
    let now = Utc::now();
    let delivery_id = Uuid::new_v4();
    let payload_str = invoice_payload(inv, attempt, now).to_string();
    let signature = secrets.sign_webhook_payload(&inv.merchant_id.to_string(), &payload_str);

    let outcome = post_signed(&inv.callback_url, &payload_str, &signature, now, delivery_id).await;

    record_webhook_delivery(pool, inv.id, outcome.delivered, outcome.status_code, outcome.error.as_deref())
        .await
        .ok();

    let next = if outcome.delivered || attempt >= MAX_WEBHOOK_ATTEMPTS {
        None
    } else {
        Some(now + ChronoDuration::from_std(next_retry_delay(attempt)).unwrap_or_else(|_| ChronoDuration::seconds(30)))
    };
    schedule_webhook_retry(pool, inv.id, next).await.ok();

    if !outcome.delivered {
        tracing::warn!(
            invoice_id = %inv.id,
            merchant_id = %inv.merchant_id,
            attempt,
            max_attempts = MAX_WEBHOOK_ATTEMPTS,
            status = ?outcome.status_code,
            error = ?outcome.error,
            exhausted = attempt >= MAX_WEBHOOK_ATTEMPTS,
            "WEBHOOK_DELIVERY_FAILED"
        );
    }

    outcome
}

async fn post_signed(
    callback_url: &str,
    payload_str: &str,
    signature: &str,
    now: DateTime<Utc>,
    delivery_id: Uuid,
) -> DeliveryOutcome {
    let url = match url::Url::parse(callback_url) {
        Ok(u) => u,
        Err(_) => return failed(CallbackUrlError::NotAUrl.to_string()),
    };
    if let Err(e) = validate_callback_url(callback_url) {
        return failed(e.to_string());
    }
    if let Err(e) = resolve_is_public(&url).await {
        return failed(e.to_string());
    }

    let client = match reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        // A redirect could otherwise walk us from a public host into the
        // internal network after the SSRF check passed.
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => return failed(e.to_string()),
    };

    match client
        .post(url)
        .header("Content-Type", "application/json")
        .header("X-SatsPay-Signature", format!("sha256={signature}"))
        .header("X-SatsPay-Event", EVENT_DEPOSIT_CONFIRMED)
        .header("X-SatsPay-Timestamp", now.timestamp().to_string())
        .header("X-SatsPay-Delivery", delivery_id.to_string())
        .body(payload_str.to_string())
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16() as i32;
            let delivered = resp.status().is_success();
            DeliveryOutcome {
                delivered,
                status_code: Some(status),
                error: if delivered { None } else { Some(format!("HTTP status {status}")) },
            }
        }
        Err(e) => failed(e.to_string()),
    }
}

fn failed(error: String) -> DeliveryOutcome {
    DeliveryOutcome { delivered: false, status_code: None, error: Some(error) }
}

#[cfg(test)]
mod tests;
