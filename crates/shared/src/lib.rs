pub mod coins;
pub mod lend;
pub mod stake;
pub mod swap;

pub use coins::{
    coin_config, format_amount, from_onchain_amount, is_deposit_withdraw_paused, is_swap_l2_coin,
    is_swap_l2_pair, to_onchain_amount, Coin, CoinConfig, COINS, DEPOSIT_WITHDRAW_PAUSED_COINS,
    SWAP_L2_COINS,
};
pub use lend::{
    accrue_index, amount_to_scaled, calc_borrow_rate_bps, calc_supply_rate_bps, calc_utilization_bps,
    lend_market, scaled_to_amount, LendMarket, RAY,
};
pub use stake::{calc_flex_reward, calc_stake_reward, is_flexible_plan, StakePlan, STAKE_PLANS};
pub use swap::{compute_swap, SwapError, SwapQuote};

#[cfg(test)]
mod tests;
