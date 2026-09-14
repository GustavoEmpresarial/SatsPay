//! HOUSE swap execute + history with rows + telemetry + order status branches.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_execute_house_history_telemetry(pool: PgPool) {
    std::env::set_var("SWAP_HOUSE_ENABLED", "true");
    db::house::ensure_house_inventory(&pool).await.expect("house");
    common::seed_price_cache(&pool).await;
    common::seed_house_liquidity(&pool, &[Coin::Pol, Coin::Usdt], 10_000_000_000_000).await;

    let (state, token, user_id, _) = common::register_user(pool.clone(), "swex").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, Coin::Pol, 50_000_000).await;

    let exec = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.63")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000000",
                        "idempotencyKey": "http-swap-1",
                        "source": "house",
                        "provider": "SatsPay Liquidity",
                        "routeId": "house:POL:USDT:1000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = exec.status();
    let bytes = exec.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "execute={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["source"], "house");
    assert_eq!(body["status"], "COMPLETED");
    let swap_id = body["id"].as_str().expect("swap id");

    // Idempotent replay via legacy path
    let replay = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap/execute")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.63")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000000",
                        "idempotencyKey": "http-swap-1",
                        "source": "house"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), axum::http::StatusCode::OK);
    let bytes = replay.into_body().collect().await.unwrap().to_bytes();
    let replay_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(replay_body["id"], swap_id);

    let history = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/history")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.63")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = history.status();
    let bytes = history.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let hist: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!hist["swaps"].as_array().unwrap().is_empty());

    // HOUSE swaps are not dex_swaps → order status 404
    let order = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/swap/orders/{swap_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.63")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(order.status(), axum::http::StatusCode::NOT_FOUND);

    // Non-admin telemetry → 403
    let tel_user = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/telemetry")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.63")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tel_user.status(), axum::http::StatusCode::FORBIDDEN);

    let (_a, admin_token) = common::admin_login(pool).await;
    let tel = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/telemetry")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-real-ip", "203.0.113.63")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = tel.status();
    let bytes = tel.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "telemetry={}",
        String::from_utf8_lossy(&bytes)
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_execute_bad_request_branches(pool: PgPool) {
    std::env::set_var("SWAP_HOUSE_ENABLED", "true");
    common::seed_price_cache(&pool).await;
    let (state, token, _, _) = common::register_user(pool, "swbad").await;

    let bad = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.64")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "NOPE",
                        "toCoin": "USDT",
                        "fromAmount": "1",
                        "idempotencyKey": "bad-1",
                        "source": "house"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), axum::http::StatusCode::BAD_REQUEST);

    let bad_min = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.64")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000",
                        "minToAmount": "not-a-number",
                        "idempotencyKey": "bad-2",
                        "source": "house"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_min.status(), axum::http::StatusCode::BAD_REQUEST);
}
