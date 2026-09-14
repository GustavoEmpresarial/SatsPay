//! OAuth error/edge branches not covered by happy-path oauth_*_http suites.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn oneshot(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut b = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("x-real-ip", "203.0.113.220");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
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
async fn oauth_edges_invalid_client_redirect_create_pkce(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "oauth-edge").await;

    // authorize/info — unknown client
    let (st, body) = oneshot(
        state.clone(),
        "GET",
        "/v1/oauth/authorize/info?client_id=does-not-exist",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid_client");

    // create app — empty name
    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/oauth/apps",
        Some(&token),
        Some(json!({ "name": "  ", "redirect_uris": ["https://ok.example/cb"] })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // create app — evil redirect
    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/oauth/apps",
        Some(&token),
        Some(json!({ "name": "Bad", "redirect_uris": ["http://evil.com/cb"] })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // create app — empty redirects
    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/oauth/apps",
        Some(&token),
        Some(json!({ "name": "Bad2", "redirect_uris": [] })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // create ok
    let (st, created) = oneshot(
        state.clone(),
        "POST",
        "/v1/oauth/apps",
        Some(&token),
        Some(json!({
            "name": "Edge App",
            "redirect_uris": ["https://app.example/cb", "http://localhost:5173/cb"]
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{created}");
    let client_id = created["client_id"].as_str().or_else(|| created["app"]["client_id"].as_str()).unwrap();
    let app_id = created["id"].as_str().or_else(|| created["app"]["id"].as_str()).unwrap();

    // authorize/info — redirect not registered
    let (st, body) = oneshot(
        state.clone(),
        "GET",
        &format!(
            "/v1/oauth/authorize/info?client_id={client_id}&redirect_uri={}",
            urlencoding_lite("https://evil.example/cb")
        ),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid_redirect_uri");

    // authorize submit — invalid client
    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/oauth/authorize",
        Some(&token),
        Some(json!({
            "client_id": "nope",
            "redirect_uri": "https://app.example/cb",
            "decision": "approve"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // authorize submit — approve without PKCE
    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/oauth/authorize",
        Some(&token),
        Some(json!({
            "client_id": client_id,
            "redirect_uri": "https://app.example/cb",
            "decision": "approve"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "invalid_request");

    // authorize submit — bad PKCE method
    let (st, body) = oneshot(
        state.clone(),
        "POST",
        "/v1/oauth/authorize",
        Some(&token),
        Some(json!({
            "client_id": client_id,
            "redirect_uri": "https://app.example/cb",
            "decision": "approve",
            "code_challenge": "a".repeat(43),
            "code_challenge_method": "plain"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // update with bad redirect
    let (st, body) = oneshot(
        state.clone(),
        "PUT",
        &format!("/v1/oauth/apps/{app_id}"),
        Some(&token),
        Some(json!({
            "name": "Edge App",
            "redirect_uris": ["javascript:alert(1)"]
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // peer cannot delete
    let (state2, token2, _, _) = common::register_user(pool, "oauth-peer").await;
    let (st, body) = oneshot(
        state2,
        "DELETE",
        &format!("/v1/oauth/apps/{app_id}"),
        Some(&token2),
        None,
    )
    .await;
    assert!(
        st == axum::http::StatusCode::INTERNAL_SERVER_ERROR
            || st == axum::http::StatusCode::NOT_FOUND
            || st == axum::http::StatusCode::FORBIDDEN
            || st == axum::http::StatusCode::BAD_REQUEST,
        "{st} {body}"
    );

    // revoke unknown consent id
    let missing = Uuid::new_v4();
    let (st, body) = oneshot(
        state.clone(),
        "DELETE",
        &format!("/v1/oauth/authorized-apps/{missing}"),
        Some(&token),
        None,
    )
    .await;
    assert!(
        st == axum::http::StatusCode::INTERNAL_SERVER_ERROR
            || st == axum::http::StatusCode::NOT_FOUND
            || st == axum::http::StatusCode::NO_CONTENT,
        "{st} {body}"
    );

    // userinfo without auth
    let (st, _) = oneshot(state, "GET", "/v1/oauth/userinfo", None, None).await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED);
}

fn urlencoding_lite(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
