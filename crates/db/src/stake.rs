//! Port 1:1 of legacy `apps/api/src/modules/stake/{services,repositories}/stake.*.ts`.

use crate::house::{debit_house, HouseError};
use crate::ledger::{apply_ledger_entry, LedgerCreditInput};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use shared::{calc_flex_reward, calc_stake_reward, is_flexible_plan, Coin, STAKE_PLANS};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StakeError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    House(#[from] HouseError),
    #[error("invalid stake plan")]
    InvalidPlan,
    #[error("wallet not found")]
    WalletNotFound,
    #[error("stake not found")]
    NotFound,
    #[error("stake is not active")]
    NotActive,
    #[error("stake has not matured yet")]
    NotMatured,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StakingStrategyRow {
    pub coin: String,
    pub protocol: String,
    pub strategy_name: String,
    pub strategy_type: String,
    pub base_apy_bps: u32,
    pub performance_fee_bps: u32,
    pub net_apy_bps: u32,
    pub min_stake: f64,
    pub tvl_usd: f64,
    pub is_active: bool,
}

pub async fn list_staking_strategies(pool: &PgPool) -> Result<Vec<StakingStrategyRow>, StakeError> {
    let rows = sqlx::query(
        r#"
        SELECT coin, protocol, strategy_name, strategy_type, 
               base_apy_bps, performance_fee_bps, 
               min_stake::float8 as min_stake, 
               tvl_usd::float8 as tvl_usd, 
               is_active
        FROM staking_strategies
        WHERE is_active = TRUE
        ORDER BY base_apy_bps DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let out = rows
        .into_iter()
        .map(|r| {
            let base: i32 = r.get("base_apy_bps");
            let fee: i32 = r.get("performance_fee_bps");
            let net = (base as u32).saturating_sub((base as u32 * fee as u32) / 10_000);
            StakingStrategyRow {
                coin: r.get("coin"),
                protocol: r.get("protocol"),
                strategy_name: r.get("strategy_name"),
                strategy_type: r.get("strategy_type"),
                base_apy_bps: base as u32,
                performance_fee_bps: fee as u32,
                net_apy_bps: net,
                min_stake: r.get("min_stake"),
                tvl_usd: r.get("tvl_usd"),
                is_active: r.get("is_active"),
            }
        })
        .collect();

    Ok(out)
}

#[derive(Debug, Clone)]
pub struct StakeRow {
    pub id: Uuid,
    pub coin: Coin,
    pub principal: u128,
    pub reward_bps: u32,
    pub lock_days: u32,
    pub reward: u128,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub matures_at: DateTime<Utc>,
}

pub async fn create_stake(pool: &PgPool, user_id: Uuid, coin: Coin, amount: u128, lock_days: u32) -> Result<StakeRow, StakeError> {
    let plan = STAKE_PLANS.iter().find(|p| p.lock_days == lock_days).ok_or(StakeError::InvalidPlan)?;

    let mut tx = pool.begin().await?;
    let wallet_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
        .bind(user_id)
        .bind(coin.as_str())
        .fetch_optional(&mut *tx)
        .await?;
    let wallet_id = wallet_id.ok_or(StakeError::WalletNotFound)?;

    let matures_at = Utc::now() + Duration::days(lock_days as i64);
    let stake_id: Uuid = sqlx::query_scalar(
        "INSERT INTO stakes (user_id, coin, principal, reward_bps, lock_days, matures_at) VALUES ($1, $2::coin, $3, $4, $5, $6) RETURNING id",
    )
    .bind(user_id)
    .bind(coin.as_str())
    .bind(BigDecimal::from(amount))
    .bind(plan.reward_bps as i32)
    .bind(lock_days as i32)
    .bind(matures_at)
    .fetch_one(&mut *tx)
    .await?;

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id,
            amount: -BigDecimal::from(amount),
            ledger_type: "STAKE_LOCK",
            reference_id: Some(stake_id),
            reference_type: Some("Stake"),
            memo: Some(&format!("Stake locked · {}", plan.label)),
        },
    )
    .await
    .map_err(|e| StakeError::Db(sqlx::Error::Protocol(e.to_string())))?;

    tx.commit().await?;
    Ok(StakeRow { id: stake_id, coin, principal: amount, reward_bps: plan.reward_bps, lock_days, reward: 0, status: "ACTIVE".to_string(), started_at: Utc::now(), matures_at })
}

