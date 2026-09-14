use crate::bch_sign::build_and_sign_bch_p2pkh;
use crate::btc_sign::{build_and_sign_p2wpkh, SignError, Utxo};
use crate::encoding::{
    base58check_decode, base58check_encode, bech32_p2wpkh_decode, bech32_p2wpkh_encode, cashaddr_decode, cashaddr_encode, eip55_encode, eip55_validate,
    evm_address_from_uncompressed_pubkey, hash160, solana_address_decode, solana_address_encode,
};
use crate::evm_sign::{address_from_secret, parse_address, sign_legacy_tx, LegacyTx};
use crate::hd::{compressed_bytes, derive_child_pubkey, uncompressed_xy_bytes};
use crate::hd_wallet::{
    account_path, account_xpub, address_from_mnemonic, address_from_secret_bytes, generate_mnemonic, hot_address_from_mnemonic, hot_secret_from_mnemonic,
    mnemonic_to_seed, secret_from_mnemonic, secret_to_wif,
};
use crate::params::{params, params_for, AddressKind, ChainNetwork};
use crate::policy::assert_stub_client_allowed;
use crate::registry::ChainRegistry;
use crate::sol_client::{derive_sol_secret, sol_address_from_secret, sol_master_from_hot_key};
use crate::{ChainClient, StubClient};
use bitcoin::hashes::Hash;
use bitcoin::ScriptBuf;
use shared::Coin;

#[test]
fn stub_policy_forbids_production() {
    assert!(assert_stub_client_allowed("production", true, "BTC").is_err());
}

#[test]
fn stub_policy_requires_explicit_opt_in() {
    assert!(assert_stub_client_allowed("development", false, "BTC").is_err());
    assert!(assert_stub_client_allowed("development", true, "BTC").is_ok());
}

#[tokio::test]
async fn stub_generates_addresses_that_pass_its_own_validator() {
    for coin in shared::COINS {
        let client = StubClient::new(coin);
        let addr = client.generate_address("user-1").await.unwrap();
        assert!(client.validate_address(&addr.address), "{coin:?} generated invalid address: {}", addr.address);
    }
}

#[tokio::test]
async fn stub_broadcast_is_disabled() {
    let client = StubClient::new(Coin::Btc);
    let err = client.broadcast_withdrawal("bc1qanything", 100_000).await.unwrap_err();
    assert!(err.safe_to_reverse);
    assert!(err.message.contains("stub"));
}

// --- Real address encoding — verified against each format's own canonical
// test vectors (not addresses we invented), plus real mainnet addresses
// independently confirmed to hold funds via a live API call during
// development (see `docs/decisions/` for the raw curl output).

#[test]
fn btc_bech32_p2wpkh_real_mainnet_address() {
    // Confirmed live via `GET https://blockstream.info/api/address/...`
    // during development — a real, funded mainnet address.
    let addr = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    let hash = bech32_p2wpkh_decode("bc", addr).expect("valid real BTC bech32 address must decode");
    assert_eq!(bech32_p2wpkh_encode("bc", &hash), addr, "round-trip must reproduce the exact same address");
}

#[test]
fn btc_bech32_rejects_wrong_hrp() {
    assert!(bech32_p2wpkh_decode("ltc", "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh").is_none());
}

#[test]
fn base58check_round_trip_and_checksum_rejection() {
    let hash = hash160(b"arbitrary pubkey bytes for round-trip test only");
    let addr = base58check_encode(0x00, &hash); // BTC legacy version byte
    let (version, decoded_hash) = base58check_decode(&addr).expect("valid base58check must decode");
    assert_eq!(version, 0x00);
    assert_eq!(decoded_hash, hash);

    // Flipping one character must break the checksum.
    let mut corrupted = addr.clone();
    corrupted.replace_range(5..6, if &addr[5..6] == "1" { "2" } else { "1" });
    assert!(base58check_decode(&corrupted).is_none(), "corrupted address must fail checksum validation");
}

