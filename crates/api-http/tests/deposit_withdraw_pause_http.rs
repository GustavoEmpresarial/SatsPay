//! Coin pause list: BTC/LTC/DOGE/BCH/DGB block personal deposit address,
//! withdrawals, and merchant deposit gateway — but NOT `/v1/public/send`.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use shared::{is_deposit_withdraw_paused, Coin, DEPOSIT_WITHDRAW_PAUSED_COINS};
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
        .header("x-real-ip", "203.0.113.240");
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
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    (st, v)
}

#[test]
fn pause_list_matches_shared_constant() {
    assert_eq!(
        DEPOSIT_WITHDRAW_PAUSED_COINS,
        [Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Bch, Coin::Dgb]
    );
    for c in DEPOSIT_WITHDRAW_PAUSED_COINS {
        assert!(is_deposit_withdraw_paused(c));
    }
    assert!(!is_deposit_withdraw_paused(Coin::Zer));
    assert!(!is_deposit_withdraw_paused(Coin::Pol));
    assert!(!is_deposit_withdraw_paused(Coin::Usdt));
    assert!(!is_deposit_withdraw_paused(Coin::Sol));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn personal_deposit_address_paused_coins_return_503(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "pause-dep").await;

    for coin in ["BTC", "LTC", "DOGE", "BCH", "DGB"] {
        let (st, body) = oneshot(
            state.clone(),
            "GET",
            &format!("/v1/deposits/address/{coin}"),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::SERVICE_UNAVAILABLE, "{coin} {body}");
        assert_eq!(body["code"], "DEPOSIT_PAUSED", "{coin}");
        assert_eq!(body["coin"], coin);
    }

    for coin in ["POL", "SOL", "ZER"] {
        let (st, body) = oneshot(
            state.clone(),
            "GET",
            &format!("/v1/deposits/address/{coin}"),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::OK, "{coin} {body}");
        assert!(!body["address"].as_str().unwrap_or("").is_empty(), "{coin}");
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn personal_withdrawal_paused_coins_return_503(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "pause-wd").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    for coin in [Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Bch, Coin::Dgb] {
        common::credit_personal(&pool, uid, coin, 50_000_000).await;
    }

    for (coin, addr) in [
        ("BTC", "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"),
        ("LTC", "ltc1qabcdefghijklmnopqrstuvwxyz0123456789abcd"),
        ("DOGE", "DDogepartyxxxxxxxxxxxxxxxxxxxxxxxxx"),
        ("BCH", "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a"),
        ("DGB", "DXYZabcdefghijklmnopqrstuvwxyz012345"),
    ] {
        let (st, body) = oneshot(
            state.clone(),
            "POST",
            "/v1/withdrawals",
            Some(&token),
            Some(json!({
                "coin": coin,
                "toAddress": addr,
                "amount": "1000000"
            })),
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::SERVICE_UNAVAILABLE, "{coin} {body}");
        assert_eq!(body["code"], "WITHDRAWAL_PAUSED", "{coin}");
        assert_eq!(body["coin"], coin);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_gateway_paused_but_pol_ok(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "pause-mgw").await;

    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/merchant/deposits/create",
        Some(&token),
        Some(json!({
            "coin": "BTC",
            "amount": "100000",
            "orderId": format!("paused-{}", Uuid::new_v4()),
            "callbackUrl": "https://merchant.example/hook"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["code"], "DEPOSIT_PAUSED");

    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/merchant/deposits/create",
        Some(&token),
        Some(json!({
            "coin": "POL",
            "amount": "100000",
            "orderId": format!("ok-{}", Uuid::new_v4()),
            "callbackUrl": "https://merchant.example/hook",
            "siteName": "Pause Shop"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{body}");
    assert!(body["id"].as_str().is_some());

    let (st, body) = oneshot(
        state,
        "POST",
        "/v1/merchant/deposits/create",
        Some(&token),
        Some(json!({
            "coin": "DGB",
            "amount": "100000",
            "orderId": format!("paused-dgb-{}", Uuid::new_v4()),
            "callbackUrl": "https://merchant.example/hook",
            "siteName": "Pause Shop DGB"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["code"], "DEPOSIT_PAUSED");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn public_send_still_allows_paused_coins(pool: PgPool) {
    // Merchant ledger → user email must keep working for BTC while on-chain
    // deposit/withdraw gateway is paused.
    let (state, token, user_id, email) = common::register_user(pool.clone(), "pause-send").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    // Developer wallet for public send (same pattern as public_api_http).
    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'DEVELOPER') \
         ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    let wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = 'BTC' AND kind = 'DEVELOPER'",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput {
                reference_key: None,
                wallet_id,
                amount: bigdecimal::BigDecimal::from(10_000_000u64),
                ledger_type: "ADJUSTMENT",
                reference_id: None,
                reference_type: Some("HttpTestSeed"),
                memo: Some("dev seed"),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let (_s2, _t2, _id2, email2) = common::register_user(pool.clone(), "pause-recv").await;

    let issue = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/api-keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.241")
                .body(Body::from(
                    json!({
                        "label": "send-pause",
                        "scopes": ["*"],
                        "requireSignature": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(issue.status(), axum::http::StatusCode::CREATED);
    let bytes = issue.into_body().collect().await.unwrap().to_bytes();
    let issued: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let raw_key = issued["key"].as_str().unwrap();

    let send = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/public/send")
                .header("content-type", "application/json")
                .header("x-api-key", raw_key)
                .header("x-real-ip", "203.0.113.241")
                .body(Body::from(
                    json!({
                        "coin": "BTC",
                        "toEmail": email2,
                        "amount": "100000",
                        "idempotencyKey": format!("pause-send-{}", Uuid::new_v4())
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = send.status();
    let bytes = send.into_body().collect().await.unwrap().to_bytes();
    assert!(
        st.is_success(),
        "public send must stay open for paused coins; from={email} st={st} body={}",
        String::from_utf8_lossy(&bytes)
    );
}
