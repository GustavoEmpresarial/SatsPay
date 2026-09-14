//! Live Solana JSON-RPC client: unique ed25519 deposit addresses, balance,
//! incoming transfers, and SystemProgram withdrawals.
//!
//! All traffic goes through `SOL_RPC_URL` (node VM proxy). No public fallbacks.

use crate::encoding::{solana_address_decode, solana_address_encode};
use crate::types::{BroadcastError, BroadcastResult, ChainError, OnchainTx};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use shared::{from_onchain_amount, to_onchain_amount, Coin};

type HmacSha256 = Hmac<Sha256>;

const SYSTEM_PROGRAM: [u8; 32] = [0u8; 32];

/// Default when `SOL_RPC_URL` is unset — node VM JSON-RPC proxy.
pub const DEFAULT_SOL_RPC: &str = "http://62.171.138.114:8899";

pub fn derive_sol_secret(master: &[u8], domain: &[u8], index: u32) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(master).expect("HMAC-SHA256 accepts any key length");
    mac.update(domain);
    mac.update(&index.to_be_bytes());
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

pub fn sol_address_from_secret(secret: &[u8; 32]) -> String {
    let sk = SigningKey::from_bytes(secret);
    solana_address_encode(sk.verifying_key().as_bytes())
}

pub fn sol_master_from_hot_key(hot_key: &str) -> [u8; 32] {
    use sha2::Digest;
    let clean = hot_key.trim().strip_prefix("0x").unwrap_or(hot_key.trim());
    let mut hasher = Sha256::new();
    hasher.update(b"bitcosats-sol-master");
    hasher.update(clean.as_bytes());
    hasher.finalize().into()
}

pub struct SolClient {
    http: reqwest::Client,
    rpc_url: String,
}