pub async fn list_stakes(pool: &PgPool, user_id: Uuid) -> Result<Vec<StakeRow>, StakeError> {
    let rows = sqlx::query(
        "SELECT id, coin::text as coin, principal, reward_bps, lock_days, reward, status::text as status, started_at, matures_at \
         FROM stakes WHERE user_id = $1 ORDER BY status ASC, started_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(row_to_stake).collect())
}

fn row_to_stake(r: sqlx::postgres::PgRow) -> Option<StakeRow> {
    let coin_str: String = r.get("coin");
    let coin = coin_str.parse::<Coin>().ok()?;
    let principal: BigDecimal = r.get("principal");
    let reward: BigDecimal = r.get("reward");
    Some(StakeRow {
        id: r.get("id"),
        coin,
        principal: principal.to_string().parse().unwrap_or(0),
        reward_bps: r.get::<i32, _>("reward_bps") as u32,
        lock_days: r.get::<i32, _>("lock_days") as u32,
        reward: reward.to_string().parse().unwrap_or(0),
        status: r.get("status"),
        started_at: r.get("started_at"),
        matures_at: r.get("matures_at"),
    })
}

/// Locks the stake row FIRST, then reads its status — two concurrent claims
/// would otherwise both observe ACTIVE and both pay out principal + reward.
pub async fn claim_stake(pool: &PgPool, user_id: Uuid, stake_id: Uuid) -> Result<StakeRow, StakeError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM stakes WHERE id = $1 FOR UPDATE").bind(stake_id).fetch_optional(&mut *tx).await?;

    let row = sqlx::query(
        "SELECT id, user_id, coin::text as coin, principal, reward_bps, lock_days, status::text as status, started_at, matures_at \
         FROM stakes WHERE id = $1",
    )
    .bind(stake_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else { return Err(StakeError::NotFound) };
    let owner: Uuid = row.get("user_id");
    if owner != user_id {
        return Err(StakeError::NotFound);
    }
    let status: String = row.get("status");
    if status != "ACTIVE" {
        return Err(StakeError::NotActive);
    }

    let coin_str: String = row.get("coin");
    let coin = coin_str.parse::<Coin>().map_err(|_| StakeError::NotFound)?;
    let principal: BigDecimal = row.get("principal");
    let principal_u128: u128 = principal.to_string().parse().unwrap_or(0);
    let reward_bps: i32 = row.get("reward_bps");
    let lock_days: i32 = row.get("lock_days");
    let started_at: DateTime<Utc> = row.get("started_at");
    let matures_at: DateTime<Utc> = row.get("matures_at");

    let flexible = is_flexible_plan(lock_days as u32);
    if !flexible && matures_at > Utc::now() {
        return Err(StakeError::NotMatured);
    }

    let wallet_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
        .bind(user_id)
        .bind(coin.as_str())
        .fetch_optional(&mut *tx)
        .await?;
    let wallet_id = wallet_id.ok_or(StakeError::WalletNotFound)?;

    let reward = if flexible {
        let elapsed_ms = (Utc::now() - started_at).num_milliseconds();
        calc_flex_reward(principal_u128, reward_bps as u32, elapsed_ms)
    } else {
        calc_stake_reward(principal_u128, reward_bps as u32, lock_days as u32)
    };

    // Principal returns from lock (not from HOUSE). Reward is paid from HOUSE inventory.
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id,
            amount: principal.clone(),
            ledger_type: "STAKE_UNLOCK",
            reference_id: Some(stake_id),
            reference_type: Some("Stake"),
            memo: Some("Stake principal returned"),
        },
    )
    .await
    .map_err(|e| StakeError::Db(sqlx::Error::Protocol(e.to_string())))?;

    if reward > 0 {
        let memo = format!("Stake reward paid · {}% APY × {}d", reward_bps as f64 / 100.0, lock_days);
        debit_house(&mut tx, coin, BigDecimal::from(reward), "STAKE_REWARD", stake_id, "Stake", &memo).await?;
        apply_ledger_entry(
            &mut tx,
            LedgerCreditInput { reference_key: None,
                wallet_id,
                amount: BigDecimal::from(reward),
                ledger_type: "STAKE_REWARD",
                reference_id: Some(stake_id),
                reference_type: Some("Stake"),
                memo: Some(&memo),
            },
        )
        .await
        .map_err(|e| StakeError::Db(sqlx::Error::Protocol(e.to_string())))?;
    }

    sqlx::query("UPDATE stakes SET status = 'COMPLETED'::stake_status, claimed_at = now(), reward = $2 WHERE id = $1")
        .bind(stake_id)
        .bind(BigDecimal::from(reward))
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(StakeRow { id: stake_id, coin, principal: principal_u128, reward_bps: reward_bps as u32, lock_days: lock_days as u32, reward, status: "COMPLETED".to_string(), started_at, matures_at })
}

/// Early exit: returns principal only, forfeits any reward.
pub async fn cancel_stake(pool: &PgPool, user_id: Uuid, stake_id: Uuid) -> Result<(), StakeError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM stakes WHERE id = $1 FOR UPDATE").bind(stake_id).fetch_optional(&mut *tx).await?;

    let row = sqlx::query("SELECT user_id, coin::text as coin, principal, status::text as status FROM stakes WHERE id = $1").bind(stake_id).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Err(StakeError::NotFound) };
    let owner: Uuid = row.get("user_id");
    if owner != user_id {
        return Err(StakeError::NotFound);
    }
    let status: String = row.get("status");
    if status != "ACTIVE" {
        return Err(StakeError::NotActive);
    }
    let coin_str: String = row.get("coin");
    let coin = coin_str.parse::<Coin>().map_err(|_| StakeError::NotFound)?;
    let principal: BigDecimal = row.get("principal");

    let wallet_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
        .bind(user_id)
        .bind(coin.as_str())
        .fetch_optional(&mut *tx)
        .await?;
    let wallet_id = wallet_id.ok_or(StakeError::WalletNotFound)?;

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id,
            amount: principal,
            ledger_type: "STAKE_UNLOCK",
            reference_id: Some(stake_id),
            reference_type: Some("Stake"),
            memo: Some("Stake canceled early (no reward)"),
        },
    )
    .await
    .map_err(|e| StakeError::Db(sqlx::Error::Protocol(e.to_string())))?;

    sqlx::query("UPDATE stakes SET status = 'CANCELED'::stake_status, claimed_at = now() WHERE id = $1").bind(stake_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
