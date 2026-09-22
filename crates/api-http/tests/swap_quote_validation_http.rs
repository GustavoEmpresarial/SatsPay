//! Swap quote validation / build_quotes early Err branches (no DEX needed).

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;

async fn get_quote(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    qs: &str,
) -> (axum::http::StatusCode, serde_json::Value) {
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/swap/quote?{qs}"))
                .header("x-real-ip", "203.0.113.270")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }));
    (status, v)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_quote_validation_edges(pool: PgPool) {
    let (state, _, _, _) = common::register_user(pool, "swq").await;

    // invalid parse
    let (st, body) = get_quote(state.clone(), "fromCoin=NOPE&toCoin=USDT&fromAmount=1").await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid query");

    let (st, body) = get_quote(state.clone(), "fromCoin=USDT&toCoin=USDC&fromAmount=abc").await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // same coin
    let (st, body) = get_quote(state.clone(), "fromCoin=USDT&toCoin=USDT&fromAmount=1000").await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"].as_str().unwrap_or("").contains("differ"),
        "{body}"
    );

    // zero amount
    let (st, body) = get_quote(state.clone(), "fromCoin=USDT&toCoin=USDC&fromAmount=0").await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"].as_str().unwrap_or("").contains("> 0"),
        "{body}"
    );

    // BTC↔LTC is a ChangeNOW L1 bridge now: without a provider it is "not configured", not blocked.
    let (st, body) = get_quote(state.clone(), "fromCoin=BTC&toCoin=LTC&fromAmount=1000").await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "SWAP_NOT_CONFIGURED", "{body}");

    // L2 pair without SwapKit configured → DEX not configured / no routes
    let (st, body) = get_quote(state, "fromCoin=USDT&toCoin=USDC&fromAmount=1000000").await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    let msg = body["error"].as_str().unwrap_or("");
    assert!(
        msg.contains("DEX not configured") || msg.contains("no routes"),
        "{body}"
    );
}
