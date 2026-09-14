//! Live smoke test: real CoinGecko prices cached into Postgres, then a real
//! swap executed against that cache (not run in CI —
//! `cargo run -p db --example pricing_swap_live_smoke`).

use bigdecimal::BigDecimal;
use pricing::MultiProviderOracle;
use shared::Coin;
use std::time::Duration;
use uuid::Uuid;

const PRICE_DECIMALS: u32 = 8;
const MAX_STALE: Duration = Duration::from_secs(600);

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    db::house::ensure_house_inventory(&pool).await.expect("ensure house inventory");

    // --- Refresh the price cache from the real CoinGecko API ---
    let oracle = MultiProviderOracle::default();
    db::pricing::refresh_all(&pool, &oracle, PRICE_DECIMALS).await.expect("refresh_all");

    let (btc_price, decimals) = db::pricing::get_price(&pool, Coin::Btc, MAX_STALE).await.expect("cached BTC price");
    println!("cached BTC price = {btc_price} (scaled 1e{decimals})");
    assert!(btc_price > 0);

    // Stale-cache rejection: a max_stale of 0 must fail closed even though
    // we just refreshed (no invented fallback price, ever).
    let too_strict = db::pricing::get_price(&pool, Coin::Btc, Duration::ZERO).await;
    assert!(too_strict.is_err(), "zero staleness budget must reject even a fresh cache");
    println!("zero-staleness budget correctly rejected: {:?}", too_strict.err());

    // --- Real swap quote using the live cached price ---
    let quote = db::swap::quote(&pool, Coin::Btc, Coin::Ltc, 10_000_000, MAX_STALE).await.expect("live quote");
    println!("live quote: 0.1 BTC -> {} LTC units (fee={})", quote.to_amount, quote.fee_amount);
    assert!(quote.to_amount > 0);

    // --- Real swap execution against HOUSE liquidity, priced from CoinGecko ---
    for coin in [Coin::Btc, Coin::Ltc] {
        let mut tx = pool.begin().await.unwrap();
        let wallet_id = db::house::get_house_wallet_id(&mut tx, coin).await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id, amount: BigDecimal::from(10_000_000_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let email = format!("pricing-swap-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let user_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&email).bind(crypto::hash_password("x").unwrap()).fetch_one(&pool).await.unwrap();
    let btc_wallet: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();
    let _ltc_wallet: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'LTC', 'PERSONAL') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();
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

    let (swap, created) = db::swap::execute(&pool, user_id, Coin::Btc, Coin::Ltc, 10_000_000, None, "pricing-live-swap-1", MAX_STALE).await.expect("live swap execute");
    println!("real swap executed using live CoinGecko price: from={} to={} fee={}", swap.from_amount, swap.to_amount, swap.fee_amount);
    assert!(created);
    assert!(swap.to_amount > 0);
    assert_eq!(swap.from_amount, 10_000_000);

    println!("\nALL ASSERTIONS PASSED");
}
