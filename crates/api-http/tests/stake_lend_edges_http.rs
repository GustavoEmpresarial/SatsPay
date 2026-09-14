//! Stake claim + validation Err; lend supply/withdraw/borrow/repay DB Err + happy supply.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn post_json(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    uri: &str,
    token: &str,
    body: serde_json::Value,
) -> (axum::http::StatusCode, serde_json::Value) {
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.250")
                .body(Body::from(body.to_string()))
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
async fn stake_invalid_and_claim_paths(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let (state, token, user_id, _) = common::register_user(pool.clone(), "stk2").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, shared::Coin::Btc, 50_000_000).await;

    // invalid coin / amount
    let (st, body) = post_json(
        state.clone(),
        "/v1/stake",
        &token,
        json!({ "coin": "NOPE", "amount": "1", "lockDays": 0 }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    let (st, body) = post_json(
        state.clone(),
        "/v1/stake",
        &token,
        json!({ "coin": "BTC", "amount": "xyz", "lockDays": 0 }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // claim missing stake → BAD_REQUEST
    let missing = Uuid::new_v4();
    let (st, body) = post_json(
        state.clone(),
        &format!("/v1/stake/{missing}/claim"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // cancel missing
    let (st, body) = post_json(
        state.clone(),
        &format!("/v1/stake/{missing}/cancel"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // create + force mature + claim
    let (st, created) = post_json(
        state.clone(),
        "/v1/stake",
        &token,
        json!({ "coin": "BTC", "amount": "1000000", "lockDays": 0 }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    let stake_id = created["id"].as_str().expect("id");
    let sid = Uuid::parse_str(stake_id).unwrap();

    sqlx::query("UPDATE stakes SET matures_at = NOW() - INTERVAL '1 minute' WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .expect("mature stake");

    let (st, claimed) = post_json(
        state,
        &format!("/v1/stake/{stake_id}/claim"),
        &token,
        json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{claimed}");
    assert_eq!(claimed["status"].as_str().unwrap_or(""), "COMPLETED");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn lend_supply_happy_and_insufficient_ops(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    db::lend::ensure_lend_reserves(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;

    let (state, token, user_id, _) = common::register_user(pool.clone(), "lend2").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, shared::Coin::Btc, 5_000_000).await;

    // supply ok
    let (st, body) = post_json(
        state.clone(),
        "/v1/lend/supply/BTC",
        &token,
        json!({ "amount": "1000000" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    // withdraw all supplied, then withdraw again → DB Err → 400
    let (st, body) = post_json(
        state.clone(),
        "/v1/lend/withdraw/BTC",
        &token,
        json!({ "amount": "1000000" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let (st, body) = post_json(
        state.clone(),
        "/v1/lend/withdraw/BTC",
        &token,
        json!({ "amount": "1" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // borrow without collateral → 400
    let (st, body) = post_json(
        state.clone(),
        "/v1/lend/borrow/LTC",
        &token,
        json!({ "amount": "1000000" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // repay with nothing owed → 400
    let (st, body) = post_json(
        state.clone(),
        "/v1/lend/repay/BTC",
        &token,
        json!({ "amount": "1" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // supply with insufficient wallet → 400
    let (st, body) = post_json(
        state,
        "/v1/lend/supply/BTC",
        &token,
        json!({ "amount": "999999999999" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
}
