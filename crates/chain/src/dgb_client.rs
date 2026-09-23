//! DigiByte client — self-hosted JSON-RPC (`scantxoutset`) first, Insight/Blockbook fallback.
//!
//! Every indexer call has a timeout and every operation the sweep needs
//! (UTXOs, fee, broadcast) falls back to Blockbook: when the Insight hosts
//! went down, UTXO listing and broadcast were Insight-only, so deposits sat
//! on their addresses unswept.

use crate::btc_sign::Utxo;
use crate::types::{ChainError, OnchainTx};
use crate::utxo_node_rpc::UtxoNodeRpc;
use serde::Deserialize;
use serde_json::json;

/// Blockbook explorer used when the Insight hosts are down.
pub const BLOCKBOOK_FALLBACK: &str = "https://digibyte.atomicwallet.io/api";

pub struct DgbClient {
    http: reqwest::Client,
    bases: Vec<String>,
    rpc: Option<UtxoNodeRpc>,
}

impl DgbClient {
    pub fn new(primary: &str) -> Self {
        Self::with_rpc(primary, None)
    }

    pub fn with_rpc(primary: &str, rpc_url: Option<&str>) -> Self {
        let mut bases = vec![primary.trim_end_matches('/').to_string()];
        for extra in [
            "https://digiexplorer.info/api",
            "https://explorer.digibyte.host/api",
            // Blockbook (`/v2/...`), independent operator — reserve for the Insight hosts.
            BLOCKBOOK_FALLBACK,
        ] {
            if !bases.iter().any(|b| b == extra) {
                bases.push(extra.to_string());
            }
        }
        let rpc = rpc_url.and_then(|raw| {
            let raw = raw.trim();
            if raw.is_empty() {
                return None;
            }
            match UtxoNodeRpc::new(raw) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!(error = %e.message, "DGB_RPC_URL invalid; Insight fallback only");
                    None
                }
            }
        });
        let http = reqwest::Client::builder()
            .user_agent("SatsPay-Dgb/1.0")
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { http, bases, rpc }
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
        if let Some(rpc) = &self.rpc {
            match rpc.scan_address(address).await {
                Ok(txs) => return Ok(txs),
                Err(e) => tracing::warn!(error = %e.message, address, "DGB scantxoutset failed; trying Insight"),
            }
        }
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
        if let Some(rpc) = &self.rpc {
            match rpc.get_balance(address).await {
                Ok(sats) => return Ok(sats),
                Err(e) => tracing::warn!(error = %e.message, address, "DGB node balance failed; trying Insight"),
            }
        }
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
        if let Some(rpc) = &self.rpc {
            match rpc.fetch_utxos(address).await {
                Ok(utxos) => return Ok(utxos),
                Err(e) => tracing::warn!(error = %e.message, address, "DGB node UTXOs failed; trying Insight"),
            }
        }
        match self.get_json::<Vec<InsightUtxo>>(&format!("/addr/{address}/utxo")).await {
            Ok(raw) => Ok(raw
                .into_iter()
                .map(|u| Utxo {
                    txid: u.txid,
                    vout: u.vout,
                    value: u.satoshis.unwrap_or_else(|| ((u.amount.unwrap_or(0.0)) * 100_000_000.0).round() as u128) as u64,
                    script_pubkey_hex: u.script_pub_key.unwrap_or_default(),
                })
                .collect()),
            Err(insight_err) => {
                tracing::warn!(error = %insight_err.message, address, "DGB Insight UTXOs failed; trying Blockbook");
                // Blockbook has no scriptPubKey per UTXO: the caller derives it
                // from the address (see `fill_missing_scripts`).
                let book: BlockbookUtxos = self.get_json(&format!("/v2/utxo/{address}?confirmed=true")).await?;
                blockbook_to_utxos(book)
            }
        }
    }

    pub async fn estimate_fee_sat_per_byte(&self) -> Result<u64, ChainError> {
        if let Some(rpc) = &self.rpc {
            if let Ok(fee) = rpc.estimate_fee_sat_per_byte().await {
                return Ok(fee.max(1));
            }
        }
        let fee_per_kb: f64 = self.get_json("/utils/estimatefee?nbBlocks=2").await.unwrap_or(-1.0);
        if fee_per_kb > 0.0 {
            return Ok(((fee_per_kb * 100_000_000.0) / 1000.0).max(1.0) as u64);
        }
        // Blockbook: {"result":"0.0001"} (DGB per kB).
        if let Ok(v) = self.get_json::<serde_json::Value>("/v2/estimatefee/2").await {
            if let Some(per_kb) = v.get("result").and_then(|r| r.as_str()).and_then(|s| s.parse::<f64>().ok()) {
                if per_kb > 0.0 {
                    return Ok(((per_kb * 100_000_000.0) / 1000.0).max(1.0) as u64);
                }
            }
        }
        // DigiByte floors at 1 sat/vB; keep withdrawals unblocked if indexers are down.
        Ok(1)
    }

    pub async fn broadcast(&self, raw_hex: &str) -> Result<String, ChainError> {
        if let Some(rpc) = &self.rpc {
            match rpc.broadcast(raw_hex).await {
                Ok(txid) => return Ok(txid),
                Err(e) => tracing::warn!(error = %e.message, "DGB sendrawtransaction failed; trying Insight"),
            }
        }
        let mut errors: Vec<String> = Vec::new();
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
                    errors.push(format!("{url}: {v}"));
                }
                Ok(resp) => errors.push(format!("{url} HTTP {}", resp.status())),
                Err(e) => errors.push(e.to_string()),
            }
            // Blockbook: POST /v2/sendtx/ with the raw hex as body → {"result": txid}.
            let bb = format!("{base}/v2/sendtx/");
            match self.http.post(&bb).body(raw_hex.to_string()).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let v: serde_json::Value = resp.json().await.map_err(|e| ChainError { message: e.to_string() })?;
                    if let Some(txid) = v.get("result").and_then(|t| t.as_str()) {
                        return Ok(txid.to_string());
                    }
                    errors.push(format!("{bb}: {v}"));
                }
                Ok(resp) => errors.push(format!("{bb} HTTP {}", resp.status())),
                Err(e) => errors.push(e.to_string()),
            }
        }
        Err(ChainError { message: format!("DGB broadcast failed: {}", errors.join("; ")) })
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

