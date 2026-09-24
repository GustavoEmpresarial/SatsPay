//! Relay as second swap provider — quote merge + execute without SwapKit /v3/swap.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const HOT_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

const RELAY_QUOTE_BODY: &str = r#"{
  "steps": [
    {
      "id": "approve-1",
      "action": "approve",
      "description": "Approve USDT",
      "kind": "transaction",
      "requestId": "abc-123",
      "items": [{
        "status": "incomplete",
        "data": {
          "to": "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",
          "data": "0x095ea7b30000000000000000000000006c0ad82f9721a6dc986381d19338601a2e6370e5ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
          "value": "0",
          "chainId": 137
        }
      }]
    },
    {
      "id": "swap-1",
      "action": "swap",
      "description": "Swap",
      "kind": "transaction",
      "items": [{
        "status": "incomplete",
        "data": {
          "to": "0xf574Fd487255dd69852e861250208640f040948a",
          "data": "0xdeadbeef",
          "value": "0",
          "chainId": 137
        }
      }]
    }
  ],
  "details": {
    "currencyOut": { "amount": "995000" }
  }
}"#;

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_relay_quote_and_execute(pool: PgPool) {
    common::set_hot_addresses(HOT_MNEMONIC);
    std::env::set_var("CHAIN_NETWORK", "mainnet");
    std::env::set_var("SWAPKIT_ENABLED", "false");

    let relay = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/quote/v2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(RELAY_QUOTE_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&relay)
        .await;

    db::house::ensure_house_inventory(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;

    let state = common::test_state_with_relay(pool.clone(), &relay.uri());
    let email = format!("relay-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.65")
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
    let uid = Uuid::parse_str(v["user"]["id"].as_str().unwrap()).unwrap();
    common::credit_personal(&pool, uid, Coin::Usdt, 50_000_000_000).await;

    let quote = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/quote?fromCoin=USDT&toCoin=USDC&fromAmount=1000000000")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.65")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = quote.status();
    let bytes = quote.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let q: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let routes = q["routes"].as_array().cloned().unwrap_or_default();
    let relay_route = routes
        .iter()
        .find(|r| r["source"] == "relay" && r["provider"] == "RELAY")
        .expect("relay route");
    assert_eq!(relay_route["routeId"], "relay:abc-123");
    assert_eq!(relay_route["txHint"], "contractCall");

    let expected = relay_route["youReceive"]["amount"].as_str().unwrap();
    let exec = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.65")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "USDT",
                        "toCoin": "USDC",
                        "fromAmount": "1000000000",
                        "expectedToAmount": expected,
                        "minToAmount": expected,
                        "idempotencyKey": "relay-http-1",
                        "routeId": "relay:abc-123",
                        "provider": "RELAY",
                        "source": "relay",
                        "platformFeeBps": 50
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = exec.status();
    let bytes = exec.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["source"], "relay");
    assert_eq!(body["provider"], "RELAY");

    let swap_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let row = db::dex_swap::get_by_id(&pool, swap_id).await.unwrap().unwrap();
    assert_eq!(row.tx_hint.as_deref(), Some("contractCall"));
    assert_eq!(row.provider, "RELAY");
    let payload = row.swap_payload.expect("payload");
    assert_eq!(payload["provider"], "relay");
    assert!(payload.get("tx").is_some());
}

const RELAY_BRIDGE_SOL_BODY: &str = r#"{
  "requestId": "0xbridge-sol",
  "steps": [{
    "id": "deposit",
    "action": "Confirm transaction in your wallet",
    "description": "Depositing funds",
    "kind": "transaction",
    "items": [{
      "status": "incomplete",
      "data": {
        "instructions": [{
          "keys": [{"pubkey":"11111111111111111111111111111111","isSigner":false,"isWritable":false}],
          "programId": "11111111111111111111111111111111",
          "data": "00"
        }],
        "addressLookupTableAddresses": []
      },
      "check": {"endpoint":"/intents/status/v3?requestId=0xbridge-sol","method":"GET"}
    }]
  }],
  "details": { "currencyOut": { "amount": "1136561" } }
}"#;

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_relay_bridge_sol_to_usdt(pool: PgPool) {
    common::set_hot_addresses(HOT_MNEMONIC);
    std::env::set_var("CHAIN_NETWORK", "mainnet");
    std::env::set_var("SWAPKIT_ENABLED", "false");

    let relay = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/quote/v2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(RELAY_BRIDGE_SOL_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&relay)
        .await;

    db::house::ensure_house_inventory(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;

    let state = common::test_state_with_relay(pool.clone(), &relay.uri());
    let email = format!("bridge-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.66")
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
    let uid = Uuid::parse_str(v["user"]["id"].as_str().unwrap()).unwrap();
    common::credit_personal(&pool, uid, Coin::Sol, 50_000_000_000).await;

    let quote = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/quote?fromCoin=SOL&toCoin=USDT&fromAmount=10000000")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.66")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = quote.status();
    let bytes = quote.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let q: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let routes = q["routes"].as_array().cloned().unwrap_or_default();
    let relay_route = routes
        .iter()
        .find(|r| r["source"] == "relay" && r["tags"].as_array().map(|t| t.iter().any(|x| x == "BRIDGE")).unwrap_or(false))
        .expect("bridge route");
    assert_eq!(relay_route["txHint"], "solanaRelay");

    let expected = relay_route["youReceive"]["amount"].as_str().unwrap();
    let exec = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.66")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "SOL",
                        "toCoin": "USDT",
                        "fromAmount": "10000000",
                        "expectedToAmount": expected,
                        "minToAmount": expected,
                        "idempotencyKey": "relay-bridge-1",
                        "routeId": relay_route["routeId"],
                        "provider": "RELAY",
                        "source": "relay",
                        "platformFeeBps": 50
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = exec.status();
    let bytes = exec.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let swap_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let row = db::dex_swap::get_by_id(&pool, swap_id).await.unwrap().unwrap();
    assert_eq!(row.tx_hint.as_deref(), Some("solanaRelay"));
    let payload = row.swap_payload.expect("payload");
    assert_eq!(payload["isBridge"], true);
    assert!(payload.get("solanaTx").is_some());
    assert!(payload.get("requestId").is_some());
}
