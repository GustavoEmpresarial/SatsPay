//! Boot-time loading of wallet key material — ciphertext at rest, plaintext
//! only in RAM, and only in the signer process (worker).
//!
//! Every signer secret `NAME` is read from, in order:
//! - `NAME_ENC_FILE` — path to a file holding the AES-GCM ciphertext
//! - `NAME_ENC` — the ciphertext itself
//! - `NAME` — plaintext; accepted **only outside production**
//!
//! Plaintext `NAME` is removed from the process env once read. That keeps it
//! out of child processes, but `/proc/<pid>/environ` still shows the original
//! env — which is why production refuses plaintext instead of scrubbing it.
//!
//! The internet-facing api-server must hold none of this: it calls
//! [`assert_no_signer_env`] at boot.

use crate::secrets::{CryptoError, SecretsService};
use std::fmt;
use thiserror::Error;
use zeroize::Zeroize;

/// A secret that never prints and is zeroed when dropped.
#[derive(Clone)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The only way to read the value. Callers must not log or persist it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// One signer secret: its env name and the AES-GCM AAD that binds its
/// ciphertext to that name (a `DEPOSIT_MNEMONIC_ENC` cannot be replayed as
/// `HOT_MNEMONIC_ENC`).
#[derive(Clone, Copy, Debug)]
pub struct SecretSpec {
    pub name: &'static str,
    pub aad: &'static [u8],
}

/// AAD kept from the original `hot_mnemonic` module so existing
/// `HOT_MNEMONIC_ENC` values still decrypt.
pub const HOT_MNEMONIC_AAD: &[u8] = b"bitcosats:hot_mnemonic:v1";

pub const HOT_MNEMONIC: SecretSpec = SecretSpec { name: "HOT_MNEMONIC", aad: HOT_MNEMONIC_AAD };
pub const DEPOSIT_MNEMONIC: SecretSpec = SecretSpec { name: "DEPOSIT_MNEMONIC", aad: b"bitcosats:deposit_mnemonic:v1" };
pub const HOT_WALLET_WIF: SecretSpec = SecretSpec { name: "HOT_WALLET_WIF", aad: b"bitcosats:hot_wallet_wif:v1" };
pub const HOT_WALLET_PRIVATE_KEY: SecretSpec =
    SecretSpec { name: "HOT_WALLET_PRIVATE_KEY", aad: b"bitcosats:hot_wallet_private_key:v1" };
pub const POL_HOT_WALLET_KEY: SecretSpec = SecretSpec { name: "POL_HOT_WALLET_KEY", aad: b"bitcosats:pol_hot_wallet_key:v1" };

/// Single hot key, first match wins — same precedence `ChainRegistry` has always used.
pub const HOT_WALLET_KEY_SPECS: [SecretSpec; 3] = [HOT_WALLET_WIF, HOT_WALLET_PRIVATE_KEY, POL_HOT_WALLET_KEY];

pub const ALL_SIGNER_SPECS: [SecretSpec; 5] =
    [HOT_MNEMONIC, DEPOSIT_MNEMONIC, HOT_WALLET_WIF, HOT_WALLET_PRIVATE_KEY, POL_HOT_WALLET_KEY];

