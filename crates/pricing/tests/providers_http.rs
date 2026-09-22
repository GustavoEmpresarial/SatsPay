use pricing::{BinanceProvider, CoinGeckoClient, KrakenProvider, OkxProvider, PriceProvider};
use shared::Coin;
use std::time::Duration;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn binance_parses_tickers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/ticker/price"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"[{"symbol":"BTCUSDT","price":"45000.5"},{"symbol":"LTCUSDT","price":"80.1"}]"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let p = BinanceProvider::new(Some(server.uri()), Duration::from_secs(5));
    let map = p.fetch_prices(&[Coin::Btc, Coin::Ltc, Coin::Usdt]).await.unwrap();
    assert!((map[&Coin::Btc] - 45000.5).abs() < 1e-6);
    assert!((map[&Coin::Ltc] - 80.1).abs() < 1e-6);
    assert_eq!(map[&Coin::Usdt], 1.0);
}

#[tokio::test]
async fn kraken_parses_ticker() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/0/public/Ticker.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"error":[],"result":{"XXBTZUSD":{"c":["42000.0","1"]},"XLTCZUSD":{"c":["70.0","1"]}}}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let p = KrakenProvider::new(Some(server.uri()), Duration::from_secs(5));
    let map = p.fetch_prices(&[Coin::Btc, Coin::Ltc]).await.unwrap();
    assert!((map[&Coin::Btc] - 42000.0).abs() < 1e-6);
    assert!((map[&Coin::Ltc] - 70.0).abs() < 1e-6);
}

#[tokio::test]
async fn okx_parses_tickers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/market/tickers"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"code":"0","data":[{"instId":"BTC-USDT","last":"41000"},{"instId":"LTC-USDT","last":"65"}]}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let p = OkxProvider::new(Some(server.uri()), Duration::from_secs(5));
    let map = p.fetch_prices(&[Coin::Btc, Coin::Ltc, Coin::Usdt]).await.unwrap();
    assert!((map[&Coin::Btc] - 41000.0).abs() < 1e-6);
    assert_eq!(map[&Coin::Usdt], 1.0);
}

#[tokio::test]
async fn coingecko_parses_simple_price() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/simple/price.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"bitcoin":{"usd":40000.0},"litecoin":{"usd":60.0}}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let p = CoinGeckoClient::new(server.uri(), Duration::from_secs(5));
    let map = p.fetch_prices(&[Coin::Btc, Coin::Ltc]).await.unwrap();
    assert!((map[&Coin::Btc] - 40000.0).abs() < 1e-6);
}

#[tokio::test]
async fn providers_fail_on_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(BinanceProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
    assert!(KrakenProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
    assert!(OkxProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
    assert!(CoinGeckoClient::new(server.uri(), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
}

#[tokio::test]
async fn binance_covers_all_mapped_symbols() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/ticker/price"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"[
                      {"symbol":"BTCUSDT","price":"1"},
                      {"symbol":"LTCUSDT","price":"2"},
                      {"symbol":"DOGEUSDT","price":"3"},
                      {"symbol":"BCHUSDT","price":"4"},
                      {"symbol":"POLUSDT","price":"5"},
                      {"symbol":"DGBUSDT","price":"6"},
                      {"symbol":"SOLUSDT","price":"7"},
                      {"symbol":"USDCUSDT","price":"8"},
                      {"symbol":"PEPEUSDT","price":"0.000004"}
                    ]"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let p = BinanceProvider::new(Some(server.uri()), Duration::from_secs(5));
    let coins = [
        Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Bch, Coin::Pol, Coin::Dgb, Coin::Sol, Coin::Usdc, Coin::Usdt, Coin::Pepe,
    ];
    let map = p.fetch_prices(&coins).await.unwrap();
    assert_eq!(map.len(), 10);
    assert_eq!(p.name(), "Binance");
}

#[tokio::test]
async fn okx_and_kraken_cover_more_coins_and_names() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/market/tickers"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"code":"0","data":[
                      {"instId":"BTC-USDT","last":"1"},
                      {"instId":"LTC-USDT","last":"2"},
                      {"instId":"DOGE-USDT","last":"3"},
                      {"instId":"BCH-USDT","last":"4"},
                      {"instId":"POL-USDT","last":"5"},
                      {"instId":"DGB-USDT","last":"6"},
                      {"instId":"SOL-USDT","last":"7"},
                      {"instId":"USDC-USDT","last":"8"}
                    ]}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let o = OkxProvider::new(Some(server.uri()), Duration::from_secs(5));
    let coins = shared::COINS.to_vec();
    let map = o.fetch_prices(&coins).await.unwrap();
    assert!(map.len() >= 8);
    assert_eq!(o.name(), "OKX");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/0/public/Ticker.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"error":[],"result":{
                      "XXBTZUSD":{"c":["1","1"]},
                      "XLTCZUSD":{"c":["2","1"]},
                      "XDGUSD":{"c":["3","1"]},
                      "BCHUSD":{"c":["4","1"]},
                      "POLUSD":{"c":["5","1"]},
                      "SOLUSD":{"c":["6","1"]},
                      "USDCUSD":{"c":["7","1"]},
                      "USDTUSD":{"c":["8","1"]}
                    }}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let k = KrakenProvider::new(Some(server.uri()), Duration::from_secs(5));
    let map = k.fetch_prices(&coins).await.unwrap();
    assert!(map.len() >= 7);
    assert_eq!(k.name(), "Kraken");
}

#[tokio::test]
async fn coingecko_all_ids_and_fetch_usd_prices() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/simple/price.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{
                      "bitcoin":{"usd":1.0},
                      "litecoin":{"usd":2.0},
                      "dogecoin":{"usd":3.0},
                      "bitcoin-cash":{"usd":4.0},
                      "polygon-ecosystem-token":{"usd":5.0},
                      "digibyte":{"usd":6.0},
                      "solana":{"usd":7.0},
                      "tether":{"usd":8.0},
                      "usd-coin":{"usd":9.0},
                      "zero":{"usd":0.01},
                      "pepe":{"usd":0.000004}
                    }"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let c = CoinGeckoClient::new(server.uri(), Duration::from_secs(5));
    assert_eq!(c.name(), "CoinGecko");
    let map = c.fetch_prices(&shared::COINS).await.unwrap();
    assert_eq!(map.len(), shared::COINS.len());
    let list = c.fetch_usd_prices(&shared::COINS).await.unwrap();
    assert_eq!(list.len(), shared::COINS.len());
}

#[tokio::test]
async fn providers_fail_on_malformed_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("not-json")
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(BinanceProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
    assert!(KrakenProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
    assert!(OkxProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
    assert!(CoinGeckoClient::new(server.uri(), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
}

#[tokio::test]
async fn kraken_error_array_and_okx_bad_code() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/0/public/Ticker.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"error":["EQuery:Unknown asset"],"result":null}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(KrakenProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/market/tickers"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"code":"50001","data":null}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    assert!(OkxProvider::new(Some(server.uri()), Duration::from_secs(2))
        .fetch_prices(&[Coin::Btc])
        .await
        .is_err());
}
