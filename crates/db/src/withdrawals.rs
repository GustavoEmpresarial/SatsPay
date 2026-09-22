//! Port 1:1 of legacy `apps/api/src/modules/withdrawals/{services,repositories}/withdrawals.*.ts`.
//!
//! State machine (unchanged from legacy, "A3′"):
//!   QUEUED|APPROVED  → BROADCASTING   (atomic claim, compare-and-swap UPDATE)
//!   BROADCASTING     → BROADCASTED    (txHash persisted; funds already on-chain)
//!   BROADCASTED      → CONFIRMED      (bookkeeping only; fee stays on hot wallet)
//!   BROADCASTING     → FAILED         (only when the chain client guarantees no send)
//!   BROADCASTING     → stays          (ambiguous RPC error → manual reconciliation)
//!
//! Never reverse a withdrawal after a successful or ambiguous on-chain send.

use crate::ledger::{apply_ledger_entry, LedgerCreditInput};
use crate::privacy::{self, KIND_WD_TO};
use bigdecimal::BigDecimal;
use chain::{BroadcastError, ChainClient};
use crypto::SecretsService;
use events::domain::{WithdrawalBroadcasted, WithdrawalConfirmed, WithdrawalFailed};
use events::{DomainEvent, OutboxWriter};
use shared::{coin_config, Coin};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Manual-approval threshold in the coin's smallest unit.
///
/// Override with `WITHDRAWAL_APPROVAL_THRESHOLD_<COIN>` (e.g. `…_BTC=1500000`).
/// Invalid / empty values fall back to [`coin_config`] defaults (~USD 1 000).
pub fn approval_threshold_atomic(coin: Coin) -> u128 {
    let key = format!(
        "WITHDRAWAL_APPROVAL_THRESHOLD_{}",
        coin.as_str().to_ascii_uppercase()
    );
    match std::env::var(&key) {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return coin_config(coin).approval_threshold;
            }
            match trimmed.parse::<u128>() {
                Ok(v) => v,
                Err(_) => {
                    tracing::warn!(
                        env = %key,
                        raw = %trimmed,
                        "invalid withdrawal approval threshold; using coin default"
                    );
                    coin_config(coin).approval_threshold
                }
            }
        }
        Err(_) => coin_config(coin).approval_threshold,
    }
}

