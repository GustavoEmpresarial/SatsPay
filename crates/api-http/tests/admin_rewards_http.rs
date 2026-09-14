//! Admin rewards programs list/create/set-active — validation + happy paths.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn admin_req(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let b = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("x-real-ip", "203.0.113.260");
    let req = if let Some(j) = body {
        b.header("content-type", "application/json")
            .body(Body::from(j.to_string()))
            .unwrap()
    } else {
        b.body(Body::empty()).unwrap()
    };
    let res = api_http::app_without_metrics(state).oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
    };
    (status, v)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_rewards_programs_crud_edges(pool: PgPool) {
    let (state, admin_token) = common::admin_login(pool).await;

    let (st, list) = admin_req(
        state.clone(),
        "GET",
        "/v1/admin/rewards/programs",
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{list}");

    // invalid create
    let (st, body) = admin_req(
        state.clone(),
        "POST",
        "/v1/admin/rewards/programs",
        &admin_token,
        Some(json!({
            "rewardCoin": "NOPE",
            "marketCoin": "BTC",
            "side": "SUPPLY",
            "emissionPerDay": "1000"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    let (st, body) = admin_req(
        state.clone(),
        "POST",
        "/v1/admin/rewards/programs",
        &admin_token,
        Some(json!({
            "rewardCoin": "BTC",
            "marketCoin": "LTC",
            "side": "SUPPLY",
            "emissionPerDay": "not-a-number"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // create ok
    let (st, created) = admin_req(
        state.clone(),
        "POST",
        "/v1/admin/rewards/programs",
        &admin_token,
        Some(json!({
            "rewardCoin": "BTC",
            "marketCoin": "LTC",
            "side": "SUPPLY",
            "emissionPerDay": "1000"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    let id = created["id"]
        .as_str()
        .or_else(|| created["programId"].as_str())
        .expect("program id")
        .to_string();

    // deactivate
    let (st, body) = admin_req(
        state.clone(),
        "POST",
        &format!("/v1/admin/rewards/programs/{id}/active"),
        &admin_token,
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT, "{body}");

    // missing program → NOT_FOUND
    let missing = Uuid::new_v4();
    let (st, body) = admin_req(
        state,
        "POST",
        &format!("/v1/admin/rewards/programs/{missing}/active"),
        &admin_token,
        Some(json!({ "active": true })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND, "{body}");
}
