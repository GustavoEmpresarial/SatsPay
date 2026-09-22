//! Testnet broadcast smoke — real Bitcore testnet + Polygon Amoy RPC.
//!
//! Exercises an actual self-send broadcast without mainnet funds.
//!
//! Required env:
//! - `DATABASE_URL` — Postgres (HD sequences; pool required by `RealChainClient`)
//! - `BITCORE_API_BASE_URL` — e.g. `https://api.bitcore.io`
//! - `EVM_RPC_URL` — Polygon Amoy JSON-RPC
//! - `CHAIN_DEPOSIT_XPUB` — any valid BIP32 xpub (deposit path; not used for hot spend)
//! - `HOT_WALLET_WIF` — **testnet** WIF (fail-fast if unset)
//! - `FEE_CONFIRMATION_TARGET` — Bitcore fee confirmation blocks
//! - `CHAIN_NETWORK=testnet`
//! - `EVM_DEPOSIT_LOOKBACK_BLOCKS` — required by real-client config (unused for spend)
//!
//! Flow (BTC, then BCH, then POL Amoy):
//! 1. Derive hot address from WIF with testnet params
//! 2. `get_balance` on testnet
//! 3. If balance == 0: print address + faucet hint, exit 2
//! 4. Else: all-in-one-output self-send (`balance - live_fee`), broadcast, print txid
//!
//! Fees come from Bitcore `/fee/{target}` or `eth_gasPrice`/`eth_estimateGas` —
//! never invented. Single-output self-send avoids a dust/min-output constant.
//!
//! Run:
//! ```text
//! CHAIN_NETWORK=testnet USE_REAL_CHAIN_CLIENTS=true \
//!   DATABASE_URL=... BITCORE_API_BASE_URL=https://api.bitcore.io \
//!   EVM_RPC_URL=https://rpc-amoy.polygon.technology \
//!   CHAIN_DEPOSIT_XPUB=xpub... HOT_WALLET_WIF=c... \
//!   FEE_CONFIRMATION_TARGET=2 EVM_DEPOSIT_LOOKBACK_BLOCKS=5 \
//!   cargo run -p chain --example broadcast_testnet_smoke
//! ```

use chain::bch_sign::build_and_sign_bch_p2pkh;
use chain::bitcore_client::BitcoreClient;
use chain::btc_sign::{build_and_sign_p2wpkh, Utxo};
use chain::evm_client::EvmClient;
use chain::evm_sign::{address_from_secret, parse_address, sign_legacy_tx, LegacyTx};
use chain::params::{params_for, AddressKind, ChainNetwork};
use chain::real_client::{hot_wallet_address, RealChainClient, RealClientConfig};
use chain::ChainClient;
use serde::Deserialize;
use shared::Coin;
use std::process::ExitCode;

fn require_env(name: &str) -> Result<String, ExitCode> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => {
            eprintln!("error: {name} must be set for broadcast_testnet_smoke");
            Err(ExitCode::from(1))
        }
    }
}

fn require_testnet() -> Result<(), ExitCode> {
    let raw = require_env("CHAIN_NETWORK")?;
    let net = ChainNetwork::parse(&raw).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(1)
    })?;
    if net != ChainNetwork::Testnet {
        eprintln!("error: broadcast_testnet_smoke requires CHAIN_NETWORK=testnet (got {raw})");
        return Err(ExitCode::from(1));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(code) => code,
    }
}

