//! Register → refresh with cookie → usable accessToken.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn cookie_header_from(set_cookie_values: &[String]) -> String {
    set_cookie_values
        .iter()
        .filter_map(|sc| {
            let pair = sc.split(';').next()?.trim();
            if pair.starts_with("refresh_token=") || pair.starts_with("__Host-refresh_token=") {
                Some(pair.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[sqlx::test(migrations = "../db/migrations")]
async fn refresh_issues_usable_access_token(pool: PgPool) {
    let state = common::test_state(pool);
    let email = format!("ref-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);

    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.16")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "username": username,
                        "password": "Password1234",
                        "confirmPassword": "Password1234",
                        "acceptTerms": true,
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status(), axum::http::StatusCode::CREATED);
    let set_cookies: Vec<String> = reg
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let cookie = cookie_header_from(&set_cookies);
    assert!(!cookie.is_empty(), "no refresh cookie in {set_cookies:?}");

    let response = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/refresh")
                .header("content-type", "application/json")
                .header("origin", "http://localhost:5173")
                .header("cookie", &cookie)
                .header("x-real-ip", "203.0.113.16")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let refresh_cookies: Vec<String> = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "body={}",
        String::from_utf8_lossy(&bytes)
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["accessToken"].as_str().unwrap_or("");
    assert!(!token.is_empty());
    assert!(
        !cookie_header_from(&refresh_cookies).is_empty(),
        "refresh must rotate cookie: {refresh_cookies:?}"
    );

    let me = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.16")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), axum::http::StatusCode::OK);
    let me_bytes = me.into_body().collect().await.unwrap().to_bytes();
    let me_json: serde_json::Value = serde_json::from_slice(&me_bytes).unwrap();
    assert_eq!(me_json["user"]["email"], email);
}
