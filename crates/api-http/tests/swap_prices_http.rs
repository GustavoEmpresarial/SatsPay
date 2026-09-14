//! GET /v1/swap/prices after seeding price_cache.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_prices_returns_seeded_map(pool: PgPool) {
    common::seed_price_cache(&pool).await;
    let state = common::test_state(pool);

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/prices")
                .header("x-real-ip", "203.0.113.20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "body={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["priceDecimals"], 8);
    assert!(body["prices"]["BTC"].as_str().is_some());
    assert_eq!(body["prices"]["USDT"], "100000000");
}