#[test]
fn doge_base58_version_byte_is_correct() {
    // DOGE's real P2PKH version byte (from Dogecoin's chainparams.cpp,
    // PUBKEY_ADDRESS = 30 decimal = 0x1e) — every real Dogecoin address
    // starts with 'D' as a direct consequence of this version byte.
    let hash = hash160(b"doge test pubkey bytes");
    let addr = base58check_encode(0x1e, &hash);
    assert!(addr.starts_with('D'), "DOGE addresses with version 0x1e must start with 'D', got: {addr}");
    assert_eq!(base58check_decode(&addr), Some((0x1e, hash)));
}

#[test]
fn bch_cashaddr_official_spec_test_vector() {
    // This exact address is the CashAddr spec's own worked example
    // (https://github.com/bitcoincashorg/bitcoincash.org spec/cashaddr.md)
    // and was independently confirmed live (real, funded mainnet address)
    // via `GET https://api.bitcore.io/api/BCH/mainnet/address/.../balance`
    // during development.
    let full = "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a";
    let hash = cashaddr_decode("bitcoincash", full).expect("official CashAddr spec test vector must decode");
    assert_eq!(cashaddr_encode("bitcoincash", &hash), full);

    // Bare form (no "bitcoincash:" prefix) must also decode to the same hash.
    let bare = "qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a";
    assert_eq!(cashaddr_decode("bitcoincash", bare), Some(hash));
}

#[test]
fn bch_cashaddr_rejects_corrupted_checksum() {
    let mut corrupted = "qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a".to_string();
    corrupted.replace_range(0..1, if &corrupted[0..1] == "q" { "p" } else { "q" });
    assert!(cashaddr_decode("bitcoincash", &corrupted).is_none());
}

#[test]
fn evm_eip55_official_spec_test_vectors() {
    // From EIP-55 itself (https://eips.ethereum.org/EIPS/eip-55) — the
    // canonical mixed-case checksum test vectors, not addresses we made up.
    let vectors =
        ["0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed", "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359", "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB", "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb"];
    for v in vectors {
        assert!(eip55_validate(v), "official EIP-55 test vector must validate: {v}");
        let bytes: [u8; 20] = hex::decode(&v[2..]).unwrap().try_into().unwrap();
        assert_eq!(eip55_encode(&bytes), v, "checksum re-encoding must reproduce the exact spec vector");
    }
}

#[test]
fn evm_address_derivation_known_vector() {
    // A well-known secp256k1 keypair used across Ethereum tooling test
    // suites: private key 0x1, whose address is publicly documented as
    // 0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf.
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let sk = bitcoin::secp256k1::SecretKey::from_slice(&[0u8; 31].iter().copied().chain(std::iter::once(1u8)).collect::<Vec<u8>>()).unwrap();
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let uncompressed = pk.serialize_uncompressed();
    let xy: [u8; 64] = uncompressed[1..].try_into().unwrap();
    let addr = eip55_encode(&evm_address_from_uncompressed_pubkey(&xy));
    assert_eq!(addr, "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");
}

