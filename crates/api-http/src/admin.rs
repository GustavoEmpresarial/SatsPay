//! Thin axum routes for admin — port of legacy `admin.routes.ts` (the
//! transactional actions; the dashboard-stats endpoint is a couple of
//! `count(*)` queries and isn't ported here since it protects no invariant).

use crate::client_ip::ClientIp;
use crate::middleware::{require_admin, AuthUser};
use crate::notify_email::{faucet_site_owner_email, send_best_effort};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/admin/stats", axum::routing::get(admin_stats::<R>))
        .route("/v1/admin/economics", axum::routing::get(admin_economics::<R>))
        .route("/v1/admin/treasury-wallets", axum::routing::get(treasury_wallets::<R>))
        .route("/v1/admin/treasury-health", axum::routing::get(treasury_health::<R>))
        .route("/v1/admin/pending-withdrawals", axum::routing::get(pending_withdrawals::<R>))
        .route("/v1/admin/withdrawals", axum::routing::get(list_all_withdrawals::<R>))
        .route("/v1/admin/withdrawals/:id/approve", post(approve_withdrawal::<R>))
        .route("/v1/admin/withdrawals/:id/reject", post(reject_withdrawal::<R>))
        .route("/v1/admin/merchants", axum::routing::get(list_merchants::<R>))
        .route("/v1/admin/merchants/stats", axum::routing::get(merchant_stats::<R>))
        .route("/v1/admin/merchants/:id/approve", post(approve_merchant::<R>))
        .route("/v1/admin/merchants/:id/suspend", post(suspend_merchant::<R>))
        .route("/v1/admin/house/fund", post(fund_house::<R>))
        .route("/v1/admin/lend-pool/fund", post(fund_lend_pool::<R>))
        .route("/v1/admin/faucetlist", axum::routing::get(list_faucets::<R>))
        .route("/v1/admin/faucetlist/:id/approve", post(approve_faucet_site::<R>))
        .route("/v1/admin/faucetlist/:id/reject", post(reject_faucet_site::<R>))
        .route("/v1/admin/faucetlist/:id/suspend", post(suspend_faucet_site::<R>))
        .route("/v1/admin/rewards/programs", axum::routing::get(list_reward_programs::<R>).post(create_reward_program::<R>))
        .route("/v1/admin/rewards/programs/:id/active", post(set_reward_program_active::<R>))
        .route("/v1/admin/audit-logs", axum::routing::get(list_audit_logs::<R>))
        .route("/v1/admin/telemetry/overview", axum::routing::get(telemetry_overview::<R>))
        .route("/v1/admin/telemetry/metrics-history", axum::routing::get(telemetry_metrics_history::<R>))
        .route("/v1/admin/telemetry/errors", axum::routing::get(list_telemetry_errors::<R>))
        .route("/v1/admin/telemetry/errors/:id/resolve", post(resolve_telemetry_error::<R>))
        .route("/v1/admin/telemetry/errors/:id/ignore", post(ignore_telemetry_error::<R>))
        .route("/v1/admin/telemetry/errors/batch-resolve", post(batch_resolve_telemetry_errors::<R>))
        .route("/v1/admin/telemetry/errors/resolve-all", post(resolve_all_telemetry_errors::<R>))
        .route("/v1/admin/telemetry/errors/clear", post(clear_telemetry_errors::<R>))
        .route("/v1/admin/telemetry/test-error", post(trigger_test_error::<R>))
        .route("/v1/telemetry/client-error", post(record_client_error::<R>))
        .route("/v1/telemetry/client-errors", post(record_client_errors_batch::<R>))
}

#[derive(serde::Serialize)]
struct TreasuryWalletRow {
    role: String,
    coin: String,
    address: String,
    hd_index: Option<i64>,
    email: Option<String>,
    onchain: String,
    ledger: String,
    error: Option<String>,
}

fn explorer_base(coin: &str) -> &'static str {
    match coin {
        "BTC" => "https://mempool.space/address/",
        "LTC" => "https://blockchair.com/litecoin/address/",
        "DOGE" => "https://dogechain.info/address/",
        "BCH" => "https://blockchair.com/bitcoin-cash/address/",
        "DGB" => "https://digiexplorer.info/address/",
        "SOL" => "https://solscan.io/account/",
        _ => "https://polygonscan.com/address/",
    }
}

