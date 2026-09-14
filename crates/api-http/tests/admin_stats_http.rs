//! GET /v1/admin/stats with ADMIN role (register then UPDATE).

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_stats_ok_after_role_promote(pool: PgPool) {
    let state = common::test_state(pool.clone());
    let email = format!("adm-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);

    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.23")
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
    let bytes = reg.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let user_id = v["user"]["id"].as_str().expect("user.id");
    let token = v["tokens"]["accessToken"].as_str().unwrap();

    sqlx::query("UPDATE users SET role = 'ADMIN'::user_role WHERE id = $1::uuid")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("promote admin");

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/admin/stats")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.23")
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
        "body={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body.get("total_users").is_some(), "stats keys={body}");
    assert!(body.get("server").is_some());
}
