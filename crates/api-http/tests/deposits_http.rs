//! GET /v1/deposits/address/:coin + deposit history (stub chain).

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn deposit_address_and_history(pool: PgPool) {
    let state = common::test_state(pool);
    let email = format!("dep-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);

    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.28")
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
    assert_eq!(reg.status(), axum::http::StatusCode::CREATED);
    let bytes = reg.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["tokens"]["accessToken"].as_str().unwrap();

    let addr_resp = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/address/POL")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.28")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = addr_resp.status();
    let bytes = addr_resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "address={}",
        String::from_utf8_lossy(&bytes)
    );
    let addr: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!addr["address"].as_str().unwrap_or("").is_empty());

    let hist = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/history")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.28")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = hist.status();
    let bytes = hist.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "history={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["deposits"].as_array().is_some());
}
