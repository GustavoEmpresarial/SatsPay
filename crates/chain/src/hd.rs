//! Real BIP32 HD derivation. The server holds only an extended **public**
//! key (`xpub`) per coin — deriving deposit addresses from it never exposes
//! or requires a private key, so a compromised API/worker process cannot
//! leak spending keys, only watch-only addresses (which are meant to be
//! public anyway).

use bitcoin::bip32::{ChildNumber, Xpub};
use bitcoin::secp256k1::Secp256k1;
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum HdError {
    #[error("invalid extended public key: {0}")]
    InvalidXpub(String),
    #[error("child index {0} is not a valid non-hardened BIP32 index")]
    InvalidIndex(u32),
    #[error("derivation failed: {0}")]
    DerivationFailed(String),
}

/// Derives the child public key at the given **non-hardened** index under
/// `xpub`. Non-hardened is required here — the server never holds the
/// private key, so hardened derivation (which needs it) is impossible from
/// an xpub alone, by design.
pub fn derive_child_pubkey(xpub_str: &str, index: u32) -> Result<bitcoin::secp256k1::PublicKey, HdError> {
    derive_pub_at(xpub_str, &[index])
}

/// Receive address under an account xpub: `{account}/0/{index}`.
/// This is the same key `address_from_mnemonic` spends. A single child
/// (`{account}/{index}`) is a different address — funds sent there cannot
/// be swept by the BIP44 key.
pub fn derive_receive_pubkey(xpub_str: &str, index: u32) -> Result<bitcoin::secp256k1::PublicKey, HdError> {
    derive_pub_at(xpub_str, &[0, index])
}

fn derive_pub_at(xpub_str: &str, indices: &[u32]) -> Result<bitcoin::secp256k1::PublicKey, HdError> {
    let secp = Secp256k1::verification_only();
    let xpub = Xpub::from_str(xpub_str).map_err(|e| HdError::InvalidXpub(e.to_string()))?;
    let mut path = Vec::with_capacity(indices.len());
    for index in indices {
        path.push(ChildNumber::from_normal_idx(*index).map_err(|_| HdError::InvalidIndex(*index))?);
    }
    let child = xpub.derive_pub(&secp, &path).map_err(|e| HdError::DerivationFailed(e.to_string()))?;
    Ok(child.public_key)
}

/// Compressed (33-byte) SEC1 encoding — used for base58check/bech32 P2PKH/P2WPKH hashing.
pub fn compressed_bytes(pubkey: &bitcoin::secp256k1::PublicKey) -> [u8; 33] {
    pubkey.serialize()
}

/// Uncompressed (64-byte, X||Y without the 0x04 prefix) encoding — used for
/// the EVM address derivation (`keccak256(X||Y)`).
pub fn uncompressed_xy_bytes(pubkey: &bitcoin::secp256k1::PublicKey) -> [u8; 64] {
    let full = pubkey.serialize_uncompressed(); // 0x04 || X(32) || Y(32), 65 bytes
    full[1..].try_into().expect("serialize_uncompressed always returns 65 bytes")
}
