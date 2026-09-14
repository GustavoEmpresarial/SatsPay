//! Postgres-backed cache in front of the real CoinGecko price feed
//! (`pricing::CoinGeckoClient`). A background worker task calls
//! `refresh_all` on a fixed interval; request-path code (swap quote/execute)
//! only ever reads `get_price`, which fails closed if the cache is missing
//! or older than the caller's staleness budget — no invented fallback price.

use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::{DateTime, Utc};
use pricing::MultiProviderOracle;
use shared::Coin;
use sqlx::{PgPool, Postgres, Row};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum PricingDbError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Fetch(#[from] pricing::PricingError),
    #[error("no cached price for {0:?}")]
    NoCachedPrice(Coin),
    #[error("cached price for {0:?} is stale (age {1:?} exceeds budget {2:?})")]
    StalePrice(Coin, Duration, Duration),
}

const ALL_COINS: &[Coin] = &shared::COINS;

/// Fetches live USD prices for all coins and overwrites `price_cache`.
/// Called by the worker on a `PRICE_REFRESH_INTERVAL_SECS` tick.
pub async fn refresh_all(pool: &PgPool, oracle: &MultiProviderOracle, price_decimals: u32) -> Result<(), PricingDbError> {
    let prices = oracle.fetch_usd_prices(ALL_COINS).await?;
    let scale = 10u128.pow(price_decimals);
    let now = Utc::now();

    let mut tx = pool.begin().await?;
    for (coin, usd) in prices {
        let scaled = (usd * scale as f64).round() as i128;
        sqlx::query(
            r#"
            INSERT INTO price_cache (coin, price_scaled, price_decimals, fetched_at)
            VALUES ($1::coin, $2, $3, $4)
            ON CONFLICT (coin) DO UPDATE SET price_scaled = EXCLUDED.price_scaled, price_decimals = EXCLUDED.price_decimals, fetched_at = EXCLUDED.fetched_at
            "#,
        )
        .bind(coin.as_str())
        .bind(BigDecimal::from(scaled))
        .bind(price_decimals as i32)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Reads the cached price for `coin`, failing closed (never guessing a
/// number) if there is no cached row or it's older than `max_stale`. Generic
/// over the executor so callers already inside a transaction (lend/rewards)
/// can read a consistent snapshot without a separate pool connection.
pub async fn get_price<'e, E>(executor: E, coin: Coin, max_stale: Duration) -> Result<(u128, u32), PricingDbError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let row = sqlx::query("SELECT price_scaled, price_decimals, fetched_at FROM price_cache WHERE coin = $1::coin").bind(coin.as_str()).fetch_optional(executor).await?;
    let Some(row) = row else { return Err(PricingDbError::NoCachedPrice(coin)) };

    let fetched_at: DateTime<Utc> = row.get("fetched_at");
    let age = (Utc::now() - fetched_at).to_std().unwrap_or(Duration::ZERO);
    if age > max_stale {
        return Err(PricingDbError::StalePrice(coin, age, max_stale));
    }

    let price_scaled: BigDecimal = row.get("price_scaled");
    let price_decimals: i32 = row.get("price_decimals");
    let price_u128 = price_scaled.to_u128().ok_or(PricingDbError::NoCachedPrice(coin))?;
    Ok((price_u128, price_decimals as u32))
}

/// A snapshot of every coin's cached price, fetched once per call site
/// (lend/rewards operate over multiple coins per transaction).
pub struct PriceTable {
    prices: HashMap<Coin, (u128, u32)>,
}

impl PriceTable {
    /// USD value scaled by the table's `price_decimals`, consistent across
    /// coins — `amount * price / 10^coin_decimals`.
    pub fn value_usd(&self, coin: Coin, amount: u128, coin_decimals: u32) -> u128 {
        let (price, _price_decimals) = self.prices.get(&coin).copied().unwrap_or((0, 0));
        let result = (num_bigint::BigUint::from(amount) * num_bigint::BigUint::from(price)) / num_bigint::BigUint::from(10u128.pow(coin_decimals));
        num_traits::ToPrimitive::to_u128(&result).unwrap_or(u128::MAX)
    }

    /// The raw cached price (scaled by `price_decimals`) for `coin`.
    pub fn price_scaled(&self, coin: Coin) -> u128 {
        self.prices.get(&coin).copied().map(|(price, _)| price).unwrap_or(0)
    }
}

/// Loads a `PriceTable` covering every coin, failing closed if any coin's
/// price is missing or stale — a partial price table would let liquidation /
/// health-factor math silently treat a coin as worthless.
pub async fn load_price_table(tx: &mut sqlx::Transaction<'_, Postgres>, max_stale: Duration) -> Result<PriceTable, PricingDbError> {
    let mut prices = HashMap::with_capacity(ALL_COINS.len());
    for &coin in ALL_COINS {
        prices.insert(coin, get_price(&mut **tx, coin, max_stale).await?);
    }
    Ok(PriceTable { prices })
}

/// Best-effort snapshot for the UI price ticker — returns whatever is in
/// `price_cache` without the fail-closed stale check (swap execution still
/// uses `get_price` / `load_price_table`). Missing coins are omitted.
pub async fn list_cached_prices(pool: &PgPool) -> Result<(u32, HashMap<Coin, u128>), PricingDbError> {
    let rows = sqlx::query("SELECT coin::text AS coin, price_scaled, price_decimals FROM price_cache").fetch_all(pool).await?;
    let mut prices = HashMap::with_capacity(rows.len());
    let mut decimals = 8u32;
    for row in rows {
        let coin_str: String = row.get("coin");
        let Ok(coin) = coin_str.parse::<Coin>() else { continue };
        let price_scaled: BigDecimal = row.get("price_scaled");
        let price_decimals: i32 = row.get("price_decimals");
        decimals = price_decimals as u32;
        if let Some(scaled) = price_scaled.to_u128() {
            prices.insert(coin, scaled);
        }
    }
    Ok((decimals, prices))
}
