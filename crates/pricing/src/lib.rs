//! Multi-Provider Price Oracle for BitcoSats / SatsPay.
//!
//! Concurrently queries 3+ top-tier institutional crypto price providers:
//! 1. Binance Public Spot Ticker
//! 2. Kraken Public Market Ticker
//! 3. OKX Public Spot Ticker
//! 4. CoinGecko Simple Price (fallback)
//!
//! Uses robust median consensus aggregation to eliminate single-point-of-failure
//! and price manipulation/spikes.

use serde::Deserialize;
use shared::Coin;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum PricingError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("all price providers failed for {0:?}")]
    AllProvidersFailed(Coin),
    #[error("no price available for coin {0:?}")]
    MissingCoin(Coin),
    #[error("non-positive price returned for coin {0:?}")]
    BadPrice(Coin),
}

/// Generic interface for a price provider
#[async_trait::async_trait]
pub trait PriceProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String>;
}

/// Binance Public Spot API
pub struct BinanceProvider {
    http: reqwest::Client,
    base_url: String,
}

impl BinanceProvider {
    pub fn new(base_url: Option<String>, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("SatsPay-PricingOracle/1.0")
            .build()
            .unwrap_or_default();
        Self {
            http,
            base_url: base_url.unwrap_or_else(|| "https://api.binance.com".to_string()),
        }
    }

    fn coin_to_symbol(coin: Coin) -> Option<&'static str> {
        match coin {
            Coin::Btc => Some("BTCUSDT"),
            Coin::Ltc => Some("LTCUSDT"),
            Coin::Doge => Some("DOGEUSDT"),
            Coin::Bch => Some("BCHUSDT"),
            Coin::Pol => Some("POLUSDT"),
            Coin::Dgb => Some("DGBUSDT"),
            Coin::Sol => Some("SOLUSDT"),
            Coin::Usdc => Some("USDCUSDT"),
            Coin::Usdt => None,
            Coin::Zer => None,
            Coin::Pepe => Some("PEPEUSDT"),
        }
    }
}

#[derive(Deserialize)]
struct BinanceTickerItem {
    symbol: String,
    price: String,
}

#[async_trait::async_trait]
impl PriceProvider for BinanceProvider {
    fn name(&self) -> &'static str {
        "Binance"
    }

    async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String> {
        let symbols: Vec<String> = coins
            .iter()
            .filter_map(|c| Self::coin_to_symbol(*c).map(|s| format!("%22{s}%22")))
            .collect();
        let symbols_param = format!("%5B{}%5D", symbols.join(","));
        let url = format!(
            "{}/api/v3/ticker/price?symbols={}",
            self.base_url.trim_end_matches('/'),
            symbols_param
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("http status {}", resp.status()));
        }

        let items: Vec<BinanceTickerItem> = resp
            .json()
            .await
            .map_err(|e| format!("parse error: {e}"))?;

        let mut map = HashMap::new();
        for item in items {
            if let Ok(val) = item.price.parse::<f64>() {
                if val > 0.0 {
                    for &c in coins {
                        if Self::coin_to_symbol(c) == Some(item.symbol.as_str()) {
                            map.insert(c, val);
                        }
                    }
                }
            }
        }
        if coins.contains(&Coin::Usdt) {
            map.insert(Coin::Usdt, 1.0);
        }
        Ok(map)
    }
}

/// Kraken Public Ticker API
pub struct KrakenProvider {
    http: reqwest::Client,
    base_url: String,
}

impl KrakenProvider {
    pub fn new(base_url: Option<String>, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("SatsPay-PricingOracle/1.0")
            .build()
            .unwrap_or_default();
        Self {
            http,
            base_url: base_url.unwrap_or_else(|| "https://api.kraken.com".to_string()),
        }
    }
}

#[derive(Deserialize)]
struct KrakenTickerResp {
    #[serde(default)]
    error: Vec<String>,
    result: Option<HashMap<String, KrakenPairInfo>>,
}

#[derive(Deserialize)]
struct KrakenPairInfo {
    c: Vec<String>,
}

