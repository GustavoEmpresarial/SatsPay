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
