//! Port 1:1 of legacy `apps/api/src/modules/deposits/{services,repositories}/deposits.*.ts`.

use crate::ledger::{apply_ledger_entry, get_wallet_balance, lock_wallet, LedgerCreditInput, LedgerError};
use bigdecimal::BigDecimal;
use chain::ChainClient;
use chrono::Utc;
use events::domain::DepositConfirmed;
use events::{DomainEvent, OutboxWriter};
use shared::coin_config;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum DepositsError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Outbox(#[from] events::OutboxError),
    #[error("wallet not found")]
    NotFound,
    #[error("chain error: {0}")]
    Chain(String),
}

/// Returns the wallet's deposit address, generating one via `chain` the
/// first time. Serializes concurrent first-time requests: locks the wallet
/// row, re-checks inside the transaction, and only generates/assigns if
/// still unset — otherwise two parallel calls would each mint an address
/// (orphaning one).
pub async fn get_or_create_address(pool: &PgPool, user_id: Uuid, coin: shared::Coin, client: &dyn ChainClient) -> Result<String, DepositsError> {
    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, 'PERSONAL') ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, 'DEVELOPER') ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .execute(pool)
    .await?;
    let wallet_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
            .bind(user_id)
            .bind(coin.as_str())
            .fetch_optional(pool)
            .await?;
    let wallet_id = wallet_id.ok_or(DepositsError::NotFound)?;

    // Address alone is enough — stub clients may omit hd_index; regenerating
    // would orphan the previously assigned deposit address.
    let existing: Option<String> =
        sqlx::query_scalar("SELECT address FROM wallets WHERE id = $1").bind(wallet_id).fetch_one(pool).await?;
    if let Some(address) = existing {
        return Ok(address);
    }

    let mut tx = pool.begin().await?;
    lock_wallet(&mut tx, wallet_id).await?;
    let fresh: Option<String> =
        sqlx::query_scalar("SELECT address FROM wallets WHERE id = $1").bind(wallet_id).fetch_one(&mut *tx).await?;
    if let Some(address) = fresh {
        tx.commit().await?;
        return Ok(address);
    }

    let generated = client.generate_address(&user_id.to_string()).await.map_err(|e| DepositsError::Chain(e.to_string()))?;
    sqlx::query("UPDATE wallets SET address = $2, hd_index = $3 WHERE id = $1")
        .bind(wallet_id)
        .bind(&generated.address)
        .bind(generated.hd_index.map(|i| i as i64))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(generated.address)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditDepositOutcome {
    /// Already CREDITED — no side-effects.
    Noop,
    /// Deposit row created/updated but not yet ledger-credited.
    Recorded,
    /// Ledger credit applied for the first time.
    NewlyCredited,
}

