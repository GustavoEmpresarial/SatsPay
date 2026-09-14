//! OAuth authorize submit + PKCE token exchange + userinfo + revoke.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

/// RFC 7636 Appendix B vectors.
fn pkce_pair() -> (&'static str, &'static str) {
    (
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
    )
}

#[sqlx::test(migrations = "../db/migrations")]
async fn oauth_authorize_token_userinfo_flow(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "oatht").await;
    let (verifier, challenge) = pkce_pair();

    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/apps")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.82")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Token Flow App",
                        "redirect_uris": ["https://app.example/cb"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = create.status();
    let bytes = create.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let client_id = created["client_id"].as_str().expect("client_id");
    let client_secret = created["client_secret"].as_str().expect("client_secret");
    let app_id = created["id"].as_str().expect("id");

    // authorize info without redirect_uri uses first registered
    let info = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/oauth/authorize/info?client_id={client_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.82")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(info.status(), axum::http::StatusCode::OK);

    // deny path
    let deny = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/authorize")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.82")
                .body(Body::from(
                    serde_json::json!({
                        "client_id": client_id,
                        "redirect_uri": "https://app.example/cb",
                        "decision": "deny",
                        "state": "xyz"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let deny_st = deny.status();
    let bytes = deny.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(deny_st, axum::http::StatusCode::OK);
    let deny_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(deny_body["redirect_url"]
        .as_str()
        .unwrap()
        .contains("access_denied"));

    // approve + PKCE
    let approve = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/authorize")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.82")
                .body(Body::from(
                    serde_json::json!({
                        "client_id": client_id,
                        "redirect_uri": "https://app.example/cb",
                        "decision": "approve",
                        "state": "s1",
                        "scope": "openid profile email",
                        "code_challenge": challenge,
                        "code_challenge_method": "S256"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = approve.status();
    let bytes = approve.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let approve_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let redirect = approve_body["redirect_url"].as_str().unwrap();
    let code = redirect
        .split("code=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();

    let form = format!(
        "grant_type=authorization_code&code={code}&client_id={client_id}&client_secret={client_secret}&redirect_uri=https%3A%2F%2Fapp.example%2Fcb&code_verifier={verifier}"
    );
    let tok = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("x-real-ip", "203.0.113.82")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = tok.status();
    let bytes = tok.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "token={}", String::from_utf8_lossy(&bytes));
    let tok_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let access = tok_body["access_token"].as_str().expect("access_token");

    let userinfo = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/oauth/userinfo")
                .header("authorization", format!("Bearer {access}"))
                .header("x-real-ip", "203.0.113.82")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = userinfo.status();
    let bytes = userinfo.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));

    let authorized = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/oauth/authorized-apps")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.82")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), axum::http::StatusCode::OK);
    let bytes = authorized.into_body().collect().await.unwrap().to_bytes();
    let authz: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let apps = authz.as_array().expect("authorized list array");
    assert!(!apps.is_empty());
    let consent_app_id = apps[0]["application_id"]
        .as_str()
        .or_else(|| apps[0]["applicationId"].as_str())
        .unwrap_or(app_id);

    let revoke = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/oauth/authorized-apps/{consent_app_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.82")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke.status(), axum::http::StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn oauth_token_unsupported_grant(pool: PgPool) {
    let state = common::test_state(pool);
    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("x-real-ip", "203.0.113.83")
                .body(Body::from("grant_type=client_credentials"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
}