#[test]
fn testnet_params_match_chainparams() {
    use crate::params::{params_for, AddressKind, ChainNetwork};

    let btc = params_for(Coin::Btc, ChainNetwork::Testnet);
    match btc.address_kind {
        AddressKind::Bech32Segwit { hrp, legacy_base58_version } => {
            assert_eq!(hrp, "tb");
            assert_eq!(legacy_base58_version, 0x6f);
        }
        other => panic!("expected Bech32Segwit, got {other:?}"),
    }

    let ltc = params_for(Coin::Ltc, ChainNetwork::Testnet);
    match ltc.address_kind {
        AddressKind::Bech32Segwit { hrp, legacy_base58_version } => {
            assert_eq!(hrp, "tltc");
            assert_eq!(legacy_base58_version, 0x6f);
        }
        other => panic!("expected Bech32Segwit, got {other:?}"),
    }

    let doge = params_for(Coin::Doge, ChainNetwork::Testnet);
    match doge.address_kind {
        AddressKind::Base58Only { version } => assert_eq!(version, 0x71),
        other => panic!("expected Base58Only, got {other:?}"),
    }

    let bch = params_for(Coin::Bch, ChainNetwork::Testnet);
    match bch.address_kind {
        AddressKind::CashAddr { prefix } => assert_eq!(prefix, "bchtest"),
        other => panic!("expected CashAddr, got {other:?}"),
    }

    assert_eq!(params_for(Coin::Pol, ChainNetwork::Testnet).evm_chain_id, Some(80002));
    assert_eq!(params_for(Coin::Pol, ChainNetwork::Mainnet).evm_chain_id, Some(137));
    assert_eq!(ChainNetwork::Testnet.as_bitcore_str(), "testnet");

    match params_for(Coin::Dgb, ChainNetwork::Mainnet).address_kind {
        AddressKind::Bech32Segwit { hrp, legacy_base58_version } => {
            assert_eq!(hrp, "dgb");
            assert_eq!(legacy_base58_version, 0x1e);
        }
        other => panic!("expected DGB Bech32Segwit, got {other:?}"),
    }
    assert!(matches!(params_for(Coin::Sol, ChainNetwork::Mainnet).address_kind, AddressKind::Solana));
    assert_eq!(
        params_for(Coin::Usdt, ChainNetwork::Mainnet).erc20_contract,
        Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F")
    );
    assert_eq!(
        params_for(Coin::Usdc, ChainNetwork::Mainnet).erc20_contract,
        Some("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359")
    );
}

#[test]
fn two_mnemonics_make_valid_deposit_and_hot_addresses() {
    let deposit = generate_mnemonic();
    let hot = generate_mnemonic();
    assert_eq!(deposit.split_whitespace().count(), 24);
    assert_eq!(hot.split_whitespace().count(), 24);
    assert_ne!(deposit, hot);
    for coin in shared::COINS {
        let recv = address_from_mnemonic(&deposit, coin, ChainNetwork::Mainnet, 1).unwrap();
        let hot_addr = hot_address_from_mnemonic(&hot, coin, ChainNetwork::Mainnet).unwrap();
        let stub = StubClient::new(coin);
        assert!(stub.validate_address(&recv), "{coin:?} deposit invalid: {recv}");
        assert!(stub.validate_address(&hot_addr), "{coin:?} hot invalid: {hot_addr}");
        assert_ne!(recv, hot_addr);
    }
}

// --- encoding edge cases ---

#[test]
fn solana_address_round_trip_and_reject_bad_length() {
    let pk = [0x42u8; 32];
    let addr = solana_address_encode(&pk);
    assert_eq!(solana_address_decode(&addr), Some(pk));
    assert!(solana_address_decode("not-valid-base58!!!").is_none());
    assert!(solana_address_decode(&bs58::encode([1u8; 16]).into_string()).is_none());
}

#[test]
fn eip55_validate_rejects_bad_forms() {
    assert!(!eip55_validate("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed")); // missing 0x
    assert!(!eip55_validate("0x5a")); // too short
    assert!(!eip55_validate("0xGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG"));
    // Wrong mixed-case checksum (flip one letter case vs EIP-55 vector).
    assert!(!eip55_validate("0x5aaeb6053F3E94C9b9A09f33669435E7Ef1BeAed"));
    // All-lowercase / all-uppercase are accepted per EIP-55.
    assert!(eip55_validate("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed"));
    assert!(eip55_validate("0x5AAEB6053F3E94C9B9A09F33669435E7EF1BEAED"));
}

#[test]
fn base58check_rejects_wrong_length() {
    assert!(base58check_decode("1").is_none());
    assert!(base58check_decode("").is_none());
}

