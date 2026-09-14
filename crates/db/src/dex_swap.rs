//! Custodial DEX swap orders — ledger lock → on-chain SwapKit fill → credit.
//!
//! Instant HOUSE fallback still lives in `crate::swap`; this module owns the
//! async FSM for provider-backed routes.

use crate::ledger::{apply_ledger_entry, lock_wallets, LedgerCreditInput};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::Coin;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum DexSwapError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Ledger(#[from] crate::ledger::LedgerError),
    #[error("fromCoin and toCoin must differ")]
    SameCoin,
    #[error("wallet not found")]
    NotFound,
    #[error("idempotency conflict")]
    IdempotencyConflict,
    #[error("invalid status transition from {0}")]
    BadStatus(String),
    #[error("swap not found")]
    SwapNotFound,
    #[error("slippage exceeded")]
    SlippageExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DexSwapStatus {
    Quoted,
    Locked,
    Broadcasting,
    InFlight,
    Crediting,
    Completed,
    Failed,
    Refunding,
    Refunded,
}

impl DexSwapStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quoted => "QUOTED",
            Self::Locked => "LOCKED",
            Self::Broadcasting => "BROADCASTING",
            Self::InFlight => "IN_FLIGHT",
            Self::Crediting => "CREDITING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Refunding => "REFUNDING",
            Self::Refunded => "REFUNDED",
        }
    }

    #[allow(dead_code)]
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "QUOTED" => Some(Self::Quoted),
            "LOCKED" => Some(Self::Locked),
            "BROADCASTING" => Some(Self::Broadcasting),
            "IN_FLIGHT" => Some(Self::InFlight),
            "CREDITING" => Some(Self::Crediting),
            "COMPLETED" => Some(Self::Completed),
            "FAILED" => Some(Self::Failed),
            "REFUNDING" => Some(Self::Refunding),
            "REFUNDED" => Some(Self::Refunded),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LockDexSwapInput<'a> {
    pub user_id: Uuid,
    pub from_coin: Coin,
    pub to_coin: Coin,
    pub from_amount: u128,
    pub expected_to_amount: u128,
    pub min_to_amount: Option<u128>,
    pub provider: &'a str,
    pub providers: &'a [String],
    pub route_id: Option<&'a str>,
    pub quote_id: Option<&'a str>,
    pub platform_fee_bps: u32,
    pub platform_fee_amount: u128,
    pub fees_json: Value,
    pub eta_seconds: Option<i32>,
    pub tx_hint: Option<&'a str>,
    pub destination_address: Option<&'a str>,
    pub source_address: Option<&'a str>,
    pub swap_payload: Option<Value>,
    pub idempotency_key: &'a str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DexSwapRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub from_coin: String,
    pub to_coin: String,
    pub from_amount: String,
    pub expected_to_amount: String,
    pub min_to_amount: Option<String>,
    pub actual_to_amount: Option<String>,
    pub status: String,
    pub provider: String,
    pub providers: Vec<String>,
    pub route_id: Option<String>,
    pub quote_id: Option<String>,
    pub platform_fee_bps: i32,
    pub platform_fee_amount: String,
    pub fees_json: Value,
    pub eta_seconds: Option<i32>,
    pub tx_hint: Option<String>,
    pub inbound_memo: Option<String>,
    pub deposit_address: Option<String>,
    pub destination_address: Option<String>,
    pub source_address: Option<String>,
    pub inbound_tx: Option<String>,
    pub outbound_tx: Option<String>,
    pub swap_payload: Option<Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Debit user `fromCoin` and insert a LOCKED dex_swaps row (idempotent).