/// Credits a confirmed on-chain deposit to the wallet ledger — idempotent
/// per (tx_hash, vout). Called by the `deposit_watcher` job. Publishes
/// `DepositConfirmed` to the outbox in the same transaction as the ledger
/// write, so the event can never be observed without the credit having
/// actually committed (or vice versa).
#[allow(clippy::too_many_arguments)]
pub async fn credit_deposit(
    pool: &PgPool,
    wallet_id: Uuid,
    coin: shared::Coin,
    tx_hash: &str,
    vout: i32,
    amount: BigDecimal,
    confirmations: i32,
) -> Result<CreditDepositOutcome, DepositsError> {
    let min_confs = coin_config(coin).min_confirmations as i32;

    let mut tx = pool.begin().await?;
    // Lock the wallet BEFORE reading deposit status so two watchers (or a
    // retried poll) can't both observe "not yet CREDITED" and credit twice.
    lock_wallet(&mut tx, wallet_id).await?;

    let existing = sqlx::query("SELECT id, status::text as status FROM deposits WHERE tx_hash = $1 AND vout = $2")
        .bind(tx_hash)
        .bind(vout)
        .fetch_optional(&mut *tx)
        .await?;

    if let Some(row) = &existing {
        let status: String = row.get("status");
        if status == "CREDITED" {
            return Ok(CreditDepositOutcome::Noop);
        }
    }

    let new_status = if confirmations >= min_confs { "CONFIRMED" } else { "PENDING" };
    let deposit_id: Uuid = if let Some(row) = &existing {
        let id: Uuid = row.get("id");
        sqlx::query("UPDATE deposits SET confirmations = $2, status = $3::deposit_status WHERE id = $1")
            .bind(id)
            .bind(confirmations)
            .bind(new_status)
            .execute(&mut *tx)
            .await?;
        id
    } else {
        sqlx::query_scalar(
            "INSERT INTO deposits (wallet_id, tx_hash, vout, amount, confirmations, status)
             VALUES ($1, $2, $3, $4, $5, $6::deposit_status) RETURNING id",
        )
        .bind(wallet_id)
        .bind(tx_hash)
        .bind(vout)
        .bind(&amount)
        .bind(confirmations)
        .bind(new_status)
        .fetch_one(&mut *tx)
        .await?
    };

    if new_status == "CONFIRMED" {
        apply_ledger_entry(
            &mut tx,
            LedgerCreditInput { reference_key: None,
                wallet_id,
                amount: amount.clone(),
                ledger_type: "DEPOSIT",
                reference_id: Some(deposit_id),
                reference_type: Some("Deposit"),
                memo: Some(&format!("Deposit {}…", &tx_hash[..tx_hash.len().min(12)])),
            },
        )
        .await?;
        sqlx::query("UPDATE deposits SET status = 'CREDITED'::deposit_status, credited_at = now() WHERE id = $1")
            .bind(deposit_id)
            .execute(&mut *tx)
            .await?;

        OutboxWriter::stage(
            &mut tx,
            &DomainEvent::DepositConfirmed(DepositConfirmed {
                wallet_id,
                deposit_id,
                coin: coin.as_str().to_string(),
                amount: amount.to_string(),
                tx_hash: tx_hash.to_string(),
                confirmed_at: Utc::now(),
            }),
        )
        .await?;

        tx.commit().await?;
        return Ok(CreditDepositOutcome::NewlyCredited);
    }

    tx.commit().await?;
    Ok(CreditDepositOutcome::Recorded)
}

