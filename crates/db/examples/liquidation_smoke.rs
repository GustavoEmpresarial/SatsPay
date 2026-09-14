//! Manual smoke test for lend liquidation against a real Postgres (not run
//! in CI — `cargo run -p db --example liquidation_smoke`).

use bigdecimal::BigDecimal;
use shared::Coin;
use std::time::Duration;
use uuid::Uuid;

const CLOSE_FACTOR_BPS: u32 = 5000;
const TEST_PRICE_MAX_STALE: Duration = Duration::from_secs(86_400);

/// Fixture USD prices (scaled 1e8) for this smoke test only — the real feed
/// is `pricing::CoinGeckoClient` (see `crates/pricing/examples`); lend's
/// health-factor math just needs *some* real cached row to read.
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
    db::lend::ensure_lend_reserves(&pool).await.expect("ensure lend reserves");
    seed_price_cache(&pool).await;

    // Seed LEND_POOL liquidity for BTC and LTC.
    for coin in [Coin::Btc, Coin::Ltc] {
        let mut tx = pool.begin().await.unwrap();
        let wallet_id = db::house::get_lend_pool_wallet_id(&mut tx, coin).await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id, amount: BigDecimal::from(10_000_000_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // --- Borrower: supplies BTC as collateral, borrows LTC near the limit ---
    let borrower_email = format!("liq-borrower-{}@bitcosats.dev", Uuid::new_v4());
    let password_hash = crypto::hash_password("irrelevant").unwrap();
    let borrower_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&borrower_email).bind(&password_hash).fetch_one(&pool).await.unwrap();
    let borrower_btc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(borrower_id).fetch_one(&pool).await.unwrap();
    let _borrower_ltc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id").bind(borrower_id).fetch_one(&pool).await.unwrap();
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id: borrower_btc, amount: BigDecimal::from(100_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed borrower") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    db::lend::supply(&pool, borrower_id, Coin::Btc, 100_000_000).await.expect("borrower supply BTC");
    // BTC collateral_factor_bps = 7500 (from shared::lend_market). Borrow close to the limit.
    let borrowed = db::lend::borrow(&pool, borrower_id, Coin::Ltc, 4_000_000_000, TEST_PRICE_MAX_STALE).await.expect("borrower borrow LTC near limit");
    println!("borrower borrowed LTC={borrowed}");

    let positions_before = db::lend::get_user_positions(&pool, borrower_id, TEST_PRICE_MAX_STALE).await.unwrap();
    println!("borrower health factor before price move: {:?} bps", positions_before.health_factor_bps);
    assert!(positions_before.health_factor_bps.unwrap() >= 10_000, "position should start healthy");

    // Simulate a collateral price crash by directly slashing the borrower's
    // scaled BTC supply in half (equivalent in effect to BTC losing ~50%
    // value against LTC) — the cleanest way to make a position unhealthy in
    // a smoke test without needing to fake the price oracle itself.
    sqlx::query("UPDATE lend_positions SET scaled_supply = scaled_supply / 20 WHERE user_id = $1 AND coin = 'BTC'::coin").bind(borrower_id).execute(&pool).await.unwrap();

    let positions_after = db::lend::get_user_positions(&pool, borrower_id, TEST_PRICE_MAX_STALE).await.unwrap();
    println!("borrower health factor after simulated crash: {:?} bps", positions_after.health_factor_bps);
    assert!(positions_after.health_factor_bps.unwrap() < 10_000, "position should now be unhealthy");

    let liquidatable = db::lend::list_liquidatable_positions(&pool, TEST_PRICE_MAX_STALE).await.unwrap();
    assert!(liquidatable.iter().any(|p| p.user_id == borrower_id), "borrower must show up as liquidatable");
    println!("liquidatable positions: {}", liquidatable.len());

    // --- Liquidator: repays LTC debt, seizes BTC collateral at a bonus ---
    let liquidator_email = format!("liq-liquidator-{}@bitcosats.dev", Uuid::new_v4());
    let liquidator_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&liquidator_email).bind(&password_hash).fetch_one(&pool).await.unwrap();
    let liquidator_ltc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id").bind(liquidator_id).fetch_one(&pool).await.unwrap();
    let liquidator_btc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(liquidator_id).fetch_one(&pool).await.unwrap();
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id: liquidator_ltc, amount: BigDecimal::from(10_000_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed liquidator") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // Self-liquidation must be rejected.
    let self_liq = db::lend::liquidate(&pool, borrower_id, borrower_id, Coin::Ltc, Coin::Btc, 1_000_000, CLOSE_FACTOR_BPS, TEST_PRICE_MAX_STALE).await;
    assert!(self_liq.is_err());
    println!("self-liquidation correctly rejected: {:?}", self_liq.err());

    let result = db::lend::liquidate(&pool, liquidator_id, borrower_id, Coin::Ltc, Coin::Btc, 2_000_000_000, CLOSE_FACTOR_BPS, TEST_PRICE_MAX_STALE).await.expect("liquidate");
    println!("liquidation result: repaid={} seized={}", result.repaid, result.seized);
    assert!(result.repaid > 0 && result.seized > 0);

    let liquidator_btc_balance: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1").bind(liquidator_btc).fetch_one(&pool).await.unwrap();
    println!("liquidator BTC balance after seizing collateral: {liquidator_btc_balance}");
    use bigdecimal::Zero;
    assert!(!liquidator_btc_balance.is_zero());

    // Healthy position must reject liquidation.
    let healthy_borrower_email = format!("liq-healthy-{}@bitcosats.dev", Uuid::new_v4());
    let healthy_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&healthy_borrower_email).bind(&password_hash).fetch_one(&pool).await.unwrap();
    let healthy_btc: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(healthy_id).fetch_one(&pool).await.unwrap();
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id: healthy_btc, amount: BigDecimal::from(50_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed healthy") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    db::lend::supply(&pool, healthy_id, Coin::Btc, 50_000_000).await.unwrap();
    let healthy_liq = db::lend::liquidate(&pool, liquidator_id, healthy_id, Coin::Ltc, Coin::Btc, 1_000, CLOSE_FACTOR_BPS, TEST_PRICE_MAX_STALE).await;
    assert!(healthy_liq.is_err(), "liquidating a position with no debt must fail");
    println!("liquidation of no-debt position correctly rejected: {:?}", healthy_liq.err());

    println!("\nALL ASSERTIONS PASSED");
}