pub async fn lock_and_create(pool: &PgPool, input: LockDexSwapInput<'_>) -> Result<(DexSwapRow, bool), DexSwapError> {
    if input.from_coin == input.to_coin {
        return Err(DexSwapError::SameCoin);
    }
    if let Some(min) = input.min_to_amount {
        if input.expected_to_amount < min {
            return Err(DexSwapError::SlippageExceeded);
        }
    }

    let scoped_key = format!("{}:{}", input.user_id, input.idempotency_key);
    if let Some(existing) = find_by_idempotency(pool, &scoped_key).await? {
        return Ok((existing, false));
    }

    let mut tx = pool.begin().await?;
    if let Some(existing) = find_by_idempotency(&mut *tx, &scoped_key).await? {
        tx.commit().await?;
        return Ok((existing, false));
    }

    let from_wallet_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'",
    )
    .bind(input.user_id)
    .bind(input.from_coin.as_str())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(from_wallet_id) = from_wallet_id else {
        return Err(DexSwapError::NotFound);
    };
    // Ensure destination wallet exists for later credit.
    let to_wallet_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'",
    )
    .bind(input.user_id)
    .bind(input.to_coin.as_str())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(to_wallet_id) = to_wallet_id else {
        return Err(DexSwapError::NotFound);
    };
    let _ = to_wallet_id;

    lock_wallets(&mut tx, &[from_wallet_id]).await?;

    let providers: Vec<String> = input.providers.to_vec();
    let insert = sqlx::query(
        r#"
        INSERT INTO dex_swaps (
            user_id, from_coin, to_coin, from_amount, expected_to_amount, min_to_amount,
            status, provider, providers, route_id, quote_id, platform_fee_bps, platform_fee_amount,
            fees_json, eta_seconds, tx_hint, destination_address, source_address, swap_payload,
            idempotency_key, locked_at
        ) VALUES (
            $1, $2::coin, $3::coin, $4, $5, $6,
            'LOCKED', $7, $8, $9, $10, $11, $12,
            $13::jsonb, $14, $15, $16, $17, $18::jsonb,
            $19, now()
        )
        RETURNING id
        "#,
    )
    .bind(input.user_id)
    .bind(input.from_coin.as_str())
    .bind(input.to_coin.as_str())
    .bind(BigDecimal::from(input.from_amount))
    .bind(BigDecimal::from(input.expected_to_amount))
    .bind(input.min_to_amount.map(BigDecimal::from))
    .bind(input.provider)
    .bind(&providers)
    .bind(input.route_id)
    .bind(input.quote_id)
    .bind(input.platform_fee_bps as i32)
    .bind(BigDecimal::from(input.platform_fee_amount))
    .bind(input.fees_json.to_string())
    .bind(input.eta_seconds)
    .bind(input.tx_hint)
    .bind(input.destination_address)
    .bind(input.source_address)
    .bind(input.swap_payload.as_ref().map(|v| v.to_string()))
    .bind(&scoped_key)
    .fetch_one(&mut *tx)
    .await;

    let swap_id: Uuid = match insert {
        Ok(row) => row.get("id"),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            drop(tx);
            if let Some(existing) = find_by_idempotency(pool, &scoped_key).await? {
                return Ok((existing, false));
            }
            return Err(DexSwapError::IdempotencyConflict);
        }
        Err(e) => return Err(e.into()),
    };

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            reference_key: None,
            wallet_id: from_wallet_id,
            amount: -BigDecimal::from(input.from_amount),
            ledger_type: "DEX_SWAP_OUT",
            reference_id: Some(swap_id),
            reference_type: Some("DexSwap"),
            memo: Some(&format!("DEX swap → {}", input.to_coin.as_str())),
        },
    )
    .await?;

    tx.commit().await?;
    let row = get_by_id(pool, swap_id).await?.ok_or(DexSwapError::SwapNotFound)?;
    Ok((row, true))
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<DexSwapRow>, DexSwapError> {
    let row = sqlx::query(DEX_SWAP_SELECT).bind(id).fetch_optional(pool).await?;
    Ok(row.map(map_row))
}

