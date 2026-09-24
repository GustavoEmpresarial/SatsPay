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
use std::hash::{Hash, Hasher};

/// Blockbook explorer used when the Insight hosts are down.
pub const BLOCKBOOK_FALLBACK: &str = "https://digibyte.atomicwallet.io/api";

pub struct DgbClient {
    http: reqwest::Client,
    bases: Vec<String>,
    rpc: Option<UtxoNodeRpc>,
    rpc_provider: Option<String>,
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
        let (rpc, rpc_provider) = rpc_url.map_or((None, None), |raw| {
            let raw = raw.trim();
            if raw.is_empty() {
                return (None, None);
            }
            match UtxoNodeRpc::new(raw) {
                Ok(c) => {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    raw.hash(&mut hasher);
                    (Some(c), Some(format!("dgb_node_rpc_{:x}", hasher.finish())))
                }
                Err(e) => {
                    tracing::warn!(error = %e.message, "DGB_RPC_URL invalid; Insight fallback only");
                    (None, None)
                }
            }
        });
        let http = reqwest::Client::builder()
            .user_agent("SatsPay-Dgb/1.0")
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { http, bases, rpc, rpc_provider }
    }

    /// First host that answers 2xx **with a body that parses as `T`** wins.
    /// A 200 carrying HTML (maintenance page, SPA catch-all) is a failure of
    /// that host, not of the whole call: record it and try the next one.
    /// Explicit host list, no built-in fallbacks: tests must never reach a real explorer.
    #[cfg(test)]
    fn with_bases_for_test(bases: Vec<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            bases,
            rpc: None,
            rpc_provider: None,
        }
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, suffix: &str) -> Result<T, ChainError> {
        let mut errors: Vec<String> = Vec::new();
        for (index, base) in self.bases.iter().enumerate() {
            let url = format!("{base}{suffix}");
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            base.hash(&mut hasher);
            // Insight and Blockbook paths may coexist on a host. A 404 for
            // one dialect must not open the other's circuit.
            let dialect = if suffix.starts_with("/v2/") {
                "blockbook"
            } else {
                "insight"
            };
            let provider = format!("dgb_{dialect}_{index}_{:x}", hasher.finish());
            match crate::provider_circuit::call(&provider, || async {
                let resp = self.http.get(&url).send().await.map_err(|e| ChainError {
                    message: e.to_string(),
                })?;
                if !resp.status().is_success() {
                    return Err(ChainError {
                        message: format!("HTTP {}", resp.status()),
                    });
                }
                resp.json::<T>().await.map_err(|e| ChainError {
                    message: format!("bad body: {e}"),
                })
            })
            .await
            {
                Ok(v) => return Ok(v),
                Err(e) => errors.push(format!("{provider}: {}", e.message)),
            }
        }
        Err(ChainError {
            message: format!("DGB indexers failed: {}", errors.join("; ")),
        })
    }

    pub async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        if let Some(rpc) = &self.rpc {
            let provider = self.rpc_provider.as_deref().expect("RPC provider accompanies RPC client");
            match crate::provider_circuit::call(provider, || rpc.scan_address(address)).await {
                Ok(txs) => return Ok(txs),
                Err(e) => {
                    tracing::warn!(error = %e.message, address, "DGB scantxoutset failed; trying Insight")
                }
            }
        }
        if let Ok(utxos) = self
            .get_json::<Vec<InsightUtxo>>(&format!("/addr/{address}/utxo"))
            .await
        {
            return Ok(utxos
                .into_iter()
                .map(|u| OnchainTx {
                    tx_hash: u.txid,
                    vout: u.vout,
                    amount: u.satoshis.unwrap_or_else(|| {
                        ((u.amount.unwrap_or(0.0)) * 100_000_000.0).round() as u128
                    }),
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
            let provider = self.rpc_provider.as_deref().expect("RPC provider accompanies RPC client");
            match crate::provider_circuit::call(provider, || rpc.get_balance(address)).await {
                Ok(sats) => return Ok(sats),
                Err(e) => {
                    tracing::warn!(error = %e.message, address, "DGB node balance failed; trying Insight")
                }
            }
        }
        // Insight-style `/addr/{address}`
        if let Ok(v) = self
            .get_json::<serde_json::Value>(&format!("/addr/{address}"))
            .await
        {
            if let Some(s) = v
                .get("balanceSat")
                .and_then(|x| x.as_u64())
                .map(|u| u as u128)
            {
                return Ok(s);
            }
            if let Some(s) = v
                .get("balanceSat")
                .and_then(|x| x.as_str())
                .and_then(|s| s.parse().ok())
            {
                return Ok(s);
            }
            if let Some(b) = v.get("balance").and_then(|x| x.as_f64()) {
                return Ok((b * 100_000_000.0).round() as u128);
            }
            if let Some(b) = v
                .get("balance")
                .and_then(|x| x.as_str())
                .and_then(|s| s.parse::<f64>().ok())
            {
                return Ok((b * 100_000_000.0).round() as u128);
            }
        }
        // Blockbook `/v2/address/{address}` — balance string in sats
        if let Ok(v) = self
            .get_json::<serde_json::Value>(&format!("/v2/address/{address}"))
            .await
        {
            if let Some(s) = v
                .get("balance")
                .and_then(|x| x.as_str())
                .and_then(|s| s.parse().ok())
            {
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
        let resp = self.http.get(&url).send().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("cryptoid HTTP {}", resp.status()),
            });
        }
        let text = resp.text().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;
        let trimmed = text.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('<')
            || trimmed.to_ascii_lowercase().contains("invalid")
        {
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
            let provider = self.rpc_provider.as_deref().expect("RPC provider accompanies RPC client");
            match crate::provider_circuit::call(provider, || rpc.fetch_utxos(address)).await {
                Ok(utxos) => return Ok(utxos),
                Err(e) => {
                    tracing::warn!(error = %e.message, address, "DGB node UTXOs failed; trying Insight")
                }
            }
        }
        match self
            .get_json::<Vec<InsightUtxo>>(&format!("/addr/{address}/utxo"))
            .await
        {
            Ok(raw) => Ok(raw
                .into_iter()
                .map(|u| Utxo {
                    txid: u.txid,
                    vout: u.vout,
                    value: u.satoshis.unwrap_or_else(|| {
                        ((u.amount.unwrap_or(0.0)) * 100_000_000.0).round() as u128
                    }) as u64,
                    script_pubkey_hex: u.script_pub_key.unwrap_or_default(),
                })
                .collect()),
            Err(insight_err) => {
                tracing::warn!(error = %insight_err.message, address, "DGB Insight UTXOs failed; trying Blockbook");
                // Blockbook has no scriptPubKey per UTXO: the caller derives it
                // from the address (see `fill_missing_scripts`).
                let book: BlockbookUtxos = self
                    .get_json(&format!("/v2/utxo/{address}?confirmed=true"))
                    .await?;
                blockbook_to_utxos(book)
            }
        }
    }

    pub async fn estimate_fee_sat_per_byte(&self) -> Result<u64, ChainError> {
        if let Some(rpc) = &self.rpc {
            let provider = self.rpc_provider.as_deref().expect("RPC provider accompanies RPC client");
            if let Ok(fee) = crate::provider_circuit::call(provider, || rpc.estimate_fee_sat_per_byte()).await {
                return Ok(fee.max(1));
            }
        }
        let fee_per_kb: f64 = self
            .get_json("/utils/estimatefee?nbBlocks=2")
            .await
            .unwrap_or(-1.0);
        if fee_per_kb > 0.0 {
            return Ok(((fee_per_kb * 100_000_000.0) / 1000.0).max(1.0) as u64);
        }
        // Blockbook: {"result":"0.0001"} (DGB per kB).
        if let Ok(v) = self
            .get_json::<serde_json::Value>("/v2/estimatefee/2")
            .await
        {
            if let Some(per_kb) = v
                .get("result")
                .and_then(|r| r.as_str())
                .and_then(|s| s.parse::<f64>().ok())
            {
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
            let provider = self.rpc_provider.as_deref().expect("RPC provider accompanies RPC client");
            match crate::provider_circuit::call(provider, || rpc.broadcast(raw_hex)).await {
                Ok(txid) => return Ok(txid),
                Err(e) => {
                    tracing::warn!(error = %e.message, "DGB sendrawtransaction failed; trying Insight")
                }
            }
        }
        let mut errors: Vec<String> = Vec::new();
        for (index, base) in self.bases.iter().enumerate() {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            base.hash(&mut hasher);
            let host = hasher.finish();
            let insight = format!("dgb_broadcast_insight_{index}_{host:x}");
            // Re-sending one signed transaction is idempotent (same txid).
            match crate::provider_circuit::call(&insight, || async {
                let resp = self
                    .http
                    .post(format!("{base}/tx/send"))
                    .json(&json!({ "rawtx": raw_hex }))
                    .send()
                    .await
                    .map_err(|_| ChainError {
                        message: "request failed".into(),
                    })?;
                if !resp.status().is_success() {
                    return Err(ChainError {
                        message: format!("HTTP {}", resp.status()),
                    });
                }
                let v: serde_json::Value = resp.json().await.map_err(|_| ChainError {
                    message: "bad body".into(),
                })?;
                v.get("txid")
                    .and_then(|t| t.as_str())
                    .or_else(|| v.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| ChainError {
                        message: "missing txid".into(),
                    })
            })
            .await
            {
                Ok(txid) => return Ok(txid),
                Err(e) => errors.push(format!("{insight}: {}", e.message)),
            }
            let blockbook = format!("dgb_broadcast_blockbook_{index}_{host:x}");
            match crate::provider_circuit::call(&blockbook, || async {
                let resp = self
                    .http
                    .post(format!("{base}/v2/sendtx/"))
                    .body(raw_hex.to_string())
                    .send()
                    .await
                    .map_err(|_| ChainError {
                        message: "request failed".into(),
                    })?;
                if !resp.status().is_success() {
                    return Err(ChainError {
                        message: format!("HTTP {}", resp.status()),
                    });
                }
                let v: serde_json::Value = resp.json().await.map_err(|_| ChainError {
                    message: "bad body".into(),
                })?;
                v.get("result")
                    .and_then(|t| t.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| ChainError {
                        message: "missing txid".into(),
                    })
            })
            .await
            {
                Ok(txid) => return Ok(txid),
                Err(e) => errors.push(format!("{blockbook}: {}", e.message)),
            }
        }
        Err(ChainError {
            message: format!("DGB broadcast failed: {}", errors.join("; ")),
        })
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
                message: format!(
                    "DGB Blockbook bad UTXO value {:?} for {}:{}",
                    u.value, u.txid, u.vout
                ),
            })?;
            Ok(Utxo {
                txid: u.txid,
                vout: u.vout,
                value,
                script_pubkey_hex: String::new(),
            })
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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

        let bad: BlockbookUtxos =
            serde_json::from_str(r#"[{"txid":"bb","vout":0,"value":"1.5"}]"#).unwrap();
        assert!(blockbook_to_utxos(bad).is_err());
    }

    #[test]
    fn fill_missing_scripts_only_touches_empty_ones() {
        let mut utxos = vec![
            Utxo {
                txid: "a".into(),
                vout: 0,
                value: 1,
                script_pubkey_hex: String::new(),
            },
            Utxo {
                txid: "b".into(),
                vout: 0,
                value: 1,
                script_pubkey_hex: "0014ff".into(),
            },
        ];
        fill_missing_scripts(&mut utxos, "0014aa");
        assert_eq!(utxos[0].script_pubkey_hex, "0014aa");
        assert_eq!(utxos[1].script_pubkey_hex, "0014ff");
    }

    /// Tiny HTTP/1.1 responder: `routes` maps "METHOD /path" → (status, body).
    /// Anything unmapped answers 404. One response per connection.
    async fn mock_host(routes: Vec<(&'static str, u16, &'static str)>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let routes = routes.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    // Read headers, then the declared body, so the client never sees a reset.
                    loop {
                        let n = sock.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
                            let len = head
                                .lines()
                                .find_map(|l| l.strip_prefix("content-length:"))
                                .and_then(|v| v.trim().parse::<usize>().ok())
                                .unwrap_or(0);
                            if buf.len() >= end + 4 + len {
                                break;
                            }
                        }
                    }
                    let req = String::from_utf8_lossy(&buf);
                    let mut first = req.lines().next().unwrap_or("").split_whitespace();
                    let key = format!(
                        "{} {}",
                        first.next().unwrap_or(""),
                        first.next().unwrap_or("")
                    );
                    let (status, body) = routes
                        .iter()
                        .find(|(k, _, _)| *k == key)
                        .map(|(_, st, b)| (*st, *b))
                        .unwrap_or((404, "not found"));
                    let ct = if body.starts_with('<') {
                        "text/html"
                    } else {
                        "application/json"
                    };
                    let resp = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: {ct}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        format!("http://{addr}/api")
    }

    async fn failing_rpc() -> (String, Arc<AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let server_calls = calls.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                server_calls.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut request = [0u8; 4096];
                    let _ = socket.read(&mut request).await;
                    let body = r#"{"result":null,"error":{"code":-1,"message":"down"}}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        (format!("http://{addr}"), calls)
    }

    const HTML: &str = "<html>maintenance</html>";

    #[tokio::test]
    async fn fetch_utxos_skips_a_host_that_answers_200_with_html() {
        let a = mock_host(vec![
            ("GET /api/addr/DADDR/utxo", 200, HTML),
            ("GET /api/v2/utxo/DADDR?confirmed=true", 200, HTML),
        ])
        .await;
        let b = mock_host(vec![(
            "GET /api/v2/utxo/DADDR?confirmed=true",
            200,
            r#"[{"txid":"ff","vout":2,"value":"5000","confirmations":3}]"#,
        )])
        .await;
        let utxos = DgbClient::with_bases_for_test(vec![a, b])
            .fetch_utxos("DADDR")
            .await
            .unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].value, 5000);
    }

    #[tokio::test]
    async fn broadcast_reaches_blockbook_on_the_same_host_after_an_html_200() {
        // Same host: Insight route is an SPA catch-all (200 HTML), Blockbook route works.
        let host = mock_host(vec![
            ("POST /api/tx/send", 200, HTML),
            ("POST /api/v2/sendtx/", 200, r#"{"result":"txid-bb"}"#),
        ])
        .await;
        let txid = DgbClient::with_bases_for_test(vec![host])
            .broadcast("00ff")
            .await
            .unwrap();
        assert_eq!(txid, "txid-bb");
    }

    #[tokio::test]
    async fn broadcast_moves_to_the_next_host_and_reports_every_failure() {
        let a = mock_host(vec![
            ("POST /api/tx/send", 200, HTML),
            ("POST /api/v2/sendtx/", 200, HTML),
        ])
        .await;
        let b = mock_host(vec![("POST /api/tx/send", 200, r#"{"txid":"txid-b"}"#)]).await;
        let txid = DgbClient::with_bases_for_test(vec![a.clone(), b])
            .broadcast("00ff")
            .await
            .unwrap();
        assert_eq!(txid, "txid-b");

        let err = DgbClient::with_bases_for_test(vec![a])
            .broadcast("00ff")
            .await
            .unwrap_err();
        assert!(
            err.message.contains("dgb_broadcast_insight_0_") && err.message.contains(": bad body"),
            "{}",
            err.message
        );
        assert!(
            err.message.contains("dgb_broadcast_blockbook_0_") && err.message.contains(": bad body"),
            "{}",
            err.message
        );
    }

    #[tokio::test]
    async fn configured_node_opens_circuit_after_three_failures() {
        let (rpc, calls) = failing_rpc().await;
        let insight = mock_host(vec![(
            "GET /api/addr/DADDR/utxo",
            200,
            r#"[{"txid":"ff","vout":0,"satoshis":42,"confirmations":2}]"#,
        )])
        .await;
        let client = DgbClient::with_rpc(&insight, Some(&rpc));
        for _ in 0..4 {
            let deposits = client.fetch_deposits("DADDR").await.unwrap();
            assert_eq!(deposits[0].amount, 42);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 3, "fourth wallet must skip the open 120s RPC path");
    }

    #[test]
    fn blockbook_is_always_in_the_fallback_list() {
        let c = DgbClient::new("https://my-insight.example/api/");
        assert_eq!(c.bases[0], "https://my-insight.example/api");
        assert!(c.bases.iter().any(|b| b == BLOCKBOOK_FALLBACK));
    }
}
