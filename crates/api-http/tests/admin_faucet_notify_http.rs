//! Admin faucetlist approve/reject/suspend — hits owner-email + send_best_effort paths.

mod common;

use axum::body::Body;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn post(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    uri: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> axum::http::StatusCode {
    let b = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("x-real-ip", "203.0.113.230");
    let req = if let Some(j) = body {
        b.header("content-type", "application/json")
            .body(Body::from(j.to_string()))
            .unwrap()
    } else {
        b.body(Body::empty()).unwrap()
    };
    api_http::app_without_metrics(state)
        .oneshot(req)
        .await
        .unwrap()
        .status()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_faucetlist_approve_reject_suspend_notify(pool: PgPool) {
    let (_state, _owner_token, owner_id, _) = common::register_user(pool.clone(), "fs-owner").await;
    let owner = Uuid::parse_str(&owner_id).unwrap();

    // PENDING row so approve hits Ok + owner-email notify path
    let pending_id: Uuid = sqlx::query_scalar(
        "INSERT INTO faucet_sites (owner_id, name, url, description, coins, status) \
         VALUES ($1, 'Notify Pending', 'https://faucet.example', 'description long enough', \
                 ARRAY['BTC']::coin[], 'PENDING') RETURNING id",
    )
    .bind(owner)
    .fetch_one(&pool)
    .await
    .expect("insert pending");

    let (admin_state, admin_token) = common::admin_login(pool.clone()).await;

    // Conflict: approve unknown
    let missing = Uuid::new_v4();
    let st = post(
        admin_state.clone(),
        &format!("/v1/admin/faucetlist/{missing}/approve"),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT);

    // approve → faucet_site_owner_email + send_best_effort
    let st = post(
        admin_state.clone(),
        &format!("/v1/admin/faucetlist/{pending_id}/approve"),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT, "approve");

    // suspend → email
    let st = post(
        admin_state.clone(),
        &format!("/v1/admin/faucetlist/{pending_id}/suspend"),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT, "suspend");

    // PENDING for reject + email body with reason
    let reject_id: Uuid = sqlx::query_scalar(
        "INSERT INTO faucet_sites (owner_id, name, url, description, coins, status) \
         VALUES ($1, 'Reject Me', 'https://reject.example', 'another long enough description', \
                 ARRAY['BTC']::coin[], 'PENDING') RETURNING id",
    )
    .bind(owner)
    .fetch_one(&pool)
    .await
    .expect("insert reject pending");

    let st = post(
        admin_state.clone(),
        &format!("/v1/admin/faucetlist/{reject_id}/reject"),
        &admin_token,
        Some(json!({ "reason": "spam / phishing" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT, "reject");

    // conflict reject unknown
    let st = post(
        admin_state.clone(),
        &format!("/v1/admin/faucetlist/{missing}/reject"),
        &admin_token,
        Some(json!({ "reason": "gone" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT);

    let list = api_http::app_without_metrics(admin_state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/admin/faucetlist")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-real-ip", "203.0.113.232")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), axum::http::StatusCode::OK);
}
