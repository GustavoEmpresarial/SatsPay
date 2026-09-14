//! Live smoke test against the multi-provider price oracle
//! (`cargo run -p pricing --example coingecko_live_smoke`).

use pricing::MultiProviderOracle;
use shared::Coin;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let oracle = MultiProviderOracle::default();
    let coins = [Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Bch, Coin::Pol];
    let price_list = oracle.fetch_usd_prices(&coins).await.expect("fetch_usd_prices");

    let prices: HashMap<Coin, f64> = price_list.into_iter().collect();

    println!("\n=== LIVE ORACLE CONSENSUS SPOT PRICES ===");
    for coin in coins {
        let usd = prices.get(&coin).expect("price present for coin");
        println!("🪙 {coin:?} = ${usd:.6} USD");
        assert!(*usd > 0.0, "{coin:?} price must be positive");
    }

    let btc = prices[&Coin::Btc];
    for coin in [Coin::Ltc, Coin::Doge, Coin::Bch, Coin::Pol] {
        assert!(btc > prices[&coin], "BTC price should exceed {coin:?}");
    }

    println!("\n[✔] ALL ORACLE PRICE CONSENSUS ASSERTIONS PASSED!\n");
}
