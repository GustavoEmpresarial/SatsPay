//! Thin axum routes for faucet + faucetlist — port of legacy
//! `faucet.controller.ts` / `faucetlist` routes.

use crate::client_ip::ClientIp;
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use captcha::VerifyOptions;
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// The Turnstile widget `action` attribute the frontend sets for the faucet
/// claim flow — matched against Cloudflare's response so a token minted for
/// a different action/widget on the site can't be replayed here.
pub const FAUCET_TURNSTILE_ACTION: &str = "faucet_claim";

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/faucet/status", get(status::<R>))
        .route("/v1/faucet/claim", post(claim_body::<R>))
        .route("/v1/faucet/claim/:coin", post(claim::<R>))
        .route("/v1/faucetlist", get(list_approved::<R>).post(create_site::<R>))
        .route("/v1/faucetlist/mine", get(list_mine::<R>))
        .route("/v1/faucetlist/click/:id", post(register_click::<R>))
        .route("/v1/faucetlist/:id", delete(delete_site::<R>))
        // Aliases without /v1
        .route("/faucet/status", get(status::<R>))
        .route("/faucet/claim", post(claim_body::<R>))
        .route("/faucet/claim/:coin", post(claim::<R>))
        .route("/faucetlist", get(list_approved::<R>).post(create_site::<R>))
        .route("/faucetlist/mine", get(list_mine::<R>))
        .route("/faucetlist/click/:id", post(register_click::<R>))
        .route("/faucetlist/:id", delete(delete_site::<R>))
}

#[derive(Deserialize)]
struct ClaimRequest {
    coin: Option<String>,
    #[serde(rename = "captchaToken", alias = "captcha_token")]
    captcha_token: String,
}

#[derive(Serialize)]
struct ClaimResponse {
    amount: String,
    coin: String,
    #[serde(rename = "nextClaimAt")]
    next_claim_at: String,
    /// True when FAUCET_CLAIM points were written for an ACTIVE season.
    #[serde(rename = "pointsAwarded")]
    points_awarded: bool,
    /// Mirrors airdrop season presence at claim time.
    #[serde(rename = "seasonActive")]
    season_active: bool,
}

#[derive(Serialize)]
struct StatusResponse {
    #[serde(rename = "cooldownMinutes")]
    cooldown_minutes: i64,
    coins: Vec<CoinStatus>,
}

#[derive(Serialize)]
struct CoinStatus {
    coin: String,
    #[serde(rename = "nextClaimAt")]
    next_claim_at: Option<String>,
}

async fn status<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    ClientIp(remote_ip): ClientIp,
) -> Response {
    let ip_fp = state.secrets.ip_fingerprint(&remote_ip);
    match db::faucet::cooldowns(&state.pool, user.id, &ip_fp, state.settings.faucet_cooldown_minutes).await {
        Ok(rows) => Json(StatusResponse {
            cooldown_minutes: state.settings.faucet_cooldown_minutes,
            coins: rows
                .into_iter()
                .map(|row| CoinStatus {
                    coin: row.coin,
                    next_claim_at: row.next_claim_at.map(|t| t.to_rfc3339()),
                })
                .collect(),
        })
        .into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn claim_body<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    ClientIp(remote_ip): ClientIp,
    Json(body): Json<ClaimRequest>,
) -> Response {
    let coin_str = match body.coin.as_deref() {
        Some(c) => c,
        None => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "coin is required" }))).into_response(),
    };
    let Ok(coin) = coin_str.parse::<shared::Coin>() else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "unknown coin" }))).into_response();
    };
    execute_claim(state, user, coin, remote_ip, &body.captcha_token).await
}

async fn claim<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(coin_str): Path<String>,
    ClientIp(remote_ip): ClientIp,
    Json(body): Json<ClaimRequest>,
) -> Response {
    let Ok(coin) = coin_str.parse::<shared::Coin>() else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "unknown coin" }))).into_response();
    };
    execute_claim(state, user, coin, remote_ip, &body.captcha_token).await
}

