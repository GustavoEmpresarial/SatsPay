//! Custodial DEX swap worker: broadcast deposit → track SwapKit → credit/refund.

use chain::ChainRegistry;
use serde::Deserialize;
use shared::Coin;
use sqlx::PgPool;
use swapkit::{parse_human_to_ledger, SwapKitClient};
use uuid::Uuid;

#[derive(Deserialize)]
struct BroadcastPayload {
    #[serde(rename = "dexSwapId")]
    dex_swap_id: Uuid,
}

pub async fn drain_broadcast_queue(pool: &PgPool, registry: &ChainRegistry, swapkit: &SwapKitClient, locked_by: &str) {
    loop {
        let claimed = match queue::claim_next(pool, "dex_swap_broadcast", locked_by).await {
            Ok(Some(job)) => job,
            Ok(None) => break,
            Err(e) => {
                tracing::error!(error = %e, "dex_swap: failed to claim broadcast job");
                break;
            }
        };

        let outcome = match serde_json::from_value::<BroadcastPayload>(claimed.payload.clone()) {
            Ok(p) => process_broadcast(pool, registry, swapkit, p.dex_swap_id).await,
            Err(e) => Err(format!("malformed job payload: {e}")),
        };

        match outcome {
            Ok(()) => {
                if let Err(e) = queue::complete(pool, claimed.id).await {
                    tracing::error!(job_id = %claimed.id, error = %e, "dex_swap: complete failed");
                }
            }
            Err(err) => {
                tracing::error!(job_id = %claimed.id, error = %err, "dex_swap: broadcast job failed");
                if let Err(e) = queue::fail(pool, claimed.id, &err).await {
                    tracing::error!(job_id = %claimed.id, error = %e, "dex_swap: mark fail failed");
                }
            }
        }
    }
}

async fn process_broadcast(
    pool: &PgPool,
    registry: &ChainRegistry,
    swapkit: &SwapKitClient,
    id: Uuid,
) -> Result<(), String> {
    let row = db::dex_swap::get_by_id(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "dex swap not found".to_string())?;

    if matches!(row.status.as_str(), "COMPLETED" | "REFUNDED" | "IN_FLIGHT" | "CREDITING") {
        return Ok(());
    }

    let from_coin: Coin = row.from_coin.parse().map_err(|_| "bad from_coin")?;
    let from_amount: u128 = row.from_amount.parse().map_err(|_| "bad from_amount")?;
    let onchain_amount = shared::to_onchain_amount(from_coin, from_amount);

    let deposit = row
        .deposit_address
        .clone()
        .ok_or_else(|| "missing deposit address".to_string())?;
    let memo = row.inbound_memo.clone();
    let tx_hint = row.tx_hint.as_deref().unwrap_or("simpleTransfer");

    if tx_hint == "contractCall" {
        let _ = db::dex_swap::mark_failed(pool, id, "contractCall routes require EVM/SOL calldata broadcast (phase 3)").await;
        let _ = db::dex_swap::refund(pool, id, "contractCall not yet supported — refunded").await;
        return Err("contractCall not supported yet".into());
    }

    if memo.as_ref().map(|m| !m.is_empty()).unwrap_or(false) && tx_hint == "transferWithMemo" {
        // Hot wallet UTXO path does not attach OP_RETURN yet — fail closed + refund.
        // Prefer simpleTransfer quotes in the API sort order.
        let _ = db::dex_swap::mark_failed(pool, id, "transferWithMemo not yet supported by hot wallet").await;
        let _ = db::dex_swap::refund(pool, id, "memo transfer unsupported — refunded").await;
        return Err("transferWithMemo not supported".into());
    }

    let client = registry.get(from_coin);

    match client.broadcast_withdrawal(&deposit, onchain_amount).await {
        Ok(res) => {
            db::dex_swap::mark_in_flight(pool, id, &res.tx_hash)
                .await
                .map_err(|e| e.to_string())?;
            if let Err(e) = db::network_fees::record_network_fee(
                pool,
                db::network_fees::RecordNetworkFeeInput {
                    coin: from_coin,
                    kind: db::network_fees::NetworkFeeKind::DexDeposit,
                    amount: res.fee_amount,
                    tx_hash: Some(&res.tx_hash),
                    reference_id: Some(id),
                    reference_type: Some("DexSwap"),
                },
            )
            .await
            {
                tracing::warn!(dex_swap_id = %id, error = %e, "failed to record dex deposit network fee");
            }
            // Best-effort immediate track; tracker loop will finish credit.
            let _ = try_track_and_credit(pool, swapkit, id).await;
            Ok(())
        }
        Err(e) => {
            if e.safe_to_reverse {
                let _ = db::dex_swap::mark_failed(pool, id, &e.message).await;
                let _ = db::dex_swap::refund(pool, id, &format!("broadcast failed: {}", e.message)).await;
            } else {
                let _ = db::dex_swap::mark_failed(pool, id, &format!("ambiguous broadcast: {}", e.message)).await;
            }
            Err(e.message)
        }
    }
}

