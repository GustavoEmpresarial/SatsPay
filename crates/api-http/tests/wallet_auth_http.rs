//! Auth gate: protected routes reject missing/invalid bearer.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn wallet_without_bearer_is_401(pool: PgPool) {
    let app = api_http::app_without_metrics(common::test_state(pool));
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/wallet")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "unauthorized");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn wallet_with_garbage_bearer_is_401(pool: PgPool) {
    let app = api_http::app_without_metrics(common::test_state(pool));
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/wallet")
                .header("authorization", "Bearer not-a-jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
}
