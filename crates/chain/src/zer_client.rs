//! Zero (ZER) — public Insight/explorer first, local `zerod` for signing.
//! Transparent `t1` only. Broadcast prefers public Insight `tx/send` until the
//! private node is fully synced (no public JSON-RPC exists for ZER).

use crate::btc_sign::Utxo;
use crate::encoding::zcash_t1_validate;
use crate::params::{params_for, AddressKind, ChainNetwork};
use crate::types::{ChainError, OnchainTx};
use crate::utxo_node_rpc::UtxoNodeRpc;
use serde_json::{json, Value};
use shared::Coin;

/// Public Insight bases that accept POST `/tx/send` with `{ "rawtx": "..." }`.
const PUBLIC_BROADCAST_BASES: &[&str] = &["https://insight.zerocurrency.io/insight-api-zero"];

pub struct ZerClient {
    http: reqwest::Client,
    explorer_base: String,
    explorer_key: Option<String>,
    rpc: Option<UtxoNodeRpc>,
}

impl ZerClient {
    pub fn with_rpc(
        explorer_base: &str,
        explorer_key: Option<&str>,
        rpc_url: Option<&str>,
    ) -> Self {
        let rpc = rpc_url.and_then(|raw| {
            let raw = raw.trim();
            if raw.is_empty() {
                return None;
            }
            if !trusted_signer_rpc(raw) {
                tracing::warn!("ZER_RPC_URL must name a local/private signer; public RPC rejected");
                return None;
            }
            match UtxoNodeRpc::new(raw) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!(error = %e.message, "ZER_RPC_URL invalid; explorer fallback only");
                    None
                }
            }
        });
        Self {
            http: reqwest::Client::new(),
            explorer_base: explorer_base.trim_end_matches('/').to_string(),
            explorer_key: explorer_key
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            rpc,
        }
    }

    pub fn rpc_ready(&self) -> bool {
        self.rpc.is_some()
    }

    pub fn validate_t1(address: &str, network: ChainNetwork) -> bool {
        match params_for(Coin::Zer, network).address_kind {
            AddressKind::ZcashTransparent { version } => zcash_t1_validate(address, version),
            _ => false,
        }
    }

    pub fn validate_address_kind(&self, address: &str, network: ChainNetwork) -> bool {
        Self::validate_t1(address, network)
    }

    fn explorer_url(&self, path: &str) -> String {
        match &self.explorer_key {
            Some(key) => format!("{}/{path}/{key}", self.explorer_base),
            None => format!("{}/{path}", self.explorer_base),
        }
    }

    async fn explorer_json(&self, path: &str) -> Result<Value, ChainError> {
        let url = self.explorer_url(path);
        let resp = self.http.get(&url).send().await.map_err(|_| ChainError {
            message: "ZER explorer request failed".into(),
        })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("ZER explorer HTTP {}", resp.status()),
            });
        }
        resp.json().await.map_err(|_| ChainError {
            message: "ZER explorer invalid JSON".into(),
        })
    }

    pub async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        if let Some(rpc) = &self.rpc {
            match rpc.scan_address(address).await {
                Ok(txs) => return Ok(txs),
                Err(e) => {
                    tracing::warn!(error = %e.message, address, "ZER scantxoutset failed; trying explorer")
                }
            }
        }
        self.explorer_txs(address).await
    }

    async fn explorer_txs(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let v = self.explorer_json(&format!("txs/{address}/0")).await?;
        let rows = v
            .as_array()
            .cloned()
            .or_else(|| v.get("txs").and_then(Value::as_array).cloned())
            .or_else(|| v.get("data").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        Ok(rows
            .into_iter()
            .filter_map(|tx| {
                let tx_hash = tx
                    .get("txid")
                    .or_else(|| tx.get("hash"))
                    .and_then(Value::as_str)?
                    .to_string();
                let confirmations =
                    tx.get("confirmations").and_then(Value::as_u64).unwrap_or(0) as u32;
                let amount = tx
                    .get("amount")
                    .and_then(value_to_sats)
                    .or_else(|| tx.get("value").and_then(value_to_sats))
                    .unwrap_or(0);
                Some(OnchainTx {
                    tx_hash,
                    vout: tx.get("vout").and_then(Value::as_u64).unwrap_or(0) as u32,
                    amount,
                    confirmations,
                    address: address.to_string(),
                })
            })
            .collect())
    }

    pub async fn get_balance(&self, address: &str) -> Result<u128, ChainError> {
        if let Some(rpc) = &self.rpc {
            match rpc.get_balance(address).await {
                Ok(sats) => return Ok(sats),
                Err(e) => {
                    tracing::warn!(error = %e.message, address, "ZER node balance failed; trying explorer")
                }
            }
        }
        let v = self
            .explorer_json(&format!("addressinfo/{address}"))
            .await?;
        if let Some(sats) = v.get("balanceSat").and_then(value_to_sats) {
            return Ok(sats);
        }
        if let Some(sats) = v.get("balance").and_then(value_to_sats) {
            return Ok(sats);
        }
        Ok(self
            .fetch_deposits(address)
            .await?
            .into_iter()
            .map(|d| d.amount)
            .sum())
    }

    pub async fn fetch_utxos(&self, address: &str) -> Result<Vec<Utxo>, ChainError> {
        if let Some(rpc) = &self.rpc {
            match rpc.fetch_utxos(address).await {
                Ok(u) if !u.is_empty() => return Ok(u),
                Ok(_) => tracing::warn!(address, "ZER node returned no UTXOs; trying explorer"),
                Err(e) => {
                    tracing::warn!(error = %e.message, address, "ZER node UTXOs failed; trying explorer")
                }
            }
        }
        self.explorer_utxos(address).await
    }

    /// Build spendable UTXOs from zerochain.info `txs/{addr}/0` (Insight-style).
    /// Used when zerod has no `scantxoutset` and Insight addressindex is off.
    async fn explorer_utxos(&self, address: &str) -> Result<Vec<Utxo>, ChainError> {
        let v = self.explorer_json(&format!("txs/{address}/0")).await?;
        let rows = v
            .as_array()
            .cloned()
            .or_else(|| v.get("txs").and_then(Value::as_array).cloned())
            .or_else(|| v.get("data").and_then(Value::as_array).cloned())
            .unwrap_or_default();

        let mut spent = std::collections::HashSet::<(String, u32)>::new();
        let mut candidates = Vec::new();
        for tx in &rows {
            let txid = tx
                .get("txid")
                .or_else(|| tx.get("hash"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let Some(vins) = tx.get("vin").and_then(Value::as_array) {
                for vin in vins {
                    let Some(prev) = vin.get("txid").and_then(Value::as_str) else {
                        continue;
                    };
                    let vout = vin.get("vout").and_then(Value::as_u64).unwrap_or(0) as u32;
                    spent.insert((prev.to_string(), vout));
                }
            }
            let confs = tx.get("confirmations").and_then(Value::as_u64).unwrap_or(0);
            if confs == 0 {
                continue;
            }
            let Some(vouts) = tx.get("vout").and_then(Value::as_array) else {
                continue;
            };
            for (vout, out) in vouts.iter().enumerate() {
                let spk = out.get("scriptPubKey").cloned().unwrap_or(Value::Null);
                let owned = spk
                    .get("addresses")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().any(|x| x.as_str() == Some(address)))
                    .unwrap_or(false)
                    || spk.get("address").and_then(Value::as_str) == Some(address);
                if !owned {
                    continue;
                }
                let value_sats = out
                    .get("valueSat")
                    .and_then(value_to_sats)
                    .or_else(|| out.get("value").and_then(value_to_sats))
                    .unwrap_or(0);
                let script_pubkey_hex = spk
                    .get("hex")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if value_sats == 0 || txid.is_empty() {
                    continue;
                }
                candidates.push(Utxo {
                    txid: txid.to_string(),
                    vout: vout as u32,
                    value: value_sats as u64,
                    script_pubkey_hex,
                });
            }
        }
        let utxos: Vec<Utxo> = candidates
            .into_iter()
            .filter(|u| !spent.contains(&(u.txid.clone(), u.vout)))
            .collect();
        if utxos.is_empty() {
            return Err(ChainError {
                message: format!("ZER explorer found no spendable UTXOs for {address}"),
            });
        }
        Ok(utxos)
    }

    /// Network tip from public Insight — used for `expiryheight` while zerod lags.
    async fn public_tip_height(&self) -> Result<u64, ChainError> {
        for base in PUBLIC_BROADCAST_BASES {
            let url = format!("{base}/status?q=getInfo");
            match self.http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let v: Value = resp.json().await.map_err(|e| ChainError {
                        message: e.to_string(),
                    })?;
                    if let Some(h) = v.pointer("/info/blocks").and_then(Value::as_u64) {
                        return Ok(h);
                    }
                    if let Some(h) = v.get("blocks").and_then(Value::as_u64) {
                        return Ok(h);
                    }
                }
                Ok(resp) => {
                    tracing::warn!(status = %resp.status(), url, "ZER Insight tip HTTP error")
                }
                Err(e) => tracing::warn!(error = %e, url, "ZER Insight tip request failed"),
            }
        }
        Err(ChainError {
            message: "ZER public Insight tip unavailable (need expiryheight)".into(),
        })
    }

    /// Prefer Insight `/addr/{}/utxo` (clean scriptPubKey) over zerochain tx reconstruction.
    async fn insight_utxos(&self, address: &str) -> Result<Vec<Utxo>, ChainError> {
        let mut last = String::new();
        for base in PUBLIC_BROADCAST_BASES {
            let url = format!("{base}/addr/{address}/utxo");
            match self.http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let rows: Vec<Value> = resp.json().await.map_err(|e| ChainError {
                        message: e.to_string(),
                    })?;
                    let mut utxos = Vec::new();
                    for u in rows {
                        let txid = u
                            .get("txid")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let vout = u.get("vout").and_then(Value::as_u64).unwrap_or(0) as u32;
                        let value = u
                            .get("satoshis")
                            .and_then(value_to_sats)
                            .or_else(|| u.get("amount").and_then(value_to_sats))
                            .unwrap_or(0) as u64;
                        let script_pubkey_hex = u
                            .get("scriptPubKey")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        if txid.is_empty() || value == 0 || script_pubkey_hex.is_empty() {
                            continue;
                        }
                        utxos.push(Utxo {
                            txid,
                            vout,
                            value,
                            script_pubkey_hex,
                        });
                    }
                    if !utxos.is_empty() {
                        return Ok(utxos);
                    }
                    last = format!("{url} empty");
                }
                Ok(resp) => last = format!("{url} HTTP {}", resp.status()),
                Err(e) => last = e.to_string(),
            }
        }
        Err(ChainError {
            message: format!("ZER Insight UTXOs failed: {last}"),
        })
    }

    pub async fn broadcast_signed(
        &self,
        wif: &str,
        to_address: &str,
        amount_sats: u64,
        change_address: &str,
    ) -> Result<(String, u64), ChainError> {
        let rpc = self.rpc.as_ref().ok_or_else(|| ChainError {
            message:
                "ZER_RPC_URL required for withdraw (zerod signs; broadcast uses public Insight)"
                    .into(),
        })?;
        // Prefer public Insight UTXOs while private node lags tip.
        let utxos = match self.insight_utxos(change_address).await {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e.message, "ZER Insight UTXOs failed; trying explorer/node");
                match self.explorer_utxos(change_address).await {
                    Ok(u) => u,
                    Err(e2) => {
                        tracing::warn!(error = %e2.message, "ZER explorer UTXOs failed; trying node");
                        self.fetch_utxos(change_address).await?
                    }
                }
            }
        };
        let total: u64 = utxos.iter().map(|u| u.value).sum();
        // Transparent ZER relays typically need a small fee; keep dust for the miner.
        let fee: u64 = 10_000; // 0.0001 ZER
        if total < amount_sats.saturating_add(fee) {
            return Err(ChainError {
                message: format!(
                    "ZER insufficient hot funds: have {total} need {}",
                    amount_sats.saturating_add(fee)
                ),
            });
        }
        let change = total - amount_sats - fee;
        let inputs: Vec<(String, u32)> = utxos.iter().map(|u| (u.txid.clone(), u.vout)).collect();
        let mut outputs = vec![(to_address.to_string(), sats_to_coin_str(amount_sats))];
        if change > 0 {
            outputs.push((change_address.to_string(), sats_to_coin_str(change)));
        }
        // zerod sets expiry from its lagging tip → Insight rejects tx-expiring-soon.
        // Force expiry to public tip + 40 blocks (~network requirement).
        let tip = self.public_tip_height().await?;
        let expiry = tip.saturating_add(40);
        tracing::info!(
            tip,
            expiry,
            "ZER createrawtransaction with public expiryheight"
        );
        let raw = rpc
            .create_raw_transaction_ex(&inputs, &outputs, Some(expiry))
            .await?;
        // Pass prevouts so signing works even when zerod UTXO set is behind tip.
        let signed = match rpc.sign_raw_transaction_legacy(&raw, wif, &utxos).await {
            Ok(hex) => hex,
            Err(e) => {
                tracing::warn!(error = %e.message, "signrawtransaction+prevouts failed; trying signrawtransactionwithkey");
                rpc.sign_raw_transaction_with_key(&raw, wif).await?
            }
        };
        let txid = self.broadcast_signed_hex(&signed).await?;
        Ok((txid, fee))
    }

    /// Public Insight first (synced tip), then local zerod. No public JSON-RPC for ZER.
    async fn broadcast_signed_hex(&self, raw_hex: &str) -> Result<String, ChainError> {
        let mut last = String::new();
        for base in PUBLIC_BROADCAST_BASES {
            let url = format!("{base}/tx/send");
            match self
                .http
                .post(&url)
                .json(&json!({ "rawtx": raw_hex }))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let v: Value = resp.json().await.map_err(|e| ChainError {
                        message: e.to_string(),
                    })?;
                    if let Some(txid) = v.get("txid").and_then(Value::as_str) {
                        tracing::info!(txid, url, "ZER broadcasted via public Insight");
                        return Ok(txid.to_string());
                    }
                    if let Some(txid) = v.as_str() {
                        tracing::info!(txid, url, "ZER broadcasted via public Insight");
                        return Ok(txid.to_string());
                    }
                    last = format!("{url} ok but no txid: {v}");
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    last = format!("{url} HTTP {status} body={body}");
                    // Do not fall through to lagging local node for these — keeps the real reason.
                    if body.contains("Missing inputs")
                        || body.contains("already in block chain")
                        || body.contains("txn-mempool-conflict")
                        || body.contains("tx-expiring-soon")
                        || body.contains("expiryheight")
                    {
                        return Err(ChainError { message: last });
                    }
                }
                Err(e) => last = e.to_string(),
            }
        }
        if let Some(rpc) = &self.rpc {
            match rpc.broadcast(raw_hex).await {
                Ok(txid) => {
                    tracing::info!(txid, "ZER broadcasted via local zerod");
                    return Ok(txid);
                }
                Err(e) => {
                    tracing::warn!(error = %e.message, insight_err = %last, "ZER local sendrawtransaction failed after public Insight");
                    if last.is_empty() {
                        last = e.message;
                    } else {
                        last = format!("{last}; local={}", e.message);
                    }
                }
            }
        }
        Err(ChainError {
            message: format!("ZER broadcast failed (public Insight + local): {last}"),
        })
    }
}

