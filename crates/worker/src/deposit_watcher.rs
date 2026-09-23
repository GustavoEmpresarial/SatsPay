//! Port of legacy `apps/api/src/jobs/depositWatcher.ts`.
//! Polls every wallet with an assigned address for new/updated on-chain
//! deposits via the (currently stub) chain client, crediting confirmed ones.

use chain::ChainRegistry;
use shared::Coin;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

static FAILURE_STREAKS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

fn streak(key: &str, failed: bool) -> u32 {
    let mut all = FAILURE_STREAKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let value = all.entry(key.to_owned()).or_default();
    if failed {
        *value = value.saturating_add(1);
    } else {
        *value = 0;
    }
    *value
}

pub async fn run_once(pool: &PgPool, registry: &ChainRegistry) {
    let rows = match sqlx::query("SELECT id, user_id, coin::text as coin, address, hd_index FROM wallets WHERE kind = 'PERSONAL' AND address IS NOT NULL")
        .fetch_all(pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error = %e, "deposit_watcher: failed to list wallets");
            return;
        }
    };

    let mut provider_failures = HashSet::new();
    let mut sweep_failures = HashSet::new();
    let mut seen_coins = HashSet::new();
    for row in rows {
        let wallet_id: Uuid = row.get("id");
        let coin_str: String = row.get("coin");
        let address: String = row.get("address");
        let Some(coin) = parse_coin(&coin_str) else {
            continue;
        };
        seen_coins.insert(coin_str.clone());
        let client = registry.get(coin);

        // A failed history lookup must not skip the sweep. USDT often fails
        // the indexer while the tokens are already on the HD address.
        let deposits = match client.fetch_deposits(&address).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(wallet_id = %wallet_id, error = %e, "deposit_watcher: fetch_deposits failed; still attempting sweep");
                provider_failures.insert(coin_str.clone());
                Vec::new()
            }
        };

        let mut credited = false;
        let user_id: Uuid = row.get("user_id");
        for onchain_tx in deposits {
            let amount = bigdecimal::BigDecimal::from(onchain_tx.amount);
            match db::deposits::credit_deposit(
                pool,
                wallet_id,
                coin,
                &onchain_tx.tx_hash,
                onchain_tx.vout as i32,
                amount,
                onchain_tx.confirmations as i32,
            )
            .await
            {
                Ok(db::deposits::CreditDepositOutcome::NewlyCredited) => {
                    credited = true;
                    if let Err(e) = db::airdrop::award_airdrop_points(
                        pool,
                        user_id,
                        100,
                        0,
                        "DEPOSIT_CONFIRMED",
                    )
                    .await
                    {
                        tracing::warn!(
                            %user_id,
                            %wallet_id,
                            tx_hash = %onchain_tx.tx_hash,
                            error = %e,
                            "deposit_watcher: airdrop DEPOSIT_CONFIRMED award failed"
                        );
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!(wallet_id = %wallet_id, tx_hash = %onchain_tx.tx_hash, error = %e, "deposit_watcher: credit_deposit failed");
                }
            }
        }

        let hd_index: Option<i64> = row.get("hd_index");
        if let Some(idx) = hd_index {
            match client.sweep_deposit_to_hot(idx as u32).await {
                Ok(Some(tx)) => {
                    tracing::info!(wallet_id = %wallet_id, coin = %coin_str, tx_hash = %tx.tx_hash, "swept deposit to hot wallet");
                    if let Err(e) = db::network_fees::record_network_fee(
                        pool,
                        db::network_fees::RecordNetworkFeeInput {
                            coin,
                            kind: db::network_fees::NetworkFeeKind::Sweep,
                            amount: tx.fee_amount,
                            tx_hash: Some(&tx.tx_hash),
                            reference_id: Some(wallet_id),
                            reference_type: Some("WalletSweep"),
                        },
                    )
                    .await
                    {
                        tracing::warn!(wallet_id = %wallet_id, error = %e, "failed to record sweep network fee");
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(wallet_id = %wallet_id, coin = %coin_str, error = %e, "sweep to hot failed (will retry)");
                    sweep_failures.insert(coin_str.clone());
                }
            }
        }
        let _ = credited;
    }
    for coin in seen_coins {
        let provider_key = format!("provider_{coin}");
        let provider_streak = streak(&provider_key, provider_failures.contains(&coin));
        if provider_streak >= 3 {
            db::telemetry::record_worker_error(
                pool,
                "CRITICAL",
                &provider_key,
                &format!("CHAIN_PROVIDERS_UNAVAILABLE {coin}"),
                None,
            )
            .await;
        } else if provider_streak == 0 {
            let _ = db::telemetry::resolve_worker_alert(pool, &provider_key).await;
        }
        let sweep_key = format!("sweep_{coin}");
        let sweep_streak = streak(&sweep_key, sweep_failures.contains(&coin));
        if sweep_streak >= 3 {
            db::telemetry::record_worker_error(
                pool,
                "CRITICAL",
                &sweep_key,
                &format!("SWEEP_REPEATED_FAILURE {coin}"),
                None,
            )
            .await;
        } else if sweep_streak == 0 {
            let _ = db::telemetry::resolve_worker_alert(pool, &sweep_key).await;
        }
    }
}

fn parse_coin(s: &str) -> Option<Coin> {
    shared::COINS.into_iter().find(|c| c.as_str() == s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Coin;

    #[test]
    fn parse_coin_known_and_unknown() {
        assert_eq!(parse_coin("BTC"), Some(Coin::Btc));
        assert_eq!(parse_coin("LTC"), Some(Coin::Ltc));
        assert!(parse_coin("NOPE").is_none());
        assert!(parse_coin("").is_none());
    }

    #[test]
    fn consecutive_failures_reset_after_success() {
        let key = format!("test-{}", Uuid::new_v4());
        assert_eq!(streak(&key, true), 1);
        assert_eq!(streak(&key, true), 2);
        assert_eq!(streak(&key, false), 0);
        assert_eq!(streak(&key, true), 1);
    }
}