/// Test helper: resolve threshold from an optional env string (no process env).
pub fn resolve_approval_threshold(coin: Coin, env_val: Option<&str>) -> u128 {
    match env_val {
        None => coin_config(coin).approval_threshold,
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                coin_config(coin).approval_threshold
            } else {
                trimmed
                    .parse::<u128>()
                    .unwrap_or_else(|_| coin_config(coin).approval_threshold)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WithdrawalsError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Outbox(#[from] events::OutboxError),
    #[error("invalid destination address")]
    InvalidAddress,
    #[error("minimum withdrawal is {min} (smallest unit)")]
    BelowMinimum { min: u128 },
    #[error("wallet not found")]
    NotFound,
}

#[derive(Debug, Clone)]
pub struct WithdrawalRow {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub to_address: String,
    pub amount: BigDecimal,
    pub fee_amount: BigDecimal,
    pub status: String,
    pub requires_approval: bool,
}

/// Debits PERSONAL. Invoice net credits live on MERCHANT — pass that kind to
/// [`request_withdrawal_from`].
#[allow(clippy::too_many_arguments)]
pub async fn request_withdrawal(
    pool: &PgPool,
    user_id: Uuid,
    coin: Coin,
    to_address: &str,
    amount: BigDecimal,
    chain: &dyn ChainClient,
    requested_ip: &str,
    idempotency_key: Option<&str>,
    secrets: Option<&SecretsService>,
) -> Result<(WithdrawalRow, bool), WithdrawalsError> {
    request_withdrawal_from(
        pool,
        user_id,
        coin,
        to_address,
        amount,
        chain,
        requested_ip,
        idempotency_key,
        secrets,
        "PERSONAL",
    )
    .await
}

/// Same as [`request_withdrawal`], but debits `wallet_kind` (`PERSONAL` or
/// `MERCHANT` only; anything else is treated as PERSONAL).
#[allow(clippy::too_many_arguments)]
pub async fn request_withdrawal_from(
    pool: &PgPool,
    user_id: Uuid,
    coin: Coin,
    to_address: &str,
    amount: BigDecimal,
    chain: &dyn ChainClient,
    requested_ip: &str,
    idempotency_key: Option<&str>,
    secrets: Option<&SecretsService>,
    wallet_kind: &str,
) -> Result<(WithdrawalRow, bool), WithdrawalsError> {
    let wallet_kind = if wallet_kind == "MERCHANT" { "MERCHANT" } else { "PERSONAL" };
    if !chain.validate_address(to_address) {
        return Err(WithdrawalsError::InvalidAddress);
    }
    let cfg = coin_config(coin);
    let min_withdrawal = BigDecimal::from(cfg.min_withdrawal);
    if amount < min_withdrawal {
        return Err(WithdrawalsError::BelowMinimum { min: cfg.min_withdrawal });
    }

    // Scope the client-supplied key to the user — the DB column is globally
    // unique, so without scoping one user could reserve/probe another user's key.
    let scoped_key = idempotency_key.map(|k| format!("{user_id}:{k}"));

    if let Some(key) = &scoped_key {
        if let Some(existing) = find_by_idempotency_key(pool, key, secrets).await? {
            return Ok((existing, false));
        }
    }

    let approval_threshold = BigDecimal::from(approval_threshold_atomic(coin));
    let requires_approval = amount >= approval_threshold;
    let fee_amount = BigDecimal::from(cfg.withdrawal_fee);

    let mut tx = pool.begin().await?;

    if let Some(key) = &scoped_key {
        if let Some(existing) = find_by_idempotency_key(&mut *tx, key, secrets).await? {
            tx.commit().await?;
            return Ok((existing, false));
        }
    }

    let wallet_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = $3::wallet_kind",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .bind(wallet_kind)
    .fetch_optional(&mut *tx)
    .await?;
    let wallet_id = wallet_id.ok_or(WithdrawalsError::NotFound)?;

    let status = if requires_approval { "PENDING" } else { "QUEUED" };
    let withdrawal_id = Uuid::new_v4();
    let stored_addr = privacy::seal_opt(secrets, KIND_WD_TO, &withdrawal_id.to_string(), to_address);
    let stored_ip = privacy::store_ip(secrets, requested_ip);
    let insert_result = sqlx::query(
        r#"
        INSERT INTO withdrawals (id, wallet_id, to_address, amount, fee_amount, status, requires_approval, requested_ip, idempotency_key)
        VALUES ($1, $2, $3, $4, $5, $6::withdrawal_status, $7, $8, $9)
        "#,
    )
    .bind(withdrawal_id)
    .bind(wallet_id)
    .bind(&stored_addr)
    .bind(&amount)
    .bind(&fee_amount)
    .bind(status)
    .bind(requires_approval)
    .bind(&stored_ip)
    .bind(&scoped_key)
    .execute(&mut *tx)
    .await;

    match insert_result {
        Ok(_) => {}
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            // True concurrent race on the idempotency key: the transaction
            // rolls back (no debit performed here), resolve to the winner.
            drop(tx);
            if let Some(key) = &scoped_key {
                if let Some(existing) = find_by_idempotency_key(pool, key, secrets).await? {
                    return Ok((existing, false));
                }
            }
            return Err(WithdrawalsError::NotFound);
        }
        Err(e) => return Err(e.into()),
    };

    // Debit immediately (hold funds), tagged with the withdrawal id. If
    // broadcast later fails safely, we reverse with a WITHDRAWAL_REVERSAL entry.
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id,
            amount: -amount.clone(),
            ledger_type: "WITHDRAWAL",
            reference_id: Some(withdrawal_id),
            reference_type: Some("Withdrawal"),
            memo: Some(&format!("withdrawal:{withdrawal_id}")),
        },
    )
    .await
    .map_err(sqlx_from_ledger)?;

    if cfg.withdrawal_fee > 0 {
        apply_ledger_entry(
            &mut tx,
            LedgerCreditInput { reference_key: None,
                wallet_id,
                amount: -fee_amount.clone(),
                ledger_type: "WITHDRAWAL_FEE",
                reference_id: Some(withdrawal_id),
                reference_type: Some("Withdrawal"),
                memo: Some("Withdrawal network fee (-25% discount)"),
            },
        )
        .await
        .map_err(sqlx_from_ledger)?;
    }

    tx.commit().await?;

    Ok((WithdrawalRow { id: withdrawal_id, wallet_id, to_address: to_address.to_string(), amount, fee_amount, status: status.to_string(), requires_approval }, true))
}