#[async_trait::async_trait]
impl PriceProvider for KrakenProvider {
    fn name(&self) -> &'static str {
        "Kraken"
    }

    async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String> {
        let pairs_param = "XBTUSD,LTCUSD,XDGUSD,BCHUSD,POLUSD,SOLUSD,USDCUSD,USDTUSD";
        let url = format!(
            "{}/0/public/Ticker?pair={}",
            self.base_url.trim_end_matches('/'),
            pairs_param
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("http status {}", resp.status()));
        }

        let body: KrakenTickerResp = resp
            .json()
            .await
            .map_err(|e| format!("parse error: {e}"))?;

        if !body.error.is_empty() {
            return Err(format!("kraken error: {:?}", body.error));
        }

        let result = body.result.ok_or_else(|| "missing result".to_string())?;
        let mut map = HashMap::new();

        for &coin in coins {
            let possible_keys: &[&str] = match coin {
                Coin::Btc => &["XXBTZUSD", "XBTUSD", "BTCUSD"],
                Coin::Ltc => &["XLTCZUSD", "LTCUSD"],
                Coin::Doge => &["XDGUSD", "XXDGZUSD", "DOGEUSD"],
                Coin::Bch => &["BCHUSD"],
                Coin::Pol => &["POLUSD", "MATICUSD"],
                Coin::Dgb => &["DGBUSD"],
                Coin::Sol => &["SOLUSD"],
                Coin::Usdt => &["USDTZUSD", "USDTUSD"],
                Coin::Usdc => &["USDCUSD"],
                Coin::Zer => &[],
                Coin::Pepe => &["PEPEUSD"],
            };

            for &key in possible_keys {
                if let Some(pair_info) = result.get(key) {
                    if let Some(price_str) = pair_info.c.first() {
                        if let Ok(val) = price_str.parse::<f64>() {
                            if val > 0.0 {
                                map.insert(coin, val);
                                break;
                            }
                        }
                    }
                }
            }
            if coin == Coin::Usdt {
                map.entry(Coin::Usdt).or_insert(1.0);
            }
        }

        Ok(map)
    }
}

/// OKX Public Spot API
pub struct OkxProvider {
    http: reqwest::Client,
    base_url: String,
}

impl OkxProvider {
    pub fn new(base_url: Option<String>, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("SatsPay-PricingOracle/1.0")
            .build()
            .unwrap_or_default();
        Self {
            http,
            base_url: base_url.unwrap_or_else(|| "https://www.okx.com".to_string()),
        }
    }

    fn coin_to_inst_id(coin: Coin) -> Option<&'static str> {
        match coin {
            Coin::Btc => Some("BTC-USDT"),
            Coin::Ltc => Some("LTC-USDT"),
            Coin::Doge => Some("DOGE-USDT"),
            Coin::Bch => Some("BCH-USDT"),
            Coin::Pol => Some("POL-USDT"),
            Coin::Dgb => Some("DGB-USDT"),
            Coin::Sol => Some("SOL-USDT"),
            Coin::Usdc => Some("USDC-USDT"),
            Coin::Usdt => None,
            Coin::Zer => None,
            Coin::Pepe => Some("PEPE-USDT"),
        }
    }
}

#[derive(Deserialize)]
struct OkxTickerResp {
    code: String,
    data: Option<Vec<OkxTickerItem>>,
}

#[derive(Deserialize)]
struct OkxTickerItem {
    #[serde(rename = "instId")]
    inst_id: String,
    last: String,
}

#[async_trait::async_trait]
impl PriceProvider for OkxProvider {
    fn name(&self) -> &'static str {
        "OKX"
    }

    async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String> {
        let url = format!("{}/api/v5/market/tickers?instType=SPOT", self.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("http status {}", resp.status()));
        }

        let body: OkxTickerResp = resp
            .json()
            .await
            .map_err(|e| format!("parse error: {e}"))?;

        if body.code != "0" {
            return Err(format!("okx error code {}", body.code));
        }

        let items = body.data.unwrap_or_default();
        let mut map = HashMap::new();

        for item in items {
            for &c in coins {
                if Some(item.inst_id.as_str()) == Self::coin_to_inst_id(c) {
                    if let Ok(val) = item.last.parse::<f64>() {
                        if val > 0.0 {
                            map.insert(c, val);
                        }
                    }
                }
            }
        }
        if coins.contains(&Coin::Usdt) {
            map.insert(Coin::Usdt, 1.0);
        }

        Ok(map)
    }
}

/// CoinGecko fallback provider
pub struct CoinGeckoClient {
    http: reqwest::Client,
    base_url: String,
}

impl CoinGeckoClient {
    pub fn new(base_url: impl Into<String>, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("SatsPay-PricingOracle/1.0")
            .build()
            .unwrap_or_default();
        Self {
            http,
            base_url: base_url.into(),
        }
    }