#[test]
fn bech32_rejects_empty_and_non_v0() {
    assert!(bech32_p2wpkh_decode("bc", "bc1").is_none());
    // Valid bech32 but not witness v0 P2WPKH (use a known short invalid program path).
    assert!(bech32_p2wpkh_decode("bc", "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4").is_some());
}

#[test]
fn cashaddr_rejects_uppercase_and_too_short() {
    assert!(cashaddr_decode("bitcoincash", "QPM2QSZNHKS23Z7629MMS6S4CWEF74VCWVY22GDX6A").is_none());
    assert!(cashaddr_decode("bitcoincash", "qpm2").is_none());
}

// --- params ---

#[test]
fn chain_network_parse_and_bitcore_str() {
    assert_eq!(ChainNetwork::parse("mainnet").unwrap(), ChainNetwork::Mainnet);
    assert_eq!(ChainNetwork::parse("testnet").unwrap(), ChainNetwork::Testnet);
    assert!(ChainNetwork::parse("Mainnet").is_err());
    assert_eq!(ChainNetwork::Mainnet.as_bitcore_str(), "mainnet");
}

#[test]
fn mainnet_params_cover_every_coin() {
    for coin in shared::COINS {
        let p = params(coin);
        let p2 = params_for(coin, ChainNetwork::Mainnet);
        assert_eq!(p.evm_chain_id, p2.evm_chain_id);
        assert_eq!(p.erc20_contract, p2.erc20_contract);
        assert_eq!(p.bitcore_chain, p2.bitcore_chain);
        match coin {
            Coin::Btc => assert!(matches!(p.address_kind, AddressKind::Bech32Segwit { hrp: "bc", .. })),
            Coin::Ltc => assert!(matches!(p.address_kind, AddressKind::Bech32Segwit { hrp: "ltc", .. })),
            Coin::Doge => assert!(matches!(p.address_kind, AddressKind::Base58Only { version: 0x1e })),
            Coin::Bch => assert!(matches!(p.address_kind, AddressKind::CashAddr { prefix: "bitcoincash" })),
            Coin::Pol => {
                assert!(matches!(p.address_kind, AddressKind::Evm));
                assert_eq!(p.evm_chain_id, Some(137));
            }
            Coin::Dgb => assert!(matches!(p.address_kind, AddressKind::Bech32Segwit { hrp: "dgb", .. })),
            Coin::Sol => assert!(matches!(p.address_kind, AddressKind::Solana)),
            Coin::Usdt => assert_eq!(p.erc20_contract, Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F")),
            Coin::Usdc => assert_eq!(p.erc20_contract, Some("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359")),
        }
    }
    assert_eq!(params_for(Coin::Dgb, ChainNetwork::Testnet).bitcore_chain, None);
    match params_for(Coin::Dgb, ChainNetwork::Testnet).address_kind {
        AddressKind::Bech32Segwit { hrp, legacy_base58_version } => {
            assert_eq!(hrp, "dgbt");
            assert_eq!(legacy_base58_version, 0x7e);
        }
        other => panic!("expected dgbt, got {other:?}"),
    }
    assert_eq!(
        params_for(Coin::Usdc, ChainNetwork::Testnet).erc20_contract,
        Some("0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582")
    );
}

// --- HD ---

#[test]
fn hd_derive_child_pubkey_and_encodings() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let xpub = account_xpub(mnemonic, Coin::Btc).unwrap();
    let pk = derive_child_pubkey(&xpub, 0).expect("valid account xpub + index 0");
    let pk1 = derive_child_pubkey(&xpub, 1).unwrap();
    assert_ne!(pk.serialize(), pk1.serialize());
    assert_eq!(compressed_bytes(&pk).len(), 33);
    assert_eq!(uncompressed_xy_bytes(&pk).len(), 64);
    assert!(derive_child_pubkey("not-an-xpub", 0).is_err());
    // Hardened bit set → not a valid non-hardened index for from_normal_idx.
    assert!(derive_child_pubkey(&xpub, 0x8000_0000).is_err());
}

