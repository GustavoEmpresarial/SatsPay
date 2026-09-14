//! Manual smoke test for stake + lend against a real Postgres (not run in CI
//! — `cargo run -p db --example stake_lend_smoke`).

use bigdecimal::BigDecimal;
use shared::Coin;
use std::time::Duration;
use uuid::Uuid;

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

const TEST_PRICE_MAX_STALE: Duration = Duration::from_secs(86_400);

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    db::house::ensure_house_inventory(&pool).await.expect("ensure house inventory");
    db::lend::ensure_lend_reserves(&pool).await.expect("ensure lend reserves");
    seed_price_cache(&pool).await;

    // Seed HOUSE (for stake rewards) and LEND_POOL (for lend liquidity) with BTC/LTC.
    for coin in [Coin::Btc, Coin::Ltc] {
        for kind in ["HOUSE", "LEND_POOL"] {
            let mut tx = pool.begin().await.unwrap();
            let wallet_id: Uuid = sqlx::query_scalar("SELECT w.id FROM wallets w JOIN users u ON u.id = w.user_id WHERE u.email = $1 AND w.coin = $2::coin AND w.kind = $3::wallet_kind")
                .bind(db::house::HOUSE_EMAIL)
                .bind(coin.as_str())
                .bind(kind)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            db::ledger::apply_ledger_entry(
                &mut tx,
                db::ledger::LedgerCreditInput { reference_key: None, wallet_id, amount: BigDecimal::from(10_000_000_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed") },
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
        }
    }

    let email = format!("stakelend-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let password_hash = crypto::hash_password("irrelevant").unwrap();
    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&email).bind(&password_hash).fetch_one(&pool).await.unwrap();
    let btc_wallet: Uuid =
        sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();
    let _ltc_wallet: Uuid =
        sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();

    // Seed user BTC balance.
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id: btc_wallet, amount: BigDecimal::from(100_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed user") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // --- Stake: flexible plan (lockDays=0), claim immediately ---
    let stake = db::stake::create_stake(&pool, user_id, Coin::Btc, 10_000_000, 0).await.expect("create_stake");
    println!("stake created id={} status={}", stake.id, stake.status);
    assert_eq!(stake.status, "ACTIVE");

    let btc_after_lock: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1").bind(btc_wallet).fetch_one(&pool).await.unwrap();
    assert_eq!(btc_after_lock, BigDecimal::from(90_000_000u64));

    let claimed = db::stake::claim_stake(&pool, user_id, stake.id).await.expect("claim_stake");
    println!("stake claimed status={} reward={}", claimed.status, claimed.reward);
    assert_eq!(claimed.status, "COMPLETED");

    // Double-claim must fail (not active anymore).
    let double_claim = db::stake::claim_stake(&pool, user_id, stake.id).await;
    assert!(double_claim.is_err(), "double claim must fail");
    println!("double-claim correctly rejected: {:?}", double_claim.err());

    // --- Lend: supply BTC as collateral, borrow LTC against it, repay, withdraw ---
    let supplied = db::lend::supply(&pool, user_id, Coin::Btc, 50_000_000).await.expect("lend supply");
    println!("lend supplied={supplied}");

    let borrowed = db::lend::borrow(&pool, user_id, Coin::Ltc, 1_000_000, TEST_PRICE_MAX_STALE).await.expect("lend borrow (within collateral limit)");
    println!("lend borrowed={borrowed}");

    // Borrowing far beyond collateral value must fail closed.
    let over_borrow = db::lend::borrow(&pool, user_id, Coin::Ltc, 100_000_000_000, TEST_PRICE_MAX_STALE).await;
    assert!(over_borrow.is_err(), "over-collateral borrow must fail");
    println!("over-borrow correctly rejected: {:?}", over_borrow.err());

    let repaid = db::lend::repay(&pool, user_id, Coin::Ltc, 1_000_000).await.expect("lend repay");
    println!("lend repaid={repaid}");

    let withdrawn = db::lend::withdraw(&pool, user_id, Coin::Btc, 50_000_000, TEST_PRICE_MAX_STALE).await.expect("lend withdraw (debt cleared, safe)");
    println!("lend withdrawn={withdrawn}");
    assert!(withdrawn > 0);

    println!("\nALL ASSERTIONS PASSED");
}
