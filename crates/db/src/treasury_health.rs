//! Admin treasury-health aggregations: USD P&L, break-even, HOUSE runway,
//! pending liabilities, hot buffers, fee time series, fee-margin hard-block.

use crate::house;
use crate::pricing;
use chrono::{Duration as ChronoDuration, Utc};
use serde::Serialize;
use shared::{Coin, COINS};
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, thiserror::Error)]
pub enum TreasuryHealthError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Pricing(#[from] pricing::PricingDbError),
    #[error(transparent)]
    House(#[from] house::HouseError),
}

fn parse_i128(s: &str) -> i128 {
    s.parse().unwrap_or(0)
}

fn i128_str(v: i128) -> String {
    v.to_string()
}

#[derive(Debug, Serialize, Clone)]
pub struct PnlUsd {
    pub fees_earned_usd: String,
    pub network_paid_usd: String,
    pub faucet_cost_usd: String,
    pub fee_margin_usd: String,
    pub operating_margin_usd: String,
    pub price_decimals: u32,
    pub prices_available: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct BreakEvenCoin {
    pub coin: String,
    pub withdrawal_fee: String,
    pub avg_network_fee: String,
    pub sample_count: i64,
    pub covers: bool,
    pub gap: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct HouseRunwayCoin {
    pub coin: String,
    pub house_balance: String,
    pub burn_24h: String,
    pub burn_7d: String,
    pub days_at_24h_rate: Option<f64>,
    pub days_at_7d_rate: Option<f64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PendingLiabilityCoin {
    pub coin: String,
    pub count: i64,
    pub amount: String,
    pub platform_fees: String,
    pub est_network_fees: String,
    pub total_out: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct HotBufferCoin {
    pub coin: String,
    pub onchain: String,
    pub custody: String,
    pub pending_out: String,
    pub target: String,
    pub shortfall: String,
    pub status: String, // ok | low | critical | rpc
}

#[derive(Debug, Serialize, Clone)]
pub struct FeeSeriesDay {
    pub day: String,
    pub fees_earned: String,
    pub network_paid: String,
    pub fees_earned_usd: String,
    pub network_paid_usd: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FeeMarginBlockStatus {
    pub enabled: bool,
    pub blocked_coins: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UnsweptCoin {
    pub coin: String,
    pub address_count: i64,
    pub onchain_total: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct TreasuryHealth {
    pub pnl_usd: PnlUsd,
    pub break_even: Vec<BreakEvenCoin>,
    pub house_runway: Vec<HouseRunwayCoin>,
    pub pending_liabilities: Vec<PendingLiabilityCoin>,
    pub hot_buffers: Vec<HotBufferCoin>,
    pub fee_series_7d: Vec<FeeSeriesDay>,
    pub fee_series_30d: Vec<FeeSeriesDay>,
    pub fee_margin_block: FeeMarginBlockStatus,
    /// Populated by API layer after scanning deposit wallets (optional).
    pub unswept_deposits: Vec<UnsweptCoin>,
}

fn hard_block_enabled() -> bool {
    match std::env::var("FEE_MARGIN_HARD_BLOCK") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t == "0" || t == "false" || t == "off" || t == "no")
        }
        // Default ON — never spend more network than we earn.
        Err(_) => true,
    }
}

/// Returns true when withdrawals/faucet for `coin` must be rejected.
pub async fn fee_margin_blocks_coin(pool: &PgPool, coin: Coin) -> Result<bool, TreasuryHealthError> {
    if !hard_block_enabled() {
        return Ok(false);
    }
    let blocked = blocked_coins(pool).await?;
    Ok(blocked.iter().any(|c| c == coin.as_str()))
}

async fn blocked_coins(pool: &PgPool) -> Result<Vec<String>, TreasuryHealthError> {
    let rows = sqlx::query(
        r#"
        WITH earned_raw AS (
            SELECT coin::text AS coin, fee_amount AS fees
            FROM merchant_deposit_invoices WHERE status = 'CONFIRMED'
            UNION ALL
            SELECT wa.coin::text, w.fee_amount
            FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id
            WHERE w.status IN ('BROADCASTED', 'CONFIRMED')
            UNION ALL
            SELECT from_coin::text, platform_fee_amount
            FROM dex_swaps WHERE status = 'COMPLETED'
        ),
        earned AS (
            SELECT coin, COALESCE(SUM(fees), 0) AS fees FROM earned_raw GROUP BY coin
        ),
        paid AS (
            SELECT coin::text AS coin, COALESCE(SUM(amount), 0) AS network
            FROM network_fee_events GROUP BY coin
        )
        SELECT COALESCE(e.coin, p.coin) AS coin
        FROM earned e
        FULL OUTER JOIN paid p ON p.coin = e.coin
        WHERE COALESCE(p.network, 0) > 0 AND COALESCE(e.fees, 0) < COALESCE(p.network, 0)
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.get::<String, _>("coin")).collect())
}

fn usd_of(prices: &HashMap<Coin, u128>, coin: Coin, amount: i128, _price_decimals: u32) -> u128 {
    if amount <= 0 {
        return 0;
    }
    let price = *prices.get(&coin).unwrap_or(&0);
    if price == 0 {
        return 0;
    }
    // ledger units are 8 decimals; USD = amount * price / 10^8
    let amt = amount as u128;
    let result = (num_bigint::BigUint::from(amt) * num_bigint::BigUint::from(price))
        / num_bigint::BigUint::from(10u128.pow(8));
    num_traits::ToPrimitive::to_u128(&result).unwrap_or(0)
}

async fn sum_fees_all_time(pool: &PgPool) -> Result<BTreeMap<String, i128>, TreasuryHealthError> {
    let mut map = BTreeMap::new();
    let gw = sqlx::query(
        "SELECT coin::text as coin, COALESCE(SUM(fee_amount),0)::text as fees \
         FROM merchant_deposit_invoices WHERE status='CONFIRMED' GROUP BY coin",
    )
    .fetch_all(pool)
    .await?;
    for r in gw {
        *map.entry(r.get::<String, _>("coin")).or_default() += parse_i128(&r.get::<String, _>("fees"));
    }
    let wd = sqlx::query(
        "SELECT wa.coin::text as coin, COALESCE(SUM(w.fee_amount),0)::text as fees \
         FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id \
         WHERE w.status IN ('BROADCASTED','CONFIRMED') GROUP BY wa.coin",
    )
    .fetch_all(pool)
    .await?;
    for r in wd {
        *map.entry(r.get::<String, _>("coin")).or_default() += parse_i128(&r.get::<String, _>("fees"));
    }
    let sw = sqlx::query(
        "SELECT from_coin::text as coin, COALESCE(SUM(platform_fee_amount),0)::text as fees \
         FROM dex_swaps WHERE status='COMPLETED' GROUP BY from_coin",
    )
    .fetch_all(pool)
    .await?;
    for r in sw {
        *map.entry(r.get::<String, _>("coin")).or_default() += parse_i128(&r.get::<String, _>("fees"));
    }
    Ok(map)
}

async fn sum_network_all_time(pool: &PgPool) -> Result<BTreeMap<String, i128>, TreasuryHealthError> {
    let rows = sqlx::query(
        "SELECT coin::text as coin, COALESCE(SUM(amount),0)::text as amount FROM network_fee_events GROUP BY coin",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<String, _>("coin"), parse_i128(&r.get::<String, _>("amount"))))
        .collect())
}

async fn sum_faucet_all_time(pool: &PgPool) -> Result<BTreeMap<String, i128>, TreasuryHealthError> {
    let rows = sqlx::query(
        "SELECT coin::text as coin, COALESCE(SUM(amount),0)::text as amount FROM faucet_claims GROUP BY coin",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<String, _>("coin"), parse_i128(&r.get::<String, _>("amount"))))
        .collect())
}