async fn execute_claim<R: AuthRepo>(
    state: AppState<R>,
    user: AuthUser,
    coin: shared::Coin,
    remote_ip: String,
    captcha_token: &str,
) -> Response {
    let captcha_ok = state
        .captcha
        .verify(&state.pool, captcha_token, Some(remote_ip.as_str()), VerifyOptions { expected_action: Some(FAUCET_TURNSTILE_ACTION), expected_hostnames: &state.settings.captcha_expected_hostnames })
        .await;
    match captcha_ok {
        Ok(true) => {}
        Ok(false) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "captcha verification failed" }))).into_response(),
        Err(e) => return crate::http_error::internal_error(&e),
    }

    match db::treasury_health::fee_margin_blocks_coin(&state.pool, coin).await {
        Ok(true) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "faucet paused: network fees exceed platform fee revenue for this coin",
                    "code": "FEE_MARGIN_NEGATIVE",
                    "coin": coin.as_str(),
                })),
            )
                .into_response();
        }
        Ok(false) => {}
        Err(e) => {
            if db::treasury_health::hard_block_enabled() {
                tracing::error!(error = %e, coin = %coin.as_str(), "fee margin check failed; blocking claim");
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
            tracing::warn!(error = %e, coin = %coin.as_str(), "fee margin check failed; hard block off, allowing claim");
        }
    }

    let ip_fp = state.secrets.ip_fingerprint(&remote_ip);
    match db::faucet::claim(&state.pool, user.id, coin, &ip_fp, state.settings.faucet_cooldown_minutes).await {
        Ok(result) => {
            let uid = user.id;
            let c_str = result.coin.as_str().to_string();
            let comm_amt = bigdecimal::BigDecimal::from(result.amount) / bigdecimal::BigDecimal::from(10);
            let amount_usd = db::pricing::usd_from_ledger_amount(&state.pool, result.coin, &comm_amt).await;
            let points_awarded = result.points_awarded;
            let season_active = result.season_active;

            if let Err(e) = db::referral::record_referral_commission(
                &state.pool,
                uid,
                "FAUCET_CLAIM",
                &c_str,
                comm_amt,
                amount_usd,
            )
            .await
            {
                tracing::warn!(user_id = %uid, error = %e, "referral commission on faucet failed");
            }

            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "FAUCET_CLAIM".into(),
                "Faucet".into(),
                None,
                Some(ip_fp.clone()),
                Some(json!({ "coin": result.coin.as_str(), "amount": result.amount.to_string() })),
            );
            Json(ClaimResponse {
                amount: result.amount.to_string(),
                coin: result.coin.as_str().to_string(),
                next_claim_at: result.next_claim_at.to_rfc3339(),
                points_awarded,
                season_active,
            })
            .into_response()
        }
        Err(e) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "FAUCET_CLAIM_FAILED".into(),
                "Faucet".into(),
                None,
                Some(ip_fp),
                Some(json!({ "coin": coin.as_str(), "reason": e.to_string() })),
            );
            match e {
                db::faucet::FaucetError::Cooldown { next_claim_at } => (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({
                        "error": {
                            "code": "FAUCET_COOLDOWN",
                            "message": "Aguarde o cooldown do faucet.",
                            "nextClaimAt": next_claim_at.to_rfc3339()
                        }
                    })),
                )
                    .into_response(),
                db::faucet::FaucetError::NotFound => (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": { "code": "WALLET_NOT_FOUND", "message": "Carteira não encontrada." } })),
                )
                    .into_response(),
                other => (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": { "code": "FAUCET_ERROR", "message": other.to_string() } })),
                )
                    .into_response(),
            }
        }
    }
}

async fn list_approved<R: AuthRepo>(State(state): State<AppState<R>>) -> Response {
    match db::faucetlist::list_approved(&state.pool).await {
        Ok(sites) => Json(json!({ "sites": sites })).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn register_click<R: AuthRepo>(State(state): State<AppState<R>>, Path(id): Path<Uuid>) -> Response {
    match db::faucetlist::register_click(&state.pool, id).await {
        Ok(url) => Redirect::to(&url).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, Json(json!({ "error": "faucet site not found" }))).into_response(),
    }
}

async fn list_mine<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match db::faucetlist::list_mine(&state.pool, user.id).await {
        Ok(sites) => Json(json!({ "sites": sites })).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

#[derive(Deserialize)]
struct CreateSiteReq {
    name: String,
    url: String,
    description: String,
    coins: Vec<String>,
    #[serde(rename = "rewardInfo")]
    reward_info: Option<String>,
}

async fn create_site<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Json(body): Json<CreateSiteReq>) -> Response {
    if body.name.trim().is_empty() || body.url.trim().is_empty() || body.description.trim().len() < 10 {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid name, url or description (min 10 chars)" }))).into_response();
    }

    let coin_strs: Vec<&str> = body.coins.iter().map(String::as_str).collect();
    match db::faucetlist::create_site(&state.pool, user.id, &body.name, &body.url, &body.description, &coin_strs, body.reward_info.as_deref()).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id, "status": "PENDING" }))).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn delete_site<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    match db::faucetlist::delete_site(&state.pool, user.id, id).await {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, Json(json!({ "error": "faucet site not found or not owned" }))).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faucet_turnstile_action_is_pinned() {
        assert_eq!(FAUCET_TURNSTILE_ACTION, "faucet_claim");
    }
}
