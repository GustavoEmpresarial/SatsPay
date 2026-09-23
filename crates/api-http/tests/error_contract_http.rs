//! Framework extractor rejections (malformed JSON, wrong field type, missing
//! content-type) are normalized to the stable `{ error, code }` contract and
//! never echo axum's parser detail.
//!
//! A security probe found the raw axum messages leaking field names, types and
//! parse offsets in a `text/plain` body — off-contract and minor info
//! disclosure.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

async fn post_raw(state: &api_http::AppState<db::auth::PgAuthRepo>, uri: &str, ct: Option<&str>, body: &str) -> (StatusCode, Option<String>, Value) {
    let mut b = Request::builder().method("POST").uri(uri).header("x-real-ip", "203.0.113.200");
    if let Some(ct) = ct {
        b = b.header("content-type", ct);
    }
    let res = api_http::app_without_metrics(state.clone())
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let ct = res.headers().get("content-type").and_then(|v| v.to_str().ok()).map(str::to_string);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, ct, json)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn malformed_json_is_normalized_and_leaks_no_parser_detail(pool: PgPool) {
    let state = common::test_state(pool);

    // Syntactically broken JSON.
    let (status, ct, body) = post_raw(&state, "/v1/auth/login", Some("application/json"), "{bad json").await;
    assert!(status.is_client_error(), "{status}");
    assert!(ct.as_deref().unwrap_or("").starts_with("application/json"), "ct={ct:?}");
    assert_eq!(body["code"], "INVALID_JSON");
    let text = body.to_string();
    assert!(!text.contains("Failed to parse"), "leaked parser detail: {text}");
    assert!(!text.contains("column"), "leaked parse offset: {text}");

    // Right JSON, wrong field type — must not echo the field name/type.
    let (status, _, body) = post_raw(&state, "/v1/auth/login", Some("application/json"), r#"{"email":123,"password":[]}"#).await;
    assert!(status.is_client_error(), "{status}");
    assert_eq!(body["code"], "INVALID_JSON");
    let text = body.to_string();
    assert!(!text.contains("invalid type"), "leaked type detail: {text}");
    assert!(!text.contains("email"), "leaked field name: {text}");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn missing_content_type_is_normalized(pool: PgPool) {
    let state = common::test_state(pool);
    let (status, ct, body) = post_raw(&state, "/v1/auth/login", None, r#"{"email":"a@b.c","password":"x"}"#).await;
    // axum rejects a JSON extractor without the JSON content-type.
    assert!(status.is_client_error(), "{status}");
    assert!(ct.as_deref().unwrap_or("").starts_with("application/json"));
    assert_eq!(body["code"], "INVALID_JSON");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn valid_json_still_reaches_the_handler(pool: PgPool) {
    let state = common::test_state(pool);
    // Well-formed but wrong credentials: the handler runs and answers its own
    // contract error, proving the normalizer does not swallow real responses.
    let (status, ct, body) = post_raw(
        &state,
        "/v1/auth/login",
        Some("application/json"),
        r#"{"email":"nobody@bitcosats.test","password":"Password1234","captchaToken":"x"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(ct.as_deref().unwrap_or("").starts_with("application/json"));
    // Auth uses the nested error envelope.
    assert_eq!(body["error"]["code"], "INVALID_CREDENTIALS");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn bad_query_string_is_validation_error_not_invalid_json(pool: PgPool) {
    // axum's query rejection also starts with "Failed to deserialize"; it must
    // not be reported as INVALID_JSON (review finding).
    let (state, token, _, _) = common::register_user(pool, "badquery").await;
    let res = api_http::app_without_metrics(state.clone())
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/withdrawals/history?limit=abc")
                .header("x-real-ip", "203.0.113.201")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "VALIDATION_ERROR", "{body}");
    assert!(!body.to_string().contains("limit"), "must not echo the field name: {body}");
}
