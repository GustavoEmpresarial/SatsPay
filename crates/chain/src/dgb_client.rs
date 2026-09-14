//! DigiByte Insight / Blockbook client — live UTXOs, fee estimate, broadcast.

use crate::btc_sign::Utxo;
use crate::types::{ChainError, OnchainTx};
use serde::Deserialize;
use serde_json::json;

pub struct DgbClient {
    http: reqwest::Client,
    bases: Vec<String>,
}

impl DgbClient {
    pub fn new(primary: &str) -> Self {
        let mut bases = vec![primary.trim_end_matches('/').to_string()];
        for extra in [
            "https://digiexplorer.info/api",
            "https://explorer.digibyte.host/api",
        ] {
            if !bases.iter().any(|b| b == extra) {
                bases.push(extra.to_string());
            }
        }
        Self { http: reqwest::Client::new(), bases }
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, suffix: &str) -> Result<T, ChainError> {
        let mut last = String::new();
        for base in &self.bases {
            let url = format!("{base}{suffix}");
            match self.http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    return resp.json().await.map_err(|e| ChainError { message: e.to_string() });
                }
                Ok(resp) => last = format!("{url} HTTP {}", resp.status()),
                Err(e) => last = e.to_string(),
            }
        }
        Err(ChainError { message: format!("DGB insight failed: {last}") })
    }

    pub async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        if let Ok(utxos) = self.get_json::<Vec<InsightUtxo>>(&format!("/addr/{address}/utxo")).await {
            return Ok(utxos
                .into_iter()
                .map(|u| OnchainTx {
                    tx_hash: u.txid,
                    vout: u.vout,
                    amount: u.satoshis.unwrap_or_else(|| ((u.amount.unwrap_or(0.0)) * 100_000_000.0).round() as u128),
                    confirmations: u.confirmations.unwrap_or(0),
                    address: address.to_string(),
                })
                .collect());
        }
        let book: BlockbookUtxos = self.get_json(&format!("/v2/utxo/{address}")).await?;
        Ok(book
            .into_iter()
            .map(|u| OnchainTx {
                tx_hash: u.txid,
                vout: u.vout,
                amount: u.value.parse().unwrap_or(0),
                confirmations: u.confirmations.unwrap_or(0),
                address: address.to_string(),
            })
            .collect())
    }

    pub async fn get_balance(&self, address: &str) -> Result<u128, ChainError> {
        // Insight-style `/addr/{address}`
        if let Ok(v) = self.get_json::<serde_json::Value>(&format!("/addr/{address}")).await {
            if let Some(s) = v.get("balanceSat").and_then(|x| x.as_u64()).map(|u| u as u128) {
                return Ok(s);
            }
            if let Some(s) = v.get("balanceSat").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()) {
                return Ok(s);
            }
            if let Some(b) = v.get("balance").and_then(|x| x.as_f64()) {
                return Ok((b * 100_000_000.0).round() as u128);
            }
            if let Some(b) = v.get("balance").and_then(|x| x.as_str()).and_then(|s| s.parse::<f64>().ok()) {
                return Ok((b * 100_000_000.0).round() as u128);
            }
        }
        // Blockbook `/v2/address/{address}` — balance string in sats
        if let Ok(v) = self.get_json::<serde_json::Value>(&format!("/v2/address/{address}")).await {
            if let Some(s) = v.get("balance").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()) {
                return Ok(s);
            }
            if let Some(s) = v.get("balance").and_then(|x| x.as_u64()).map(|u| u as u128) {
                return Ok(s);
            }
        }
        // Cryptoid (legacy + bech32) — float DGB string, e.g. "0" / "12.34"
        if let Ok(sats) = self.cryptoid_balance(address).await {
            return Ok(sats);
        }
        // Last resort: sum UTXOs from insight/blockbook
        let deps = self.fetch_deposits(address).await?;
        Ok(deps.into_iter().map(|d| d.amount).sum())
    }

    async fn cryptoid_balance(&self, address: &str) -> Result<u128, ChainError> {
        let url = format!("https://chainz.cryptoid.info/dgb/api.dws?q=getbalance&a={address}");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ChainError { message: e.to_string() })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("cryptoid HTTP {}", resp.status()),
            });
        }
        let text = resp.text().await.map_err(|e| ChainError { message: e.to_string() })?;
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.starts_with('<') || trimmed.to_ascii_lowercase().contains("invalid") {
            return Err(ChainError {
                message: format!("cryptoid bad body: {trimmed}"),
            });
        }
        let dgb: f64 = trimmed.parse().map_err(|_| ChainError {
            message: format!("cryptoid parse: {trimmed}"),
        })?;
        Ok((dgb * 100_000_000.0).round().max(0.0) as u128)
    }

    pub async fn fetch_utxos(&self, address: &str) -> Result<Vec<Utxo>, ChainError> {
        let raw: Vec<InsightUtxo> = self.get_json(&format!("/addr/{address}/utxo")).await?;
        Ok(raw
            .into_iter()
            .map(|u| Utxo {
                txid: u.txid,
                vout: u.vout,
                value: u.satoshis.unwrap_or_else(|| ((u.amount.unwrap_or(0.0)) * 100_000_000.0).round() as u128) as u64,
                script_pubkey_hex: u.script_pub_key.unwrap_or_default(),
            })
            .collect())
    }

    pub async fn estimate_fee_sat_per_byte(&self) -> Result<u64, ChainError> {
        let fee_per_kb: f64 = self.get_json("/utils/estimatefee?nbBlocks=2").await.unwrap_or(-1.0);
        if fee_per_kb > 0.0 {
            return Ok(((fee_per_kb * 100_000_000.0) / 1000.0).max(1.0) as u64);
        }
        Err(ChainError { message: "DGB fee estimate unavailable".into() })
    }

    pub async fn broadcast(&self, raw_hex: &str) -> Result<String, ChainError> {
        let mut last = String::new();
        for base in &self.bases {
            let url = format!("{base}/tx/send");
            match self.http.post(&url).json(&json!({ "rawtx": raw_hex })).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let v: serde_json::Value = resp.json().await.map_err(|e| ChainError { message: e.to_string() })?;
                    if let Some(txid) = v.get("txid").and_then(|t| t.as_str()) {
                        return Ok(txid.to_string());
                    }
                    if let Some(txid) = v.as_str() {
                        return Ok(txid.to_string());
                    }
                    last = v.to_string();
                }
                Ok(resp) => last = format!("{url} HTTP {}", resp.status()),
                Err(e) => last = e.to_string(),
            }
        }
        Err(ChainError { message: format!("DGB broadcast failed: {last}") })
    }
}

#[derive(Debug, Deserialize)]
struct InsightUtxo {
    txid: String,
    vout: u32,
    #[serde(default)]
    satoshis: Option<u128>,
    #[serde(default)]
    amount: Option<f64>,
    #[serde(default)]
    confirmations: Option<u32>,
    #[serde(rename = "scriptPubKey", default)]
    script_pub_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlockbookUtxo {
    txid: String,
    vout: u32,
    value: String,
    #[serde(default)]
    confirmations: Option<u32>,
}

type BlockbookUtxos = Vec<BlockbookUtxo>;
