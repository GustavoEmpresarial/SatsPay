//! Sanity/parity tests against hand-computed values from the legacy `coins.ts`
//! formulas (same integer-division semantics, spot-checked by hand since no
//! Node runtime is available in this environment to diff against the TS
//! implementation directly).

use crate::{
    accrue_index, amount_to_scaled, calc_borrow_rate_bps, calc_flex_reward, calc_stake_reward,
    calc_supply_rate_bps, calc_utilization_bps, coin_config, compute_swap, format_amount,
    from_onchain_amount, is_flexible_plan, lend_market, scaled_to_amount, to_onchain_amount, Coin,
    COINS, RAY, STAKE_PLANS,
};

#[test]
fn format_amount_trims_trailing_zeros() {
    assert_eq!(format_amount(123_456_789, Coin::Btc), "1.23456789");
    assert_eq!(format_amount(100_000_000, Coin::Btc), "1");
    assert_eq!(format_amount(150_000_000, Coin::Btc), "1.5");
    assert_eq!(format_amount(0, Coin::Btc), "0");
}

#[test]
fn coin_config_matches_legacy_constants() {
    let btc = coin_config(Coin::Btc);
    assert_eq!(btc.decimals, 8);
    assert_eq!(btc.min_withdrawal, 1);
    assert_eq!(btc.withdrawal_fee, 1_000);
    assert_eq!(btc.approval_threshold, 1_500_000);

    let pol = coin_config(Coin::Pol);
    assert_eq!(pol.decimals, 8);
    assert_eq!(pol.min_withdrawal, 1);
    assert_eq!(pol.withdrawal_fee, 3_000_000);
    assert_eq!(pol.approval_threshold, 250_000_000_000);

    assert_eq!(COINS.len(), 9);
    assert_eq!(coin_config(Coin::Dgb).name, "DigiByte");
    assert_eq!(coin_config(Coin::Sol).name, "Solana");
    assert_eq!(coin_config(Coin::Usdt).name, "Tether USD");
    assert_eq!(coin_config(Coin::Usdc).name, "USD Coin");
}

#[test]
fn swap_l2_allowlist() {
    use crate::{is_swap_l2_coin, is_swap_l2_pair, SWAP_L2_COINS};
    assert_eq!(SWAP_L2_COINS, [Coin::Pol, Coin::Usdt, Coin::Usdc]);
    assert!(is_swap_l2_coin(Coin::Pol));
    assert!(!is_swap_l2_coin(Coin::Btc));
    assert!(is_swap_l2_pair(Coin::Pol, Coin::Usdt));
    assert!(!is_swap_l2_pair(Coin::Pol, Coin::Pol));
    assert!(!is_swap_l2_pair(Coin::Btc, Coin::Ltc));
}

#[test]
fn deposit_withdraw_pause_list() {
    use crate::{is_deposit_withdraw_paused, DEPOSIT_WITHDRAW_PAUSED_COINS};
    assert_eq!(DEPOSIT_WITHDRAW_PAUSED_COINS, [Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Dgb]);
    assert!(is_deposit_withdraw_paused(Coin::Btc));
    assert!(is_deposit_withdraw_paused(Coin::Ltc));
    assert!(is_deposit_withdraw_paused(Coin::Doge));
    assert!(is_deposit_withdraw_paused(Coin::Dgb));
    assert!(!is_deposit_withdraw_paused(Coin::Pol));
    assert!(!is_deposit_withdraw_paused(Coin::Bch));
}

#[test]
fn compute_swap_gross_minus_fee() {
    // 1 BTC (8 decimals) -> LTC (8 decimals), same price scale, 50bps fee.
    // priceFrom=priceTo means 1:1, so gross == fromAmount (same decimals).
    let quote = compute_swap(Coin::Btc, Coin::Ltc, 100_000_000, 1, 1, 50, 8).unwrap();
    assert_eq!(quote.fee_amount, 500_000); // 0.5% of 100_000_000
    assert_eq!(quote.to_amount, 99_500_000);
    assert_eq!(quote.fee_amount + quote.to_amount, 100_000_000);
}

#[test]
fn compute_swap_rejects_zero_amount() {
    assert!(compute_swap(Coin::Btc, Coin::Ltc, 0, 1, 1, 50, 8).is_err());
}

#[test]
fn compute_swap_handles_pol_scale_without_overflow() {
    // POL has 18 decimals; this would overflow a naive u128 gross computation
    // (from_amount * price * 10^18) for large amounts — BigUint avoids that.
    let quote = compute_swap(
        Coin::Pol,
        Coin::Btc,
        500_000_000_000_000_000_000, // 500 POL
        7_000_000_000,               // price scaled by 1e8
        7_000_000_000_000,           // price scaled by 1e8 (BTC ~$70000)
        50,
        8,
    )
    .unwrap();
    assert!(quote.to_amount > 0);
}

