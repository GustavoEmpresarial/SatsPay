//! pricing.refresh_all (stub oracle) + swap list + faucetlist delete.

mod common;

use async_trait::async_trait;
use pricing::{MultiProviderOracle, PriceProvider};
use shared::Coin;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;

struct FixedOracle;

#[async_trait]
impl PriceProvider for FixedOracle {
    fn name(&self) -> &'static str {
        "fixed"
    }
    async fn fetch_prices(&self, coins: &[Coin]) -> Result<HashMap<Coin, f64>, String> {
        Ok(coins.iter().map(|&c| (c, 100.0)).collect())
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn pricing_refresh_all_with_stub_oracle(pool: PgPool) {
    let oracle = MultiProviderOracle::new(vec![Box::new(FixedOracle)]);
    db::pricing::refresh_all(&pool, &oracle, 8).await.unwrap();
    let (scaled, dec) = db::pricing::get_price(&pool, Coin::Btc, Duration::from_secs(60))
        .await
        .unwrap();
    assert!(scaled > 0);
    assert_eq!(dec, 8);
    let (d, map) = db::pricing::list_cached_prices(&pool).await.unwrap();
    assert_eq!(d, 8);
    assert!(map.len() >= 5);
}

#[sqlx::test(migrations = "./migrations")]
async fn swap_list_and_faucetlist_delete(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;
    common::seed_house_liquidity(&pool, &[Coin::Btc, Coin::Ltc], 1_000_000_000_000).await;

    let user = common::insert_user(&pool, "swx").await;
    let btc = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    let _ltc = common::insert_personal_wallet(&pool, user, Coin::Ltc).await;
    common::credit_wallet(&pool, btc, 5_000_000, "seed").await;

    let q = db::swap::quote(&pool, Coin::Btc, Coin::Ltc, 1_000_000, Duration::from_secs(86_400))
        .await
        .unwrap();
    let _ = db::swap::execute(
        &pool,
        user,
        Coin::Btc,
        Coin::Ltc,
        1_000_000,
        Some(q.to_amount / 2),
        "sqlx-extra-1",
        Duration::from_secs(86_400),
    )
    .await
    .unwrap();
    let hist = db::swap::list_user_swaps(&pool, user, 20).await.unwrap();
    assert!(!hist.is_empty());

    let site = db::faucetlist::create_site(
        &pool,
        user,
        "Tmp",
        "https://tmp.example",
        "d",
        &["BTC"],
        None,
    )
    .await
    .unwrap();
    db::faucetlist::delete_site(&pool, user, site).await.unwrap();
    assert!(db::faucetlist::delete_site(&pool, user, site).await.is_err());
}
