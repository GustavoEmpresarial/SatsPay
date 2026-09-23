//! Custody split (ADR 0012): the internet-facing api-server holds only
//! **public** wallet data; the worker is the only process with signing keys.
//!
//! Public, from env, shared by both binaries:
//! - `DEPOSIT_XPUB_<COIN>` — account-level xpub (`account_path(coin)`); addresses
//!   are `…/0/{index}`, identical to deriving from `DEPOSIT_MNEMONIC`.
//! - `HOT_ADDRESS_<COIN>` — hot wallet address (swap source/destination, treasury view).
//! - `CHAIN_DEPOSIT_XPUB` — single-xpub fallback, **outside production only**.
//!
//! Private, worker only: [`SignerKeys`], handed in by the caller (decrypted by
//! `crypto::bootstrap_signer_secrets`), never read from env here.
//! [`verify_signer_matches`] makes the worker refuse to boot when the public
//! config the api-server uses disagrees with the keys that can spend.

use crate::hd_wallet::{account_xpub, hot_address_from_mnemonic};
use crate::params::ChainNetwork;
use crate::real_client::hot_wallet_address;
use bitcoin::bip32::Xpub;
use shared::{Coin, COINS};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WalletConfigError {
    #[error("production requires DEPOSIT_XPUB_<COIN> for: {0}")]
    MissingXpub(String),
    #[error("{var} is not a valid xpub")]
    InvalidXpub { var: String },
    #[error("DEPOSIT_XPUB_{coin} does not match DEPOSIT_MNEMONIC — api-server would issue addresses the worker cannot sweep")]
    DepositKeyMismatch { coin: &'static str },
    #[error("HOT_ADDRESS_{coin} does not match the hot key the worker signs with")]
    HotAddressMismatch { coin: &'static str },
}

impl WalletConfigError {
    /// Stable code. Every variant is FATAL at boot (origin BLOCKCHAIN).
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingXpub(_) => "CHAIN_CONFIG_MISSING_XPUB",
            Self::InvalidXpub { .. } => "CHAIN_CONFIG_INVALID_XPUB",
            Self::DepositKeyMismatch { .. } => "CHAIN_DEPOSIT_KEY_MISMATCH",
            Self::HotAddressMismatch { .. } => "HOT_ADDRESS_MISMATCH",
        }
    }
}

/// Signing keys. Built only by the worker; `Debug` is redacted.
#[derive(Clone, Default)]
pub struct SignerKeys {
    pub hot_mnemonic: Option<String>,
    pub deposit_mnemonic: Option<String>,
    /// `HOT_WALLET_WIF` / `HOT_WALLET_PRIVATE_KEY` / `POL_HOT_WALLET_KEY`.
    pub hot_wallet_key: Option<String>,
}

