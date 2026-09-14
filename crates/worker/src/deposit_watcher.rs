//! Port of legacy `apps/api/src/jobs/depositWatcher.ts`.
//! Polls every wallet with an assigned address for new/updated on-chain
//! deposits via the (currently stub) chain client, crediting confirmed ones.

use chain::ChainRegistry;
use shared::Coin;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

    for row in rows {
        let wallet_id: Uuid = row.get("id");
        let coin_str: String = row.get("coin");
        let address: String = row.get("address");
        let Some(coin) = parse_coin(&coin_str) else { continue };
        let client = registry.get(coin);

        let deposits = match client.fetch_deposits(&address).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(wallet_id = %wallet_id, error = %e, "deposit_watcher: fetch_deposits failed");
                continue;
            }
        };

        let mut credited = false;
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
                Ok(()) => credited = true,
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
                }
            }
        }
        let _ = credited;
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
}