#[test]
fn coin_parse_round_trips_all_tickers() {
    for coin in COINS {
        assert_eq!(coin.as_str().parse::<Coin>().unwrap(), coin);
    }
    assert_eq!("btc".parse::<Coin>().unwrap(), Coin::Btc); // case-insensitive
    assert!("ETH".parse::<Coin>().is_err());
    assert!("".parse::<Coin>().is_err());
}

#[test]
fn onchain_ledger_round_trip_for_scaled_coins() {
    // BTC 8↔8 identity
    assert_eq!(to_onchain_amount(Coin::Btc, 100_000_000), 100_000_000);
    assert_eq!(from_onchain_amount(Coin::Btc, 100_000_000), 100_000_000);
    // USDT 8↔6
    assert_eq!(to_onchain_amount(Coin::Usdt, 100_000_000), 1_000_000); // 1 USDT
    assert_eq!(from_onchain_amount(Coin::Usdt, 1_000_000), 100_000_000);
    // POL 8↔18
    assert_eq!(to_onchain_amount(Coin::Pol, 100_000_000), 100_000_000 * 10u128.pow(10));
    assert_eq!(from_onchain_amount(Coin::Pol, 10u128.pow(18)), 100_000_000);
    // SOL 8↔9
    assert_eq!(to_onchain_amount(Coin::Sol, 100_000_000), 1_000_000_000);
}

#[test]
fn stake_plans_and_rewards() {
    assert!(is_flexible_plan(0));
    assert!(!is_flexible_plan(30));
    assert_eq!(STAKE_PLANS.len(), 5);
    // 1 BTC @ 1000 bps for 365 days ≈ 10% = 0.1 BTC
    let reward = calc_stake_reward(100_000_000, 1000, 365);
    assert_eq!(reward, 10_000_000);
    // Flexible: 1 day elapsed of 200 bps annual
    let flex = calc_flex_reward(100_000_000, 200, 24 * 60 * 60 * 1000);
    assert_eq!(flex, calc_stake_reward(100_000_000, 200, 1));
    assert_eq!(calc_flex_reward(100_000_000, 200, -1), 0);
}

#[test]
fn lend_utilization_and_kinked_rates() {
    assert_eq!(calc_utilization_bps(0, 0), 0);
    assert_eq!(calc_utilization_bps(50, 50), 5_000);
    assert_eq!(calc_utilization_bps(100, 0), 10_000);

    let m = lend_market(Coin::Btc);
    assert!(m.can_be_collateral);
    assert!(m.borrow_enabled);
    assert!(m.liquidation_threshold_bps > m.collateral_factor_bps);

    let below = calc_borrow_rate_bps(4_000, &m); // under kink
    let at = calc_borrow_rate_bps(m.optimal_utilization_bps, &m);
    let above = calc_borrow_rate_bps(9_500, &m);
    assert!(below <= at);
    assert!(above > at);

    let supply = calc_supply_rate_bps(above, 9_500, m.reserve_factor_bps);
    assert!(supply < above); // reserve factor cuts supply rate

    let idx = accrue_index(RAY, 1_000, 0);
    assert_eq!(idx, RAY);
    let grown = accrue_index(RAY, 1_000, 365 * 24 * 60 * 60 * 1000); // ~1 year @ 10%
    assert!(grown > RAY);

    let scaled = amount_to_scaled(100, RAY);
    assert_eq!(scaled_to_amount(scaled, RAY), 100);
    assert_eq!(amount_to_scaled(100, 0), 0);
}

#[test]
fn lend_markets_defined_for_every_coin() {
    for coin in COINS {
        let m = lend_market(coin);
        assert!(m.optimal_utilization_bps > 0 && m.optimal_utilization_bps < 10_000);
        assert!(m.collateral_factor_bps <= m.liquidation_threshold_bps);
    }
}

#[test]
fn coin_config_defined_for_every_coin() {
    for coin in COINS {
        let c = coin_config(coin);
        assert_eq!(c.symbol, coin);
        assert!(!c.name.is_empty());
        assert_eq!(c.decimals, 8);
        assert!(c.min_withdrawal >= 1);
        assert!(c.withdrawal_fee >= 1);
        assert!(c.min_confirmations >= 1);
        assert!(c.faucet_reward >= 1);
        assert!(c.display_color.starts_with('#'));
        assert!(c.approval_threshold > c.withdrawal_fee);
        assert!(!format_amount(c.withdrawal_fee, coin).is_empty());
    }
}

#[test]
fn compute_swap_fee_scales_with_bps() {
    let low = compute_swap(Coin::Btc, Coin::Ltc, 100_000_000, 1, 1, 25, 8).unwrap();
    let high = compute_swap(Coin::Btc, Coin::Ltc, 100_000_000, 1, 1, 100, 8).unwrap();
    assert_eq!(low.fee_amount, 250_000);
    assert_eq!(high.fee_amount, 1_000_000);
    assert!(high.to_amount < low.to_amount);
}