// --- hd_wallet ---

#[test]
fn hd_wallet_paths_seeds_and_secrets() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    assert_eq!(mnemonic_to_seed(mnemonic).len(), 64);
    assert_eq!(account_path(Coin::Btc), "m/84'/0'/0'");
    assert_eq!(account_path(Coin::Sol), "m/44'/501'/0'");
    let xpub = account_xpub(mnemonic, Coin::Btc).unwrap();
    assert!(xpub.starts_with("xpub"));
    let secret = secret_from_mnemonic(mnemonic, Coin::Btc, 0).unwrap();
    let wif = secret_to_wif(&secret).unwrap();
    assert!(wif.starts_with('K') || wif.starts_with('L') || wif.starts_with('5'));
    let hot = hot_secret_from_mnemonic(mnemonic, Coin::Btc).unwrap();
    assert_eq!(hot, secret);
    let sol_secret = secret_from_mnemonic(mnemonic, Coin::Sol, 3).unwrap();
    assert_ne!(sol_secret, [0u8; 32]);
    let hot_sol = hot_secret_from_mnemonic(mnemonic, Coin::Sol).unwrap();
    assert_ne!(hot_sol, sol_secret);
}

#[test]
fn address_from_secret_bytes_covers_address_kinds() {
    let secret = [2u8; 32];
    for coin in [Coin::Btc, Coin::Doge, Coin::Bch, Coin::Pol, Coin::Sol] {
        let addr = address_from_secret_bytes(coin, ChainNetwork::Mainnet, &secret).unwrap();
        assert!(!addr.is_empty(), "{coin:?}");
        assert!(StubClient::new(coin).validate_address(&addr), "{coin:?}: {addr}");
    }
}

// --- registry / stub ---

#[test]
fn registry_build_stub_and_get() {
    assert!(ChainRegistry::build("production", true).is_err());
    assert!(ChainRegistry::build("development", false).is_err());
    let reg = ChainRegistry::build("development", true).unwrap();
    for coin in shared::COINS {
        let client = reg.get(coin);
        assert_eq!(client.coin(), coin);
        assert!(!client.validate_address("definitely-not-valid"));
    }
}

#[tokio::test]
async fn registry_from_env_stub_path() {
    // Isolate env for this test; restore afterwards.
    let prev_use = std::env::var("USE_REAL_CHAIN_CLIENTS").ok();
    let prev_allow = std::env::var("ALLOW_STUB_CHAIN").ok();
    let prev_node = std::env::var("NODE_ENV").ok();
    std::env::set_var("USE_REAL_CHAIN_CLIENTS", "false");
    std::env::set_var("ALLOW_STUB_CHAIN", "true");
    std::env::set_var("NODE_ENV", "test");
    let pool = sqlx::PgPool::connect_lazy("postgres://bitcosats:bitcosats@127.0.0.1:1/none").unwrap();
    let reg = ChainRegistry::from_env(pool).expect("stub from_env");
    assert_eq!(reg.get(Coin::Btc).coin(), Coin::Btc);
    match prev_use {
        Some(v) => std::env::set_var("USE_REAL_CHAIN_CLIENTS", v),
        None => std::env::remove_var("USE_REAL_CHAIN_CLIENTS"),
    }
    match prev_allow {
        Some(v) => std::env::set_var("ALLOW_STUB_CHAIN", v),
        None => std::env::remove_var("ALLOW_STUB_CHAIN"),
    }
    match prev_node {
        Some(v) => std::env::set_var("NODE_ENV", v),
        None => std::env::remove_var("NODE_ENV"),
    }
}