pub async fn list_user(pool: &PgPool, user_id: Uuid, limit: i64) -> Result<Vec<DexSwapRow>, DexSwapError> {
    let rows = sqlx::query(&format!("{DEX_SWAP_SELECT_BASE} WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2"))
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub async fn list_active(pool: &PgPool, limit: i64) -> Result<Vec<DexSwapRow>, DexSwapError> {
    let rows = sqlx::query(&format!(
        "{DEX_SWAP_SELECT_BASE} WHERE status IN ('LOCKED','BROADCASTING','IN_FLIGHT','CREDITING','FAILED','REFUNDING') ORDER BY updated_at ASC LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(map_row).collect())
}

pub async fn mark_broadcasting(
    pool: &PgPool,
    id: Uuid,
    deposit_address: &str,
    memo: Option<&str>,
    swap_payload: &Value,
) -> Result<(), DexSwapError> {
    let n = sqlx::query(
        r#"
        UPDATE dex_swaps SET
            status = 'BROADCASTING',
            deposit_address = $2,
            inbound_memo = $3,
            swap_payload = $4::jsonb,
            updated_at = now()
        WHERE id = $1 AND status IN ('LOCKED', 'BROADCASTING')
        "#,
    )
    .bind(id)
    .bind(deposit_address)
    .bind(memo)
    .bind(swap_payload.to_string())
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(DexSwapError::BadStatus("not LOCKED".into()));
    }
    Ok(())
}

pub async fn mark_in_flight(pool: &PgPool, id: Uuid, inbound_tx: &str) -> Result<(), DexSwapError> {
    let n = sqlx::query(
        r#"
        UPDATE dex_swaps SET
            status = 'IN_FLIGHT',
            inbound_tx = $2,
            broadcast_at = coalesce(broadcast_at, now()),
            updated_at = now()
        WHERE id = $1 AND status IN ('BROADCASTING', 'IN_FLIGHT')
        "#,
    )
    .bind(id)
    .bind(inbound_tx)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(DexSwapError::BadStatus("not BROADCASTING".into()));
    }
    Ok(())
}

/// Credit user `toCoin` and mark COMPLETED.
pub async fn credit_and_complete(pool: &PgPool, id: Uuid, to_amount: u128, outbound_tx: Option<&str>) -> Result<(), DexSwapError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        r#"
        SELECT id, user_id, to_coin::text AS to_coin, status::text AS status
        FROM dex_swaps WHERE id = $1 FOR UPDATE
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DexSwapError::SwapNotFound)?;

    let status: String = row.get("status");
    if status == "COMPLETED" {
        tx.commit().await?;
        return Ok(());
    }
    if !matches!(status.as_str(), "IN_FLIGHT" | "CREDITING" | "BROADCASTING") {
        return Err(DexSwapError::BadStatus(status));
    }

    let user_id: Uuid = row.get("user_id");
    let to_coin: String = row.get("to_coin");

    sqlx::query("UPDATE dex_swaps SET status = 'CREDITING', updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    let to_wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'",
    )
    .bind(user_id)
    .bind(&to_coin)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DexSwapError::NotFound)?;

    lock_wallets(&mut tx, &[to_wallet_id]).await?;

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            reference_key: None,
            wallet_id: to_wallet_id,
            amount: BigDecimal::from(to_amount),
            ledger_type: "DEX_SWAP_IN",
            reference_id: Some(id),
            reference_type: Some("DexSwap"),
            memo: Some("DEX swap credit"),
        },
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE dex_swaps SET
            status = 'COMPLETED',
            actual_to_amount = $2,
            outbound_tx = coalesce($3, outbound_tx),
            completed_at = now(),
            updated_at = now(),
            error = NULL
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(BigDecimal::from(to_amount))
    .bind(outbound_tx)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn mark_failed(pool: &PgPool, id: Uuid, error: &str) -> Result<(), DexSwapError> {
    sqlx::query(
        r#"
        UPDATE dex_swaps SET status = 'FAILED', error = $2, updated_at = now()
        WHERE id = $1 AND status NOT IN ('COMPLETED', 'REFUNDED')
        "#,
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Refund locked `fromCoin` to the user after a failed broadcast / provider reject.
pub async fn refund(pool: &PgPool, id: Uuid, reason: &str) -> Result<(), DexSwapError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        r#"
        SELECT id, user_id, from_coin::text AS from_coin, from_amount, status::text AS status
        FROM dex_swaps WHERE id = $1 FOR UPDATE
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DexSwapError::SwapNotFound)?;

    let status: String = row.get("status");
    if status == "REFUNDED" {
        tx.commit().await?;
        return Ok(());
    }
    if matches!(status.as_str(), "COMPLETED" | "CREDITING") {
        return Err(DexSwapError::BadStatus(status));
    }

    let user_id: Uuid = row.get("user_id");
    let from_coin: String = row.get("from_coin");
    let from_amount: BigDecimal = row.get("from_amount");

    sqlx::query("UPDATE dex_swaps SET status = 'REFUNDING', error = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(reason)
        .execute(&mut *tx)
        .await?;

    let from_wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'",
    )
    .bind(user_id)
    .bind(&from_coin)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DexSwapError::NotFound)?;

    lock_wallets(&mut tx, &[from_wallet_id]).await?;

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            reference_key: Some(&format!("dex-swap-refund:{id}")),
            wallet_id: from_wallet_id,
            amount: from_amount,
            ledger_type: "DEX_SWAP_REFUND",
            reference_id: Some(id),
            reference_type: Some("DexSwap"),
            memo: Some(reason),
        },
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE dex_swaps SET status = 'REFUNDED', completed_at = now(), updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

async fn find_by_idempotency<'e, E>(executor: E, key: &str) -> Result<Option<DexSwapRow>, DexSwapError>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(&format!("{DEX_SWAP_SELECT_BASE} WHERE idempotency_key = $1"))
        .bind(key)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(map_row))
}

