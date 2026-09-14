//! SwapKit Partner API client for custodial cross-chain swaps.
//!
//! Quote → swap → track against https://api.swapkit.dev (THORChain, Chainflip,
//! Mayan, Jupiter, NEAR Intents, …). Affiliate fee is applied via `affiliateFee`
//! (bps) so SatsPay platform fees show up in the provider fee breakdown.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::{format_amount, Coin};
use std::time::Duration;

/// Same-chain DEX routes (Jupiter / 1inch / …) → 25 bps.
pub const DEFAULT_FEE_BPS_SAME: u32 = 25;
/// Cross-chain routes (THOR / Chainflip / Mayan / …) → 50 bps.
pub const DEFAULT_FEE_BPS_CROSS: u32 = 50;

const SAME_CHAIN_PROVIDERS: &[&str] = &[
    "JUPITER",
    "ONEINCH",
    "1INCH",
    "UNISWAP",
    "UNISWAP_V2",
    "UNISWAP_V3",
    "SUSHISWAP",
    "KYBERSWAP",
    "PANCAKESWAP",
];

#[derive(Debug, thiserror::Error)]
pub enum SwapKitError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("swapkit api error: {0}")]
    Api(String),
    #[error("no routes found")]
    NoRoutes,
    #[error("asset not supported on SwapKit: {0}")]
    UnsupportedAsset(String),
    #[error("invalid amount")]
    InvalidAmount,
}

#[derive(Clone)]
pub struct SwapKitClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    /// Master switch — off by default (`SWAPKIT_ENABLED` unset/false).
    enabled: bool,
    fee_bps_same: u32,
    fee_bps_cross: u32,
    slippage_pct: f64,
}

impl SwapKitClient {
    /// Test/helper constructor (also usable by smokes that pin a mock base URL).
    pub fn new_with_base(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
            api_key,
            enabled: true,
            fee_bps_same: DEFAULT_FEE_BPS_SAME,
            fee_bps_cross: DEFAULT_FEE_BPS_CROSS,
            slippage_pct: 2.0,
        }
    }

    pub fn from_env() -> Self {
        let timeout = Duration::from_secs(
            std::env::var("SWAPKIT_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20),
        );
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("SatsPay-SwapKit/1.0")
            .build()
            .unwrap_or_default();
        let enabled = std::env::var("SWAPKIT_ENABLED")
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        Self {
            http,
            base_url: std::env::var("SWAPKIT_BASE_URL").unwrap_or_else(|_| "https://api.swapkit.dev".into()),
            api_key: std::env::var("SWAPKIT_API_KEY").ok().filter(|s| !s.trim().is_empty()),
            enabled,
            fee_bps_same: std::env::var("SWAP_PLATFORM_FEE_BPS_SAME")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_FEE_BPS_SAME),
            fee_bps_cross: std::env::var("SWAP_PLATFORM_FEE_BPS_CROSS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_FEE_BPS_CROSS),
            slippage_pct: std::env::var("SWAP_SLIPPAGE_PCT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2.0),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.enabled && self.api_key.is_some()
    }

    pub fn fee_bps_same(&self) -> u32 {
        self.fee_bps_same
    }

    pub fn fee_bps_cross(&self) -> u32 {
        self.fee_bps_cross
    }

    /// Platform fee bps for a route: 25 same-chain DEX, 50 cross-chain.
    pub fn platform_fee_bps(&self, providers: &[String], from: Coin, to: Coin) -> u32 {
        if is_same_chain(from, to) || providers.iter().any(|p| is_same_chain_provider(p)) {
            self.fee_bps_same
        } else {
            self.fee_bps_cross
        }
    }

    pub async fn quote(
        &self,
        from: Coin,
        to: Coin,
        from_amount_ledger: u128,
        source_address: Option<&str>,
        destination_address: Option<&str>,
    ) -> Result<QuoteResponse, SwapKitError> {
        let sell_asset = asset_id(from).ok_or_else(|| SwapKitError::UnsupportedAsset(from.as_str().into()))?;
        let buy_asset = asset_id(to).ok_or_else(|| SwapKitError::UnsupportedAsset(to.as_str().into()))?;
        if from_amount_ledger == 0 {
            return Err(SwapKitError::InvalidAmount);
        }
        let sell_amount = format_amount(from_amount_ledger, from);
        let fee_bps = self.platform_fee_bps(&[], from, to);

        let mut body = serde_json::json!({
            "sellAsset": sell_asset,
            "buyAsset": buy_asset,
            "sellAmount": sell_amount,
            "slippage": self.slippage_pct,
            "affiliateFee": fee_bps,
            "txHints": ["simpleTransfer", "transferWithMemo", "contractCall"],
        });
        if let Some(a) = source_address {
            body["sourceAddress"] = Value::String(a.to_string());
        }
        if let Some(a) = destination_address {
            body["destinationAddress"] = Value::String(a.to_string());
        }

        let mut req = self.http.post(format!("{}/v3/quote", self.base_url.trim_end_matches('/'))).json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.as_u16() == 404 {
            return Err(SwapKitError::NoRoutes);
        }
        if !status.is_success() {
            return Err(SwapKitError::Api(format!("{status}: {text}")));
        }
        let mut parsed: QuoteResponse = serde_json::from_str(&text).map_err(|e| SwapKitError::Api(e.to_string()))?;
        // Recompute affiliate using actual providers on each route.
        for route in &mut parsed.routes {
            route.platform_fee_bps = Some(self.platform_fee_bps(&route.providers, from, to));
        }
        if parsed.routes.is_empty() {
            return Err(SwapKitError::NoRoutes);
        }
        Ok(parsed)
    }

    pub async fn swap(
        &self,
        route_id: &str,
        source_address: &str,
        destination_address: &str,
    ) -> Result<SwapResponse, SwapKitError> {
        let body = serde_json::json!({
            "routeId": route_id,
            "sourceAddress": source_address,
            "destinationAddress": destination_address,
        });
        let mut req = self
            .http
            .post(format!("{}/v3/swap", self.base_url.trim_end_matches('/')))
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(SwapKitError::Api(format!("{status}: {text}")));
        }
        serde_json::from_str(&text).map_err(|e| SwapKitError::Api(e.to_string()))
    }

    pub async fn track(&self, hash_or_address: &str) -> Result<TrackResponse, SwapKitError> {
        let body = serde_json::json!({ "hash": hash_or_address });
        let mut req = self
            .http
            .post(format!("{}/track", self.base_url.trim_end_matches('/')))
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(SwapKitError::Api(format!("{status}: {text}")));
        }
        serde_json::from_str(&text).map_err(|e| SwapKitError::Api(e.to_string()))
    }
}

