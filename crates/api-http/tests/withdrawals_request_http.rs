//! POST /v1/withdrawals + history alias (list already covered elsewhere).

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn withdrawal_request_and_history_alias(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "wdreq").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, shared::Coin::Pol, 100_000_000).await;

    let bad_addr = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/withdrawals")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.95")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "toAddress": "not-a-pol-address",
                        "amount": "1000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_addr.status(), axum::http::StatusCode::BAD_REQUEST);

    let ok = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/withdrawals")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.95")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "toAddress": "0x1111111111111111111111111111111111111111",
                        "amount": "1500000",
                        "idempotencyKey": "wd-oneshot-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = ok.status();
    let bytes = ok.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::CREATED,
        "wd={}",
        String::from_utf8_lossy(&bytes)
    );

    let hist = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/withdrawals/history?coin=POL&limit=10")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.95")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = hist.status();
    let bytes = hist.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "hist={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!body["withdrawals"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn withdrawal_request_unauthorized(pool: PgPool) {
    let state = common::test_state(pool);
    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/withdrawals")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.96")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "BTC",
                        "toAddress": "bc1qsomevaliddestinationaddressxxxxxxxxxxxxxxx",
                        "amount": "1000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn withdrawal_request_smtp_requires_otp(pool: PgPool) {
    let (mut state, token, user_id, _) = common::register_user(pool.clone(), "wdotp").await;
    state.settings.smtp_enabled = true;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, shared::Coin::Pol, 100_000_000).await;

    let challenge = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/withdrawals")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.97")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "toAddress": "0x1111111111111111111111111111111111111111",
                        "amount": "1500000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = challenge.status();
    let bytes = challenge.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["codeSent"], true);

    let hash = crypto::hash_password("123456").unwrap();
    sqlx::query(
        "INSERT INTO email_otps (user_id, purpose, code_hash, expires_at) \
         VALUES ($1, 'WITHDRAWAL', $2, now() + interval '10 minutes')",
    )
    .bind(uid)
    .bind(&hash)
    .execute(&pool)
    .await
    .unwrap();

    let ok = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/withdrawals")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.97")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "toAddress": "0x1111111111111111111111111111111111111111",
                        "amount": "1500000",
                        "emailCode": "123456",
                        "idempotencyKey": "wd-otp-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = ok.status();
    let bytes = ok.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::CREATED,
        "wd otp={}",
        String::from_utf8_lossy(&bytes)
    );
}

/// Seeds a MERCHANT wallet with `amount` and returns its id. There is no
/// `credit_merchant` helper — registration only creates PERSONAL wallets.
async fn seed_merchant(pool: &PgPool, user_id: Uuid, coin: shared::Coin, amount: u64) -> Uuid {
    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, 'MERCHANT') \
         ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .execute(pool)
    .await
    .unwrap();
    let wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'MERCHANT'",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .fetch_one(pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    db::ledger::apply_ledger_entry(
        &mut tx,
        db::ledger::LedgerCreditInput {
            reference_key: None,
            wallet_id,
            amount: bigdecimal::BigDecimal::from(amount),
            ledger_type: "ADJUSTMENT",
            reference_id: None,
            reference_type: Some("HttpTestSeed"),
            memo: Some("merchant seed"),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    wallet_id
}

async fn ledger_balance(pool: &PgPool, wallet_id: Uuid) -> bigdecimal::BigDecimal {
    sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM ledger_entries WHERE wallet_id = $1")
        .bind(wallet_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn withdraw_req(token: &str, body: serde_json::Value) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method("POST")
        .uri("/v1/withdrawals")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-real-ip", "203.0.113.98")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Merchant caixa is business float: it must be moved to PERSONAL before it can
/// leave the platform, so the on-chain path refuses it outright and debits
/// nothing. A stale SPA still posts `walletKind: MERCHANT`, hence the server gate.
#[sqlx::test(migrations = "../db/migrations")]
async fn withdrawal_from_merchant_wallet_is_blocked(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "wdmerch").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    let merchant_wallet = seed_merchant(&pool, uid, shared::Coin::Pol, 100_000_000).await;
    let before = ledger_balance(&pool, merchant_wallet).await;

    let blocked = api_http::app_without_metrics(state.clone())
        .oneshot(withdraw_req(
            &token,
            serde_json::json!({
                "coin": "POL",
                "toAddress": "0x1111111111111111111111111111111111111111",
                "amount": "1500000",
                "walletKind": "MERCHANT"
            }),
        ))
        .await
        .unwrap();
    let st = blocked.status();
    let bytes = blocked.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{}", String::from_utf8_lossy(&bytes));
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "WITHDRAWAL_MERCHANT_BLOCKED");

    assert_eq!(
        ledger_balance(&pool, merchant_wallet).await,
        before,
        "blocked attempt must not touch the merchant ledger"
    );

    // `kind` is a serde alias for the same field, so it is gated too.
    let aliased = api_http::app_without_metrics(state.clone())
        .oneshot(withdraw_req(
            &token,
            serde_json::json!({
                "coin": "POL",
                "toAddress": "0x1111111111111111111111111111111111111111",
                "amount": "1500000",
                "kind": "merchant"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(aliased.status(), axum::http::StatusCode::FORBIDDEN);

    let garbage = api_http::app_without_metrics(state)
        .oneshot(withdraw_req(
            &token,
            serde_json::json!({
                "coin": "POL",
                "toAddress": "0x1111111111111111111111111111111111111111",
                "amount": "1500000",
                "walletKind": "LIXO"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(garbage.status(), axum::http::StatusCode::BAD_REQUEST);
}

/// The block runs before the OTP step-up, so a blocked attempt never triggers a
/// code email.
#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_block_precedes_otp_challenge(pool: PgPool) {
    let (mut state, token, user_id, _) = common::register_user(pool.clone(), "wdmerchotp").await;
    state.settings.smtp_enabled = true;
    let uid = Uuid::parse_str(&user_id).unwrap();
    seed_merchant(&pool, uid, shared::Coin::Pol, 100_000_000).await;

    let res = api_http::app_without_metrics(state)
        .oneshot(withdraw_req(
            &token,
            serde_json::json!({
                "coin": "POL",
                "toAddress": "0x1111111111111111111111111111111111111111",
                "amount": "1500000",
                "walletKind": "MERCHANT"
            }),
        ))
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{}", String::from_utf8_lossy(&bytes));

    let otps: i64 = sqlx::query_scalar("SELECT count(*) FROM email_otps WHERE user_id = $1")
        .bind(uid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(otps, 0, "blocked attempt must not send an OTP");
}

/// An explicit `PERSONAL` keeps working — the field itself is not rejected.
#[sqlx::test(migrations = "../db/migrations")]
async fn withdrawal_with_explicit_personal_wallet_kind_succeeds(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "wdpers").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, shared::Coin::Pol, 100_000_000).await;

    let res = api_http::app_without_metrics(state)
        .oneshot(withdraw_req(
            &token,
            serde_json::json!({
                "coin": "POL",
                "toAddress": "0x1111111111111111111111111111111111111111",
                "amount": "1500000",
                "walletKind": "PERSONAL",
                "idempotencyKey": "wd-personal-1"
            }),
        ))
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::CREATED, "{}", String::from_utf8_lossy(&bytes));
}
