//! Thin axum routes for stake — port of legacy `stake.controller.ts`.

use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/stake", get(list::<R>).post(create::<R>))
        .route("/v1/stake/strategies", get(strategies::<R>))
        .route("/v1/stake/:id/claim", post(claim::<R>))
        .route("/v1/stake/:id/cancel", post(cancel::<R>))
}

async fn strategies<R: AuthRepo>(State(state): State<AppState<R>>) -> Response {
    match db::stake::list_staking_strategies(&state.pool).await {
        Ok(strategies) => Json(json!({ "strategies": strategies })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Serialize)]
struct StakeResponse {
    id: String,
    coin: String,
    principal: String,
    #[serde(rename = "rewardBps")]
    reward_bps: u32,
    #[serde(rename = "lockDays")]
    lock_days: u32,
    reward: String,
    status: String,
    #[serde(rename = "maturesAt")]
    matures_at: String,
}

impl From<db::stake::StakeRow> for StakeResponse {
    fn from(s: db::stake::StakeRow) -> Self {
        Self {
            id: s.id.to_string(),
            coin: s.coin.as_str().to_string(),
            principal: s.principal.to_string(),
            reward_bps: s.reward_bps,
            lock_days: s.lock_days,
            reward: s.reward.to_string(),
            status: s.status,
            matures_at: s.matures_at.to_rfc3339(),
        }
    }
}

async fn list<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match db::stake::list_stakes(&state.pool, user.id).await {
        Ok(stakes) => Json(stakes.into_iter().map(StakeResponse::from).collect::<Vec<_>>()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct CreateStakeRequest {
    coin: String,
    amount: String,
    #[serde(rename = "lockDays")]
    lock_days: u32,
}

async fn create<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Json(body): Json<CreateStakeRequest>) -> Response {
    let (Ok(coin), Ok(amount)) = (body.coin.parse::<shared::Coin>(), body.amount.parse::<u128>()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::stake::create_stake(&state.pool, user.id, coin, amount, body.lock_days).await {
        Ok(stake) => (StatusCode::CREATED, Json(StakeResponse::from(stake))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn claim<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    match db::stake::claim_stake(&state.pool, user.id, id).await {
        Ok(stake) => Json(StakeResponse::from(stake)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn cancel<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    match db::stake::cancel_stake(&state.pool, user.id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
