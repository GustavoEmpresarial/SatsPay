//! Port 1:1 of the BitcoSats Lend (Aave-like money-market) section of legacy `coins.ts`.

use crate::coins::Coin;
use num_bigint::BigUint;
use num_traits::ToPrimitive;

/// Fixed-point scale for interest indexes (a "ray"-like unit). 1.0 == RAY.
pub const RAY: u128 = 1_000_000_000_000_000_000; // 10^18
const MS_PER_YEAR: u128 = 365 * 24 * 60 * 60 * 1000;
const BPS: u32 = 10_000;

#[derive(Debug, Clone, Copy)]
pub struct LendMarket {
    /// Max borrowing power this asset grants as collateral (LTV), in bps.
    pub collateral_factor_bps: u32,
    /// Collateral value ratio below which the position is liquidatable, in bps.
    pub liquidation_threshold_bps: u32,
    /// Bonus a liquidator receives on seized collateral, in bps.
    pub liquidation_bonus_bps: u32,
    /// Protocol cut of borrower interest routed to HOUSE reserves, in bps.
    pub reserve_factor_bps: u32,
    /// Kinked interest-rate model (all annualized, in bps).
    pub base_rate_bps: u32,
    pub slope1_bps: u32,
    pub slope2_bps: u32,
    /// Utilization kink (bps of 100%) where slope2 takes over.
    pub optimal_utilization_bps: u32,
    pub borrow_enabled: bool,
    pub can_be_collateral: bool,
}

