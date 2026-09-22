//! Withdrawal request + stub-broadcast failure via `#[sqlx::test]`.
//! Stub clients deliberately refuse broadcast (see `chain::stub`); we assert
//! debit on request and safe reversal when broadcast fails closed.

use bigdecimal::BigDecimal;
use chain::ChainRegistry;
use shared::Coin;
use sqlx::{PgPool, Row};
use std::str::FromStr;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn withdrawal_request_debits_and_stub_broadcast_reverses(pool: PgPool) {
    let registry = ChainRegistry::build("development", true).expect("registry");

    let email = format!("wd-sqlx-{}@bitcosats.test", Uuid::new_v4());
    let password_hash = crypto::hash_password("irrelevant").unwrap();
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(format!("u{}", Uuid::new_v4().simple()))
    .fetch_one(&pool)
    .await
    .expect("insert user");

    let wallet_id: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert wallet");

    let tx_hash = format!("sqlxtx{}", Uuid::new_v4().simple());
    db::deposits::credit_deposit(
        &pool,
        wallet_id,
        Coin::Btc,
        &tx_hash,
        0,
        BigDecimal::from(5_000_000u64),
        6,
    )
    .await
    .expect("credit_deposit");

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
        None,
    )
    .await
    .expect("request_withdrawal");

    assert!(created);
    assert_eq!(withdrawal.status, "QUEUED");
    assert!(!withdrawal.requires_approval);

    let balance_after_debit: BigDecimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1",
    )
    .bind(wallet_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // principal 100_000 + network fee 1_000
    assert_eq!(balance_after_debit, BigDecimal::from(4_899_000u64));

    db::withdrawals::process_broadcast(&pool, withdrawal.id, &registry, None)
        .await
        .expect("process_broadcast");

    let row = sqlx::query("SELECT status::text as status FROM withdrawals WHERE id = $1")
        .bind(withdrawal.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let status: String = row.get("status");
    assert_eq!(status, "FAILED");

    let balance_after_reverse: BigDecimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1",
    )
    .bind(wallet_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(balance_after_reverse, BigDecimal::from(5_000_000u64));

    let failed_events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM outbox_events WHERE aggregate_id = $1 AND event_type = 'WithdrawalFailed'",
    )
    .bind(wallet_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed_events, 1);

    let history = db::withdrawals::list_user_withdrawals(&pool, user_id, Some(Coin::Btc), 20, None)
        .await
        .unwrap();
    assert!(!history.is_empty());
    let history_all = db::withdrawals::list_user_withdrawals(&pool, user_id, None, 20, None)
        .await
        .unwrap();
    assert_eq!(history_all.len(), history.len());

    // Fresh QUEUED for requeue listing + finalize no-op on FAILED
    let (w2, _) = db::withdrawals::request_withdrawal(
        &pool,
        user_id,
        Coin::Btc,
        "bc1qanotherdestinationaddressxxxxxxxxxxxxxxx",
        BigDecimal::from_str("50000").unwrap(),
        client.as_ref(),
        "127.0.0.1",
        None,
        None,
    )
    .await
    .unwrap();
    let eligible = db::withdrawals::list_eligible_for_requeue(&pool, 50)
        .await
        .unwrap();
    assert!(eligible.iter().any(|id| *id == w2.id));
    db::withdrawals::finalize_broadcasted(&pool, withdrawal.id)
        .await
        .unwrap(); // no-op: not BROADCASTED

    // Force BROADCASTED then finalize → CONFIRMED
    sqlx::query("UPDATE withdrawals SET status = 'BROADCASTED'::withdrawal_status WHERE id = $1")
        .bind(w2.id)
        .execute(&pool)
        .await
        .unwrap();
    db::withdrawals::finalize_broadcasted(&pool, w2.id)
        .await
        .unwrap();
    let st2: String = sqlx::query_scalar("SELECT status::text FROM withdrawals WHERE id = $1")
        .bind(w2.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(st2, "CONFIRMED");
}