fn blockbook_to_utxos(book: BlockbookUtxos) -> Result<Vec<Utxo>, ChainError> {
    book.into_iter()
        .map(|u| {
            let value = u.value.parse::<u64>().map_err(|_| ChainError {
                message: format!("DGB Blockbook bad UTXO value {:?} for {}:{}", u.value, u.txid, u.vout),
            })?;
            Ok(Utxo { txid: u.txid, vout: u.vout, value, script_pubkey_hex: String::new() })
        })
        .collect()
}

/// Indexers without a per-UTXO script (Blockbook) leave `script_pubkey_hex`
/// empty; every UTXO of a single-key address pays to that address's script.
pub fn fill_missing_scripts(utxos: &mut [Utxo], address_script_hex: &str) {
    for u in utxos.iter_mut().filter(|u| u.script_pubkey_hex.is_empty()) {
        u.script_pubkey_hex = address_script_hex.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blockbook_utxos_parse_and_reject_bad_values() {
        let book: BlockbookUtxos = serde_json::from_str(
            r#"[{"txid":"aa","vout":1,"value":"24696746902","height":5,"confirmations":21,"coinbase":true}]"#,
        )
        .unwrap();
        let utxos = blockbook_to_utxos(book).unwrap();
        assert_eq!(utxos[0].value, 24_696_746_902);
        assert_eq!(utxos[0].vout, 1);
        assert!(utxos[0].script_pubkey_hex.is_empty());

        let bad: BlockbookUtxos = serde_json::from_str(r#"[{"txid":"bb","vout":0,"value":"1.5"}]"#).unwrap();
        assert!(blockbook_to_utxos(bad).is_err());
    }

    #[test]
    fn fill_missing_scripts_only_touches_empty_ones() {
        let mut utxos = vec![
            Utxo { txid: "a".into(), vout: 0, value: 1, script_pubkey_hex: String::new() },
            Utxo { txid: "b".into(), vout: 0, value: 1, script_pubkey_hex: "0014ff".into() },
        ];
        fill_missing_scripts(&mut utxos, "0014aa");
        assert_eq!(utxos[0].script_pubkey_hex, "0014aa");
        assert_eq!(utxos[1].script_pubkey_hex, "0014ff");
    }

    #[test]
    fn blockbook_is_always_in_the_fallback_list() {
        let c = DgbClient::new("https://my-insight.example/api/");
        assert_eq!(c.bases[0], "https://my-insight.example/api");
        assert!(c.bases.iter().any(|b| b == BLOCKBOOK_FALLBACK));
    }
}