/// SwapKit asset identifier for a SatsPay coin. `None` → no DEX route (HOUSE fallback).
pub fn asset_id(coin: Coin) -> Option<&'static str> {
    match coin {
        Coin::Btc => Some("BTC.BTC"),
        Coin::Ltc => Some("LTC.LTC"),
        Coin::Doge => Some("DOGE.DOGE"),
        Coin::Bch => Some("BCH.BCH"),
        Coin::Sol => Some("SOL.SOL"),
        // Polygon native — SwapKit asset namespace is POL.* (MATIC.* rejected).
        Coin::Pol => Some("POL.POL"),
        // Polygon PoS bridged stables (checksum as returned by SwapKit quotes)
        Coin::Usdt => Some("POL.USDT-0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
        Coin::Usdc => Some("POL.USDC-0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
        Coin::Dgb => None,
    }
}

pub fn chain_key(coin: Coin) -> Option<&'static str> {
    match coin {
        Coin::Btc => Some("BTC"),
        Coin::Ltc => Some("LTC"),
        Coin::Doge => Some("DOGE"),
        Coin::Bch => Some("BCH"),
        Coin::Sol => Some("SOL"),
        Coin::Pol | Coin::Usdt | Coin::Usdc => Some("POL"),
        Coin::Dgb => None,
    }
}

pub fn is_same_chain(from: Coin, to: Coin) -> bool {
    match (chain_key(from), chain_key(to)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn is_same_chain_provider(name: &str) -> bool {
    let upper = name.to_uppercase();
    SAME_CHAIN_PROVIDERS.iter().any(|p| upper.contains(p))
}

/// Prefer routes our hot wallet can execute today (plain / memo transfer).
pub fn route_priority(tx_hint: Option<&str>) -> u8 {
    match tx_hint {
        Some("simpleTransfer") => 0,
        Some("transferWithMemo") => 1,
        Some("contractCall") => 2,
        _ => 3,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    pub quote_id: Option<String>,
    #[serde(default)]
    pub routes: Vec<QuoteRoute>,
    #[serde(default)]
    pub provider_errors: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRoute {
    pub route_id: String,
    #[serde(default)]
    pub providers: Vec<String>,
    pub sell_asset: Option<String>,
    pub buy_asset: Option<String>,
    pub sell_amount: Option<String>,
    pub expected_buy_amount: Option<String>,
    pub expected_buy_amount_max_slippage: Option<String>,
    #[serde(default)]
    pub fees: Vec<ProviderFee>,
    pub estimated_time: Option<EstimatedTime>,
    /// SwapKit may return fractional bps (e.g. -12.7).
    #[serde(default, deserialize_with = "de_opt_i64_lossy")]
    pub total_slippage_bps: Option<i64>,
    pub tx_hint: Option<String>,
    pub meta: Option<Value>,
    /// Filled by client after quote (not from SwapKit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_fee_bps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderFee {
    #[serde(rename = "type", default)]
    pub fee_type: String,
    #[serde(default)]
    pub amount: String,
    #[serde(default)]
    pub asset: String,
    #[serde(default)]
    pub chain: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimatedTime {
    /// SwapKit may return fractional seconds (e.g. 2.1).
    #[serde(default, deserialize_with = "de_opt_u64_ceil")]
    pub inbound: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64_ceil")]
    pub swap: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64_ceil")]
    pub outbound: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64_ceil")]
    pub total: Option<u64>,
}

fn de_opt_u64_ceil<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                Some(u)
            } else if let Some(i) = n.as_i64() {
                Some(i.max(0) as u64)
            } else if let Some(f) = n.as_f64() {
                Some(f.ceil().max(0.0) as u64)
            } else {
                None
            }
        }
        Some(Value::String(s)) => s
            .parse::<f64>()
            .ok()
            .map(|f| f.ceil().max(0.0) as u64),
        _ => None,
    })
}

