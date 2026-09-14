//! Minimal ledger smoke: credit + debit in one tx must leave SUM(amount)=0.
//! Uses `#[sqlx::test]` with this crate's `./migrations` (requires a running
//! Postgres reachable via `DATABASE_URL` — same as other sqlx migrate tests).

use bigdecimal::BigDecimal;
use db::ledger::{apply_ledger_entry, get_wallet_balance, LedgerCreditInput};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn ledger_credit_debit_sums_to_zero(pool: PgPool) {
    let email = format!("ledger-smoke-{}@bitcosats.test", Uuid::new_v4());
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&email)
    .bind("test-hash-not-verified")
    .bind(format!("u{}", Uuid::new_v4().simple()))
    .fetch_one(&pool)
    .await
    .expect("insert user");

    let wallet_id: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("insert wallet");

    let amount = BigDecimal::from(1_000_000u64);
    let mut tx = pool.begin().await.expect("begin");

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            reference_key: None,
            wallet_id,
            amount: amount.clone(),
            ledger_type: "ADJUSTMENT",
            reference_id: Some(Uuid::new_v4()),
            reference_type: Some("LedgerSmoke"),
            memo: Some("smoke credit"),
        },
    )
    .await
    .expect("credit");

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            reference_key: None,
            wallet_id,
            amount: -amount,
            ledger_type: "ADJUSTMENT",
            reference_id: Some(Uuid::new_v4()),
            reference_type: Some("LedgerSmoke"),
            memo: Some("smoke debit"),
        },
    )
    .await
    .expect("debit");

    let balance = get_wallet_balance(&mut tx, wallet_id).await.expect("balance");
    tx.commit().await.expect("commit");

    assert_eq!(balance, BigDecimal::from(0));

    let sum: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM ledger_entries WHERE wallet_id = $1")
        .bind(wallet_id)
        .fetch_one(&pool)
        .await
        .expect("sum");
    assert_eq!(sum, BigDecimal::from(0));
}
