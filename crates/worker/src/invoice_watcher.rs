//! Closes the merchant gateway loop for on-chain payments.
//!
//! Until this job existed, `db::merchant_deposits::confirm_invoice` had no
//! production caller: an invoice paid on-chain stayed PENDING forever, the
//! merchant was never credited and the `deposit.confirmed` webhook never
//! fired. The only working path was "pay with SatsPay balance" on the hosted
//! checkout.
//!
//! Each tick:
//! 1. expires invoices whose window closed;
//! 2. polls every open invoice's dedicated address, records what arrived,
//!    and confirms once the full amount has `min_confirmations`;
//! 3. sweeps the confirmed address into the hot wallet;
//! 4. redelivers webhooks whose backoff has elapsed.
//!
//! Amounts from `ChainClient::fetch_deposits` are already in ledger units
//! (1e-8) — see `chain::rpc_client` — so they compare directly against
//! `invoice.amount`.

use bigdecimal::BigDecimal;
use chain::ChainRegistry;
use crypto::SecretsService;
use db::merchant_deposits::MerchantDepositInvoice;
use shared::{coin_config, Coin};
use sqlx::PgPool;

/// Invoices inspected per tick.
const SCAN_BATCH: i64 = 200;
/// Webhook redeliveries attempted per tick.
const RETRY_BATCH: i64 = 50;

pub async fn run_once(pool: &PgPool, registry: &ChainRegistry, secrets: &SecretsService) {
    match db::merchant_deposits::expire_due_invoices(pool).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(expired = n, "invoice_watcher: expired overdue invoices"),
        Err(e) => tracing::error!(error = %e, "invoice_watcher: failed to expire invoices"),
    }

    let invoices = match db::merchant_deposits::list_open_invoices(pool, SCAN_BATCH).await {
        Ok(list) => list,
        Err(e) => {
            tracing::error!(error = %e, "invoice_watcher: failed to list open invoices");
            return;
        }
    };

    for inv in invoices {
        scan_invoice(pool, registry, secrets, &inv).await;
    }

    redeliver_webhooks(pool, secrets).await;
}

async fn scan_invoice(
    pool: &PgPool,
    registry: &ChainRegistry,
    secrets: &SecretsService,
    inv: &MerchantDepositInvoice,
) {
    let Some(coin) = parse_coin(&inv.coin) else {
        tracing::error!(invoice_id = %inv.id, coin = %inv.coin, "invoice_watcher: unknown coin on invoice");
        return;
    };
    let client = registry.get(coin);

    let deposits = match client.fetch_deposits(&inv.deposit_address).await {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(invoice_id = %inv.id, coin = %inv.coin, error = %e, "invoice_watcher: fetch_deposits failed");
            return;
        }
    };
    if deposits.is_empty() {
        return;
    }

    let min_confs = coin_config(coin).min_confirmations;
    let mut seen = BigDecimal::from(0);
    let mut confirmed = BigDecimal::from(0);
    let mut best_confs: u32 = 0;
    let mut tx_hash: Option<String> = None;

    for tx in &deposits {
        let amount = BigDecimal::from(tx.amount);
        seen += &amount;
        if tx.confirmations >= min_confs {
            confirmed += &amount;
        }
        if tx.confirmations >= best_confs {
            best_confs = tx.confirmations;
        }
        if tx_hash.is_none() {
            tx_hash = Some(tx.tx_hash.clone());
        }
    }

    // Record what the chain shows before deciding — an underpayment or a
    // still-shallow payment must be visible to the merchant instead of
    // looking like "nothing arrived".
    if let Err(e) = db::merchant_deposits::mark_invoice_detected(
        pool,
        inv.id,
        seen.clone(),
        best_confs.min(i32::MAX as u32) as i32,
        tx_hash.as_deref(),
    )
    .await
    {
        tracing::error!(invoice_id = %inv.id, error = %e, "invoice_watcher: failed to record detection");
        return;
    }

    if confirmed < inv.amount {
        tracing::debug!(
            invoice_id = %inv.id,
            coin = %inv.coin,
            confirmations = best_confs,
            min_confirmations = min_confs,
            seen = %seen,
            expected = %inv.amount,
            "invoice_watcher: payment not yet complete"
        );
        return;
    }

    // `confirm_invoice` credits the merchant ledger under an idempotent
    // reference key and is a no-op on an already CONFIRMED invoice, so a
    // repeated tick cannot double-credit.
    let confirmed_inv = match db::merchant_deposits::confirm_invoice(pool, inv.id, tx_hash.as_deref()).await {
        Ok(i) => i,
        Err(e) => {
            tracing::error!(invoice_id = %inv.id, error = %e, "invoice_watcher: confirm_invoice failed");
            return;
        }
    };

    tracing::info!(
        invoice_id = %inv.id,
        merchant_id = %inv.merchant_id,
        coin = %inv.coin,
        tx_hash = ?tx_hash,
        confirmations = best_confs,
        amount = %inv.amount,
        net_amount = %inv.net_amount,
        "invoice_watcher: invoice confirmed on-chain"
    );

    sweep_invoice_address(pool, registry, inv, coin).await;
    webhooks::dispatch_invoice_webhook(pool, &confirmed_inv, secrets).await;
}

