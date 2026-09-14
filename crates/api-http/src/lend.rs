//! Thin axum routes for the lend/borrow money market — port of legacy
//! `lend.controller.ts`. Reconstructed from the `db::lend` surface and the
//! frontend calls in `client/src/pages/LendPage.tsx`
//! (`GET /v1/lend/markets`, `GET /v1/lend/positions`,
//! `POST /v1/lend/{supply,withdraw,borrow,repay}/{coin}` with `{ amount }`).
//!
//! NOTE: `db::lend::{MarketView, AccountLiquidityView}` serialize with their
//! Rust (snake_case) field names; if the UI expects camelCase, wrap them in
//! dedicated response structs like `swap.rs` does.

use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde::Deserialize;
use serde_json::json;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/lend/markets", get(markets::<R>))
        .route("/v1/lend/positions", get(positions::<R>))
        .route("/v1/lend/supply/:coin", post(supply::<R>))
        .route("/v1/lend/withdraw/:coin", post(withdraw::<R>))
        .route("/v1/lend/borrow/:coin", post(borrow::<R>))
        .route("/v1/lend/repay/:coin", post(repay::<R>))
}

async fn markets<R: AuthRepo>(State(state): State<AppState<R>>, _user: AuthUser) -> Response {
    match db::lend::get_markets(&state.pool).await {
        Ok(markets) => Json(json!({ "markets": markets })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn positions<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match db::lend::get_user_positions(&state.pool, user.id, state.settings.price_max_stale).await {
        Ok(view) => Json(view).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct AmountBody {
    amount: String,
}

fn parse_coin_amount(coin: &str, amount: &str) -> Option<(shared::Coin, u128)> {
    Some((coin.parse::<shared::Coin>().ok()?, amount.parse::<u128>().ok()?))
}

async fn supply<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(coin): Path<String>,
    Json(body): Json<AmountBody>,
) -> Response {
    let Some((coin, amount)) = parse_coin_amount(&coin, &body.amount) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::lend::supply(&state.pool, user.id, coin, amount).await {
        Ok(v) => Json(json!({ "amount": v.to_string() })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn withdraw<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(coin): Path<String>,
    Json(body): Json<AmountBody>,
) -> Response {
    let Some((coin, amount)) = parse_coin_amount(&coin, &body.amount) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::lend::withdraw(&state.pool, user.id, coin, amount, state.settings.price_max_stale).await {
        Ok(v) => Json(json!({ "amount": v.to_string() })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn borrow<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(coin): Path<String>,
    Json(body): Json<AmountBody>,
) -> Response {
    let Some((coin, amount)) = parse_coin_amount(&coin, &body.amount) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::lend::borrow(&state.pool, user.id, coin, amount, state.settings.price_max_stale).await {
        Ok(v) => Json(json!({ "amount": v.to_string() })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn repay<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(coin): Path<String>,
    Json(body): Json<AmountBody>,
) -> Response {
    let Some((coin, amount)) = parse_coin_amount(&coin, &body.amount) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::lend::repay(&state.pool, user.id, coin, amount).await {
        Ok(v) => Json(json!({ "amount": v.to_string() })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
