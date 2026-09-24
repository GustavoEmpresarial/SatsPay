//! ADR 0012: keeps pre-derived deposit addresses available for coins the
//! keyless api-server cannot derive (SOL / ed25519). The api-server claims
//! rows from `deposit_address_pool`; this job refills them.

use chain::ChainRegistry;
use shared::{Coin, COINS};
use sqlx::PgPool;

pub async fn run_once(pool: &PgPool, registry: &ChainRegistry, target: u32) {
    for &coin in COINS.iter() {
        if coin != Coin::Sol {
            continue;
        }
        match registry.get(coin).top_up_deposit_pool(target).await {
            Ok(0) => {}
            Ok(added) => tracing::info!(coin = %coin.as_str(), added, "deposit_pool: topped up"),
            Err(e) => {
                // SIGNER_NOT_AVAILABLE here means new SOL deposit addresses
                // cannot be issued until DEPOSIT_MNEMONIC reaches the worker.
                tracing::error!(code = "DEPOSIT_POOL_TOPUP_FAILED", coin = %coin.as_str(), error = %e.message, "deposit_pool: top-up failed");
                db::telemetry::record_worker_error(pool, "ERROR", "deposit_pool_topup", &e.message, None).await;
            }
        }
    }
}
