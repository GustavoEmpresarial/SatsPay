//! Port 1:1 of legacy `apps/api/src/modules/swap/services/swap.service.ts`.
//!
//! Atomic swap against HOUSE inventory:
//!  1. Debit user fromCoin
//!  2. Credit HOUSE fromCoin (inventory + fee retained in pool)
//!  3. Debit HOUSE toCoin (liquidity check — fails closed if pool empty)
//!  4. Credit user toCoin
//!
//! If HOUSE lacks toCoin, the whole transaction rolls back — the user is
//! never debited without receiving the swap output.

use crate::house::{credit_house, debit_house, HouseError};
use crate::ledger::{apply_ledger_entry, lock_wallets, LedgerCreditInput};
use crate::pricing::PricingDbError;
use bigdecimal::BigDecimal;
use events::domain::SwapExecuted;
use events::{DomainEvent, OutboxWriter};
use shared::{compute_swap, Coin, SwapError as QuoteError, SwapQuote};
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

const SWAP_FEE_BPS: u32 = 25; // 0.25% (25 bps)

#[derive(Debug, thiserror::Error)]
pub enum SwapServiceError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    House(#[from] HouseError),
    #[error(transparent)]
    Outbox(#[from] events::OutboxError),
    #[error(transparent)]
    Quote(#[from] QuoteError),
    #[error(transparent)]
    Pricing(#[from] PricingDbError),
    #[error("fromCoin and toCoin must differ")]
    SameCoin,
    #[error("amount too small to swap after fee")]
    AmountTooSmall,
    #[error("slippage exceeded: quote lower than minToAmount")]
    SlippageExceeded,
    #[error("wallet not found")]
    NotFound,
}

#[derive(Debug, Clone)]
pub struct SwapRow {
    pub id: Uuid,
    pub from_amount: u128,
    pub to_amount: u128,
    pub fee_amount: u128,
}

/// Real spot price, read from the Postgres-backed CoinGecko cache
/// (`crate::pricing`). Fails closed (no invented fallback number) if the
/// cache is missing or older than `max_stale`.
pub async fn quote(pool: &PgPool, from_coin: Coin, to_coin: Coin, from_amount: u128, max_stale: Duration) -> Result<SwapQuote, SwapServiceError> {
    if from_coin == to_coin {
        return Err(SwapServiceError::SameCoin);
    }
    let (price_from, decimals_from) = crate::pricing::get_price(pool, from_coin, max_stale).await?;
    let (price_to, decimals_to) = crate::pricing::get_price(pool, to_coin, max_stale).await?;
    debug_assert_eq!(decimals_from, decimals_to, "price cache must use one consistent scale across all coins");
    Ok(compute_swap(from_coin, to_coin, from_amount, price_from, price_to, SWAP_FEE_BPS, decimals_from)?)
}

/// Idempotent per `idempotency_key` scoped to the user: a retried /
/// double-clicked request returns the original swap instead of executing a
/// second conversion.
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    pool: &PgPool,
    user_id: Uuid,
    from_coin: Coin,
    to_coin: Coin,
    from_amount: u128,
    min_to_amount: Option<u128>,
    idempotency_key: &str,
    max_stale: Duration,
) -> Result<(SwapRow, bool), SwapServiceError> {
    let q = quote(pool, from_coin, to_coin, from_amount, max_stale).await?;
    if q.to_amount == 0 {
        return Err(SwapServiceError::AmountTooSmall);
    }
    if let Some(min) = min_to_amount {
        if q.to_amount < min {
            return Err(SwapServiceError::SlippageExceeded);
        }
    }

    let scoped_key = format!("{user_id}:{idempotency_key}");
    if let Some(existing) = find_by_idempotency_key(pool, &scoped_key).await? {
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    if let Some(existing) = find_by_idempotency_key(&mut *tx, &scoped_key).await? {
        tx.commit().await?;
        return Ok((existing, false));
    }

    let from_wallet_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
        .bind(user_id)
        .bind(from_coin.as_str())
        .fetch_optional(&mut *tx)
        .await?;
    let to_wallet_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
        .bind(user_id)
        .bind(to_coin.as_str())
        .fetch_optional(&mut *tx)
        .await?;
    let (Some(from_wallet_id), Some(to_wallet_id)) = (from_wallet_id, to_wallet_id) else { return Err(SwapServiceError::NotFound) };

    let from_house_id = crate::house::get_house_wallet_id(&mut tx, from_coin).await?;
    let to_house_id = crate::house::get_house_wallet_id(&mut tx, to_coin).await?;
    lock_wallets(&mut tx, &[from_wallet_id, to_wallet_id, from_house_id, to_house_id]).await.map_err(|e| SwapServiceError::Db(sqlx::Error::Protocol(e.to_string())))?;

    let insert_result = sqlx::query(
        r#"
        INSERT INTO swaps (user_id, from_coin, to_coin, from_amount, to_amount, fee_amount, fee_bps, price_from, price_to, price_decimals, idempotency_key)
        VALUES ($1, $2::coin, $3::coin, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(from_coin.as_str())
    .bind(to_coin.as_str())
    .bind(BigDecimal::from(q.from_amount))
    .bind(BigDecimal::from(q.to_amount))
    .bind(BigDecimal::from(q.fee_amount))
    .bind(q.fee_bps as i32)
    .bind(BigDecimal::from(q.price_from))
    .bind(BigDecimal::from(q.price_to))
    .bind(q.price_decimals as i32)
    .bind(&scoped_key)
    .fetch_one(&mut *tx)
    .await;

    let swap_id: Uuid = match insert_result {
        Ok(row) => row.get("id"),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            drop(tx);
            if let Some(existing) = find_by_idempotency_key(pool, &scoped_key).await? {
                return Ok((existing, false));
            }
            return Err(SwapServiceError::NotFound);
        }
        Err(e) => return Err(e.into()),
    };

    let from_amount_dec = BigDecimal::from(q.from_amount);
    let to_amount_dec = BigDecimal::from(q.to_amount);

    // User → HOUSE (source coin, including fee retained by platform).
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id: from_wallet_id,
            amount: -from_amount_dec.clone(),
            ledger_type: "SWAP_OUT",
            reference_id: Some(swap_id),
            reference_type: Some("Swap"),
            memo: Some(&format!("Swap → {}", to_coin.as_str())),
        },
    )
    .await
    .map_err(|e| SwapServiceError::Db(sqlx::Error::Protocol(e.to_string())))?;
    credit_house(&mut tx, from_coin, from_amount_dec, "SWAP_IN", swap_id, "Swap", &format!("Swap intake from user {}…", &user_id.to_string()[..8])).await?;

    // HOUSE → User (destination coin — fails closed if pool empty).
    debit_house(&mut tx, to_coin, to_amount_dec.clone(), "SWAP_OUT", swap_id, "Swap", &format!("Swap payout to user {}…", &user_id.to_string()[..8])).await?;
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id: to_wallet_id,
            amount: to_amount_dec,
            ledger_type: "SWAP_IN",
            reference_id: Some(swap_id),
            reference_type: Some("Swap"),
            memo: Some(&format!("Swap from {}", from_coin.as_str())),
        },
    )
    .await
    .map_err(|e| SwapServiceError::Db(sqlx::Error::Protocol(e.to_string())))?;

    OutboxWriter::stage(
        &mut tx,
        &DomainEvent::SwapExecuted(SwapExecuted {
            swap_id,
            wallet_id: from_wallet_id,
            from_coin: from_coin.as_str().to_string(),
            to_coin: to_coin.as_str().to_string(),
            from_amount: q.from_amount.to_string(),
            to_amount: q.to_amount.to_string(),
            fee_amount: q.fee_amount.to_string(),
        }),
    )
    .await?;

    tx.commit().await?;

    Ok((SwapRow { id: swap_id, from_amount: q.from_amount, to_amount: q.to_amount, fee_amount: q.fee_amount }, true))
}

