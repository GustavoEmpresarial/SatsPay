//! Direct Relay Protocol HTTP client (`https://api.relay.link`).
//!
//! Same-chain Polygon swaps + cross-chain SOL ↔ Polygon bridges via `/quote/v2`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::{to_onchain_amount, Coin};
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.relay.link";
pub const POLYGON_CHAIN_ID: u64 = 137;
pub const SOLANA_CHAIN_ID: u64 = 792_703_809;
pub const BSC_CHAIN_ID: u64 = 56;
const EVM_NATIVE: &str = "0x0000000000000000000000000000000000000000";
/// Relay docs: native SOL mint sentinel (not wSOL).
const SOL_NATIVE: &str = "11111111111111111111111111111111";
/// Binance-Peg PEPE (BEP-20) — SatsPay custodial PEPE, not Ethereum PEPE.
const BSC_PEPE: &str = "0x25d887ce7a35172c62febfd67a1856f20faebb00";

#[derive(Debug, Clone, Copy)]
pub struct RelayAsset {
    pub chain_id: u64,
    pub currency: &'static str,
}

pub fn relay_asset(coin: Coin) -> Option<RelayAsset> {
    match coin {
        Coin::Pol => Some(RelayAsset {
            chain_id: POLYGON_CHAIN_ID,
            currency: EVM_NATIVE,
        }),
        Coin::Usdt => Some(RelayAsset {
            chain_id: POLYGON_CHAIN_ID,
            currency: "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",
        }),
        Coin::Usdc => Some(RelayAsset {
            chain_id: POLYGON_CHAIN_ID,
            currency: "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
        }),
        Coin::Sol => Some(RelayAsset {
            chain_id: SOLANA_CHAIN_ID,
            currency: SOL_NATIVE,
        }),
        Coin::Pepe => Some(RelayAsset {
            chain_id: BSC_CHAIN_ID,
            currency: BSC_PEPE,
        }),
        _ => None,
    }
}

pub fn is_polygon_l2(coin: Coin) -> bool {
    matches!(coin, Coin::Pol | Coin::Usdt | Coin::Usdc)
}

pub fn is_bsc_pepe(coin: Coin) -> bool {
    coin == Coin::Pepe
}

pub fn is_bridge_pair(from: Coin, to: Coin) -> bool {
    if from == to {
        return false;
    }
    // SOL ↔ Polygon (existing)
    if (from == Coin::Sol && is_polygon_l2(to)) || (to == Coin::Sol && is_polygon_l2(from)) {
        return true;
    }
    // PEPE (BSC) ↔ Polygon or SOL
    if is_bsc_pepe(from) && (is_polygon_l2(to) || to == Coin::Sol) {
        return true;
    }
    if is_bsc_pepe(to) && (is_polygon_l2(from) || from == Coin::Sol) {
        return true;
    }
    false
}

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("relay not configured (set RELAY_ENABLED=true)")]
    NotConfigured,
    #[error("unsupported coin for relay: {0:?}")]
    UnsupportedCoin(Coin),
    #[error("relay http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("relay: {0}")]
    Msg(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

impl RelayError {
    /// Relay puts a machine-readable `errorCode` in its 4xx bodies, e.g.
    /// `AMOUNT_TOO_LOW` when route fees would exceed the swap output. Callers
    /// map it to their own stable code — the raw body carries a requestId and
    /// provider internals that should never reach an end user.
    pub fn upstream_code(&self) -> Option<String> {
        let Self::Http { body, .. } = self else { return None };
        serde_json::from_str::<Value>(body)
            .ok()?
            .get("errorCode")?
            .as_str()
            .map(str::to_string)
    }
}

#[derive(Clone)]
pub struct RelayClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    enabled: bool,
}

impl RelayClient {
    pub fn from_env() -> Self {
        let enabled = std::env::var("RELAY_ENABLED")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        let base_url = std::env::var("RELAY_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let api_key = std::env::var("RELAY_API_KEY").ok().filter(|s| !s.trim().is_empty());
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            enabled,
        }
    }

