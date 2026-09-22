//! Thin axum routes for withdrawals — port of legacy `withdrawals.controller.ts`.
//! Owns the auth/2FA gate (user must have 2FA enabled + a fresh WITHDRAWAL
//! OTP); `db::withdrawals` owns the ledger/state-machine invariants.

use crate::client_ip::ClientIp;
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bigdecimal::BigDecimal;
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;
use uuid::Uuid;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/withdrawals", post(request_withdrawal::<R>).get(list_withdrawals::<R>))
        .route("/v1/withdrawals/history", get(list_withdrawals::<R>))
        .route("/v1/withdrawals/addresses", get(list_addresses::<R>).post(add_address::<R>))
        .route("/v1/withdrawals/addresses/:id", delete(delete_address::<R>))
}

#[derive(Deserialize)]
struct WithdrawRequest {
    coin: String,
    #[serde(rename = "toAddress", alias = "address")]
    to_address: String,
    amount: String,
    #[serde(rename = "emailCode")]
    email_code: Option<String>,
    #[serde(rename = "totpCode")]
    totp_code: Option<String>,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: Option<String>,
    /// Only `PERSONAL` (the default) is accepted. Invoice net credits land on
    /// the MERCHANT wallet, which must be moved to PERSONAL before it can leave
    /// the platform on-chain.
    #[serde(rename = "walletKind", alias = "kind")]
    wallet_kind: Option<String>,
}

#[derive(Serialize)]
struct WithdrawResponse {
    id: String,
    status: String,
    #[serde(rename = "requiresApproval")]
    requires_approval: bool,
}

async fn request_withdrawal<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, ClientIp(ip): ClientIp, Json(body): Json<WithdrawRequest>) -> Response {
    let Ok(coin) = body.coin.parse::<shared::Coin>() else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "unknown coin" }))).into_response();
    };
    if shared::is_deposit_withdraw_paused(coin) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": format!("{} withdrawals are temporarily paused", coin.as_str()),
                "code": "WITHDRAWAL_PAUSED",
                "coin": coin.as_str(),
            })),
        )
            .into_response();
    }
    let Ok(amount) = BigDecimal::from_str(&body.amount) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid amount" }))).into_response();
    };

    let ip_fp = state.secrets.ip_fingerprint(&ip);

    // On-chain withdrawals always debit the personal wallet. Merchant caixa is
    // business float: it has to be moved to PERSONAL deliberately before any of
    // it can leave the platform, since an on-chain send cannot be undone.
    // Gated before the OTP step so a blocked attempt never sends a code email.
    match body.wallet_kind.as_deref().unwrap_or("PERSONAL").to_ascii_uppercase().as_str() {
        "PERSONAL" => {}
        "MERCHANT" => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "WITHDRAWAL_MERCHANT_BLOCKED".into(),
                "Withdrawal".into(),
                None,
                Some(ip_fp),
                Some(json!({ "coin": coin.as_str(), "walletKind": "MERCHANT" })),
            );
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "on-chain withdrawals debit the personal wallet; transfer the merchant balance to personal first",
                    "code": "WITHDRAWAL_MERCHANT_BLOCKED",
                    "coin": coin.as_str(),
                })),
            )
                .into_response();
        }
        _ => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": "walletKind must be PERSONAL" }))).into_response();
        }
    }

    let account = match state.auth.get_user_by_id(user.id).await {
        Ok(Some(u)) => u,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "user not found" }))).into_response(),
        Err(e) => return crate::http_error::internal_error(&e),
    };
    if state.settings.smtp_enabled || account.two_factor_enabled {
        let code = body.email_code.as_deref().or(body.totp_code.as_deref());
        if let Err(resp) = crate::auth::require_step_up_otp(&state.auth, user.id, "WITHDRAWAL", code).await {
            return resp;
        }
    }

    let client = state.chain_registry.get(coin);

    match db::treasury_health::fee_margin_blocks_coin(&state.pool, coin).await {
        Ok(true) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "withdrawals paused: network fees exceed platform fee revenue for this coin",
                    "code": "FEE_MARGIN_NEGATIVE",
                    "coin": coin.as_str(),
                })),
            )
                .into_response();
        }
        Ok(false) => {}
        Err(e) => {
            if db::treasury_health::hard_block_enabled() {
                tracing::error!(error = %e, coin = %coin.as_str(), "fee margin check failed; blocking withdrawal");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "error": "fee margin check unavailable",
                        "code": "FEE_MARGIN_CHECK_UNAVAILABLE",
                        "coin": coin.as_str(),
                    })),
                )
                    .into_response();
            }
            tracing::warn!(error = %e, coin = %coin.as_str(), "fee margin check failed; hard block off, allowing withdrawal");
        }
    }

    let result = db::withdrawals::request_withdrawal(
        &state.pool,
        user.id,
        coin,
        &body.to_address,
        amount,
        client.as_ref(),
        &ip,
        body.idempotency_key.as_deref(),
        Some(&state.secrets),
    )
    .await;

    match result {
        Ok((withdrawal, freshly_created)) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "WITHDRAWAL_REQUESTED".into(),
                "Withdrawal".into(),
                Some(withdrawal.id),
                Some(ip_fp.clone()),
                Some(json!({
                    "coin": coin.as_str(),
                    "amount": body.amount,
                    "requiresApproval": withdrawal.requires_approval,
                })),
            );
            if freshly_created && !withdrawal.requires_approval {
                if let Err(e) = queue::enqueue(&state.pool, "withdrawal_broadcast", &json!({ "withdrawalId": withdrawal.id })).await {
                    tracing::error!(withdrawal_id = %withdrawal.id, error = %e, "failed to enqueue withdrawal_broadcast job");
                }
            }
            (
                StatusCode::CREATED,
                Json(WithdrawResponse { id: withdrawal.id.to_string(), status: withdrawal.status, requires_approval: withdrawal.requires_approval }),
            )
                .into_response()
        }
        Err(e) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "WITHDRAWAL_REQUEST_FAILED".into(),
                "Withdrawal".into(),
                None,
                Some(ip_fp),
                Some(json!({ "coin": coin.as_str(), "reason": e.to_string() })),
            );
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response()
        }
    }
}

