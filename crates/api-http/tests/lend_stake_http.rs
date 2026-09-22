//! Lend markets/positions + stake strategies/list/create/cancel.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
#[ignore = "lend em manutenção (LEND_MAINTENANCE); reativar quando /lend reabrir — contrato atual em coverage_sweep_http::lend_is_in_maintenance_and_moves_no_money"]
async fn lend_markets_positions_and_bad_supply(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    db::lend::ensure_lend_reserves(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;

    let (state, token, _, _) = common::register_user(pool, "lend").await;

    let markets = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/lend/markets")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.70")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = markets.status();
    let bytes = markets.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "markets={}", String::from_utf8_lossy(&bytes));

    let positions = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/lend/positions")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.70")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(positions.status(), axum::http::StatusCode::OK);

    // Invalid coin → 400 (covers parse path without needing funded pool ops).
    let bad = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/lend/supply/NOTACOIN")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.70")
                .body(Body::from(r#"{"amount":"1"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), axum::http::StatusCode::BAD_REQUEST);

    for path in [
        "/v1/lend/withdraw/BTC",
        "/v1/lend/borrow/BTC",
        "/v1/lend/repay/BTC",
    ] {
        let resp = api_http::app_without_metrics(state.clone())
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-real-ip", "203.0.113.70")
                    .body(Body::from(r#"{"amount":"abc"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST, "{path}");
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn stake_strategies_create_list_cancel(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let (state, token, user_id, _) = common::register_user(pool.clone(), "stake").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, shared::Coin::Btc, 20_000_000).await;

    let strategies = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/stake/strategies")
                .header("x-real-ip", "203.0.113.71")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = strategies.status();
    let bytes = strategies.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "strategies={}",
        String::from_utf8_lossy(&bytes)
    );

    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/stake")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.71")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "BTC",
                        "amount": "5000000",
                        "lockDays": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = create.status();
    let bytes = create.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::CREATED,
        "create={}",
        String::from_utf8_lossy(&bytes)
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let stake_id = v["id"].as_str().unwrap();

    let list = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/stake")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.71")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), axum::http::StatusCode::OK);

    let cancel = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/stake/{stake_id}/cancel"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.71")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel.status(), axum::http::StatusCode::NO_CONTENT);
}
