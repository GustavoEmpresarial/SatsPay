//! Shared helpers for `#[sqlx::test]` integration tests in this crate.
//! Each test binary uses a different subset, hence the crate-wide allow.
#![allow(dead_code)]

use bigdecimal::BigDecimal;
use shared::Coin;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn seed_price_cache(pool: &PgPool) {
    for (coin, price) in [
        (Coin::Btc, 4_500_000_000_000u64),
        (Coin::Ltc, 8_000_000_000),
        (Coin::Doge, 15_000_000),
        (Coin::Bch, 45_000_000_000),
        (Coin::Pol, 45_000_000),
        (Coin::Dgb, 1_000_000),
        (Coin::Sol, 15_000_000_000),
        (Coin::Usdt, 100_000_000),
        (Coin::Usdc, 100_000_000),
        (Coin::Zer, 1_000_000),
        (Coin::Pepe, 400),
    ] {
        sqlx::query(
            "INSERT INTO price_cache (coin, price_scaled, price_decimals, fetched_at) \
             VALUES ($1::coin, $2, 8, now()) \
             ON CONFLICT (coin) DO UPDATE SET price_scaled = EXCLUDED.price_scaled, fetched_at = EXCLUDED.fetched_at",
        )
        .bind(coin.as_str())
        .bind(BigDecimal::from(price))
        .execute(pool)
        .await
        .expect("seed price");
    }
}

pub async fn insert_user(pool: &PgPool, prefix: &str) -> Uuid {
    let email = format!("{prefix}-{}@bitcosats.test", Uuid::new_v4());
    let password_hash = crypto::hash_password("irrelevant").unwrap();
    let username = format!("u{}", Uuid::new_v4().simple());
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(&username)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

pub async fn insert_personal_wallet(pool: &PgPool, user_id: Uuid, coin: Coin) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, 'PERSONAL') RETURNING id",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .fetch_one(pool)
    .await
    .expect("insert wallet")
}

pub async fn credit_wallet(pool: &PgPool, wallet_id: Uuid, amount: u64, memo: &str) {
    let mut tx = pool.begin().await.unwrap();
    db::ledger::apply_ledger_entry(
        &mut tx,
        db::ledger::LedgerCreditInput {
            reference_key: None,
            wallet_id,
            amount: BigDecimal::from(amount),
            ledger_type: "ADJUSTMENT",
            reference_id: None,
            reference_type: Some("SqlxTestSeed"),
            memo: Some(memo),
        },
    )
    .await
    .expect("credit");
    tx.commit().await.unwrap();
}

#[allow(dead_code)] // used by swap_faucet_sqlx; each test binary compiles common separately
pub async fn seed_house_liquidity(pool: &PgPool, coins: &[Coin], amount: u64) {
    for &coin in coins {
        let mut tx = pool.begin().await.unwrap();
        let wallet_id = db::house::get_house_wallet_id(&mut tx, coin).await.expect("house wallet");
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput {
                reference_key: None,
                wallet_id,
                amount: BigDecimal::from(amount),
                ledger_type: "ADJUSTMENT",
                reference_id: None,
                reference_type: Some("SqlxTestSeed"),
                memo: Some("house seed"),
            },
        )
        .await
        .expect("house credit");
        tx.commit().await.unwrap();
    }
}

pub async fn wallet_balance(pool: &PgPool, wallet_id: Uuid) -> BigDecimal {
    sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1")
        .bind(wallet_id)
        .fetch_one(pool)
        .await
        .unwrap()
}
