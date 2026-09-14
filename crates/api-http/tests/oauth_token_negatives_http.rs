//! OAuth token_exchange + userinfo negative branches.

mod common;

use axum::body::Body;
use base64::Engine;
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;

async fn form_post(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    uri: &str,
    form: &str,
    extra_auth: Option<&str>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut b = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("x-real-ip", "203.0.113.240");
    if let Some(a) = extra_auth {
        b = b.header("authorization", a);
    }
    let res = api_http::app_without_metrics(state)
        .oneshot(b.body(Body::from(form.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }));
    (status, v)
}

async fn get_userinfo(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    auth: Option<&str>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut b = axum::http::Request::builder()
        .uri("/v1/oauth/userinfo")
        .header("x-real-ip", "203.0.113.241");
    if let Some(a) = auth {
        b = b.header("authorization", a);
    }
    let res = api_http::app_without_metrics(state)
        .oneshot(b.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }));
    (status, v)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn oauth_token_and_userinfo_negatives(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "oauth-tok").await;

    // Create app for later invalid_grant
    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/apps")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.242")
                .body(Body::from(
                    json!({
                        "name": "Tok App",
                        "redirect_uris": ["https://tok.example/cb"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), axum::http::StatusCode::OK);
    let bytes = create.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let client_id = created["client_id"]
        .as_str()
        .or_else(|| created["app"]["client_id"].as_str())
        .expect("client_id")
        .to_string();
    let client_secret = created["client_secret"]
        .as_str()
        .or_else(|| created["app"]["client_secret"].as_str())
        .unwrap_or("not-a-secret")
        .to_string();

    // unsupported_grant_type
    let (st, body) = form_post(
        state.clone(),
        "/v1/oauth/token",
        "grant_type=client_credentials&client_id=x&client_secret=y&code=z",
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "unsupported_grant_type");

    // missing params
    let (st, body) = form_post(
        state.clone(),
        "/v1/oauth/token",
        "grant_type=authorization_code",
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid_request");

    // invalid_grant (bad code) — credentials via form
    let (st, body) = form_post(
        state.clone(),
        "/v1/oauth/token",
        &format!(
            "grant_type=authorization_code&code=bogus&client_id={client_id}&client_secret={client_secret}&redirect_uri=https%3A%2F%2Ftok.example%2Fcb"
        ),
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid_grant");

    // Basic auth path for client_id/secret + invalid grant
    let basic = base64::engine::general_purpose::STANDARD
        .encode(format!("{client_id}:{client_secret}"));
    let (st, body) = form_post(
        state.clone(),
        "/v1/oauth/token",
        "grant_type=authorization_code&code=still-bogus&redirect_uri=https%3A%2F%2Ftok.example%2Fcb",
        Some(&format!("Basic {basic}")),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid_grant");

    // userinfo — missing Authorization
    let (st, body) = get_userinfo(state.clone(), None).await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"], "missing_token");

    // userinfo — not Bearer
    let (st, body) = get_userinfo(state.clone(), Some("Token abc")).await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"], "invalid_token");

    // userinfo — Bearer but invalid/expired
    let (st, body) = get_userinfo(state, Some("Bearer not-a-real-oauth-token")).await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"], "invalid_token");
}
