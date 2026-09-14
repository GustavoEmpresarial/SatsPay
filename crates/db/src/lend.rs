//! Port of legacy `apps/api/src/modules/lend/{services,repositories}/lend.*.ts`.
//!
//! Covers reserve interest accrual, supply/withdraw/borrow/repay with the
//! health-factor / borrow-power checks that keep the pool solvent. The
//! liquidation endpoint (letting a third party force-close an unhealthy
//! position) is **not** ported yet — see
//! `docs/decisions/` TODO(fase 8/hardening) — supply/withdraw/borrow/repay
//! already refuse operations that would push a position underwater, which
//! covers the everyday solvency path; liquidation is the recovery path for
//! positions that go underwater from price movement alone (no user action),
//! and needs its own dedicated pass given how risk-sensitive it is.

use crate::house::get_lend_pool_wallet_id;
use crate::ledger::{apply_ledger_entry, get_wallet_balance, lock_wallets, LedgerCreditInput};
use crate::pricing::{load_price_table, PriceTable, PricingDbError};
use bigdecimal::BigDecimal;
use chrono::Utc;
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use shared::{accrue_index, amount_to_scaled, calc_borrow_rate_bps, calc_supply_rate_bps, calc_utilization_bps, coin_config, lend_market, scaled_to_amount, Coin, COINS, RAY};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum LendError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    House(#[from] crate::house::HouseError),
    #[error(transparent)]
    Pricing(#[from] PricingDbError),
    #[error("amount must be > 0")]
    InvalidAmount,
    #[error("wallet not found")]
    WalletNotFound,
    #[error("borrowing {0:?} is disabled")]
    BorrowDisabled(Coin),
    #[error("not enough available pool liquidity")]
    InsufficientLiquidity,
    #[error("no supply position")]
    NoSupply,
    #[error("no outstanding debt")]
    NoDebt,
    #[error("withdrawal would put your position below a safe health factor")]
    UnsafeWithdraw,
    #[error("insufficient collateral for this borrow (exceeds your borrow limit)")]
    InsufficientCollateral,
    #[error("cannot liquidate your own position")]
    SelfLiquidation,
    #[error("borrower has no debt in this coin")]
    NoBorrowerDebt,
    #[error("borrower has no seizable collateral in this coin")]
    NoSeizableCollateral,
    #[error("position is healthy — liquidation not allowed")]
    PositionHealthy,
    #[error("liquidation amount too small")]
    LiquidationTooSmall,
}

#[derive(Clone, Copy)]
struct ReserveState {
    liquidity_index: u128,
    borrow_index: u128,
}

fn value_usd(prices: &PriceTable, coin: Coin, amount: u128) -> u128 {
    prices.value_usd(coin, amount, coin_config(coin).decimals)
}

/// Ensures a `lend_reserves` row exists for every coin, indexes seeded at RAY.
pub async fn ensure_lend_reserves(pool: &PgPool) -> Result<(), LendError> {
    for coin in COINS {
        sqlx::query("INSERT INTO lend_reserves (coin, liquidity_index, borrow_index) VALUES ($1::coin, $2, $2) ON CONFLICT DO NOTHING")
            .bind(coin.as_str())
            .bind(BigDecimal::from(RAY))
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Lazily accrues interest for a coin inside `tx`: locks the reserve row,
/// advances both indexes by elapsed time at current utilization, persists.
async fn accrue(tx: &mut Transaction<'_, Postgres>, coin: Coin) -> Result<ReserveState, LendError> {
    sqlx::query("SELECT coin FROM lend_reserves WHERE coin = $1::coin FOR UPDATE").bind(coin.as_str()).fetch_optional(&mut **tx).await?;

    let row = sqlx::query("SELECT liquidity_index, borrow_index, total_scaled_debt, last_accrued_at FROM lend_reserves WHERE coin = $1::coin")
        .bind(coin.as_str())
        .fetch_optional(&mut **tx)
        .await?;
    let (liquidity_index, borrow_index, total_scaled_debt, last_accrued_at) = match row {
        Some(r) => {
            let li: BigDecimal = r.get("liquidity_index");
            let bi: BigDecimal = r.get("borrow_index");
            let tsd: BigDecimal = r.get("total_scaled_debt");
            (li.to_string().parse().unwrap_or(RAY), bi.to_string().parse().unwrap_or(RAY), tsd.to_string().parse().unwrap_or(0u128), r.get::<chrono::DateTime<Utc>, _>("last_accrued_at"))
        }
        None => {
            sqlx::query("INSERT INTO lend_reserves (coin, liquidity_index, borrow_index) VALUES ($1::coin, $2, $2)")
                .bind(coin.as_str())
                .bind(BigDecimal::from(RAY))
                .execute(&mut **tx)
                .await?;
            (RAY, RAY, 0u128, Utc::now())
        }
    };

    let now = Utc::now();
    let elapsed_ms = (now - last_accrued_at).num_milliseconds();
    let market = lend_market(coin);

    let real_debt = scaled_to_amount(total_scaled_debt, borrow_index);
    let pool_id = get_lend_pool_wallet_id_tx(tx, coin).await?;
    let available_dec = get_wallet_balance(tx, pool_id).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;
    let available: u128 = available_dec.to_string().parse().unwrap_or(0);

    let util_bps = calc_utilization_bps(real_debt, available);
    let borrow_rate_bps = calc_borrow_rate_bps(util_bps, &market);
    let supply_rate_bps = calc_supply_rate_bps(borrow_rate_bps, util_bps, market.reserve_factor_bps);

    let new_borrow_index = accrue_index(borrow_index, borrow_rate_bps, elapsed_ms);
    let new_liquidity_index = accrue_index(liquidity_index, supply_rate_bps, elapsed_ms);

    sqlx::query("UPDATE lend_reserves SET liquidity_index = $2, borrow_index = $3, last_accrued_at = $4, updated_at = now() WHERE coin = $1::coin")
        .bind(coin.as_str())
        .bind(BigDecimal::from(new_liquidity_index))
        .bind(BigDecimal::from(new_borrow_index))
        .bind(now)
        .execute(&mut **tx)
        .await?;

    Ok(ReserveState { liquidity_index: new_liquidity_index, borrow_index: new_borrow_index })
}

async fn get_lend_pool_wallet_id_tx(tx: &mut Transaction<'_, Postgres>, coin: Coin) -> Result<Uuid, LendError> {
    Ok(get_lend_pool_wallet_id(tx, coin).await?)
}

async fn get_user_wallet_id(tx: &mut Transaction<'_, Postgres>, user_id: Uuid, coin: Coin) -> Result<Uuid, LendError> {
    sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
        .bind(user_id)
        .bind(coin.as_str())
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(LendError::WalletNotFound)
}

struct Position {
    id: Uuid,
    scaled_supply: u128,
    scaled_debt: u128,
    use_as_collateral: bool,
}

async fn get_or_create_position(tx: &mut Transaction<'_, Postgres>, user_id: Uuid, coin: Coin) -> Result<Position, LendError> {
    let row = sqlx::query("SELECT id, scaled_supply, scaled_debt, use_as_collateral FROM lend_positions WHERE user_id = $1 AND coin = $2::coin")
        .bind(user_id)
        .bind(coin.as_str())
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(r) = row {
        let ss: BigDecimal = r.get("scaled_supply");
        let sd: BigDecimal = r.get("scaled_debt");
        return Ok(Position { id: r.get("id"), scaled_supply: ss.to_string().parse().unwrap_or(0), scaled_debt: sd.to_string().parse().unwrap_or(0), use_as_collateral: r.get("use_as_collateral") });
    }
    let id: Uuid = sqlx::query_scalar("INSERT INTO lend_positions (user_id, coin) VALUES ($1, $2::coin) RETURNING id").bind(user_id).bind(coin.as_str()).fetch_one(&mut **tx).await?;
    Ok(Position { id, scaled_supply: 0, scaled_debt: 0, use_as_collateral: true })
}

/// Aggregate account liquidity across every coin position, using the given
/// coins' freshly-accrued indexes (`fresh` — usually one coin for
/// borrow/withdraw, two for liquidate's debt+collateral pair) and stored (at
/// most one tick stale) indexes for the rest — safe for a
/// borrow-power/health-factor check.
async fn compute_account_liquidity(tx: &mut Transaction<'_, Postgres>, user_id: Uuid, fresh: &[(Coin, &ReserveState)], prices: &PriceTable) -> Result<(u128, u128, u128), LendError> {
    let rows = sqlx::query("SELECT coin::text as coin, scaled_supply, scaled_debt, use_as_collateral FROM lend_positions WHERE user_id = $1").bind(user_id).fetch_all(&mut **tx).await?;

    let mut liquidation_collateral_usd: u128 = 0;
    let mut borrow_power_usd: u128 = 0;
    let mut debt_usd: u128 = 0;

    for row in rows {
        let coin_str: String = row.get("coin");
        let Ok(coin) = coin_str.parse::<Coin>() else { continue };
        let ss: BigDecimal = row.get("scaled_supply");
        let sd: BigDecimal = row.get("scaled_debt");
        let scaled_supply: u128 = ss.to_string().parse().unwrap_or(0);
        let scaled_debt: u128 = sd.to_string().parse().unwrap_or(0);
        let use_as_collateral: bool = row.get("use_as_collateral");
        let market = lend_market(coin);

        let (liquidity_index, borrow_index) = if let Some((_, r)) = fresh.iter().find(|(c, _)| *c == coin) {
            (r.liquidity_index, r.borrow_index)
        } else {
            let r = sqlx::query("SELECT liquidity_index, borrow_index FROM lend_reserves WHERE coin = $1::coin").bind(coin.as_str()).fetch_optional(&mut **tx).await?;
            match r {
                Some(r) => {
                    let li: BigDecimal = r.get("liquidity_index");
                    let bi: BigDecimal = r.get("borrow_index");
                    (li.to_string().parse().unwrap_or(RAY), bi.to_string().parse().unwrap_or(RAY))
                }
                None => (RAY, RAY),
            }
        };

        if scaled_supply > 0 && use_as_collateral && market.can_be_collateral {
            let supply_real = scaled_to_amount(scaled_supply, liquidity_index);
            let usd = value_usd(prices, coin, supply_real);
            liquidation_collateral_usd += (usd * market.liquidation_threshold_bps as u128) / 10_000;
            borrow_power_usd += (usd * market.collateral_factor_bps as u128) / 10_000;
        }
        if scaled_debt > 0 {
            let debt_real = scaled_to_amount(scaled_debt, borrow_index);
            debt_usd += value_usd(prices, coin, debt_real);
        }
    }

    Ok((liquidation_collateral_usd, borrow_power_usd, debt_usd))
}

pub async fn supply(pool: &PgPool, user_id: Uuid, coin: Coin, amount: u128) -> Result<u128, LendError> {
    if amount == 0 {
        return Err(LendError::InvalidAmount);
    }
    let mut tx = pool.begin().await?;
    let idx = accrue(&mut tx, coin).await?;
    let wallet_id = get_user_wallet_id(&mut tx, user_id, coin).await?;
    let pool_id = get_lend_pool_wallet_id_tx(&mut tx, coin).await?;
    lock_wallets(&mut tx, &[wallet_id, pool_id]).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;

    let ref_id = format!("{user_id}:{}", coin.as_str());
    ledger_move(&mut tx, wallet_id, pool_id, amount, "LEND_SUPPLY", &ref_id, &format!("Supply {}", coin.as_str()), &format!("Supply intake {}", coin.as_str())).await?;

    let scaled = amount_to_scaled(amount, idx.liquidity_index);
    let position = get_or_create_position(&mut tx, user_id, coin).await?;
    sqlx::query("UPDATE lend_positions SET scaled_supply = scaled_supply + $2, updated_at = now() WHERE id = $1").bind(position.id).bind(BigDecimal::from(scaled)).execute(&mut *tx).await?;
    sqlx::query("UPDATE lend_reserves SET total_scaled_supply = total_scaled_supply + $2, updated_at = now() WHERE coin = $1::coin").bind(coin.as_str()).bind(BigDecimal::from(scaled)).execute(&mut *tx).await?;

    tx.commit().await?;
    Ok(amount)
}

pub async fn withdraw(pool: &PgPool, user_id: Uuid, coin: Coin, amount: u128, max_stale: Duration) -> Result<u128, LendError> {
    if amount == 0 {
        return Err(LendError::InvalidAmount);
    }
    let mut tx = pool.begin().await?;
    let idx = accrue(&mut tx, coin).await?;
    let prices = load_price_table(&mut tx, max_stale).await?;
    let wallet_id = get_user_wallet_id(&mut tx, user_id, coin).await?;
    let pool_id = get_lend_pool_wallet_id_tx(&mut tx, coin).await?;
    lock_wallets(&mut tx, &[wallet_id, pool_id]).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;

    let position = get_or_create_position(&mut tx, user_id, coin).await?;
    if position.scaled_supply == 0 {
        return Err(LendError::NoSupply);
    }
    let supply_real = scaled_to_amount(position.scaled_supply, idx.liquidity_index);
    let out = amount.min(supply_real);

    let available_dec = get_wallet_balance(&mut tx, pool_id).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;
    let available: u128 = available_dec.to_string().parse().unwrap_or(0);
    if out > available {
        return Err(LendError::InsufficientLiquidity);
    }

    let (liq_collateral, _borrow_power, debt_usd) = compute_account_liquidity(&mut tx, user_id, &[(coin, &idx)], &prices).await?;
    let market = lend_market(coin);
    if position.use_as_collateral && market.can_be_collateral && debt_usd > 0 {
        let removed_usd = value_usd(&prices, coin, out);
        let removed_liq = (removed_usd * market.liquidation_threshold_bps as u128) / 10_000;
        let new_liq_collateral = liq_collateral.saturating_sub(removed_liq);
        if new_liq_collateral < debt_usd {
            return Err(LendError::UnsafeWithdraw);
        }
    }

    let ref_id = format!("{user_id}:{}", coin.as_str());
    ledger_move(&mut tx, pool_id, wallet_id, out, "LEND_WITHDRAW", &ref_id, &format!("Withdraw payout {}", coin.as_str()), &format!("Withdraw {} (incl. interest)", coin.as_str())).await?;

    let scaled_out = amount_to_scaled(out, idx.liquidity_index);
    let new_scaled = position.scaled_supply.saturating_sub(scaled_out);
    sqlx::query("UPDATE lend_positions SET scaled_supply = $2, updated_at = now() WHERE id = $1").bind(position.id).bind(BigDecimal::from(new_scaled)).execute(&mut *tx).await?;
    sqlx::query("UPDATE lend_reserves SET total_scaled_supply = GREATEST(total_scaled_supply - $2, 0), updated_at = now() WHERE coin = $1::coin")
        .bind(coin.as_str())
        .bind(BigDecimal::from(scaled_out))
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(out)
}

pub async fn borrow(pool: &PgPool, user_id: Uuid, coin: Coin, amount: u128, max_stale: Duration) -> Result<u128, LendError> {
    if amount == 0 {
        return Err(LendError::InvalidAmount);
    }
    let market = lend_market(coin);
    if !market.borrow_enabled {
        return Err(LendError::BorrowDisabled(coin));
    }

    let mut tx = pool.begin().await?;
    let idx = accrue(&mut tx, coin).await?;
    let prices = load_price_table(&mut tx, max_stale).await?;
    let wallet_id = get_user_wallet_id(&mut tx, user_id, coin).await?;
    let pool_id = get_lend_pool_wallet_id_tx(&mut tx, coin).await?;
    lock_wallets(&mut tx, &[wallet_id, pool_id]).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;

    let available_dec = get_wallet_balance(&mut tx, pool_id).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;
    let available: u128 = available_dec.to_string().parse().unwrap_or(0);
    if amount > available {
        return Err(LendError::InsufficientLiquidity);
    }

    let (_liq_collateral, borrow_power, debt_usd) = compute_account_liquidity(&mut tx, user_id, &[(coin, &idx)], &prices).await?;
    let new_debt_usd = debt_usd + value_usd(&prices, coin, amount);
    if new_debt_usd > borrow_power {
        return Err(LendError::InsufficientCollateral);
    }

    let ref_id = format!("{user_id}:{}", coin.as_str());
    ledger_move(&mut tx, pool_id, wallet_id, amount, "LEND_BORROW", &ref_id, &format!("Borrow disbursement {}", coin.as_str()), &format!("Borrow {}", coin.as_str())).await?;

    let scaled = amount_to_scaled(amount, idx.borrow_index);
    let position = get_or_create_position(&mut tx, user_id, coin).await?;
    sqlx::query("UPDATE lend_positions SET scaled_debt = scaled_debt + $2, updated_at = now() WHERE id = $1").bind(position.id).bind(BigDecimal::from(scaled)).execute(&mut *tx).await?;
    sqlx::query("UPDATE lend_reserves SET total_scaled_debt = total_scaled_debt + $2, updated_at = now() WHERE coin = $1::coin").bind(coin.as_str()).bind(BigDecimal::from(scaled)).execute(&mut *tx).await?;

    tx.commit().await?;
    Ok(amount)
}

pub async fn repay(pool: &PgPool, user_id: Uuid, coin: Coin, amount: u128) -> Result<u128, LendError> {
    if amount == 0 {
        return Err(LendError::InvalidAmount);
    }
    let mut tx = pool.begin().await?;
    let idx = accrue(&mut tx, coin).await?;
    let wallet_id = get_user_wallet_id(&mut tx, user_id, coin).await?;
    let pool_id = get_lend_pool_wallet_id_tx(&mut tx, coin).await?;
    lock_wallets(&mut tx, &[wallet_id, pool_id]).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;

    let position = get_or_create_position(&mut tx, user_id, coin).await?;
    if position.scaled_debt == 0 {
        return Err(LendError::NoDebt);
    }
    let debt_real = scaled_to_amount(position.scaled_debt, idx.borrow_index);
    let pay = amount.min(debt_real);

    let ref_id = format!("{user_id}:{}", coin.as_str());
    ledger_move(&mut tx, wallet_id, pool_id, pay, "LEND_REPAY", &ref_id, &format!("Repay {} (incl. interest)", coin.as_str()), &format!("Repay intake {}", coin.as_str())).await?;

    let scaled_paid = amount_to_scaled(pay, idx.borrow_index);
    let new_scaled = position.scaled_debt.saturating_sub(scaled_paid);
    sqlx::query("UPDATE lend_positions SET scaled_debt = $2, updated_at = now() WHERE id = $1").bind(position.id).bind(BigDecimal::from(new_scaled)).execute(&mut *tx).await?;
    sqlx::query("UPDATE lend_reserves SET total_scaled_debt = GREATEST(total_scaled_debt - $2, 0), updated_at = now() WHERE coin = $1::coin")
        .bind(coin.as_str())
        .bind(BigDecimal::from(scaled_paid))
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(pay)
}

#[allow(clippy::too_many_arguments)]
async fn ledger_move(
    tx: &mut Transaction<'_, Postgres>,
    from_wallet: Uuid,
    to_wallet: Uuid,
    amount: u128,
    ledger_type: &str,
    reference_id_text: &str,
    memo_out: &str,
    memo_in: &str,
) -> Result<(), LendError> {
    // LendPosition reference ids are `"{user_id}:{coin}"` text, not a single
    // row's UUID (a lend position isn't its own row at the time of the first
    // operation) — carried in `reference_key`, matching legacy's free-text
    // `referenceId` semantics without abusing the uuid-typed `reference_id`.
    apply_ledger_entry(
        tx,
        LedgerCreditInput { reference_key: Some(reference_id_text), wallet_id: from_wallet, amount: -BigDecimal::from(amount), ledger_type, reference_id: None, reference_type: Some("LendPosition"), memo: Some(memo_out) },
    )
    .await
    .map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;
    apply_ledger_entry(
        tx,
        LedgerCreditInput { reference_key: Some(reference_id_text), wallet_id: to_wallet, amount: BigDecimal::from(amount), ledger_type, reference_id: None, reference_type: Some("LendPosition"), memo: Some(memo_in) },
    )
    .await
    .map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;
    Ok(())
}

/// Converts a USD value (scaled to the price table's decimals, same scale as
/// `value_usd`) back into a coin's smallest units.
fn usd_to_amount(prices: &PriceTable, coin: Coin, usd_scaled: u128) -> u128 {
    let price = prices.price_scaled(coin);
    if price == 0 {
        return 0;
    }
    let decimals = coin_config(coin).decimals;
    let result = (BigUint::from(usd_scaled) * BigUint::from(10u128.pow(decimals))) / BigUint::from(price);
    result.to_u128().unwrap_or(u128::MAX)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PositionView {
    pub coin: Coin,
    pub supplied: u128,
    pub debt: u128,
    pub use_as_collateral: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountLiquidityView {
    pub positions: Vec<PositionView>,
    pub debt_usd: u128,
    pub borrow_power_usd: u128,
    pub available_borrow_usd: u128,
    pub liquidation_collateral_usd: u128,
    /// Health factor scaled by 1e4 (10_000 == HF of 1.0); `None` when the
    /// account has no debt (HF is conceptually infinite).
    pub health_factor_bps: Option<u128>,
}

/// A user's positions plus aggregate health factor and remaining borrow
/// power. Read-only — does not accrue (uses stored, at-most-one-tick-stale
/// indexes), same as legacy's `getUserPositions`.
pub async fn get_user_positions(pool: &PgPool, user_id: Uuid, max_stale: Duration) -> Result<AccountLiquidityView, LendError> {
    let mut tx = pool.begin().await?;
    let prices = load_price_table(&mut tx, max_stale).await?;

    let reserve_rows = sqlx::query("SELECT coin::text as coin, liquidity_index, borrow_index FROM lend_reserves").fetch_all(&mut *tx).await?;
    let mut reserves: std::collections::HashMap<Coin, ReserveState> = std::collections::HashMap::new();
    for r in reserve_rows {
        let coin_str: String = r.get("coin");
        if let Ok(coin) = coin_str.parse::<Coin>() {
            let li: BigDecimal = r.get("liquidity_index");
            let bi: BigDecimal = r.get("borrow_index");
            reserves.insert(coin, ReserveState { liquidity_index: li.to_string().parse().unwrap_or(RAY), borrow_index: bi.to_string().parse().unwrap_or(RAY) });
        }
    }

    let position_rows = sqlx::query("SELECT coin::text as coin, scaled_supply, scaled_debt, use_as_collateral FROM lend_positions WHERE user_id = $1").bind(user_id).fetch_all(&mut *tx).await?;

    let mut positions = Vec::new();
    let mut liquidation_collateral_usd: u128 = 0;
    let mut borrow_power_usd: u128 = 0;
    let mut debt_usd: u128 = 0;

    for row in position_rows {
        let coin_str: String = row.get("coin");
        let Ok(coin) = coin_str.parse::<Coin>() else { continue };
        let ss: BigDecimal = row.get("scaled_supply");
        let sd: BigDecimal = row.get("scaled_debt");
        let scaled_supply: u128 = ss.to_string().parse().unwrap_or(0);
        let scaled_debt: u128 = sd.to_string().parse().unwrap_or(0);
        let use_as_collateral: bool = row.get("use_as_collateral");
        let idx = reserves.get(&coin).copied().unwrap_or(ReserveState { liquidity_index: RAY, borrow_index: RAY });
        let market = lend_market(coin);

        let supply_real = scaled_to_amount(scaled_supply, idx.liquidity_index);
        let debt_real = scaled_to_amount(scaled_debt, idx.borrow_index);

        if scaled_supply > 0 && use_as_collateral && market.can_be_collateral {
            let usd = value_usd(&prices, coin, supply_real);
            liquidation_collateral_usd += (usd * market.liquidation_threshold_bps as u128) / 10_000;
            borrow_power_usd += (usd * market.collateral_factor_bps as u128) / 10_000;
        }
        if scaled_debt > 0 {
            debt_usd += value_usd(&prices, coin, debt_real);
        }
        if scaled_supply > 0 || scaled_debt > 0 {
            positions.push(PositionView { coin, supplied: supply_real, debt: debt_real, use_as_collateral });
        }
    }

    tx.commit().await?;

    let health_factor_bps = (liquidation_collateral_usd * 10_000).checked_div(debt_usd);
    let available_borrow_usd = borrow_power_usd.saturating_sub(debt_usd);

    Ok(AccountLiquidityView { positions, debt_usd, borrow_power_usd, available_borrow_usd, liquidation_collateral_usd, health_factor_bps })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MarketView {
    pub coin: Coin,
    pub total_supply: u128,
    pub total_debt: u128,
    pub available: u128,
    pub utilization_bps: u32,
    pub supply_apy_bps: u32,
    pub borrow_apy_bps: u32,
    pub collateral_factor_bps: u32,
    pub liquidation_threshold_bps: u32,
    pub borrow_enabled: bool,
    pub can_be_collateral: bool,
}

/// Public market stats per coin (liquidity, utilization, APYs) — port of
/// legacy `getMarkets()`.
pub async fn get_markets(pool: &PgPool) -> Result<Vec<MarketView>, LendError> {
    let pool_balances = crate::house::list_lend_pool_balances(pool).await?;
    let mut available_by_coin = std::collections::HashMap::<Coin, u128>::new();
    for (coin, bal) in pool_balances {
        available_by_coin.insert(coin, bal.to_string().parse().unwrap_or(0));
    }

    let rows = sqlx::query(
        "SELECT coin::text as coin, liquidity_index, borrow_index, total_scaled_supply, total_scaled_debt FROM lend_reserves",
    )
    .fetch_all(pool)
    .await?;

    let mut by_coin = std::collections::HashMap::<Coin, (u128, u128, u128, u128)>::new();
    for r in rows {
        let coin_str: String = r.get("coin");
        let Ok(coin) = coin_str.parse::<Coin>() else { continue };
        let li: BigDecimal = r.get("liquidity_index");
        let bi: BigDecimal = r.get("borrow_index");
        let tss: BigDecimal = r.get("total_scaled_supply");
        let tsd: BigDecimal = r.get("total_scaled_debt");
        by_coin.insert(
            coin,
            (
                li.to_string().parse().unwrap_or(RAY),
                bi.to_string().parse().unwrap_or(RAY),
                tss.to_string().parse().unwrap_or(0),
                tsd.to_string().parse().unwrap_or(0),
            ),
        );
    }

    let aave_rates = crate::aave_sync::get_aave_rates(pool).await.unwrap_or_default();

    let mut out = Vec::with_capacity(COINS.len());
    for coin in COINS {
        let (li, bi, tss, tsd) = by_coin.get(&coin).copied().unwrap_or((RAY, RAY, 0, 0));
        let total_supply = scaled_to_amount(tss, li);
        let total_debt = scaled_to_amount(tsd, bi);
        let available = *available_by_coin.get(&coin).unwrap_or(&0);
        let market = lend_market(coin);
        let utilization_bps = calc_utilization_bps(total_debt, available);
        
        let (supply_apy_bps, borrow_apy_bps, ltv_bps, liq_thresh_bps) = if let Some(ar) = aave_rates.get(&coin) {
            (
                ar.supply_apy_bps.max(calc_supply_rate_bps(calc_borrow_rate_bps(utilization_bps, &market), utilization_bps, market.reserve_factor_bps)),
                ar.borrow_apy_bps.max(calc_borrow_rate_bps(utilization_bps, &market)),
                ar.collateral_factor_bps,
                ar.liquidation_threshold_bps,
            )
        } else {
            let borrow_apy = calc_borrow_rate_bps(utilization_bps, &market);
            let supply_apy = calc_supply_rate_bps(borrow_apy, utilization_bps, market.reserve_factor_bps);
            (supply_apy, borrow_apy, market.collateral_factor_bps, market.liquidation_threshold_bps)
        };

        out.push(MarketView {
            coin,
            total_supply,
            total_debt,
            available,
            utilization_bps,
            supply_apy_bps,
            borrow_apy_bps,
            collateral_factor_bps: ltv_bps,
            liquidation_threshold_bps: liq_thresh_bps,
            borrow_enabled: market.borrow_enabled,
            can_be_collateral: market.can_be_collateral,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LiquidatablePosition {
    pub user_id: Uuid,
    pub email: String,
    pub health_factor_bps: Option<u128>,
    pub debt_usd: u128,
}

/// Admin/keeper read: every borrower whose health factor is below 1.0
/// (10_000 bps) — the liquidation candidates.
pub async fn list_liquidatable_positions(pool: &PgPool, max_stale: Duration) -> Result<Vec<LiquidatablePosition>, LendError> {
    let debtor_ids: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT user_id FROM lend_positions WHERE scaled_debt > 0").fetch_all(pool).await?;
    let mut out = Vec::new();
    for user_id in debtor_ids {
        let view = get_user_positions(pool, user_id, max_stale).await?;
        if let Some(hf) = view.health_factor_bps {
            if hf < 10_000 {
                let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = $1").bind(user_id).fetch_optional(pool).await?;
                out.push(LiquidatablePosition { user_id, email: email.unwrap_or_else(|| "?".to_string()), health_factor_bps: Some(hf), debt_usd: view.debt_usd });
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LiquidationResult {
    pub borrower_id: Uuid,
    pub debt_coin: Coin,
    pub collateral_coin: Coin,
    pub repaid: u128,
    pub seized: u128,
}

/// Liquidates an unhealthy borrower (HF < 1): the liquidator repays up to
/// `close_factor_bps` of the borrower's `debt_coin` debt and seizes the
/// borrower's `collateral_coin` collateral at that market's bonus. Only
/// allowed while HF < 1. `close_factor_bps` is caller-supplied (not
/// hardcoded here) — legacy fixed it at 5000 (50%), kept configurable so an
/// operator can tune it without a code change.
#[allow(clippy::too_many_arguments)]
pub async fn liquidate(pool: &PgPool, liquidator_id: Uuid, borrower_id: Uuid, debt_coin: Coin, collateral_coin: Coin, amount: u128, close_factor_bps: u32, max_stale: Duration) -> Result<LiquidationResult, LendError> {
    if amount == 0 {
        return Err(LendError::InvalidAmount);
    }
    if liquidator_id == borrower_id {
        return Err(LendError::SelfLiquidation);
    }

    let mut tx = pool.begin().await?;
    let prices = load_price_table(&mut tx, max_stale).await?;
    let idx_debt = accrue(&mut tx, debt_coin).await?;
    let idx_coll = if collateral_coin == debt_coin { ReserveState { liquidity_index: idx_debt.liquidity_index, borrow_index: idx_debt.borrow_index } } else { accrue(&mut tx, collateral_coin).await? };

    let debt_position = get_or_create_position(&mut tx, borrower_id, debt_coin).await?;
    if debt_position.scaled_debt == 0 {
        return Err(LendError::NoBorrowerDebt);
    }
    let coll_position = get_or_create_position(&mut tx, borrower_id, collateral_coin).await?;
    let coll_market = lend_market(collateral_coin);
    if coll_position.scaled_supply == 0 || !coll_position.use_as_collateral || !coll_market.can_be_collateral {
        return Err(LendError::NoSeizableCollateral);
    }

    // Health check: only liquidatable while liquidation-collateral < debt.
    let fresh: Vec<(Coin, &ReserveState)> = if collateral_coin == debt_coin { vec![(debt_coin, &idx_debt)] } else { vec![(debt_coin, &idx_debt), (collateral_coin, &idx_coll)] };
    let (liq_collateral, _borrow_power, debt_usd) = compute_account_liquidity(&mut tx, borrower_id, &fresh, &prices).await?;
    if !(debt_usd > 0 && liq_collateral < debt_usd) {
        return Err(LendError::PositionHealthy);
    }

    // Repay is capped at the close factor of outstanding debt.
    let debt_real = scaled_to_amount(debt_position.scaled_debt, idx_debt.borrow_index);
    let max_close = (debt_real * close_factor_bps as u128) / 10_000;
    let mut repay = amount.min(max_close);

    // Collateral seized = repay value × (1 + bonus), clamped to available collateral.
    let repay_usd = value_usd(&prices, debt_coin, repay);
    let seize_usd = (repay_usd * (10_000 + coll_market.liquidation_bonus_bps as u128)) / 10_000;
    let coll_real = scaled_to_amount(coll_position.scaled_supply, idx_coll.liquidity_index);
    let mut seize = usd_to_amount(&prices, collateral_coin, seize_usd);
    if seize > coll_real {
        // Not enough collateral for the full bonus — seize all of it and
        // scale the repay down proportionally so the liquidator never overpays.
        seize = coll_real;
        let capped_repay_usd = (value_usd(&prices, collateral_coin, seize) * 10_000) / (10_000 + coll_market.liquidation_bonus_bps as u128);
        repay = usd_to_amount(&prices, debt_coin, capped_repay_usd);
    }
    if repay == 0 || seize == 0 {
        return Err(LendError::LiquidationTooSmall);
    }

    let liq_debt_wallet = get_user_wallet_id(&mut tx, liquidator_id, debt_coin).await?;
    let liq_coll_wallet = get_user_wallet_id(&mut tx, liquidator_id, collateral_coin).await?;
    let pool_debt = get_lend_pool_wallet_id_tx(&mut tx, debt_coin).await?;
    let pool_coll = get_lend_pool_wallet_id_tx(&mut tx, collateral_coin).await?;
    lock_wallets(&mut tx, &[liq_debt_wallet, liq_coll_wallet, pool_debt, pool_coll]).await.map_err(|e| LendError::Db(sqlx::Error::Protocol(e.to_string())))?;

    let debt_ref = format!("{borrower_id}:{}", debt_coin.as_str());
    let coll_ref = format!("{borrower_id}:{}", collateral_coin.as_str());

    // Liquidator repays borrower's debt into the pool.
    ledger_move(
        &mut tx,
        liq_debt_wallet,
        pool_debt,
        repay,
        "LIQUIDATION",
        &debt_ref,
        &format!("Liquidation repay {} for {}…", debt_coin.as_str(), &borrower_id.to_string()[..8]),
        &format!("Liquidation repay intake {}", debt_coin.as_str()),
    )
    .await?;
    // Pool releases seized collateral to the liquidator.
    ledger_move(
        &mut tx,
        pool_coll,
        liq_coll_wallet,
        seize,
        "LIQUIDATION",
        &coll_ref,
        &format!("Liquidation collateral release {}", collateral_coin.as_str()),
        &format!("Liquidation collateral seized {} (incl. bonus)", collateral_coin.as_str()),
    )
    .await?;

    // Reduce borrower's debt and collateral (scaled units).
    let scaled_repaid = amount_to_scaled(repay, idx_debt.borrow_index);
    let new_debt = debt_position.scaled_debt.saturating_sub(scaled_repaid);
    sqlx::query("UPDATE lend_positions SET scaled_debt = $2, updated_at = now() WHERE id = $1").bind(debt_position.id).bind(BigDecimal::from(new_debt)).execute(&mut *tx).await?;
    sqlx::query("UPDATE lend_reserves SET total_scaled_debt = GREATEST(total_scaled_debt - $2, 0), updated_at = now() WHERE coin = $1::coin").bind(debt_coin.as_str()).bind(BigDecimal::from(scaled_repaid)).execute(&mut *tx).await?;

    let scaled_seized = amount_to_scaled(seize, idx_coll.liquidity_index);
    let new_coll = coll_position.scaled_supply.saturating_sub(scaled_seized);
    sqlx::query("UPDATE lend_positions SET scaled_supply = $2, updated_at = now() WHERE id = $1").bind(coll_position.id).bind(BigDecimal::from(new_coll)).execute(&mut *tx).await?;
    sqlx::query("UPDATE lend_reserves SET total_scaled_supply = GREATEST(total_scaled_supply - $2, 0), updated_at = now() WHERE coin = $1::coin")
        .bind(collateral_coin.as_str())
        .bind(BigDecimal::from(scaled_seized))
        .execute(&mut *tx)
        .await?;

    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id, metadata) VALUES ($1, 'LEND_LIQUIDATE', 'LendPosition', $2, $3)")
        .bind(liquidator_id)
        .bind(borrower_id)
        .bind(serde_json::json!({ "debtCoin": debt_coin.as_str(), "collateralCoin": collateral_coin.as_str(), "repaid": repay.to_string(), "seized": seize.to_string() }))
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(LiquidationResult { borrower_id, debt_coin, collateral_coin, repaid: repay, seized: seize })
}