#[derive(Debug, Error)]
pub enum SecretBootstrapError {
    #[error("{name} plaintext is forbidden when NODE_ENV=production; set {name}_ENC")]
    PlaintextForbidden { name: &'static str },
    #[error("failed to decrypt {name}_ENC: {source}")]
    Decrypt { name: &'static str, source: CryptoError },
    #[error("cannot read {var}: {reason}")]
    File { var: String, reason: String },
    #[error("signer key material must not reach the api-server: {0}")]
    SignerSecretInApi(String),
}

impl SecretBootstrapError {
    /// Stable code for logs / alerting. Every variant is FATAL at boot.
    pub fn code(&self) -> &'static str {
        match self {
            Self::PlaintextForbidden { .. } => "SECRET_PLAINTEXT_FORBIDDEN",
            Self::Decrypt { .. } => "SECRET_DECRYPT_FAILED",
            Self::File { .. } => "SECRET_FILE_UNREADABLE",
            Self::SignerSecretInApi(_) => "SIGNER_SECRET_IN_API",
        }
    }
}

/// Decrypted signer key material. Only the worker should ever build this.
#[derive(Clone, Debug, Default)]
pub struct SignerSecrets {
    pub hot_mnemonic: Option<SecretValue>,
    pub deposit_mnemonic: Option<SecretValue>,
    /// `HOT_WALLET_WIF` → `HOT_WALLET_PRIVATE_KEY` → `POL_HOT_WALLET_KEY`.
    pub hot_wallet_key: Option<SecretValue>,
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `NAME_FILE` (preferred) or `NAME`. For non-wallet secrets too, e.g.
/// `ENCRYPTION_KEY_FILE=/run/secrets/encryption_key`.
pub fn read_env_or_file(name: &str) -> Result<Option<String>, SecretBootstrapError> {
    read_env_or_file_with(name, &|k| std::env::var(k).ok())
}

fn read_env_or_file_with(name: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<Option<String>, SecretBootstrapError> {
    let file_var = format!("{name}_FILE");
    if let Some(path) = non_empty(env(&file_var)) {
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| SecretBootstrapError::File { var: file_var, reason: e.kind().to_string() })?;
        return Ok(non_empty(Some(raw)));
    }
    Ok(non_empty(env(name)))
}

/// Pure core of [`load_secret`]: `env` is injected so tests never touch the
/// process environment. Returns the value and whether plaintext was used.
fn load_secret_with(
    secrets: &SecretsService,
    spec: SecretSpec,
    is_production: bool,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(Option<SecretValue>, bool), SecretBootstrapError> {
    let enc_name = format!("{}_ENC", spec.name);
    if let Some(ct) = read_env_or_file_with(&enc_name, env)? {
        let plain = secrets
            .decrypt_with_aad(&ct, spec.aad)
            .map_err(|source| SecretBootstrapError::Decrypt { name: spec.name, source })?;
        return Ok((Some(SecretValue::new(plain)), false));
    }
    match non_empty(env(spec.name)) {
        Some(_) if is_production => Err(SecretBootstrapError::PlaintextForbidden { name: spec.name }),
        Some(plain) => Ok((Some(SecretValue::new(plain)), true)),
        None => Ok((None, false)),
    }
}

fn is_production() -> bool {
    std::env::var("NODE_ENV").is_ok_and(|v| v.eq_ignore_ascii_case("production"))
}

/// Loads one signer secret from the process env and removes its plaintext var.
pub fn load_secret(secrets: &SecretsService, spec: SecretSpec) -> Result<Option<SecretValue>, SecretBootstrapError> {
    let prod = is_production();
    let result = load_secret_with(secrets, spec, prod, &|k| std::env::var(k).ok());
    if std::env::var_os(spec.name).is_some() {
        std::env::remove_var(spec.name);
    }
    let (value, from_plaintext) = result?;
    if from_plaintext {
        tracing::warn!(
            code = "SECRET_PLAINTEXT_DEV",
            secret = spec.name,
            "plaintext signer secret accepted outside production — encrypt it before deploying"
        );
    }
    Ok(value)
}

/// Loads every signer secret. Worker only.
pub fn bootstrap_signer_secrets(secrets: &SecretsService) -> Result<SignerSecrets, SecretBootstrapError> {
    let hot_mnemonic = load_secret(secrets, HOT_MNEMONIC)?;
    let deposit_mnemonic = load_secret(secrets, DEPOSIT_MNEMONIC)?;
    let mut hot_wallet_key = None;
    for spec in HOT_WALLET_KEY_SPECS {
        // Load all three so every plaintext var is validated and scrubbed.
        let v = load_secret(secrets, spec)?;
        if hot_wallet_key.is_none() {
            hot_wallet_key = v;
        }
    }
    Ok(SignerSecrets { hot_mnemonic, deposit_mnemonic, hot_wallet_key })
}

/// Names of signer vars (plaintext, `_ENC` or `_ENC_FILE`) present in `env`.
fn signer_vars_present(env: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let mut found = Vec::new();
    for spec in ALL_SIGNER_SPECS {
        for var in [spec.name.to_string(), format!("{}_ENC", spec.name), format!("{}_ENC_FILE", spec.name)] {
            if non_empty(env(&var)).is_some() {
                found.push(var);
            }
        }
    }
    found
}

/// api-server boot guard. Production: any signer var in env is FATAL.
/// Elsewhere: the vars are removed with a warning so a shared dev `.env`
/// keeps working.
pub fn assert_no_signer_env() -> Result<(), SecretBootstrapError> {
    let found = signer_vars_present(&|k| std::env::var(k).ok());
    if found.is_empty() {
        return Ok(());
    }
    if is_production() {
        return Err(SecretBootstrapError::SignerSecretInApi(found.join(", ")));
    }
    for var in &found {
        std::env::remove_var(var);
    }
    tracing::warn!(code = "SIGNER_SECRET_IN_API", vars = %found.join(", "), "signer vars ignored by api-server");
    Ok(())
}

/// Encrypts a secret for storage as `NAME_ENC` (operator helper / tests).
pub fn encrypt_secret(secrets: &SecretsService, spec: SecretSpec, value: &str) -> String {
    secrets.encrypt_with_aad(value.trim(), spec.aad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    fn svc() -> SecretsService {
        SecretsService::from_hex(KEY).unwrap()
    }

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn production_rejects_plaintext_for_every_signer_secret() {
        for spec in ALL_SIGNER_SPECS {
            let env = env_of(&[(spec.name, "secret-value")]);
            let err = load_secret_with(&svc(), spec, true, &env).unwrap_err();
            assert_eq!(err.code(), "SECRET_PLAINTEXT_FORBIDDEN", "{}", spec.name);
            assert!(!err.to_string().contains("secret-value"));
        }
    }

    #[test]
    fn development_accepts_plaintext_and_flags_it() {
        let env = env_of(&[("DEPOSIT_MNEMONIC", WORDS)]);
        let (v, plain) = load_secret_with(&svc(), DEPOSIT_MNEMONIC, false, &env).unwrap();
        assert_eq!(v.unwrap().expose(), WORDS);
        assert!(plain);
    }

    #[test]
    fn ciphertext_round_trips_and_wins_over_plaintext() {
        let s = svc();
        let ct = encrypt_secret(&s, DEPOSIT_MNEMONIC, WORDS);
        let env = env_of(&[("DEPOSIT_MNEMONIC_ENC", &ct), ("DEPOSIT_MNEMONIC", "ignored")]);
        let (v, plain) = load_secret_with(&s, DEPOSIT_MNEMONIC, true, &env).unwrap();
        assert_eq!(v.unwrap().expose(), WORDS);
        assert!(!plain);
    }

    #[test]
    fn aad_binds_ciphertext_to_its_name() {
        let s = svc();
        let ct = encrypt_secret(&s, DEPOSIT_MNEMONIC, WORDS);
        let env = env_of(&[("HOT_MNEMONIC_ENC", &ct)]);
        let err = load_secret_with(&s, HOT_MNEMONIC, true, &env).unwrap_err();
        assert_eq!(err.code(), "SECRET_DECRYPT_FAILED");
    }

    #[test]
    fn legacy_hot_mnemonic_ciphertext_still_decrypts() {
        let s = svc();
        let legacy = s.encrypt_with_aad(WORDS, b"bitcosats:hot_mnemonic:v1");
        let env = env_of(&[("HOT_MNEMONIC_ENC", &legacy)]);
        let (v, _) = load_secret_with(&s, HOT_MNEMONIC, true, &env).unwrap();
        assert_eq!(v.unwrap().expose(), WORDS);
    }

    #[test]
    fn enc_file_is_preferred_over_enc_env() {
        let s = svc();
        let dir = std::env::temp_dir().join(format!("bitcosats-secret-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hot_wif.enc");
        std::fs::write(&path, format!("{}\n", encrypt_secret(&s, HOT_WALLET_WIF, "from-file"))).unwrap();
        let env = env_of(&[
            ("HOT_WALLET_WIF_ENC_FILE", path.to_str().unwrap()),
            ("HOT_WALLET_WIF_ENC", &encrypt_secret(&s, HOT_WALLET_WIF, "from-env")),
        ]);
        let (v, _) = load_secret_with(&s, HOT_WALLET_WIF, true, &env).unwrap();
        assert_eq!(v.unwrap().expose(), "from-file");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_file_is_a_typed_error_without_path_contents() {
        let env = env_of(&[("ENCRYPTION_KEY_FILE", "/nonexistent/bitcosats/key")]);
        let err = read_env_or_file_with("ENCRYPTION_KEY", &env).unwrap_err();
        assert_eq!(err.code(), "SECRET_FILE_UNREADABLE");
    }

    #[test]
    fn absent_secret_is_none() {
        let (v, plain) = load_secret_with(&svc(), HOT_MNEMONIC, true, &env_of(&[])).unwrap();
        assert!(v.is_none() && !plain);
    }

    #[test]
    fn debug_never_prints_the_value() {
        let s = SignerSecrets {
            hot_mnemonic: Some(SecretValue::new(WORDS.into())),
            deposit_mnemonic: None,
            hot_wallet_key: Some(SecretValue::new("L1aW4aubDFB7yfras2S1mN3bqg9nwySY8nkoLmJebSLD5BWv3ENZ".into())),
        };
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("abandon") && !dbg.contains("L1aW4"));
        assert!(dbg.contains("[REDACTED]"));
    }

    #[test]
    fn signer_vars_present_lists_names_in_all_forms() {
        let env = env_of(&[("HOT_MNEMONIC_ENC", "x"), ("DEPOSIT_MNEMONIC", "y"), ("POL_HOT_WALLET_KEY_ENC_FILE", "/p"), ("HOT_WALLET_WIF", " ")]);
        let found = signer_vars_present(&env);
        assert_eq!(found, vec!["HOT_MNEMONIC_ENC", "DEPOSIT_MNEMONIC", "POL_HOT_WALLET_KEY_ENC_FILE"]);
        assert!(signer_vars_present(&env_of(&[("CHAIN_DEPOSIT_XPUB", "xpub")])).is_empty());
    }
}
