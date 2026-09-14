//! GET /v1/swap/quote (HOUSE via price_cache) + history 401/200 + order 404.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_quote_get_with_price_cache(pool: PgPool) {
    std::env::set_var("SWAP_HOUSE_ENABLED", "true");
    common::seed_price_cache(&pool).await;
    let state = common::test_state(pool);

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/quote?fromCoin=POL&toCoin=USDT&fromAmount=1000000")
                .header("x-real-ip", "203.0.113.60")
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
        "quote={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let routes = body["routes"].as_array().expect("routes");
    assert!(!routes.is_empty());
    assert!(routes.iter().any(|r| r["source"] == "house"));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_history_unauthorized_and_empty(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "swaph").await;

    let unauth = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/history")
                .header("x-real-ip", "203.0.113.61")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), axum::http::StatusCode::UNAUTHORIZED);

    let response = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/history")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.61")
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
        "history={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["swaps"].as_array().map(|a| a.len()), Some(0));

    let missing = Uuid::new_v4();
    let order = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/swap/orders/{missing}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.61")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(order.status(), axum::http::StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_quote_bad_query_and_post(pool: PgPool) {
    std::env::set_var("SWAP_HOUSE_ENABLED", "true");
    common::seed_price_cache(&pool).await;
    let state = common::test_state(pool);

    let bad = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/quote?fromCoin=POL&toCoin=POL&fromAmount=1")
                .header("x-real-ip", "203.0.113.62")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), axum::http::StatusCode::BAD_REQUEST);

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap/quote")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.62")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "500000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "post quote={}",
        String::from_utf8_lossy(&bytes)
    );
}
