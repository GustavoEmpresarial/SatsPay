//! Shared AppState builder for `api-http` Axum oneshot tests.

use api_http::{AppSettings, AppState};
use axum::body::Body;
use bigdecimal::BigDecimal;
use captcha::{TurnstileConfig, TurnstileVerifier};
use chain::ChainRegistry;
use crypto::{JwtService, SecretsService};
use db::auth::PgAuthRepo;
use domain::auth::{AuthConfig, AuthService, NoopEmailSender};
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use relay::RelayClient;
use changenow::ChangeNowClient;
use swapkit::SwapKitClient;
use tower::ServiceExt;
use uuid::Uuid;

/// Seed `price_cache` so GET /v1/swap/prices (and pricing-dependent routes) succeed.
#[allow(dead_code)] // used by swap_prices_http; each test binary compiles common separately
pub async fn seed_price_cache(pool: &PgPool) {
    for (coin, price) in [
        (Coin::Btc, 4_500_000_000_000u64),
        (Coin::Ltc, 8_000_000_000),
        (Coin::Doge, 15_000_000),
        (Coin::Bch, 45_000_000_000),
        (Coin::Pol, 45_000_000),
        (Coin::Dgb, 1_000_000),
        (Coin::Sol, 15_000_000_000),
        (Coin::Usdt, 100_000_000),
        (Coin::Usdc, 100_000_000),
        (Coin::Zer, 1_000_000),
        (Coin::Pepe, 400),
    ] {
        sqlx::query(
            "INSERT INTO price_cache (coin, price_scaled, price_decimals, fetched_at) \
             VALUES ($1::coin, $2, 8, now()) \
             ON CONFLICT (coin) DO UPDATE SET price_scaled = EXCLUDED.price_scaled, fetched_at = EXCLUDED.fetched_at",
        )
        .bind(coin.as_str())
        .bind(BigDecimal::from(price))
        .execute(pool)
        .await
        .expect("seed price");
    }
}

pub fn test_state(pool: PgPool) -> AppState<PgAuthRepo> {
    let jwt = JwtService::new("test-jwt-secret-for-http-oneshot!!").expect("test jwt");
    let config = AuthConfig {
        jwt_access_ttl_secs: 900,
        jwt_refresh_ttl_secs: 7 * 86_400,
        admin_emails: vec!["admin@bitcosats.test".into()],
        house_email: "house@bitcosats.internal".into(),
        otp_code_ttl_secs: 600,
        otp_resend_cooldown_secs: 30,
        otp_max_attempts: 5,
        refresh_reuse_grace_secs: 0,
    };
    let email: Arc<dyn domain::auth::EmailSender> = Arc::new(NoopEmailSender);
    let secrets = Arc::new(SecretsService::from_hex(&"ab".repeat(32)).unwrap());
    let auth = Arc::new(AuthService::new(
        Arc::new(PgAuthRepo::with_secrets(pool.clone(), secrets.clone())),
        jwt,
        secrets.clone(),
        config,
        email.clone(),
    ));
    let captcha = Arc::new(TurnstileVerifier::new(TurnstileConfig {
        secret: "disabled_in_dev".into(),
        node_env: "development".into(),
        timeout: Duration::from_secs(5),
        max_token_age: Duration::from_secs(300),
        siteverify_url: None,
    }));
    AppState {
        auth,
        email,
        pool,
        chain_registry: Arc::new(ChainRegistry::build("development", true).unwrap()),
        secrets,
        hot_mnemonic: std::env::var("HOT_MNEMONIC")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(Arc::<str>::from),
        captcha,
        settings: AppSettings {
            faucet_cooldown_minutes: 60,
            public_api_daily_send_limit: 100,
            captcha_expected_hostnames: vec!["localhost".into()],
            lend_close_factor_bps: 5000,
            price_max_stale: Duration::from_secs(86_400),
            public_api_signature_max_skew: Duration::from_secs(300),
            smtp_enabled: false,
            public_base_url: "https://www.satspay.pro".into(),
        },
        swapkit: Arc::new(SwapKitClient::from_env()),
        relay: Arc::new(RelayClient::from_env()),
        changenow: Arc::new(ChangeNowClient::from_env()),
    }
}