/// Called by the `withdrawal_reconciler` worker job. Atomically claims the
/// withdrawal for broadcast (only one worker replica can flip
/// `QUEUED|APPROVED` → `BROADCASTING`), calls the chain client, and advances
/// the state machine per the module docs.
pub async fn process_broadcast(pool: &PgPool, withdrawal_id: Uuid, registry: &chain::ChainRegistry, secrets: Option<&SecretsService>) -> Result<(), WithdrawalsError> {
    let claimed = sqlx::query(
        "UPDATE withdrawals SET status = 'BROADCASTING'::withdrawal_status, updated_at = now() \
         WHERE id = $1 AND status IN ('QUEUED', 'APPROVED') RETURNING id",
    )
    .bind(withdrawal_id)
    .fetch_optional(pool)
    .await?;

    if claimed.is_none() {
        // Already claimed / not in a broadcastable state. If stuck in
        // BROADCASTED (partial persist), try to finalize to CONFIRMED.
        let status: Option<String> = sqlx::query_scalar("SELECT status::text FROM withdrawals WHERE id = $1").bind(withdrawal_id).fetch_optional(pool).await?;
        if status.as_deref() == Some("BROADCASTED") {
            finalize_broadcasted(pool, withdrawal_id).await?;
        }
        return Ok(());
    }

    let row = sqlx::query(
        "SELECT w.wallet_id, w.to_address, w.amount, w.fee_amount, wa.coin::text as coin, wa.user_id
         FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id WHERE w.id = $1",
    )
    .bind(withdrawal_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(()) };
    let wallet_id: Uuid = row.get("wallet_id");
    let owner_id: Uuid = row.get("user_id");
    let to_address = privacy::open_opt(secrets, KIND_WD_TO, &withdrawal_id.to_string(), &row.get::<String, _>("to_address"));
    let amount: BigDecimal = row.get("amount");
    let fee_amount: BigDecimal = row.get("fee_amount");
    let coin_str: String = row.get("coin");
    let Ok(coin) = coin_str.parse::<Coin>() else { return Ok(()) };

    let client = registry.get(coin);
    let amount_u128: u128 = amount.to_string().parse().unwrap_or(0);

    // Phase 1 — external send. Only reverse when the client is certain
    // nothing left the hot wallet.
    let broadcast_result = client.broadcast_withdrawal(&to_address, amount_u128).await;
    let result = match broadcast_result {
        Ok(r) => r,
        Err(BroadcastError { message, safe_to_reverse }) => {
            if safe_to_reverse {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE withdrawals SET status = 'FAILED'::withdrawal_status, updated_at = now() WHERE id = $1")
                    .bind(withdrawal_id)
                    .execute(&mut *tx)
                    .await?;
                let total_reversal = &amount + &fee_amount;
                apply_ledger_entry(
                    &mut tx,
                    LedgerCreditInput { reference_key: None,
                        wallet_id,
                        amount: total_reversal,
                        ledger_type: "WITHDRAWAL_REVERSAL",
                        reference_id: Some(withdrawal_id),
                        reference_type: Some("Withdrawal"),
                        memo: Some(&format!("Reversal (safe): {message}")),
                    },
                )
                .await
                .map_err(sqlx_from_ledger)?;
                OutboxWriter::stage(
                    &mut tx,
                    &DomainEvent::WithdrawalFailed(WithdrawalFailed { wallet_id, withdrawal_id, reason: message.clone(), safe_to_reverse: true }),
                )
                .await?;
                tx.commit().await?;
                let _ = crate::audit::record_log(
                    pool,
                    Some(owner_id),
                    "WITHDRAWAL_FAILED",
                    "Withdrawal",
                    Some(withdrawal_id),
                    None,
                    Some(serde_json::json!({
                        "coin": coin_str,
                        "reason": message,
                        "safeToReverse": true
                    })),
                )
                .await;
            } else {
                // Ambiguous failure (timeout, disconnect, unknown). Do NOT
                // reverse — the tx may still confirm on-chain. Leave
                // BROADCASTING for manual reconciliation.
                tracing::error!(withdrawal_id = %withdrawal_id, error = %message, "withdrawal broadcast failed with ambiguous error — NOT reversing; requires chain reconciliation");
            }
            return Ok(());
        }
    };

    // Phase 2 — funds are on-chain. Persist BROADCASTED + tx_hash BEFORE any
    // fee bookkeeping so a later DB failure never loses the chain reference.
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE withdrawals SET status = 'BROADCASTED'::withdrawal_status, tx_hash = $2, updated_at = now() WHERE id = $1",
    )
    .bind(withdrawal_id)
    .bind(&result.tx_hash)
    .execute(&mut *tx)
    .await?;
    OutboxWriter::stage(
        &mut tx,
        &DomainEvent::WithdrawalBroadcasted(WithdrawalBroadcasted {
            wallet_id,
            withdrawal_id,
            coin: coin.as_str().to_string(),
            amount: amount.to_string(),
            tx_hash: result.tx_hash.clone(),
        }),
    )
    .await?;
    tx.commit().await?;

    // Telemetry: network fee paid by hot (not user ledger). Best-effort / idempotent.
    if let Err(e) = crate::network_fees::record_network_fee(
        pool,
        crate::network_fees::RecordNetworkFeeInput {
            coin,
            kind: crate::network_fees::NetworkFeeKind::Withdrawal,
            amount: result.fee_amount,
            tx_hash: Some(&result.tx_hash),
            reference_id: Some(withdrawal_id),
            reference_type: Some("Withdrawal"),
        },
    )
    .await
    {
        tracing::warn!(withdrawal_id = %withdrawal_id, error = %e, "failed to record withdrawal network fee");
    }

    // Phase 3 — bookkeeping only (no user fee debit). Safe to retry.
    finalize_broadcasted(pool, withdrawal_id).await
}

