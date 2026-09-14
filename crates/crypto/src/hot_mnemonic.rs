//! Hot-wallet mnemonic bootstrap — ciphertext at rest, plaintext only in RAM.
//!
//! Production (`NODE_ENV=production`):
//! - Requires `HOT_MNEMONIC_ENC` (AES-GCM via [`SecretsService`])
//! - Refuses bare `HOT_MNEMONIC` in the process environment at boot
//!
//! Development / test:
//! - Accepts plaintext `HOT_MNEMONIC` with a loud warning
//! - Still prefers `HOT_MNEMONIC_ENC` when both are set

use crate::secrets::{CryptoError, SecretsService};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HotMnemonicError {
    #[error("HOT_MNEMONIC plaintext is forbidden when NODE_ENV=production; set HOT_MNEMONIC_ENC")]
    PlaintextForbiddenInProduction,
    #[error("failed to decrypt HOT_MNEMONIC_ENC: {0}")]
    Decrypt(#[from] CryptoError),
}

/// Resolve the hot mnemonic for this process. Caller should hold the result
/// in `Arc<str>` / `ChainRegistry` and **not** re-export it via logs.
pub fn bootstrap_hot_mnemonic(secrets: &SecretsService) -> Result<Option<String>, HotMnemonicError> {
    let node_env = std::env::var("NODE_ENV").unwrap_or_else(|_| "development".to_string());
    let is_production = node_env.eq_ignore_ascii_case("production");

    let enc = std::env::var("HOT_MNEMONIC_ENC")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let plain = std::env::var("HOT_MNEMONIC")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(ct) = enc {
        if plain.is_some() {
            // Scrub operator-supplied plaintext so /proc/*/environ cannot leak it.
            std::env::remove_var("HOT_MNEMONIC");
            tracing::warn!("HOT_MNEMONIC plaintext ignored — using HOT_MNEMONIC_ENC only");
        }
        let mnemonic = secrets.decrypt(&ct)?;
        if mnemonic.split_whitespace().count() < 12 {
            tracing::warn!("HOT_MNEMONIC_ENC decrypted but word count looks short");
        }
        return Ok(Some(mnemonic));
    }

    if let Some(m) = plain {
        if is_production {
            return Err(HotMnemonicError::PlaintextForbiddenInProduction);
        }
        tracing::warn!(
            "HOT_MNEMONIC is plaintext in env — encrypt with SecretsService and set HOT_MNEMONIC_ENC before production"
        );
        return Ok(Some(m));
    }

    Ok(None)
}

/// Encrypt a mnemonic for storage as `HOT_MNEMONIC_ENC` (ops helper / tests).
pub fn encrypt_hot_mnemonic(secrets: &SecretsService, mnemonic: &str) -> String {
    secrets.encrypt(mnemonic.trim())
}
