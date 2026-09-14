//! Manual smoke test for reward programs against a real Postgres (not run in
//! CI — `cargo run -p db --example rewards_smoke`).

use bigdecimal::BigDecimal;
use shared::Coin;
use std::time::Duration;
use uuid::Uuid;

const TEST_PRICE_MAX_STALE: Duration = Duration::from_secs(86_400);

/// Fixture USD prices (scaled 1e8) for this smoke test only — the real feed
/// is `pricing::CoinGeckoClient` (see `crates/pricing/examples`); rewards'
/// weight calc just needs *some* real cached row to read.
async fn seed_price_cache(pool: &sqlx::PgPool) {
    for (coin, price) in [(Coin::Btc, 4_500_000_000_000u64), (Coin::Ltc, 8_000_000_000), (Coin::Doge, 15_000_000), (Coin::Bch, 45_000_000_000), (Coin::Pol, 45_000_000)] {
        sqlx::query("INSERT INTO price_cache (coin, price_scaled, price_decimals, fetched_at) VALUES ($1::coin, $2, 8, now()) ON CONFLICT (coin) DO UPDATE SET price_scaled = EXCLUDED.price_scaled, fetched_at = EXCLUDED.fetched_at")
            .bind(coin.as_str())
            .bind(BigDecimal::from(price))
            .execute(pool)
            .await
            .unwrap();
    }
}

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    db::house::ensure_house_inventory(&pool).await.expect("ensure house inventory");
    seed_price_cache(&pool).await;

    // Seed HOUSE with LTC inventory to fund the reward program.
    let mut tx = pool.begin().await.unwrap();
    let house_ltc = db::house::get_house_wallet_id(&mut tx, Coin::Ltc).await.unwrap();
    db::ledger::apply_ledger_entry(
        &mut tx,
        db::ledger::LedgerCreditInput { reference_key: None, wallet_id: house_ltc, amount: BigDecimal::from(1_000_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed reward pool") },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // --- Two users with different BTC wallet balances (different weights) ---
    let admin_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(format!("rewards-admin-{}@bitcosats.dev", Uuid::new_v4())).bind(crypto::hash_password("x").unwrap()).fetch_one(&pool).await.unwrap();

    let user_a: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(format!("rewards-a-{}@bitcosats.dev", Uuid::new_v4())).bind(crypto::hash_password("x").unwrap()).fetch_one(&pool).await.unwrap();
    let user_a_btc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(user_a).fetch_one(&pool).await.unwrap();
    let user_a_ltc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id").bind(user_a).fetch_one(&pool).await.unwrap();

    let user_b: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(format!("rewards-b-{}@bitcosats.dev", Uuid::new_v4())).bind(crypto::hash_password("x").unwrap()).fetch_one(&pool).await.unwrap();
    let user_b_btc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(user_b).fetch_one(&pool).await.unwrap();
    let user_b_ltc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id").bind(user_b).fetch_one(&pool).await.unwrap();

    // A has 3x the BTC balance of B -> should receive ~3x the reward.
    for (wallet, amount) in [(user_a_btc, 30_000_000u64), (user_b_btc, 10_000_000u64)] {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(&mut tx, db::ledger::LedgerCreditInput { reference_key: None, wallet_id: wallet, amount: BigDecimal::from(amount), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed weight") }).await.unwrap();
        tx.commit().await.unwrap();
    }

    // --- Create an LTC reward program, backdated so it has visible elapsed time ---
    let start_at = chrono::Utc::now() - chrono::Duration::hours(1);
    let program = db::rewards::create_program(&pool, admin_id, Coin::Ltc, Coin::Btc, "BOTH", 240_000_000, Some(start_at), None).await.expect("create_program");
    println!("created program id={} emission_per_day={}", program.id, program.emission_per_day);
    assert!(program.active);

    // Backdate last_emitted_at to match start_at (create_program sets it to start_at already).
    db::rewards::distribute_all_programs(&pool, TEST_PRICE_MAX_STALE).await.expect("distribute_all_programs");

    let rewards_a = db::rewards::get_user_rewards(&pool, user_a).await.unwrap();
    let rewards_b = db::rewards::get_user_rewards(&pool, user_b).await.unwrap();
    println!("user A rewards: {rewards_a:?}");
    println!("user B rewards: {rewards_b:?}");
    assert_eq!(rewards_a.len(), 1);
    assert_eq!(rewards_b.len(), 1);
    assert!(rewards_a[0].received_total > rewards_b[0].received_total, "user A (3x weight) must receive more reward than user B");

    let a_ltc_balance: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1").bind(user_a_ltc).fetch_one(&pool).await.unwrap();
    let b_ltc_balance: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1").bind(user_b_ltc).fetch_one(&pool).await.unwrap();
    println!("user A LTC wallet balance (real ledger credit): {a_ltc_balance}");
    println!("user B LTC wallet balance (real ledger credit): {b_ltc_balance}");
    use bigdecimal::Zero;
    assert!(!a_ltc_balance.is_zero());
    assert!(!b_ltc_balance.is_zero());

    // --- Double-tick within the same instant must not double-pay (no elapsed time) ---
    let a_before = rewards_a[0].received_total;
    db::rewards::distribute_all_programs(&pool, TEST_PRICE_MAX_STALE).await.expect("second tick");
    let rewards_a_after = db::rewards::get_user_rewards(&pool, user_a).await.unwrap();
    println!("user A rewards after immediate re-tick: {} (was {a_before})", rewards_a_after[0].received_total);
    // Elapsed time since the first tick is near-zero but not provably zero in
    // a live test, so just assert it didn't jump by anything close to the
    // first distribution's magnitude.
    assert!(rewards_a_after[0].received_total < a_before * 2, "immediate re-tick must not double-pay a full window's worth");

    // --- Admin can deactivate the program ---
    db::rewards::set_program_active(&pool, program.id, false).await.unwrap();
    let programs = db::rewards::list_programs(&pool).await.unwrap();
    let updated = programs.iter().find(|p| p.id == program.id).unwrap();
    assert!(!updated.active);
    println!("program deactivated: active={}", updated.active);

    println!("\nALL ASSERTIONS PASSED");
}
