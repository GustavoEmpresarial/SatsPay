//! Public API keys CRUD + balance + send + HMAC auth.

mod common;

use axum::body::Body;
use chrono::Utc;
use db::public_api::sign_request;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn credit_developer(pool: &PgPool, user_id: Uuid, amount: u64) {
    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'DEVELOPER') \
         ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    let wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = 'BTC' AND kind = 'DEVELOPER'",
    )
    .bind(user_id)
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
            memo: Some("dev seed"),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn api_keys_balance_send_and_hmac(pool: PgPool) {
    let (state, token, user_id, email) = common::register_user(pool.clone(), "papi").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    credit_developer(&state.pool, uid, 10_000_000).await;

    let (state2, _t2, _id2, email2) = common::register_user(pool.clone(), "recv").await;
    let _ = state2;

    // Issue key
    let issue = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/api-keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.50")
                .body(Body::from(
                    serde_json::json!({
                        "label": "oneshot",
                        "scopes": ["send", "*"],
                        "allowedIps": [],
                        "requireSignature": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = issue.status();
    let bytes = issue.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::CREATED, "issue={}", String::from_utf8_lossy(&bytes));
    let issued: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let key_id = issued["id"].as_str().expect("id");
    let raw_key = issued["key"].as_str().expect("key");
    assert_eq!(raw_key.len(), 64);

    let list = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/api-keys")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.50")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), axum::http::StatusCode::OK);

    // Balance via x-api-key
    let bal = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/public/balance")
                .header("x-api-key", raw_key)
                .header("x-real-ip", "203.0.113.50")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = bal.status();
    let bytes = bal.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "bal={}", String::from_utf8_lossy(&bytes));

    // Send
    let send_body = serde_json::json!({
        "coin": "BTC",
        "toEmail": email2,
        "amount": "100000",
        "idempotencyKey": format!("send-{}", Uuid::new_v4())
    })
    .to_string();
    let send = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/public/send")
                .header("content-type", "application/json")
                .header("x-api-key", raw_key)
                .header("x-real-ip", "203.0.113.50")
                .body(Body::from(send_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = send.status();
    let bytes = send.into_body().collect().await.unwrap().to_bytes();
    assert!(
        st.is_success(),
        "send={} body={}",
        st,
        String::from_utf8_lossy(&bytes)
    );

    // Same account that owns the key cannot be toEmail. Not a balance problem.
    let self_body = serde_json::json!({
        "coin": "BTC",
        "toEmail": email,
        "amount": "100000",
        "idempotencyKey": format!("self-{}", Uuid::new_v4())
    })
    .to_string();
    let self_send = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/public/send")
                .header("content-type", "application/json")
                .header("x-api-key", raw_key)
                .header("x-real-ip", "203.0.113.50")
                .body(Body::from(self_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let self_st = self_send.status();
    let self_bytes = self_send.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        self_st,
        axum::http::StatusCode::BAD_REQUEST,
        "self-send={}",
        String::from_utf8_lossy(&self_bytes)
    );
    let self_json: serde_json::Value = serde_json::from_slice(&self_bytes).unwrap();
    assert_eq!(self_json["code"], "SEND_TO_SELF");
    assert!(
        self_json["error"].as_str().unwrap_or("").contains("owns this API key"),
        "error={}",
        self_json
    );

    // Missing key → 401
    let no_key = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/public/balance")
                .header("x-real-ip", "203.0.113.50")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_key.status(), axum::http::StatusCode::UNAUTHORIZED);

    // HMAC key
    let issue2 = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/api-keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.50")
                .body(Body::from(
                    serde_json::json!({
                        "label": "hmac",
                        "scopes": ["*"],
                        "requireSignature": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = issue2.into_body().collect().await.unwrap().to_bytes();
    let hmac_issued: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let hmac_id = hmac_issued["id"].as_str().unwrap();
    let hmac_key = hmac_issued["key"].as_str().unwrap();
    let ts = Utc::now().timestamp().to_string();
    let path = "/v1/public/balance";
    let sig = sign_request(hmac_key, &ts, "GET", path, b"");
    let signed = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri(path)
                .header("x-key-id", hmac_id)
                .header("x-timestamp", &ts)
                .header("x-signature", &sig)
                .header("x-real-ip", "203.0.113.50")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        signed.status(),
        axum::http::StatusCode::OK,
        "hmac bal"
    );

    // Rotate + disable original key
    let rot = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/api-keys/{key_id}/rotate"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.50")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(rot.status().is_success());

    let dis = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/api-keys/{key_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.50")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        dis.status() == axum::http::StatusCode::NO_CONTENT || dis.status().is_success()
    );
    let _ = (email, Coin::Btc);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn issue_key_rejects_unknown_scope_and_balance_needs_scope(pool: PgPool) {
    let (state, token, _user_id, _email) = common::register_user(pool, "sc").await;
    let bad = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/api-keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({ "label": "x", "scopes": ["nfts"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), axum::http::StatusCode::BAD_REQUEST);

    let issue = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/api-keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({ "label": "dep", "scopes": ["deposits"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = issue.into_body().collect().await.unwrap().to_bytes();
    let issued: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let raw_key = issued["key"].as_str().expect("key");

    let bal = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/public/balance")
                .header("x-api-key", raw_key)
                .header("x-real-ip", "203.0.113.50")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bal.status(), axum::http::StatusCode::FORBIDDEN);
}
