//! Generate a throwaway *testnet* WIF + hot addresses for broadcast_testnet_smoke.
//! Does not print mainnet keys. Safe to fund via public faucets only.
//!
//! Run: `cargo run -p chain --example gen_testnet_faucet_wallet`

use chain::params::ChainNetwork;
use chain::real_client::hot_wallet_address;
use shared::Coin;

fn main() {
    let sk = bitcoin::secp256k1::SecretKey::new(&mut bitcoin::secp256k1::rand::thread_rng());
    let wif = bitcoin::PrivateKey::new(sk, bitcoin::Network::Testnet).to_wif();
    println!("HOT_WALLET_WIF={wif}");
    println!("CHAIN_NETWORK=testnet");
    println!();
    for coin in [Coin::Btc, Coin::Bch, Coin::Pol] {
        match hot_wallet_address(coin, ChainNetwork::Testnet, &wif) {
            Ok(addr) => println!("{coin:?}_ADDRESS={addr}"),
            Err(e) => eprintln!("{coin:?}_ADDRESS_ERROR={e}"),
        }
    }
    println!();
    println!("# Faucets (fund then re-run broadcast_testnet_smoke):");
    println!("# BTC testnet4/testnet: https://coinfaucet.eu/en/btc-testnet/ or https://bitcoinfaucet.uo1.net/");
    println!("# BCH testnet: https://tbch.googol.cash/ (bchtest)");
    println!("# POL Amoy: https://faucet.polygon.technology/ (Amoy)");
}
