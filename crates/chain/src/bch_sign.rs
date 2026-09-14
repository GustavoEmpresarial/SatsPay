//! Bitcoin Cash P2PKH signing with `SIGHASH_ALL | SIGHASH_FORKID` (BIP143-style
//! digest + fork id). BCH rejected BTC legacy sighash at the UAHF; the
//! bitcoin crate's sighash helpers do not cover FORKID, so this module
//! implements the Cash digest per the BCH specification.

use crate::btc_sign::{SignError, Utxo};
use bitcoin::consensus::Encodable;
use bitcoin::hashes::{sha256d, Hash};
use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
use bitcoin::{absolute::LockTime, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};
use bitcoin::transaction::Version;
use std::str::FromStr;

/// `SIGHASH_ALL (0x01) | SIGHASH_FORKID (0x40)` — required by BCH nodes for
/// standard P2PKH spends after the UAHF. The `0x40` bit is what separates
/// this digest from BTC BIP143.
const SIGHASH_ALL_FORKID: u32 = 0x41;

/// Builds and signs a BCH P2PKH (scriptSig) transaction. Inputs must be
/// P2PKH UTXOs controlled by `wif_private_key`. `fee` comes from a live
/// fee-rate query — never a constant baked in here.
pub fn build_and_sign_bch_p2pkh(
    wif_private_key: &str,
    utxos: &[Utxo],
    to_script_pubkey: &ScriptBuf,
    amount: u64,
    fee: u64,
    change_script_pubkey: &ScriptBuf,
) -> Result<Transaction, SignError> {
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
            Ok(TxIn {
                previous_output: OutPoint { txid, vout: u.vout },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            })
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

    let hash_prevouts = hash_prevouts(&tx);
    let hash_sequence = hash_sequence(&tx);
    let hash_outputs = hash_outputs(&tx);

    for (i, utxo) in utxos.iter().enumerate() {
        let script_code = p2pkh_script_code(&prevouts[i].script_pubkey)?;
        let digest = bip143_forkid_digest(
            &tx,
            i,
            &script_code,
            utxo.value,
            SIGHASH_ALL_FORKID,
            &hash_prevouts,
            &hash_sequence,
            &hash_outputs,
        );
        let msg = Message::from_digest(digest);
        let sig = secp.sign_ecdsa(&msg, &secret_key);
        let mut sig_bytes = sig.serialize_der().to_vec();
        sig_bytes.push(SIGHASH_ALL_FORKID as u8);

        let mut script_sig = Vec::new();
        script_sig.push(sig_bytes.len() as u8);
        script_sig.extend_from_slice(&sig_bytes);
        let pk = pubkey.to_bytes();
        script_sig.push(pk.len() as u8);
        script_sig.extend_from_slice(&pk);
        tx.input[i].script_sig = ScriptBuf::from_bytes(script_sig);
    }

    Ok(tx)
}

fn p2pkh_script_code(script_pubkey: &ScriptBuf) -> Result<ScriptBuf, SignError> {
    let bytes = script_pubkey.as_bytes();
    // P2PKH: OP_DUP OP_HASH160 OP_PUSHBYTES_20 <20> OP_EQUALVERIFY OP_CHECKSIG
    if bytes.len() == 25 && bytes[0] == 0x76 && bytes[1] == 0xa9 && bytes[2] == 0x14 && bytes[23] == 0x88 && bytes[24] == 0xac {
        return Ok(script_pubkey.clone());
    }
    Err(SignError::InvalidScript("BCH signer requires P2PKH scriptPubKey".into()))
}

fn hash_prevouts(tx: &Transaction) -> [u8; 32] {
    let mut buf = Vec::new();
    for input in &tx.input {
        input.previous_output.consensus_encode(&mut buf).expect("vec");
    }
    sha256d::Hash::hash(&buf).to_byte_array()
}

fn hash_sequence(tx: &Transaction) -> [u8; 32] {
    let mut buf = Vec::new();
    for input in &tx.input {
        input.sequence.consensus_encode(&mut buf).expect("vec");
    }
    sha256d::Hash::hash(&buf).to_byte_array()
}

fn hash_outputs(tx: &Transaction) -> [u8; 32] {
    let mut buf = Vec::new();
    for output in &tx.output {
        output.consensus_encode(&mut buf).expect("vec");
    }
    sha256d::Hash::hash(&buf).to_byte_array()
}

fn bip143_forkid_digest(
    tx: &Transaction,
    input_index: usize,
    script_code: &ScriptBuf,
    amount: u64,
    sighash_type: u32,
    hash_prevouts: &[u8; 32],
    hash_sequence: &[u8; 32],
    hash_outputs: &[u8; 32],
) -> [u8; 32] {
    let mut buf = Vec::with_capacity(156 + script_code.len());
    tx.version.consensus_encode(&mut buf).expect("vec");
    buf.extend_from_slice(hash_prevouts);
    buf.extend_from_slice(hash_sequence);
    tx.input[input_index].previous_output.consensus_encode(&mut buf).expect("vec");
    script_code.consensus_encode(&mut buf).expect("vec");
    Amount::from_sat(amount).consensus_encode(&mut buf).expect("vec");
    tx.input[input_index].sequence.consensus_encode(&mut buf).expect("vec");
    buf.extend_from_slice(hash_outputs);
    tx.lock_time.consensus_encode(&mut buf).expect("vec");
    buf.extend_from_slice(&sighash_type.to_le_bytes());
    sha256d::Hash::hash(&buf).to_byte_array()
}
