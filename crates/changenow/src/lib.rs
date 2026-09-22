//! ChangeNOW Partner API client (`https://api.changenow.io/v2`).
//!
//! Instant L1 / cross-chain swaps via deposit address:
//! estimate → create exchange → send to `payinAddress` → poll status → credit.

use serde_json::{json, Value};
use shared::{format_amount, Coin};
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.changenow.io/v2";

/// ChangeNOW ticker + network for a SatsPay coin (custodial hot wallet network).
#[derive(Debug, Clone, Copy)]
pub struct CnAsset {
    pub ticker: &'static str,
    pub network: &'static str,
}

/// Map SatsPay ledger coins onto ChangeNOW native networks we actually hold.
pub fn cn_asset(coin: Coin) -> Option<CnAsset> {
    match coin {
        Coin::Btc => Some(CnAsset {
            ticker: "btc",
            network: "btc",
        }),
        Coin::Ltc => Some(CnAsset {
            ticker: "ltc",
            network: "ltc",
        }),
        Coin::Doge => Some(CnAsset {
            ticker: "doge",
            network: "doge",
        }),
        Coin::Bch => Some(CnAsset {
            ticker: "bch",
            network: "bch",
        }),
        Coin::Dgb => Some(CnAsset {
            ticker: "dgb",
            network: "dgb",
        }),
        // Native POL on Polygon = ChangeNOW `matic` / `matic`.
        Coin::Pol => Some(CnAsset {
            ticker: "matic",
            network: "matic",
        }),
        Coin::Usdt => Some(CnAsset {
            ticker: "usdt",
            network: "matic",
        }),
        Coin::Usdc => Some(CnAsset {
            ticker: "usdc",
            network: "matic",
        }),
        Coin::Sol => Some(CnAsset {
            ticker: "sol",
            network: "sol",
        }),
        Coin::Zer | Coin::Pepe => None,
    }
}

pub fn supports_pair(from: Coin, to: Coin) -> bool {
    from != to && cn_asset(from).is_some() && cn_asset(to).is_some()
}

/// True when ChangeNOW is the right venue (involves an L1 UTXO leg).
pub fn prefers_changenow(from: Coin, to: Coin) -> bool {
    supports_pair(from, to) && (is_l1_utxo(from) || is_l1_utxo(to))
}

pub fn is_l1_utxo(coin: Coin) -> bool {
    matches!(
        coin,
        Coin::Btc | Coin::Ltc | Coin::Doge | Coin::Bch | Coin::Dgb
    )
}

#[derive(Debug, Error)]
pub enum ChangeNowError {
    #[error("changenow not configured (set CHANGENOW_ENABLED=true + CHANGENOW_API_KEY)")]
    NotConfigured,
    #[error("unsupported coin for changenow: {0:?}")]
    UnsupportedCoin(Coin),
    #[error("changenow http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("changenow: {0}")]
    Msg(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct ChangeNowClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    enabled: bool,
}

impl ChangeNowClient {
    pub fn from_env() -> Self {
        let enabled = std::env::var("CHANGENOW_ENABLED")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        let base_url =
            std::env::var("CHANGENOW_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let api_key = std::env::var("CHANGENOW_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            enabled,
        }
    }

    pub fn new_for_tests(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: Some(api_key.into()),
            enabled: true,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.enabled && self.api_key.as_ref().is_some_and(|k| !k.is_empty())
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn auth_header(&self) -> Result<&str, ChangeNowError> {
        self.api_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .ok_or(ChangeNowError::NotConfigured)
    }

    /// Standard-flow estimate (human amounts).
    pub async fn estimate(
        &self,
        from: Coin,
        to: Coin,
        from_amount_ledger: u128,
    ) -> Result<ChangeNowEstimate, ChangeNowError> {
        if !self.is_configured() {
            return Err(ChangeNowError::NotConfigured);
        }
        let from_a = cn_asset(from).ok_or(ChangeNowError::UnsupportedCoin(from))?;
        let to_a = cn_asset(to).ok_or(ChangeNowError::UnsupportedCoin(to))?;
        let from_human = format_amount(from_amount_ledger, from);

        let url = format!(
            "{}/exchange/estimated-amount?fromCurrency={}&toCurrency={}&fromAmount={}&fromNetwork={}&toNetwork={}&flow=standard",
            self.base_url,
            from_a.ticker,
            to_a.ticker,
            urlencoding_lite(&from_human),
            from_a.network,
            to_a.network,
        );
        let key = self.auth_header()?;
        let resp = self
            .http
            .get(&url)
            .header("x-changenow-api-key", key)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChangeNowError::Http {
                status: status.as_u16(),
                body: text.chars().take(800).collect(),
            });
        }
        let value: Value = serde_json::from_str(&text)?;
        parse_estimate(&value, from, to, from_amount_ledger)
    }