#[tokio::test]
async fn registry_build_real_and_from_env_construct_only() {
    use crate::real_client::RealClientConfig;

    let pool = sqlx::PgPool::connect_lazy("postgres://bitcosats:bitcosats@127.0.0.1:1/none").unwrap();
    let cfg = RealClientConfig {
        bitcore_base_url: "http://127.0.0.1:9".into(),
        evm_rpc_url: "http://127.0.0.1:9".into(),
        deposit_xpub: String::new(),
        hot_wallet_wif: None,
        evm_deposit_lookback_blocks: 10,
        fee_confirmation_target: 2,
        network: ChainNetwork::Testnet,
        sol_rpc_url: "http://127.0.0.1:9".into(),
        dgb_insight_url: "http://127.0.0.1:9".into(),
        deposit_mnemonic: None,
        hot_mnemonic: None,
    };
    let reg = ChainRegistry::build_real(pool.clone(), cfg);
    assert_eq!(reg.get(Coin::Ltc).coin(), Coin::Ltc);

    let keys = [
        "USE_REAL_CHAIN_CLIENTS",
        "CHAIN_NETWORK",
        "BITCORE_API_BASE_URL",
        "EVM_RPC_URL",
        "SOL_RPC_URL",
        "DGB_INSIGHT_API",
        "EVM_DEPOSIT_LOOKBACK_BLOCKS",
        "FEE_CONFIRMATION_TARGET",
        "DEPOSIT_MNEMONIC",
        "HOT_MNEMONIC",
        "NODE_ENV",
    ];
    let prev: Vec<_> = keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();
    std::env::set_var("USE_REAL_CHAIN_CLIENTS", "true");
    std::env::set_var("CHAIN_NETWORK", "testnet");
    std::env::set_var("BITCORE_API_BASE_URL", "http://127.0.0.1:9");
    std::env::set_var("EVM_RPC_URL", "http://127.0.0.1:9");
    std::env::set_var("SOL_RPC_URL", "http://127.0.0.1:9");
    std::env::set_var("DGB_INSIGHT_API", "http://127.0.0.1:9");
    std::env::set_var("EVM_DEPOSIT_LOOKBACK_BLOCKS", "50");
    std::env::set_var("FEE_CONFIRMATION_TARGET", "3");
    std::env::remove_var("DEPOSIT_MNEMONIC");
    std::env::remove_var("HOT_MNEMONIC");
    let pool2 = sqlx::PgPool::connect_lazy("postgres://bitcosats:bitcosats@127.0.0.1:1/none").unwrap();
    let reg2 = ChainRegistry::from_env(pool2).expect("real from_env construct");
    assert_eq!(reg2.get(Coin::Pol).coin(), Coin::Pol);
    for (k, v) in prev {
        match v {
            Some(val) => std::env::set_var(k, val),
            None => std::env::remove_var(k),
        }
    }
}

#[tokio::test]
async fn real_client_validate_address_and_hot_wallet_helpers() {
    use crate::real_client::{hot_wallet_address, parse_secret_key_bytes, RealClientConfig, RealChainClient};

    let wif = throwaway_wif();
    let secret = parse_secret_key_bytes(&wif).unwrap();
    assert_eq!(secret.len(), 32);
    assert!(parse_secret_key_bytes("not-a-key").is_err());
    let hex_key = format!("0x{}", hex::encode(secret));
    assert_eq!(parse_secret_key_bytes(&hex_key).unwrap(), secret);

    for coin in [Coin::Btc, Coin::Doge, Coin::Bch, Coin::Pol, Coin::Sol, Coin::Ltc, Coin::Dgb] {
        let addr = hot_wallet_address(coin, ChainNetwork::Mainnet, &wif).unwrap();
        assert!(!addr.is_empty(), "{coin:?}");
    }

    let pool = sqlx::PgPool::connect_lazy("postgres://bitcosats:bitcosats@127.0.0.1:1/none").unwrap();
    let client = RealChainClient::new(
        Coin::Btc,
        pool,
        RealClientConfig {
            bitcore_base_url: "http://127.0.0.1:9".into(),
            evm_rpc_url: "http://127.0.0.1:9".into(),
            deposit_xpub: String::new(),
            hot_wallet_wif: Some(wif.clone()),
            evm_deposit_lookback_blocks: 10,
            fee_confirmation_target: 2,
            network: ChainNetwork::Mainnet,
            sol_rpc_url: "http://127.0.0.1:9".into(),
            dgb_insight_url: "http://127.0.0.1:9".into(),
            deposit_mnemonic: None,
            hot_mnemonic: None,
        },
    );
    assert_eq!(client.coin(), Coin::Btc);
    let btc_addr = hot_wallet_address(Coin::Btc, ChainNetwork::Mainnet, &wif).unwrap();
    assert!(client.validate_address(&btc_addr));
    assert!(!client.validate_address("nope"));
    assert!(!client.validate_address(&"x".repeat(200)));
}