/// ZER's signing RPC receives a WIF. It must never point at a public API.
fn trusted_signer_rpc(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "http" && url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if host == "localhost" {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v) => v.is_private() || v.is_loopback(),
            std::net::IpAddr::V6(v) => v.is_loopback() || v.is_unique_local(),
        };
    }
    // Docker service names contain no dot; public DNS names do.
    !host.contains('.') && host.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

fn value_to_sats(v: &Value) -> Option<u128> {
    match v {
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                // Heuristic: integers >= 1e6 are already sats; smaller floats are coins.
                if n.as_f64().is_some_and(|f| f.fract() == 0.0) && u >= 1_000_000 {
                    return Some(u as u128);
                }
            }
            n.as_f64()
                .map(|f| (f * 100_000_000.0).round().max(0.0) as u128)
        }
        Value::String(s) => {
            if s.contains('.') {
                s.parse::<f64>()
                    .ok()
                    .map(|f| (f * 100_000_000.0).round().max(0.0) as u128)
            } else {
                s.parse().ok()
            }
        }
        _ => None,
    }
}

fn sats_to_coin_str(sats: u64) -> String {
    format!("{}.{:08}", sats / 100_000_000, sats % 100_000_000)
}

#[cfg(test)]
mod signer_rpc_tests {
    use super::*;

    #[test]
    fn private_signer_addresses_only() {
        assert!(trusted_signer_rpc("http://127.0.0.1:23832"));
        assert!(trusted_signer_rpc("http://10.0.0.3:23832"));
        assert!(trusted_signer_rpc("http://zerod:23832"));
        assert!(!trusted_signer_rpc("https://public-rpc.example:443"));
        assert!(!trusted_signer_rpc("https://8.8.8.8:443"));
    }

    #[tokio::test]
    async fn explorer_error_does_not_expose_key_in_url() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let n = socket.read(&mut request).await.unwrap();
            assert!(String::from_utf8_lossy(&request[..n]).contains("secret-test-key"));
            socket
                .write_all(b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        let client = ZerClient::with_rpc(&format!("http://{addr}"), Some("secret-test-key"), None);
        let err = client.explorer_json("addressinfo/test").await.unwrap_err();
        assert!(err.message.contains("429"));
        assert!(!err.message.contains("secret-test-key"));
        assert!(!err.message.contains(&addr.to_string()));
        server.await.unwrap();
    }
}
