//! Two-wallet HD model:
//! - **deposit** mnemonic → unique per-user receive addresses (`…/0/{index}`)
//! - **hot** mnemonic → single spend key per coin (`…/0/0`) that pays withdrawals
//!   and receives sweeps.
//!
//! One BIP-39 seed covers every secp256k1 coin (BTC/LTC/DOGE/BCH/DGB/POL/USDT/USDC/PEPE).
//! Solana uses HMAC-SHA256 of the same seed (ed25519), not secp256k1.

use crate::encoding::{
    base58check_encode, bech32_p2wpkh_encode, cashaddr_encode, eip55_encode, evm_address_from_uncompressed_pubkey, hash160,
    zcash_t1_encode,
};
use crate::hd::{compressed_bytes, uncompressed_xy_bytes};
use crate::params::{params_for, AddressKind, ChainNetwork};
use crate::sol_client::{derive_sol_secret, sol_address_from_secret, sol_master_from_hot_key};
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv, Xpub};
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::Network;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand::RngCore;
use sha2::{Digest, Sha256, Sha512};
use shared::Coin;
use std::str::FromStr;

const WORDLIST: &str = include_str!("../resources/bip39_english.txt");

#[derive(Debug, thiserror::Error)]
pub enum HdWalletError {
    #[error("{0}")]
    Msg(String),
}

impl From<String> for HdWalletError {
    fn from(s: String) -> Self {
        Self::Msg(s)
    }
}

pub fn generate_mnemonic() -> String {
    let mut entropy = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut entropy);
    entropy_to_mnemonic(&entropy)
}

fn wordlist() -> Vec<&'static str> {
    WORDLIST.lines().filter(|l| !l.is_empty()).collect()
}

fn entropy_to_mnemonic(entropy: &[u8; 32]) -> String {
    let hash = Sha256::digest(entropy);
    let mut bits = Vec::with_capacity(264);
    for b in entropy {
        for i in (0..8).rev() {
            bits.push((b >> i) & 1);
        }
    }
    for i in (0..8).rev() {
        bits.push((hash[0] >> i) & 1);
    }
    let words = wordlist();
    let mut out = Vec::with_capacity(24);
    for chunk in bits.chunks(11) {
        let mut idx = 0u16;
        for (i, bit) in chunk.iter().enumerate() {
            idx |= (*bit as u16) << (10 - i);
        }
        out.push(words[idx as usize]);
    }
    out.join(" ")
}

pub fn mnemonic_to_seed(mnemonic: &str) -> [u8; 64] {
    let mut seed = [0u8; 64];
    pbkdf2::<Hmac<Sha512>>(mnemonic.as_bytes(), b"mnemonic", 2048, &mut seed).expect("pbkdf2");
    seed
}

pub fn master_xpriv(mnemonic: &str) -> Result<Xpriv, HdWalletError> {
    let seed = mnemonic_to_seed(mnemonic);
    Xpriv::new_master(Network::Bitcoin, &seed).map_err(|e| HdWalletError::Msg(e.to_string()))
}

/// BIP-44/84 account path (up to `account'`). Address is `{account}/0/{index}`.
pub fn account_path(coin: Coin) -> &'static str {
    match coin {
        Coin::Btc => "m/84'/0'/0'",
        Coin::Ltc => "m/84'/2'/0'",
        Coin::Doge => "m/44'/3'/0'",
        Coin::Bch => "m/44'/145'/0'",
        Coin::Dgb => "m/84'/20'/0'",
        Coin::Pol | Coin::Usdt | Coin::Usdc | Coin::Pepe => "m/44'/60'/0'",
        Coin::Sol => "m/44'/501'/0'",
        Coin::Zer => "m/44'/323'/0'",
    }
}

fn derive_secp_secret(mnemonic: &str, coin: Coin, index: u32) -> Result<[u8; 32], HdWalletError> {
    derive_secp_children(mnemonic, coin, &[0, index])
}

/// `{account}/{index}` — the key that matches `derive_child_pubkey(account_xpub, index)`.
/// Issued by mistake when the API had only the xpub. Not the BIP44 receive path.
pub fn secret_from_account_index(mnemonic: &str, coin: Coin, index: u32) -> Result<[u8; 32], HdWalletError> {
    if coin == Coin::Sol {
        return Err(HdWalletError::Msg("sol has no secp account index".into()));
    }
    derive_secp_children(mnemonic, coin, &[index])
}

