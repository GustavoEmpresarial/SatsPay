//! Offline signing-path smoke test — no network broadcast, no real funds.
//!
//! Proves BTC-family / BCH / POL signing code typechecks and runs:
//! - empty UTXO list → `InsufficientUtxos` (BTC P2WPKH + BCH P2PKH)
//! - POL legacy EIP-155 sign → `0x`-prefixed raw hex with non-empty RLP body
//!
//! Optional env: `HOT_WALLET_WIF` (if unset, a throwaway WIF is used).
//! Run: `cargo run -p chain --example broadcast_sign_smoke`

use bitcoin::hashes::Hash;
use bitcoin::ScriptBuf;
use chain::bch_sign::build_and_sign_bch_p2pkh;
use chain::btc_sign::{build_and_sign_p2wpkh, SignError};
use chain::evm_sign::{sign_legacy_tx, LegacyTx};
use chain::params::params;
use shared::Coin;

fn main() {
    let wif = match std::env::var("HOT_WALLET_WIF") {
        Ok(w) if !w.is_empty() => {
            println!("using HOT_WALLET_WIF from env (signing only — no broadcast)");
            w
        }
        _ => {
            println!("HOT_WALLET_WIF unset — using throwaway WIF for offline sign-path check");
            let sk = bitcoin::secp256k1::SecretKey::from_slice(&[1u8; 32]).expect("nonzero");
            bitcoin::PrivateKey::new(sk, bitcoin::Network::Bitcoin).to_wif()
        }
    };

    let zero_hash = bitcoin::hashes::hash160::Hash::from_byte_array([0u8; 20]);
    let dummy_wpkh = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_raw_hash(zero_hash));
    let dummy_pkh = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(zero_hash));

    // --- BTC family (P2WPKH): empty utxos → InsufficientUtxos ---
    match build_and_sign_p2wpkh(&wif, &[], &dummy_wpkh, 1_000, 200, &dummy_wpkh) {
        Err(SignError::InsufficientUtxos { needed }) => {
            assert_eq!(needed, 1_200);
            println!("BTC P2WPKH path OK (InsufficientUtxos needed={needed})");
        }
        other => panic!("expected InsufficientUtxos for empty BTC utxos, got {other:?}"),
    }

    // --- BCH (P2PKH + FORKID): same empty-utxo assertion ---
    match build_and_sign_bch_p2pkh(&wif, &[], &dummy_pkh, 1_000, 200, &dummy_pkh) {
        Err(SignError::InsufficientUtxos { needed }) => {
            assert_eq!(needed, 1_200);
            println!("BCH P2PKH+FORKID path OK (InsufficientUtxos needed={needed})");
        }
        other => panic!("expected InsufficientUtxos for empty BCH utxos, got {other:?}"),
    }

    // --- POL: sign a throwaway legacy tx (no broadcast) ---
    let chain_id = params(Coin::Pol).evm_chain_id.expect("POL has evm_chain_id");
    assert_eq!(chain_id, 137);
    let sk = [0x11u8; 32];
    let raw = sign_legacy_tx(
        &sk,
        &LegacyTx {
            nonce: 0,
            // Caller-supplied values in production come from eth_gasPrice /
            // eth_estimateGas; here they are arbitrary placeholders for an
            // offline RLP/sign check only.
            gas_price_wei: 1,
            gas_limit: 21_000,
            to: [0x22; 20],
            value_wei: 1,
            data: Vec::new(),
            chain_id,
        },
    )
    .expect("sign_legacy_tx");
    assert!(raw.starts_with("0x"), "raw tx must be 0x-prefixed hex");
    let body = hex::decode(raw.trim_start_matches("0x")).expect("hex");
    assert!(!body.is_empty(), "RLP payload length must be > 0");
    assert!(body[0] >= 0xc0, "signed tx must be an RLP list");
    println!("POL EIP-155 legacy sign OK (raw len={} bytes)", body.len());

    println!("broadcast_sign_smoke: all signing paths OK (no funds sent)");
}
