//! Port 1:1 of legacy `apps/api/src/core/crypto/crypto.ts`.
//!
//! Same wire format (`iv || tag || ciphertext`, base64) and the same HKDF
//! info-string labels, so ciphertext produced by the legacy service remains
//! decryptable here during the migration, given the same `ENCRYPTION_KEY`.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;

const IV_LEN: usize = 12;
const TAG_LEN: usize = 16;
const AES_INFO: &[u8] = b"bitcosats:aes-256-gcm:v1";
const HMAC_INFO: &[u8] = b"bitcosats:hmac-sha256:v1";
/// Dedicated HKDF info for refresh-token fingerprints — never reuse the
/// general API-key HMAC subkey for session tokens.
const REFRESH_HMAC_INFO: &[u8] = b"bitcosats:refresh-token-hmac:v1";
/// Dedicated HKDF info for merchant webhook HMAC — never reuse API-key HMAC.
const WEBHOOK_HMAC_INFO: &[u8] = b"bitcosats:webhook:v1";
const REFRESH_HASH_PREFIX: &str = "v2:";

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("master key must be 32 bytes (64 hex chars)")]
    InvalidMasterKeyLength,
    #[error("invalid base64 payload")]
    InvalidBase64,
    #[error("payload too short to contain iv+tag")]
    PayloadTooShort,
    #[error("decryption failed (bad key or tampered ciphertext)")]
    DecryptionFailed,
}

/// Holds the purpose-separated subkeys derived from the master `ENCRYPTION_KEY`,
/// so the same bytes are never used for both AES-GCM encryption and HMAC
/// fingerprints (mirrors the legacy `deriveKey` split).
pub struct SecretsService {
    aes_key: [u8; 32],
    hmac_key: [u8; 32],
    refresh_hmac_key: [u8; 32],
    webhook_hmac_key: [u8; 32],
}

