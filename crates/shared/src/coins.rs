//! Port 1:1 of legacy `packages/shared/src/coins.ts`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Coin {
    #[serde(rename = "BTC")]
    Btc,
    #[serde(rename = "LTC")]
    Ltc,
    #[serde(rename = "DOGE")]
    Doge,
    #[serde(rename = "BCH")]
    Bch,
    #[serde(rename = "POL")]
    Pol,
    #[serde(rename = "DGB")]
    Dgb,
    #[serde(rename = "SOL")]
    Sol,
    #[serde(rename = "USDT")]
    Usdt,
    #[serde(rename = "USDC")]
    Usdc,
}

pub const COINS: [Coin; 9] = [
    Coin::Btc,
    Coin::Ltc,
    Coin::Doge,
    Coin::Bch,
    Coin::Pol,
    Coin::Dgb,
    Coin::Sol,
    Coin::Usdt,
    Coin::Usdc,
];

/// Polygon L2 assets currently allowed for DEX swap (no L1 / no HOUSE inventory).
pub const SWAP_L2_COINS: [Coin; 3] = [Coin::Pol, Coin::Usdt, Coin::Usdc];

/// Temporary pause: personal deposits/withdrawals **and** merchant deposit
/// gateway invoices stay listed where applicable but are rejected by the API.
/// Currently: BTC, LTC, DOGE, DGB.
/// Does **not** affect `/v1/public/send` (merchant ledger payout to users).
pub const DEPOSIT_WITHDRAW_PAUSED_COINS: [Coin; 4] = [Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Dgb];

/// True when `coin` is in the temporary L2-only swap allowlist.
pub fn is_swap_l2_coin(coin: Coin) -> bool {
    SWAP_L2_COINS.contains(&coin)
}

/// True when deposits/withdrawals for `coin` are temporarily paused.
pub fn is_deposit_withdraw_paused(coin: Coin) -> bool {
    DEPOSIT_WITHDRAW_PAUSED_COINS.contains(&coin)
}

/// Both legs must be L2 allowlisted and distinct.
pub fn is_swap_l2_pair(from: Coin, to: Coin) -> bool {
    from != to && is_swap_l2_coin(from) && is_swap_l2_coin(to)
}

impl Coin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Coin::Btc => "BTC",
            Coin::Ltc => "LTC",
            Coin::Doge => "DOGE",
            Coin::Bch => "BCH",
            Coin::Pol => "POL",
            Coin::Dgb => "DGB",
            Coin::Sol => "SOL",
            Coin::Usdt => "USDT",
            Coin::Usdc => "USDC",
        }
    }

    /// On-chain smallest-unit decimals. Internal ledger is always 8.
    pub fn onchain_decimals(self) -> u32 {
        match self {
            Coin::Btc | Coin::Ltc | Coin::Doge | Coin::Bch | Coin::Dgb => 8,
            Coin::Pol => 18,
            Coin::Usdt | Coin::Usdc => 6,
            Coin::Sol => 9,
        }
    }
}

/// Convert ledger units (8 decimals) to on-chain smallest units.
pub fn to_onchain_amount(coin: Coin, internal: u128) -> u128 {
    let on = coin.onchain_decimals();
    if on >= 8 {
        internal.saturating_mul(10u128.pow(on - 8))
    } else {
        internal / 10u128.pow(8 - on)
    }
}

/// Convert on-chain smallest units to ledger units (8 decimals).
pub fn from_onchain_amount(coin: Coin, onchain: u128) -> u128 {
    let on = coin.onchain_decimals();
    if on >= 8 {
        onchain / 10u128.pow(on - 8)
    } else {
        onchain.saturating_mul(10u128.pow(8 - on))
    }
}

impl std::str::FromStr for Coin {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        COINS.into_iter().find(|c| c.as_str().eq_ignore_ascii_case(s)).ok_or(())
    }
}

/// Amounts >= `approval_threshold` (in the coin's smallest unit) require manual
/// admin approval. Per-coin because 1 unit of POL (18 decimals) is nothing like
/// 1 sat of BTC.
#[derive(Debug, Clone, Copy)]
pub struct CoinConfig {
    pub symbol: Coin,
    pub name: &'static str,
    pub decimals: u32,
    pub min_withdrawal: u128,
    pub withdrawal_fee: u128,
    pub min_confirmations: u32,
    pub faucet_reward: u128,
    pub display_color: &'static str,
    pub approval_threshold: u128,
}

