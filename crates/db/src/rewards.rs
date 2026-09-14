//! Port 1:1 of legacy `apps/api/src/modules/rewards/{services,repositories}/rewards.*.ts`
//! — liquidity-mining programs: emit `reward_coin` continuously to every
//! user weighted by their total economic activity (wallet balances + active
//! stake principal + lend supply/debt), paid from HOUSE inventory, no claim
//! step (auto-credited straight to PERSONAL wallets).

use crate::house::{debit_house, HouseError, HOUSE_EMAIL};
use crate::ledger::{apply_ledger_entry, get_wallet_balance, lock_wallets, LedgerCreditInput};
use crate::pricing::{load_price_table, PriceTable, PricingDbError};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};
use shared::{coin_config, scaled_to_amount, Coin, RAY};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RewardsError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    House(#[from] HouseError),
    #[error(transparent)]
    Pricing(#[from] PricingDbError),
    #[error("emissionPerDay must be > 0")]
    InvalidEmission,
    #[error("program not found")]
    NotFound,
}

/// USD value (scaled to the price table's decimals) of an amount, consistent
/// across coins. Signed (`i128`) because it's summed across many positions
/// before ever being converted back to an amount, and legacy's `bigint` has
/// no unsigned constraint either.
fn value_usd(prices: &PriceTable, coin: Coin, amount: BigInt) -> BigInt {
    let price = BigInt::from(prices.price_scaled(coin));
    let decimals = coin_config(coin).decimals;
    (amount * price) / BigInt::from(10u128.pow(decimals))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RewardProgramView {
    pub id: Uuid,
    pub reward_coin: Coin,
    pub market_coin: Coin,
    pub side: String,
    pub emission_per_day: u128,
    pub start_at: DateTime<Utc>,
    pub end_at: Option<DateTime<Utc>>,
    pub active: bool,
}

#[allow(clippy::too_many_arguments)]
pub async fn create_program(pool: &PgPool, admin_id: Uuid, reward_coin: Coin, market_coin: Coin, side: &str, emission_per_day: u128, start_at: Option<DateTime<Utc>>, end_at: Option<DateTime<Utc>>) -> Result<RewardProgramView, RewardsError> {
    if emission_per_day == 0 {
        return Err(RewardsError::InvalidEmission);
    }
    let start = start_at.unwrap_or_else(Utc::now);
    let row = sqlx::query(
        "INSERT INTO reward_programs (reward_coin, market_coin, side, emission_per_day, start_at, end_at, last_emitted_at, created_by_id) \
         VALUES ($1::coin, $2::coin, $3::reward_side, $4, $5, $6, $5, $7) \
         RETURNING id, reward_coin::text as reward_coin, market_coin::text as market_coin, side::text as side, emission_per_day, start_at, end_at, active",
    )
    .bind(reward_coin.as_str())
    .bind(market_coin.as_str())
    .bind(side)
    .bind(BigDecimal::from(emission_per_day))
    .bind(start)
    .bind(end_at)
    .bind(admin_id)
    .fetch_one(pool)
    .await?;
    row_to_program(row).ok_or(RewardsError::NotFound)
}

pub async fn list_programs(pool: &PgPool) -> Result<Vec<RewardProgramView>, RewardsError> {
    let rows = sqlx::query(
        "SELECT id, reward_coin::text as reward_coin, market_coin::text as market_coin, side::text as side, emission_per_day, start_at, end_at, active \
         FROM reward_programs ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(row_to_program).collect())
}

pub async fn set_program_active(pool: &PgPool, id: Uuid, active: bool) -> Result<(), RewardsError> {
    let result = sqlx::query("UPDATE reward_programs SET active = $2 WHERE id = $1").bind(id).bind(active).execute(pool).await?;
    if result.rows_affected() == 0 {
        return Err(RewardsError::NotFound);
    }
    Ok(())
}

fn row_to_program(row: sqlx::postgres::PgRow) -> Option<RewardProgramView> {
    let reward_coin: String = row.get("reward_coin");
    let market_coin: String = row.get("market_coin");
    let emission: BigDecimal = row.get("emission_per_day");
    Some(RewardProgramView {
        id: row.get("id"),
        reward_coin: reward_coin.parse().ok()?,
        market_coin: market_coin.parse().ok()?,
        side: row.get("side"),
        emission_per_day: emission.to_string().parse().unwrap_or(0),
        start_at: row.get("start_at"),
        end_at: row.get("end_at"),
        active: row.get("active"),
    })
}

/// Aggregates each user's total economic weight in USD (scaled 1e8): PERSONAL
/// wallet balances + active stake principals + lending supply + debt. The
/// internal HOUSE/system account is excluded.
async fn advance_program(tx: &mut Transaction<'_, Postgres>, program_id: Uuid, now: DateTime<Utc>, finished: bool) -> Result<(), sqlx::Error> {
    if finished {
        sqlx::query("UPDATE reward_programs SET last_emitted_at = $2, active = false WHERE id = $1").bind(program_id).bind(now).execute(&mut **tx).await?;
    } else {
        sqlx::query("UPDATE reward_programs SET last_emitted_at = $2 WHERE id = $1").bind(program_id).bind(now).execute(&mut **tx).await?;
    }
    Ok(())
}

async fn compute_user_weights(tx: &mut Transaction<'_, Postgres>, prices: &PriceTable) -> Result<HashMap<Uuid, BigInt>, RewardsError> {
    let house_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1").bind(HOUSE_EMAIL).fetch_optional(&mut **tx).await?;

    let mut weights: HashMap<Uuid, BigInt> = HashMap::new();
    let mut add = |user_id: Uuid, coin: Coin, amount: BigInt| {
        if Some(user_id) == house_id || !amount.is_positive() {
            return;
        }
        let usd = value_usd(prices, coin, amount);
        weights.entry(user_id).and_modify(|w| *w += usd.clone()).or_insert(usd);
    };

    // 1) Wallet money (PERSONAL balances, derived from the ledger).
    let wallet_rows = sqlx::query(
        "SELECT w.user_id as uid, w.coin::text as coin, COALESCE(SUM(l.amount), 0) as bal \
         FROM wallets w LEFT JOIN ledger_entries l ON l.wallet_id = w.id WHERE w.kind = 'PERSONAL' GROUP BY w.user_id, w.coin",
    )
    .fetch_all(&mut **tx)
    .await?;
    for r in wallet_rows {
        let coin_str: String = r.get("coin");
        if let Ok(coin) = coin_str.parse::<Coin>() {
            let bal: BigDecimal = r.get("bal");
            add(r.get("uid"), coin, bal.to_string().parse().unwrap_or_else(|_| BigInt::from(0)));
        }
    }

    // 2) Active stakes (locked principal still counts as skin in the game).
    let stake_rows = sqlx::query("SELECT user_id as uid, coin::text as coin, COALESCE(SUM(principal), 0) as p FROM stakes WHERE status = 'ACTIVE'::stake_status GROUP BY user_id, coin").fetch_all(&mut **tx).await?;
    for r in stake_rows {
        let coin_str: String = r.get("coin");
        if let Ok(coin) = coin_str.parse::<Coin>() {
            let p: BigDecimal = r.get("p");
            add(r.get("uid"), coin, p.to_string().parse().unwrap_or_else(|_| BigInt::from(0)));
        }
    }

    // 3) Lending positions (supply + outstanding debt), valued at live indexes.
    let reserve_rows = sqlx::query("SELECT coin::text as coin, liquidity_index, borrow_index FROM lend_reserves").fetch_all(&mut **tx).await?;
    let mut idx: HashMap<Coin, (u128, u128)> = HashMap::new();
    for r in reserve_rows {
        let coin_str: String = r.get("coin");
        if let Ok(coin) = coin_str.parse::<Coin>() {
            let li: BigDecimal = r.get("liquidity_index");
            let bi: BigDecimal = r.get("borrow_index");
            idx.insert(coin, (li.to_string().parse().unwrap_or(RAY), bi.to_string().parse().unwrap_or(RAY)));
        }
    }
    let position_rows = sqlx::query("SELECT user_id as uid, coin::text as coin, scaled_supply, scaled_debt FROM lend_positions").fetch_all(&mut **tx).await?;
    for r in position_rows {
        let coin_str: String = r.get("coin");
        let Ok(coin) = coin_str.parse::<Coin>() else { continue };
        let (li, bi) = idx.get(&coin).copied().unwrap_or((RAY, RAY));
        let ss: BigDecimal = r.get("scaled_supply");
        let sd: BigDecimal = r.get("scaled_debt");
        let scaled_supply: u128 = ss.to_string().parse().unwrap_or(0);
        let scaled_debt: u128 = sd.to_string().parse().unwrap_or(0);
        let uid: Uuid = r.get("uid");
        add(uid, coin, BigInt::from(scaled_to_amount(scaled_supply, li)));
        add(uid, coin, BigInt::from(scaled_to_amount(scaled_debt, bi)));
    }

    Ok(weights)
}

/// Distributes one program's due emission for the elapsed window, crediting
/// every eligible user's PERSONAL wallet directly (no claim). Paid from
/// HOUSE reward inventory. Locks the program row so overlapping ticks never
/// double-pay. If the reward pool is empty, the clock is NOT advanced —
/// waits for funding instead of forfeiting the emission window.
async fn distribute_program(pool: &PgPool, program_id: Uuid, max_stale: Duration) -> Result<(), RewardsError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM reward_programs WHERE id = $1 FOR UPDATE").bind(program_id).fetch_optional(&mut *tx).await?;

    let row = sqlx::query(
        "SELECT reward_coin::text as reward_coin, active, emission_per_day, start_at, end_at, last_emitted_at FROM reward_programs WHERE id = $1",
    )
    .bind(program_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else { return Ok(()) };
    let active: bool = row.get("active");
    if !active {
        return Ok(());
    }
    let reward_coin: Coin = row.get::<String, _>("reward_coin").parse().map_err(|_| RewardsError::NotFound)?;
    let emission_per_day: BigDecimal = row.get("emission_per_day");
    let start_at: DateTime<Utc> = row.get("start_at");
    let end_at: Option<DateTime<Utc>> = row.get("end_at");
    let last_emitted_at: DateTime<Utc> = row.get("last_emitted_at");

    let now = Utc::now();
    if now < start_at {
        return Ok(());
    }
    let effective_now = match end_at {
        Some(end) if now > end => end,
        _ => now,
    };
    let from = last_emitted_at.max(start_at);
    let elapsed_ms = (effective_now - from).num_milliseconds();
    let finished = end_at.is_some_and(|end| now >= end);

    if elapsed_ms <= 0 {
        advance_program(&mut tx, program_id, now, finished).await?;
        tx.commit().await?;
        return Ok(());
    }

    const MS_PER_DAY: i64 = 24 * 60 * 60 * 1000;
    let emission_per_day_u128: u128 = emission_per_day.to_string().parse().unwrap_or(0);
    let mut emission = (BigInt::from(emission_per_day_u128) * BigInt::from(elapsed_ms)) / BigInt::from(MS_PER_DAY);
    if !emission.is_positive() {
        advance_program(&mut tx, program_id, now, finished).await?;
        tx.commit().await?;
        return Ok(());
    }

    // Cap emission to available HOUSE inventory of the reward coin.
    let house_wallet_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT w.id FROM wallets w JOIN users u ON u.id = w.user_id WHERE u.email = $1 AND w.coin = $2::coin AND w.kind = 'HOUSE'",
    )
    .bind(HOUSE_EMAIL)
    .bind(reward_coin.as_str())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(house_wallet_id) = house_wallet_id else { return Ok(()) };
    let house_balance = get_wallet_balance(&mut tx, house_wallet_id).await.map_err(|e| RewardsError::Db(sqlx::Error::Protocol(e.to_string())))?;
    let house_balance_int: BigInt = house_balance.to_string().parse().unwrap_or_else(|_| BigInt::from(0));
    if !house_balance_int.is_positive() {
        // Wait for funding; clock not advanced.
        tx.commit().await?;
        return Ok(());
    }
    if emission > house_balance_int {
        emission = house_balance_int;
    }

    let prices = load_price_table(&mut tx, max_stale).await?;
    let weights = compute_user_weights(&mut tx, &prices).await?;
    let total: BigInt = weights.values().cloned().sum();
    if !total.is_positive() {
        advance_program(&mut tx, program_id, now, finished).await?;
        tx.commit().await?;
        return Ok(());
    }

    let mut payouts: Vec<(Uuid, Uuid, u128)> = Vec::new();
    for (user_id, weight) in &weights {
        let wallet_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'").bind(user_id).bind(reward_coin.as_str()).fetch_optional(&mut *tx).await?;
        let Some(wallet_id) = wallet_id else { continue };
        let share = (&emission * weight) / &total;
        if share.is_positive() {
            payouts.push((*user_id, wallet_id, share.to_u128().unwrap_or(0)));
        }
    }
    let sum_shares: u128 = payouts.iter().map(|(_, _, s)| s).sum();
    if sum_shares == 0 {
        advance_program(&mut tx, program_id, now, finished).await?;
        tx.commit().await?;
        return Ok(());
    }

    let mut wallet_ids: Vec<Uuid> = payouts.iter().map(|(_, w, _)| *w).collect();
    wallet_ids.push(house_wallet_id);
    lock_wallets(&mut tx, &wallet_ids).await.map_err(|e| RewardsError::Db(sqlx::Error::Protocol(e.to_string())))?;

    debit_house(&mut tx, reward_coin, BigDecimal::from(sum_shares), "REWARD", program_id, "RewardProgram", &format!("Auto reward distribution {}", reward_coin.as_str())).await?;
    for (user_id, wallet_id, share) in &payouts {
        apply_ledger_entry(
            &mut tx,
            LedgerCreditInput { reference_key: None, wallet_id: *wallet_id, amount: BigDecimal::from(*share), ledger_type: "REWARD", reference_id: Some(program_id), reference_type: Some("RewardProgram"), memo: Some(&format!("Reward {}", reward_coin.as_str())) },
        )
        .await
        .map_err(|e| RewardsError::Db(sqlx::Error::Protocol(e.to_string())))?;

        sqlx::query(
            "INSERT INTO reward_accruals (user_id, program_id, claimed_total) VALUES ($1, $2, $3) \
             ON CONFLICT (user_id, program_id) DO UPDATE SET claimed_total = reward_accruals.claimed_total + $3, updated_at = now()",
        )
        .bind(user_id)
        .bind(program_id)
        .bind(BigDecimal::from(*share))
        .execute(&mut *tx)
        .await?;
    }

    advance_program(&mut tx, program_id, now, finished).await?;
    tx.commit().await?;
    tracing::info!(program_id = %program_id, coin = reward_coin.as_str(), distributed = sum_shares, recipients = payouts.len(), "reward distribution tick");
    Ok(())
}

/// Distributes every active program — called by the worker's economy tick.
pub async fn distribute_all_programs(pool: &PgPool, max_stale: Duration) -> Result<(), RewardsError> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM reward_programs WHERE active = true").fetch_all(pool).await?;
    for id in ids {
        if let Err(e) = distribute_program(pool, id, max_stale).await {
            tracing::error!(program_id = %id, error = %e, "reward distribution failed");
        }
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UserRewardView {
    pub program_id: Uuid,
    pub reward_coin: Coin,
    pub market_coin: Coin,
    pub side: String,
    pub received_total: u128,
}

/// A user's lifetime auto-reward totals per program (no claim — already credited).
pub async fn get_user_rewards(pool: &PgPool, user_id: Uuid) -> Result<Vec<UserRewardView>, RewardsError> {
    let rows = sqlx::query(
        "SELECT ra.program_id, ra.claimed_total, rp.reward_coin::text as reward_coin, rp.market_coin::text as market_coin, rp.side::text as side \
         FROM reward_accruals ra JOIN reward_programs rp ON rp.id = ra.program_id \
         WHERE ra.user_id = $1 AND ra.claimed_total > 0 ORDER BY ra.updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let reward_coin: String = r.get("reward_coin");
            let market_coin: String = r.get("market_coin");
            let claimed: BigDecimal = r.get("claimed_total");
            Some(UserRewardView { program_id: r.get("program_id"), reward_coin: reward_coin.parse().ok()?, market_coin: market_coin.parse().ok()?, side: r.get("side"), received_total: claimed.to_string().parse().unwrap_or(0) })
        })
        .collect())
}
