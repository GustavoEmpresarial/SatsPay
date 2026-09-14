//! Airdrop / rewards / referral remaining GET surfaces.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn airdrop_rewards_referral_surfaces(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "arr").await;

    for path in [
        "/v1/airdrop/overview",
        "/v1/airdrop/profile",
        "/v1/airdrop/leaderboard",
        "/v1/airdrop/logs",
        "/v1/airdrop/history",
        "/v1/rewards",
        "/v1/referral/stats",
        "/v1/referral/commissions",
        "/v1/referral/users",
        "/v1/referral/list",
    ] {
        let response = api_http::app_without_metrics(state.clone())
            .oneshot(
                axum::http::Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-real-ip", "203.0.113.95")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let st = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            st,
            axum::http::StatusCode::OK,
            "{path} -> {}",
            String::from_utf8_lossy(&bytes)
        );
    }
}
