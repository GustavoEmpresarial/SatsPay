//! Fixed-window, per-client-IP rate limiting.
//!
//! Sensitive classes (`auth-credentials`, `faucet-claim`) use a Postgres
//! bucket table so multi-replica deployments share one budget. Other classes
//! stay in-process (low cost, high volume).

use crate::client_ip::resolve_client_ip;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use domain::auth::AuthRepo;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

struct Limit {
    max: u32,
    window: Duration,
}

fn public_pay_suffix(path: &str) -> Option<&str> {
    path.strip_prefix("/v1/public/pay")
        .or_else(|| path.strip_prefix("/public/pay"))
}

fn classify(method: &axum::http::Method, path: &str) -> (&'static str, Limit) {
    const MIN: u64 = 60;
    if path == "/v1/auth/login" || path == "/v1/auth/register" || path == "/v1/auth/admin/login" {
        ("auth-credentials", Limit { max: 10, window: Duration::from_secs(5 * MIN) })
    } else if path.starts_with("/v1/faucet") && method == axum::http::Method::POST {
        ("faucet-claim", Limit { max: 20, window: Duration::from_secs(5 * MIN) })
    } else if (path == "/v1/telemetry/client-error" || path == "/v1/telemetry/client-errors")
        && method == axum::http::Method::POST
    {
        // Public ingest — tighter than generic /v1 to limit flood/abuse.
        ("telemetry-ingest", Limit { max: 60, window: Duration::from_secs(MIN) })
    } else if let Some(rest) = public_pay_suffix(path) {
        if method == axum::http::Method::POST && rest.ends_with("/balance") {
            ("public-pay-balance", Limit { max: 10, window: Duration::from_secs(MIN) })
        } else if method == axum::http::Method::POST && rest.ends_with("/select-coin") {
            ("public-pay-select", Limit { max: 20, window: Duration::from_secs(MIN) })
        } else if method == axum::http::Method::GET {
            ("public-pay-get", Limit { max: 60, window: Duration::from_secs(MIN) })
        } else {
            ("api", Limit { max: 2400, window: Duration::from_secs(5 * MIN) })
        }
    } else if path.starts_with("/v1/withdrawals") && method == axum::http::Method::POST {
        ("withdrawals-post", Limit { max: 120, window: Duration::from_secs(5 * MIN) })
    } else if path.starts_with("/v1/withdrawals") {
        ("withdrawals-get", Limit { max: 2400, window: Duration::from_secs(5 * MIN) })
    } else if path.starts_with("/v1/auth/") {
        ("auth-session", Limit { max: 2400, window: Duration::from_secs(5 * MIN) })
    } else if path.starts_with("/v1/") {
        ("api", Limit { max: 2400, window: Duration::from_secs(5 * MIN) })
    } else {
        ("other", Limit { max: 4800, window: Duration::from_secs(5 * MIN) })
    }
}

fn uses_shared_store(class: &str) -> bool {
    matches!(
        class,
        "auth-credentials"
            | "faucet-claim"
            | "telemetry-ingest"
            | "withdrawals-post"
            | "public-pay-select"
            | "public-pay-balance"
    )
}

struct Counters {
    hits: HashMap<String, (Instant, u32)>,
    last_prune: Instant,
}

const MAX_TRACKED_KEYS: usize = 100_000;
const PRUNE_EVERY: Duration = Duration::from_secs(60);

fn counters() -> &'static Mutex<Counters> {
    static COUNTERS: OnceLock<Mutex<Counters>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new(Counters { hits: HashMap::new(), last_prune: Instant::now() }))
}

fn check_local(key: String, limit: &Limit) -> Result<(), u64> {
    let now = Instant::now();
    let mut guard = match counters().lock() {
        Ok(g) => g,
        Err(poisoned) => {
            tracing::error!("rate limiter mutex poisoned; continuing with recovered state");
            poisoned.into_inner()
        }
    };

    if now.duration_since(guard.last_prune) > PRUNE_EVERY {
        let cutoff = limit.window;
        guard.hits.retain(|_, (start, _)| now.duration_since(*start) < cutoff);
        guard.last_prune = now;
    }
    if guard.hits.len() > MAX_TRACKED_KEYS {
        guard.hits.clear();
    }

    let entry = guard.hits.entry(key).or_insert((now, 0));
    if now.duration_since(entry.0) >= limit.window {
        *entry = (now, 0);
    }
    entry.1 += 1;
    if entry.1 > limit.max {
        let elapsed = now.duration_since(entry.0);
        return Err(limit.window.saturating_sub(elapsed).as_secs().max(1));
    }
    Ok(())
}

/// Atomic shared counter via Postgres upsert.
async fn check_shared(pool: &PgPool, key: &str, limit: &Limit) -> Result<(), u64> {
    let window_secs = limit.window.as_secs() as i64;
    let max = limit.max as i32;

    let row = sqlx::query_as::<_, (i32,)>(
        r#"
        INSERT INTO rate_limit_buckets (bucket_key, window_start, hits)
        VALUES ($1, now(), 1)
        ON CONFLICT (bucket_key) DO UPDATE SET
          hits = CASE
            WHEN rate_limit_buckets.window_start + make_interval(secs => $2) < now()
              THEN 1
            ELSE rate_limit_buckets.hits + 1
          END,
          window_start = CASE
            WHEN rate_limit_buckets.window_start + make_interval(secs => $2) < now()
              THEN now()
            ELSE rate_limit_buckets.window_start
          END
        RETURNING hits
        "#,
    )
    .bind(key)
    .bind(window_secs)
    .fetch_one(pool)
    .await;

    match row {
        Ok((hits,)) if hits > max => Err(window_secs.max(1) as u64),
        Ok(_) => Ok(()),
        Err(err) => {
            // Table missing / DB blip — fall back to local so we don't open the floodgates
            // completely, but also don't take the API down.
            tracing::warn!(error = %err, "shared rate limit failed; using in-process fallback");
            check_local(key.to_string(), limit)
        }
    }
}