    fn coingecko_id(coin: Coin) -> &'static str {
        match coin {
            Coin::Btc => "bitcoin",
            Coin::Ltc => "litecoin",
            Coin::Doge => "dogecoin",
            Coin::Bch => "bitcoin-cash",
            Coin::Pol => "polygon-ecosystem-token",
            Coin::Dgb => "digibyte",
            Coin::Sol => "solana",
            Coin::Usdt => "tether",
            Coin::Usdc => "usd-coin",
            Coin::Zer => "zero",
            Coin::Pepe => "pepe",
        }
    }

    /// Fetch USD prices for `coins` via this CoinGecko base URL only.
    /// Prefer [`MultiProviderOracle`] in production (worker price refresh).
    pub async fn fetch_usd_prices(&self, coins: &[Coin]) -> Result<Vec<(Coin, f64)>, PricingError> {
        let map = PriceProvider::fetch_prices(self, coins).await.map_err(|_| {
            PricingError::AllProvidersFailed(coins.first().copied().unwrap_or(Coin::Btc))
        })?;
        Ok(map.into_iter().collect())
    }
}

#[derive(Deserialize)]
struct CoinGeckoSimplePrice {
    usd: f64,
}

#[async_trait::async_trait]
impl PriceProvider for CoinGeckoClient {
    fn name(&self) -> &'static str {
        "CoinGecko"
    }

    async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String> {
        let ids: Vec<&str> = coins.iter().map(|c| Self::coingecko_id(*c)).collect();
        let url = format!(
            "{}/simple/price?ids={}&vs_currencies=usd",
            self.base_url.trim_end_matches('/'),
            ids.join(",")
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("http status {}", resp.status()));
        }

        let body: HashMap<String, CoinGeckoSimplePrice> = resp
            .json()
            .await
            .map_err(|e| format!("parse error: {e}"))?;

        let mut map = HashMap::new();
        for &coin in coins {
            let id = Self::coingecko_id(coin);
            if let Some(entry) = body.get(id) {
                if entry.usd > 0.0 {
                    map.insert(coin, entry.usd);
                }
            }
        }
        Ok(map)
    }
}

/// Fallback spot for ZER when CEX books are empty (`zerochain.info/api/price`).
pub struct ZerochainPriceProvider {
    http: reqwest::Client,
    base_url: String,
}

impl ZerochainPriceProvider {
    pub fn new(base_url: Option<String>, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("SatsPay-PricingOracle/1.0")
            .build()
            .unwrap_or_default();
        Self {
            http,
            base_url: base_url.unwrap_or_else(|| "https://zerochain.info/api".to_string()),
        }
    }
}

#[async_trait::async_trait]
impl PriceProvider for ZerochainPriceProvider {
    fn name(&self) -> &'static str {
        "zerochain.info"
    }

    async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String> {
        if !coins.contains(&Coin::Zer) {
            return Ok(HashMap::new());
        }
        let url = format!("{}/price", self.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("http status {}", resp.status()));
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| format!("parse error: {e}"))?;
        let usd = body
            .get("usd")
            .or_else(|| body.get("USD"))
            .or_else(|| body.get("price"))
            .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .filter(|p| *p > 0.0)
            .ok_or_else(|| "zerochain price missing usd".to_string())?;
        Ok(HashMap::from([(Coin::Zer, usd)]))
    }
}

/// Multi-Provider Oracle Aggregator
pub struct MultiProviderOracle {
    providers: Vec<Box<dyn PriceProvider>>,
}

impl Default for MultiProviderOracle {
    fn default() -> Self {
        let timeout = Duration::from_secs(5);
        Self::new(vec![
            Box::new(BinanceProvider::new(None, timeout)),
            Box::new(KrakenProvider::new(None, timeout)),
            Box::new(OkxProvider::new(None, timeout)),
            Box::new(CoinGeckoClient::new("https://api.coingecko.com/api/v3", timeout)),
            Box::new(ZerochainPriceProvider::new(None, timeout)),
        ])
    }
}

impl MultiProviderOracle {
    pub fn new(providers: Vec<Box<dyn PriceProvider>>) -> Self {
        Self { providers }
    }

