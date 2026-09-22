//! Custodial DEX swap worker: broadcast deposit → track SwapKit / Relay / ChangeNOW → credit/refund.

use chain::{ChainRegistry, EvmContractCall};
use changenow::ChangeNowClient;
use relay::RelayClient;
use serde::Deserialize;
use serde_json::Value;
use shared::Coin;
use sqlx::PgPool;
use swapkit::{parse_contract_call_tx, parse_human_to_ledger, SwapKitClient};
use uuid::Uuid;

#[derive(Deserialize)]
struct BroadcastPayload {
    #[serde(rename = "dexSwapId")]
    dex_swap_id: Uuid,
}

pub async fn drain_broadcast_queue(
    pool: &PgPool,
    registry: &ChainRegistry,
    swapkit: &SwapKitClient,
    relay: &RelayClient,
    changenow: &ChangeNowClient,
    locked_by: &str,
) {
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
            Ok(p) => process_broadcast(pool, registry, swapkit, relay, changenow, p.dex_swap_id).await,
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

fn is_relay_row(row: &db::dex_swap::DexSwapRow) -> bool {
    row.provider.eq_ignore_ascii_case("RELAY")
        || row
            .swap_payload
            .as_ref()
            .and_then(|p| p.get("provider"))
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("relay"))
            .unwrap_or(false)
}

fn is_changenow_row(row: &db::dex_swap::DexSwapRow) -> bool {
    row.provider.eq_ignore_ascii_case("CHANGENOW")
        || row
            .swap_payload
            .as_ref()
            .and_then(|p| p.get("provider"))
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("changenow"))
            .unwrap_or(false)
}