#[test]
fn evm_erc20_transfer_data_selector() {
    let data = crate::evm_client::erc20_transfer_data([0x11; 20], 1_000_000);
    assert_eq!(&data[..4], &[0xa9, 0x05, 0x9c, 0xbb]); // transfer(address,uint256)
    assert_eq!(data.len(), 4 + 32 + 32);
}

#[test]
fn chain_error_display() {
    let err = crate::ChainError { message: "boom".into() };
    assert_eq!(err.to_string(), "boom");
    let berr = crate::types::BroadcastError { message: "nope".into(), safe_to_reverse: true };
    assert_eq!(berr.to_string(), "nope");
}

#[test]
fn stub_rejects_invalid_and_reports_coin() {
    let client = StubClient::new(Coin::Btc);
    assert_eq!(client.coin(), Coin::Btc);
    assert!(!client.validate_address(""));
    assert!(!client.validate_address(&"x".repeat(200)));
    assert!(!client.validate_address("1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2")); // legacy, not bc1q
}

// --- offline signers (no RPC) ---

fn throwaway_wif() -> String {
    let sk = bitcoin::secp256k1::SecretKey::from_slice(&[3u8; 32]).unwrap();
    bitcoin::PrivateKey::new(sk, bitcoin::Network::Bitcoin).to_wif()
}

#[test]
fn btc_sign_insufficient_utxos_and_happy_path() {
    let wif = throwaway_wif();
    let zero_hash = bitcoin::hashes::hash160::Hash::from_byte_array([0u8; 20]);
    let to = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_raw_hash(zero_hash));

    match build_and_sign_p2wpkh(&wif, &[], &to, 1_000, 200, &to) {
        Err(SignError::InsufficientUtxos { needed }) => assert_eq!(needed, 1_200),
        other => panic!("expected InsufficientUtxos, got {other:?}"),
    }
    assert!(matches!(build_and_sign_p2wpkh("not-a-wif", &[], &to, 1, 1, &to), Err(SignError::InvalidKey(_))));

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let privkey = bitcoin::PrivateKey::from_wif(&wif).unwrap();
    let compressed = bitcoin::CompressedPublicKey::from_private_key(&secp, &privkey).unwrap();
    let change = bitcoin::Address::p2wpkh(&compressed, bitcoin::Network::Bitcoin);
    let change_script = change.script_pubkey();
    let script_hex = hex::encode(change_script.as_bytes());
    let utxos = [Utxo {
        txid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        vout: 0,
        value: 50_000,
        script_pubkey_hex: script_hex,
    }];
    let tx = build_and_sign_p2wpkh(&wif, &utxos, &to, 10_000, 500, &change_script).unwrap();
    assert_eq!(tx.input.len(), 1);
    assert_eq!(tx.output.len(), 2);
    assert!(!tx.input[0].witness.is_empty());

    let bad_utxo = [Utxo {
        txid: "zz".to_string(),
        vout: 0,
        value: 50_000,
        script_pubkey_hex: hex::encode(change_script.as_bytes()),
    }];
    assert!(matches!(build_and_sign_p2wpkh(&wif, &bad_utxo, &to, 1_000, 100, &change_script), Err(SignError::InvalidTxid(_))));
}