/// Flip BROADCASTED → CONFIRMED. Idempotent; no ledger side-effects.
pub async fn finalize_broadcasted(pool: &PgPool, withdrawal_id: Uuid) -> Result<(), WithdrawalsError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "UPDATE withdrawals SET status = 'CONFIRMED'::withdrawal_status, updated_at = now() \
         WHERE id = $1 AND status = 'BROADCASTED'::withdrawal_status RETURNING wallet_id",
    )
    .bind(withdrawal_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(row) = row {
        let wallet_id: Uuid = row.get("wallet_id");
        OutboxWriter::stage(&mut tx, &DomainEvent::WithdrawalConfirmed(WithdrawalConfirmed { wallet_id, withdrawal_id, confirmations: 1 })).await?;
        tx.commit().await?;
        tracing::info!(withdrawal_id = %withdrawal_id, "withdrawal finalized CONFIRMED");
    }
    Ok(())
}

/// Re-enqueues QUEUED/APPROVED withdrawals that never reached the worker
/// (missed job, worker downtime). The internal job's stable id (see
/// `queue::enqueue` caller) prevents double-broadcast.
pub async fn list_eligible_for_requeue(pool: &PgPool, take: i64) -> Result<Vec<Uuid>, WithdrawalsError> {
    let rows = sqlx::query("SELECT id FROM withdrawals WHERE status IN ('QUEUED', 'APPROVED') ORDER BY created_at ASC LIMIT $1")
        .bind(take.min(500))
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("id")).collect())
}