fn relay_is_bridge(payload: &Value) -> bool {
    payload
        .get("isBridge")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn relay_request_id(payload: &Value) -> Option<String> {
    payload
        .get("requestId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            payload
                .get("quoteId")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
}

async fn process_broadcast(
    pool: &PgPool,
    registry: &ChainRegistry,
    swapkit: &SwapKitClient,
    relay: &RelayClient,
    changenow: &ChangeNowClient,
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

    let deposit = row.deposit_address.clone().unwrap_or_default();
    let memo = row.inbound_memo.clone();
    let tx_hint = row.tx_hint.as_deref().unwrap_or("simpleTransfer");

    let client = registry.get(from_coin);

    if tx_hint == "solanaRelay" {
        let payload = row
            .swap_payload
            .as_ref()
            .ok_or_else(|| "missing swap_payload for solanaRelay".to_string())?;
        let solana_tx = payload
            .get("solanaTx")
            .ok_or_else(|| "swap_payload missing solanaTx".to_string())?;

        return match client.broadcast_solana_relay_tx(solana_tx).await {
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
                let _ = try_credit_relay(pool, relay, id).await;
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
        };
    }

    if tx_hint == "contractCall" {
        let payload = row
            .swap_payload
            .as_ref()
            .ok_or_else(|| "missing swap_payload for contractCall".to_string())?;
        let call = parse_contract_call_tx(payload)
            .ok_or_else(|| "swap_payload missing EVM tx for contractCall".to_string())?;

        if from_coin == Coin::Pol && call.value_wei > 0 {
            let expected = onchain_amount;
            let delta = call.value_wei.abs_diff(expected);
            if delta > expected / 1_000_000 && delta > 1_000_000_000 {
                let _ = db::dex_swap::mark_failed(
                    pool,
                    id,
                    &format!("contractCall value mismatch: quote={} expected={}", call.value_wei, expected),
                )
                .await;
                let _ = db::dex_swap::refund(pool, id, "contractCall value mismatch — refunded").await;
                return Err("contractCall value mismatch".into());
            }
        }

        let approval = call
            .approval_address
            .clone()
            .or_else(|| {
                payload
                    .get("approvalAddress")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });

        let evm_call = EvmContractCall {
            to: call.to,
            data_hex: call.data,
            value_wei: call.value_wei,
            gas_limit_hint: call.gas_limit,
            gas_price_hint: call.gas_price_wei,
            from_hint: call.from,
            erc20_sell_amount: match from_coin {
                Coin::Usdt | Coin::Usdc | Coin::Pepe => Some(onchain_amount),
                _ => None,
            },
            approval_address: approval,
        };

        return match client.broadcast_evm_contract_call(evm_call).await {
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

                if is_relay_row(&row) {
                    if relay_is_bridge(payload) {
                        let _ = try_credit_relay(pool, relay, id).await;
                    } else if try_track_and_credit(pool, swapkit, id).await.is_err() {
                        if let Err(e) = credit_same_chain_after_receipt(pool, id, &res.tx_hash).await {
                            tracing::debug!(dex_swap_id = %id, error = %e, "dex_swap: same-chain credit fallback pending");
                        }
                    }
                } else if try_track_and_credit(pool, swapkit, id).await.is_err() {
                    if let Err(e) = credit_same_chain_after_receipt(pool, id, &res.tx_hash).await {
                        tracing::debug!(dex_swap_id = %id, error = %e, "dex_swap: same-chain credit fallback pending");
                    }
                }
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
        };
    }

    if deposit.is_empty() {
        return Err("missing deposit address".into());
    }

    if memo.as_ref().map(|m| !m.is_empty()).unwrap_or(false) && tx_hint == "transferWithMemo" {
        let _ = db::dex_swap::mark_failed(pool, id, "transferWithMemo not yet supported by hot wallet").await;
        let _ = db::dex_swap::refund(pool, id, "memo transfer unsupported — refunded").await;
        return Err("transferWithMemo not supported".into());
    }

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
            if is_changenow_row(&row) {
                let _ = try_credit_changenow(pool, changenow, id).await;
            } else {
                let _ = try_track_and_credit(pool, swapkit, id).await;
            }
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

pub async fn track_inflight(
    pool: &PgPool,
    swapkit: &SwapKitClient,
    relay: &RelayClient,
    changenow: &ChangeNowClient,
) {
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
                if is_changenow_row(&row) {
                    if let Err(e) = try_credit_changenow(pool, changenow, row.id).await {
                        tracing::debug!(swap_id = %row.id, error = %e, "dex_swap: changenow status pending");
                    }
                } else if is_relay_row(&row) {
                    if let Err(e) = try_credit_relay(pool, relay, row.id).await {
                        tracing::debug!(swap_id = %row.id, error = %e, "dex_swap: relay status pending");
                    }
                } else if let Err(e) = try_track_and_credit(pool, swapkit, row.id).await {
                    tracing::debug!(swap_id = %row.id, error = %e, "dex_swap: track pending");
                }
                if let Some(eta) = row.eta_seconds {
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
                if row.inbound_tx.is_none() {
                    if let Err(e) =
                        queue::enqueue(pool, "dex_swap_broadcast", &serde_json::json!({ "dexSwapId": row.id })).await
                    {
                        tracing::warn!(swap_id = %row.id, error = %e, "dex_swap: requeue failed");
                    }
                }
            }
            "FAILED" => {
                if let Err(e) = db::dex_swap::refund(pool, row.id, row.error.as_deref().unwrap_or("failed")).await {
                    tracing::debug!(swap_id = %row.id, error = %e, "dex_swap: refund skip");
                }
            }
            _ => {}
        }
    }
}

async fn try_credit_changenow(
    pool: &PgPool,
    changenow: &ChangeNowClient,
    id: Uuid,
) -> Result<(), String> {
    if !changenow.is_configured() {
        return Err("changenow not configured".into());
    }
    let row = db::dex_swap::get_by_id(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "not found".to_string())?;
    if row.status != "IN_FLIGHT" && row.status != "CREDITING" {
        return Ok(());
    }
    let payload = row
        .swap_payload
        .as_ref()
        .ok_or_else(|| "missing swap_payload".to_string())?;
    let exchange_id = payload
        .get("exchangeId")
        .and_then(|v| v.as_str())
        .or_else(|| row.quote_id.as_deref())
        .ok_or_else(|| "missing exchangeId".to_string())?;

    let status = changenow
        .status(exchange_id)
        .await
        .map_err(|e| e.to_string())?;

    if status.is_failed() {
        let _ = db::dex_swap::mark_failed(pool, id, &format!("changenow status: {}", status.status)).await;
        return Err(format!("changenow failed: {}", status.status));
    }
    if !status.is_success() {
        return Err(format!("changenow status={}", status.status));
    }

    let to_coin: Coin = row.to_coin.parse().map_err(|_| "bad to_coin")?;
    let to_amount = status
        .to_amount_human
        .as_deref()
        .and_then(|h| changenow::human_to_ledger(h, to_coin))
        .or_else(|| row.expected_to_amount.parse().ok())
        .ok_or_else(|| "cannot resolve changenow to_amount".to_string())?;

    if let Some(min) = row.min_to_amount.as_ref().and_then(|s| s.parse::<u128>().ok()) {
        if to_amount < min {
            let _ = db::dex_swap::mark_failed(pool, id, "slippage: received below minToAmount").await;
            return Err("slippage".into());
        }
    }

    db::dex_swap::credit_and_complete(pool, id, to_amount, status.payout_hash.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn try_credit_relay(pool: &PgPool, relay: &RelayClient, id: Uuid) -> Result<(), String> {
    if !relay.is_configured() {
        return Err("relay not configured".into());
    }
    let row = db::dex_swap::get_by_id(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "not found".to_string())?;
    if row.status != "IN_FLIGHT" && row.status != "CREDITING" {
        return Ok(());
    }
    let payload = row
        .swap_payload
        .as_ref()
        .ok_or_else(|| "missing swap_payload".to_string())?;
    let request_id = relay_request_id(payload).ok_or_else(|| "missing requestId".to_string())?;

    let status = relay
        .intent_status(&request_id)
        .await
        .map_err(|e| e.to_string())?;

    if status.is_failed() {
        let msg = match status.fail_reason() {
            Some(reason) => format!("relay status: {} ({reason})", status.status),
            None => format!("relay status: {}", status.status),
        };
        let _ = db::dex_swap::mark_failed(pool, id, &msg).await;
        return Err(format!("relay failed: {}", status.status));
    }
    if !status.is_success() {
        // Same-chain Relay may settle without intent flip; allow receipt fallback only when not bridge.
        if !relay_is_bridge(payload) {
            if let Some(inbound) = row.inbound_tx.as_deref() {
                let _ = credit_same_chain_after_receipt(pool, id, inbound).await;
            }
        }
        return Err(format!("relay status={}", status.status));
    }

    let expected: u128 = row
        .expected_to_amount
        .parse()
        .map_err(|_| "bad expected_to_amount".to_string())?;
    if let Some(min) = row.min_to_amount.as_ref().and_then(|s| s.parse::<u128>().ok()) {
        if expected < min {
            let _ = db::dex_swap::mark_failed(pool, id, "slippage: expected below minToAmount").await;
            return Err("slippage".into());
        }
    }
    db::dex_swap::credit_and_complete(pool, id, expected, status.outbound_tx.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn try_track_and_credit(pool: &PgPool, swapkit: &SwapKitClient, id: Uuid) -> Result<(), String> {
    let row = db::dex_swap::get_by_id(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "not found".to_string())?;

    let hash = row
        .inbound_tx
        .as_deref()
        .ok_or_else(|| "no inbound tx".to_string())?;
    let track = swapkit.track(hash).await.map_err(|e| e.to_string())?;
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
            return Err("slippage".into());
        }
    }

    db::dex_swap::credit_and_complete(pool, id, to_amount, track.outbound_tx().as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// After a confirmed same-chain contractCall receipt, credit `expected_to_amount`
/// when SwapKit track has not yet flipped to COMPLETE.
async fn credit_same_chain_after_receipt(pool: &PgPool, id: Uuid, inbound_tx: &str) -> Result<(), String> {
    let row = db::dex_swap::get_by_id(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "not found".to_string())?;
    if row.status != "IN_FLIGHT" {
        return Ok(());
    }
    let from: Coin = row.from_coin.parse().map_err(|_| "bad from")?;
    let to: Coin = row.to_coin.parse().map_err(|_| "bad to")?;
    let same = swapkit::is_same_chain(from, to)
        || row.providers.iter().any(|p| {
            let u = p.to_uppercase();
            u.contains("ONEINCH")
                || u.contains("1INCH")
                || u.contains("UNISWAP")
                || u.contains("SUSHISWAP")
                || u.contains("KYBERSWAP")
                || u.contains("PANCAKE")
                || u.contains("JUPITER")
                || u.contains("RELAY")
        });
    if !same {
        return Err("not same-chain".into());
    }
    // Never same-chain-credit a Relay bridge.
    if row
        .swap_payload
        .as_ref()
        .map(relay_is_bridge)
        .unwrap_or(false)
    {
        return Err("relay bridge".into());
    }
    let expected: u128 = row
        .expected_to_amount
        .parse()
        .map_err(|_| "bad expected_to_amount".to_string())?;
    if let Some(min) = row.min_to_amount.as_ref().and_then(|s| s.parse::<u128>().ok()) {
        if expected < min {
            return Err("expected below min".into());
        }
    }
    db::dex_swap::credit_and_complete(pool, id, expected, Some(inbound_tx))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
