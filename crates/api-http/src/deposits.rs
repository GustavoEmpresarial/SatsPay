//! Thin axum routes for deposits — port of legacy `deposits.controller.ts`
//! (minus the QR code data URL, which stays a frontend concern here).

use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/deposits/address/:coin", get(get_or_create_address::<R>))
        .route("/v1/deposits/history", get(list_deposits::<R>))
}

#[derive(Serialize)]
struct AddressResponse {
    address: String,
}

#[derive(Deserialize)]
struct DepositHistoryQuery {
    coin: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct DepositHistoryItemResp {
    id: String,
    coin: String,
    #[serde(rename = "txHash")]
    tx_hash: String,
    vout: i32,
    amount: String,
    confirmations: i32,
    #[serde(rename = "minConfirmations")]
    min_confirmations: i32,
    status: String,
    #[serde(rename = "detectedAt")]
    detected_at: String,
    #[serde(rename = "creditedAt")]
    credited_at: Option<String>,
}

async fn list_deposits<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Query(q): Query<DepositHistoryQuery>,
) -> Response {
    let coin_filter = q.coin.and_then(|c| shared::COINS.into_iter().find(|coin| coin.as_str().eq_ignore_ascii_case(&c)));
    let limit = q.limit.unwrap_or(50).clamp(1, 100);

    match db::deposits::list_user_deposits(&state.pool, user.id, coin_filter, limit).await {
        Ok(deposits) => {
            let items: Vec<DepositHistoryItemResp> = deposits
                .into_iter()
                .map(|d| DepositHistoryItemResp {
                    id: d.id.to_string(),
                    coin: d.coin,
                    tx_hash: d.tx_hash,
                    vout: d.vout,
                    amount: d.amount,
                    confirmations: d.confirmations,
                    min_confirmations: d.min_confirmations,
                    status: d.status,
                    detected_at: d.detected_at.to_rfc3339(),
                    credited_at: d.credited_at.map(|t| t.to_rfc3339()),
                })
                .collect();
            Json(serde_json::json!({ "deposits": items })).into_response()
        }
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn get_or_create_address<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(coin_str): Path<String>) -> Response {
    let Some(coin) = shared::COINS.into_iter().find(|c| c.as_str().eq_ignore_ascii_case(&coin_str)) else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "unknown coin" }))).into_response();
    };
    if shared::is_deposit_withdraw_paused(coin) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": format!("{} deposits are temporarily paused", coin.as_str()),
                "code": "DEPOSIT_PAUSED",
                "coin": coin.as_str(),
            })),
        )
            .into_response();
    }
    let client = state.chain_registry.get(coin);
    match db::deposits::get_or_create_address(&state.pool, user.id, coin, client.as_ref()).await {
        Ok(address) => Json(AddressResponse { address }).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}