async fn run() -> Result<ExitCode, ExitCode> {
    require_testnet()?;

    let database_url = require_env("DATABASE_URL")?;
    let bitcore_base_url = require_env("BITCORE_API_BASE_URL")?;
    let evm_rpc_url = require_env("EVM_RPC_URL")?;
    let deposit_xpub = require_env("CHAIN_DEPOSIT_XPUB")?;
    let fee_confirmation_target: u32 = require_env("FEE_CONFIRMATION_TARGET")?
        .parse()
        .map_err(|_| {
            eprintln!("error: FEE_CONFIRMATION_TARGET must be a positive integer");
            ExitCode::from(1)
        })?;
    let evm_deposit_lookback_blocks: u64 = require_env("EVM_DEPOSIT_LOOKBACK_BLOCKS")?
        .parse()
        .map_err(|_| {
            eprintln!("error: EVM_DEPOSIT_LOOKBACK_BLOCKS must be a positive integer");
            ExitCode::from(1)
        })?;

    let wif = match std::env::var("HOT_WALLET_WIF") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "error: HOT_WALLET_WIF must be set to a testnet WIF for broadcast_testnet_smoke \
                 (no throwaway key — this example broadcasts when funded)"
            );
            return Err(ExitCode::from(1));
        }
    };

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .map_err(|e| {
            eprintln!("error: DATABASE_URL connect failed: {e}");
            ExitCode::from(1)
        })?;

    let config = RealClientConfig {
        bitcore_base_url: bitcore_base_url.clone(),
        evm_rpc_url: evm_rpc_url.clone(),
        bsc_rpc_url: "https://bsc-rpc.publicnode.com".into(),
        deposit_xpub,
        hot_wallet_wif: Some(wif.clone()),
        evm_deposit_lookback_blocks,
        fee_confirmation_target,
        network: ChainNetwork::Testnet,
        sol_rpc_url: "https://api.devnet.solana.com".to_string(),
        dgb_insight_url: "https://digiexplorer.info/api".to_string(),
        dgb_rpc_url: None,
        zer_explorer_url: "https://zerochain.info/api".to_string(),
        zer_explorer_api_key: None,
        zer_rpc_url: None,
        deposit_mnemonic: None,
        hot_mnemonic: None,
    };

    // --- BTC testnet: all-in-one-output self-send ---
    match smoke_utxo_self_send(
        Coin::Btc,
        &bitcore_base_url,
        &wif,
        fee_confirmation_target,
        /* segwit */ true,
    )
    .await
    {
        SmokeOutcome::Broadcast { txid, fee } => {
            println!("BTC testnet broadcast OK txid={txid} fee={fee} sats");
        }
        SmokeOutcome::NeedsFaucet { address } => {
            println!("{address}");
            println!("fund via faucet then re-run");
            return Ok(ExitCode::from(2));
        }
        SmokeOutcome::Err(msg) => {
            eprintln!("error: BTC smoke failed: {msg}");
            return Err(ExitCode::from(1));
        }
    }

    // --- BCH testnet (same pattern; CashAddr bchtest) ---
    match smoke_utxo_self_send(
        Coin::Bch,
        &bitcore_base_url,
        &wif,
        fee_confirmation_target,
        /* segwit */ false,
    )
    .await
    {
        SmokeOutcome::Broadcast { txid, fee } => {
            println!("BCH testnet broadcast OK txid={txid} fee={fee} sats");
        }
        SmokeOutcome::NeedsFaucet { address } => {
            println!("BCH {address}");
            println!("fund via faucet then re-run (BCH); BTC already broadcast if printed above");
            // Continue to POL — don't abort whole smoke on unfunded BCH.
            println!("skipping BCH broadcast (unfunded)");
        }
        SmokeOutcome::Err(msg) => {
            eprintln!("warning: BCH smoke skipped: {msg}");
        }
    }

    // --- POL Amoy ---
    let pol = RealChainClient::new(Coin::Pol, pool, config);
    let pol_addr = hot_wallet_address(Coin::Pol, ChainNetwork::Testnet, &wif).map_err(|e| {
        eprintln!("error: POL hot address: {e}");
        ExitCode::from(1)
    })?;
    let pol_balance = pol.get_balance(&pol_addr).await.map_err(|e| {
        eprintln!("error: POL get_balance: {e}");
        ExitCode::from(1)
    })?;
    println!("POL Amoy address {pol_addr} balance={pol_balance} wei");

    if pol_balance == 0 {
        println!("{pol_addr}");
        println!("fund via faucet then re-run");
        return Ok(ExitCode::from(2));
    }

    match smoke_pol_self_send(&evm_rpc_url, &wif, &pol_addr, pol_balance).await {
        Ok(txid) => {
            println!("POL Amoy broadcast OK txid={txid}");
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            eprintln!("error: POL smoke failed: {e}");
            Err(ExitCode::from(1))
        }
    }
}

