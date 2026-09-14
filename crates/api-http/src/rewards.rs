//! Thin axum route for a user's own reward history — port of legacy
//! `rewards.controller.ts`'s `getUserRewards`. Program management lives
//! under `/v1/admin/rewards/*` (`admin.rs`).

use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde::Serialize;
use serde_json::json;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new().route("/v1/rewards", get(get_user_rewards::<R>))
}

#[derive(Serialize)]
struct RewardResponse {
    #[serde(rename = "programId")]
    program_id: String,
    #[serde(rename = "rewardCoin")]
    reward_coin: String,
    #[serde(rename = "marketCoin")]
    market_coin: String,
    side: String,
    #[serde(rename = "receivedTotal")]
    received_total: String,
}

async fn get_user_rewards<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match db::rewards::get_user_rewards(&state.pool, user.id).await {
        Ok(rewards) => {
            let payload: Vec<RewardResponse> = rewards
                .into_iter()
                .map(|r| RewardResponse {
                    program_id: r.program_id.to_string(),
                    reward_coin: r.reward_coin.as_str().to_string(),
                    market_coin: r.market_coin.as_str().to_string(),
                    side: r.side,
                    received_total: r.received_total.to_string(),
                })
                .collect();
            Json(json!({ "rewards": payload })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