    pub fn new_for_tests(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: None,
            enabled: true,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.enabled
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn supports_pair(from: Coin, to: Coin) -> bool {
        if from == to {
            return false;
        }
        // Same-chain Polygon
        if is_polygon_l2(from) && is_polygon_l2(to) {
            return true;
        }
        is_bridge_pair(from, to)
    }

    pub async fn quote(
        &self,
        from: Coin,
        to: Coin,
        amount_ledger: u128,
        user: &str,
        recipient: &str,
    ) -> Result<RelayQuote, RelayError> {
        if !self.enabled {
            return Err(RelayError::NotConfigured);
        }
        let origin = relay_asset(from).ok_or(RelayError::UnsupportedCoin(from))?;
        let dest = relay_asset(to).ok_or(RelayError::UnsupportedCoin(to))?;
        let amount = to_onchain_amount(from, amount_ledger).to_string();

        let mut body = json!({
            "user": user,
            "recipient": recipient,
            "originChainId": origin.chain_id,
            "destinationChainId": dest.chain_id,
            "originCurrency": origin.currency,
            "destinationCurrency": dest.currency,
            "amount": amount,
            "tradeType": "EXACT_INPUT",
            "explicitDeposit": false,
        });
        // Keep Solana-origin txs under the 1232-byte wire limit.
        if origin.chain_id == SOLANA_CHAIN_ID {
            body["maxRouteLength"] = json!(4);
        }

        let mut req = self.http.post(format!("{}/quote/v2", self.base_url)).json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(RelayError::Http {
                status: status.as_u16(),
                body: text.chars().take(800).collect(),
            });
        }
        let value: Value = serde_json::from_str(&text)?;
        parse_quote_response(&value, from, to)
    }

    /// Poll intent status. Returns Ok(true) when complete/success.
    pub async fn intent_status(&self, request_id: &str) -> Result<RelayIntentStatus, RelayError> {
        if !self.enabled {
            return Err(RelayError::NotConfigured);
        }
        let url = format!("{}/intents/status/v3?requestId={request_id}", self.base_url);
        let mut req = self.http.get(&url);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(RelayError::Http {
                status: status.as_u16(),
                body: text.chars().take(800).collect(),
            });
        }
        let value: Value = serde_json::from_str(&text)?;
        Ok(RelayIntentStatus::from_value(&value))
    }
}

#[derive(Debug, Clone)]
pub struct RelayIntentStatus {
    pub status: String,
    pub outbound_tx: Option<String>,
    pub raw: Value,
}