/// Per-coin market parameters. Conservative, tunable defaults: BTC is the
/// strongest collateral; volatile/low-cap assets get lower LTVs and higher
/// slopes. All rates are illustrative and safe to adjust later.
pub fn lend_market(coin: Coin) -> LendMarket {
    match coin {
        Coin::Btc => LendMarket {
            collateral_factor_bps: 7500,
            liquidation_threshold_bps: 8000,
            liquidation_bonus_bps: 500,
            reserve_factor_bps: 1000,
            base_rate_bps: 0,
            slope1_bps: 400,
            slope2_bps: 30000,
            optimal_utilization_bps: 8000,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Ltc => LendMarket {
            collateral_factor_bps: 7000,
            liquidation_threshold_bps: 7500,
            liquidation_bonus_bps: 750,
            reserve_factor_bps: 1500,
            base_rate_bps: 0,
            slope1_bps: 500,
            slope2_bps: 30000,
            optimal_utilization_bps: 8000,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Bch => LendMarket {
            collateral_factor_bps: 7000,
            liquidation_threshold_bps: 7500,
            liquidation_bonus_bps: 750,
            reserve_factor_bps: 1500,
            base_rate_bps: 0,
            slope1_bps: 500,
            slope2_bps: 30000,
            optimal_utilization_bps: 8000,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Doge => LendMarket {
            collateral_factor_bps: 5000,
            liquidation_threshold_bps: 6000,
            liquidation_bonus_bps: 1000,
            reserve_factor_bps: 2000,
            base_rate_bps: 0,
            slope1_bps: 700,
            slope2_bps: 40000,
            optimal_utilization_bps: 7000,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Pol => LendMarket {
            collateral_factor_bps: 5000,
            liquidation_threshold_bps: 6000,
            liquidation_bonus_bps: 1000,
            reserve_factor_bps: 2000,
            base_rate_bps: 0,
            slope1_bps: 700,
            slope2_bps: 40000,
            optimal_utilization_bps: 7000,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Dgb => LendMarket {
            collateral_factor_bps: 4500,
            liquidation_threshold_bps: 5500,
            liquidation_bonus_bps: 1000,
            reserve_factor_bps: 2000,
            base_rate_bps: 0,
            slope1_bps: 800,
            slope2_bps: 40000,
            optimal_utilization_bps: 6500,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Sol => LendMarket {
            collateral_factor_bps: 6500,
            liquidation_threshold_bps: 7000,
            liquidation_bonus_bps: 750,
            reserve_factor_bps: 1500,
            base_rate_bps: 0,
            slope1_bps: 500,
            slope2_bps: 30000,
            optimal_utilization_bps: 7500,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Zer => LendMarket {
            collateral_factor_bps: 4000,
            liquidation_threshold_bps: 5000,
            liquidation_bonus_bps: 1000,
            reserve_factor_bps: 2000,
            base_rate_bps: 0,
            slope1_bps: 800,
            slope2_bps: 40000,
            optimal_utilization_bps: 6500,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Pepe => LendMarket {
            collateral_factor_bps: 4000,
            liquidation_threshold_bps: 5000,
            liquidation_bonus_bps: 1000,
            reserve_factor_bps: 2000,
            base_rate_bps: 0,
            slope1_bps: 800,
            slope2_bps: 40000,
            optimal_utilization_bps: 6500,
            borrow_enabled: true,
            can_be_collateral: true,
        },
        Coin::Usdt | Coin::Usdc => LendMarket {
            collateral_factor_bps: 8000,
            liquidation_threshold_bps: 8500,
            liquidation_bonus_bps: 500,
            reserve_factor_bps: 1000,
            base_rate_bps: 0,
            slope1_bps: 400,
            slope2_bps: 20000,
            optimal_utilization_bps: 9000,
            borrow_enabled: true,
            can_be_collateral: true,
        },
    }
}

/// Utilization = totalDebt / (available + totalDebt), returned in bps (0..10000).
pub fn calc_utilization_bps(total_debt: u128, available: u128) -> u32 {
    let denom = total_debt.saturating_add(available);
    if denom == 0 {
        return 0;
    }
    ((total_debt * BPS as u128) / denom) as u32
}

/// Kinked borrow rate model (annualized, in bps).
///   U <= optimal:  base + slope1 * U/optimal
///   U >  optimal:  base + slope1 + slope2 * (U-optimal)/(1-optimal)
pub fn calc_borrow_rate_bps(utilization_bps: u32, m: &LendMarket) -> u32 {
    let u = utilization_bps.clamp(0, 10_000);
    let opt = m.optimal_utilization_bps;
    if u <= opt {
        let ratio = if opt == 0 { 0.0 } else { u as f64 / opt as f64 };
        (m.base_rate_bps as f64 + m.slope1_bps as f64 * ratio).round() as u32
    } else {
        let excess = (u - opt) as f64 / (10_000 - opt) as f64;
        (m.base_rate_bps as f64 + m.slope1_bps as f64 + m.slope2_bps as f64 * excess).round() as u32
    }
}

/// Supply rate = borrowRate * utilization * (1 - reserveFactor), annualized bps.
pub fn calc_supply_rate_bps(borrow_rate_bps: u32, utilization_bps: u32, reserve_factor_bps: u32) -> u32 {
    let gross_bps = (borrow_rate_bps as u64 * utilization_bps as u64) / 10_000;
    ((gross_bps * (10_000 - reserve_factor_bps as u64)) / 10_000) as u32
}

/// Grow an interest index by simple interest over an elapsed window.
///   index' = index * (1 + rate * dt/year)
/// Simple (not compound) per tick; compounding emerges across ticks.
/// Uses `BigUint` internally to match the arbitrary-precision semantics of
/// the legacy `bigint` implementation and avoid overflow on large indexes.
pub fn accrue_index(index: u128, rate_per_year_bps: u32, elapsed_ms: i64) -> u128 {
    if elapsed_ms <= 0 || rate_per_year_bps == 0 {
        return index;
    }
    let index_b = BigUint::from(index);
    let growth = (&index_b * BigUint::from(rate_per_year_bps) * BigUint::from(elapsed_ms as u64))
        / (BigUint::from(BPS) * BigUint::from(MS_PER_YEAR));
    (index_b + growth).to_u128().expect("accrued index exceeds u128 range")
}

/// Real amount from a scaled position balance: scaled * index / RAY.
pub fn scaled_to_amount(scaled: u128, index: u128) -> u128 {
    let result = (BigUint::from(scaled) * BigUint::from(index)) / BigUint::from(RAY);
    result.to_u128().expect("scaled amount exceeds u128 range")
}

/// Scaled units for a real amount at the current index: amount * RAY / index.
pub fn amount_to_scaled(amount: u128, index: u128) -> u128 {
    if index == 0 {
        return 0;
    }
    let result = (BigUint::from(amount) * BigUint::from(RAY)) / BigUint::from(index);
    result.to_u128().expect("scaled amount exceeds u128 range")
}