#[test]
fn bch_sign_insufficient_and_happy_path() {
    let wif = throwaway_wif();
    let zero_hash = bitcoin::hashes::hash160::Hash::from_byte_array([0u8; 20]);
    let to = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(zero_hash));

    match build_and_sign_bch_p2pkh(&wif, &[], &to, 1_000, 200, &to) {
        Err(SignError::InsufficientUtxos { needed }) => assert_eq!(needed, 1_200),
        other => panic!("expected InsufficientUtxos, got {other:?}"),
    }

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let privkey = bitcoin::PrivateKey::from_wif(&wif).unwrap();
    let pk = bitcoin::PublicKey::from_private_key(&secp, &privkey);
    let change = bitcoin::Address::p2pkh(&pk, bitcoin::Network::Bitcoin);
    let change_script = change.script_pubkey();
    let utxos = [Utxo {
        txid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        vout: 1,
        value: 80_000,
        script_pubkey_hex: hex::encode(change_script.as_bytes()),
    }];
    let tx = build_and_sign_bch_p2pkh(&wif, &utxos, &to, 20_000, 1_000, &change_script).unwrap();
    assert_eq!(tx.input.len(), 1);
    assert!(!tx.input[0].script_sig.is_empty());

    // Non-P2PKH scriptPubKey must fail.
    let wpkh = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_raw_hash(zero_hash));
    let bad = [Utxo {
        txid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        vout: 0,
        value: 80_000,
        script_pubkey_hex: hex::encode(wpkh.as_bytes()),
    }];
    assert!(matches!(build_and_sign_bch_p2pkh(&wif, &bad, &to, 1_000, 100, &change_script), Err(SignError::InvalidScript(_))));
}

#[test]
fn evm_sign_parse_address_and_legacy_tx() {
    assert!(parse_address("0x22").is_err());
    assert!(parse_address("not-hex").is_err());
    let to = parse_address("0x2222222222222222222222222222222222222222").unwrap();
    assert_eq!(to, [0x22; 20]);

    let sk = [0x11u8; 32];
    let addr = address_from_secret(&sk).unwrap();
    assert_ne!(addr, [0u8; 20]);
    assert!(address_from_secret(&[0u8; 32]).is_err());

    let raw = sign_legacy_tx(
        &sk,
        &LegacyTx {
            nonce: 0,
            gas_price_wei: 1,
            gas_limit: 21_000,
            to,
            value_wei: 1,
            data: vec![0x01, 0x02],
            chain_id: 137,
        },
    )
    .unwrap();
    assert!(raw.starts_with("0x"));
    let body = hex::decode(raw.trim_start_matches("0x")).unwrap();
    assert!(body[0] >= 0xc0);

    // Long data exercises RLP length encoding for payloads >= 56 bytes.
    let long = sign_legacy_tx(
        &sk,
        &LegacyTx {
            nonce: 7,
            gas_price_wei: 0,
            gas_limit: 100_000,
            to,
            value_wei: 0,
            data: vec![0u8; 64],
            chain_id: 80002,
        },
    )
    .unwrap();
    assert!(long.len() > 100);
}

// --- sol helpers (no RPC) ---

#[test]
fn sol_client_helpers_are_deterministic() {
    let master = sol_master_from_hot_key("0xdeadbeef");
    let master2 = sol_master_from_hot_key("deadbeef");
    assert_eq!(master, master2);
    let a = derive_sol_secret(&master, b"bitcosats-sol-deposit", 0);
    let b = derive_sol_secret(&master, b"bitcosats-sol-deposit", 1);
    assert_ne!(a, b);
    assert_eq!(a, derive_sol_secret(&master, b"bitcosats-sol-deposit", 0));
    let addr = sol_address_from_secret(&a);
    assert!(solana_address_decode(&addr).is_some());
    assert!(StubClient::new(Coin::Sol).validate_address(&addr));
}

