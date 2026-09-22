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
    #[serde(rename = "ZER")]
    Zer,
    #[serde(rename = "PEPE")]
    Pepe,
}

pub const COINS: [Coin; 11] = [
    Coin::Btc,
    Coin::Ltc,
    Coin::Doge,
    Coin::Bch,
    Coin::Pol,
    Coin::Dgb,
    Coin::Sol,
    Coin::Usdt,
    Coin::Usdc,
    Coin::Zer,
    Coin::Pepe,
];

/// Assets allowed for custodial DEX / Relay (Polygon L2 + SOL bridge).
pub const SWAP_L2_COINS: [Coin; 4] = [Coin::Pol, Coin::Usdt, Coin::Usdc, Coin::Sol];

/// Layer-1 UTXO coins swapped via ChangeNOW (deposit-address flow).
pub const SWAP_L1_COINS: [Coin; 5] = [Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Bch, Coin::Dgb];

/// Full swap picker: L1 (ChangeNOW) + L2/SOL (DEX/Relay). SOL near top for UX.
pub const SWAP_COINS: [Coin; 9] = [
    Coin::Btc,
    Coin::Sol,
    Coin::Ltc,
    Coin::Doge,
    Coin::Bch,
    Coin::Dgb,
    Coin::Pol,
    Coin::Usdt,
    Coin::Usdc,
];

/// Same-network DEX (UI "Swap" tab): Polygon only — POL ↔ USDT ↔ USDC.
pub const DEX_SWAP_COINS: [Coin; 3] = [Coin::Pol, Coin::Usdt, Coin::Usdc];

/// Custodial chain for a coin on SatsPay (UI + docs). PEPE is BSC, not ETH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoinNetwork {
    pub id: &'static str,
    pub short: &'static str,
    pub label: &'static str,
}

pub fn coin_network(coin: Coin) -> CoinNetwork {
    match coin {
        Coin::Btc => CoinNetwork {
            id: "bitcoin",
            short: "Bitcoin",
            label: "Bitcoin",
        },
        Coin::Ltc => CoinNetwork {
            id: "litecoin",
            short: "Litecoin",
            label: "Litecoin",
        },
        Coin::Doge => CoinNetwork {
            id: "dogecoin",
            short: "Dogecoin",
            label: "Dogecoin",
        },
        Coin::Bch => CoinNetwork {
            id: "bitcoincash",
            short: "Bitcoin Cash",
            label: "Bitcoin Cash",
        },
        Coin::Dgb => CoinNetwork {
            id: "digibyte",
            short: "DigiByte",
            label: "DigiByte",
        },
        Coin::Pol => CoinNetwork {
            id: "polygon",
            short: "Polygon",
            label: "Polygon PoS",
        },
        Coin::Usdt => CoinNetwork {
            id: "polygon",
            short: "Polygon",
            label: "Polygon PoS (USDT bridged)",
        },
        Coin::Usdc => CoinNetwork {
            id: "polygon",
            short: "Polygon",
            label: "Polygon PoS (USDC native)",
        },
        Coin::Sol => CoinNetwork {
            id: "solana",
            short: "Solana",
            label: "Solana",
        },
        Coin::Zer => CoinNetwork {
            id: "zero",
            short: "Zero",
            label: "Zero (transparent t1)",
        },
        // Binance-Peg PEPE — gas in BNB. Not Ethereum PEPE.
        Coin::Pepe => CoinNetwork {
            id: "bsc",
            short: "BSC",
            label: "BNB Smart Chain (BEP-20)",
        },
    }
}

pub fn is_dex_swap_coin(coin: Coin) -> bool {
    DEX_SWAP_COINS.contains(&coin)
}

pub fn is_same_swap_network(from: Coin, to: Coin) -> bool {
    coin_network(from).id == coin_network(to).id
}

/// UI Swap tab: same-network DEX pair (Polygon stables today).
pub fn is_dex_swap_pair(from: Coin, to: Coin) -> bool {
    from != to && is_dex_swap_coin(from) && is_dex_swap_coin(to)
}

/// UI Bridge tab: allowlisted coins on distinct custodial networks.
pub fn is_bridge_pair(from: Coin, to: Coin) -> bool {
    from != to && is_swap_coin(from) && is_swap_coin(to) && !is_same_swap_network(from, to)
}

/// Temporary pause: personal deposits/withdrawals **and** merchant deposit
/// gateway invoices stay listed where applicable but are rejected by the API.
/// Currently: BTC, LTC, DOGE, BCH, DGB.
/// Does **not** affect `/v1/public/send` (merchant ledger payout to users).
/// Does **not** block custodial ChangeNOW swaps from ledger balance.
pub const DEPOSIT_WITHDRAW_PAUSED_COINS: [Coin; 5] =
    [Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Bch, Coin::Dgb];

/// True when `coin` is in the Polygon/SOL DEX allowlist.
pub fn is_swap_l2_coin(coin: Coin) -> bool {
    SWAP_L2_COINS.contains(&coin)
}

/// True when `coin` is an L1 UTXO swap asset (ChangeNOW).
pub fn is_swap_l1_coin(coin: Coin) -> bool {
    SWAP_L1_COINS.contains(&coin)
}

/// True when `coin` appears in the swap UI / API allowlist.
pub fn is_swap_coin(coin: Coin) -> bool {
    SWAP_COINS.contains(&coin)
}

/// True when deposits/withdrawals for `coin` are temporarily paused.
pub fn is_deposit_withdraw_paused(coin: Coin) -> bool {
    DEPOSIT_WITHDRAW_PAUSED_COINS.contains(&coin)
}

/// Both legs must be L2/SOL swap-allowlisted and distinct.
pub fn is_swap_l2_pair(from: Coin, to: Coin) -> bool {
    from != to && is_swap_l2_coin(from) && is_swap_l2_coin(to)
}

/// Any distinct pair among full swap allowlist (L1 + L2/SOL).
pub fn is_swap_pair(from: Coin, to: Coin) -> bool {
    from != to && is_swap_coin(from) && is_swap_coin(to)
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
            Coin::Zer => "ZER",
            Coin::Pepe => "PEPE",
        }
    }

    /// On-chain smallest-unit decimals. Internal ledger is always 8.
    pub fn onchain_decimals(self) -> u32 {
        match self {
            Coin::Btc | Coin::Ltc | Coin::Doge | Coin::Bch | Coin::Dgb | Coin::Zer => 8,
            Coin::Pol | Coin::Pepe => 18,
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
        Coin::Zer => CoinConfig {
            symbol: Coin::Zer,
            name: "Zero",
            decimals: 8,
            min_withdrawal: 1,
            // On-chain miner fee is ~0.0001 ZER; platform fee 0.001 ZER.
            withdrawal_fee: 100_000, // 0.001 ZER
            min_confirmations: 10,
            faucet_reward: 1,
            display_color: "#1a1a1a",
            approval_threshold: 10_000_000_000_000, // 100,000 ZER ≈ $1,000
        },
        Coin::Pepe => CoinConfig {
            symbol: Coin::Pepe,
            name: "Pepe",
            decimals: 8,
            min_withdrawal: 1,
            // Product fee in PEPE. On-chain gas is paid in BNB by the hot wallet.
            withdrawal_fee: 5_000_000_000_000, // 50,000 PEPE
            min_confirmations: 15,
            faucet_reward: 1,
            display_color: "#3CB43C",
            approval_threshold: 25_000_000_000_000_000, // 250,000,000 PEPE ≈ $1,000
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
