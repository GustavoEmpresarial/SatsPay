use shared::Coin;
use swapkit::{SwapKitClient, SwapKitError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn quote_swap_track_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/quote"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"routes":[{"routeId":"r1","providers":["THORCHAIN"],"expectedBuyAmount":"1.0","expectedBuyAmountMaxSlippage":"0.9"}]}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v3/swap"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"routeId":"r1","targetAddress":"bc1qdest","memo":"=:LTC"}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/track"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"status":"completed","outbound":{"hash":"tx1","amount":"1.0"}}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    let c = SwapKitClient::new_with_base(server.uri(), Some("k".into()));
    assert!(c.is_configured());
    let q = c
        .quote(Coin::Btc, Coin::Ltc, 100_000_000, Some("bc1qin"), Some("ltc1out"))
        .await
        .unwrap();
    assert!(!q.routes.is_empty());
    assert!(q.routes[0].platform_fee_bps.is_some());

    let s = c.swap("r1", "bc1qin", "ltc1out").await.unwrap();
    assert_eq!(s.deposit_address(), Some("bc1qdest"));

    let t = c.track("txhash").await.unwrap();
    assert!(t.is_complete());
}

#[tokio::test]
async fn quote_404_is_no_routes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/quote"))
        .respond_with(ResponseTemplate::new(404).set_body_string("none"))
        .mount(&server)
        .await;
    let c = SwapKitClient::new_with_base(server.uri(), None);
    assert!(matches!(
        c.quote(Coin::Btc, Coin::Ltc, 1, None, None).await.unwrap_err(),
        SwapKitError::NoRoutes
    ));
}

#[tokio::test]
async fn quote_rejects_dgb_and_zero() {
    let c = SwapKitClient::new_with_base("http://127.0.0.1:9", None);
    assert!(matches!(
        c.quote(Coin::Dgb, Coin::Btc, 1, None, None).await.unwrap_err(),
        SwapKitError::UnsupportedAsset(_)
    ));
    assert!(matches!(
        c.quote(Coin::Btc, Coin::Ltc, 0, None, None).await.unwrap_err(),
        SwapKitError::InvalidAmount
    ));
}
