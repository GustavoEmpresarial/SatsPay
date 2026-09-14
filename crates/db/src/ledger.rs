//! Core ledger primitives — port 1:1 of legacy
//! `apps/api/src/modules/wallet/domain/ledger.ts`. See docs/security/BALANCE_SECURITY.md.
//!
//! There is no balance column anywhere in the schema. A wallet's balance is
//! always `SUM(ledger_entries.amount)`. Critically, the lock that serializes
//! concurrent balance mutations is taken on the **wallet row**
//! (`SELECT id FROM wallets WHERE id = $1 FOR UPDATE`), not on the
//! `ledger_entries` aggregate — a wallet with zero existing entries has
//! nothing for `SELECT ... FOR UPDATE` to lock on the entries table, so
//! locking the aggregate alone does not serialize the very first credit/debit
//! race. Every balance-changing operation must lock the wallet(s) first.

use bigdecimal::BigDecimal;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("insufficient balance: wallet {wallet_id} has {available}, needs {required}")]
    InsufficientBalance { wallet_id: Uuid, available: BigDecimal, required: BigDecimal },
    #[error("wallet {wallet_id} is restricted pending deposit reorganization review")]
    ReorgHold { wallet_id: Uuid },
}

pub struct LedgerCreditInput<'a> {
    pub wallet_id: Uuid,
    /// Positive = credit, negative = debit.
    pub amount: BigDecimal,
    pub ledger_type: &'a str,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<&'a str>,
    /// Free-text reference for operations without a single-row UUID to point
    /// at (e.g. lend positions keyed `"{user_id}:{coin}"`, public-API
    /// transfers keyed `"pubapi:{key_id}:{idempotency_key}"`). Mutually
    /// usable alongside `reference_id`; most callers leave this `None`.
    pub reference_key: Option<&'a str>,
    pub memo: Option<&'a str>,
}

impl<'a> LedgerCreditInput<'a> {
    /// Convenience for the common case (no `reference_key`), so existing
    /// call sites don't need to name every field.
    pub fn simple(wallet_id: Uuid, amount: BigDecimal, ledger_type: &'a str, reference_id: Option<Uuid>, reference_type: Option<&'a str>, memo: Option<&'a str>) -> Self {
        Self { wallet_id, amount, ledger_type, reference_id, reference_type, reference_key: None, memo }
    }
}

/// Takes a row-level lock on a wallet inside `tx` WITHOUT mutating it. Call
/// this at the start of a transaction whenever a cooldown/idempotency check
/// reads state that a later `apply_ledger_entry` depends on — it forces
/// concurrent transactions to serialize *before* the check, closing
/// check-then-act races (faucet cooldown, public-API idempotency, deposit
/// credit).
pub async fn lock_wallet(tx: &mut Transaction<'_, Postgres>, wallet_id: Uuid) -> Result<(), LedgerError> {
    sqlx::query("SELECT id FROM wallets WHERE id = $1 FOR UPDATE").bind(wallet_id).fetch_optional(&mut **tx).await?;
    Ok(())
}

/// Locks a group of wallets in a canonical (sorted, deduped) order. Every
/// operation that touches more than one balance (transfers, swaps) must use
/// this before applying entries: otherwise two operations that happen to
/// acquire the same pair of wallets in opposite order can deadlock in Postgres.
pub async fn lock_wallets(tx: &mut Transaction<'_, Postgres>, wallet_ids: &[Uuid]) -> Result<(), LedgerError> {
    let mut sorted: Vec<Uuid> = wallet_ids.to_vec();
    sorted.sort();
    sorted.dedup();
    for wallet_id in sorted {
        lock_wallet(tx, wallet_id).await?;
    }
    Ok(())
}

/// `SUM(ledger_entries.amount)` for a wallet. Callers that need this value to
/// gate a balance-changing decision MUST call `lock_wallet` first in the same
/// transaction — this function does not lock by itself (see module docs).
pub async fn get_wallet_balance(tx: &mut Transaction<'_, Postgres>, wallet_id: Uuid) -> Result<BigDecimal, LedgerError> {
    let row: (Option<BigDecimal>,) =
        sqlx::query_as("SELECT COALESCE(SUM(amount), 0) FROM ledger_entries WHERE wallet_id = $1")
            .bind(wallet_id)
            .fetch_one(&mut **tx)
            .await?;
    Ok(row.0.unwrap_or_else(|| BigDecimal::from(0)))
}

/// Credits or debits a wallet inside `tx`. Locks the wallet row first (see
/// module docs), rejects balance-changing operations while the wallet is
/// under `reorg_hold_at` (except the compensating `DEPOSIT_REVERSAL` itself,
/// which must remain permitted so a later retry can settle it), and for
/// debits (`amount < 0`) verifies the resulting balance would not go
/// negative. This is the only function in the codebase allowed to write to
/// `ledger_entries` — there is intentionally no `update_balance`.
pub async fn apply_ledger_entry(tx: &mut Transaction<'_, Postgres>, input: LedgerCreditInput<'_>) -> Result<Uuid, LedgerError> {
    lock_wallet(tx, input.wallet_id).await?;

    let reorg_hold_at: Option<(Option<chrono::DateTime<chrono::Utc>>,)> =
        sqlx::query_as("SELECT reorg_hold_at FROM wallets WHERE id = $1").bind(input.wallet_id).fetch_optional(&mut **tx).await?;
    if let Some((Some(_),)) = reorg_hold_at {
        if input.ledger_type != "DEPOSIT_REVERSAL" {
            return Err(LedgerError::ReorgHold { wallet_id: input.wallet_id });
        }
    }

    if input.amount.sign() == bigdecimal::num_bigint::Sign::Minus {
        let balance = get_wallet_balance(tx, input.wallet_id).await?;
        if (&balance + &input.amount).sign() == bigdecimal::num_bigint::Sign::Minus {
            return Err(LedgerError::InsufficientBalance { wallet_id: input.wallet_id, available: balance, required: -input.amount });
        }
    }

    let id: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO ledger_entries (wallet_id, amount, type, reference_id, reference_type, reference_key, memo)
        VALUES ($1, $2, $3::ledger_type, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(input.wallet_id)
    .bind(input.amount)
    .bind(input.ledger_type)
    .bind(input.reference_id)
    .bind(input.reference_type)
    .bind(input.reference_key)
    .bind(input.memo)
    .fetch_one(&mut **tx)
    .await?;

    Ok(id.0)
}