const DEX_SWAP_SELECT_BASE: &str = r#"
    SELECT id, user_id,
           from_coin::text AS from_coin, to_coin::text AS to_coin,
           from_amount, expected_to_amount, min_to_amount, actual_to_amount,
           status::text AS status, provider, providers, route_id, quote_id,
           platform_fee_bps, platform_fee_amount, fees_json::text AS fees_json, eta_seconds, tx_hint,
           inbound_memo, deposit_address, destination_address, source_address,
           inbound_tx, outbound_tx, swap_payload::text AS swap_payload, error,
           created_at, updated_at, completed_at
    FROM dex_swaps
"#;

const DEX_SWAP_SELECT: &str = r#"
    SELECT id, user_id,
           from_coin::text AS from_coin, to_coin::text AS to_coin,
           from_amount, expected_to_amount, min_to_amount, actual_to_amount,
           status::text AS status, provider, providers, route_id, quote_id,
           platform_fee_bps, platform_fee_amount, fees_json::text AS fees_json, eta_seconds, tx_hint,
           inbound_memo, deposit_address, destination_address, source_address,
           inbound_tx, outbound_tx, swap_payload::text AS swap_payload, error,
           created_at, updated_at, completed_at
    FROM dex_swaps WHERE id = $1
"#;

fn map_row(r: sqlx::postgres::PgRow) -> DexSwapRow {
    let bd = |col: &str| -> String {
        let v: BigDecimal = r.get(col);
        v.to_string()
    };
    let bd_opt = |col: &str| -> Option<String> {
        let v: Option<BigDecimal> = r.get(col);
        v.map(|x| x.to_string())
    };
    let fees_s: String = r.get("fees_json");
    let fees: Value = serde_json::from_str(&fees_s).unwrap_or(Value::Array(vec![]));
    let payload_s: Option<String> = r.get("swap_payload");
    let payload = payload_s.and_then(|t| serde_json::from_str(&t).ok());

    DexSwapRow {
        id: r.get("id"),
        user_id: r.get("user_id"),
        from_coin: r.get("from_coin"),
        to_coin: r.get("to_coin"),
        from_amount: bd("from_amount"),
        expected_to_amount: bd("expected_to_amount"),
        min_to_amount: bd_opt("min_to_amount"),
        actual_to_amount: bd_opt("actual_to_amount"),
        status: r.get("status"),
        provider: r.get("provider"),
        providers: r.get("providers"),
        route_id: r.get("route_id"),
        quote_id: r.get("quote_id"),
        platform_fee_bps: r.get("platform_fee_bps"),
        platform_fee_amount: bd("platform_fee_amount"),
        fees_json: fees,
        eta_seconds: r.get("eta_seconds"),
        tx_hint: r.get("tx_hint"),
        inbound_memo: r.get("inbound_memo"),
        deposit_address: r.get("deposit_address"),
        destination_address: r.get("destination_address"),
        source_address: r.get("source_address"),
        inbound_tx: r.get("inbound_tx"),
        outbound_tx: r.get("outbound_tx"),
        swap_payload: payload,
        error: r.get("error"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        completed_at: r.get("completed_at"),
    }
}

/// Admin/telemetry aggregates for open DEX swaps.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DexSwapTelemetry {
    pub open_count: i64,
    pub completed_24h: i64,
    pub failed_24h: i64,
    pub refunded_24h: i64,
    pub avg_platform_fee_bps_24h: Option<f64>,
}

pub async fn telemetry_snapshot(pool: &PgPool) -> Result<DexSwapTelemetry, DexSwapError> {
    let open_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM dex_swaps WHERE status NOT IN ('COMPLETED','REFUNDED')",
    )
    .fetch_one(pool)
    .await?;
    let completed_24h: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM dex_swaps WHERE status = 'COMPLETED' AND completed_at > now() - interval '24 hours'",
    )
    .fetch_one(pool)
    .await?;
    let failed_24h: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM dex_swaps WHERE status = 'FAILED' AND updated_at > now() - interval '24 hours'",
    )
    .fetch_one(pool)
    .await?;
    let refunded_24h: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM dex_swaps WHERE status = 'REFUNDED' AND completed_at > now() - interval '24 hours'",
    )
    .fetch_one(pool)
    .await?;
    let avg: Option<f64> = sqlx::query_scalar(
        "SELECT avg(platform_fee_bps)::float8 FROM dex_swaps WHERE created_at > now() - interval '24 hours'",
    )
    .fetch_one(pool)
    .await?;
    Ok(DexSwapTelemetry {
        open_count,
        completed_24h,
        failed_24h,
        refunded_24h,
        avg_platform_fee_bps_24h: avg,
    })
}

#[allow(dead_code)]
fn _status_used(s: DexSwapStatus) -> &'static str {
    s.as_str()
}
