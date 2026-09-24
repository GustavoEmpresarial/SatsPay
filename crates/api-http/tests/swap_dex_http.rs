//! DEX swap path via wiremock SwapKit + public HOT_ADDRESS_* for hot addresses.

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

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_dex_quote_and_execute(pool: PgPool) {
    common::set_hot_addresses(HOT_MNEMONIC);
    std::env::set_var("CHAIN_NETWORK", "mainnet");

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/quote"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"routes":[{"routeId":"dex-r1","providers":["ONEINCH"],"expectedBuyAmount":"50.0","expectedBuyAmountMaxSlippage":"45.0","estimatedTime":{"inbound":30,"swap":60,"outbound":30,"total":120},"meta":{"tags":["recommended"]},"fees":[{"type":"inbound","amount":"0.0001","asset":"POL.POL"}],"txType":"simpleTransfer"}]}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v3/swap"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"routeId":"dex-r1","providers":["ONEINCH"],"targetAddress":"0x1111111111111111111111111111111111111111","memo":"=:POL.USDT","inboundAddress":"0x1111111111111111111111111111111111111111"}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    db::house::ensure_house_inventory(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;

    let state = common::test_state_with_swapkit(pool.clone(), &server.uri());
    // register via this state's pool
    let email = format!("dex-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.64")
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
    common::credit_personal(&pool, uid, Coin::Pol, 50_000_000).await;

    // Quote should include swapkit route
    let quote = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/quote?fromCoin=POL&toCoin=USDT&fromAmount=1000000")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.64")
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
    assert!(
        routes.iter().any(|r| r["source"] == "swapkit" || r["routeId"] == "dex-r1"),
        "expected dex route in {q}"
    );

    // DEX execute (not house)
    let exec = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.64")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000000",
                        "expectedToAmount": "5000000000",
                        "minToAmount": "4500000000",
                        "idempotencyKey": "dex-http-1",
                        "source": "swapkit",
                        "provider": "THORCHAIN",
                        "routeId": "dex-r1",
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
    assert!(
        st.is_success(),
        "dex execute={} {}",
        st,
        String::from_utf8_lossy(&bytes)
    );
    let exec_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let swap_id = exec_body["id"].as_str().unwrap().to_string();

    // History maps DEX rows; order status happy path
    let hist = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/swap/history")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.64")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hist.status(), axum::http::StatusCode::OK);
    let order = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/swap/orders/{swap_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.64")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(order.status(), axum::http::StatusCode::OK);

    // Missing routeId for DEX
    let bad = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/swap")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.64")
                .body(Body::from(
                    serde_json::json!({
                        "fromCoin": "POL",
                        "toCoin": "USDT",
                        "fromAmount": "1000000",
                        "expectedToAmount": "1",
                        "idempotencyKey": "dex-bad",
                        "source": "swapkit",
                        "provider": "THORCHAIN",
                        "routeId": ""
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), axum::http::StatusCode::BAD_REQUEST);

    common::clear_hot_addresses();
}
