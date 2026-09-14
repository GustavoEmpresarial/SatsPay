//! Register → 201 + accessToken + Set-Cookie refresh.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn register_issues_access_token_and_refresh_cookie(pool: PgPool) {
    let app = api_http::app_without_metrics(common::test_state(pool));
    let email = format!("reg-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let body = serde_json::json!({
        "email": email,
        "username": username,
        "password": "Password1234",
        "confirmPassword": "Password1234",
        "acceptTerms": true,
        "captchaToken": "dev-bypass"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.10")
                .header("user-agent", "sqlx-test")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let set_cookie = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_text = String::from_utf8_lossy(&bytes);
    assert_eq!(status, axum::http::StatusCode::CREATED, "body={body_text}");
    assert!(
        set_cookie.contains("refresh_token=") || set_cookie.contains("__Host-refresh_token="),
        "missing refresh cookie: {set_cookie}"
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!v["tokens"]["accessToken"].as_str().unwrap_or("").is_empty());
    assert_eq!(v["user"]["email"], email);
}
