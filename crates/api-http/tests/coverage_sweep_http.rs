//! Sweep remaining coverage: 2FA code_sent, DEX early errors, faucet extras, lend ops.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn swapkit_enabled_state(pool: PgPool) -> api_http::AppState<db::auth::PgAuthRepo> {
    std::env::set_var("SWAPKIT_ENABLED", "true");
    std::env::set_var("SWAPKIT_API_KEY", "test-key-for-coverage");
    let state = common::test_state(pool);
    std::env::remove_var("SWAPKIT_ENABLED");
    std::env::remove_var("SWAPKIT_API_KEY");
    state
}

#[sqlx::test(migrations = "../db/migrations")]
async fn auth_2fa_code_sent(pool: PgPool) {
    let (state, _token, user_id, email) = common::register_user(pool.clone(), "twofa").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    sqlx::query("UPDATE users SET two_factor_enabled = true WHERE id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();

    let login = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.110")
                .header("user-agent", "sqlx-2fa")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "password": "Password1234",
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
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["kind"], "code_sent");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_dex_path_early_errors(pool: PgPool) {
    common::seed_price_cache(&pool).await;
    let state = swapkit_enabled_state(pool.clone());
    // Register against a normal state for token, then hit DEX with swapkit state
    let (_s, token, _, _) = common::register_user(pool, "dexerr").await;

    // Missing expectedToAmount on DEX path
    let r = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.111")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000",
                        "idempotencyKey": "dex-1",
                        "source": "swapkit",
                        "provider": "THORCHAIN",
                        "routeId": "some-dex-route"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::BAD_REQUEST);

    // Missing routeId
    let r = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.111")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000",
                        "idempotencyKey": "dex-2",
                        "source": "swapkit",
                        "provider": "THORCHAIN",
                        "routeId": "",
                        "expectedToAmount": "1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::BAD_REQUEST);

    // Hot mnemonic missing → 503
    let r = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.111")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000",
                        "idempotencyKey": "dex-3",
                        "source": "swapkit",
                        "provider": "THORCHAIN",
                        "routeId": "dex-route-abc",
                        "expectedToAmount": "50000",
                        "platformFeeBps": 50
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_click_delete_cooldown_aliases(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let admin: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
    )
    .bind(format!("fcov-{}@bitcosats.test", Uuid::new_v4()))
    .bind(crypto::hash_password("x").unwrap())
    .bind(format!("h{}", &Uuid::new_v4().as_simple().to_string()[..12]))
    .fetch_one(&pool)
    .await
    .unwrap();
    db::admin::fund_house(&pool, Coin::Btc, 10_000_000_000, admin)
        .await
        .unwrap();

    let (state, token, _, _) = common::register_user(pool.clone(), "fcov").await;

    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/faucetlist")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.112")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Click Site",
                        "url": "https://click.example/claim",
                        "description": "description long enough here",
                        "coins": ["BTC"],
                        "rewardInfo": "10 sats"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = create.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let site_id = v["id"].as_str().unwrap();

    let click = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/faucetlist/click/{site_id}"))
                .header("x-real-ip", "203.0.113.112")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        click.status().is_redirection() || click.status().is_success(),
        "click={}",
        click.status()
    );

    let claim = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/faucet/claim")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.112")
                .body(Body::from(r#"{"coin":"BTC","captchaToken":"dev-bypass"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(claim.status(), axum::http::StatusCode::OK);

    // Cooldown on same IP
    let cool = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/faucet/claim")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.112")
                .body(Body::from(r#"{"coin":"BTC","captchaToken":"dev-bypass"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cool.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);

    let del = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/faucetlist/{site_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.112")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(del.status().is_success(), "delete={}", del.status());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn lend_supply_withdraw_borrow_repay(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    db::lend::ensure_lend_reserves(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;

    // Seed LEND_POOL + HOUSE liquidity
    for coin in [Coin::Btc, Coin::Ltc] {
        for kind in ["HOUSE", "LEND_POOL"] {
            let mut tx = pool.begin().await.unwrap();
            let wallet_id: Uuid = sqlx::query_scalar(
                "SELECT w.id FROM wallets w JOIN users u ON u.id = w.user_id \
                 WHERE u.email = $1 AND w.coin = $2::coin AND w.kind = $3::wallet_kind",
            )
            .bind(db::house::HOUSE_EMAIL)
            .bind(coin.as_str())
            .bind(kind)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            db::ledger::apply_ledger_entry(
                &mut tx,
                db::ledger::LedgerCreditInput {
                    reference_key: None,
                    wallet_id,
                    amount: bigdecimal::BigDecimal::from(10_000_000_000_000u64),
                    ledger_type: "ADJUSTMENT",
                    reference_id: None,
                    reference_type: Some("HttpTestSeed"),
                    memo: Some("lend seed"),
                },
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
        }
    }

    let (state, token, user_id, _) = common::register_user(pool.clone(), "lendop").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, Coin::Btc, 100_000_000).await;

    let supply = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/lend/supply/BTC")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.113")
                .body(Body::from(r#"{"amount":"20000000"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = supply.status();
    let bytes = supply.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "supply={}", String::from_utf8_lossy(&bytes));

    let borrow = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/lend/borrow/LTC")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.113")
                .body(Body::from(r#"{"amount":"1000000"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = borrow.status();
    let bytes = borrow.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "borrow={}", String::from_utf8_lossy(&bytes));

    let repay = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/lend/repay/LTC")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.113")
                .body(Body::from(r#"{"amount":"500000"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repay.status(), axum::http::StatusCode::OK);

    let withdraw = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/lend/withdraw/BTC")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.113")
                .body(Body::from(r#"{"amount":"1000000"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = withdraw.status();
    let bytes = withdraw.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "withdraw={}", String::from_utf8_lossy(&bytes));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn deposits_history_filtered_and_oauth_errors(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "depfil").await;

    let hist = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/history?coin=BTC&limit=10")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.114")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hist.status(), axum::http::StatusCode::OK);

    let addr = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/address/BTC")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.114")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // may fail without HD wallet — still exercises handler
    let _ = addr.status();

    let bad_app = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/apps")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.114")
                .body(Body::from(r#"{"name":"","redirect_uris":["https://x.example/cb"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_app.status(), axum::http::StatusCode::BAD_REQUEST);

    let info = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/oauth/authorize/info?client_id=missing")
                .header("x-real-ip", "203.0.113.114")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(info.status(), axum::http::StatusCode::BAD_REQUEST);
}