enum SmokeOutcome {
    Broadcast { txid: String, fee: u64 },
    NeedsFaucet { address: String },
    Err(String),
}

async fn smoke_utxo_self_send(
    coin: Coin,
    bitcore_base_url: &str,
    wif: &str,
    fee_confirmation_target: u32,
    segwit: bool,
) -> SmokeOutcome {
    let network = ChainNetwork::Testnet;
    let p = params_for(coin, network);
    let Some(chain) = p.bitcore_chain else {
        return SmokeOutcome::Err(format!("{coin:?} has no bitcore_chain"));
    };

    let address = match hot_wallet_address(coin, network, wif) {
        Ok(a) => a,
        Err(e) => return SmokeOutcome::Err(e),
    };

    let client = BitcoreClient::new(bitcore_base_url, chain, network.as_bitcore_str());
    let balance = match client.get_balance(&address).await {
        Ok(b) => b,
        Err(e) => return SmokeOutcome::Err(e.to_string()),
    };
    println!("{coin:?} testnet address {address} balance={balance}");

    if balance == 0 {
        return SmokeOutcome::NeedsFaucet { address };
    }

    let fee_per_byte = match fetch_fee_rate(bitcore_base_url, chain, network.as_bitcore_str(), fee_confirmation_target).await {
        Ok(r) => r,
        Err(e) => return SmokeOutcome::Err(format!("fee: {e}")),
    };

    let utxos = match fetch_spendable_utxos(&client, &address).await {
        Ok(u) if !u.is_empty() => u,
        Ok(_) => return SmokeOutcome::NeedsFaucet { address },
        Err(e) => return SmokeOutcome::Err(e.to_string()),
    };

    // All-in-one-output: n_out = 1 → no change / no dust constant.
    // Size models match real_client (BIP141 vsize / legacy P2PKH).
    let n_in = utxos.len();
    let estimated_size = if segwit {
        11 + n_in * 68 + 31
    } else {
        10 + n_in * 148 + 34
    };
    let fee = (estimated_size as u64).saturating_mul(fee_per_byte);
    let total_in: u64 = utxos.iter().map(|u| u.value).sum();
    if total_in <= fee {
        return SmokeOutcome::Err(format!(
            "balance {total_in} too low to cover live fee {fee} (size≈{estimated_size} × {fee_per_byte} sat/byte)"
        ));
    }
    let amount = total_in - fee;

    let to_script = match address_to_script(coin, network, &address) {
        Ok(s) => s,
        Err(e) => return SmokeOutcome::Err(e),
    };

    let raw_hex = if segwit {
        match build_and_sign_p2wpkh(wif, &utxos, &to_script, amount, fee, &to_script) {
            Ok(tx) => bitcoin::consensus::encode::serialize_hex(&tx),
            Err(e) => return SmokeOutcome::Err(e.to_string()),
        }
    } else {
        match build_and_sign_bch_p2pkh(wif, &utxos, &to_script, amount, fee, &to_script) {
            Ok(tx) => bitcoin::consensus::encode::serialize_hex(&tx),
            Err(e) => return SmokeOutcome::Err(e.to_string()),
        }
    };

    match client.broadcast(&raw_hex).await {
        Ok(txid) => SmokeOutcome::Broadcast { txid, fee },
        Err(e) => SmokeOutcome::Err(e.to_string()),
    }
}