pub fn coin_config(coin: Coin) -> CoinConfig {
    match coin {
        Coin::Btc => CoinConfig {
            symbol: Coin::Btc,
            name: "Bitcoin",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 1_000, // 0.00001000 BTC — FaucetPay saque normal
            min_confirmations: 2,
            faucet_reward: 1, // 0.00000001 BTC (1 sat) — near-zero
            display_color: "#F7931A",
            approval_threshold: 1_500_000, // 0.015 BTC (~$1,000 USD)
        },
        Coin::Ltc => CoinConfig {
            symbol: Coin::Ltc,
            name: "Litecoin",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 2_000, // 0.00002000 LTC — FaucetPay saque normal
            min_confirmations: 6,
            faucet_reward: 1, // 0.00000001 LTC — near-zero
            display_color: "#345D9D",
            approval_threshold: 1_000_000_000, // 10 LTC (~$1,000 USD)
        },
        Coin::Doge => CoinConfig {
            symbol: Coin::Doge,
            name: "Dogecoin",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 100_000_000, // 1 DOGE — FaucetPay saque normal
            min_confirmations: 20,
            faucet_reward: 1, // 0.00000001 DOGE — near-zero
            display_color: "#C2A633",
            approval_threshold: 600_000_000_000, // 6,000 DOGE (~$1,000 USD)
        },
        Coin::Bch => CoinConfig {
            symbol: Coin::Bch,
            name: "Bitcoin Cash",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 5_000, // 0.00005000 BCH — FaucetPay saque normal
            min_confirmations: 2,
            faucet_reward: 1, // 0.00000001 BCH — near-zero
            display_color: "#0AC18E",
            approval_threshold: 250_000_000, // 2.5 BCH (~$1,000 USD)
        },
        Coin::Pol => CoinConfig {
            symbol: Coin::Pol,
            name: "Polygon",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 3_000_000, // 0.03000000 POL — FaucetPay saque normal
            min_confirmations: 30,
            faucet_reward: 1, // 0.00000001 POL — near-zero
            display_color: "#8247E5",
            approval_threshold: 250_000_000_000, // 2,500 POL (~$1,000 USD)
        },
        Coin::Dgb => CoinConfig {
            symbol: Coin::Dgb,
            name: "DigiByte",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 25_000_000, // 0.25000000 DGB — FaucetPay saque normal
            min_confirmations: 40,
            faucet_reward: 1, // 0.00000001 DGB — near-zero
            display_color: "#0066CC",
            approval_threshold: 12_000_000_000_000, // 120,000 DGB (~$1,000 USD)
        },
        Coin::Sol => CoinConfig {
            symbol: Coin::Sol,
            name: "Solana",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 10_000, // 0.00010000 SOL — FaucetPay saque normal
            min_confirmations: 32,
            faucet_reward: 1, // 0.00000001 SOL — near-zero
            display_color: "#14F195",
            approval_threshold: 600_000_000, // 6 SOL (~$1,000 USD)
        },
        Coin::Usdt => CoinConfig {
            symbol: Coin::Usdt,
            name: "Tether USD",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 1_000_000, // 0.01000000 USDT
            min_confirmations: 30,
            faucet_reward: 1, // 0.00000001 USDT — near-zero
            display_color: "#26A17B",
            approval_threshold: 100_000_000_000, // 1,000 USDT
        },
        Coin::Usdc => CoinConfig {
            symbol: Coin::Usdc,
            name: "USD Coin",
            decimals: 8,
            min_withdrawal: 1, // no minimum — 1 atomic unit
            withdrawal_fee: 1_000_000, // 0.01000000 USDC
            min_confirmations: 30,
            faucet_reward: 1, // 0.00000001 USDC — near-zero
            display_color: "#2775CA",
            approval_threshold: 100_000_000_000, // 1,000 USDC
        },
    }
}

/// Renders an integer smallest-unit amount as a human-readable decimal string,
/// trimming trailing zeros — port of `formatAmount`.
pub fn format_amount(amount: u128, coin: Coin) -> String {
    let cfg = coin_config(coin);
    let divisor = 10u128.pow(cfg.decimals);
    let whole = amount / divisor;
    let frac = amount % divisor;
    let frac_str = format!("{:0width$}", frac, width = cfg.decimals as usize);
    let trimmed = frac_str.trim_end_matches('0');
    if trimmed.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{trimmed}")
    }
}
