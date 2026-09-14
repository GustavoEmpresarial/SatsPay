//! Manual smoke test for `RealChainClient` against the REAL public network
//! (Bitcore API for BTC/LTC/DOGE/BCH, a public Polygon RPC for POL) — no
//! testnet, no mocks. Not run in CI (needs internet + a Postgres for the HD
//! index sequences) — `cargo run -p chain --example real_client_live_smoke`.
//!
//! Uses well-known, independently-verifiable, currently-funded mainnet
//! addresses for the balance/deposit checks, so a nonzero result is a real
//! signal the client is actually talking to each network correctly — not
//! just internally self-consistent.

use chain::real_client::{RealChainClient, RealClientConfig};
use chain::ChainClient;
use shared::Coin;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(&database_url).await.expect("connect");

    let config = RealClientConfig {
        bitcore_base_url: "https://api.bitcore.io".to_string(),
        evm_rpc_url: "https://polygon-bor-rpc.publicnode.com".to_string(),
        // A real, published BIP32 xpub — Bitcoin Wiki's BIP32 test vector 1
        // master xpub. Used here only to prove address derivation works;
        // production would use a wallet-specific xpub from real cold storage.
        deposit_xpub: "xpub661MyMwAqRbcFtXgS5sYJABqqG9YLmC4Q1Rdap9gSE8NqtwybGhePY2gZ29ESFjqJoCu1Rupje8YtGqsefD265TMg7usUDFdp6W1EGMcet8".to_string(),
        hot_wallet_wif: None,
        evm_deposit_lookback_blocks: 5,
        fee_confirmation_target: 2,
        network: chain::ChainNetwork::Mainnet,
        sol_rpc_url: "http://62.171.138.114:8899".to_string(),
        dgb_insight_url: "https://digiexplorer.info/api".to_string(),
        deposit_mnemonic: None,
        hot_mnemonic: None,
    };

    // --- BTC: known real, currently-funded mainnet address ---
    let btc = RealChainClient::new(Coin::Btc, pool.clone(), config.clone());
    let addr = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    assert!(btc.validate_address(addr), "real BTC address must validate");
    assert!(!btc.validate_address("not-a-real-address"), "garbage must be rejected");
    let balance = btc.get_balance(addr).await.expect("live BTC balance query");
    println!("BTC balance of {addr}: {balance} sats (real network query)");
    assert!(balance > 0, "this address is known to hold funds");
    let deposits = btc.fetch_deposits(addr).await.expect("live BTC deposit history query");
    println!("BTC fetch_deposits returned {} real UTXOs ever received", deposits.len());
    assert!(!deposits.is_empty());

    // --- BTC: real HD address generation, using the coin's own Postgres
    // sequence for the derivation index (not a hash of anything) ---
    let generated_1 = btc.generate_address("live-smoke-user").await.expect("generate_address");
    let generated_2 = btc.generate_address("live-smoke-user").await.expect("generate_address");
    println!("BTC generated addresses: {} , {}", generated_1.address, generated_2.address);
    assert_ne!(generated_1.address, generated_2.address, "sequential HD indices must never repeat the same address");
    assert!(btc.validate_address(&generated_1.address));
    assert!(btc.validate_address(&generated_2.address));

    // --- LTC: a real address freshly paid by the current tip block's
    // coinbase transaction (fetched live from the same API right before
    // writing this test — guaranteed funded and current, not a guessed or
    // recalled-from-memory address). ---
    let ltc = RealChainClient::new(Coin::Ltc, pool.clone(), config.clone());
    let ltc_addr = "ltc1qtw0lpdvk6w7p0eupw7xuqszex6muxkjglz4sw0";
    let ltc_balance = ltc.get_balance(ltc_addr).await.expect("live LTC balance query");
    println!("LTC balance of {ltc_addr}: {ltc_balance} litoshi (real network query)");
    assert!(ltc_balance > 0, "this address was freshly paid by the current tip block's coinbase");

    // --- DOGE: same approach — current tip block's coinbase payout address. ---
    let doge = RealChainClient::new(Coin::Doge, pool.clone(), config.clone());
    let doge_addr = "DTxRXpG63mq4UwroHJUcuGFwjX9myKj9Gm";
    let doge_balance = doge.get_balance(doge_addr).await.expect("live DOGE balance query");
    println!("DOGE balance of {doge_addr}: {doge_balance} (real network query)");
    assert!(doge_balance > 0, "this address was freshly paid by the current tip block's coinbase");

    // --- BCH: CashAddr spec's own worked example, independently confirmed funded ---
    let bch = RealChainClient::new(Coin::Bch, pool.clone(), config.clone());
    let bch_addr = "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a";
    assert!(bch.validate_address(bch_addr));
    let bch_balance = bch.get_balance(bch_addr).await.expect("live BCH balance query");
    println!("BCH balance of {bch_addr}: {bch_balance} (real network query)");
    assert!(bch_balance > 0);

    // --- POL: real JSON-RPC eth_getBalance against a well-known funded contract/address ---
    let pol = RealChainClient::new(Coin::Pol, pool.clone(), config.clone());
    let pol_addr = "0x000000000000000000000000000000000000dead"; // the well-known "burn" address (all-lowercase form, always spec-valid), always nonzero on active chains
    assert!(pol.validate_address(pol_addr));
    let pol_balance = pol.get_balance(pol_addr).await.expect("live POL balance query");
    println!("POL balance of burn address: {pol_balance} wei (real JSON-RPC query)");

    println!("\nALL LIVE NETWORK ASSERTIONS PASSED");
}