    /// Create exchange; `address` = our hot wallet for `to`, `refund_address` = hot for `from`.
    pub async fn create_exchange(
        &self,
        from: Coin,
        to: Coin,
        from_amount_ledger: u128,
        payout_address: &str,
        refund_address: &str,
    ) -> Result<ChangeNowExchange, ChangeNowError> {
        if !self.is_configured() {
            return Err(ChangeNowError::NotConfigured);
        }
        let from_a = cn_asset(from).ok_or(ChangeNowError::UnsupportedCoin(from))?;
        let to_a = cn_asset(to).ok_or(ChangeNowError::UnsupportedCoin(to))?;
        let from_human = format_amount(from_amount_ledger, from);
        let key = self.auth_header()?;

        let body = json!({
            "fromCurrency": from_a.ticker,
            "toCurrency": to_a.ticker,
            "fromNetwork": from_a.network,
            "toNetwork": to_a.network,
            "fromAmount": from_human,
            "address": payout_address,
            "refundAddress": refund_address,
            "flow": "standard",
            "type": "direct",
        });

        let resp = self
            .http
            .post(format!("{}/exchange", self.base_url))
            .header("x-changenow-api-key", key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChangeNowError::Http {
                status: status.as_u16(),
                body: text.chars().take(800).collect(),
            });
        }
        let value: Value = serde_json::from_str(&text)?;
        parse_exchange(&value, from, to)
    }

    pub async fn status(&self, exchange_id: &str) -> Result<ChangeNowStatus, ChangeNowError> {
        if !self.is_configured() {
            return Err(ChangeNowError::NotConfigured);
        }
        let key = self.auth_header()?;
        let url = format!("{}/exchange/by-id?id={}", self.base_url, urlencoding_lite(exchange_id));
        let resp = self
            .http
            .get(&url)
            .header("x-changenow-api-key", key)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChangeNowError::Http {
                status: status.as_u16(),
                body: text.chars().take(800).collect(),
            });
        }
        let value: Value = serde_json::from_str(&text)?;
        Ok(ChangeNowStatus::from_value(&value))
    }
}

#[derive(Debug, Clone)]
pub struct ChangeNowEstimate {
    pub from: Coin,
    pub to: Coin,
    pub from_amount_ledger: u128,
    pub to_amount_ledger: u128,
    pub to_amount_human: String,
    pub deposit_fee_human: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone)]
pub struct ChangeNowExchange {
    pub id: String,
    pub payin_address: String,
    pub payin_extra_id: Option<String>,
    pub from: Coin,
    pub to: Coin,
    pub from_amount_ledger: u128,
    pub to_amount_ledger: u128,
    pub to_amount_human: String,
    pub raw: Value,
}

impl ChangeNowExchange {
    pub fn route_id(&self) -> String {
        format!("changenow:{}", self.id)
    }

