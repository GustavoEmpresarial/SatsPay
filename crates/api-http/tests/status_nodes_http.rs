//! GET /v1/status/nodes — always JSON (degraded fallbacks if RPCs unreachable).

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn status_nodes_returns_health_fields(pool: PgPool) {
    let state = common::test_state(pool);

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/status/nodes")
                .header("x-real-ip", "203.0.113.22")
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
    for key in [
        "btc_rpc",
        "ltc_rpc",
        "doge_rpc",
        "polygon_rpc",
        "sol_rpc",
        "solana_pay",
        "bch_rpc",
    ] {
        assert!(body[key]["status"].as_str().is_some(), "missing {key}");
        assert!(body[key]["provider"].as_str().is_some(), "missing provider {key}");
    }
}