/// Moves the invoice's funds to the hot wallet, mirroring `deposit_watcher`.
/// `hd_index` is cleared afterwards so a later tick does not re-broadcast.
async fn sweep_invoice_address(pool: &PgPool, registry: &ChainRegistry, inv: &MerchantDepositInvoice, coin: Coin) {
    let Some(hd_index) = inv.hd_index else { return };
    let Ok(idx) = u32::try_from(hd_index) else { return };

    match registry.get(coin).sweep_deposit_to_hot(idx).await {
        Ok(Some(tx)) => {
            tracing::info!(invoice_id = %inv.id, coin = %inv.coin, tx_hash = %tx.tx_hash, "invoice_watcher: swept invoice address to hot wallet");
            if let Err(e) = db::network_fees::record_network_fee(
                pool,
                db::network_fees::RecordNetworkFeeInput {
                    coin,
                    kind: db::network_fees::NetworkFeeKind::Sweep,
                    amount: tx.fee_amount,
                    tx_hash: Some(&tx.tx_hash),
                    reference_id: Some(inv.id),
                    reference_type: Some("MerchantInvoiceSweep"),
                },
            )
            .await
            {
                tracing::warn!(invoice_id = %inv.id, error = %e, "invoice_watcher: failed to record sweep network fee");
            }
            if let Err(e) = db::merchant_deposits::mark_invoice_swept(pool, inv.id).await {
                tracing::warn!(invoice_id = %inv.id, error = %e, "invoice_watcher: failed to clear hd_index after sweep");
            }
        }
        Ok(None) => {}
        Err(e) => tracing::warn!(invoice_id = %inv.id, coin = %inv.coin, error = %e.message, "invoice_watcher: sweep failed"),
    }
}

async fn redeliver_webhooks(pool: &PgPool, secrets: &SecretsService) {
    let due = match db::merchant_deposits::list_webhook_retries(pool, webhooks::MAX_WEBHOOK_ATTEMPTS, RETRY_BATCH).await {
        Ok(list) => list,
        Err(e) => {
            tracing::error!(error = %e, "invoice_watcher: failed to list webhook retries");
            return;
        }
    };

    for inv in due {
        let outcome = webhooks::dispatch_invoice_webhook(pool, &inv, secrets).await;
        if !outcome.delivered && inv.webhook_attempts + 1 >= webhooks::MAX_WEBHOOK_ATTEMPTS {
            // Terminal: the merchant will not learn about this payment from
            // us. Surfaced as an error record so it shows up on the error
            // dashboard instead of only in logs.
            let payload = db::telemetry::NewErrorPayload {
                service: "worker".to_string(),
                level: "ERROR".to_string(),
                message: format!("WEBHOOK_DELIVERY_EXHAUSTED invoice={} merchant={}", inv.id, inv.merchant_id),
                stack_trace: None,
                endpoint: None,
                method: Some("POST".to_string()),
                status_code: outcome.status_code,
                user_id: Some(inv.merchant_id),
                ip_address: None,
                request_payload: None,
                user_agent: None,
            };
            if let Err(e) = db::telemetry::record_error(pool, payload).await {
                tracing::error!(invoice_id = %inv.id, error = %e, "invoice_watcher: failed to record exhausted webhook");
            }
        }
    }
}

fn parse_coin(raw: &str) -> Option<Coin> {
    shared::COINS.into_iter().find(|c| c.as_str() == raw)
}