fn derive_secp_children(mnemonic: &str, coin: Coin, children: &[u32]) -> Result<[u8; 32], HdWalletError> {
    let secp = Secp256k1::new();
    let master = master_xpriv(mnemonic)?;
    let account = master
        .derive_priv(&secp, &DerivationPath::from_str(account_path(coin)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let path: Vec<ChildNumber> = children
        .iter()
        .map(|index| ChildNumber::from_normal_idx(*index).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let child = account.derive_priv(&secp, &path).map_err(|e| e.to_string())?;
    Ok(child.private_key.secret_bytes())
}

pub fn derive_sol_secret_from_mnemonic(mnemonic: &str, domain: &[u8], index: u32) -> [u8; 32] {
    let master = sol_master_from_hot_key(mnemonic);
    derive_sol_secret(&master, domain, index)
}

pub fn address_from_mnemonic(mnemonic: &str, coin: Coin, network: ChainNetwork, index: u32) -> Result<String, HdWalletError> {
    if coin == Coin::Sol {
        let secret = derive_sol_secret_from_mnemonic(mnemonic, b"bitcosats-sol-deposit", index);
        return Ok(sol_address_from_secret(&secret));
    }
    let secret = derive_secp_secret(mnemonic, coin, index)?;
    address_from_secret_bytes(coin, network, &secret)
}

pub fn secret_from_mnemonic(mnemonic: &str, coin: Coin, index: u32) -> Result<[u8; 32], HdWalletError> {
    if coin == Coin::Sol {
        return Ok(derive_sol_secret_from_mnemonic(mnemonic, b"bitcosats-sol-deposit", index));
    }
    derive_secp_secret(mnemonic, coin, index)
}

/// Hot wallet is always index 0 of the hot mnemonic. SOL uses the hot domain.
pub fn hot_secret_from_mnemonic(mnemonic: &str, coin: Coin) -> Result<[u8; 32], HdWalletError> {
    if coin == Coin::Sol {
        return Ok(derive_sol_secret_from_mnemonic(mnemonic, b"bitcosats-sol-hot", 0));
    }
    derive_secp_secret(mnemonic, coin, 0)
}

pub fn hot_address_from_mnemonic(mnemonic: &str, coin: Coin, network: ChainNetwork) -> Result<String, HdWalletError> {
    if coin == Coin::Sol {
        let secret = hot_secret_from_mnemonic(mnemonic, coin)?;
        return Ok(sol_address_from_secret(&secret));
    }
    let secret = hot_secret_from_mnemonic(mnemonic, coin)?;
    address_from_secret_bytes(coin, network, &secret)
}

pub fn secret_to_wif(secret: &[u8; 32]) -> Result<String, HdWalletError> {
    let sk = SecretKey::from_slice(secret).map_err(|e| e.to_string())?;
    Ok(bitcoin::PrivateKey::new(sk, Network::Bitcoin).to_wif())
}

pub fn account_xpub(mnemonic: &str, coin: Coin) -> Result<String, HdWalletError> {
    let secp = Secp256k1::new();
    let master = master_xpriv(mnemonic)?;
    let account = master
        .derive_priv(&secp, &DerivationPath::from_str(account_path(coin)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(Xpub::from_priv(&secp, &account).to_string())
}

pub fn address_from_secret_bytes(coin: Coin, network: ChainNetwork, secret: &[u8; 32]) -> Result<String, HdWalletError> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(secret).map_err(|e| e.to_string())?;
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    Ok(match params_for(coin, network).address_kind {
        AddressKind::Bech32Segwit { hrp, .. } => bech32_p2wpkh_encode(hrp, &hash160(&compressed_bytes(&pk))),
        AddressKind::Base58Only { version } => base58check_encode(version, &hash160(&compressed_bytes(&pk))),
        AddressKind::CashAddr { prefix } => cashaddr_encode(prefix, &hash160(&compressed_bytes(&pk))),
        AddressKind::Evm => eip55_encode(&evm_address_from_uncompressed_pubkey(&uncompressed_xy_bytes(&pk))),
        AddressKind::Solana => {
            let secret: [u8; 32] = *secret;
            sol_address_from_secret(&secret)
        }
        AddressKind::ZcashTransparent { version } => zcash_t1_encode(version, &hash160(&compressed_bytes(&pk))),
    })
}
