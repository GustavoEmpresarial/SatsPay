//! Swap quote math — port 1:1 of legacy
//! `packages/shared/src/coins.ts::computeSwap` (also mirrored in
//! `client/src/shared/coins.ts`). Pure, no I/O.
//!
//! `gross = from_amount * price_from * 10^to_dec / (price_to * 10^from_dec)`,
//! then `fee = gross * fee_bps / 10_000` and `to_amount = gross - fee`, all
//! in integer (floor) arithmetic. Computed via `BigUint` so a large POL
//! (18-decimal) `from_amount * price * 10^dec` product cannot overflow
//! `u128` mid-calculation. `price_decimals` cancels out of the ratio (both
//! prices share one scale) and is only carried through for the caller / DB
//! row, matching the TS port's `void priceDecimals`.

use crate::coins::coin_config;
use crate::Coin;
use num_bigint::BigUint;
use num_traits::ToPrimitive;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SwapError {
    #[error("fromAmount must be > 0")]
    ZeroAmount,
    #[error("invalid prices")]
    InvalidPrices,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapQuote {
    pub from_amount: u128,
    pub to_amount: u128,
    pub fee_amount: u128,
    pub fee_bps: u32,
    pub price_from: u128,
    pub price_to: u128,
    pub price_decimals: u32,
}

pub fn compute_swap(
    from_coin: Coin,
    to_coin: Coin,
    from_amount: u128,
    price_from: u128,
    price_to: u128,
    fee_bps: u32,
    price_decimals: u32,
) -> Result<SwapQuote, SwapError> {
    if from_amount == 0 {
        return Err(SwapError::ZeroAmount);
    }
    if price_from == 0 || price_to == 0 {
        return Err(SwapError::InvalidPrices);
    }

    let from_dec = coin_config(from_coin).decimals;
    let to_dec = coin_config(to_coin).decimals;

    let ten = BigUint::from(10u32);
    let numer = BigUint::from(from_amount) * BigUint::from(price_from) * ten.pow(to_dec);
    let denom = BigUint::from(price_to) * ten.pow(from_dec);
    let gross_big = numer / denom;

    let fee_big = (&gross_big * BigUint::from(fee_bps)) / BigUint::from(10_000u32);
    let to_big = &gross_big - &fee_big;

    Ok(SwapQuote {
        from_amount,
        to_amount: to_big.to_u128().unwrap_or(u128::MAX),
        fee_amount: fee_big.to_u128().unwrap_or(u128::MAX),
        fee_bps,
        price_from,
        price_to,
        price_decimals,
    })
}
