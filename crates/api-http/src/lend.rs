//! Thin axum routes for the lend/borrow money market.
//!
//! **Manutenção:** todas as rotas respondem `503 LEND_MAINTENANCE` até a aba
//! `/lend` reabrir. Posições/ledger existentes não são alteradas aqui.

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

fn maintenance() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": {
                "code": "LEND_MAINTENANCE",
                "message": "Empréstimos (Aave V3) em manutenção. Tente mais tarde."
            }
        })),
    )
        .into_response()
}

async fn markets<R: AuthRepo>(State(_state): State<AppState<R>>, _user: AuthUser) -> Response {
    maintenance()
}

async fn positions<R: AuthRepo>(State(_state): State<AppState<R>>, _user: AuthUser) -> Response {
    maintenance()
}

#[derive(Deserialize)]
struct AmountBody {
    #[allow(dead_code)]
    amount: String,
}

async fn supply<R: AuthRepo>(
    State(_state): State<AppState<R>>,
    _user: AuthUser,
    Path(_coin): Path<String>,
    Json(_body): Json<AmountBody>,
) -> Response {
    maintenance()
}

async fn withdraw<R: AuthRepo>(
    State(_state): State<AppState<R>>,
    _user: AuthUser,
    Path(_coin): Path<String>,
    Json(_body): Json<AmountBody>,
) -> Response {
    maintenance()
}

async fn borrow<R: AuthRepo>(
    State(_state): State<AppState<R>>,
    _user: AuthUser,
    Path(_coin): Path<String>,
    Json(_body): Json<AmountBody>,
) -> Response {
    maintenance()
}

async fn repay<R: AuthRepo>(
    State(_state): State<AppState<R>>,
    _user: AuthUser,
    Path(_coin): Path<String>,
    Json(_body): Json<AmountBody>,
) -> Response {
    maintenance()
}