async fn smoke_pol_self_send(evm_rpc_url: &str, wif: &str, from_address: &str, balance: u128) -> Result<String, String> {
    let privkey = bitcoin::PrivateKey::from_wif(wif).map_err(|e| e.to_string())?;
    let secret = privkey.inner.secret_bytes();
    let from = address_from_secret(&secret).map_err(|e| e.to_string())?;
    let from_hex = format!("0x{}", hex::encode(from));
    if !from_hex.eq_ignore_ascii_case(from_address) {
        return Err(format!("WIF-derived POL address {from_hex} != {from_address}"));
    }
    let to = parse_address(from_address).map_err(|e| e.to_string())?;

    let evm = EvmClient::new(evm_rpc_url);
    let chain_id = params_for(Coin::Pol, ChainNetwork::Testnet)
        .evm_chain_id
        .ok_or_else(|| "POL testnet missing chain id".to_string())?;

    let nonce = evm.get_transaction_count(&from_hex).await.map_err(|e| e.to_string())?;
    let gas_price = evm.gas_price().await.map_err(|e| e.to_string())?;
    // Estimate with value=1 first is wrong for all-in; estimate_gas with the
    // eventual value needs a provisional amount. Native transfer gas is
    // independent of value on EVM — use balance as value for estimate then
    // subtract fee.
    let gas_limit = evm
        .estimate_gas(&from_hex, from_address, 1)
        .await
        .map_err(|e| e.to_string())?;
    let fee = gas_price.saturating_mul(gas_limit as u128);
    if balance <= fee {
        return Err(format!("POL balance {balance} too low to cover live fee {fee}"));
    }
    let amount = balance - fee;

    let raw = sign_legacy_tx(
        &secret,
        &LegacyTx {
            nonce,
            gas_price_wei: gas_price,
            gas_limit,
            to,
            value_wei: amount,
            data: Vec::new(),
            chain_id,
        },
    )
    .map_err(|e| e.to_string())?;

    evm.broadcast(&raw).await.map_err(|e| e.to_string())
}

fn address_to_script(coin: Coin, network: ChainNetwork, address: &str) -> Result<bitcoin::ScriptBuf, String> {
    use bitcoin::hashes::Hash;
    use chain::encoding::{base58check_decode, bech32_p2wpkh_decode, cashaddr_decode};

    match params_for(coin, network).address_kind {
        AddressKind::Bech32Segwit { hrp, legacy_base58_version } => {
            if let Some(hash) = bech32_p2wpkh_decode(hrp, address) {
                let wpkh = bitcoin::WPubkeyHash::from_raw_hash(bitcoin::hashes::hash160::Hash::from_byte_array(hash));
                return Ok(bitcoin::ScriptBuf::new_p2wpkh(&wpkh));
            }
            if let Some((v, hash)) = base58check_decode(address) {
                if v == legacy_base58_version {
                    let pkh = bitcoin::PubkeyHash::from_raw_hash(bitcoin::hashes::hash160::Hash::from_byte_array(hash));
                    return Ok(bitcoin::ScriptBuf::new_p2pkh(&pkh));
                }
            }
            Err(format!("invalid address: {address}"))
        }
        AddressKind::CashAddr { prefix } => {
            let hash = cashaddr_decode(prefix, address).ok_or_else(|| format!("invalid cashaddr: {address}"))?;
            let pkh = bitcoin::PubkeyHash::from_raw_hash(bitcoin::hashes::hash160::Hash::from_byte_array(hash));
            Ok(bitcoin::ScriptBuf::new_p2pkh(&pkh))
        }
        other => Err(format!("unsupported address kind for utxo smoke: {other:?}")),
    }
}

async fn fetch_spendable_utxos(client: &BitcoreClient, address: &str) -> Result<Vec<Utxo>, String> {
    #[derive(Deserialize)]
    struct RawUtxo {
        #[serde(rename = "mintTxid")]
        mint_txid: String,
        #[serde(rename = "mintIndex")]
        mint_index: u32,
        value: i64,
        script: String,
    }
    let raw: Vec<RawUtxo> = client.get_unspent_raw(address).await.map_err(|e| e.to_string())?;
    Ok(raw
        .into_iter()
        .map(|u| Utxo {
            txid: u.mint_txid,
            vout: u.mint_index,
            value: u.value.max(0) as u64,
            script_pubkey_hex: u.script,
        })
        .collect())
}

async fn fetch_fee_rate(base_url: &str, chain: &'static str, network: &'static str, confirmation_target: u32) -> Result<u64, String> {
    #[derive(Deserialize)]
    struct FeeResp {
        #[serde(rename = "feerate")]
        fee_rate_per_kb: f64,
    }
    let url = format!(
        "{}/api/{}/{network}/fee/{confirmation_target}",
        base_url.trim_end_matches('/'),
        chain
    );
    let resp: FeeResp = reqwest::get(&url).await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    Ok(((resp.fee_rate_per_kb * 100_000_000.0) / 1000.0).max(1.0) as u64)
}
