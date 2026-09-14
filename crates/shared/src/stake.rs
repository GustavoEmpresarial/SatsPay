//! Port 1:1 of the staking section of legacy `coins.ts`.

#[derive(Debug, Clone, Copy)]
pub struct StakePlan {
    /// 0 = flexible — no lock, unstake anytime, reward accrues pro-rata.
    pub lock_days: u32,
    pub reward_bps: u32,
    pub label: &'static str,
}

pub const STAKE_PLANS: [StakePlan; 5] = [
    StakePlan { lock_days: 0, reward_bps: 200, label: "Flexible" },
    StakePlan { lock_days: 7, reward_bps: 300, label: "Starter · 7d" },
    StakePlan { lock_days: 30, reward_bps: 600, label: "Standard · 30d" },
    StakePlan { lock_days: 90, reward_bps: 1000, label: "Growth · 90d" },
    StakePlan { lock_days: 180, reward_bps: 1500, label: "Locked · 180d" },
];

pub fn is_flexible_plan(lock_days: u32) -> bool {
    lock_days == 0
}

/// reward = principal * (reward_bps / 10000) * (lock_days / 365)
pub fn calc_stake_reward(principal: u128, reward_bps: u32, lock_days: u32) -> u128 {
    principal
        .saturating_mul(reward_bps as u128)
        .saturating_mul(lock_days as u128)
        / (10_000u128 * 365)
}

/// Pro-rata accrued reward for a flexible stake given elapsed time.
pub fn calc_flex_reward(principal: u128, reward_bps: u32, elapsed_ms: i64) -> u128 {
    let elapsed_days = (elapsed_ms.max(0) / (24 * 60 * 60 * 1000)) as u32;
    calc_stake_reward(principal, reward_bps, elapsed_days)
}
