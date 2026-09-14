//! Aave V3 (Polygon / EVM) & Kamino (Solana) Live Market Rates Oracle & Sync Engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::Coin;
use sqlx::{PgPool, Row};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AaveMarketRate {
    pub coin: String,
    pub supply_apy_bps: u32,
    pub borrow_apy_bps: u32,
    pub utilization_bps: u32,
    pub collateral_factor_bps: u32,
    pub liquidation_threshold_bps: u32,
    pub total_liquidity_usd: f64,
    pub protocol: String,
    pub updated_at: DateTime<Utc>,
}

/// Fetches cached live Aave V3 market rates from PostgreSQL.
pub async fn get_aave_rates(pool: &PgPool) -> Result<HashMap<Coin, AaveMarketRate>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT coin, supply_apy_bps, borrow_apy_bps, utilization_bps, 
               collateral_factor_bps, liquidation_threshold_bps, 
               total_liquidity_usd::float8 as total_liquidity_usd, 
               protocol, updated_at
        FROM aave_market_rates
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut map = HashMap::new();
    for r in rows {
        let coin_str: String = r.get("coin");
        let Ok(coin) = coin_str.parse::<Coin>() else { continue };

        let rate = AaveMarketRate {
            coin: coin_str,
            supply_apy_bps: r.get::<i32, _>("supply_apy_bps") as u32,
            borrow_apy_bps: r.get::<i32, _>("borrow_apy_bps") as u32,
            utilization_bps: r.get::<i32, _>("utilization_bps") as u32,
            collateral_factor_bps: r.get::<i32, _>("collateral_factor_bps") as u32,
            liquidation_threshold_bps: r.get::<i32, _>("liquidation_threshold_bps") as u32,
            total_liquidity_usd: r.get::<f64, _>("total_liquidity_usd"),
            protocol: r.get("protocol"),
            updated_at: r.get("updated_at"),
        };
        map.insert(coin, rate);
    }

    Ok(map)
}

/// Sinks fresh Aave market rates into PostgreSQL cache.
pub async fn upsert_aave_rate(
    pool: &PgPool,
    coin: &str,
    supply_apy_bps: u32,
    borrow_apy_bps: u32,
    utilization_bps: u32,
    collateral_factor_bps: u32,
    liquidation_threshold_bps: u32,
    total_liquidity_usd: f64,
    protocol: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO aave_market_rates (
            coin, supply_apy_bps, borrow_apy_bps, utilization_bps, 
            collateral_factor_bps, liquidation_threshold_bps, 
            total_liquidity_usd, protocol, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        ON CONFLICT (coin) DO UPDATE SET
            supply_apy_bps = EXCLUDED.supply_apy_bps,
            borrow_apy_bps = EXCLUDED.borrow_apy_bps,
            utilization_bps = EXCLUDED.utilization_bps,
            collateral_factor_bps = EXCLUDED.collateral_factor_bps,
            liquidation_threshold_bps = EXCLUDED.liquidation_threshold_bps,
            total_liquidity_usd = EXCLUDED.total_liquidity_usd,
            protocol = EXCLUDED.protocol,
            updated_at = NOW()
        "#,
    )
    .bind(coin)
    .bind(supply_apy_bps as i32)
    .bind(borrow_apy_bps as i32)
    .bind(utilization_bps as i32)
    .bind(collateral_factor_bps as i32)
    .bind(liquidation_threshold_bps as i32)
    .bind(total_liquidity_usd)
    .bind(protocol)
    .execute(pool)
    .await?;

    Ok(())
}

/// Connects to AaveKit GraphQL API and updates the cache.
pub async fn sync_live_aave_rates(pool: &PgPool, http: &reqwest::Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sync_live_aave_rates_at(pool, http, "https://api.v3.aave.com/graphql").await
}

/// Same as [`sync_live_aave_rates`] but with an injectable GraphQL endpoint (tests / mirrors).
pub async fn sync_live_aave_rates_at(
    pool: &PgPool,
    http: &reqwest::Client,
    graphql_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let query = serde_json::json!({
        "query": r#"
            query GetAaveRates {
              markets(request: { chainIds: [137] }) {
                currency {
                  symbol
                  name
                  decimals
                }
                supplyApy
                variableBorrowApy
                utilizationRate
                totalLiquidity
              }
            }
        "#
    });

    let resp = http
        .post(graphql_url)
        .json(&query)
        .header("User-Agent", "SatsPay-DeFi-Sync/1.0")
        .send()
        .await;

    match resp {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(data) = res.json::<serde_json::Value>().await {
                    if let Some(markets) = data.get("data").and_then(|d| d.get("markets")).and_then(|m| m.as_array()) {
                        for item in markets {
                            let symbol = item.get("currency").and_then(|c| c.get("symbol")).and_then(|s| s.as_str()).unwrap_or("");
                            let supply_apy = item.get("supplyApy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let borrow_apy = item.get("variableBorrowApy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let util = item.get("utilizationRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let liq = item.get("totalLiquidity").and_then(|v| v.as_f64()).unwrap_or(0.0);

                            let coin_key = match symbol.to_uppercase().as_str() {
                                "USDT" => Some(("USDT", 8000, 8500)),
                                "USDC" => Some(("USDC", 8500, 9000)),
                                "WMATIC" | "POL" => Some(("POL", 7000, 7500)),
                                "WBTC" | "BTC" => Some(("BTC", 7500, 8000)),
                                _ => None,
                            };

                            if let Some((coin, ltv, liq_thresh)) = coin_key {
                                let supply_bps = (supply_apy * 100.0).round() as u32;
                                let borrow_bps = (borrow_apy * 100.0).round() as u32;
                                let util_bps = (util * 100.0).round() as u32;

                                let _ = upsert_aave_rate(
                                    pool,
                                    coin,
                                    supply_bps.max(100),
                                    borrow_bps.max(200),
                                    util_bps,
                                    ltv,
                                    liq_thresh,
                                    liq,
                                    "Aave V3",
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Aave V3 GraphQL sync tick warning: {}. Using PostgreSQL cached rates.", e);
        }
    }

    Ok(())
}
