//! Worker entrypoint — paridade com `legacy/apps/api/src/worker.ts`.
//! Roda jobs internos (deposit_watcher, invoice_watcher, withdrawal_reconciler) e o
//! outbox_relay que publica eventos de domínio no Kafka.
//!
//! TODO(fase 5+): balance_reconciliation, lend_accrual, worker_heartbeat.

mod deposit_watcher;
mod dex_swap_runner;
mod invoice_watcher;
mod withdrawal_reconciler;

use chain::ChainRegistry;
use events::EventProducer;
use std::sync::Arc;
use std::time::Duration;
use swapkit::SwapKitClient;

fn required_env_secs(name: &str) -> Duration {
    let secs: u64 = std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set")).parse().unwrap_or_else(|_| panic!("{name} must be a positive integer number of seconds"));
    Duration::from_secs(secs)
}

fn required_env_millis(name: &str) -> Duration {
    let ms: u64 = std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set")).parse().unwrap_or_else(|_| panic!("{name} must be a positive integer number of milliseconds"));
    Duration::from_millis(ms)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    db::telemetry::install_panic_hook("worker");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("failed to connect to Postgres");
    db::run_migrations(&pool).await.expect("failed to run migrations");
    db::house::ensure_house_inventory(&pool).await.expect("failed to ensure house inventory");
    db::lend::ensure_lend_reserves(&pool).await.expect("failed to ensure lend reserves");

    let registry = std::sync::Arc::new(ChainRegistry::from_env(pool.clone()).expect("failed to build chain registry"));
    let swapkit = Arc::new(SwapKitClient::from_env());
    let worker_id = std::env::var("HOSTNAME").unwrap_or_else(|_| format!("worker-{}", uuid::Uuid::new_v4()));

    let kafka_bootstrap = std::env::var("KAFKA_BOOTSTRAP_SERVERS").ok();
    let outbox_batch_size: i64 = std::env::var("OUTBOX_RELAY_BATCH_SIZE").expect("OUTBOX_RELAY_BATCH_SIZE must be set").parse().expect("OUTBOX_RELAY_BATCH_SIZE must be a positive integer");

    tracing::info!(worker_id, "worker started");

    let deposit_interval = required_env_secs("DEPOSIT_WATCHER_INTERVAL_SECS");
    let deposit_pool = pool.clone();
    let deposit_registry = registry.clone();
    let deposit_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(deposit_interval);
        loop {
            interval.tick().await;
            deposit_watcher::run_once(&deposit_pool, &deposit_registry).await;
        }
    });

    // Same HKDF-derived per-merchant webhook key the API signs with — both
    // sides derive it from ENCRYPTION_KEY, nothing is stored.
    let encryption_key = std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set (64 hex chars)");
    let secrets = Arc::new(
        crypto::SecretsService::from_hex(&encryption_key).expect("ENCRYPTION_KEY must be 64 hex chars"),
    );

    // Merchant gateway: confirm on-chain invoice payments, sweep, and retry
    // webhooks. Optional interval with a default so an existing deployment
    // does not fail to boot on a missing variable.
    let invoice_interval = Duration::from_secs(
        std::env::var("INVOICE_WATCHER_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|s| *s > 0)
            .unwrap_or(20),
    );
    let invoice_pool = pool.clone();
    let invoice_registry = registry.clone();
    let invoice_secrets = secrets.clone();
    let invoice_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(invoice_interval);
        loop {
            interval.tick().await;
            invoice_watcher::run_once(&invoice_pool, &invoice_registry, &invoice_secrets).await;
        }
    });

    let broadcast_interval_dur = required_env_secs("WITHDRAWAL_BROADCAST_INTERVAL_SECS");
    let requeue_interval_dur = required_env_secs("WITHDRAWAL_REQUEUE_INTERVAL_SECS");
    let withdrawal_pool = pool.clone();
    let withdrawal_registry = registry.clone();
    let withdrawal_worker_id = worker_id.clone();
    let withdrawal_task = tokio::spawn(async move {
        let mut broadcast_interval = tokio::time::interval(broadcast_interval_dur);
        let mut requeue_interval = tokio::time::interval(requeue_interval_dur);
        loop {
            tokio::select! {
                _ = broadcast_interval.tick() => {
                    withdrawal_reconciler::drain_broadcast_queue(&withdrawal_pool, &withdrawal_registry, &withdrawal_worker_id).await;
                }
                _ = requeue_interval.tick() => {
                    withdrawal_reconciler::requeue_eligible(&withdrawal_pool).await;
                }
            }
        }
    });

    let dex_pool = pool.clone();
    let dex_registry = registry.clone();
    let dex_swapkit = swapkit.clone();
    let dex_worker_id = worker_id.clone();
    let dex_interval = Duration::from_secs(
        std::env::var("DEX_SWAP_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(15),
    );
    let dex_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(dex_interval);
        loop {
            interval.tick().await;
            dex_swap_runner::drain_broadcast_queue(&dex_pool, &dex_registry, &dex_swapkit, &dex_worker_id).await;
            dex_swap_runner::track_inflight(&dex_pool, &dex_swapkit).await;
        }
    });

    let price_max_stale = required_env_secs("PRICE_MAX_STALE_SECS");
    let rewards_interval_dur = required_env_secs("REWARDS_TICK_INTERVAL_SECS");
    let rewards_pool = pool.clone();
    let rewards_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(rewards_interval_dur);
        loop {
            interval.tick().await;
            if let Err(e) = db::rewards::distribute_all_programs(&rewards_pool, price_max_stale).await {
                tracing::error!(error = %e, "rewards tick failed");
                db::telemetry::record_worker_error(&rewards_pool, "ERROR", "rewards_distribute", &e.to_string(), None).await;
            }
        }
    });

    let price_refresh_interval_dur = required_env_secs("PRICE_REFRESH_INTERVAL_SECS");
    let price_decimals: u32 = std::env::var("PRICE_DECIMALS").expect("PRICE_DECIMALS must be set").parse().expect("PRICE_DECIMALS must be a positive integer");
    let coingecko_base_url = std::env::var("COINGECKO_API_BASE_URL").expect("COINGECKO_API_BASE_URL must be set");
    let coingecko_timeout = required_env_secs("COINGECKO_TIMEOUT_SECS");
    let price_pool = pool.clone();
    let price_task = tokio::spawn(async move {
        let timeout = coingecko_timeout;
        let oracle = pricing::MultiProviderOracle::new(vec![
            Box::new(pricing::BinanceProvider::new(None, timeout)),
            Box::new(pricing::KrakenProvider::new(None, timeout)),
            Box::new(pricing::OkxProvider::new(None, timeout)),
            Box::new(pricing::CoinGeckoClient::new(coingecko_base_url, timeout)),
        ]);
        let mut interval = tokio::time::interval(price_refresh_interval_dur);
        loop {
            interval.tick().await;
            if let Err(e) = db::pricing::refresh_all(&price_pool, &oracle, price_decimals).await {
                tracing::error!(error = %e, "price refresh tick failed");
                db::telemetry::record_worker_error(&price_pool, "ERROR", "pricing_refresh", &e.to_string(), None).await;
            }
        }
    });

    let aave_pool = pool.clone();
    let aave_task = tokio::spawn(async move {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = db::aave_sync::sync_live_aave_rates(&aave_pool, &http).await {
                tracing::warn!(error = %e, "aave sync tick warning");
                db::telemetry::record_worker_error(&aave_pool, "WARN", "aave_sync", &e.to_string(), None).await;
            }
        }
    });

    let outbox_task = kafka_bootstrap.map(|bootstrap| {
        let outbox_pool = pool.clone();
        let outbox_interval_dur = required_env_millis("OUTBOX_RELAY_INTERVAL_MS");
        tokio::spawn(async move {
            let producer = match EventProducer::new(&bootstrap) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "outbox_relay: failed to create Kafka producer, relay disabled");
                    db::telemetry::record_worker_error(&outbox_pool, "CRITICAL", "outbox_relay_init", &e.to_string(), None).await;
                    return;
                }
            };
            let mut interval = tokio::time::interval(outbox_interval_dur);
            loop {
                interval.tick().await;
                match events::relay_once(&outbox_pool, &producer, outbox_batch_size).await {
                    Ok(n) if n > 0 => tracing::debug!(published = n, "outbox_relay: batch published"),
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!(error = %e, "outbox_relay: batch failed");
                        db::telemetry::record_worker_error(&outbox_pool, "ERROR", "outbox_relay", &e.to_string(), None).await;
                    }
                }
            }
        })
    });

    let metrics_pool = pool.clone();
    let metrics_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = db::telemetry::capture_metrics_snapshot(&metrics_pool).await {
                tracing::warn!(error = %e, "telemetry metrics snapshot capture failed");
                db::telemetry::record_worker_error(&metrics_pool, "WARN", "metrics_snapshot", &e.to_string(), None).await;
            }
        }
    });

    if outbox_task.is_none() {
        tracing::warn!("KAFKA_BOOTSTRAP_SERVERS not set — outbox_relay disabled, events will accumulate unpublished");
    }

    let _ = tokio::join!(
        deposit_task,
        invoice_task,
        withdrawal_task,
        dex_task,
        rewards_task,
        price_task,
        aave_task,
        metrics_task
    );
}
