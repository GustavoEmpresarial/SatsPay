//! Real JSON-RPC client for Polygon (POL), using a public no-API-key RPC
//! endpoint (verified live against mainnet during development). Native
//! transfers don't emit logs (that's only for ERC-20/contract events), so
//! detecting deposits without a paid indexer/trace API means scanning
//! recent blocks for transactions addressed to our deposit address — real,
//! correct, and free, but O(blocks scanned); a production deployment with
//! meaningful volume should replace this with a funded indexer API
//! (Alchemy/Infura/self-hosted) or a websocket log subscription.

use crate::types::OnchainTx;
use serde_json::{json, Value};

#[derive(Debug, thiserror::Error)]
pub enum EvmError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("unexpected response shape: {0}")]
    UnexpectedShape(String),
}

pub struct EvmClient {
    http: reqwest::Client,
    rpc_url: String,
}

impl EvmClient {
    pub fn new(rpc_url: &str) -> Self {
        Self { http: reqwest::Client::new(), rpc_url: rpc_url.to_string() }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, EvmError> {
        let body = json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": 1 });
        let resp: Value = self.http.post(&self.rpc_url).json(&body).send().await?.error_for_status()?.json().await?;
        if let Some(err) = resp.get("error") {
            return Err(EvmError::Rpc(err.to_string()));
        }
        resp.get("result").cloned().ok_or_else(|| EvmError::UnexpectedShape(resp.to_string()))
    }

    fn parse_hex_u128(v: &Value) -> Result<u128, EvmError> {
        let s = v.as_str().ok_or_else(|| EvmError::UnexpectedShape(v.to_string()))?;
        u128::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|e| EvmError::UnexpectedShape(e.to_string()))
    }