pub async fn get_treasury_health(
    pool: &PgPool,
    hot_onchain: &HashMap<String, (String, Option<String>)>, // coin -> (onchain, error)
    custody: &HashMap<String, String>,
    unswept: Vec<UnsweptCoin>,
) -> Result<TreasuryHealth, TreasuryHealthError> {
    let (price_decimals, prices) = pricing::list_cached_prices(pool).await.unwrap_or((8, HashMap::new()));
    let prices_available = !prices.is_empty();

    let earned = sum_fees_all_time(pool).await?;
    let network = sum_network_all_time(pool).await?;
    let faucet = sum_faucet_all_time(pool).await?;

    let mut fees_usd = 0u128;
    let mut net_usd = 0u128;
    let mut faucet_usd = 0u128;
    for coin in COINS {
        let c = coin.as_str().to_string();
        fees_usd += usd_of(&prices, coin, *earned.get(&c).unwrap_or(&0), price_decimals);
        net_usd += usd_of(&prices, coin, *network.get(&c).unwrap_or(&0), price_decimals);
        faucet_usd += usd_of(&prices, coin, *faucet.get(&c).unwrap_or(&0), price_decimals);
    }
    let fee_margin_usd = fees_usd as i128 - net_usd as i128;
    let operating_margin_usd = fee_margin_usd - faucet_usd as i128;

    let pnl_usd = PnlUsd {
        fees_earned_usd: fees_usd.to_string(),
        network_paid_usd: net_usd.to_string(),
        faucet_cost_usd: faucet_usd.to_string(),
        fee_margin_usd: fee_margin_usd.to_string(),
        operating_margin_usd: operating_margin_usd.to_string(),
        price_decimals,
        prices_available,
    };

    // Break-even: avg WITHDRAWAL network fee vs CoinConfig.withdrawal_fee
    let avg_rows = sqlx::query(
        "SELECT coin::text as coin, COUNT(*)::bigint as n, \
                COALESCE(TRUNC(AVG(amount)),0)::text as avg_fee \
         FROM network_fee_events WHERE kind = 'WITHDRAWAL' \
         GROUP BY coin",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut avg_map: HashMap<String, (i64, i128)> = HashMap::new();
    for r in avg_rows {
        let avg = parse_i128(&r.get::<String, _>("avg_fee"));
        avg_map.insert(r.get("coin"), (r.get("n"), avg));
    }
    let break_even: Vec<_> = COINS
        .iter()
        .map(|coin| {
            let fee = shared::coin_config(*coin).withdrawal_fee as i128;
            let (n, avg) = avg_map.get(coin.as_str()).copied().unwrap_or((0, 0));
            let covers = n == 0 || fee >= avg;
            BreakEvenCoin {
                coin: coin.as_str().to_string(),
                withdrawal_fee: i128_str(fee),
                avg_network_fee: i128_str(avg),
                sample_count: n,
                covers,
                gap: i128_str(fee - avg),
            }
        })
        .collect();

    // HOUSE runway
    let house_bals = house::list_house_balances(pool).await.unwrap_or_default();
    let mut house_map: HashMap<String, i128> = HashMap::new();
    for (coin, bal) in &house_bals {
        house_map.insert(coin.as_str().to_string(), parse_i128(&bal.to_string()));
    }
    let burn_24h_rows = sqlx::query(
        "SELECT coin::text as coin, COALESCE(SUM(amount),0)::text as amount \
         FROM faucet_claims WHERE created_at >= NOW() - INTERVAL '24 hours' GROUP BY coin",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let burn_7d_rows = sqlx::query(
        "SELECT coin::text as coin, COALESCE(SUM(amount),0)::text as amount \
         FROM faucet_claims WHERE created_at >= NOW() - INTERVAL '7 days' GROUP BY coin",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut b24: HashMap<String, i128> = HashMap::new();
    let mut b7: HashMap<String, i128> = HashMap::new();
    for r in burn_24h_rows {
        b24.insert(r.get("coin"), parse_i128(&r.get::<String, _>("amount")));
    }
    for r in burn_7d_rows {
        b7.insert(r.get("coin"), parse_i128(&r.get::<String, _>("amount")));
    }
    let house_runway: Vec<_> = COINS
        .iter()
        .filter_map(|coin| {
            let bal = *house_map.get(coin.as_str()).unwrap_or(&0);
            let d24 = *b24.get(coin.as_str()).unwrap_or(&0);
            let d7 = *b7.get(coin.as_str()).unwrap_or(&0);
            if bal == 0 && d24 == 0 && d7 == 0 {
                return None;
            }
            let days_24 = if d24 > 0 {
                Some((bal as f64) / (d24 as f64))
            } else {
                None
            };
            let days_7 = if d7 > 0 {
                Some((bal as f64) / ((d7 as f64) / 7.0))
            } else {
                None
            };
            Some(HouseRunwayCoin {
                coin: coin.as_str().to_string(),
                house_balance: i128_str(bal),
                burn_24h: i128_str(d24),
                burn_7d: i128_str(d7),
                days_at_24h_rate: days_24,
                days_at_7d_rate: days_7,
            })
        })
        .collect();

    // Pending liabilities
    let pending_rows = sqlx::query(
        "SELECT wa.coin::text as coin, COUNT(*)::bigint as n, \
                COALESCE(SUM(w.amount),0)::text as amount, \
                COALESCE(SUM(w.fee_amount),0)::text as fees \
         FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id \
         WHERE w.status IN ('PENDING','APPROVED','QUEUED','BROADCASTING','BROADCASTED') \
         GROUP BY wa.coin",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let pending_liabilities: Vec<_> = pending_rows
        .into_iter()
        .map(|r| {
            let coin: String = r.get("coin");
            let n: i64 = r.get("n");
            let amount = parse_i128(&r.get::<String, _>("amount"));
            let platform_fees = parse_i128(&r.get::<String, _>("fees"));
            let avg = avg_map.get(&coin).map(|(_, a)| *a).unwrap_or_else(|| {
                coin.parse::<Coin>()
                    .ok()
                    .map(|c| shared::coin_config(c).withdrawal_fee as i128)
                    .unwrap_or(0)
            });
            let est_network = avg.saturating_mul(n as i128);
            PendingLiabilityCoin {
                coin,
                count: n,
                amount: i128_str(amount),
                platform_fees: i128_str(platform_fees),
                est_network_fees: i128_str(est_network),
                total_out: i128_str(amount + est_network),
            }
        })
        .collect();
    let pending_out_map: HashMap<String, i128> = pending_liabilities
        .iter()
        .map(|p| (p.coin.clone(), parse_i128(&p.total_out)))
        .collect();

    // Hot buffers
    let hot_buffers: Vec<_> = COINS
        .iter()
        .map(|coin| {
            let c = coin.as_str().to_string();
            let (onchain_s, err) = hot_onchain
                .get(&c)
                .cloned()
                .unwrap_or_else(|| ("0".into(), Some("missing".into())));
            let onchain = parse_i128(&onchain_s);
            let cust = parse_i128(custody.get(&c).map(|s| s.as_str()).unwrap_or("0"));
            let pend = *pending_out_map.get(&c).unwrap_or(&0);
            let fee = shared::coin_config(*coin).withdrawal_fee as i128;
            // Target: cover pending outs + 20× withdrawal fee buffer (or 5% custody).
            let target = pend
                .saturating_add(fee.saturating_mul(20))
                .max(cust / 20);
            let shortfall = (target - onchain).max(0);
            let status = if err.is_some() {
                "rpc".into()
            } else if onchain >= target {
                "ok".into()
            } else if onchain >= pend {
                "low".into()
            } else {
                "critical".into()
            };
            HotBufferCoin {
                coin: c,
                onchain: i128_str(onchain),
                custody: i128_str(cust),
                pending_out: i128_str(pend),
                target: i128_str(target),
                shortfall: i128_str(shortfall),
                status,
            }
        })
        .collect();

    let fee_series_7d = fee_series(pool, &prices, price_decimals, 7).await?;
    let fee_series_30d = fee_series(pool, &prices, price_decimals, 30).await?;

    let blocked = if hard_block_enabled() {
        blocked_coins(pool).await?
    } else {
        vec![]
    };

    Ok(TreasuryHealth {
        pnl_usd,
        break_even,
        house_runway,
        pending_liabilities,
        hot_buffers,
        fee_series_7d,
        fee_series_30d,
        fee_margin_block: FeeMarginBlockStatus {
            enabled: hard_block_enabled(),
            blocked_coins: blocked,
        },
        unswept_deposits: unswept,
    })
}

async fn fee_series(
    pool: &PgPool,
    prices: &HashMap<Coin, u128>,
    price_decimals: u32,
    days: i64,
) -> Result<Vec<FeeSeriesDay>, TreasuryHealthError> {
    // Build day buckets for earned (gateway+wd+swap) and network
    let since = Utc::now() - ChronoDuration::days(days);

    let mut earned_by_day: BTreeMap<String, BTreeMap<String, i128>> = BTreeMap::new();
    let mut network_by_day: BTreeMap<String, BTreeMap<String, i128>> = BTreeMap::new();

    let gw = sqlx::query(
        "SELECT (paid_at::date)::text as day, coin::text as coin, COALESCE(SUM(fee_amount),0)::text as fees \
         FROM merchant_deposit_invoices \
         WHERE status='CONFIRMED' AND paid_at >= $1 \
         GROUP BY paid_at::date, coin",
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for r in gw {
        let day: String = r.get("day");
        let coin: String = r.get("coin");
        *earned_by_day.entry(day).or_default().entry(coin).or_default() += parse_i128(&r.get::<String, _>("fees"));
    }

    let wd = sqlx::query(
        "SELECT (w.created_at::date)::text as day, wa.coin::text as coin, COALESCE(SUM(w.fee_amount),0)::text as fees \
         FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id \
         WHERE w.status IN ('BROADCASTED','CONFIRMED') AND w.created_at >= $1 \
         GROUP BY w.created_at::date, wa.coin",
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for r in wd {
        let day: String = r.get("day");
        let coin: String = r.get("coin");
        *earned_by_day.entry(day).or_default().entry(coin).or_default() += parse_i128(&r.get::<String, _>("fees"));
    }

    let sw = sqlx::query(
        "SELECT (created_at::date)::text as day, from_coin::text as coin, COALESCE(SUM(platform_fee_amount),0)::text as fees \
         FROM dex_swaps WHERE status='COMPLETED' AND created_at >= $1 \
         GROUP BY created_at::date, from_coin",
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for r in sw {
        let day: String = r.get("day");
        let coin: String = r.get("coin");
        *earned_by_day.entry(day).or_default().entry(coin).or_default() += parse_i128(&r.get::<String, _>("fees"));
    }

    let net = sqlx::query(
        "SELECT (created_at::date)::text as day, coin::text as coin, COALESCE(SUM(amount),0)::text as amount \
         FROM network_fee_events WHERE created_at >= $1 \
         GROUP BY created_at::date, coin",
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for r in net {
        let day: String = r.get("day");
        let coin: String = r.get("coin");
        *network_by_day.entry(day).or_default().entry(coin).or_default() += parse_i128(&r.get::<String, _>("amount"));
    }

    let mut days_set: BTreeMap<String, ()> = BTreeMap::new();
    for d in earned_by_day.keys() {
        days_set.insert(d.clone(), ());
    }
    for d in network_by_day.keys() {
        days_set.insert(d.clone(), ());
    }

    let mut out = Vec::new();
    for day in days_set.keys() {
        let e_map = earned_by_day.get(day).cloned().unwrap_or_default();
        let n_map = network_by_day.get(day).cloned().unwrap_or_default();
        let mut e_tot = 0i128;
        let mut n_tot = 0i128;
        let mut e_usd = 0u128;
        let mut n_usd = 0u128;
        let mut coins: std::collections::BTreeSet<String> = e_map.keys().cloned().collect();
        coins.extend(n_map.keys().cloned());
        for c in coins {
            let ev = *e_map.get(&c).unwrap_or(&0);
            let nv = *n_map.get(&c).unwrap_or(&0);
            e_tot += ev;
            n_tot += nv;
            if let Ok(coin) = c.parse::<Coin>() {
                e_usd += usd_of(prices, coin, ev, price_decimals);
                n_usd += usd_of(prices, coin, nv, price_decimals);
            }
        }
        out.push(FeeSeriesDay {
            day: day.clone(),
            fees_earned: i128_str(e_tot),
            network_paid: i128_str(n_tot),
            fees_earned_usd: e_usd.to_string(),
            network_paid_usd: n_usd.to_string(),
        });
    }
    Ok(out)
}
