//! GET /v1/deposits/address/:coin + deposit history (stub chain).

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn real_config_without_signer() -> chain::RealClientConfig {
    chain::RealClientConfig {
        bitcore_base_url: "http://127.0.0.1:9".into(),
        evm_rpc_url: "http://127.0.0.1:9".into(),
        bsc_rpc_url: "http://127.0.0.1:9".into(),
        wallet: chain::PublicWalletConfig::default(),
        hot_wallet_wif: None,
        evm_deposit_lookback_blocks: 10,
        fee_confirmation_target: 2,
        network: chain::ChainNetwork::Mainnet,
        sol_rpc_url: "http://127.0.0.1:9".into(),
        dgb_insight_url: "http://127.0.0.1:9".into(),
        dgb_rpc_url: None,
        zer_explorer_url: "http://127.0.0.1:9".into(),
        zer_explorer_api_key: None,
        zer_rpc_url: None,
        deposit_mnemonic: None,
        hot_mnemonic: None,
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn deposit_address_and_history(pool: PgPool) {
    let state = common::test_state(pool);
    let email = format!("dep-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);

    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.28")
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
    let token = v["tokens"]["accessToken"].as_str().unwrap();

    let addr_resp = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/address/POL")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.28")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = addr_resp.status();
    let bytes = addr_resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "address={}",
        String::from_utf8_lossy(&bytes)
    );
    let addr: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!addr["address"].as_str().unwrap_or("").is_empty());

    let hist = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/history")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.28")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = hist.status();
    let bytes = hist.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "history={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["deposits"].as_array().is_some());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn empty_sol_pool_returns_retryable_503_contract(pool: PgPool) {
    let (mut state, token, _, _) = common::register_user(pool.clone(), "sol-empty").await;
    let sol = std::sync::Arc::new(chain::RealChainClient::new(
        shared::Coin::Sol,
        pool,
        real_config_without_signer(),
    ));
    state.chain_registry = std::sync::Arc::new(
        chain::ChainRegistry::build("development", true)
            .unwrap()
            .with_client(sol),
    );

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/address/SOL")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.28")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["code"], chain::DEPOSIT_ADDRESS_POOL_EMPTY);
    assert!(body["error"].as_str().is_some_and(|v| !v.is_empty()));
}
