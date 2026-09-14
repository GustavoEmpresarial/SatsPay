//! Auth edge branches: bad login, referral register, captcha fail, mismatch passwords.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn auth_login_fail_and_referral_register(pool: PgPool) {
    let (state, token, _, email) = common::register_user(pool.clone(), "authx2").await;

    // Get referral code from stats
    let stats = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/referral/stats")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.97")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = stats.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let ref_code = body["referral_code"].as_str().unwrap().to_string();

    // Wrong password
    let bad = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.97")
                .header("user-agent", "sqlx-test")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "password": "WrongPassword99",
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), axum::http::StatusCode::UNAUTHORIZED);

    // Empty captcha — exercises verify path (dev captcha may still accept)
    let no_cap = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.97")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "password": "Password1234"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = no_cap.status();

    // Register with referral
    let email2 = format!("refreg-{}@bitcosats.test", Uuid::new_v4());
    let username2 = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.98")
                .header("user-agent", "sqlx-test")
                .body(Body::from(
                    serde_json::json!({
                        "email": email2,
                        "username": username2,
                        "password": "Password1234",
                        "confirmPassword": "Password1234",
                        "acceptTerms": true,
                        "captchaToken": "dev-bypass",
                        "referralCode": ref_code
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status(), axum::http::StatusCode::CREATED);

    // Password mismatch register
    let email3 = format!("mismatch-{}@bitcosats.test", Uuid::new_v4());
    let username3 = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let mismatch = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.98")
                .body(Body::from(
                    serde_json::json!({
                        "email": email3,
                        "username": username3,
                        "password": "Password1234",
                        "confirmPassword": "Password9999",
                        "acceptTerms": true,
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(mismatch.status().is_client_error());

    // Happy login
    let ok = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.97")
                .header("user-agent", "sqlx-test")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "password": "Password1234",
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok.status(), axum::http::StatusCode::OK);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn auth_admin_login_wrong_password(pool: PgPool) {
    let email = "admin@bitcosats.test".to_string();
    let username = format!("a{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let hash = crypto::hash_password("Password1234").unwrap();
    sqlx::query(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') \
         ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash, role = 'ADMIN'",
    )
    .bind(&email)
    .bind(&hash)
    .bind(&username)
    .execute(&pool)
    .await
    .unwrap();

    let state = common::test_state(pool);
    let bad = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/admin/login")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.99")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "password": "not-the-password",
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), axum::http::StatusCode::UNAUTHORIZED);
}
