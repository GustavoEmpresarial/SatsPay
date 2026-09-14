//! Real transaction construction + signing for BTC — standard P2WPKH
//! (segwit v0), single-key hot wallet. Used as-is for LTC and DOGE too:
//! their raw transaction *format* is byte-identical to Bitcoin's (same
//! version/locktime/witness serialization), only the address *encoding*
//! differs (already handled in `encoding.rs`/`params.rs`), so the same
//! signer works for all three. BCH is NOT compatible (needs SIGHASH_FORKID,
//! which this signer does not implement — see `real_client.rs`).

use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::transaction::Version;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("invalid private key: {0}")]
    InvalidKey(String),
    #[error("no UTXOs available to cover {needed} (smallest unit)")]
    InsufficientUtxos { needed: u128 },
    #[error("invalid script: {0}")]
    InvalidScript(String),
    #[error("invalid txid: {0}")]
    InvalidTxid(String),
}

pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    /// scriptPubKey hex, as returned by the indexer — needed to sign against
    /// the exact output being spent.
    pub script_pubkey_hex: String,
}

/// Builds and signs a single-output-plus-change P2WPKH transaction spending
/// `utxos` (assumed to all be controlled by `wif_private_key`), sending
/// `amount` to `to_address_script` and the remainder (minus `fee`) back to
/// the hot wallet's own change address.
///
/// `fee` is caller-supplied (from a real fee-rate query, e.g.
/// `/api/{chain}/{network}/fee/{target}` on the same Bitcore API) — never a
/// hardcoded constant here.
pub fn build_and_sign_p2wpkh(wif_private_key: &str, utxos: &[Utxo], to_script_pubkey: &ScriptBuf, amount: u64, fee: u64, change_script_pubkey: &ScriptBuf) -> Result<Transaction, SignError> {
    let secp = Secp256k1::new();
    let privkey = bitcoin::PrivateKey::from_wif(wif_private_key).map_err(|e| SignError::InvalidKey(e.to_string()))?;
    let secret_key: SecretKey = privkey.inner;
    let pubkey = bitcoin::PublicKey::from_private_key(&secp, &privkey);

    let total_in: u64 = utxos.iter().map(|u| u.value).sum();
    let needed = amount as u128 + fee as u128;
    if (total_in as u128) < needed {
        return Err(SignError::InsufficientUtxos { needed });
    }
    let change = total_in - amount - fee;

    let inputs: Vec<TxIn> = utxos
        .iter()
        .map(|u| {
            let txid = Txid::from_str(&u.txid).map_err(|e| SignError::InvalidTxid(e.to_string()))?;
            Ok(TxIn { previous_output: OutPoint { txid, vout: u.vout }, script_sig: ScriptBuf::new(), sequence: Sequence::ENABLE_RBF_NO_LOCKTIME, witness: Witness::new() })
        })
        .collect::<Result<_, SignError>>()?;

    let mut outputs = vec![TxOut { value: Amount::from_sat(amount), script_pubkey: to_script_pubkey.clone() }];
    if change > 0 {
        outputs.push(TxOut { value: Amount::from_sat(change), script_pubkey: change_script_pubkey.clone() });
    }

    let mut tx = Transaction { version: Version::TWO, lock_time: LockTime::ZERO, input: inputs, output: outputs };

    let prevouts: Vec<TxOut> = utxos
        .iter()
        .map(|u| {
            let script = ScriptBuf::from_hex(&u.script_pubkey_hex).map_err(|e| SignError::InvalidScript(e.to_string()))?;
            Ok(TxOut { value: Amount::from_sat(u.value), script_pubkey: script })
        })
        .collect::<Result<_, SignError>>()?;

    let mut cache = SighashCache::new(tx.clone());
    for (i, utxo) in utxos.iter().enumerate() {
        let sighash = cache
            .p2wpkh_signature_hash(i, &prevouts[i].script_pubkey, Amount::from_sat(utxo.value), EcdsaSighashType::All)
            .map_err(|e| SignError::InvalidScript(e.to_string()))?;
        let msg = Message::from_digest(sighash.to_byte_array());
        let sig = secp.sign_ecdsa(&msg, &secret_key);
        let mut sig_bytes = sig.serialize_der().to_vec();
        sig_bytes.push(EcdsaSighashType::All as u8);
        let witness = Witness::from_slice(&[sig_bytes, pubkey.to_bytes()]);
        tx.input[i].witness = witness;
    }

    Ok(tx)
}