    fn parse_hex_u64(v: &Value) -> Result<u64, EvmError> {
        let s = v.as_str().ok_or_else(|| EvmError::UnexpectedShape(v.to_string()))?;
        u64::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|e| EvmError::UnexpectedShape(e.to_string()))
    }

    /// Native POL balance, in wei (18 decimals — matches `CoinConfig::decimals` for POL).
    pub async fn get_balance(&self, address: &str) -> Result<u128, EvmError> {
        let result = self.call("eth_getBalance", json!([address, "latest"])).await?;
        Self::parse_hex_u128(&result)
    }

    pub async fn latest_block_number(&self) -> Result<u64, EvmError> {
        let result = self.call("eth_blockNumber", json!([])).await?;
        Self::parse_hex_u64(&result)
    }

    /// Scans the last `lookback_blocks` for native transfers **to**
    /// `address`. `lookback_blocks` is caller-supplied (not a hardcoded
    /// constant here) so it can be sized to the actual poll interval and
    /// chain block time by whoever configures the deposit watcher.
    pub async fn fetch_received(&self, address: &str, lookback_blocks: u64) -> Result<Vec<OnchainTx>, EvmError> {
        let tip = self.latest_block_number().await?;
        let from_block = tip.saturating_sub(lookback_blocks);
        let mut out = Vec::new();

        for block_num in from_block..=tip {
            let block_hex = format!("0x{block_num:x}");
            let block = self.call("eth_getBlockByNumber", json!([block_hex, true])).await?;
            let Some(txs) = block.get("transactions").and_then(|t| t.as_array()) else { continue };
            for tx in txs {
                let Some(to) = tx.get("to").and_then(|v| v.as_str()) else { continue }; // contract creation has to=null
                if !to.eq_ignore_ascii_case(address) {
                    continue;
                }
                let value = tx.get("value").ok_or_else(|| EvmError::UnexpectedShape("tx missing value".into()))?;
                let amount = Self::parse_hex_u128(value)?;
                if amount == 0 {
                    continue;
                }
                let hash = tx.get("hash").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                out.push(OnchainTx { tx_hash: hash, vout: 0, amount, confirmations: (tip - block_num + 1) as u32, address: to.to_string() });
            }
        }
        Ok(out)
    }

    /// `eth_getTransactionCount` at `latest` — the next nonce for `address`.
    pub async fn get_transaction_count(&self, address: &str) -> Result<u64, EvmError> {
        let result = self.call("eth_getTransactionCount", json!([address, "latest"])).await?;
        Self::parse_hex_u64(&result)
    }

    /// Live `eth_gasPrice` in wei.
    pub async fn gas_price(&self) -> Result<u128, EvmError> {
        let result = self.call("eth_gasPrice", json!([])).await?;
        Self::parse_hex_u128(&result)
    }

    /// `eth_estimateGas` for a native value transfer (`data` empty).
    pub async fn estimate_gas(&self, from: &str, to: &str, value_wei: u128) -> Result<u64, EvmError> {
        let tx = json!({
            "from": from,
            "to": to,
            "value": format!("0x{value_wei:x}"),
        });
        let result = self.call("eth_estimateGas", json!([tx])).await?;
        Self::parse_hex_u64(&result)
    }

    /// Broadcasts a signed raw transaction (hex, `0x`-prefixed). Returns the tx hash.
    pub async fn broadcast(&self, raw_tx_hex: &str) -> Result<String, EvmError> {
        let result = self.call("eth_sendRawTransaction", json!([raw_tx_hex])).await?;
        result.as_str().map(str::to_string).ok_or_else(|| EvmError::UnexpectedShape(result.to_string()))
    }

    pub async fn estimate_gas_data(&self, from: &str, to: &str, data: &str) -> Result<u64, EvmError> {
        let tx = json!({ "from": from, "to": to, "data": data, "value": "0x0" });
        let result = self.call("eth_estimateGas", json!([tx])).await?;
        Self::parse_hex_u64(&result)
    }

    /// `eth_getTransactionReceipt` → `Some(true)` success, `Some(false)` reverted, `None` not mined yet.
    pub async fn transaction_receipt_ok(&self, tx_hash: &str) -> Result<Option<bool>, EvmError> {
        let result = self.call("eth_getTransactionReceipt", json!([tx_hash])).await?;
        if result.is_null() {
            return Ok(None);
        }
        let status = result
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvmError::UnexpectedShape("receipt missing status".into()))?;
        Ok(Some(status == "0x1" || status == "0x01"))
    }

    /// Poll until receipt exists or attempts exhausted.
    pub async fn wait_receipt_success(&self, tx_hash: &str, attempts: u32, delay_secs: u64) -> Result<bool, EvmError> {
        for _ in 0..attempts {
            match self.transaction_receipt_ok(tx_hash).await? {
                Some(ok) => return Ok(ok),
                None => tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await,
            }
        }
        Err(EvmError::Rpc(format!("receipt for {tx_hash} not available after {attempts} attempts")))
    }

    pub async fn erc20_balance(&self, token: &str, holder: &str) -> Result<u128, EvmError> {
        let holder_bytes = crate::evm_sign::parse_address(holder).map_err(|e| EvmError::UnexpectedShape(e.to_string()))?;
        let mut data = vec![0x70, 0xa0, 0x82, 0x31];
        data.extend_from_slice(&[0u8; 12]);
        data.extend_from_slice(&holder_bytes);
        let result = self
            .call("eth_call", json!([{ "to": token, "data": format!("0x{}", hex::encode(data)) }, "latest"]))
            .await?;
        Self::parse_hex_u128(&result)
    }

    pub async fn fetch_erc20_received(&self, token: &str, to_address: &str, lookback_blocks: u64) -> Result<Vec<OnchainTx>, EvmError> {
        let tip = self.latest_block_number().await?;
        let from_block = tip.saturating_sub(lookback_blocks);
        let to_bytes = crate::evm_sign::parse_address(to_address).map_err(|e| EvmError::UnexpectedShape(e.to_string()))?;
        let topic_to = format!("0x{:0>64}", hex::encode(to_bytes));
        let logs = self
            .call(
                "eth_getLogs",
                json!([{
                    "fromBlock": format!("0x{from_block:x}"),
                    "toBlock": format!("0x{tip:x}"),
                    "address": token,
                    "topics": [
                        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                        Value::Null,
                        topic_to
                    ]
                }]),
            )
            .await?;
        let Some(arr) = logs.as_array() else {
            return Ok(vec![]);
        };
        let mut out = Vec::new();
        for log in arr {
            let hash = log.get("transactionHash").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let data = log.get("data").and_then(|v| v.as_str()).unwrap_or("0x0");
            let amount = u128::from_str_radix(data.trim_start_matches("0x"), 16).unwrap_or(0);
            if amount == 0 {
                continue;
            }
            let block = log
                .get("blockNumber")
                .and_then(|v| v.as_str())
                .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                .unwrap_or(tip);
            out.push(OnchainTx {
                tx_hash: hash,
                vout: 0,
                amount,
                confirmations: (tip.saturating_sub(block) + 1) as u32,
                address: to_address.to_string(),
            });
        }
        Ok(out)
    }
}

pub fn erc20_transfer_data(to: [u8; 20], amount: u128) -> Vec<u8> {
    let mut data = vec![0xa9, 0x05, 0x9c, 0xbb];
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(&to);
    let mut amt = [0u8; 32];
    amt[16..].copy_from_slice(&amount.to_be_bytes());
    data.extend_from_slice(&amt);
    data
}
