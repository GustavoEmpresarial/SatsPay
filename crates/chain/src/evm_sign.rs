//! EIP-155 legacy transaction signing for Polygon (POL). Builds, RLP-encodes,
//! and secp256k1-signs a native value transfer. Chain id, nonce, gas price
//! and gas limit are all caller-supplied from live RPC — nothing invented
//! here.

use sha3::{Digest, Keccak256};

#[derive(Debug, thiserror::Error)]
pub enum EvmSignError {
    #[error("invalid private key: {0}")]
    InvalidKey(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("rlp/sign error: {0}")]
    Sign(String),
}

/// Fields of an EIP-155 legacy ethereum transaction (pre-EIP-1559). Polygon
/// mainnet still accepts these; EIP-1559 would need tip/cap fields from RPC.
pub struct LegacyTx {
    pub nonce: u64,
    pub gas_price_wei: u128,
    pub gas_limit: u64,
    pub to: [u8; 20],
    pub value_wei: u128,
    pub data: Vec<u8>,
    /// Network chain id (Polygon mainnet = 137) — taken from `params`, never
    /// invented by the signer.
    pub chain_id: u64,
}

/// Signs `tx` with a secp256k1 secret key (32 bytes) and returns the
/// `0x`-prefixed raw transaction hex ready for `eth_sendRawTransaction`.
pub fn sign_legacy_tx(secret_key_bytes: &[u8; 32], tx: &LegacyTx) -> Result<String, EvmSignError> {
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let secret_key = bitcoin::secp256k1::SecretKey::from_slice(secret_key_bytes)
        .map_err(|e| EvmSignError::InvalidKey(e.to_string()))?;

    // Signing payload per EIP-155: RLP([nonce, gasPrice, gas, to, value, data, chainId, 0, 0])
    let signing_list = vec![
        rlp_u64(tx.nonce),
        rlp_u128(tx.gas_price_wei),
        rlp_u64(tx.gas_limit),
        rlp_bytes(&tx.to),
        rlp_u128(tx.value_wei),
        rlp_bytes(&tx.data),
        rlp_u64(tx.chain_id),
        rlp_u64(0),
        rlp_u64(0),
    ];
    let signing_rlp = rlp_list(&signing_list);
    let hash = keccak256(&signing_rlp);

    let msg = bitcoin::secp256k1::Message::from_digest(hash);
    let sig = secp.sign_ecdsa_recoverable(&msg, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&sig_bytes[..32]);
    s.copy_from_slice(&sig_bytes[32..]);

    // EIP-155 v = recovery_id + chain_id * 2 + 35
    let v = recovery_id.to_i32() as u64 + tx.chain_id * 2 + 35;

    let signed_list = vec![
        rlp_u64(tx.nonce),
        rlp_u128(tx.gas_price_wei),
        rlp_u64(tx.gas_limit),
        rlp_bytes(&tx.to),
        rlp_u128(tx.value_wei),
        rlp_bytes(&tx.data),
        rlp_u64(v),
        rlp_bytes(&r),
        rlp_bytes(&s),
    ];
    let raw = rlp_list(&signed_list);
    Ok(format!("0x{}", hex::encode(raw)))
}

/// Parse a `0x`-prefixed or bare 40-hex-char address into 20 bytes.
pub fn parse_address(addr: &str) -> Result<[u8; 20], EvmSignError> {
    let hex_part = addr.strip_prefix("0x").unwrap_or(addr);
    if hex_part.len() != 40 {
        return Err(EvmSignError::InvalidAddress(addr.to_string()));
    }
    let bytes = hex::decode(hex_part).map_err(|e| EvmSignError::InvalidAddress(e.to_string()))?;
    bytes.try_into().map_err(|_| EvmSignError::InvalidAddress(addr.to_string()))
}

/// Derive the Ethereum address for a secp256k1 secret key (uncompressed
/// pubkey → keccak256 → last 20 bytes).
pub fn address_from_secret(secret_key_bytes: &[u8; 32]) -> Result<[u8; 20], EvmSignError> {
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let sk = bitcoin::secp256k1::SecretKey::from_slice(secret_key_bytes)
        .map_err(|e| EvmSignError::InvalidKey(e.to_string()))?;
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let full = pk.serialize_uncompressed(); // 0x04 || X || Y
    let xy: [u8; 64] = full[1..].try_into().expect("65-byte uncompressed");
    Ok(crate::encoding::evm_address_from_uncompressed_pubkey(&xy))
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

// --- Minimal RLP (Ethereum Yellow Paper Appendix B) ---

fn rlp_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    let mut out = rlp_length(data.len(), 0x80);
    out.extend_from_slice(data);
    out
}

fn rlp_u64(n: u64) -> Vec<u8> {
    if n == 0 {
        return vec![0x80]; // empty byte string
    }
    let be = n.to_be_bytes();
    let start = be.iter().position(|&b| b != 0).unwrap_or(7);
    rlp_bytes(&be[start..])
}

fn rlp_u128(n: u128) -> Vec<u8> {
    if n == 0 {
        return vec![0x80];
    }
    let be = n.to_be_bytes();
    let start = be.iter().position(|&b| b != 0).unwrap_or(15);
    rlp_bytes(&be[start..])
}

fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload_len: usize = items.iter().map(Vec::len).sum();
    let mut out = rlp_length(payload_len, 0xc0);
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

fn rlp_length(len: usize, offset: u8) -> Vec<u8> {
    if len < 56 {
        return vec![offset + len as u8];
    }
    let be = (len as u64).to_be_bytes();
    let start = be.iter().position(|&b| b != 0).unwrap_or(7);
    let len_of_len = 8 - start;
    let mut out = vec![offset + 55 + len_of_len as u8];
    out.extend_from_slice(&be[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rlp_u64_zero_is_empty_string() {
        assert_eq!(rlp_u64(0), vec![0x80]);
    }

    #[test]
    fn rlp_encodes_small_and_long_payloads() {
        assert_eq!(rlp_bytes(&[0x00]), vec![0x00]);
        assert_eq!(rlp_u64(1), vec![0x01]);
        assert_eq!(rlp_u128(0), vec![0x80]);
        let long = rlp_bytes(&[0xabu8; 60]);
        assert!(long[0] > 0xb7); // length-of-length form
        let list = rlp_list(&[rlp_u64(0), rlp_bytes(&[1, 2, 3])]);
        assert!(list[0] >= 0xc0);
    }

    #[test]
    fn address_from_secret_matches_known_key_one() {
        let mut sk = [0u8; 32];
        sk[31] = 1;
        let addr = address_from_secret(&sk).unwrap();
        assert_eq!(hex::encode(addr), "7e5f4552091a69125d5dfcb7b8c2659029395bdf");
    }
}
