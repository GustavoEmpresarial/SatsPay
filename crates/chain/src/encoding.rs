//! Real address encoding/decoding per coin — no stubs. Each coin's address
//! format is implemented per its actual spec (base58check, bech32, or
//! CashAddr), verified against known real-world addresses in `tests.rs`.

use bech32::{FromBase32, ToBase32, Variant};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

/// HASH160 = RIPEMD160(SHA256(data)) — the standard pubkey-hash used by
/// every Bitcoin-derived chain's P2PKH/P2WPKH address.
pub fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let ripe = Ripemd160::digest(sha);
    ripe.into()
}

fn sha256d(data: &[u8]) -> [u8; 32] {
    Sha256::digest(Sha256::digest(data)).into()
}

/// Base58Check-encodes `payload` with `version` — used by BTC/LTC/DOGE
/// legacy (P2PKH) addresses.
pub fn base58check_encode(version: u8, payload: &[u8; 20]) -> String {
    let mut buf = Vec::with_capacity(1 + 20 + 4);
    buf.push(version);
    buf.extend_from_slice(payload);
    let checksum = sha256d(&buf);
    buf.extend_from_slice(&checksum[..4]);
    bs58::encode(buf).into_string()
}

/// Decodes and verifies a Base58Check address, returning `(version, hash160)`.
pub fn base58check_decode(s: &str) -> Option<(u8, [u8; 20])> {
    let data = bs58::decode(s).into_vec().ok()?;
    if data.len() != 25 {
        return None;
    }
    let (payload, checksum) = data.split_at(21);
    let expected = sha256d(payload);
    if &expected[..4] != checksum {
        return None;
    }
    let mut hash = [0u8; 20];
    hash.copy_from_slice(&payload[1..]);
    Some((payload[0], hash))
}

/// Bech32-encodes a P2WPKH (segwit v0) address for the given human-readable
/// prefix (`"bc"` for BTC, `"ltc"` for LTC).
pub fn bech32_p2wpkh_encode(hrp: &str, pubkey_hash: &[u8; 20]) -> String {
    let mut data = vec![bech32::u5::try_from_u8(0).unwrap()]; // witness version 0
    data.extend(pubkey_hash.to_base32());
    bech32::encode(hrp, data, Variant::Bech32).expect("valid bech32 encoding")
}

/// Decodes and verifies a bech32 P2WPKH address for `hrp`, returning the
/// 20-byte witness program.
pub fn bech32_p2wpkh_decode(hrp: &str, s: &str) -> Option<[u8; 20]> {
    let (decoded_hrp, data, variant) = bech32::decode(s).ok()?;
    if decoded_hrp != hrp || variant != Variant::Bech32 || data.is_empty() {
        return None;
    }
    if data[0].to_u8() != 0 {
        return None; // only witness v0 (P2WPKH) is generated/accepted here
    }
    let program = Vec::<u8>::from_base32(&data[1..]).ok()?;
    program.try_into().ok()
}

// --- CashAddr (Bitcoin Cash) — BIP-none, spec: https://github.com/bitcoincashorg/bitcoincash.org/blob/master/spec/cashaddr.md ---

const CASHADDR_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

fn cashaddr_polymod(values: &[u8]) -> u64 {
    let generator: [u64; 5] = [0x98f2bc8e61, 0x79b76d99e2, 0xf33e5fb3c4, 0xae2eabe2a8, 0x1e4f43e470];
    let mut chk: u64 = 1;
    for &v in values {
        let top = chk >> 35;
        chk = ((chk & 0x07ffffffff) << 5) ^ (v as u64);
        for (i, g) in generator.iter().enumerate() {
            if (top >> i) & 1 != 0 {
                chk ^= g;
            }
        }
    }
    chk ^ 1
}

fn cashaddr_expand_prefix(prefix: &str) -> Vec<u8> {
    prefix.bytes().map(|b| b & 0x1f).chain(std::iter::once(0)).collect()
}

