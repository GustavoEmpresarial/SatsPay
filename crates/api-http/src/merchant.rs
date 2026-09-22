//! Thin axum routes for merchant — port of legacy `merchant.controller.ts`.
//! Admin review endpoints live under `/v1/admin/*`, gated by `require_admin`.

use crate::middleware::{require_admin, AuthUser};
use crate::notify_email::{send_best_effort, user_email};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/merchant/status", get(status::<R>))
        .route("/v1/merchant/apply", post(apply::<R>))
        .route("/v1/admin/merchant/applications", get(list_applications::<R>))
        .route("/v1/admin/merchant/:id/approve", post(approve::<R>))
        .route("/v1/admin/merchant/:id/reject", post(reject::<R>))
}

async fn status<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match db::merchant::get_status(&state.pool, Some(&state.secrets), user.id).await {
        Ok(view) => Json(view).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct ApplyRequest {
    #[serde(rename = "businessName")]
    business_name: String,
    website: String,
    description: String,
}

async fn apply<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Json(body): Json<ApplyRequest>) -> Response {
    match db::merchant::apply(&state.pool, Some(&state.secrets), user.id, &body.business_name, &body.website, &body.description).await {
        Ok(view) => Json(view).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_applications<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::merchant::list_applications(&state.pool, Some(&state.secrets), None).await {
        Ok(apps) => Json(apps).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn approve<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::merchant::approve(&state.pool, id, user.id).await {
        Ok(()) => {
            if let Some(to) = user_email(&state.pool, Some(&state.secrets), id).await {
                send_best_effort(
                    state.email.as_ref(),
                    &to,
                    "BitcoSats merchant application approved",
                    "Your merchant application has been approved.",
                )
                .await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct RejectRequest {
    reason: String,
}

async fn reject<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>, Json(body): Json<RejectRequest>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::merchant::reject(&state.pool, id, user.id, &body.reason).await {
        Ok(()) => {
            if let Some(to) = user_email(&state.pool, Some(&state.secrets), id).await {
                let body_text = format!("Your merchant application was rejected.\n\nReason: {}", body.reason);
                send_best_effort(state.email.as_ref(), &to, "BitcoSats merchant application rejected", &body_text).await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