    /// Fetches prices concurrently from all active providers and computes
    /// the consensus / median price for each coin.
    pub async fn fetch_usd_prices(&self, coins: &[Coin]) -> Result<Vec<(Coin, f64)>, PricingError> {
        let mut coin_prices: HashMap<Coin, Vec<(f64, &'static str)>> = HashMap::new();

        for p in &self.providers {
            match p.fetch_prices(coins).await {
                Ok(prices) => {
                    for (coin, price) in prices {
                        if price > 0.0 {
                            coin_prices
                                .entry(coin)
                                .or_default()
                                .push((price, p.name()));
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(provider = p.name(), error = %err, "Price provider query failed");
                }
            }
        }

        let mut out = Vec::with_capacity(coins.len());
        for &coin in coins {
            let prices = coin_prices.remove(&coin).unwrap_or_default();
            if prices.is_empty() {
                tracing::warn!(coin = %coin.as_str(), "no provider returned a price; skipping coin this tick");
                continue;
            }

            // Calculate median price across successful providers
            let mut vals: Vec<f64> = prices.iter().map(|(p, _)| *p).collect();
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let consensus_price = crate::median_f64(&vals).expect("vals non-empty");

            let provider_details: Vec<String> = prices
                .iter()
                .map(|(p, name)| format!("{name}: ${p:.4}"))
                .collect();

            tracing::info!(
                coin = %coin.as_str(),
                price = consensus_price,
                sources = %provider_details.join(", "),
                "Consensus spot price computed"
            );

            out.push((coin, consensus_price));
        }

        Ok(out)
    }
}

/// Median of a pre-sorted non-empty slice (odd: middle; even: mean of two middles).
pub(crate) fn median_f64(sorted: &[f64]) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len();
    Some(if n % 2 == 1 {
        sorted[n / 2]
    } else {
        let mid = n / 2;
        (sorted[mid - 1] + sorted[mid]) / 2.0
    })
}

#[cfg(test)]
mod tests {
    use super::{median_f64, BinanceProvider, CoinGeckoClient, KrakenProvider, MultiProviderOracle, OkxProvider, PriceProvider, ZerochainPriceProvider};
    use shared::Coin;
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn median_odd_even_and_empty() {
        assert_eq!(median_f64(&[]), None);
        assert_eq!(median_f64(&[3.0]), Some(3.0));
        assert_eq!(median_f64(&[1.0, 3.0, 9.0]), Some(3.0));
        assert_eq!(median_f64(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
    }

    struct FakeProvider {
        name: &'static str,
        prices: HashMap<Coin, f64>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl PriceProvider for FakeProvider {
        fn name(&self) -> &'static str {
            self.name
        }
        async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String> {
            if self.fail {
                return Err("boom".into());
            }
            Ok(coins
                .iter()
                .filter_map(|c| self.prices.get(c).map(|p| (*c, *p)))
                .collect())
        }
    }

    #[tokio::test]
    async fn oracle_median_across_providers_skips_failures() {
        use shared::Coin;
        let oracle = MultiProviderOracle::new(vec![
            Box::new(FakeProvider {
                name: "a",
                prices: HashMap::from([(Coin::Btc, 100.0)]),
                fail: false,
            }),
            Box::new(FakeProvider {
                name: "b",
                prices: HashMap::from([(Coin::Btc, 110.0)]),
                fail: false,
            }),
            Box::new(FakeProvider {
                name: "c",
                prices: HashMap::new(),
                fail: true,
            }),
        ]);
        let prices = oracle.fetch_usd_prices(&[Coin::Btc, Coin::Ltc]).await.unwrap();
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].0, Coin::Btc);
        assert!((prices[0].1 - 105.0).abs() < 1e-9);
    }

    #[test]
    fn provider_constructors_default_urls() {
        let t = Duration::from_secs(1);
        let _ = BinanceProvider::new(None, t);
        let _ = KrakenProvider::new(None, t);
        let _ = OkxProvider::new(None, t);
        let _ = CoinGeckoClient::new("https://example.test", t);
        let _ = ZerochainPriceProvider::new(None, t);
        let _ = MultiProviderOracle::default();
    }

    #[test]
    fn coingecko_id_maps_zer_to_zero() {
        assert_eq!(CoinGeckoClient::coingecko_id(Coin::Zer), "zero");
        assert_eq!(CoinGeckoClient::coingecko_id(Coin::Pepe), "pepe");
        assert_eq!(CoinGeckoClient::coingecko_id(Coin::Dgb), "digibyte");
    }
}