async fn find_by_idempotency_key<'e, E>(executor: E, key: &str) -> Result<Option<SwapRow>, SwapServiceError>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query("SELECT id, from_amount, to_amount, fee_amount FROM swaps WHERE idempotency_key = $1").bind(key).fetch_optional(executor).await?;
    Ok(row.map(|r| {
        let from_amount: BigDecimal = r.get("from_amount");
        let to_amount: BigDecimal = r.get("to_amount");
        let fee_amount: BigDecimal = r.get("fee_amount");
        SwapRow {
            id: r.get("id"),
            from_amount: from_amount.to_string().parse().unwrap_or(0),
            to_amount: to_amount.to_string().parse().unwrap_or(0),
            fee_amount: fee_amount.to_string().parse().unwrap_or(0),
        }
    }))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SwapHistoryRecord {
    pub id: Uuid,
    pub from_coin: String,
    pub to_coin: String,
    pub from_amount: String,
    pub to_amount: String,
    pub fee_amount: String,
    pub fee_bps: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_user_swaps(pool: &PgPool, user_id: Uuid, limit: i64) -> Result<Vec<SwapHistoryRecord>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, from_coin::text as from_coin, to_coin::text as to_coin,
               from_amount, to_amount, fee_amount, fee_bps, created_at
        FROM swaps
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| {
        let from_amount: BigDecimal = r.get("from_amount");
        let to_amount: BigDecimal = r.get("to_amount");
        let fee_amount: BigDecimal = r.get("fee_amount");
        SwapHistoryRecord {
            id: r.get("id"),
            from_coin: r.get("from_coin"),
            to_coin: r.get("to_coin"),
            from_amount: from_amount.to_string(),
            to_amount: to_amount.to_string(),
            fee_amount: fee_amount.to_string(),
            fee_bps: r.get("fee_bps"),
            created_at: r.get("created_at"),
        }
    }).collect())
}
