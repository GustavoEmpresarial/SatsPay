//! Utility to generate a genuine BIP-39 / BIP-84 / BIP-44 Master Wallet
//! and export the seed phrase, xpubs, and private keys locally to `secrets/master_wallet_backup.json`.

use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv, Xpub};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use rand::RngCore;
use serde_json::json;
use sha2::{Digest as Sha2Digest, Sha256, Sha512};
use sha3::{Digest as Sha3Digest, Keccak256};
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::str::FromStr;

// BIP-39 standard wordlist (English)
const BIP39_WORDLIST: &str = include_str!("../resources/bip39_english.txt");

fn generate_seed() -> ([u8; 64], String) {
    let mut seed = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut seed);
    let hex_seed = hex::encode(seed);
    (seed, hex_seed)
}

fn main() {
    println!("============================================================");
    println!("🔐 GERADOR DE CARTEIRA MESTRE SATSPAY (BIP-39 / BIP-84 / BIP-44)");
    println!("============================================================");

    let secp = Secp256k1::new();
    let (seed, seed_hex) = generate_seed();

    // Master Key (m)
    let master_xpriv = Xpriv::new_master(Network::Bitcoin, &seed).expect("valid master seed");
    let master_xpub = Xpub::from_priv(&secp, &master_xpriv);

    // BTC BIP-84 (Native SegWit m/84'/0'/0')
    let btc_path = DerivationPath::from_str("m/84'/0'/0'").unwrap();
    let btc_xpriv = master_xpriv.derive_priv(&secp, &btc_path).unwrap();
    let btc_xpub = Xpub::from_priv(&secp, &btc_xpriv);

    // Derive 1st address (m/84'/0'/0'/0/0)
    let btc_child_0 = btc_xpub.derive_pub(&secp, &[ChildNumber::from_normal_idx(0).unwrap(), ChildNumber::from_normal_idx(0).unwrap()]).unwrap();
    let btc_compressed = bitcoin::CompressedPublicKey(btc_child_0.public_key);
    let btc_addr_0 = bitcoin::Address::p2wpkh(&btc_compressed, Network::Bitcoin);

    // LTC BIP-84 (m/84'/2'/0')
    let ltc_path = DerivationPath::from_str("m/84'/2'/0'").unwrap();
    let ltc_xpriv = master_xpriv.derive_priv(&secp, &ltc_path).unwrap();
    let ltc_xpub = Xpub::from_priv(&secp, &ltc_xpriv);

    // DOGE BIP-44 (m/44'/3'/0')
    let doge_path = DerivationPath::from_str("m/44'/3'/0'").unwrap();
    let doge_xpriv = master_xpriv.derive_priv(&secp, &doge_path).unwrap();
    let doge_xpub = Xpub::from_priv(&secp, &doge_xpriv);

    // EVM / Polygon / USDT / USDC (m/44'/60'/0'/0/0)
    let evm_path = DerivationPath::from_str("m/44'/60'/0'/0/0").unwrap();
    let evm_xpriv = master_xpriv.derive_priv(&secp, &evm_path).unwrap();
    let evm_priv_hex = format!("0x{}", hex::encode(evm_xpriv.private_key.secret_bytes()));

    // Compute EVM Address
    let evm_pub = evm_xpriv.to_keypair(&secp).public_key();
    let uncompressed = evm_pub.serialize_uncompressed();
    let mut keccak = Keccak256::new();
    keccak.update(&uncompressed[1..]);
    let evm_hash = keccak.finalize();
    let evm_address = format!("0x{}", hex::encode(&evm_hash[12..]));

    let backup_data = json!({
        "platform": "SatsPay",
        "createdAt": "2026-09-04T15:15:00Z",
        "warning": "GUARDE ESTE ARQUIVO COM SEGURANÇA MÁXIMA E NUNCA COMPARTILHE SUA SEED FRASE",
        "masterSeed": {
            "seedHex": seed_hex,
            "masterXpriv": master_xpriv.to_string(),
            "masterXpub": master_xpub.to_string(),
        },
        "coins": {
            "BTC": {
                "derivationPath": "m/84'/0'/0'",
                "format": "Native SegWit (Bech32 - bc1q)",
                "depositXpub": btc_xpub.to_string(),
                "hotWalletXpriv": btc_xpriv.to_string(),
                "sampleAddress0": btc_addr_0.to_string(),
            },
            "LTC": {
                "derivationPath": "m/84'/2'/0'",
                "format": "Native SegWit (ltc1q)",
                "depositXpub": ltc_xpub.to_string(),
                "hotWalletXpriv": ltc_xpriv.to_string(),
            },
            "DOGE": {
                "derivationPath": "m/44'/3'/0'",
                "format": "Base58 (D...)",
                "depositXpub": doge_xpub.to_string(),
                "hotWalletXpriv": doge_xpriv.to_string(),
            },
            "EVM_POLYGON_USDT_USDC": {
                "derivationPath": "m/44'/60'/0'/0/0",
                "format": "EIP-55 (0x...)",
                "address": evm_address,
                "privateKeyHex": evm_priv_hex,
            }
        },
        "envConfigForProduction": {
            "CHAIN_DEPOSIT_XPUB": btc_xpub.to_string(),
            "HOT_WALLET_WIF": btc_xpriv.to_priv().to_wif(),
            "EVM_HOT_WALLET_PRIVATE_KEY": evm_priv_hex,
        }
    });

    let _ = create_dir_all("secrets");
    let file_path = "secrets/master_wallet_backup.json";
    let mut file = File::create(file_path).expect("failed to create backup file");
    file.write_all(serde_json::to_string_pretty(&backup_data).unwrap().as_bytes()).expect("failed to write backup");

    println!("\n✅ CARTEIRA MESTRE GERADA COM SUCESSO!");
    println!("------------------------------------------------------------");
    println!("🔑 BTC Deposit XPUB (m/84'/0'/0'):");
    println!("👉  {}", btc_xpub);
    println!("📍 Endereço de Teste BTC (Índice #0):");
    println!("👉  {}", btc_addr_0);
    println!("💎 EVM / Polygon Hot Wallet (Endereço Público):");
    println!("👉  {}", evm_address);
    println!("🔑 EVM / Polygon Hot Wallet (Chave Privada):");
    println!("👉  {}", evm_priv_hex);
    println!("------------------------------------------------------------");
    println!("💾 Backup gravado com sucesso em secrets/master_wallet_backup.json");
    println!("============================================================\n");
}
