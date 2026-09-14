//! First Axum oneshot: full `api_http::app` wiring responds on `/healthz`.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn healthz_returns_ok(pool: PgPool) {
    let app = api_http::app_without_metrics(common::test_state(pool));
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}
