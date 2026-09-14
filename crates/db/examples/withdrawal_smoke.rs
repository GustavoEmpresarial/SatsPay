//! Manual smoke test for the withdrawal state machine against a real
//! Postgres (not run in CI — `cargo run -p db --example withdrawal_smoke`).
//! Exercises: credit a deposit, request a withdrawal (debits immediately),
//! process_broadcast via the stub chain client, and asserts the final
//! CONFIRMED state + correct balance.

use bigdecimal::BigDecimal;
use chain::ChainRegistry;
use shared::Coin;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let registry = ChainRegistry::build("development", true).expect("registry");

    let email = format!("withdrawal-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let password_hash = crypto::hash_password("irrelevant-for-this-smoke-test").unwrap();
    let user_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id")
        .bind(&email)
        .bind(&password_hash)
        .fetch_one(&pool)
        .await
        .unwrap();
    let wallet_id: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    println!("user={user_id} wallet={wallet_id}");

    // Credit a deposit well above BTC's min_withdrawal/min_confirmations.
    let tx_hash = format!("smoketx{}", Uuid::new_v4().simple());
    db::deposits::credit_deposit(&pool, wallet_id, Coin::Btc, &tx_hash, 0, BigDecimal::from(5_000_000u64), 6).await.expect("credit_deposit");

    let balance_after_deposit: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1")
        .bind(wallet_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    println!("balance after deposit: {balance_after_deposit}");
    assert_eq!(balance_after_deposit, BigDecimal::from(5_000_000u64));

    // Request a withdrawal below the approval threshold, so it goes straight to QUEUED.
    let client = registry.get(Coin::Btc);
    let (withdrawal, created) = db::withdrawals::request_withdrawal(
        &pool,
        user_id,
        Coin::Btc,
        "bc1qsomevaliddestinationaddressxxxxxxxxxxxxxxx",
        BigDecimal::from_str("100000").unwrap(),
        client.as_ref(),
        "127.0.0.1",
        None,
    )
    .await
    .expect("request_withdrawal");
    println!("withdrawal created={created} id={} status={} requires_approval={}", withdrawal.id, withdrawal.status, withdrawal.requires_approval);
    assert!(created);
    assert_eq!(withdrawal.status, "QUEUED");
    assert!(!withdrawal.requires_approval);

    let balance_after_debit: BigDecimal =
        sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1").bind(wallet_id).fetch_one(&pool).await.unwrap();
    println!("balance after withdrawal debit: {balance_after_debit}");
    assert_eq!(balance_after_debit, BigDecimal::from(4_900_000u64));

    // Idempotency: retrying the exact same request (no idempotency key here,
    // so this actually creates a second withdrawal — that's expected, the
    // idempotency key is opt-in). Instead verify double-processing safety:
    // process_broadcast concurrently from "two workers".
    let pool2 = pool.clone();
    let registry2 = ChainRegistry::build("development", true).unwrap();
    let wid = withdrawal.id;
    let (r1, r2) = tokio::join!(db::withdrawals::process_broadcast(&pool, wid, &registry), db::withdrawals::process_broadcast(&pool2, wid, &registry2));
    r1.expect("process_broadcast 1");
    r2.expect("process_broadcast 2");

    let row = sqlx::query("SELECT status::text as status, tx_hash FROM withdrawals WHERE id = $1").bind(wid).fetch_one(&pool).await.unwrap();
    let status: String = row.get("status");
    let tx_hash: Option<String> = row.get("tx_hash");
    println!("final withdrawal status={status} tx_hash={tx_hash:?}");
    assert_eq!(status, "CONFIRMED");
    assert!(tx_hash.unwrap().starts_with("stub_btc_"));

    // Events are keyed by wallet_id (the Kafka partition key for ordering
    // per-wallet), not withdrawal_id — see DomainEvent::aggregate_id.
    let outbox_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM outbox_events WHERE aggregate_id = $1 AND event_type IN ('WithdrawalBroadcasted', 'WithdrawalConfirmed')",
    )
    .bind(wallet_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    println!("outbox events for this withdrawal's wallet: {outbox_count}");
    assert_eq!(outbox_count, 2, "expected exactly one WithdrawalBroadcasted + one WithdrawalConfirmed event (no duplicates from the concurrent process_broadcast calls)");

    println!("\nALL ASSERTIONS PASSED");
}
