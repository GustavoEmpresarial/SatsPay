//! Withdrawal money paths the stub broadcast can't reach: idempotent replay
//! (sequential and racing), request guards, approval threshold, successful and
//! ambiguous broadcasts, and the address book's ownership rules.

mod common;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chain::{BroadcastError, BroadcastResult, ChainClient, ChainError, ChainRegistry, GeneratedAddress, OnchainTx};
use db::withdrawals::{self as wd, WithdrawalsError};
use shared::{coin_config, Coin};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

const DEST: &str = "bc1qvaliddestinationforwithdrawaltests00000";

#[derive(Clone, Copy)]
enum Outcome {
    Sent,
    Ambiguous,
}

/// BTC client that accepts any `bc1…` address and answers broadcasts with a
/// fixed outcome, counting how many times it was asked to send.
struct FakeBtc {
    outcome: Outcome,
    sends: AtomicUsize,
}

impl FakeBtc {
    fn new(outcome: Outcome) -> Arc<Self> {
        Arc::new(Self { outcome, sends: AtomicUsize::new(0) })
    }
}

#[async_trait]
impl ChainClient for FakeBtc {
    fn coin(&self) -> Coin {
        Coin::Btc
    }
    async fn generate_address(&self, _user_id: &str) -> Result<GeneratedAddress, ChainError> {
        Err(ChainError { message: "not used".into() })
    }
    fn validate_address(&self, address: &str) -> bool {
        address.starts_with("bc1")
    }
    async fn fetch_deposits(&self, _address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        Ok(vec![])
    }
    async fn broadcast_withdrawal(&self, _to: &str, _amount: u128) -> Result<BroadcastResult, BroadcastError> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        match self.outcome {
            Outcome::Sent => Ok(BroadcastResult { tx_hash: format!("tx{}", Uuid::new_v4().simple()), fee_amount: 700 }),
            Outcome::Ambiguous => Err(BroadcastError { message: "rpc timeout".into(), safe_to_reverse: false }),
        }
    }
    async fn get_balance(&self, _address: &str) -> Result<u128, ChainError> {
        Ok(0)
    }
}

async fn funded_user(pool: &PgPool, prefix: &str) -> (Uuid, Uuid) {
    let user = common::insert_user(pool, prefix).await;
    let wallet = common::insert_personal_wallet(pool, user, Coin::Btc).await;
    common::credit_wallet(pool, wallet, 10_000_000, "seed").await;
    (user, wallet)
}

async fn request(pool: &PgPool, user: Uuid, amount: u64, key: Option<&str>) -> Result<(wd::WithdrawalRow, bool), WithdrawalsError> {
    let client = FakeBtc::new(Outcome::Sent);
    wd::request_withdrawal(pool, user, Coin::Btc, DEST, BigDecimal::from(amount), client.as_ref(), "203.0.113.9", key, None).await
}

async fn status(pool: &PgPool, id: Uuid) -> (String, Option<String>) {
    sqlx::query_as("SELECT status::text, tx_hash FROM withdrawals WHERE id = $1").bind(id).fetch_one(pool).await.unwrap()
}