impl SecretsService {
    /// `master_key_hex` must be a 64-char hex string (32 raw bytes), same as
    /// the legacy `env.ENCRYPTION_KEY`.
    pub fn from_hex(master_key_hex: &str) -> Result<Self, CryptoError> {
        let master = hex::decode(master_key_hex).map_err(|_| CryptoError::InvalidMasterKeyLength)?;
        if master.len() != 32 {
            return Err(CryptoError::InvalidMasterKeyLength);
        }
        let aes_key = derive_key(&master, AES_INFO);
        let hmac_key = derive_key(&master, HMAC_INFO);
        let refresh_hmac_key = derive_key(&master, REFRESH_HMAC_INFO);
        let webhook_hmac_key = derive_key(&master, WEBHOOK_HMAC_INFO);
        Ok(Self {
            aes_key,
            hmac_key,
            refresh_hmac_key,
            webhook_hmac_key,
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> String {
        self.encrypt_with_aad(plaintext, &[])
    }

    /// AES-GCM seal bound to `aad` (e.g. `api_key:{id}`). New writes should
    /// always pass a context string so ciphertexts cannot be swapped across rows.
    pub fn encrypt_with_aad(&self, plaintext: &str, aad: &[u8]) -> String {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.aes_key));
        let mut iv = [0u8; IV_LEN];
        rand::thread_rng().fill_bytes(&mut iv);
        let nonce = Nonce::from_slice(&iv);
        // aes-gcm returns ciphertext with the tag appended; legacy format is
        // iv || tag || ct, so split and reorder to match exactly.
        let ct_with_tag = cipher
            .encrypt(nonce, Payload { msg: plaintext.as_bytes(), aad })
            .expect("AES-GCM encryption is infallible for valid key/nonce sizes");
        let (ct, tag) = ct_with_tag.split_at(ct_with_tag.len() - TAG_LEN);

        let mut out = Vec::with_capacity(IV_LEN + TAG_LEN + ct.len());
        out.extend_from_slice(&iv);
        out.extend_from_slice(tag);
        out.extend_from_slice(ct);
        base64::engine::general_purpose::STANDARD.encode(out)
    }

    pub fn decrypt(&self, payload_b64: &str) -> Result<String, CryptoError> {
        self.open(payload_b64, &[])
    }

    /// Open a ciphertext with `aad`. If that fails and `aad` is non-empty,
    /// retry with empty AAD so pre-AAD rows (`HOT_MNEMONIC_ENC`, old `key_enc`)
    /// still decrypt.
    pub fn decrypt_with_aad(&self, payload_b64: &str, aad: &[u8]) -> Result<String, CryptoError> {
        match self.open(payload_b64, aad) {
            Ok(plain) => Ok(plain),
            Err(_) if !aad.is_empty() => self.open(payload_b64, &[]),
            Err(e) => Err(e),
        }
    }

    fn open(&self, payload_b64: &str, aad: &[u8]) -> Result<String, CryptoError> {
        let buf = base64::engine::general_purpose::STANDARD
            .decode(payload_b64)
            .map_err(|_| CryptoError::InvalidBase64)?;
        if buf.len() < IV_LEN + TAG_LEN {
            return Err(CryptoError::PayloadTooShort);
        }
        let (iv, rest) = buf.split_at(IV_LEN);
        let (tag, ct) = rest.split_at(TAG_LEN);

        // aes-gcm expects ct || tag for decrypt, legacy stores iv || tag || ct.
        let mut ct_with_tag = Vec::with_capacity(ct.len() + TAG_LEN);
        ct_with_tag.extend_from_slice(ct);
        ct_with_tag.extend_from_slice(tag);

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.aes_key));
        let nonce = Nonce::from_slice(iv);
        let plaintext = cipher
            .decrypt(nonce, Payload { msg: &ct_with_tag, aad })
            .map_err(|_| CryptoError::DecryptionFailed)?;
        String::from_utf8(plaintext).map_err(|_| CryptoError::DecryptionFailed)
    }

    /// Keyed hash (HMAC-SHA256) using the derived HMAC subkey. Use for
    /// API-key fingerprints: unlike a bare SHA-256, an attacker who sees a
    /// key prefix cannot precompute a rainbow table or verify guesses offline
    /// without the server secret.
    pub fn hmac_hex(&self, input: &str) -> String {
        Self::hmac_hex_with_key(&self.hmac_key, input)
    }

    /// Constant-time verify that `input` matches a previously stored `hmac_hex`.
    pub fn verify_hmac_hex(&self, input: &str, expected_hex: &str) -> bool {
        crate::ct::ct_eq_str(&self.hmac_hex(input), expected_hex)
    }

    /// Fingerprint a refresh token for DB storage.
    /// Format: `v2:` + hex(HMAC-SHA256_refresh(token)).
    pub fn refresh_token_hash(&self, refresh_token: &str) -> String {
        format!(
            "{REFRESH_HASH_PREFIX}{}",
            Self::hmac_hex_with_key(&self.refresh_hmac_key, refresh_token)
        )
    }

    /// Candidate hashes to look up for a presented refresh token.
    /// Includes legacy bare SHA-256 so sessions minted before the keyed
    /// upgrade can still rotate once into `v2:`.
    pub fn refresh_token_lookup_hashes(&self, refresh_token: &str) -> [String; 2] {
        [
            self.refresh_token_hash(refresh_token),
            sha256_hex(refresh_token),
        ]
    }

    /// Per-merchant webhook signing secret (hex). Same merchant → same secret;
    /// different merchants → different secrets. Not persisted — derived from
    /// `ENCRYPTION_KEY`.
    pub fn webhook_signing_secret(&self, merchant_id: &str) -> String {
        Self::hmac_hex_with_key(&self.webhook_hmac_key, &format!("merchant:{merchant_id}"))
    }

    /// HMAC-SHA256 hex of `payload` keyed by the merchant's webhook secret.
    pub fn sign_webhook_payload(&self, merchant_id: &str, payload: &str) -> String {
        let secret = self.webhook_signing_secret(merchant_id);
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Constant-time check that `payload` matches `expected_hex` for this merchant.
    pub fn verify_webhook_payload(&self, merchant_id: &str, payload: &str, expected_hex: &str) -> bool {
        crate::ct::ct_eq_str(&self.sign_webhook_payload(merchant_id, payload), expected_hex)
    }

    fn hmac_hex_with_key(key: &[u8; 32], input: &str) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(input.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

fn derive_key(master: &[u8], info: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(&[]), master);
    let mut out = [0u8; 32];
    hk.expand(info, &mut out).expect("32 bytes is a valid HKDF-SHA256 output length");
    out
}
