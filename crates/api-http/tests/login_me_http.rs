//! Register → login → GET /v1/auth/me with bearer.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn post_json(
    app: axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (axum::http::StatusCode, serde_json::Value, String) {
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.11")
                .header("user-agent", "sqlx-test")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, v, String::from_utf8_lossy(&bytes).into_owned())
}

#[sqlx::test(migrations = "../db/migrations")]
async fn login_then_me(pool: PgPool) {
    let state = common::test_state(pool);
    let email = format!("login-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let password = "Password1234";

    let (st, reg, raw) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/auth/register",
        serde_json::json!({
            "email": email,
            "username": username,
            "password": password,
            "confirmPassword": password,
            "acceptTerms": true,
            "captchaToken": "dev-bypass"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "register={raw}");

    let (st, login, raw) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/auth/login",
        serde_json::json!({
            "email": email,
            "password": password,
            "captchaToken": "dev-bypass"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "login={raw}");
    assert_eq!(login["kind"], "ok");
    let token = login["tokens"]["accessToken"].as_str().expect("accessToken");

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.11")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "me={}",
        String::from_utf8_lossy(&bytes)
    );
    let me: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(me["user"]["email"], email);
    assert_eq!(me["user"]["id"], reg["user"]["id"]);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn me_export_and_erase(pool: PgPool) {
    let state = common::test_state(pool);
    let email = format!("erase-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("e{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let password = "Password1234";
    let (st, _reg, raw) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/auth/register",
        serde_json::json!({
            "email": email,
            "username": username,
            "password": password,
            "confirmPassword": password,
            "acceptTerms": true,
            "captchaToken": "dev-bypass"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{raw}");
    let (st, login, raw) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/auth/login",
        serde_json::json!({
            "email": email,
            "password": password,
            "captchaToken": "dev-bypass"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{raw}");
    let token = login["tokens"]["accessToken"].as_str().expect("token");

    let exp = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/me/export")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exp.status(), axum::http::StatusCode::OK);
    let bytes = exp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["user"]["email"], email);
    assert!(v["wallets"].is_array());

    let erase = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/me/erase")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({ "confirmEmail": email, "confirm": "APAGAR" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(erase.status(), axum::http::StatusCode::OK);

    let me = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), axum::http::StatusCode::UNAUTHORIZED);
}
