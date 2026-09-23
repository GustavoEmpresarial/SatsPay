//! Zero (ZER) — public Insight/ZeroChain for reads and broadcast, local `zerod` for signing.
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
        let history = self.explorer_history(address).await?;
        let mut deposits = Vec::new();
        for page in &history {
            deposits.extend(parse_zerochain_deposits(page, address)?);
        }
        Ok(deposits)
    }

    async fn explorer_history(&self, address: &str) -> Result<Vec<Value>, ChainError> {
        const MAX_PAGES: u64 = 100;
        let first = self.explorer_json(&format!("txs/{address}/0")).await?;
        let pages = first
            .get("pagesTotal")
            .and_then(Value::as_u64)
            .ok_or_else(|| ChainError {
                message: "ZER explorer tx history missing pagesTotal".into(),
            })?;
        if pages > MAX_PAGES {
            return Err(ChainError {
                message: "ZER explorer tx history exceeds safe page limit".into(),
            });
        }
        if first.get("txs").and_then(Value::as_array).is_none() {
            return Err(ChainError {
                message: "ZER explorer tx history missing txs".into(),
            });
        }
        let mut history = vec![first];
        for page in 1..pages {
            let body = self.explorer_json(&format!("txs/{address}/{page}")).await?;
            if body.get("txs").and_then(Value::as_array).is_none() {
                return Err(ChainError {
                    message: "ZER explorer tx history missing txs".into(),
                });
            }
            history.push(body);
        }
        Ok(history)
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
        if let Some(sats) = v.get("balanceSat").and_then(atomic_to_sats) {
            return Ok(sats);
        }
        if let Some(sats) = v.get("balance").and_then(coins_to_sats) {
            return Ok(sats);
        }
        Err(ChainError {
            message: "ZER explorer balance missing valid balanceSat or balance".into(),
        })
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

    /// Build spendable UTXOs from all zerochain.info history pages.
    /// Used when zerod has no `scantxoutset` and Insight addressindex is off.
    async fn explorer_utxos(&self, address: &str) -> Result<Vec<Utxo>, ChainError> {
        let history = self.explorer_history(address).await?;
        let rows: Vec<&Value> = history
            .iter()
            .flat_map(|page| {
                page.get("txs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .collect();

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
                    if let Some(vout) = vin
                        .get("vout")
                        .and_then(Value::as_u64)
                        .and_then(|v| u32::try_from(v).ok())
                    {
                        spent.insert((prev.to_string(), vout));
                    }
                }
            }
            let confs = tx.get("confirmations").and_then(Value::as_u64).unwrap_or(0);
            if confs == 0 {
                continue;
            }
            let Some(vouts) = tx.get("vout").and_then(Value::as_array) else {
                continue;
            };
            for (index, out) in vouts.iter().enumerate() {
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
                if out
                    .get("spentTxId")
                    .and_then(Value::as_str)
                    .is_some_and(|v| !v.is_empty())
                {
                    continue;
                }
                let value_sats = out
                    .get("valueSat")
                    .and_then(atomic_to_sats)
                    .or_else(|| out.get("value").and_then(coins_to_sats))
                    .unwrap_or(0);
                let script_pubkey_hex = spk
                    .get("hex")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if value_sats == 0
                    || txid.len() != 64
                    || !txid.bytes().all(|b| b.is_ascii_hexdigit())
                    || script_pubkey_hex.is_empty()
                {
                    continue;
                }
                let vout = out
                    .get("n")
                    .and_then(Value::as_u64)
                    .unwrap_or(index as u64)
                    .try_into()
                    .map_err(|_| ChainError {
                        message: "ZER explorer vout out of range".into(),
                    })?;
                let value = u64::try_from(value_sats).map_err(|_| ChainError {
                    message: "ZER explorer UTXO amount out of range".into(),
                })?;
                candidates.push(Utxo {
                    txid: txid.to_string(),
                    vout,
                    value,
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
                        let value_sats = u
                            .get("satoshis")
                            .and_then(atomic_to_sats)
                            .or_else(|| u.get("amount").and_then(coins_to_sats))
                            .unwrap_or(0);
                        let value = u64::try_from(value_sats).map_err(|_| ChainError {
                            message: "ZER Insight UTXO amount out of range".into(),
                        })?;
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

    /// Public Insight first, then ZeroChain `rawtx` with a configured API key,
    /// then local zerod. Only an already-signed transaction leaves the worker.
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
        if let Some(key) = self.explorer_key.as_deref() {
            match self.broadcast_zerochain_raw(raw_hex, key).await {
                Ok(txid) => return Ok(txid),
                Err(e) => {
                    tracing::warn!(error = %e.message, "ZER ZeroChain rawtx broadcast failed");
                    last = format!("{last}; ZeroChain={}", e.message);
                }
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
            message: format!("ZER broadcast failed (public providers + local): {last}"),
        })
    }

    async fn broadcast_zerochain_raw(
        &self,
        raw_hex: &str,
        key: &str,
    ) -> Result<String, ChainError> {
        if raw_hex.is_empty() || !raw_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ChainError {
                message: "ZER raw transaction is not hex".into(),
            });
        }
        let url = format!("{}/rawtx/{raw_hex}/{key}", self.explorer_base);
        let response = self.http.get(url).send().await.map_err(|_| ChainError {
            message: "ZER ZeroChain rawtx request failed".into(),
        })?;
        if !response.status().is_success() {
            return Err(ChainError {
                message: format!("ZER ZeroChain rawtx HTTP {}", response.status()),
            });
        }
        let body = response.text().await.map_err(|_| ChainError {
            message: "ZER ZeroChain rawtx response unreadable".into(),
        })?;
        parse_zerochain_txid(&body).ok_or_else(|| ChainError {
            message: "ZER ZeroChain rawtx response has no txid".into(),
        })
    }
}

fn parse_zerochain_txid(body: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(body).ok();
    let candidate = match parsed.as_ref() {
        Some(Value::String(txid)) => txid.as_str(),
        Some(Value::Object(map)) => map
            .get("txid")
            .or_else(|| map.get("hash"))
            .or_else(|| map.get("result"))?
            .as_str()?,
        _ => body.trim(),
    };
    (candidate.len() == 64 && candidate.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| candidate.to_ascii_lowercase())
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

fn parse_zerochain_deposits(body: &Value, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
    let rows = body
        .get("txs")
        .and_then(Value::as_array)
        .ok_or_else(|| ChainError {
            message: "ZER explorer tx history missing txs".into(),
        })?;
    let mut deposits = Vec::new();
    for tx in rows {
        let txid = tx
            .get("txid")
            .and_then(Value::as_str)
            .ok_or_else(|| ChainError {
                message: "ZER explorer transaction missing txid".into(),
            })?;
        if txid.len() != 64 || !txid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ChainError {
                message: "ZER explorer transaction has invalid txid".into(),
            });
        }
        let confirmations = tx
            .get("confirmations")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(u32::MAX as u64) as u32;
        let outputs = tx
            .get("vout")
            .and_then(Value::as_array)
            .ok_or_else(|| ChainError {
                message: "ZER explorer transaction missing vout".into(),
            })?;
        for (index, output) in outputs.iter().enumerate() {
            let spk = output.get("scriptPubKey");
            let owned = spk
                .and_then(|s| s.get("addresses"))
                .and_then(Value::as_array)
                .is_some_and(|addresses| addresses.iter().any(|v| v.as_str() == Some(address)))
                || spk.and_then(|s| s.get("address")).and_then(Value::as_str) == Some(address);
            if !owned {
                continue;
            }
            let amount = output
                .get("valueSat")
                .and_then(atomic_to_sats)
                .or_else(|| output.get("value").and_then(coins_to_sats))
                .ok_or_else(|| ChainError {
                    message: "ZER explorer output has invalid amount".into(),
                })?;
            if amount == 0 {
                continue;
            }
            let vout = output
                .get("n")
                .and_then(Value::as_u64)
                .unwrap_or(index as u64)
                .try_into()
                .map_err(|_| ChainError {
                    message: "ZER explorer vout out of range".into(),
                })?;
            deposits.push(OnchainTx {
                tx_hash: txid.to_owned(),
                vout,
                amount,
                confirmations,
                address: address.to_owned(),
            });
        }
    }
    Ok(deposits)
}

fn atomic_to_sats(v: &Value) -> Option<u128> {
    match v {
        Value::Number(n) => n.as_u64().map(u128::from),
        Value::String(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
            s.parse().ok()
        }
        _ => None,
    }
}

fn coins_to_sats(v: &Value) -> Option<u128> {
    let number = match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.trim().to_owned(),
        _ => return None,
    };
    let (mantissa, exponent) = number
        .split_once(['e', 'E'])
        .map_or((number.as_str(), 0), |(m, e)| {
            (m, e.parse::<i32>().ok().unwrap_or(i32::MIN))
        });
    if exponent == i32::MIN {
        return None;
    }
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if (whole.is_empty() && fraction.is_empty())
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digits = format!("{whole}{fraction}");
    let coefficient: u128 = digits.parse().ok()?;
    let scale = 8i32
        .checked_add(exponent)?
        .checked_sub(fraction.len() as i32)?;
    if scale >= 0 {
        coefficient.checked_mul(10u128.checked_pow(scale as u32)?)
    } else {
        let divisor = 10u128.checked_pow(scale.unsigned_abs())?;
        (coefficient % divisor == 0).then_some(coefficient / divisor)
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

    #[tokio::test]
    async fn zerochain_broadcast_sends_only_signed_hex_and_hides_api_key_on_error() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let n = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]);
            assert!(request.starts_with("GET /api/rawtx/deadbeef/test-api-key HTTP/1.1"));
            socket
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        let client = ZerClient::with_rpc(&format!("http://{addr}/api"), Some("test-api-key"), None);
        let err = client
            .broadcast_zerochain_raw("deadbeef", "test-api-key")
            .await
            .unwrap_err();
        assert!(err.message.contains("503"));
        assert!(!err.message.contains("test-api-key"));
        server.await.unwrap();
    }

    #[test]
    fn zerochain_broadcast_requires_a_real_txid() {
        let txid = "a".repeat(64);
        assert_eq!(parse_zerochain_txid(&txid), Some(txid.clone()));
        assert_eq!(
            parse_zerochain_txid(&format!(r#"{{"txid":"{txid}"}}"#)),
            Some(txid)
        );
        assert_eq!(parse_zerochain_txid(r#"{"error":"rejected"}"#), None);
    }

    #[test]
    fn zerochain_output_parser_credits_only_matching_vout_in_atomic_units() {
        let txid = "a".repeat(64);
        let body = json!({"pagesTotal": 1, "txs": [{
            "txid": txid, "confirmations": 6,
            "vout": [
                {"n": 0, "valueSat": 25000, "value": 0.00025,
                 "scriptPubKey": {"addresses": ["mine"]}},
                {"n": 1, "valueSat": 30000,
                 "scriptPubKey": {"addresses": ["other"]}}
            ]
        }]});
        let deposits = parse_zerochain_deposits(&body, "mine").unwrap();
        assert_eq!(deposits.len(), 1);
        assert_eq!(deposits[0].amount, 25_000);
        assert_eq!(deposits[0].vout, 0);
        assert_eq!(deposits[0].confirmations, 6);
        assert!(parse_zerochain_deposits(&json!({"error":"rate limited"}), "mine").is_err());
    }

    #[test]
    fn zerochain_amounts_keep_satoshis_and_coins_distinct() {
        assert_eq!(atomic_to_sats(&json!(25_000)), Some(25_000));
        assert_eq!(coins_to_sats(&json!(0.00025)), Some(25_000));
        assert_eq!(coins_to_sats(&json!("1e-8")), Some(1));
        assert_eq!(coins_to_sats(&json!("0.000000001")), None);
        assert_eq!(atomic_to_sats(&json!("0.00025")), None);
    }

    #[tokio::test]
    async fn zerochain_utxos_account_for_spends_on_later_pages() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let spent_txid = "a".repeat(64);
        let live_txid = "b".repeat(64);
        let first = json!({"pagesTotal": 2, "txs": [{
            "txid": spent_txid, "confirmations": 3,
            "vout": [{"n": 0, "valueSat": 25_000,
                "scriptPubKey": {"addresses": ["mine"], "hex": "76a91400"}}]
        }, {
            "txid": live_txid, "confirmations": 3,
            "vout": [{"n": 2, "valueSat": 30_000,
                "scriptPubKey": {"addresses": ["mine"], "hex": "76a91411"}}]
        }]});
        let second = json!({"pagesTotal": 2, "txs": [{
            "txid": "c".repeat(64), "confirmations": 2,
            "vin": [{"txid": "a".repeat(64), "vout": 0}],
            "vout": []
        }]});
        let server = tokio::spawn(async move {
            for (page, body) in [(0, first), (1, second)] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0u8; 1024];
                let n = socket.read(&mut request).await.unwrap();
                assert!(String::from_utf8_lossy(&request[..n])
                    .starts_with(&format!("GET /api/txs/mine/{page}/test-key HTTP/1.1")));
                let body = body.to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let client = ZerClient::with_rpc(&format!("http://{addr}/api"), Some("test-key"), None);
        let utxos = client.explorer_utxos("mine").await.unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].txid, live_txid);
        assert_eq!(utxos[0].vout, 2);
        assert_eq!(utxos[0].value, 30_000);
        server.await.unwrap();
    }
}
