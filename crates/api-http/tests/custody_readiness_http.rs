mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn api_is_unready_and_refuses_address_until_matching_worker_marker(pool: PgPool) {
    const WORDS: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    std::env::set_var("NODE_ENV", "production");
    std::env::set_var("USE_REAL_CHAIN_CLIENTS", "true");
    std::env::set_var("CHAIN_NETWORK", "mainnet");
    for &coin in shared::COINS.iter().filter(|coin| **coin != shared::Coin::Sol) {
        std::env::set_var(
            format!("DEPOSIT_XPUB_{}", coin.as_str()),
            chain::hd_wallet::account_xpub(WORDS, coin).unwrap(),
        );
    }

    let mut state = common::test_state(pool.clone());
    let registry = chain::ChainRegistry::from_env(pool.clone()).unwrap();
    let fingerprint = registry
        .custody_validation_fingerprint()
        .unwrap()
        .to_string();
    state.chain_registry = std::sync::Arc::new(registry);

    let health = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);

    let email = format!("custody-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let register = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.44")
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
    let body = register.into_body().collect().await.unwrap().to_bytes();
    let token = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["tokens"]
        ["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    let blocked = address_request(state.clone(), &token).await;
    assert_eq!(blocked.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = serde_json::from_slice(
        &blocked.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(body["code"], "CUSTODY_SIGNER_NOT_VALIDATED");

    db::custody::mark_signer_validated(&pool, &fingerprint, "test-sha")
        .await
        .unwrap();
    let health = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), axum::http::StatusCode::OK);
    assert_eq!(address_request(state, &token).await.status(), axum::http::StatusCode::OK);
}

async fn address_request(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    token: &str,
) -> axum::response::Response {
    api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/address/POL")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.44")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}