impl std::fmt::Debug for SignerKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignerKeys")
            .field("hot_mnemonic", &self.hot_mnemonic.as_ref().map(|_| "[REDACTED]"))
            .field("deposit_mnemonic", &self.deposit_mnemonic.as_ref().map(|_| "[REDACTED]"))
            .field("hot_wallet_key", &self.hot_wallet_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl SignerKeys {
    /// Hot address as the signer sees it — same precedence `RealChainClient::hot_addr` uses.
    pub fn hot_address(&self, coin: Coin, network: ChainNetwork) -> Option<Result<String, String>> {
        if let Some(m) = &self.hot_mnemonic {
            return Some(hot_address_from_mnemonic(m, coin, network).map_err(|e| e.to_string()));
        }
        self.hot_wallet_key.as_deref().map(|k| hot_wallet_address(coin, network, k))
    }
}

/// Watch-only wallet data. Safe to hold in the api-server.
#[derive(Clone, Debug, Default)]
pub struct PublicWalletConfig {
    deposit_xpubs: HashMap<Coin, String>,
    hot_addresses: HashMap<Coin, String>,
}

/// Coins whose deposit addresses derive from an xpub. SOL (ed25519) has no
/// public derivation — the worker pre-generates a pool instead.
pub fn xpub_coins() -> impl Iterator<Item = Coin> {
    COINS.iter().copied().filter(|c| *c != Coin::Sol)
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl PublicWalletConfig {
    /// Same xpub for every coin — tests, smokes and dev setups.
    pub fn single_xpub(xpub: &str) -> Self {
        Self { deposit_xpubs: xpub_coins().map(|c| (c, xpub.to_string())).collect(), hot_addresses: HashMap::new() }
    }

    pub fn with_hot_address(mut self, coin: Coin, address: &str) -> Self {
        self.hot_addresses.insert(coin, address.to_string());
        self
    }

    pub fn deposit_xpub(&self, coin: Coin) -> Option<&str> {
        self.deposit_xpubs.get(&coin).map(String::as_str)
    }

    pub fn hot_address(&self, coin: Coin) -> Option<&str> {
        self.hot_addresses.get(&coin).map(String::as_str)
    }

    pub fn from_env(production: bool) -> Result<Self, WalletConfigError> {
        Self::from_env_with(production, &|k| std::env::var(k).ok())
    }

    /// `production` = `NODE_ENV=production` with real chain clients. Then every
    /// xpub coin needs its own validated `DEPOSIT_XPUB_<COIN>` and the single
    /// `CHAIN_DEPOSIT_XPUB` is refused. There is no hardcoded fallback: the old
    /// one was not even a valid xpub.
    pub fn from_env_with(production: bool, env: &dyn Fn(&str) -> Option<String>) -> Result<Self, WalletConfigError> {
        let shared_xpub = if production { None } else { non_empty(env("CHAIN_DEPOSIT_XPUB")) };
        let mut deposit_xpubs = HashMap::new();
        let mut missing = Vec::new();
        for coin in xpub_coins() {
            let var = format!("DEPOSIT_XPUB_{}", coin.as_str());
            // Outside production a missing xpub only disables that coin's
            // address generation (`CHAIN_CONFIG_MISSING_XPUB` per request).
            let Some(xpub) = non_empty(env(&var)).or_else(|| shared_xpub.clone()) else {
                missing.push(coin.as_str());
                continue;
            };
            if Xpub::from_str(&xpub).is_err() {
                return Err(WalletConfigError::InvalidXpub { var });
            }
            deposit_xpubs.insert(coin, xpub);
        }
        if production && !missing.is_empty() {
            return Err(WalletConfigError::MissingXpub(missing.join(", ")));
        }
        if !missing.is_empty() {
            tracing::warn!(code = "CHAIN_CONFIG_MISSING_XPUB", coins = %missing.join(", "), "no deposit xpub — address generation disabled for these coins");
        }

        let hot_addresses = COINS
            .iter()
            .filter_map(|&c| non_empty(env(&format!("HOT_ADDRESS_{}", c.as_str()))).map(|a| (c, a)))
            .collect();
        Ok(Self { deposit_xpubs, hot_addresses })
    }
}

/// Worker boot check: the public config (what the api-server hands out) must
/// match the keys that can spend. Any mismatch is FATAL — the api-server would
/// otherwise issue deposit addresses nobody can sweep, or route swap funds to
/// an address the worker does not control.
pub fn verify_signer_matches(
    public: &PublicWalletConfig,
    keys: &SignerKeys,
    network: ChainNetwork,
) -> Result<(), WalletConfigError> {
    if let Some(m) = &keys.deposit_mnemonic {
        for coin in xpub_coins() {
            let expected = account_xpub(m, coin).ok();
            if expected.as_deref() != public.deposit_xpub(coin) {
                return Err(WalletConfigError::DepositKeyMismatch { coin: coin.as_str() });
            }
        }
    }
    for &coin in COINS.iter() {
        let Some(configured) = public.hot_address(coin) else { continue };
        match keys.hot_address(coin, network) {
            Some(Ok(derived)) if derived.eq_ignore_ascii_case(configured) => {}
            Some(_) => return Err(WalletConfigError::HotAddressMismatch { coin: coin.as_str() }),
            None => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::derive_receive_pubkey;

    const WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const OTHER: &str = "legal winner thank year wave sausage worth useful legal winner thank yellow";

    fn env_of(pairs: Vec<(String, String)>) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs.into_iter().collect();
        move |k| map.get(k).cloned()
    }

    fn per_coin_env(mnemonic: &str) -> Vec<(String, String)> {
        xpub_coins().map(|c| (format!("DEPOSIT_XPUB_{}", c.as_str()), account_xpub(mnemonic, c).unwrap())).collect()
    }

    #[test]
    fn production_requires_every_per_coin_xpub() {
        let err = PublicWalletConfig::from_env_with(true, &env_of(vec![("CHAIN_DEPOSIT_XPUB".into(), account_xpub(WORDS, Coin::Btc).unwrap())]))
            .unwrap_err();
        assert_eq!(err.code(), "CHAIN_CONFIG_MISSING_XPUB");
        assert!(!err.to_string().contains("SOL"), "SOL uses the pool, not an xpub");

        let cfg = PublicWalletConfig::from_env_with(true, &env_of(per_coin_env(WORDS))).unwrap();
        assert_eq!(cfg.deposit_xpub(Coin::Btc).unwrap(), account_xpub(WORDS, Coin::Btc).unwrap());
        assert!(cfg.deposit_xpub(Coin::Sol).is_none());
    }

    /// The pre-ADR-0012 hardcoded default: public in the source and not even a valid xpub.
    const OLD_HARDCODED_DEFAULT: &str =
        "xpub661MyMwAqRbcFtXgS5sYJABqqG9YLmC4Q1Rdap9gSE8NqtwybGhePY2gZ29ESFehEdMTxnPTgVCrgbeG7K6AhEnREakCAZDJBgLnGL9ZSuL";

    #[test]
    fn invalid_xpubs_are_rejected_in_any_environment() {
        for bad in [OLD_HARDCODED_DEFAULT, "xpub-not-really"] {
            let mut env = per_coin_env(WORDS);
            env.retain(|(k, _)| k != "DEPOSIT_XPUB_LTC");
            env.push(("DEPOSIT_XPUB_LTC".into(), bad.into()));
            for production in [true, false] {
                let err = PublicWalletConfig::from_env_with(production, &env_of(env.clone())).unwrap_err();
                assert_eq!(err, WalletConfigError::InvalidXpub { var: "DEPOSIT_XPUB_LTC".into() });
            }
        }
    }

    #[test]
    fn development_uses_shared_xpub_or_leaves_coin_unset() {
        let dev = PublicWalletConfig::from_env_with(false, &env_of(vec![])).unwrap();
        assert_eq!(dev.deposit_xpub(Coin::Doge), None);
        let btc = account_xpub(WORDS, Coin::Btc).unwrap();
        let shared = PublicWalletConfig::from_env_with(false, &env_of(vec![("CHAIN_DEPOSIT_XPUB".into(), btc.clone())])).unwrap();
        assert_eq!(shared.deposit_xpub(Coin::Pol), Some(btc.as_str()));
    }

    /// Regression guard for the split: the api-server derives from the xpub,
    /// the worker from the mnemonic — they must be the same addresses.
    #[test]
    fn xpub_path_matches_mnemonic_path_for_every_xpub_coin() {
        for network in [ChainNetwork::Mainnet, ChainNetwork::Testnet] {
            for coin in xpub_coins() {
                let xpub = account_xpub(WORDS, coin).unwrap();
                for index in [0u32, 1, 2, 7, 19, 1000] {
                    let from_mnemonic = crate::hd_wallet::address_from_mnemonic(WORDS, coin, network, index).unwrap();
                    let from_xpub = crate::real_client::address_from_pubkey(coin, network, &derive_receive_pubkey(&xpub, index).unwrap()).unwrap();
                    assert_eq!(from_xpub, from_mnemonic, "{coin:?} {network:?} #{index}");
                }
            }
        }
    }

    #[test]
    fn verify_accepts_matching_config() {
        let keys = SignerKeys { deposit_mnemonic: Some(WORDS.into()), hot_mnemonic: Some(OTHER.into()), hot_wallet_key: None };
        let hot_btc = hot_address_from_mnemonic(OTHER, Coin::Btc, ChainNetwork::Mainnet).unwrap();
        let cfg = PublicWalletConfig::from_env_with(true, &env_of(per_coin_env(WORDS))).unwrap().with_hot_address(Coin::Btc, &hot_btc);
        verify_signer_matches(&cfg, &keys, ChainNetwork::Mainnet).unwrap();
    }

    #[test]
    fn verify_rejects_deposit_and_hot_mismatch() {
        let keys = SignerKeys { deposit_mnemonic: Some(WORDS.into()), hot_mnemonic: Some(OTHER.into()), hot_wallet_key: None };
        let wrong = PublicWalletConfig::from_env_with(true, &env_of(per_coin_env(OTHER))).unwrap();
        assert_eq!(verify_signer_matches(&wrong, &keys, ChainNetwork::Mainnet).unwrap_err().code(), "CHAIN_DEPOSIT_KEY_MISMATCH");

        let hot_of_deposit = hot_address_from_mnemonic(WORDS, Coin::Pol, ChainNetwork::Mainnet).unwrap();
        let cfg = PublicWalletConfig::from_env_with(true, &env_of(per_coin_env(WORDS))).unwrap().with_hot_address(Coin::Pol, &hot_of_deposit);
        assert_eq!(verify_signer_matches(&cfg, &keys, ChainNetwork::Mainnet).unwrap_err().code(), "HOT_ADDRESS_MISMATCH");
    }

    #[test]
    fn signer_keys_debug_is_redacted() {
        let keys = SignerKeys { deposit_mnemonic: Some(WORDS.into()), hot_mnemonic: None, hot_wallet_key: Some("L1secret".into()) };
        let dbg = format!("{keys:?}");
        assert!(!dbg.contains("abandon") && !dbg.contains("L1secret"));
    }
}