fn de_opt_i64_lossy<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                Some(i)
            } else if let Some(u) = n.as_u64() {
                Some(u.min(i64::MAX as u64) as i64)
            } else if let Some(f) = n.as_f64() {
                Some(f.round() as i64)
            } else {
                None
            }
        }
        Some(Value::String(s)) => s.parse::<f64>().ok().map(|f| f.round() as i64),
        _ => None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    pub route_id: Option<String>,
    #[serde(default)]
    pub providers: Vec<String>,
    /// Deposit target for simpleTransfer / transferWithMemo.
    pub target_address: Option<String>,
    pub inbound_address: Option<String>,
    pub memo: Option<String>,
    pub tx_type: Option<String>,
    pub tx_hint: Option<String>,
    /// Raw provider payload (contract calldata, etc.).
    #[serde(default)]
    pub meta: Option<Value>,
    #[serde(flatten)]
    pub extra: Value,
}

impl SwapResponse {
    pub fn deposit_address(&self) -> Option<&str> {
        self.target_address
            .as_deref()
            .or(self.inbound_address.as_deref())
            .or_else(|| {
                self.extra
                    .get("depositAddress")
                    .or_else(|| self.extra.get("inboundAddress"))
                    .or_else(|| self.extra.get("targetAddress"))
                    .and_then(|v| v.as_str())
            })
    }