#[derive(Deserialize)]
struct WithdrawalHistoryQuery {
    coin: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct WithdrawalHistoryItemResp {
    id: String,
    coin: String,
    #[serde(rename = "toAddress")]
    to_address: String,
    amount: String,
    #[serde(rename = "feeAmount")]
    fee_amount: String,
    status: String,
    #[serde(rename = "txHash")]
    tx_hash: Option<String>,
    #[serde(rename = "requiresApproval")]
    requires_approval: bool,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

async fn list_withdrawals<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Query(q): Query<WithdrawalHistoryQuery>,
) -> Response {
    let coin_filter = q.coin.and_then(|c| shared::COINS.into_iter().find(|coin| coin.as_str().eq_ignore_ascii_case(&c)));
    let limit = q.limit.unwrap_or(50).clamp(1, 100);

    match db::withdrawals::list_user_withdrawals(&state.pool, user.id, coin_filter, limit, Some(&state.secrets)).await {
        Ok(withdrawals) => {
            let items: Vec<WithdrawalHistoryItemResp> = withdrawals
                .into_iter()
                .map(|w| WithdrawalHistoryItemResp {
                    id: w.id.to_string(),
                    coin: w.coin,
                    to_address: w.to_address,
                    amount: w.amount,
                    fee_amount: w.fee_amount,
                    status: w.status,
                    tx_hash: w.tx_hash,
                    requires_approval: w.requires_approval,
                    created_at: w.created_at.to_rfc3339(),
                    updated_at: w.updated_at.to_rfc3339(),
                })
                .collect();
            Json(serde_json::json!({ "withdrawals": items })).into_response()
        }
        Err(e) => crate::http_error::internal_error(&e),
    }
}

#[derive(Serialize)]
struct AddressBookItemResp {
    id: String,
    coin: String,
    label: String,
    address: String,
    #[serde(rename = "createdAt")]
    created_at: String,
}

#[derive(Deserialize)]
struct AddAddressBody {
    coin: String,
    label: String,
    address: String,
}

async fn list_addresses<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match db::withdrawals::list_address_book(&state.pool, user.id).await {
        Ok(rows) => {
            let addresses: Vec<AddressBookItemResp> = rows
                .into_iter()
                .map(|r| AddressBookItemResp {
                    id: r.id.to_string(),
                    coin: r.coin,
                    label: r.label,
                    address: r.address,
                    created_at: r.created_at.to_rfc3339(),
                })
                .collect();
            Json(json!({ "addresses": addresses })).into_response()
        }
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn add_address<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Json(body): Json<AddAddressBody>,
) -> Response {
    let Ok(coin) = body.coin.parse::<shared::Coin>() else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "unknown coin" }))).into_response();
    };
    match db::withdrawals::add_address_book(&state.pool, user.id, coin, &body.label, &body.address).await {
        Ok(row) => (
            StatusCode::CREATED,
            Json(AddressBookItemResp {
                id: row.id.to_string(),
                coin: row.coin,
                label: row.label,
                address: row.address,
                created_at: row.created_at.to_rfc3339(),
            }),
        )
            .into_response(),
        Err(db::withdrawals::WithdrawalsError::InvalidAddress) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": { "code": "INVALID_ADDRESS", "message": "Endereço ou rótulo inválido." } })),
        )
            .into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn delete_address<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Response {
    match db::withdrawals::delete_address_book(&state.pool, user.id, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}