    pub fn to_swap_payload(&self) -> Value {
        json!({
            "provider": "CHANGENOW",
            "exchangeId": self.id,
            "payinAddress": self.payin_address,
            "payinExtraId": self.payin_extra_id,
            "fromAmount": self.from_amount_ledger.to_string(),
            "toAmount": self.to_amount_ledger.to_string(),
            "toAmountHuman": self.to_amount_human,
            "raw": self.raw,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ChangeNowStatus {
    pub status: String,
    pub to_amount_human: Option<String>,
    pub payout_hash: Option<String>,
    pub raw: Value,
}

impl ChangeNowStatus {
    pub fn from_value(value: &Value) -> Self {
        let status = value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let to_amount_human = value
            .get("amountTo")
            .or_else(|| value.get("toAmount"))
            .and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_f64().map(|f| format_float_trim(f)))
            });
        let payout_hash = value
            .get("payoutHash")
            .or_else(|| value.get("payinHash"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        Self {
            status,
            to_amount_human,
            payout_hash,
            raw: value.clone(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(
            self.status.to_ascii_lowercase().as_str(),
            "finished" | "completed" | "success"
        )
    }

    pub fn is_failed(&self) -> bool {
        matches!(
            self.status.to_ascii_lowercase().as_str(),
            "failed" | "refunded" | "expired"
        )
    }
}

fn parse_estimate(
    value: &Value,
    from: Coin,
    to: Coin,
    from_amount_ledger: u128,
) -> Result<ChangeNowEstimate, ChangeNowError> {
    let to_human = value
        .get("toAmount")
        .or_else(|| value.get("estimatedAmount"))
        .map(value_to_human)
        .transpose()?
        .ok_or_else(|| ChangeNowError::Msg("estimate missing toAmount".into()))?;
    let to_ledger = human_to_ledger(&to_human, to)
        .ok_or_else(|| ChangeNowError::Msg(format!("bad toAmount: {to_human}")))?;
    let deposit_fee_human = value
        .get("depositFee")
        .map(value_to_human)
        .transpose()
        .ok()
        .flatten();
    Ok(ChangeNowEstimate {
        from,
        to,
        from_amount_ledger,
        to_amount_ledger: to_ledger,
        to_amount_human: to_human,
        deposit_fee_human,
        raw: value.clone(),
    })
}

fn parse_exchange(value: &Value, from: Coin, to: Coin) -> Result<ChangeNowExchange, ChangeNowError> {
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ChangeNowError::Msg("create missing id".into()))?
        .to_string();
    let payin_address = value
        .get("payinAddress")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ChangeNowError::Msg("create missing payinAddress".into()))?
        .to_string();
    let payin_extra_id = value
        .get("payinExtraId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let from_human = value
        .get("fromAmount")
        .map(value_to_human)
        .transpose()?
        .unwrap_or_default();
    let from_ledger = if from_human.is_empty() {
        0
    } else {
        human_to_ledger(&from_human, from).unwrap_or(0)
    };
    let to_human = value
        .get("toAmount")
        .map(value_to_human)
        .transpose()?
        .unwrap_or_else(|| "0".into());
    let to_ledger = human_to_ledger(&to_human, to).unwrap_or(0);
    Ok(ChangeNowExchange {
        id,
        payin_address,
        payin_extra_id,
        from,
        to,
        from_amount_ledger: from_ledger,
        to_amount_ledger: to_ledger,
        to_amount_human: to_human,
        raw: value.clone(),
    })
}

fn value_to_human(v: &Value) -> Result<String, ChangeNowError> {
    if let Some(s) = v.as_str() {
        return Ok(s.trim().to_string());
    }
    if let Some(n) = v.as_f64() {
        return Ok(format_float_trim(n));
    }
    if let Some(n) = v.as_u64() {
        return Ok(n.to_string());
    }
    Err(ChangeNowError::Msg(format!("expected amount, got {v}")))
}

fn format_float_trim(n: f64) -> String {
    let s = format!("{n:.10}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Parse a human decimal into ledger units (always 8 decimals in SatsPay).
pub fn human_to_ledger(human: &str, _coin: Coin) -> Option<u128> {
    let s = human.trim().replace(',', "");
    if s.is_empty() {
        return None;
    }
    let neg = s.starts_with('-');
    if neg {
        return None;
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s.as_str(), ""),
    };
    if !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole_n: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let frac_padded = format!("{frac:0<8}");
    let frac_cut: String = frac_padded.chars().take(8).collect();
    let frac_n: u128 = if frac_cut.is_empty() {
        0
    } else {
        frac_cut.parse().ok()?
    };
    whole_n.checked_mul(100_000_000)?.checked_add(frac_n)
}

fn urlencoding_lite(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_map_l1_and_polygon() {
        assert_eq!(cn_asset(Coin::Btc).unwrap().ticker, "btc");
        assert_eq!(cn_asset(Coin::Pol).unwrap().ticker, "matic");
        assert_eq!(cn_asset(Coin::Usdt).unwrap().network, "matic");
        assert!(cn_asset(Coin::Pepe).is_none());
        assert!(prefers_changenow(Coin::Btc, Coin::Usdt));
        assert!(!prefers_changenow(Coin::Pol, Coin::Usdt));
        assert!(supports_pair(Coin::Btc, Coin::Ltc));
    }

    #[test]
    fn human_ledger_roundtrip() {
        assert_eq!(human_to_ledger("1.5", Coin::Btc), Some(150_000_000));
        assert_eq!(human_to_ledger("0.01", Coin::Btc), Some(1_000_000));
        assert_eq!(format_amount(150_000_000, Coin::Btc), "1.5");
    }

    #[test]
    fn status_flags() {
        let fin = ChangeNowStatus::from_value(&json!({"status": "finished", "amountTo": "1.2"}));
        assert!(fin.is_success());
        assert!(!fin.is_failed());
        let fail = ChangeNowStatus::from_value(&json!({"status": "failed"}));
        assert!(fail.is_failed());
    }

    #[tokio::test]
    async fn estimate_parses_mock() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex(".*/estimated-amount.*"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "fromCurrency": "btc",
                "toCurrency": "ltc",
                "fromAmount": 0.01,
                "toAmount": 2.5,
            })))
            .mount(&server)
            .await;

        let client = ChangeNowClient::new_for_tests(server.uri(), "test-key");
        let est = client
            .estimate(Coin::Btc, Coin::Ltc, 1_000_000)
            .await
            .unwrap();
        assert_eq!(est.to_amount_ledger, 250_000_000);
    }
}
