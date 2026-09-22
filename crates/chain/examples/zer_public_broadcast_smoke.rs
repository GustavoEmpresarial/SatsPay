//! Smoke: sign ZER withdraw with local zerod + broadcast via public Insight.
//! Env: HOT_MNEMONIC, ZER_RPC_URL. Optional: TO_ADDR (default: change to self 0.01 ZER).
//! Never prints mnemonic/WIF.

use chain::hd_wallet::{hot_address_from_mnemonic, hot_secret_from_mnemonic, secret_to_wif};
use chain::params::ChainNetwork;
use chain::zer_client::ZerClient;
use shared::Coin;

#[tokio::main]
async fn main() {
    let mnemonic = std::env::var("HOT_MNEMONIC").expect("HOT_MNEMONIC");
    let rpc = std::env::var("ZER_RPC_URL").expect("ZER_RPC_URL");
    let explorer = std::env::var("ZER_EXPLORER_API").unwrap_or_else(|_| "https://zerochain.info/api".into());
    let hot = hot_address_from_mnemonic(&mnemonic, Coin::Zer, ChainNetwork::Mainnet).expect("hot addr");
    println!("hot_address={hot}");
    let wif = secret_to_wif(&hot_secret_from_mnemonic(&mnemonic, Coin::Zer).unwrap()).unwrap();
    let client = ZerClient::with_rpc(&explorer, None, Some(&rpc));
    let to = std::env::var("TO_ADDR").unwrap_or_else(|_| hot.clone());
    let amount: u64 = std::env::var("AMOUNT_SATS").ok().and_then(|s| s.parse().ok()).unwrap_or(1_000_000); // 0.01 ZER
    match client.broadcast_signed(&wif, &to, amount, &hot).await {
        Ok((txid, fee)) => println!("OK txid={txid} fee={fee}"),
        Err(e) => {
            eprintln!("FAIL {}", e.message);
            std::process::exit(1);
        }
    }
}
