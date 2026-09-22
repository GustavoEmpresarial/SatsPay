//! Thin axum routes for wallet — port of legacy `wallet.controller.ts`.

use crate::client_ip::ClientIp;
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bigdecimal::BigDecimal;
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/wallet", get(list_wallets::<R>))
        .route("/v1/wallet/transfer", post(transfer::<R>))
        .route("/v1/wallet/ledger", get(list_ledger::<R>))
}

#[derive(Serialize)]
struct WalletResponse {
    coin: String,
    address: Option<String>,
    balance: String,
    kind: String,
}

#[derive(Deserialize)]
struct ListWalletsQuery {
    kind: Option<String>,
}

async fn list_wallets<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Query(q): Query<ListWalletsQuery>,
) -> Response {
    let kind = q.kind.as_deref().unwrap_or("PERSONAL");
    let kind = match kind.to_ascii_uppercase().as_str() {
        "DEVELOPER" => "DEVELOPER",
        "MERCHANT" => "MERCHANT",
        _ => "PERSONAL",
    };
    match db::wallet::list_wallets(&state.pool, user.id, kind).await {
        Ok(wallets) => {
            let payload: Vec<WalletResponse> =
                wallets.into_iter().map(|w| WalletResponse { coin: w.coin, address: w.address, balance: w.balance.to_string(), kind: w.kind }).collect();
            Json(payload).into_response()
        }
        Err(e) => crate::http_error::internal_error(&e),
    }
}

#[derive(Deserialize)]
struct TransferRequest {
    coin: String,
    amount: String,
    #[serde(rename = "toDeveloper")]
    to_developer: bool,
}

async fn transfer<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    user: AuthUser,
    Json(body): Json<TransferRequest>,
) -> Response {
    let Ok(amount) = BigDecimal::from_str(&body.amount) else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "invalid amount" }))).into_response();
    };
    match db::wallet::transfer_between_kinds(&state.pool, user.id, &body.coin, amount.clone(), body.to_developer).await {
        Ok(()) => {
            let direction = if body.to_developer { "PERSONAL_TO_MERCHANT" } else { "MERCHANT_TO_PERSONAL" };
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "WALLET_INTERNAL_TRANSFER".into(),
                "Wallet".into(),
                None,
                Some(state.secrets.ip_fingerprint(&ip)),
                Some(serde_json::json!({ "coin": body.coin, "amount": body.amount, "direction": direction })),
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct LedgerQuery {
    coin: Option<String>,
    kind: Option<String>,
    take: Option<i64>,
}

#[derive(Serialize)]
struct LedgerEntryResponse {
    id: String,
    coin: String,
    amount: String,
    #[serde(rename = "type")]
    entry_type: String,
    memo: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
}

async fn list_ledger<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Query(q): Query<LedgerQuery>) -> Response {
    let take = q.take.unwrap_or(50);
    let kind = q.kind.as_deref().unwrap_or("PERSONAL");
    let kind = match kind.to_ascii_uppercase().as_str() {
        "DEVELOPER" => "DEVELOPER",
        "MERCHANT" => "MERCHANT",
        _ => "PERSONAL",
    };
    match db::wallet::list_ledger_entries(&state.pool, user.id, kind, q.coin.as_deref(), take).await {
        Ok(entries) => {
            let payload: Vec<LedgerEntryResponse> = entries
                .into_iter()
                .map(|e| LedgerEntryResponse {
                    id: e.id.to_string(),
                    coin: e.coin,
                    amount: e.amount.to_string(),
                    entry_type: e.entry_type,
                    memo: e.memo,
                    created_at: e.created_at.to_rfc3339(),
                })
                .collect();
            Json(serde_json::json!({ "entries": payload })).into_response()
        }
        Err(e) => crate::http_error::internal_error(&e),
    }
}