/// Reconciles one previously detected output against a successful chain
/// snapshot. A CREDITED output that disappears or drops below the
/// confirmation threshold is orphaned. If its value has already been spent,
/// no impossible negative debit is written: the wallet is put on
/// `reorg_hold_at` instead and the loss remains auditable.
pub async fn reconcile_deposit(
    pool: &PgPool,
    wallet_id: Uuid,
    coin: shared::Coin,
    tx_hash: &str,
    vout: i32,
    onchain_confirmations: Option<i32>,
) -> Result<(), DepositsError> {
    let min_confs = coin_config(coin).min_confirmations as i32;
    if let Some(confs) = onchain_confirmations {
        if confs >= min_confs {
            return Ok(());
        }
    }

    let mut tx = pool.begin().await?;
    lock_wallet(&mut tx, wallet_id).await?;

    let deposit = sqlx::query("SELECT id, wallet_id, status::text as status, amount FROM deposits WHERE tx_hash = $1 AND vout = $2")
        .bind(tx_hash)
        .bind(vout)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = deposit else { return Ok(()) };
    let deposit_id: Uuid = row.get("id");
    let deposit_wallet_id: Uuid = row.get("wallet_id");
    let status: String = row.get("status");
    let amount: BigDecimal = row.get("amount");
    if deposit_wallet_id != wallet_id || status == "ORPHANED" {
        return Ok(());
    }

    let reason = match onchain_confirmations {
        None => "Output no longer present in successful chain snapshot".to_string(),
        Some(confs) => format!("Confirmations dropped below required minimum ({confs}/{min_confs})"),
    };

    if status != "CREDITED" {
        sqlx::query("UPDATE deposits SET status = 'ORPHANED'::deposit_status, confirmations = $2, orphaned_at = now() WHERE id = $1")
            .bind(deposit_id)
            .bind(onchain_confirmations.unwrap_or(0))
            .execute(&mut *tx)
            .await?;
        insert_audit_log(&mut tx, "DEPOSIT_ORPHANED_UNCREDITED", "Deposit", deposit_id, serde_json::json!({ "reason": reason })).await?;
        tx.commit().await?;
        return Ok(());
    }

    let balance = get_wallet_balance(&mut tx, wallet_id).await?;
    sqlx::query("UPDATE deposits SET status = 'ORPHANED'::deposit_status, confirmations = $2, orphaned_at = now() WHERE id = $1")
        .bind(deposit_id)
        .bind(onchain_confirmations.unwrap_or(0))
        .execute(&mut *tx)
        .await?;

    if balance >= amount {
        apply_ledger_entry(
            &mut tx,
            LedgerCreditInput { reference_key: None,
                wallet_id,
                amount: -amount,
                ledger_type: "DEPOSIT_REVERSAL",
                reference_id: Some(deposit_id),
                reference_type: Some("Deposit"),
                memo: Some(&format!("Reorg reversal: {reason}")),
            },
        )
        .await?;
        insert_audit_log(&mut tx, "DEPOSIT_REORG_REVERSED", "Deposit", deposit_id, serde_json::json!({ "reason": reason })).await?;
    } else {
        sqlx::query("UPDATE wallets SET reorg_hold_at = now() WHERE id = $1").bind(wallet_id).execute(&mut *tx).await?;
        insert_audit_log(
            &mut tx,
            "DEPOSIT_REORG_REVERSAL_BLOCKED",
            "Deposit",
            deposit_id,
            serde_json::json!({ "reason": reason, "amount": amount.to_string(), "availableBalance": balance.to_string() }),
        )
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn insert_audit_log(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    action: &str,
    entity: &str,
    entity_id: Uuid,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO audit_logs (action, entity, entity_id, metadata) VALUES ($1, $2, $3, $4)")
        .bind(action)
        .bind(entity)
        .bind(entity_id)
        .bind(metadata)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DepositHistoryRecord {
    pub id: Uuid,
    pub coin: String,
    pub tx_hash: String,
    pub vout: i32,
    pub amount: String,
    pub confirmations: i32,
    pub min_confirmations: i32,
    pub status: String,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub credited_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_user_deposits(
    pool: &PgPool,
    user_id: Uuid,
    coin_filter: Option<shared::Coin>,
    limit: i64,
) -> Result<Vec<DepositHistoryRecord>, sqlx::Error> {
    let rows = if let Some(c) = coin_filter {
        sqlx::query(
            r#"
            SELECT d.id, w.coin::text as coin, d.tx_hash, d.vout, d.amount, d.confirmations,
                   d.status::text as status, d.detected_at, d.credited_at
            FROM deposits d
            JOIN wallets w ON w.id = d.wallet_id
            WHERE w.user_id = $1 AND w.coin = $2::coin
            ORDER BY d.detected_at DESC
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
            SELECT d.id, w.coin::text as coin, d.tx_hash, d.vout, d.amount, d.confirmations,
                   d.status::text as status, d.detected_at, d.credited_at
            FROM deposits d
            JOIN wallets w ON w.id = d.wallet_id
            WHERE w.user_id = $1
            ORDER BY d.detected_at DESC
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
            let coin_str: String = r.get("coin");
            let coin_parsed = shared::COINS.into_iter().find(|c| c.as_str() == coin_str);
            let min_confs = coin_parsed
                .map(|c| coin_config(c).min_confirmations as i32)
                .unwrap_or(30);
            let amount: BigDecimal = r.get("amount");
            DepositHistoryRecord {
                id: r.get("id"),
                coin: coin_str,
                tx_hash: r.get("tx_hash"),
                vout: r.get("vout"),
                amount: amount.to_string(),
                confirmations: r.get("confirmations"),
                min_confirmations: min_confs,
                status: r.get("status"),
                detected_at: r.get("detected_at"),
                credited_at: r.get("credited_at"),
            }
        })
        .collect())
}