/// Like [`test_state`] but with a SwapKit client pointed at a mock base URL (enabled).
#[allow(dead_code)]
pub fn test_state_with_swapkit(pool: PgPool, swapkit_base: &str) -> AppState<PgAuthRepo> {
    let mut state = test_state(pool);
    state.swapkit = Arc::new(SwapKitClient::new_with_base(swapkit_base, Some("test-key".into())));
    state
}

#[allow(dead_code)]
pub fn test_state_with_relay(pool: PgPool, relay_base: &str) -> AppState<PgAuthRepo> {
    let mut state = test_state(pool);
    state.relay = Arc::new(RelayClient::new_for_tests(relay_base));
    state
}

/// Register a fresh user; returns `(state, access_token, user_id, email)`.
#[allow(dead_code)]
pub async fn register_user(
    pool: PgPool,
    prefix: &str,
) -> (AppState<PgAuthRepo>, String, String, String) {
    let state = test_state(pool);
    let email = format!("{prefix}-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.90")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "username": username,
                        "password": "Password1234",
                        "confirmPassword": "Password1234",
                        "acceptTerms": true,
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        reg.status(),
        axum::http::StatusCode::CREATED,
        "register failed"
    );
    let bytes = reg.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["tokens"]["accessToken"].as_str().unwrap().to_string();
    let user_id = v["user"]["id"].as_str().unwrap().to_string();
    (state, token, user_id, email)
}

/// Upsert `admin@bitcosats.test` and login via `/v1/auth/admin/login`.
/// Token is top-level `accessToken` (not nested under `tokens`).
#[allow(dead_code)]
pub async fn admin_login(pool: PgPool) -> (AppState<PgAuthRepo>, String) {
    let email = "admin@bitcosats.test".to_string();
    let username = format!("a{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let password = "Password1234";
    let hash = crypto::hash_password(password).unwrap();
    sqlx::query(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') \
         ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash, role = 'ADMIN'",
    )
    .bind(&email)
    .bind(&hash)
    .bind(&username)
    .execute(&pool)
    .await
    .unwrap();

    let state = test_state(pool);
    let login = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/admin/login")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.91")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "password": password,
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = login.status();
    let bytes = login.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "admin login={}",
        String::from_utf8_lossy(&bytes)
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["accessToken"]
        .as_str()
        .or_else(|| v["tokens"]["accessToken"].as_str())
        .expect("accessToken")
        .to_string();
    (state, token)
}

#[allow(dead_code)]
pub async fn credit_personal(pool: &PgPool, user_id: Uuid, coin: Coin, amount: u64) {
    let wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .fetch_one(pool)
    .await
    .expect("personal wallet");
    let mut tx = pool.begin().await.unwrap();
    db::ledger::apply_ledger_entry(
        &mut tx,
        db::ledger::LedgerCreditInput {
            reference_key: None,
            wallet_id,
            amount: BigDecimal::from(amount),
            ledger_type: "ADJUSTMENT",
            reference_id: None,
            reference_type: Some("HttpTestSeed"),
            memo: Some("seed"),
        },
    )
    .await
    .expect("credit");
    tx.commit().await.unwrap();
}

/// Credit HOUSE wallets so HOUSE swap execute has inventory.
#[allow(dead_code)]
pub async fn seed_house_liquidity(pool: &PgPool, coins: &[Coin], amount: u64) {
    for &coin in coins {
        let mut tx = pool.begin().await.unwrap();
        let wallet_id = db::house::get_house_wallet_id(&mut tx, coin)
            .await
            .expect("house wallet");
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput {
                reference_key: None,
                wallet_id,
                amount: BigDecimal::from(amount),
                ledger_type: "ADJUSTMENT",
                reference_id: None,
                reference_type: Some("HttpTestSeed"),
                memo: Some("house seed"),
            },
        )
        .await
        .expect("house credit");
        tx.commit().await.unwrap();
    }
}
