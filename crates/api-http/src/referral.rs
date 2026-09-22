//! Módulo HTTP para gestão de Referrals / Programa de Afiliados.

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
        .route("/v1/referral/stats", get(get_stats::<R>))
        .route("/v1/referral/commissions", get(get_commissions::<R>))
        .route("/v1/referral/users", get(get_referred_users::<R>))
        .route("/v1/referral/list", get(get_referred_users::<R>))
}

async fn get_stats<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    match db::referral::get_referral_stats(&state.pool, user.id).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn get_commissions<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    match db::referral::list_user_commissions(&state.pool, user.id, 50).await {
        Ok(list) => Json(json!({ "commissions": list })).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn get_referred_users<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    match db::referral::list_referred_users(&state.pool, user.id, 100).await {
        Ok(list) => Json(json!({ "referred_users": list })).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

