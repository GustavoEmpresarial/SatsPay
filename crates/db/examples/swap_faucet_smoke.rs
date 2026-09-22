//! Manual smoke test for swap + faucet against a real Postgres (not run in
//! CI — `cargo run -p db --example swap_faucet_smoke`).

use bigdecimal::BigDecimal;
use shared::Coin;
use std::time::Duration;
use uuid::Uuid;

const TEST_PRICE_MAX_STALE: Duration = Duration::from_secs(86_400);

/// Fixture USD prices (scaled 1e8) for this smoke test only — the real feed
/// is `pricing::CoinGeckoClient` (see `crates/pricing/examples`); swap's
/// quote math just needs *some* real cached row to read.
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

    // Seed HOUSE with LTC and BTC liquidity so swap payouts don't fail closed.
    for coin in [Coin::Btc, Coin::Ltc] {
        let mut tx = pool.begin().await.unwrap();
        let wallet_id = db::house::get_house_wallet_id(&mut tx, coin).await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None,
                wallet_id,
                amount: BigDecimal::from(10_000_000_000_000u64),
                ledger_type: "ADJUSTMENT",
                reference_id: None,
                reference_type: Some("SmokeTestSeed"),
                memo: Some("seed"),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let email = format!("swap-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let password_hash = crypto::hash_password("irrelevant").unwrap();
    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&email).bind(&password_hash).fetch_one(&pool).await.unwrap();
    let btc_wallet: Uuid =
        sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();
    let _ltc_wallet: Uuid =
        sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();

    // Credit the user with BTC to swap from.
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None,
                wallet_id: btc_wallet,
                amount: BigDecimal::from(100_000_000u64), // 1 BTC
                ledger_type: "ADJUSTMENT",
                reference_id: None,
                reference_type: Some("SmokeTestSeed"),
                memo: Some("seed user"),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // --- Swap: BTC -> LTC ---
    let (swap, created) = db::swap::execute(&pool, user_id, Coin::Btc, Coin::Ltc, 10_000_000, None, "smoke-swap-1", TEST_PRICE_MAX_STALE).await.expect("swap execute");
    println!("swap created={created} from={} to={} fee={}", swap.from_amount, swap.to_amount, swap.fee_amount);
    assert!(created);
    assert!(swap.to_amount > 0);

    // Idempotency: same key returns the same swap, no double-debit.
    let (swap2, created2) = db::swap::execute(&pool, user_id, Coin::Btc, Coin::Ltc, 10_000_000, None, "smoke-swap-1", TEST_PRICE_MAX_STALE).await.expect("swap execute replay");
    assert!(!created2);
    assert_eq!(swap.id, swap2.id);

    let btc_balance: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1").bind(btc_wallet).fetch_one(&pool).await.unwrap();
    println!("btc balance after ONE swap (replay must not double-debit): {btc_balance}");
    assert_eq!(btc_balance, BigDecimal::from(100_000_000u64 - 10_000_000));

    // --- Faucet claim ---
    let claim = db::faucet::claim(&pool, user_id, Coin::Btc, "203.0.113.5", 60).await.expect("faucet claim");
    println!("faucet claimed amount={} coin={:?}", claim.amount, claim.coin);
    assert!(claim.amount > 0);

    let cooldown_err = db::faucet::claim(&pool, user_id, Coin::Btc, "203.0.113.5", 60).await;
    assert!(cooldown_err.is_err(), "second claim within cooldown must fail");
    println!("second claim correctly rejected: {:?}", cooldown_err.err());

    // Second account on the same IP is allowed — cooldown is per user_id+coin.
    let email_b = format!("faucet-peer-{}@bitcosats.dev", Uuid::new_v4());
    let user_b: Uuid =
        sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&email_b).bind(&password_hash).fetch_one(&pool).await.unwrap();
    let _btc_b: Uuid =
        sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(user_b).fetch_one(&pool).await.unwrap();
    let peer = db::faucet::claim(&pool, user_b, Coin::Btc, "203.0.113.5", 60).await.expect("peer claim same IP");
    assert!(peer.amount > 0);
    println!("same-IP peer claim succeeded amount={}", peer.amount);

    println!("\nALL ASSERTIONS PASSED");
}