pub async fn track_inflight(pool: &PgPool, swapkit: &SwapKitClient) {
    let rows = match db::dex_swap::list_active(pool, 50).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "dex_swap: list_active failed");
            return;
        }
    };

    for row in rows {
        match row.status.as_str() {
            "IN_FLIGHT" | "CREDITING" => {
                if let Err(e) = try_track_and_credit(pool, swapkit, row.id).await {
                    tracing::debug!(swap_id = %row.id, error = %e, "dex_swap: track pending");
                }
                // Timeout: ETA × 2 (min 2h)
                if let (Some(eta), Some(broadcast_hint)) = (row.eta_seconds, row.created_at.timestamp().checked_add(0)) {
                    let _ = broadcast_hint;
                    let max_wait = (eta.max(600) as i64).saturating_mul(2);
                    let age = (chrono::Utc::now() - row.updated_at).num_seconds();
                    if age > max_wait {
                        tracing::warn!(swap_id = %row.id, age, max_wait, "dex_swap: IN_FLIGHT past ETA×2");
                        let _ = db::telemetry::record_worker_error(
                            pool,
                            "WARN",
                            "dex_swap_timeout",
                            &format!("swap {} in-flight for {}s (eta*2={})", row.id, age, max_wait),
                            None,
                        )
                        .await;
                    }
                }
            }
            "LOCKED" | "BROADCASTING" => {
                // Re-enqueue if stuck without inbound tx.
                if row.inbound_tx.is_none() {
                    if let Err(e) = queue::enqueue(pool, "dex_swap_broadcast", &serde_json::json!({ "dexSwapId": row.id })).await {
                        tracing::warn!(swap_id = %row.id, error = %e, "dex_swap: requeue failed");
                    }
                }
            }
            "FAILED" => {
                // Attempt refund if not already refunded.
                if let Err(e) = db::dex_swap::refund(pool, row.id, row.error.as_deref().unwrap_or("failed")).await {
                    tracing::debug!(swap_id = %row.id, error = %e, "dex_swap: refund skip");
                }
            }
            _ => {}
        }
    }
}

async fn try_track_and_credit(pool: &PgPool, swapkit: &SwapKitClient, id: Uuid) -> Result<(), String> {
    let row = db::dex_swap::get_by_id(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "not found".to_string())?;

    let track_key = row
        .inbound_tx
        .as_deref()
        .or(row.deposit_address.as_deref())
        .ok_or_else(|| "no track key".to_string())?;

    let track = swapkit.track(track_key).await.map_err(|e| e.to_string())?;

    if track.is_failed() {
        let _ = db::dex_swap::mark_failed(pool, id, "provider reported failure").await;
        let _ = db::dex_swap::refund(pool, id, "provider failed — refunded").await;
        return Err("provider failed".into());
    }

    if !track.is_complete() {
        return Err("not complete".into());
    }

    let to_coin: Coin = row.to_coin.parse().map_err(|_| "bad to_coin")?;
    let to_amount = track
        .outbound_amount_human()
        .and_then(|h| parse_human_to_ledger(&h, to_coin))
        .or_else(|| row.expected_to_amount.parse().ok())
        .ok_or_else(|| "cannot resolve to_amount".to_string())?;

    if let Some(min) = row.min_to_amount.as_ref().and_then(|s| s.parse::<u128>().ok()) {
        if to_amount < min {
            let _ = db::dex_swap::mark_failed(pool, id, "slippage: received below minToAmount").await;
            // Funds already left hot wallet — do NOT auto-refund ledger fromCoin.
            return Err("slippage".into());
        }
    }

    db::dex_swap::credit_and_complete(pool, id, to_amount, track.outbound_tx().as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
