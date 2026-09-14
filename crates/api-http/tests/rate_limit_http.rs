//! POST /v1/auth/login shared bucket → 429 after max hits.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn login_rate_limit_returns_429(pool: PgPool) {
    let state = common::test_state(pool);
    let ip = "198.51.100.50"; // TEST-NET-2, unique per test DB
    let body = serde_json::json!({
        "email": "nobody@bitcosats.test",
        "password": "wrong-password-xx",
        "captchaToken": "dev-bypass"
    })
    .to_string();

    let mut last_status = axum::http::StatusCode::OK;
    for _ in 0..11 {
        let response = api_http::app_without_metrics(state.clone())
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .header("x-real-ip", ip)
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        last_status = response.status();
        if last_status == axum::http::StatusCode::TOO_MANY_REQUESTS {
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(v["code"], "RATE_LIMITED");
            return;
        }
    }
    panic!("expected 429 within 11 login attempts, last={last_status}");
}
