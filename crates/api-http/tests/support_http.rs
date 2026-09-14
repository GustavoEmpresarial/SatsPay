//! HTTP coverage for `/v1/support/*` and `/v1/admin/support/*`.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn json_req(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("x-real-ip", "203.0.113.200");
    let req = if let Some(b) = body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(b.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    let res = api_http::app_without_metrics(state)
        .oneshot(req)
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
    };
    (status, v)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn support_http_user_and_admin_full(pool: PgPool) {
    let (state, user_token, _uid, _email) = common::register_user(pool.clone(), "sup-http").await;
    let (admin_state, admin_token) = common::admin_login(pool.clone()).await;

    // Unauthenticated
    let res = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/support/tickets")
                .header("x-real-ip", "203.0.113.201")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::UNAUTHORIZED);

    // Create — invalid topic
    let (st, body) = json_req(
        state.clone(),
        "POST",
        "/v1/support/tickets",
        &user_token,
        Some(json!({ "topic": "bad", "message": "x" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_topic");

    // Create — empty message
    let (st, body) = json_req(
        state.clone(),
        "POST",
        "/v1/support/tickets",
        &user_token,
        Some(json!({ "topic": "deposit", "message": "  " })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "empty_message");

    // Create OK
    let (st, created) = json_req(
        state.clone(),
        "POST",
        "/v1/support/tickets",
        &user_token,
        Some(json!({ "topic": "withdraw", "message": "saque travado" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    let ticket_id = created["ticket"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["ticket"]["status"], "OPEN");
    assert_eq!(created["messages"].as_array().unwrap().len(), 1);

    // List
    let (st, list) = json_req(state.clone(), "GET", "/v1/support/tickets", &user_token, None).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(!list["tickets"].as_array().unwrap().is_empty());

    // Get
    let (st, detail) = json_req(
        state.clone(),
        "GET",
        &format!("/v1/support/tickets/{ticket_id}"),
        &user_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(detail["ticket"]["id"], ticket_id);

    // Get missing
    let missing = Uuid::new_v4();
    let (st, body) = json_req(
        state.clone(),
        "GET",
        &format!("/v1/support/tickets/{missing}"),
        &user_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");

    // User reply
    let (st, replied) = json_req(
        state.clone(),
        "POST",
        &format!("/v1/support/tickets/{ticket_id}/messages"),
        &user_token,
        Some(json!({ "message": "mais detalhes" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{replied}");
    assert_eq!(replied["ticket"]["status"], "WAITING_STAFF");

    // Admin gate: plain user forbidden
    let (st, body) = json_req(
        state.clone(),
        "GET",
        "/v1/admin/support/tickets",
        &user_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "admin access required");

    // Admin list (ALL / empty / filter / invalid)
    let (st, admin_list) = json_req(
        admin_state.clone(),
        "GET",
        "/v1/admin/support/tickets",
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{admin_list}");
    assert!(!admin_list["tickets"].as_array().unwrap().is_empty());

    let (st, _) = json_req(
        admin_state.clone(),
        "GET",
        "/v1/admin/support/tickets?status=",
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    let (st, _) = json_req(
        admin_state.clone(),
        "GET",
        "/v1/admin/support/tickets?status=ALL",
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    let (st, filtered) = json_req(
        admin_state.clone(),
        "GET",
        "/v1/admin/support/tickets?status=WAITING_STAFF",
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(filtered["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == ticket_id));

    let (st, body) = json_req(
        admin_state.clone(),
        "GET",
        "/v1/admin/support/tickets?status=NOPE",
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_status");

    // Admin get
    let (st, admin_detail) = json_req(
        admin_state.clone(),
        "GET",
        &format!("/v1/admin/support/tickets/{ticket_id}"),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(admin_detail["ticket"]["user_email"].as_str().is_some());

    let (st, body) = json_req(
        admin_state.clone(),
        "GET",
        &format!("/v1/admin/support/tickets/{missing}"),
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");

    // Admin reply
    let (st, staff_reply) = json_req(
        admin_state.clone(),
        "POST",
        &format!("/v1/admin/support/tickets/{ticket_id}/messages"),
        &admin_token,
        Some(json!({ "message": "recebemos seu chamado" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{staff_reply}");
    assert_eq!(staff_reply["ticket"]["status"], "WAITING_USER");

    let (st, body) = json_req(
        admin_state.clone(),
        "POST",
        &format!("/v1/admin/support/tickets/{missing}/messages"),
        &admin_token,
        Some(json!({ "message": "ghost" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");

    let (st, body) = json_req(
        admin_state.clone(),
        "POST",
        &format!("/v1/admin/support/tickets/{ticket_id}/messages"),
        &admin_token,
        Some(json!({ "message": "   " })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "empty_message");

    // Status set
    let (st, body) = json_req(
        admin_state.clone(),
        "POST",
        &format!("/v1/admin/support/tickets/{ticket_id}/status"),
        &admin_token,
        Some(json!({ "status": "BAD" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_status");

    let (st, resolved) = json_req(
        admin_state.clone(),
        "POST",
        &format!("/v1/admin/support/tickets/{ticket_id}/status"),
        &admin_token,
        Some(json!({ "status": "RESOLVED" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(resolved["ticket"]["status"], "RESOLVED");

    // User reply on resolved → conflict
    let (st, body) = json_req(
        state.clone(),
        "POST",
        &format!("/v1/support/tickets/{ticket_id}/messages"),
        &user_token,
        Some(json!({ "message": "ainda preciso" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT);
    assert_eq!(body["error"], "ticket_closed");

    // Admin status on missing
    let (st, body) = json_req(
        admin_state.clone(),
        "POST",
        &format!("/v1/admin/support/tickets/{missing}/status"),
        &admin_token,
        Some(json!({ "status": "CLOSED" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");

    // Admin forbidden on each admin route (gate branches)
    for (method, uri, body) in [
        ("GET", format!("/v1/admin/support/tickets/{ticket_id}"), None),
        (
            "POST",
            format!("/v1/admin/support/tickets/{ticket_id}/messages"),
            Some(json!({ "message": "nope" })),
        ),
        (
            "POST",
            format!("/v1/admin/support/tickets/{ticket_id}/status"),
            Some(json!({ "status": "CLOSED" })),
        ),
    ] {
        let (st, body) = json_req(state.clone(), method, &uri, &user_token, body).await;
        assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{uri} {body}");
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn support_http_map_err_db_via_dropped_table(pool: PgPool) {
    let (state, token, _uid, _email) = common::register_user(pool.clone(), "sup-db-http").await;
    let (admin_state, admin_token) = common::admin_login(pool.clone()).await;
    sqlx::query("DROP TABLE support_messages CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE support_tickets CASCADE")
        .execute(&pool)
        .await
        .unwrap();

    // create → Db
    let (st, body) = json_req(
        state.clone(),
        "POST",
        "/v1/support/tickets",
        &token,
        Some(json!({ "topic": "deposit", "message": "boom" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "internal_error");

    // list → Db (covers list_tickets Err arm)
    let (st, body) = json_req(state.clone(), "GET", "/v1/support/tickets", &token, None).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "internal_error");

    // admin list → Db
    let (st, body) = json_req(
        admin_state.clone(),
        "GET",
        "/v1/admin/support/tickets",
        &admin_token,
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "internal_error");
}
