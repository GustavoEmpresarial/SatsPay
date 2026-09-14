//! Regtest broadcast smoke — proves a **real** signed tx is accepted by a
//! live bitcoind node with **real mined funds** (no faucet/captcha).
//!
//! Requires a local bitcoind `-regtest` with RPC reachable and a funded
//! UTXO for `HOT_WALLET_WIF`. See docs for the docker one-liner used in CI-ish
//! local exercises.
//!
//! Required env (fail-fast, no invented defaults):
//! - `BITCOIN_RPC_URL` — e.g. `http://127.0.0.1:18443`
//! - `BITCOIN_RPC_USER`
//! - `BITCOIN_RPC_PASSWORD`
//! - `HOT_WALLET_WIF` — WIF controlling a funded P2WPKH UTXO on that node
//!
//! Fee rate: read from the node's `estimatesmartfee` (regtest falls back to
//! the node's `minrelaytxfee` when estimates are unavailable) — never a
//! hardcoded sat/vB.

use chain::btc_sign::{build_and_sign_p2wpkh, Utxo};
use serde_json::{json, Value};
use std::process::ExitCode;

fn require_env(name: &str) -> Result<String, ExitCode> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => {
            eprintln!("error: {name} must be set");
            Err(ExitCode::from(1))
        }
    }
}

async fn rpc(url: &str, user: &str, pass: &str, method: &str, params: Value) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let body = json!({ "jsonrpc": "1.0", "id": "smoke", "method": method, "params": params });
    let resp: Value = client
        .post(url)
        .basic_auth(user, Some(pass))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(err) = resp.get("error").filter(|e| !e.is_null()) {
        return Err(format!("rpc {method}: {err}"));
    }
    resp.get("result").cloned().ok_or_else(|| format!("rpc {method}: missing result"))
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(c) => c,
    }
}

async fn run() -> Result<(), ExitCode> {
    let rpc_url = require_env("BITCOIN_RPC_URL")?;
    let rpc_user = require_env("BITCOIN_RPC_USER")?;
    let rpc_pass = require_env("BITCOIN_RPC_PASSWORD")?;
    let wif = require_env("HOT_WALLET_WIF")?;

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let privkey = bitcoin::PrivateKey::from_wif(&wif).map_err(|e| {
        eprintln!("error: invalid HOT_WALLET_WIF: {e}");
        ExitCode::from(1)
    })?;
    let compressed = bitcoin::CompressedPublicKey::from_private_key(&secp, &privkey).map_err(|e| {
        eprintln!("error: pubkey must be compressed for P2WPKH: {e}");
        ExitCode::from(1)
    })?;
    // Regtest shares the testnet WIF version byte; force regtest params for
    // address encoding when talking to a local -regtest node.
    let our_address = bitcoin::Address::p2wpkh(&compressed, bitcoin::Network::Regtest);
    let our_script = our_address.script_pubkey();
    println!("hot address (regtest): {our_address}");

    // All UTXOs for this address from the node wallet / scantxoutset.
    let scan = rpc(&rpc_url, &rpc_user, &rpc_pass, "scantxoutset", json!(["start", [format!("addr({our_address})")]])).await.map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(1)
    })?;
    let unspents = scan.get("unspents").and_then(|u| u.as_array()).cloned().unwrap_or_default();
    if unspents.is_empty() {
        eprintln!("error: no UTXOs for {our_address} — fund this address on the regtest node then re-run");
        return Err(ExitCode::from(2));
    }

    let mut utxos = Vec::new();
    for u in &unspents {
        let txid = u.get("txid").and_then(|v| v.as_str()).ok_or(ExitCode::from(1))?;
        let vout = u.get("vout").and_then(|v| v.as_u64()).ok_or(ExitCode::from(1))? as u32;
        let amount_btc = u.get("amount").and_then(|v| v.as_f64()).ok_or(ExitCode::from(1))?;
        let value = (amount_btc * 100_000_000.0).round() as u64;
        let script_pubkey_hex = u.get("scriptPubKey").and_then(|v| v.as_str()).ok_or(ExitCode::from(1))?.to_string();
        utxos.push(Utxo { txid: txid.to_string(), vout, value, script_pubkey_hex });
    }
    let total: u64 = utxos.iter().map(|u| u.value).sum();
    println!("spendable {total} sats across {} utxo(s)", utxos.len());

    // Fee: estimatesmartfee(1); on regtest this often fails — fall back to
    // minrelaytxfee from getnetworkinfo (node-reported, not invented).
    let vsize_est = 11 + utxos.len() * 68 + 31; // 1-in-N / 1-out P2WPKH formula
    let fee_rate_btc_kvb = match rpc(&rpc_url, &rpc_user, &rpc_pass, "estimatesmartfee", json!([1])).await {
        Ok(v) => v.get("feerate").and_then(|f| f.as_f64()),
        Err(_) => None,
    };
    let fee_rate_btc_kvb = match fee_rate_btc_kvb {
        Some(r) => r,
        None => {
            let net = rpc(&rpc_url, &rpc_user, &rpc_pass, "getnetworkinfo", json!([])).await.map_err(|e| {
                eprintln!("error: {e}");
                ExitCode::from(1)
            })?;
            net.get("relayfee").and_then(|f| f.as_f64()).ok_or_else(|| {
                eprintln!("error: node did not report relayfee");
                ExitCode::from(1)
            })?
        }
    };
    // BTC/kvB → sat/vB
    let sat_per_vb = ((fee_rate_btc_kvb * 100_000_000.0) / 1000.0).ceil().max(1.0) as u64;
    let fee = sat_per_vb * vsize_est as u64;
    if total <= fee {
        eprintln!("error: balance {total} sats ≤ fee {fee} sats");
        return Err(ExitCode::from(1));
    }
    let send_amount = total - fee;
    println!("fee_rate={sat_per_vb} sat/vB vsize_est={vsize_est} fee={fee} send={send_amount}");

    // Self-send single output (no change) — avoids dust constants.
    let tx = build_and_sign_p2wpkh(&wif, &utxos, &our_script, send_amount, fee, &our_script).map_err(|e| {
        eprintln!("error: sign failed: {e}");
        ExitCode::from(1)
    })?;
    let raw = bitcoin::consensus::encode::serialize_hex(&tx);

    let txid = rpc(&rpc_url, &rpc_user, &rpc_pass, "sendrawtransaction", json!([raw])).await.map_err(|e| {
        eprintln!("error: broadcast rejected: {e}");
        ExitCode::from(1)
    })?;
    let txid = txid.as_str().unwrap_or_default();
    println!("BROADCAST_OK txid={txid}");

    // Mine one block so it confirms (regtest).
    let miner = rpc(&rpc_url, &rpc_user, &rpc_pass, "getnewaddress", json!([])).await.ok();
    if let Some(Value::String(miner_addr)) = miner {
        let _ = rpc(&rpc_url, &rpc_user, &rpc_pass, "generatetoaddress", json!([1, miner_addr])).await;
        println!("mined 1 regtest block for confirmation");
    }

    Ok(())
}
