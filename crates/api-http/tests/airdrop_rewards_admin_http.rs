//! Airdrop + rewards GETs + admin merchant approve/suspend.

mod common;

use axum::body::Body;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn airdrop_rewards_and_admin_merchant(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "air").await;

    for path in [
        "/v1/airdrop/overview",
        "/v1/airdrop/logs",
        "/v1/airdrop/leaderboard",
        "/v1/rewards",
    ] {
        let r = api_http::app_without_metrics(state.clone())
            .oneshot(
                axum::http::Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-real-ip", "203.0.113.70")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), axum::http::StatusCode::OK, "{path}");
    }

    let unauth = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/airdrop/overview")
                .header("x-real-ip", "203.0.113.70")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), axum::http::StatusCode::UNAUTHORIZED);

    // Admin merchant approve/suspend
    let uid = Uuid::parse_str(&user_id).unwrap();
    sqlx::query("UPDATE users SET merchant_status = 'PENDING'::merchant_status WHERE id = $1")
        .bind(uid)
        .execute(&state.pool)
        .await
        .unwrap();

    let (admin_state, admin_token) = common::admin_login(pool).await;
    let approve = api_http::app_without_metrics(admin_state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/admin/merchants/{user_id}/approve"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-real-ip", "203.0.113.71")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        approve.status().is_success(),
        "approve={}",
        approve.status()
    );

    let suspend = api_http::app_without_metrics(admin_state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/admin/merchants/{user_id}/suspend"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-real-ip", "203.0.113.71")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        suspend.status().is_success(),
        "suspend={}",
        suspend.status()
    );
}