impl RelayIntentStatus {
    pub fn from_value(value: &Value) -> Self {
        let status = value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let outbound_tx = value
            .pointer("/txHashes/1")
            .and_then(|v| v.as_str())
            .or_else(|| value.pointer("/destinationTxHash").and_then(|v| v.as_str()))
            .or_else(|| value.get("txHash").and_then(|v| v.as_str()))
            .or_else(|| {
                value
                    .get("txHashes")
                    .and_then(|a| a.as_array())
                    .and_then(|arr| arr.last())
                    .and_then(|v| v.as_str())
            })
            .map(str::to_string);
        Self {
            status,
            outbound_tx,
            raw: value.clone(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(
            self.status.to_ascii_lowercase().as_str(),
            "success" | "completed" | "complete" | "fulfilled" | "settled"
        )
    }

    pub fn is_failed(&self) -> bool {
        matches!(
            self.status.to_ascii_lowercase().as_str(),
            "failure" | "failed" | "refunded" | "cancelled" | "canceled" | "error"
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayEvmTx {
    pub to: String,
    pub data: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub chain_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStepItem {
    pub status: String,
    /// EVM: `{to,data,value}` — Solana: `{instructions,addressLookupTableAddresses}`.
    pub data: Value,
    /// String URL or `{endpoint, method}` object.
    #[serde(default)]
    pub check: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStep {
    pub id: String,
    pub action: String,
    pub description: String,
    pub kind: String,
    #[serde(default)]
    pub items: Vec<RelayStepItem>,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayNetworkFee {
    /// Stable key for UI (`relayer`, `relayerService`, `gas`, …).
    pub fee_type: String,
    /// Human-readable amount (prefer Relay `amountFormatted`).
    pub amount: String,
    pub asset: String,
    #[serde(default)]
    pub amount_usd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayQuote {
    pub route_id: String,
    pub expected_out_ledger: u128,
    pub expected_out_onchain: String,
    pub steps: Vec<RelayStep>,
    pub approval_address: Option<String>,
    pub request_id: Option<String>,
    pub check_url: Option<String>,
    pub origin_chain_id: u64,
    pub destination_chain_id: u64,
    pub is_bridge: bool,
    /// `contractCall` (EVM origin) or `solanaRelay` (SOL origin).
    pub tx_hint: &'static str,
    /// Protocol / gas fees from Relay `fees` object (not SatsPay).
    pub network_fees: Vec<RelayNetworkFee>,
    pub raw: Value,
}

impl RelayQuote {
    fn first_item(step: &RelayStep) -> Option<&RelayStepItem> {
        step.items
            .iter()
            .find(|i| {
                i.status.eq_ignore_ascii_case("incomplete") || i.status.eq_ignore_ascii_case("ready")
            })
            .or_else(|| step.items.first())
    }

    pub fn executable_step(&self) -> Option<&RelayStep> {
        self.steps.iter().rev().find(|s| {
            if !s.kind.eq_ignore_ascii_case("transaction") {
                return false;
            }
            if s.id.eq_ignore_ascii_case("approve") {
                return false;
            }
            Self::first_item(s).is_some()
        })
    }

    pub fn evm_tx(&self) -> Option<RelayEvmTx> {
        let item = self.executable_step().and_then(Self::first_item)?;
        let to = item.data.get("to").and_then(|v| v.as_str())?.to_string();
        let data = item.data.get("data").and_then(|v| v.as_str())?.to_string();
        if to.is_empty() || data.is_empty() {
            return None;
        }
        Some(RelayEvmTx {
            to,
            data,
            value: item
                .data
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("0")
                .to_string(),
            chain_id: item.data.get("chainId").and_then(|v| v.as_u64()),
        })
    }

    pub fn solana_tx_data(&self) -> Option<&Value> {
        let item = self.executable_step().and_then(Self::first_item)?;
        if item.data.get("instructions").is_some() {
            Some(&item.data)
        } else {
            None
        }
    }

    pub fn to_swap_payload(&self) -> Value {
        let mut payload = json!({
            "provider": "relay",
            "source": "relay",
            "routeId": self.route_id,
            "requestId": self.request_id,
            "checkUrl": self.check_url,
            "approvalAddress": self.approval_address,
            "expectedBuyAmountOnchain": self.expected_out_onchain,
            "expectedBuyAmountLedger": self.expected_out_ledger.to_string(),
            "originChainId": self.origin_chain_id,
            "destinationChainId": self.destination_chain_id,
            "isBridge": self.is_bridge,
            "txHint": self.tx_hint,
            "networkFees": self.network_fees,
            "steps": self.steps,
        });
        if let Some(tx) = self.evm_tx() {
            payload["tx"] = json!({
                "to": tx.to,
                "data": tx.data,
                "value": if tx.value.is_empty() { "0" } else { tx.value.as_str() },
            });
        }
        if let Some(sol) = self.solana_tx_data() {
            payload["solanaTx"] = sol.clone();
        }
        payload
    }
}

fn extract_check_url(base: &str, item: &RelayStepItem) -> Option<String> {
    let check = item.check.as_ref()?;
    if let Some(s) = check.as_str() {
        if s.starts_with("http") {
            return Some(s.to_string());
        }
        return Some(format!("{base}{s}"));
    }
    let endpoint = check.get("endpoint").and_then(|v| v.as_str())?;
    if endpoint.starts_with("http") {
        Some(endpoint.to_string())
    } else {
        Some(format!("{base}{endpoint}"))
    }
}

fn erc20_approve_spender(data_hex: &str) -> Option<String> {
    let hex = data_hex
        .trim()
        .strip_prefix("0x")
        .or_else(|| data_hex.strip_prefix("0X"))?;
    if hex.len() < 8 + 64 {
        return None;
    }
    if !hex.get(..8)?.eq_ignore_ascii_case("095ea7b3") {
        return None;
    }
    let word = hex.get(8..8 + 64)?;
    let addr_hex = word.get(24..64)?;
    Some(format!("0x{addr_hex}"))
}

fn step_has_executable(step: &RelayStep) -> bool {
    if !step.kind.eq_ignore_ascii_case("transaction") {
        return false;
    }
    step.items.iter().any(|i| {
        let d = &i.data;
        let evm = d
            .get("to")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .is_some()
            && d
                .get("data")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .is_some();
        let sol = d
            .get("instructions")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        evm || sol
    })
}

fn parse_network_fees(value: &Value) -> Vec<RelayNetworkFee> {
    let Some(fees) = value.get("fees").and_then(|f| f.as_object()) else {
        return vec![];
    };
    // Prefer the most informative breakdown; skip zero / duplicate rollups.
    const ORDER: &[&str] = &[
        "relayerService",
        "relayerGas",
        "relayer",
        "gas",
        "app",
        "subsidized",
    ];
    let mut out = Vec::new();
    let mut seen_relayer_parts = false;
    for key in ORDER {
        let Some(entry) = fees.get(*key) else { continue };
        let amount_fmt = entry
            .get("amountFormatted")
            .and_then(|v| v.as_str())
            .unwrap_or("0");
        let amount_raw = entry.get("amount").and_then(|v| v.as_str()).unwrap_or("0");
        if amount_raw == "0" || amount_fmt == "0" || amount_fmt == "0.0" {
            continue;
        }
        // If we already have service+gas, skip the aggregate `relayer` total.
        if *key == "relayer" && seen_relayer_parts {
            continue;
        }
        if matches!(*key, "relayerService" | "relayerGas") {
            seen_relayer_parts = true;
        }
        let asset = entry
            .pointer("/currency/symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        let amount_usd = entry
            .get("amountUsd")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "0")
            .map(str::to_string);
        out.push(RelayNetworkFee {
            fee_type: (*key).to_string(),
            amount: amount_fmt.to_string(),
            asset,
            amount_usd,
        });
    }
    out
}

fn parse_quote_response(value: &Value, from: Coin, to: Coin) -> Result<RelayQuote, RelayError> {
    let origin = relay_asset(from).ok_or(RelayError::UnsupportedCoin(from))?;
    let dest = relay_asset(to).ok_or(RelayError::UnsupportedCoin(to))?;

    let steps: Vec<RelayStep> = serde_json::from_value(
        value.get("steps").cloned().unwrap_or(Value::Array(vec![])),
    )
    .map_err(|e| RelayError::Msg(format!("steps parse: {e}")))?;

    if !steps.iter().any(step_has_executable) {
        return Err(RelayError::Msg(
            "no executable transaction steps in relay quote".into(),
        ));
    }

    let mut approval_address = steps
        .iter()
        .find(|s| {
            s.id.eq_ignore_ascii_case("approve")
                || s.action.to_ascii_lowercase().contains("approv")
                || s.description.to_ascii_lowercase().contains("approv")
        })
        .and_then(RelayQuote::first_item)
        .and_then(|i| {
            i.data
                .get("data")
                .and_then(|v| v.as_str())
                .and_then(erc20_approve_spender)
        });

    // Bridge deposits often omit a dedicated approve step — approve the deposit `to`.
    if approval_address.is_none() && matches!(from, Coin::Usdt | Coin::Usdc | Coin::Pepe) {
        approval_address = steps
            .iter()
            .rev()
            .find(|s| step_has_executable(s) && !s.id.eq_ignore_ascii_case("approve"))
            .and_then(RelayQuote::first_item)
            .and_then(|i| i.data.get("to").and_then(|v| v.as_str()))
            .filter(|a| a.starts_with("0x") && a.len() == 42)
            .map(str::to_string);
    }

    let expected_out_onchain = value
        .pointer("/details/currencyOut/amount")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| RelayError::Msg("relay quote missing details.currencyOut.amount".into()))?;

    let onchain_u128: u128 = expected_out_onchain
        .parse()
        .map_err(|_| RelayError::Msg(format!("bad currencyOut.amount: {expected_out_onchain}")))?;
    let expected_out_ledger = shared::from_onchain_amount(to, onchain_u128);

    let request_id = value
        .get("requestId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            steps
                .iter()
                .find_map(|s| s.request_id.clone())
                .or_else(|| {
                    value
                        .pointer("/steps/0/requestId")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
        });

    let check_url = steps.iter().rev().find_map(|s| {
        RelayQuote::first_item(s).and_then(|i| extract_check_url(DEFAULT_BASE_URL, i))
    });

    let is_bridge = origin.chain_id != dest.chain_id;
    let tx_hint = if origin.chain_id == SOLANA_CHAIN_ID {
        "solanaRelay"
    } else {
        "contractCall"
    };

    let route_id = format!(
        "relay:{}",
        request_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
    );

    Ok(RelayQuote {
        route_id,
        expected_out_ledger,
        expected_out_onchain,
        steps,
        approval_address,
        request_id,
        check_url,
        origin_chain_id: origin.chain_id,
        destination_chain_id: dest.chain_id,
        is_bridge,
        tx_hint,
        network_fees: parse_network_fees(value),
        raw: value.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_quote_json() -> Value {
        json!({
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
                    "description": "Swap USDT to USDC",
                    "kind": "transaction",
                    "items": [{
                        "status": "incomplete",
                        "data": {
                            "to": "0xf574Fd487255dd69852e861250208640f040948a",
                            "data": "0xdeadbeef",
                            "value": "0",
                            "chainId": 137
                        },
                        "check": "https://api.relay.link/intents/status/v2?requestId=abc-123"
                    }]
                }
            ],
            "details": {
                "currencyOut": {
                    "currency": { "symbol": "USDC", "decimals": 6 },
                    "amount": "995000",
                    "amountFormatted": "0.995"
                }
            }
        })
    }

    #[test]
    fn parses_approve_and_swap_steps() {
        let q = parse_quote_response(&sample_quote_json(), Coin::Usdt, Coin::Usdc).expect("parse");
        assert_eq!(q.expected_out_onchain, "995000");
        assert_eq!(q.expected_out_ledger, 99_500_000);
        assert_eq!(
            q.approval_address.as_deref(),
            Some("0x6c0ad82f9721a6dc986381d19338601a2e6370e5")
        );
        let swap = q.evm_tx().expect("evm tx");
        assert_eq!(swap.to, "0xf574Fd487255dd69852e861250208640f040948a");
        assert!(!q.is_bridge);
        assert_eq!(q.tx_hint, "contractCall");
        assert!(q.check_url.as_ref().unwrap().contains("requestId=abc-123"));
    }

    #[test]
    fn parses_solana_bridge_deposit() {
        let v = json!({
            "requestId": "0xbridge1",
            "steps": [{
                "id": "deposit",
                "action": "Confirm transaction in your wallet",
                "description": "Depositing funds",
                "kind": "transaction",
                "items": [{
                    "status": "incomplete",
                    "data": {
                        "instructions": [{
                            "keys": [{"pubkey":"Aaa","isSigner":true,"isWritable":true}],
                            "programId": "11111111111111111111111111111111",
                            "data": "00"
                        }],
                        "addressLookupTableAddresses": ["Hm9fUgcn7qwDaiNTFiGh6pNtVATgnaRcmK6Bbx6EMZfP"]
                    },
                    "check": {
                        "endpoint": "/intents/status/v3?requestId=0xbridge1",
                        "method": "GET"
                    }
                }]
            }],
            "details": { "currencyOut": { "amount": "1136561" } }
        });
        let q = parse_quote_response(&v, Coin::Sol, Coin::Usdt).unwrap();
        assert!(q.is_bridge);
        assert_eq!(q.tx_hint, "solanaRelay");
        assert!(q.solana_tx_data().is_some());
        assert!(q.evm_tx().is_none());
        assert_eq!(
            q.check_url.as_deref(),
            Some("https://api.relay.link/intents/status/v3?requestId=0xbridge1")
        );
        let payload = q.to_swap_payload();
        assert!(payload.get("solanaTx").is_some());
        assert_eq!(payload["isBridge"], true);
    }

    #[test]
    fn parses_relay_fee_breakdown() {
        let v = json!({
            "requestId": "0xfees",
            "steps": [{
                "id": "swap",
                "action": "swap",
                "description": "swap",
                "kind": "transaction",
                "items": [{
                    "status": "incomplete",
                    "data": {
                        "to": "0xabc",
                        "data": "0xdead",
                        "value": "0"
                    }
                }]
            }],
            "fees": {
                "gas": {
                    "currency": { "symbol": "POL" },
                    "amount": "100",
                    "amountFormatted": "0.0376",
                    "amountUsd": "0.004"
                },
                "relayer": {
                    "currency": { "symbol": "USDC" },
                    "amount": "20366",
                    "amountFormatted": "0.020366",
                    "amountUsd": "0.020"
                },
                "relayerGas": {
                    "currency": { "symbol": "USDC" },
                    "amount": "220",
                    "amountFormatted": "0.00022",
                    "amountUsd": "0.0002"
                },
                "relayerService": {
                    "currency": { "symbol": "USDC" },
                    "amount": "20146",
                    "amountFormatted": "0.020146",
                    "amountUsd": "0.020"
                },
                "app": {
                    "currency": { "symbol": "USDC" },
                    "amount": "0",
                    "amountFormatted": "0.0"
                }
            },
            "details": { "currencyOut": { "amount": "995000" } }
        });
        let q = parse_quote_response(&v, Coin::Usdc, Coin::Usdc).unwrap_or_else(|_| {
            // same-coin unsupported — use Usdt→Usdc
            parse_quote_response(&v, Coin::Usdt, Coin::Usdc).unwrap()
        });
        let types: Vec<_> = q.network_fees.iter().map(|f| f.fee_type.as_str()).collect();
        assert!(types.contains(&"relayerService"));
        assert!(types.contains(&"relayerGas"));
        assert!(types.contains(&"gas"));
        // Aggregate relayer skipped when parts present
        assert!(!types.contains(&"relayer"));
        assert!(!types.contains(&"app"));
        let svc = q.network_fees.iter().find(|f| f.fee_type == "relayerService").unwrap();
        assert_eq!(svc.amount, "0.020146");
        assert_eq!(svc.asset, "USDC");
    }

    #[test]
    fn supports_bridge_and_polygon() {
        assert!(RelayClient::supports_pair(Coin::Usdt, Coin::Usdc));
        assert!(RelayClient::supports_pair(Coin::Sol, Coin::Usdt));
        assert!(RelayClient::supports_pair(Coin::Pol, Coin::Sol));
        assert!(RelayClient::supports_pair(Coin::Pepe, Coin::Usdt));
        assert!(RelayClient::supports_pair(Coin::Usdc, Coin::Pepe));
        assert!(RelayClient::supports_pair(Coin::Pepe, Coin::Sol));
        assert!(RelayClient::supports_pair(Coin::Sol, Coin::Pepe));
        assert!(!RelayClient::supports_pair(Coin::Btc, Coin::Usdt));
        assert!(!RelayClient::supports_pair(Coin::Sol, Coin::Sol));
        assert!(!RelayClient::supports_pair(Coin::Pepe, Coin::Btc));
        let pepe = relay_asset(Coin::Pepe).unwrap();
        assert_eq!(pepe.chain_id, BSC_CHAIN_ID);
        assert!(pepe.currency.eq_ignore_ascii_case(BSC_PEPE));
    }

    #[test]
    fn intent_status_helpers() {
        let s = RelayIntentStatus::from_value(&json!({"status":"success","txHashes":["a","b"]}));
        assert!(s.is_success());
        assert_eq!(s.outbound_tx.as_deref(), Some("b"));
        let f = RelayIntentStatus::from_value(&json!({"status":"failed"}));
        assert!(f.is_failed());
    }

    #[test]
    fn upstream_code_extracts_relay_error_code() {
        // Real body shape returned for a dust SOL → PEPE bridge.
        let e = RelayError::Http {
            status: 400,
            body: r#"{"message":"Swap output amount is too small to cover fees required to execute swap","errorCode":"AMOUNT_TOO_LOW","requestId":"0x179009"}"#
                .into(),
        };
        assert_eq!(e.upstream_code().as_deref(), Some("AMOUNT_TOO_LOW"));
    }

    #[test]
    fn upstream_code_is_none_for_non_http_and_unparseable() {
        assert_eq!(RelayError::NotConfigured.upstream_code(), None);
        assert_eq!(RelayError::Msg("boom".into()).upstream_code(), None);
        let garbage = RelayError::Http { status: 502, body: "<html>bad gateway</html>".into() };
        assert_eq!(garbage.upstream_code(), None);
        let no_code = RelayError::Http { status: 400, body: r#"{"message":"nope"}"#.into() };
        assert_eq!(no_code.upstream_code(), None);
    }
}