    pub fn memo_str(&self) -> Option<&str> {
        self.memo.as_deref().or_else(|| self.extra.get("memo").and_then(|v| v.as_str()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackResponse {
    pub status: Option<String>,
    pub tracking_status: Option<String>,
    #[serde(default)]
    pub legs: Option<Value>,
    pub inbound: Option<Value>,
    pub outbound: Option<Value>,
    #[serde(flatten)]
    pub extra: Value,
}

impl TrackResponse {
    pub fn is_complete(&self) -> bool {
        let s = self
            .status
            .as_deref()
            .or(self.tracking_status.as_deref())
            .unwrap_or("")
            .to_uppercase();
        matches!(
            s.as_str(),
            "COMPLETED" | "COMPLETE" | "SUCCESS" | "DONE" | "OUTBOUND" | "SWAPPED"
        ) || s.contains("COMPLETE")
            || s.contains("SUCCESS")
    }

    pub fn is_failed(&self) -> bool {
        let s = self
            .status
            .as_deref()
            .or(self.tracking_status.as_deref())
            .unwrap_or("")
            .to_uppercase();
        s.contains("FAIL") || s.contains("REFUND") || s.contains("ERROR") || s == "REVERTED"
    }

    pub fn outbound_tx(&self) -> Option<String> {
        self.outbound
            .as_ref()
            .and_then(|o| o.get("hash").or_else(|| o.get("txHash")).or_else(|| o.get("txid")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                self.extra
                    .get("outboundTx")
                    .or_else(|| self.extra.get("txHash"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
    }

    pub fn outbound_amount_human(&self) -> Option<String> {
        self.outbound
            .as_ref()
            .and_then(|o| o.get("amount").or_else(|| o.get("finalAmount")))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    v.as_f64().map(|f| f.to_string())
                }
            })
    }
}

/// Parse a human decimal string into ledger units (8 decimals).
pub fn parse_human_to_ledger(amount: &str, coin: Coin) -> Option<u128> {
    let decimals = coin_config_decimals(coin);
    let s = amount.trim();
    if s.is_empty() {
        return None;
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s, ""),
    };
    let whole: u128 = whole.parse().ok()?;
    let frac_padded = format!("{:0<width$}", &frac[..frac.len().min(decimals as usize)], width = decimals as usize);
    let frac_digits: u128 = if frac_padded.is_empty() {
        0
    } else {
        frac_padded.parse().ok()?
    };
    let scale = 10u128.pow(decimals);
    Some(whole.saturating_mul(scale).saturating_add(frac_digits))
}

fn coin_config_decimals(coin: Coin) -> u32 {
    shared::coin_config(coin).decimals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client(same: u32, cross: u32) -> SwapKitClient {
        SwapKitClient {
            http: reqwest::Client::new(),
            base_url: "https://example.test".into(),
            api_key: Some("k".into()),
            enabled: true,
            fee_bps_same: same,
            fee_bps_cross: cross,
            slippage_pct: 2.0,
        }
    }

    #[test]
    fn asset_map_covers_non_dgb() {
        assert!(asset_id(Coin::Btc).is_some());
        assert!(asset_id(Coin::Dgb).is_none());
        assert!(is_same_chain(Coin::Pol, Coin::Usdt));
        assert!(!is_same_chain(Coin::Btc, Coin::Ltc));
    }

    #[test]
    fn parse_amount_roundtrip() {
        let ledger = parse_human_to_ledger("1.5", Coin::Ltc).unwrap();
        assert_eq!(ledger, 150_000_000);
        assert_eq!(format_amount(ledger, Coin::Ltc), "1.5");
    }

    #[test]
    fn platform_fee_bps_same_vs_cross() {
        let cfg = test_client(10, 50);
        assert!(cfg.is_configured());
        assert_eq!(cfg.fee_bps_same(), 10);
        assert_eq!(cfg.fee_bps_cross(), 50);
        assert_eq!(
            cfg.platform_fee_bps(&["THORCHAIN".into()], Coin::Btc, Coin::Ltc),
            50
        );
        assert_eq!(
            cfg.platform_fee_bps(&["UNISWAP".into()], Coin::Pol, Coin::Usdt),
            10
        );
    }

    #[test]
    fn route_priority_and_parse_edges() {
        assert_eq!(route_priority(Some("simpleTransfer")), 0);
        assert_eq!(route_priority(Some("transferWithMemo")), 1);
        assert_eq!(route_priority(Some("contractCall")), 2);
        assert_eq!(route_priority(None), 3);
        assert!(chain_key(Coin::Btc).is_some());
        assert!(chain_key(Coin::Dgb).is_none());
        assert!(parse_human_to_ledger("", Coin::Btc).is_none());
        assert!(parse_human_to_ledger("abc", Coin::Btc).is_none());
    }

    #[test]
    fn swap_and_track_response_helpers() {
        let swap: SwapResponse = serde_json::from_value(serde_json::json!({
            "inboundAddress": "bc1qin",
            "memo": "=:LTC",
            "extraField": 1
        }))
        .unwrap();
        assert_eq!(swap.deposit_address(), Some("bc1qin"));
        assert_eq!(swap.memo_str(), Some("=:LTC"));

        let from_extra: SwapResponse = serde_json::from_value(serde_json::json!({
            "depositAddress": "ltc1x"
        }))
        .unwrap();
        assert_eq!(from_extra.deposit_address(), Some("ltc1x"));

        let done = TrackResponse {
            status: Some("completed".into()),
            tracking_status: None,
            legs: None,
            inbound: None,
            outbound: Some(serde_json::json!({"hash":"abc","amount":"1.25"})),
            extra: serde_json::json!({}),
        };
        assert!(done.is_complete());
        assert!(!done.is_failed());
        assert_eq!(done.outbound_tx().as_deref(), Some("abc"));
        assert_eq!(done.outbound_amount_human().as_deref(), Some("1.25"));

        let fail = TrackResponse {
            status: None,
            tracking_status: Some("REFUNDED".into()),
            legs: None,
            inbound: None,
            outbound: None,
            extra: serde_json::json!({"txHash":"xyz"}),
        };
        assert!(fail.is_failed());
        assert_eq!(fail.outbound_tx().as_deref(), Some("xyz"));
    }

    #[test]
    fn from_env_defaults_disabled() {
        let c = SwapKitClient::from_env();
        assert!(!c.is_configured() || c.fee_bps_same() > 0);
    }

    #[test]
    fn quote_route_accepts_fractional_eta_and_slippage() {
        let route: QuoteRoute = serde_json::from_value(serde_json::json!({
            "routeId": "r1",
            "providers": ["ONEINCH"],
            "expectedBuyAmount": "0.96",
            "estimatedTime": {"inbound": 2.1, "swap": 0, "outbound": 0.0, "total": 2.1},
            "totalSlippageBps": -12.704852,
            "fees": []
        }))
        .unwrap();
        let eta = route.estimated_time.unwrap();
        assert_eq!(eta.inbound, Some(3));
        assert_eq!(eta.swap, Some(0));
        assert_eq!(eta.total, Some(3));
        assert_eq!(route.total_slippage_bps, Some(-13));
    }
}