impl SolClient {
    pub fn new(rpc_url: &str) -> Self {
        let rpc_url = if rpc_url.trim().is_empty() {
            DEFAULT_SOL_RPC.to_string()
        } else {
            rpc_url.trim().to_string()
        };
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(25))
                .user_agent("satspay-sol/1.0")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            rpc_url,
        }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let resp: Value = self
            .http
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ChainError { message: format!("{}: {e}", self.rpc_url) })?
            .error_for_status()
            .map_err(|e| ChainError { message: format!("{}: {e}", self.rpc_url) })?
            .json()
            .await
            .map_err(|e| ChainError { message: format!("{}: {e}", self.rpc_url) })?;
        if let Some(err) = resp.get("error") {
            return Err(ChainError { message: format!("{}: {err}", self.rpc_url) });
        }
        resp.get("result")
            .cloned()
            .ok_or_else(|| ChainError { message: format!("{}: {resp}", self.rpc_url) })
    }

    pub async fn get_balance_lamports(&self, address: &str) -> Result<u128, ChainError> {
        let result = self.call("getBalance", json!([address])).await?;
        let lamports = result
            .get("value")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ChainError { message: "getBalance missing value".into() })?;
        Ok(lamports as u128)
    }

    pub async fn get_balance_internal(&self, address: &str) -> Result<u128, ChainError> {
        Ok(from_onchain_amount(Coin::Sol, self.get_balance_lamports(address).await?))
    }

    pub async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let sigs = self
            .call("getSignaturesForAddress", json!([address, { "limit": 50 }]))
            .await?;
        let Some(items) = sigs.as_array() else {
            return Ok(vec![]);
        };
        let mut out = Vec::new();
        for item in items.iter().take(25) {
            let Some(sig) = item.get("signature").and_then(|s| s.as_str()) else { continue };
            let err_ok = item.get("err").map(|e| e.is_null()).unwrap_or(true);
            if !err_ok {
                continue;
            }
            let tx = match self
                .call(
                    "getTransaction",
                    json!([sig, { "encoding": "jsonParsed", "maxSupportedTransactionVersion": 0 }]),
                )
                .await
            {
                Ok(v) => v,
                Err(_) => continue,
            };
            let slot = tx.get("slot").and_then(|s| s.as_u64()).unwrap_or(0);
            let confs = item
                .get("confirmationStatus")
                .and_then(|s| s.as_str())
                .map(|s| match s {
                    "finalized" => 32,
                    "confirmed" => 1,
                    _ => 0,
                })
                .unwrap_or(if slot > 0 { 32 } else { 0 });

            let Some(meta) = tx.get("meta") else { continue };
            if !meta.get("err").map(|e| e.is_null()).unwrap_or(true) {
                continue;
            }
            let keys = tx
                .pointer("/transaction/message/accountKeys")
                .and_then(|k| k.as_array())
                .cloned()
                .unwrap_or_default();
            let pre = meta.get("preBalances").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let post = meta.get("postBalances").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            for (i, key) in keys.iter().enumerate() {
                let pubkey = key
                    .get("pubkey")
                    .and_then(|p| p.as_str())
                    .or_else(|| key.as_str())
                    .unwrap_or("");
                if !pubkey.eq_ignore_ascii_case(address) {
                    continue;
                }
                let before = pre.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u128;
                let after = post.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u128;
                if after <= before {
                    continue;
                }
                let lamports = after - before;
                let amount = from_onchain_amount(Coin::Sol, lamports);
                if amount == 0 {
                    continue;
                }
                out.push(OnchainTx {
                    tx_hash: sig.to_string(),
                    vout: i as u32,
                    amount,
                    confirmations: confs,
                    address: address.to_string(),
                });
            }
        }
        Ok(out)
    }

    pub async fn broadcast_transfer(
        &self,
        from_secret: &[u8; 32],
        to_address: &str,
        amount_internal: u128,
    ) -> Result<BroadcastResult, BroadcastError> {
        let lamports = to_onchain_amount(Coin::Sol, amount_internal);
        if lamports == 0 {
            return Err(BroadcastError { message: "SOL amount too small after scale".into(), safe_to_reverse: true });
        }
        let to = solana_address_decode(to_address).ok_or_else(|| BroadcastError {
            message: format!("invalid Solana address: {to_address}"),
            safe_to_reverse: true,
        })?;
        let sk = SigningKey::from_bytes(from_secret);
        let from_pk = *sk.verifying_key().as_bytes();

        let bh = self
            .call("getLatestBlockhash", json!([{ "commitment": "finalized" }]))
            .await
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let hash_b58 = bh
            .pointer("/value/blockhash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BroadcastError { message: "missing blockhash".into(), safe_to_reverse: true })?;
        let blockhash = bs58::decode(hash_b58)
            .into_vec()
            .ok()
            .and_then(|v| <[u8; 32]>::try_from(v).ok())
            .ok_or_else(|| BroadcastError { message: "bad blockhash".into(), safe_to_reverse: true })?;

        let raw = sign_legacy_transfer(&sk, &from_pk, &to, lamports as u64, &blockhash);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
        let result = self
            .call("sendTransaction", json!([b64, { "encoding": "base64" }]))
            .await
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let sig = result
            .as_str()
            .ok_or_else(|| BroadcastError { message: result.to_string(), safe_to_reverse: false })?
            .to_string();
        Ok(BroadcastResult { tx_hash: sig, fee_amount: 5000 })
    }
}

fn compact_u16(n: u16, out: &mut Vec<u8>) {
    if n < 128 {
        out.push(n as u8);
    } else if n < 16384 {
        out.push(((n & 0x7f) as u8) | 0x80);
        out.push((n >> 7) as u8);
    } else {
        out.push(((n & 0x7f) as u8) | 0x80);
        out.push((((n >> 7) & 0x7f) as u8) | 0x80);
        out.push((n >> 14) as u8);
    }
}

fn sign_legacy_transfer(sk: &SigningKey, from: &[u8; 32], to: &[u8; 32], lamports: u64, recent_blockhash: &[u8; 32]) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.push(1); // num required signatures
    msg.push(0); // num readonly signed
    msg.push(1); // num readonly unsigned (system program)
    compact_u16(3, &mut msg);
    msg.extend_from_slice(from);
    msg.extend_from_slice(to);
    msg.extend_from_slice(&SYSTEM_PROGRAM);
    msg.extend_from_slice(recent_blockhash);
    compact_u16(1, &mut msg);
    msg.push(2); // program id index
    compact_u16(2, &mut msg);
    msg.push(0);
    msg.push(1);
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    compact_u16(data.len() as u16, &mut msg);
    msg.extend_from_slice(&data);

    let sig = sk.sign(&msg);
    let mut tx = Vec::new();
    compact_u16(1, &mut tx);
    tx.extend_from_slice(&sig.to_bytes());
    tx.extend_from_slice(&msg);
    tx
}