pub async fn layer<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    req: Request,
    next: Next,
) -> Response {
    let (parts, body) = req.into_parts();
    let (class, limit) = classify(&parts.method, parts.uri.path());
    let ip = resolve_client_ip(&parts).unwrap_or_else(|| "unknown".to_string());
    let key = format!("{class}|{ip}");

    let limited = if uses_shared_store(class) {
        check_shared(&state.pool, &key, &limit).await
    } else {
        check_local(key, &limit)
    };

    if let Err(retry_after) = limited {
        tracing::warn!(class, ip, "rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", retry_after.to_string())],
            Json(json!({ "error": "too many requests", "code": "RATE_LIMITED" })),
        )
            .into_response();
    }

    next.run(Request::from_parts(parts, body)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;

    #[test]
    fn classify_auth_credentials_is_strict() {
        let (class, limit) = classify(&Method::POST, "/v1/auth/login");
        assert_eq!(class, "auth-credentials");
        assert_eq!(limit.max, 10);
        assert!(uses_shared_store(class));
        let (admin, admin_limit) = classify(&Method::POST, "/v1/auth/admin/login");
        assert_eq!(admin, "auth-credentials");
        assert_eq!(admin_limit.max, 10);
        assert!(uses_shared_store(admin));
    }

    #[test]
    fn classify_telemetry_ingest_is_strict_shared() {
        let (class, limit) = classify(&Method::POST, "/v1/telemetry/client-errors");
        assert_eq!(class, "telemetry-ingest");
        assert_eq!(limit.max, 60);
        assert!(uses_shared_store(class));
        let (class2, _) = classify(&Method::POST, "/v1/telemetry/client-error");
        assert_eq!(class2, "telemetry-ingest");
        // GET should not use ingest class
        let (class3, _) = classify(&Method::GET, "/v1/telemetry/client-errors");
        assert_ne!(class3, "telemetry-ingest");
    }

    #[test]
    fn classify_faucet_claim_is_moderate() {
        let (class, limit) = classify(&Method::POST, "/v1/faucet/claim/BTC");
        assert_eq!(class, "faucet-claim");
        assert_eq!(limit.max, 20);
        assert!(uses_shared_store(class));
    }

    #[test]
    fn classify_withdrawals_and_generic_api() {
        let (class, _) = classify(&Method::POST, "/v1/withdrawals");
        assert_eq!(class, "withdrawals-post");
        assert!(uses_shared_store(class));
        let (class, _) = classify(&Method::GET, "/v1/withdrawals");
        assert_eq!(class, "withdrawals-get");
        let (class, _) = classify(&Method::GET, "/v1/wallet");
        assert_eq!(class, "api");
    }

    #[test]
    fn check_local_blocks_after_max() {
        let limit = Limit {
            max: 3,
            window: Duration::from_secs(60),
        };
        let key = format!("test-key-{}", uuid::Uuid::new_v4());
        assert!(check_local(key.clone(), &limit).is_ok());
        assert!(check_local(key.clone(), &limit).is_ok());
        assert!(check_local(key.clone(), &limit).is_ok());
        let err = check_local(key, &limit).unwrap_err();
        assert!(err >= 1);
    }

    #[test]
    fn classify_auth_session_and_other() {
        let (class, _) = classify(&Method::GET, "/v1/auth/me");
        assert_eq!(class, "auth-session");
        let (class, _) = classify(&Method::GET, "/healthz");
        assert_eq!(class, "other");
    }

    #[test]
    fn classify_public_pay_is_tighter_than_generic_api() {
        let (class, limit) = classify(&Method::GET, "/v1/public/pay/550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(class, "public-pay-get");
        assert_eq!(limit.max, 60);
        assert!(!uses_shared_store(class));

        let (alias, _) = classify(&Method::GET, "/public/pay/demo");
        assert_eq!(alias, "public-pay-get");

        let (sel, sel_limit) = classify(&Method::POST, "/v1/public/pay/x/select-coin");
        assert_eq!(sel, "public-pay-select");
        assert_eq!(sel_limit.max, 20);
        assert!(uses_shared_store(sel));

        let (demo_sel, _) = classify(&Method::POST, "/public/pay/demo/select-coin");
        assert_eq!(demo_sel, "public-pay-select");

        let (bal, bal_limit) = classify(&Method::POST, "/v1/public/pay/x/balance");
        assert_eq!(bal, "public-pay-balance");
        assert_eq!(bal_limit.max, 10);
        assert!(uses_shared_store(bal));
    }

    #[test]
    fn check_local_resets_after_window() {
        let limit = Limit {
            max: 1,
            window: Duration::from_millis(30),
        };
        let key = format!("reset-key-{}", uuid::Uuid::new_v4());
        assert!(check_local(key.clone(), &limit).is_ok());
        assert!(check_local(key.clone(), &limit).is_err());
        std::thread::sleep(Duration::from_millis(40));
        assert!(check_local(key, &limit).is_ok());
    }
}
