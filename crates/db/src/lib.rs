pub mod aave_sync;
pub mod admin;
pub mod airdrop;
pub mod audit;
pub mod auth;
pub mod deposits;
pub mod faucet;
pub mod faucetlist;
pub mod house;
pub mod ledger;
pub mod lend;
pub mod merchant;
pub mod merchant_deposits;
pub mod merchant_multicoin;
pub mod merchant_settings;
pub mod network_fees;
pub mod oauth;
pub mod pricing;
pub mod privacy;
pub mod public_api;
pub mod referral;
pub mod rewards;
pub mod stake;
pub mod dex_swap;
pub mod support;
pub mod swap;
pub mod telemetry;
pub mod treasury_health;
pub mod wallet;
pub mod withdrawals;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

pub async fn connect(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new().max_connections(10).connect(database_url).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    let mut migrator = sqlx::migrate!("./migrations");
    // `0007_price_cache.sql` is already applied on existing databases but its
    // file is not present in this tree; tolerate the recorded-but-missing
    // version instead of refusing to start. New (higher-numbered) migrations
    // still run normally.
    migrator.set_ignore_missing(true);
    migrator.run(pool).await?;
    Ok(())
}