fn convert_bits(data: &[u8], from_bits: u32, to_bits: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::new();
    let maxv = (1u32 << to_bits) - 1;
    for &value in data {
        acc = (acc << from_bits) | value as u32;
        bits += from_bits;
        while bits >= to_bits {
            bits -= to_bits;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad && bits > 0 {
        out.push(((acc << (to_bits - bits)) & maxv) as u8);
    } else if !pad && (bits >= from_bits || ((acc << (to_bits - bits)) & maxv) != 0) {
        return None;
    }
    Some(out)
}

/// Encodes a P2PKH CashAddr for `prefix` (`"bitcoincash"`), version byte 0x00.
pub fn cashaddr_encode(prefix: &str, pubkey_hash: &[u8; 20]) -> String {
    let mut payload = vec![0u8]; // version byte: P2PKH, 160-bit hash
    payload.extend_from_slice(pubkey_hash);
    let payload_5bit = convert_bits(&payload, 8, 5, true).expect("20-byte hash converts cleanly to 5-bit groups");

    let mut checksum_input = cashaddr_expand_prefix(prefix);
    checksum_input.extend_from_slice(&payload_5bit);
    checksum_input.extend_from_slice(&[0u8; 8]);
    let polymod = cashaddr_polymod(&checksum_input);

    let checksum: Vec<u8> = (0..8).map(|i| ((polymod >> (5 * (7 - i))) & 0x1f) as u8).collect();

    let body: String = payload_5bit.iter().chain(checksum.iter()).map(|&b| CASHADDR_CHARSET[b as usize] as char).collect();
    format!("{prefix}:{body}")
}

/// Decodes and verifies a CashAddr, returning the 20-byte pubkey hash for a
/// P2PKH (version byte 0) address. Accepts both `"prefix:addr"` and bare
/// `"addr"` (prefix assumed) forms, matching how users commonly paste them.
pub fn cashaddr_decode(prefix: &str, s: &str) -> Option<[u8; 20]> {
    let body = s.strip_prefix(&format!("{prefix}:")).unwrap_or(s);
    if !body.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        return None;
    }
    let values: Vec<u8> = body.bytes().map(|c| CASHADDR_CHARSET.iter().position(|&x| x == c)).collect::<Option<Vec<usize>>>()?.into_iter().map(|v| v as u8).collect();
    if values.len() < 8 {
        return None;
    }

    let mut checksum_input = cashaddr_expand_prefix(prefix);
    checksum_input.extend_from_slice(&values);
    if cashaddr_polymod(&checksum_input) != 0 {
        return None;
    }

    let payload_5bit = &values[..values.len() - 8];
    let payload = convert_bits(payload_5bit, 5, 8, false)?;
    if payload.len() != 21 || payload[0] != 0 {
        return None; // only P2PKH (version 0) accepted
    }
    payload[1..].try_into().ok()
}

// --- EVM (POL) — EIP-55 checksummed hex address from a Keccak256(pubkey) ---

/// Derives the 20-byte EVM address from an uncompressed secp256k1 public key
/// (64 bytes, X||Y, no 0x04 prefix): `keccak256(pubkey)[12..]`.
pub fn evm_address_from_uncompressed_pubkey(pubkey_xy: &[u8; 64]) -> [u8; 20] {
    use sha3::{Digest as Sha3Digest, Keccak256};
    let hash = Keccak256::digest(pubkey_xy);
    hash[12..].try_into().unwrap()
}

/// EIP-55 mixed-case checksum encoding.
pub fn eip55_encode(addr: &[u8; 20]) -> String {
    use sha3::{Digest as Sha3Digest, Keccak256};
    let hex_addr = hex::encode(addr);
    let hash = Keccak256::digest(hex_addr.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, c) in hex_addr.chars().enumerate() {
        if c.is_ascii_digit() {
            out.push(c);
        } else {
            let nibble = if i % 2 == 0 { hash[i / 2] >> 4 } else { hash[i / 2] & 0x0f };
            if nibble >= 8 {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c);
            }
        }
    }
    out
}

/// Validates an EIP-55 (or all-lowercase/all-uppercase, per spec) EVM address.
pub fn eip55_validate(s: &str) -> bool {
    let Some(hex_part) = s.strip_prefix("0x") else { return false };
    if hex_part.len() != 40 || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    if hex_part == hex_part.to_lowercase() || hex_part == hex_part.to_uppercase() {
        return true; // no checksum info present — format-valid, per EIP-55
    }
    let Ok(bytes) = hex::decode(hex_part) else { return false };
    let addr: [u8; 20] = match bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    eip55_encode(&addr) == s
}

/// Solana address = base58 of a 32-byte ed25519 pubkey (no checksum).
pub fn solana_address_encode(pubkey: &[u8; 32]) -> String {
    bs58::encode(pubkey).into_string()
}

pub fn solana_address_decode(s: &str) -> Option<[u8; 32]> {
    let data = bs58::decode(s).into_vec().ok()?;
    data.try_into().ok()
}