async fn ledger_types(pool: &PgPool, withdrawal: Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT type::text FROM ledger_entries WHERE reference_id = $1 ORDER BY created_at, type")
        .bind(withdrawal)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn replaying_the_same_key_returns_the_original_and_debits_once(pool: PgPool) {
    let (user, wallet) = funded_user(&pool, "wd-idem").await;

    let (first, created) = request(&pool, user, 100_000, Some("k-1")).await.unwrap();
    assert!(created);
    // A retry — even with a different amount — resolves to the original request.
    let (again, created_again) = request(&pool, user, 999_999, Some("k-1")).await.unwrap();
    assert!(!created_again);
    assert_eq!(again.id, first.id);
    assert_eq!(again.amount, BigDecimal::from(100_000u64));
    assert_eq!(again.to_address, DEST);

    let fee = coin_config(Coin::Btc).withdrawal_fee as u64;
    assert_eq!(common::wallet_balance(&pool, wallet).await, BigDecimal::from(10_000_000 - 100_000 - fee));

    // Keys are scoped per user: someone else's "k-1" is a new withdrawal.
    let (other, _) = funded_user(&pool, "wd-idem-b").await;
    let (theirs, created_theirs) = request(&pool, other, 100_000, Some("k-1")).await.unwrap();
    assert!(created_theirs);
    assert_ne!(theirs.id, first.id);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn racing_requests_with_one_key_debit_once(pool: PgPool) {
    let (user, wallet) = funded_user(&pool, "wd-race").await;

    let results = futures_join(&pool, user).await;
    let ids: std::collections::HashSet<Uuid> = results.iter().map(|(r, _)| r.id).collect();
    assert_eq!(ids.len(), 1, "every racer resolves to the same withdrawal");
    assert_eq!(results.iter().filter(|(_, created)| *created).count(), 1, "exactly one racer creates it");

    let fee = coin_config(Coin::Btc).withdrawal_fee as u64;
    assert_eq!(common::wallet_balance(&pool, wallet).await, BigDecimal::from(10_000_000 - 100_000 - fee));
}

async fn futures_join(pool: &PgPool, user: Uuid) -> Vec<(wd::WithdrawalRow, bool)> {
    let (a, b, c, d) = tokio::join!(
        request(pool, user, 100_000, Some("race")),
        request(pool, user, 100_000, Some("race")),
        request(pool, user, 100_000, Some("race")),
        request(pool, user, 100_000, Some("race")),
    );
    vec![a.unwrap(), b.unwrap(), c.unwrap(), d.unwrap()]
}

#[sqlx::test(migrations = "../db/migrations")]
async fn request_guards_reject_before_touching_the_ledger(pool: PgPool) {
    let (user, wallet) = funded_user(&pool, "wd-guard").await;
    let client = FakeBtc::new(Outcome::Sent);

    let bad = wd::request_withdrawal(&pool, user, Coin::Btc, "not-an-address", BigDecimal::from(100_000u64), client.as_ref(), "203.0.113.9", None, None).await;
    assert!(matches!(bad, Err(WithdrawalsError::InvalidAddress)), "{bad:?}");

    let tiny = wd::request_withdrawal(&pool, user, Coin::Btc, DEST, BigDecimal::from(0u64), client.as_ref(), "203.0.113.9", None, None).await;
    assert!(matches!(tiny, Err(WithdrawalsError::BelowMinimum { .. })), "{tiny:?}");

    let stranger = common::insert_user(&pool, "wd-nowallet").await;
    let none = wd::request_withdrawal(&pool, stranger, Coin::Btc, DEST, BigDecimal::from(100_000u64), client.as_ref(), "203.0.113.9", None, None).await;
    assert!(matches!(none, Err(WithdrawalsError::NotFound)), "{none:?}");

    assert_eq!(common::wallet_balance(&pool, wallet).await, BigDecimal::from(10_000_000u64));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn withdrawal_above_threshold_waits_for_approval(pool: PgPool) {
    let user = common::insert_user(&pool, "wd-big").await;
    let wallet = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    let threshold = wd::approval_threshold_atomic(Coin::Btc) as u64;
    common::credit_wallet(&pool, wallet, threshold * 2, "seed").await;

    let (row, _) = request(&pool, user, threshold, None).await.unwrap();
    assert_eq!(row.status, "PENDING");
    assert!(row.requires_approval);

    // PENDING is not broadcastable: the worker must not send it.
    let fake = FakeBtc::new(Outcome::Sent);
    let registry = ChainRegistry::build("development", true).unwrap().with_client(fake.clone());
    wd::process_broadcast(&pool, row.id, &registry, None).await.unwrap();
    assert_eq!(fake.sends.load(Ordering::SeqCst), 0);
    assert_eq!(status(&pool, row.id).await.0, "PENDING");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn successful_broadcast_confirms_once_without_reversal(pool: PgPool) {
    let (user, wallet) = funded_user(&pool, "wd-ok").await;
    let (row, _) = request(&pool, user, 100_000, None).await.unwrap();
    let before = common::wallet_balance(&pool, wallet).await;

    let fake = FakeBtc::new(Outcome::Sent);
    let registry = ChainRegistry::build("development", true).unwrap().with_client(fake.clone());
    wd::process_broadcast(&pool, row.id, &registry, None).await.unwrap();

    let (st, tx_hash) = status(&pool, row.id).await;
    assert_eq!(st, "CONFIRMED");
    assert!(tx_hash.is_some_and(|h| h.starts_with("tx")));
    assert_eq!(common::wallet_balance(&pool, wallet).await, before, "hot wallet pays the network fee, not the user");
    assert!(!ledger_types(&pool, row.id).await.iter().any(|t| t == "WITHDRAWAL_REVERSAL"));

    let fee_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM network_fee_events WHERE reference_id = $1")
        .bind(row.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fee_rows, 1, "network fee telemetry recorded");

    // A second worker tick must not send again.
    wd::process_broadcast(&pool, row.id, &registry, None).await.unwrap();
    assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn ambiguous_broadcast_failure_keeps_funds_held(pool: PgPool) {
    let (user, wallet) = funded_user(&pool, "wd-amb").await;
    let (row, _) = request(&pool, user, 100_000, None).await.unwrap();
    let held = common::wallet_balance(&pool, wallet).await;

    let registry = ChainRegistry::build("development", true).unwrap().with_client(FakeBtc::new(Outcome::Ambiguous));
    wd::process_broadcast(&pool, row.id, &registry, None).await.unwrap();

    // The tx may still confirm on-chain: no re-credit, left for reconciliation.
    assert_eq!(status(&pool, row.id).await.0, "BROADCASTING");
    assert_eq!(common::wallet_balance(&pool, wallet).await, held);
    assert!(!ledger_types(&pool, row.id).await.iter().any(|t| t == "WITHDRAWAL_REVERSAL"));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn stuck_broadcasted_withdrawal_is_finalized_not_resent(pool: PgPool) {
    let (user, _) = funded_user(&pool, "wd-stuck").await;
    let (row, _) = request(&pool, user, 100_000, None).await.unwrap();
    sqlx::query("UPDATE withdrawals SET status = 'BROADCASTED', tx_hash = 'txstuck' WHERE id = $1")
        .bind(row.id)
        .execute(&pool)
        .await
        .unwrap();

    let fake = FakeBtc::new(Outcome::Sent);
    let registry = ChainRegistry::build("development", true).unwrap().with_client(fake.clone());
    wd::process_broadcast(&pool, row.id, &registry, None).await.unwrap();

    assert_eq!(status(&pool, row.id).await, ("CONFIRMED".to_string(), Some("txstuck".to_string())));
    assert_eq!(fake.sends.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn address_book_is_per_user_and_validated(pool: PgPool) {
    let alice = common::insert_user(&pool, "ab-alice").await;
    let bob = common::insert_user(&pool, "ab-bob").await;

    let entry = wd::add_address_book(&pool, alice, Coin::Btc, "  cold  ", &format!(" {DEST} ")).await.unwrap();
    assert_eq!((entry.label.as_str(), entry.address.as_str(), entry.coin.as_str()), ("cold", DEST, "BTC"));

    // Same address again only renames it.
    let renamed = wd::add_address_book(&pool, alice, Coin::Btc, "vault", DEST).await.unwrap();
    assert_eq!(renamed.id, entry.id);
    let list = wd::list_address_book(&pool, alice).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].label, "vault");
    assert!(wd::list_address_book(&pool, bob).await.unwrap().is_empty());

    for (label, address) in [("", DEST), (&"x".repeat(65)[..], DEST), ("ok", "short"), ("ok", &"a".repeat(257)[..])] {
        let r = wd::add_address_book(&pool, alice, Coin::Btc, label, address).await;
        assert!(matches!(r, Err(WithdrawalsError::InvalidAddress)), "{label:?}/{} accepted", address.len());
    }

    assert!(!wd::delete_address_book(&pool, bob, entry.id).await.unwrap(), "bob cannot delete alice's entry");
    assert!(wd::delete_address_book(&pool, alice, entry.id).await.unwrap());
    assert!(!wd::delete_address_book(&pool, alice, entry.id).await.unwrap());
}