fn resolve_hot_address(coin: shared::Coin, hot_mnemonic: Option<&str>) -> Result<String, String> {
    let network = match std::env::var("CHAIN_NETWORK").as_deref() {
        Ok("testnet") => chain::ChainNetwork::Testnet,
        _ => chain::ChainNetwork::Mainnet,
    };
    let mnemonic = hot_mnemonic
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("HOT_MNEMONIC")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    if let Some(m) = mnemonic {
        return chain::hd_wallet::hot_address_from_mnemonic(&m, coin, network).map_err(|e| e.to_string());
    }
    let key = std::env::var("HOT_WALLET_PRIVATE_KEY")
        .ok()
        .or_else(|| std::env::var("POL_HOT_WALLET_KEY").ok())
        .or_else(|| std::env::var("HOT_WALLET_WIF").ok())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "hot key ausente".to_string())?;
    chain::hot_wallet_address(coin, network, &key)
}

async fn onchain_balance(registry: &chain::ChainRegistry, coin: shared::Coin, address: &str) -> (String, Option<String>) {
    match registry.get(coin).get_balance(address).await {
        Ok(v) => (v.to_string(), None),
        Err(e) => ("0".into(), Some(e.to_string())),
    }
}

async fn treasury_wallets<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }

    let ledger_rows = sqlx::query(
        "SELECT w.coin::text as coin, COALESCE(SUM(le.amount), 0)::text as ledger \
         FROM wallets w LEFT JOIN ledger_entries le ON le.wallet_id = w.id \
         WHERE w.kind = 'PERSONAL' GROUP BY w.coin",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let mut ledgers = std::collections::HashMap::<String, String>::new();
    for r in ledger_rows {
        use sqlx::Row;
        ledgers.insert(r.get("coin"), r.get("ledger"));
    }

    let mut hot_handles = Vec::new();
    for coin in shared::COINS {
        let registry = state.chain_registry.clone();
        let ledger = ledgers.get(coin.as_str()).cloned().unwrap_or_else(|| "0".into());
        let hot_mnemonic = state.hot_mnemonic.clone();
        hot_handles.push(tokio::spawn(async move {
            match resolve_hot_address(coin, hot_mnemonic.as_deref()) {
                Ok(address) => {
                    let (onchain, error) = onchain_balance(&registry, coin, &address).await;
                    TreasuryWalletRow {
                        role: "hot".into(),
                        coin: coin.as_str().to_string(),
                        address,
                        hd_index: None,
                        email: None,
                        onchain,
                        ledger,
                        error,
                    }
                }
                Err(e) => TreasuryWalletRow {
                    role: "hot".into(),
                    coin: coin.as_str().to_string(),
                    address: String::new(),
                    hd_index: None,
                    email: None,
                    onchain: "0".into(),
                    ledger,
                    error: Some(e),
                },
            }
        }));
    }

    let mut rows = Vec::new();
    for handle in hot_handles {
        if let Ok(row) = handle.await {
            rows.push(row);
        }
    }

    let deposits = sqlx::query(
        "SELECT u.email, w.coin::text as coin, w.address, w.hd_index, \
                COALESCE((SELECT SUM(le.amount) FROM ledger_entries le WHERE le.wallet_id = w.id), 0)::text as ledger \
         FROM wallets w JOIN users u ON u.id = w.user_id \
         WHERE w.kind = 'PERSONAL' AND w.address IS NOT NULL \
         ORDER BY u.email, w.coin \
         LIMIT 80",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut dep_handles = Vec::new();
    for r in deposits {
        use sqlx::Row;
        let coin_str: String = r.get("coin");
        let address: String = r.get("address");
        let Some(coin) = shared::COINS.into_iter().find(|c| c.as_str() == coin_str) else { continue };
        let registry = state.chain_registry.clone();
        let email: String = r.get("email");
        let hd_index: Option<i64> = r.get("hd_index");
        let ledger: String = r.get("ledger");
        dep_handles.push(tokio::spawn(async move {
            let (onchain, error) = onchain_balance(&registry, coin, &address).await;
            TreasuryWalletRow {
                role: "deposit".into(),
                coin: coin_str,
                address,
                hd_index,
                email: Some(email),
                onchain,
                ledger,
                error,
            }
        }));
    }
    for handle in dep_handles {
        if let Ok(row) = handle.await {
            rows.push(row);
        }
    }

    Json(json!({
        "wallets": rows,
        "explorers": {
            "BTC": explorer_base("BTC"),
            "LTC": explorer_base("LTC"),
            "DOGE": explorer_base("DOGE"),
            "BCH": explorer_base("BCH"),
            "DGB": explorer_base("DGB"),
            "POL": explorer_base("POL"),
            "USDT": explorer_base("USDT"),
            "USDC": explorer_base("USDC"),
            "SOL": explorer_base("SOL"),
        }
    }))
    .into_response()
}

async fn treasury_health<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }

    let ledger_rows = sqlx::query(
        "SELECT w.coin::text as coin, COALESCE(SUM(le.amount), 0)::text as ledger \
         FROM wallets w LEFT JOIN ledger_entries le ON le.wallet_id = w.id \
         WHERE w.kind = 'PERSONAL' GROUP BY w.coin",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let mut custody = std::collections::HashMap::<String, String>::new();
    for r in &ledger_rows {
        use sqlx::Row;
        custody.insert(r.get("coin"), r.get("ledger"));
    }

    let mut hot_onchain = std::collections::HashMap::<String, (String, Option<String>)>::new();
    let mut handles = Vec::new();
    for coin in shared::COINS {
        let registry = state.chain_registry.clone();
        let hot_mnemonic = state.hot_mnemonic.clone();
        handles.push(tokio::spawn(async move {
            let addr = resolve_hot_address(coin, hot_mnemonic.as_deref());
            match addr {
                Ok(address) => {
                    let (onchain, error) = onchain_balance(&registry, coin, &address).await;
                    (coin.as_str().to_string(), onchain, error)
                }
                Err(e) => (coin.as_str().to_string(), "0".into(), Some(e)),
            }
        }));
    }
    for h in handles {
        if let Ok((coin, onchain, error)) = h.await {
            hot_onchain.insert(coin, (onchain, error));
        }
    }

    match db::treasury_health::get_treasury_health(&state.pool, &hot_onchain, &custody, vec![]).await {
        Ok(h) => Json(h).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn admin_stats<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::get_dashboard_stats(&state.pool).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn admin_economics<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::get_platform_economics(&state.pool).await {
        Ok(econ) => Json(econ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_all_withdrawals<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    axum::extract::Query(q): axum::extract::Query<WithdrawalFilterQuery>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::list_all_withdrawals(&state.pool, q.status.as_deref(), q.limit.unwrap_or(100)).await {
        Ok(list) => Json(json!({ "withdrawals": list })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct WithdrawalFilterQuery {
    status: Option<String>,
    limit: Option<i64>,
}

async fn list_merchants<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::list_all_merchants(&state.pool).await {
        Ok(list) => Json(json!({ "merchants": list })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn merchant_stats<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::get_merchant_platform_stats(&state.pool).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn approve_merchant<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::approve_merchant(&state.pool, id, user.id).await {
        Ok(()) => Json(json!({ "id": id, "verified": true })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn suspend_merchant<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::suspend_merchant(&state.pool, id, user.id).await {
        Ok(()) => Json(json!({ "id": id, "verified": false })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_faucets<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::list_all_faucet_sites(&state.pool).await {
        Ok(list) => Json(json!({ "sites": list })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_audit_logs<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::audit::list_recent_logs(&state.pool, 100).await {
        Ok(logs) => Json(json!({ "logs": logs })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn pending_withdrawals<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::list_pending_withdrawals(&state.pool).await {
        Ok(list) => Json(json!({ "withdrawals": list })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn approve_withdrawal<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>, ClientIp(ip): ClientIp) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    if let Err(e) = db::admin::approve_withdrawal(&state.pool, id, user.id, Some(&ip)).await {
        return (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response();
    }
    // Stable job payload dedupes accidental double-enqueue (e.g. approve pressed twice).
    if let Err(e) = queue::enqueue(&state.pool, "withdrawal_broadcast", &json!({ "withdrawalId": id })).await {
        tracing::error!(withdrawal_id = %id, error = %e, "failed to enqueue withdrawal_broadcast after admin approval");
    }
    Json(json!({ "id": id, "status": "APPROVED" })).into_response()
}

async fn reject_withdrawal<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>, ClientIp(ip): ClientIp) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::reject_withdrawal(&state.pool, id, user.id, Some(&ip)).await {
        Ok(()) => Json(json!({ "id": id, "status": "CANCELED" })).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct FundRequest {
    coin: String,
    amount: String,
}

async fn fund_house<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Json(body): Json<FundRequest>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    let (Ok(coin), Ok(amount)) = (body.coin.parse::<shared::Coin>(), body.amount.parse::<u128>()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::admin::fund_house(&state.pool, coin, amount, user.id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn fund_lend_pool<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Json(body): Json<FundRequest>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    let (Ok(coin), Ok(amount)) = (body.coin.parse::<shared::Coin>(), body.amount.parse::<u128>()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::admin::fund_lend_pool(&state.pool, coin, amount, user.id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn approve_faucet_site<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::approve_faucet_site(&state.pool, id, user.id).await {
        Ok(()) => {
            if let Some(to) = faucet_site_owner_email(&state.pool, id).await {
                send_best_effort(
                    state.email.as_ref(),
                    &to,
                    "BitcoSats faucet site approved",
                    "Your faucetlist site has been approved and is now listed.",
                )
                .await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct RejectSiteRequest {
    reason: String,
}

async fn reject_faucet_site<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>, Json(body): Json<RejectSiteRequest>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::reject_faucet_site(&state.pool, id, user.id, &body.reason).await {
        Ok(()) => {
            if let Some(to) = faucet_site_owner_email(&state.pool, id).await {
                let body_text = format!("Your faucetlist site was rejected.\n\nReason: {}", body.reason);
                send_best_effort(state.email.as_ref(), &to, "BitcoSats faucet site rejected", &body_text).await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn suspend_faucet_site<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::admin::suspend_faucet_site(&state.pool, id, user.id).await {
        Ok(()) => {
            if let Some(to) = faucet_site_owner_email(&state.pool, id).await {
                send_best_effort(
                    state.email.as_ref(),
                    &to,
                    "BitcoSats faucet site suspended",
                    "Your faucetlist site has been suspended and is no longer listed.",
                )
                .await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_reward_programs<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::rewards::list_programs(&state.pool).await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct CreateRewardProgramRequest {
    #[serde(rename = "rewardCoin")]
    reward_coin: String,
    #[serde(rename = "marketCoin")]
    market_coin: String,
    side: String,
    #[serde(rename = "emissionPerDay")]
    emission_per_day: String,
    #[serde(rename = "startAt")]
    start_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "endAt")]
    end_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn create_reward_program<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Json(body): Json<CreateRewardProgramRequest>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    let (Ok(reward_coin), Ok(market_coin), Ok(emission_per_day)) = (body.reward_coin.parse::<shared::Coin>(), body.market_coin.parse::<shared::Coin>(), body.emission_per_day.parse::<u128>()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::rewards::create_program(&state.pool, user.id, reward_coin, market_coin, &body.side, emission_per_day, body.start_at, body.end_at).await {
        Ok(program) => (StatusCode::CREATED, Json(program)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct SetActiveRequest {
    active: bool,
}

async fn set_reward_program_active<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>, Json(body): Json<SetActiveRequest>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::rewards::set_program_active(&state.pool, id, body.active).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn telemetry_overview<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::telemetry::get_telemetry_overview(&state.pool).await {
        Ok(overview) => Json(overview).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct MetricsHistoryQuery {
    hours: Option<i64>,
}

async fn telemetry_metrics_history<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    axum::extract::Query(q): axum::extract::Query<MetricsHistoryQuery>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    let hours = q.hours.unwrap_or(24);
    match db::telemetry::get_metrics_history(&state.pool, hours).await {
        Ok(snapshots) => Json(json!({ "snapshots": snapshots })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct ErrorFilterQuery {
    status: Option<String>,
    service: Option<String>,
    level: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
}

async fn list_telemetry_errors<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    axum::extract::Query(q): axum::extract::Query<ErrorFilterQuery>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::telemetry::list_errors(
        &state.pool,
        q.status.as_deref(),
        q.service.as_deref(),
        q.level.as_deref(),
        q.search.as_deref(),
        q.limit.unwrap_or(50),
    )
    .await
    {
        Ok(errors) => Json(json!({ "errors": errors })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn resolve_telemetry_error<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::telemetry::resolve_error(&state.pool, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct BatchResolveBody {
    ids: Vec<Uuid>,
}

async fn batch_resolve_telemetry_errors<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Json(body): Json<BatchResolveBody>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::telemetry::batch_resolve_errors(&state.pool, &body.ids).await {
        Ok(resolved) => Json(json!({ "resolved": resolved })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn resolve_all_telemetry_errors<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::telemetry::resolve_all_open_errors(&state.pool).await {
        Ok(resolved) => Json(json!({ "resolved": resolved })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn ignore_telemetry_error<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::telemetry::ignore_error(&state.pool, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn clear_telemetry_errors<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::telemetry::clear_resolved_errors(&state.pool).await {
        Ok(cleared) => Json(json!({ "cleared": cleared })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn trigger_test_error<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    let payload = db::telemetry::NewErrorPayload {
        service: "api-server".to_string(),
        level: "ERROR".to_string(),
        message: "Simulated Test Error: Database query timeout or node fallback event triggered".to_string(),
        stack_trace: Some("at crates/api-http/src/admin.rs:275\nat axum::handler::Handler::call\nat tokio::runtime::task::core".to_string()),
        endpoint: Some("/v1/admin/telemetry/test-error".to_string()),
        method: Some("POST".to_string()),
        status_code: Some(500),
        user_id: Some(user.id),
        ip_address: Some("127.0.0.1".to_string()),
        request_payload: Some(json!({ "action": "trigger_test_alert", "severity": "high" })),
        user_agent: Some("SatsPay-Telemetry-Probe/1.0".to_string()),
    };
    match db::telemetry::record_error(&state.pool, payload).await {
        Ok(rec) => Json(json!({ "recorded": true, "errorId": rec.id, "isNew": rec.is_new })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
struct ClientErrorPayload {
    message: String,
    stack: Option<String>,
    url: Option<String>,
    user_agent: Option<String>,
    kind: Option<String>,
    status_code: Option<i32>,
    level: Option<String>,
    context: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ClientErrorBatchPayload {
    #[serde(default)]
    errors: Vec<ClientErrorPayload>,
}

async fn persist_client_error(
    pool: &sqlx::PgPool,
    ip: String,
    user_id: Option<Uuid>,
    body: ClientErrorPayload,
) {
    let kind = body.kind.unwrap_or_else(|| "js".to_string());
    let level = body
        .level
        .filter(|l| matches!(l.as_str(), "ERROR" | "WARN" | "CRITICAL" | "FATAL" | "INFO"))
        .unwrap_or_else(|| "ERROR".to_string());
    let mut req_payload = json!({ "kind": kind });
    if let Some(ctx) = body.context {
        if let Some(obj) = ctx.as_object() {
            if let Some(target) = req_payload.as_object_mut() {
                for (k, v) in obj {
                    target.insert(k.clone(), v.clone());
                }
            }
        }
    }

    let payload = db::telemetry::NewErrorPayload {
        service: "client-frontend".to_string(),
        level,
        message: body.message.chars().take(2000).collect(),
        stack_trace: body.stack.map(|s| s.chars().take(8000).collect()),
        endpoint: body.url.map(|u| u.chars().take(500).collect()),
        method: None,
        status_code: body.status_code,
        user_id,
        ip_address: Some(ip),
        request_payload: Some(req_payload),
        user_agent: body.user_agent.map(|u| u.chars().take(400).collect()),
    };
    let _ = db::telemetry::record_error(pool, payload).await;
}

async fn record_client_error<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    crate::middleware::OptionalAuthUser(user): crate::middleware::OptionalAuthUser,
    Json(body): Json<ClientErrorPayload>,
) -> Response {
    persist_client_error(&state.pool, ip, user.map(|u| u.id), body).await;
    StatusCode::NO_CONTENT.into_response()
}

async fn record_client_errors_batch<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    crate::middleware::OptionalAuthUser(user): crate::middleware::OptionalAuthUser,
    Json(body): Json<ClientErrorBatchPayload>,
) -> Response {
    let user_id = user.map(|u| u.id);
    // Cap batch size to avoid abuse
    for item in body.errors.into_iter().take(25) {
        if item.message.trim().is_empty() {
            continue;
        }
        persist_client_error(&state.pool, ip.clone(), user_id, item).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn explorer_base_covers_known_coins() {
        for coin in ["BTC", "LTC", "DOGE", "BCH", "DGB", "SOL", "POL", "USDT", "USDC", "ZZZ"] {
            assert!(!explorer_base(coin).is_empty(), "{coin}");
        }
    }

    #[test]
    fn resolve_hot_address_mnemonic_and_missing() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("HOT_MNEMONIC");
        std::env::remove_var("HOT_WALLET_PRIVATE_KEY");
        std::env::remove_var("POL_HOT_WALLET_KEY");
        std::env::remove_var("HOT_WALLET_WIF");
        assert!(resolve_hot_address(shared::Coin::Btc, None).is_err());

        std::env::set_var(
            "HOT_MNEMONIC",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        );
        std::env::set_var("CHAIN_NETWORK", "mainnet");
        assert!(resolve_hot_address(shared::Coin::Btc, None).is_ok());
        std::env::set_var("CHAIN_NETWORK", "testnet");
        let _ = resolve_hot_address(shared::Coin::Ltc, None);
        // Explicit mnemonic argument wins even without env.
        std::env::remove_var("HOT_MNEMONIC");
        assert!(resolve_hot_address(
            shared::Coin::Btc,
            Some("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
        )
        .is_ok());
        std::env::remove_var("CHAIN_NETWORK");
    }
}

