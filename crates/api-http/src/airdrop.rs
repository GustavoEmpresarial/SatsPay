//! Módulo HTTP para gestão de Airdrop ($SATS Seasons & Leaderboard).

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde_json::json;

use crate::middleware::AuthUser;
use crate::AppState;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/airdrop/overview", get(get_overview::<R>))
        .route("/v1/airdrop/profile", get(get_overview::<R>))
        .route("/v1/airdrop/leaderboard", get(get_leaderboard::<R>))
        .route("/v1/airdrop/logs", get(get_logs::<R>))
        .route("/v1/airdrop/history", get(get_logs::<R>))
}

async fn get_overview<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    match db::airdrop::get_user_airdrop_profile(&state.pool, user.id).await {
        Ok(profile) => Json(profile).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn get_logs<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    match db::airdrop::list_user_point_logs(&state.pool, user.id, 50).await {
        Ok(logs) => Json(json!({ "logs": logs })).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn get_leaderboard<R: AuthRepo>(
    State(state): State<AppState<R>>,
    _user: AuthUser,
) -> Response {
    match db::airdrop::get_airdrop_leaderboard(&state.pool, 100).await {
        Ok(list) => Json(json!({ "leaderboard": list })).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

