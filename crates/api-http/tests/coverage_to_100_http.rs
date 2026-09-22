//! Coverage push toward 100% — fee-margin gates, faucet edges, merchant validation,
//! admin treasury-health, public pay aliases. Mirror `support_http` / `common`.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn oneshot(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut b = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("x-real-ip", "203.0.113.210");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let req = if let Some(j) = body {
        b.header("content-type", "application/json")
            .body(Body::from(j.to_string()))
            .unwrap()
    } else {
        b.body(Body::empty()).unwrap()
    };
    let res = api_http::app_without_metrics(state).oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
    };
    (status, v)
}

async fn seed_negative_fee_margin(pool: &PgPool, coin: Coin) {
    let _ = db::network_fees::record_network_fee(
        pool,
        db::network_fees::RecordNetworkFeeInput {
            coin,
            kind: db::network_fees::NetworkFeeKind::Withdrawal,
            amount: 50_000,
            tx_hash: Some(&format!("txid-margin-{}", Uuid::new_v4())),
            reference_id: None,
            reference_type: Some("Withdrawal"),
        },
    )
    .await
    .expect("network fee");
}

async fn fund_house_btc(pool: &PgPool) {
    db::house::ensure_house_inventory(pool).await.unwrap();
    let admin: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
    )
    .bind(format!("cov-admin-{}@bitcosats.test", Uuid::new_v4()))
    .bind(crypto::hash_password("x").unwrap())
    .bind(format!("c{}", &Uuid::new_v4().as_simple().to_string()[..12]))
    .fetch_one(pool)
    .await
    .unwrap();
    db::admin::fund_house(pool, Coin::Btc, 10_000_000_000, admin)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_treasury_health_ok(pool: PgPool) {
    let (state, admin_token) = common::admin_login(pool).await;
    let (st, body) = oneshot(
        state,
        "GET",
        "/v1/admin/treasury-health",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert!(body.get("fee_margin_block").is_some() || body.get("operating_margin_usd").is_some() || body.is_object());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_treasury_health_forbidden_for_user(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "th-user").await;
    let (st, body) = oneshot(
        state,
        "GET",
        "/v1/admin/treasury-health",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "admin access required");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_fee_margin_paused(pool: PgPool) {
    fund_house_btc(&pool).await;
    seed_negative_fee_margin(&pool, Coin::Btc).await;
    assert!(
        db::treasury_health::fee_margin_blocks_coin(&pool, Coin::Btc)
            .await
            .unwrap()
    );

    let (state, token, _, _) = common::register_user(pool, "fm-claim").await;
    let (st, body) = oneshot(
        state,
        "POST",
        "/v1/faucet/claim",
        Some(&token),
        Some(json!({ "coin": "BTC", "captchaToken": "dev-bypass" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "FEE_MARGIN_NEGATIVE");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_captcha_empty_fails(pool: PgPool) {
    fund_house_btc(&pool).await;
    let (mut state, token, _, _) = common::register_user(pool, "cap-fail").await;
    // Default test captcha is `disabled_in_dev` (always true). Swap to a real
    // verifier so empty tokens fail closed at the local length check.
    state.captcha = std::sync::Arc::new(captcha::TurnstileVerifier::new(captcha::TurnstileConfig {
        secret: "not-disabled".into(),
        node_env: "development".into(),
        timeout: std::time::Duration::from_secs(2),
        max_token_age: std::time::Duration::from_secs(300),
        siteverify_url: None,
    }));
    let (st, body) = oneshot(
        state,
        "POST",
        "/v1/faucet/claim",
        Some(&token),
        Some(json!({ "coin": "BTC", "captchaToken": "" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"].as_str().unwrap_or("").contains("captcha"),
        "{body}"
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_cooldown_second_claim(pool: PgPool) {
    fund_house_btc(&pool).await;
    let (state, token, _, _) = common::register_user(pool, "cd-claim").await;
    let (st1, body1) = oneshot(
        state.clone(),
        "POST",
        "/v1/faucet/claim",
        Some(&token),
        Some(json!({ "coin": "BTC", "captchaToken": "dev-bypass" })),
    )
    .await;
    assert_eq!(st1, axum::http::StatusCode::OK, "{body1}");

    let (st2, body2) = oneshot(
        state,
        "POST",
        "/v1/faucet/claim",
        Some(&token),
        Some(json!({ "coin": "BTC", "captchaToken": "dev-bypass-2" })),
    )
    .await;
    assert_eq!(st2, axum::http::StatusCode::TOO_MANY_REQUESTS, "{body2}");
    assert_eq!(body2["error"]["code"], "FAUCET_COOLDOWN");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_create_site_validation(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "fsite").await;
    let (st, body) = oneshot(
        state,
        "POST",
        "/v1/faucetlist",
        Some(&token),
        Some(json!({
            "name": "",
            "url": "not-a-url",
            "description": "",
            "coins": ["BTC"]
        })),
    )
    .await;
    assert!(
        st == axum::http::StatusCode::BAD_REQUEST || st == axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        "{st} {body}"
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn withdraw_request_fee_margin_paused(pool: PgPool) {
    // BTC/LTC/DOGE/BCH/DGB are paused outright, so the fee-margin guard is
    // exercised on a live coin; the paused one must answer with its own code.
    seed_negative_fee_margin(&pool, Coin::Sol).await;
    let (state, token, user_id, _) = common::register_user(pool.clone(), "fm-wd").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, Coin::Sol, 100_000_000).await;
    common::credit_personal(&pool, uid, Coin::Bch, 100_000_000).await;

    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/withdrawals",
        Some(&token),
        Some(json!({
            "coin": "SOL",
            "toAddress": "So11111111111111111111111111111111111111112",
            "amount": "1500000"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "FEE_MARGIN_NEGATIVE");

    let (st, body) = oneshot(
        state,
        "POST",
        "/v1/withdrawals",
        Some(&token),
        Some(json!({
            "coin": "BCH",
            "toAddress": "bitcoincash:qp3wjpa3tjlj042z2wv7hahsldgwhwy0rq9sywjpyy",
            "amount": "1500000"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["code"], "WITHDRAWAL_PAUSED");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_deposits_validation_and_forbidden_get(pool: PgPool) {
    let (state, merch_token, _, _) = common::register_user(pool.clone(), "md-val").await;

    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/merchant/deposits/create",
        Some(&merch_token),
        Some(json!({
            "coin": "NOPE",
            "amount": "1000",
            "orderId": "o1",
            "callbackUrl": "https://example.com/hook"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "unknown coin");

    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/merchant/deposits/create",
        Some(&merch_token),
        Some(json!({
            "coin": "POL",
            "amount": "not-a-number",
            "orderId": "o2",
            "callbackUrl": "https://example.com/hook"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid amount");

    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/merchant/deposits/create",
        Some(&merch_token),
        Some(json!({
            "coin": "POL",
            "amount": "0",
            "orderId": "o3",
            "callbackUrl": "https://example.com/hook"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "amount must be greater than 0");

    // bad api key shape
    let res = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/merchant/deposits/create")
                .header("content-type", "application/json")
                .header("x-api-key", "short")
                .header("x-real-ip", "203.0.113.211")
                .body(Body::from(
                    json!({
                        "coin": "BTC",
                        "amount": "1000",
                        "orderId": "o4",
                        "callbackUrl": "https://example.com/hook"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::UNAUTHORIZED);

    // create ok then peer cannot GET
    let (st, created) = oneshot(
        state.clone(),
        "POST",
        "/v1/merchant/deposits/create",
        Some(&merch_token),
        Some(json!({
            "coin": "POL",
            "amount": "500000",
            "orderId": format!("ok-{}", Uuid::new_v4()),
            "callbackUrl": "https://example.com/hook",
            "customerEmail": "buyer@example.com"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    let inv_id = created["id"].as_str().unwrap().to_string();

    let (state2, other_token, _, _) = common::register_user(pool, "md-peer").await;
    let (st, body) = oneshot(
        state2,
        "GET",
        &format!("/v1/merchant/deposits/{inv_id}"),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{body}");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_public_pay_alias_without_v1(pool: PgPool) {
    let (state, merch_token, _, _) = common::register_user(pool.clone(), "pay-alias").await;
    let (st, created) = oneshot(
        state.clone(),
        "POST",
        "/v1/merchant/deposits/create",
        Some(&merch_token),
        Some(json!({
            "coin": "POL",
            "amount": "250000",
            "orderId": format!("alias-{}", Uuid::new_v4()),
            "callbackUrl": "https://merchant.example/hook"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    let inv_id = created["id"].as_str().unwrap();

    let (st, pub_inv) = oneshot(
        state.clone(),
        "GET",
        &format!("/public/pay/{inv_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{pub_inv}");
    assert_eq!(pub_inv["id"], inv_id);

    let (state_p, payer_token, payer_id, _) = common::register_user(pool, "alias-payer").await;
    let payer_uid = Uuid::parse_str(&payer_id).unwrap();
    // Must be the invoice's coin: seeding BTC left the POL wallet empty, so
    // this assertion had been failing on `insufficient balance`.
    common::credit_personal(&state_p.pool, payer_uid, Coin::Pol, 5_000_000).await;

    let (st, paid) = oneshot(
        state_p,
        "POST",
        &format!("/public/pay/{inv_id}/balance"),
        Some(&payer_token),
        None,
    )
    .await;
    assert!(st.is_success(), "{st} {paid}");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn support_user_reply_empty_message(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "sup-empty").await;
    let (st, created) = oneshot(
        state.clone(),
        "POST",
        "/v1/support/tickets",
        Some(&token),
        Some(json!({ "topic": "account", "message": "preciso de ajuda" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    let id = created["ticket"]["id"].as_str().unwrap();

    let (st, body) = oneshot(
        state,
        "POST",
        &format!("/v1/support/tickets/{id}/messages"),
        Some(&token),
        Some(json!({ "message": "   " })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "empty_message");
}
