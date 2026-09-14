//! Cookie auth endpoints reject cross-site Origin (CSRF).

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn refresh_blocks_evil_origin(pool: PgPool) {
    let app = api_http::app_without_metrics(common::test_state(pool));
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/refresh")
                .header("content-type", "application/json")
                .header("origin", "https://evil.example")
                .header("cookie", "refresh_token=dummy")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::FORBIDDEN,
        "body={}",
        String::from_utf8_lossy(&bytes)
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["code"], "CSRF_BLOCKED");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn logout_blocks_evil_origin(pool: PgPool) {
    let app = api_http::app_without_metrics(common::test_state(pool));
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/logout")
                .header("content-type", "application/json")
                .header("origin", "https://phish.example")
                .header("x-real-ip", "203.0.113.14")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["code"], "CSRF_BLOCKED");
}