#[derive(Debug, Clone)]
pub struct WithdrawalHistoryRecord {
    pub id: Uuid,
    pub coin: String,
    pub to_address: String,
    pub amount: String,
    pub fee_amount: String,
    pub status: String,
    pub tx_hash: Option<String>,
    pub requires_approval: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_user_withdrawals(
    pool: &PgPool,
    user_id: Uuid,
    coin_filter: Option<shared::Coin>,
    limit: i64,
    secrets: Option<&SecretsService>,
) -> Result<Vec<WithdrawalHistoryRecord>, sqlx::Error> {
    let rows = if let Some(c) = coin_filter {
        sqlx::query(
            r#"
            SELECT w.id, wa.coin::text as coin, w.to_address, w.amount, w.fee_amount,
                   w.status::text as status, w.tx_hash, w.requires_approval,
                   w.created_at, w.updated_at
            FROM withdrawals w
            JOIN wallets wa ON wa.id = w.wallet_id
            WHERE wa.user_id = $1 AND wa.coin = $2::coin
            ORDER BY w.created_at DESC
            LIMIT $3
            "#,
        )
        .bind(user_id)
        .bind(c.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT w.id, wa.coin::text as coin, w.to_address, w.amount, w.fee_amount,
                   w.status::text as status, w.tx_hash, w.requires_approval,
                   w.created_at, w.updated_at
            FROM withdrawals w
            JOIN wallets wa ON wa.id = w.wallet_id
            WHERE wa.user_id = $1
            ORDER BY w.created_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|r| {
            let amount: BigDecimal = r.get("amount");
            let fee_amount: BigDecimal = r.get("fee_amount");
            let id: Uuid = r.get("id");
            WithdrawalHistoryRecord {
                id,
                coin: r.get("coin"),
                to_address: privacy::open_opt(secrets, KIND_WD_TO, &id.to_string(), &r.get::<String, _>("to_address")),
                amount: amount.to_string(),
                fee_amount: fee_amount.to_string(),
                status: r.get("status"),
                tx_hash: r.get("tx_hash"),
                requires_approval: r.get("requires_approval"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            }
        })
        .collect())
}

async fn find_by_idempotency_key<'e, E>(executor: E, key: &str, secrets: Option<&SecretsService>) -> Result<Option<WithdrawalRow>, WithdrawalsError>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        "SELECT id, wallet_id, to_address, amount, fee_amount, status::text as status, requires_approval FROM withdrawals WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| {
        let id: Uuid = r.get("id");
        WithdrawalRow {
            id,
            wallet_id: r.get("wallet_id"),
            to_address: privacy::open_opt(secrets, KIND_WD_TO, &id.to_string(), &r.get::<String, _>("to_address")),
            amount: r.get("amount"),
            fee_amount: r.get("fee_amount"),
            status: r.get("status"),
            requires_approval: r.get("requires_approval"),
        }
    }))
}

fn sqlx_from_ledger(e: crate::ledger::LedgerError) -> sqlx::Error {
    match e {
        crate::ledger::LedgerError::Db(db_err) => db_err,
        other => sqlx::Error::Protocol(other.to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct AddressBookEntry {
    pub id: Uuid,
    pub coin: String,
    pub label: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_address_book(pool: &PgPool, user_id: Uuid) -> Result<Vec<AddressBookEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, coin::text AS coin, label, address, created_at
         FROM withdrawal_address_book WHERE user_id = $1
         ORDER BY created_at DESC LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AddressBookEntry {
            id: r.get("id"),
            coin: r.get("coin"),
            label: r.get("label"),
            address: r.get("address"),
            created_at: r.get("created_at"),
        })
        .collect())
}

pub async fn add_address_book(
    pool: &PgPool,
    user_id: Uuid,
    coin: Coin,
    label: &str,
    address: &str,
) -> Result<AddressBookEntry, WithdrawalsError> {
    let label = label.trim();
    let address = address.trim();
    if label.is_empty() || label.len() > 64 {
        return Err(WithdrawalsError::InvalidAddress);
    }
    if address.len() < 8 || address.len() > 256 {
        return Err(WithdrawalsError::InvalidAddress);
    }
    let row = sqlx::query(
        "INSERT INTO withdrawal_address_book (user_id, coin, label, address)
         VALUES ($1, $2::coin, $3, $4)
         ON CONFLICT (user_id, coin, address) DO UPDATE SET label = EXCLUDED.label
         RETURNING id, coin::text AS coin, label, address, created_at",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .bind(label)
    .bind(address)
    .fetch_one(pool)
    .await?;
    Ok(AddressBookEntry {
        id: row.get("id"),
        coin: row.get("coin"),
        label: row.get("label"),
        address: row.get("address"),
        created_at: row.get("created_at"),
    })
}

pub async fn delete_address_book(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM withdrawal_address_book WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

#[cfg(test)]
mod approval_threshold_tests {
    use super::resolve_approval_threshold;
    use shared::{coin_config, Coin};

    #[test]
    fn default_matches_coin_config() {
        for coin in shared::COINS {
            assert_eq!(
                resolve_approval_threshold(coin, None),
                coin_config(coin).approval_threshold
            );
        }
    }

    #[test]
    fn override_parses_atomic_units() {
        assert_eq!(
            resolve_approval_threshold(Coin::Btc, Some("100")),
            100
        );
        assert_eq!(
            resolve_approval_threshold(Coin::Pol, Some(" 250000000000 ")),
            250_000_000_000
        );
    }

    #[test]
    fn empty_or_invalid_falls_back() {
        let def = coin_config(Coin::Btc).approval_threshold;
        assert_eq!(resolve_approval_threshold(Coin::Btc, Some("")), def);
        assert_eq!(resolve_approval_threshold(Coin::Btc, Some("nope")), def);
    }
}
