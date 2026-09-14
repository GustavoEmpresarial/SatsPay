//! Swap routes — custodial DEX via SwapKit.
//! Temporary policy: **L2 only** (POL / USDT / USDC on Polygon). HOUSE liquidity is off
//! unless `SWAP_HOUSE_ENABLED=true` (not used in production — no inventory capital).

use crate::client_ip::ClientIp;
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::{format_amount, is_swap_l2_pair, Coin};
use swapkit::{parse_human_to_ledger, route_priority, SwapKitError};
use uuid::Uuid;

fn house_enabled() -> bool {
    std::env::var("SWAP_HOUSE_ENABLED")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn reject_non_l2(from: Coin, to: Coin) -> Option<Response> {
    if is_swap_l2_pair(from, to) {
        return None;
    }
    Some(
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "swap temporarily limited to Polygon L2 pairs (POL, USDT, USDC)",
                "code": "SWAP_L2_ONLY",
            })),
        )
            .into_response(),
    )
}

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/swap/quote", get(quote_get::<R>).post(quote_post::<R>))
        .route("/v1/swap/prices", get(prices::<R>))
        .route("/v1/swap/history", get(history::<R>))
        .route("/v1/swap/orders/:id", get(order_status::<R>))
        .route("/v1/swap/telemetry", get(telemetry::<R>))
        .route("/v1/swap", post(execute::<R>))
        .route("/v1/swap/execute", post(execute::<R>)) // legacy client path
}

fn resolve_hot_address(coin: Coin, hot_mnemonic: Option<&str>) -> Result<String, String> {
    let network = match std::env::var("CHAIN_NETWORK").unwrap_or_else(|_| "mainnet".into()).as_str() {
        "testnet" => chain::ChainNetwork::Testnet,
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
    Err(format!("HOT_MNEMONIC not configured for {}", coin.as_str()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SwapHistoryItemResp {
    id: String,
    from_coin: String,
    to_coin: String,
    from_amount: String,
    to_amount: String,
    fee_amount: String,
    fee_bps: i32,
    status: String,
    provider: String,
    created_at: String,
    inbound_tx: Option<String>,
    outbound_tx: Option<String>,
    source: String,
}

async fn history<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    let mut items: Vec<SwapHistoryItemResp> = Vec::new();

    if let Ok(dex) = db::dex_swap::list_user(&state.pool, user.id, 50).await {
        for s in dex {
            items.push(SwapHistoryItemResp {
                id: s.id.to_string(),
                from_coin: s.from_coin,
                to_coin: s.to_coin,
                from_amount: s.from_amount,
                to_amount: s.actual_to_amount.unwrap_or(s.expected_to_amount),
                fee_amount: s.platform_fee_amount,
                fee_bps: s.platform_fee_bps,
                status: s.status,
                provider: s.provider,
                created_at: s.created_at.to_rfc3339(),
                inbound_tx: s.inbound_tx,
                outbound_tx: s.outbound_tx,
                source: "dex".into(),
            });
        }
    }

    if let Ok(legacy) = db::swap::list_user_swaps(&state.pool, user.id, 50).await {
        for s in legacy {
            items.push(SwapHistoryItemResp {
                id: s.id.to_string(),
                from_coin: s.from_coin,
                to_coin: s.to_coin,
                from_amount: s.from_amount,
                to_amount: s.to_amount,
                fee_amount: s.fee_amount,
                fee_bps: s.fee_bps,
                status: "COMPLETED".into(),
                provider: "SatsPay Liquidity".into(),
                created_at: s.created_at.to_rfc3339(),
                inbound_tx: None,
                outbound_tx: None,
                source: "house".into(),
            });
        }
    }

    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    items.truncate(50);
    Json(json!({ "swaps": items })).into_response()
}

async fn order_status<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    match db::dex_swap::get_by_id(&state.pool, id).await {
        Ok(Some(s)) if s.user_id == user.id => Json(s).into_response(),
        Ok(Some(_)) => (StatusCode::FORBIDDEN, Json(json!({ "error": "forbidden" }))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn telemetry<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = crate::middleware::require_admin(&user) {
        return *r;
    }
    match db::dex_swap::telemetry_snapshot(&state.pool).await {
        Ok(t) => Json(t).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn prices<R: AuthRepo>(State(state): State<AppState<R>>) -> Response {
    match db::pricing::list_cached_prices(&state.pool).await {
        Ok((decimals, prices)) => {
            let mut map = serde_json::Map::new();
            for coin in shared::COINS {
                let scaled = prices.get(&coin).copied().unwrap_or(0);
                map.insert(coin.as_str().to_string(), json!(scaled.to_string()));
            }
            Json(json!({ "priceDecimals": decimals, "prices": map })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteQuery {
    from_coin: String,
    to_coin: String,
    from_amount: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteBody {
    from_coin: String,
    to_coin: String,
    from_amount: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AmountCoin {
    amount: String,
    amount_human: String,
    coin: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformFee {
    bps: u32,
    amount: String,
    amount_human: String,
    asset: String,
    label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkFeeItem {
    #[serde(rename = "type")]
    fee_type: String,
    amount: String,
    asset: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteRouteResp {
    route_id: String,
    provider: String,
    providers: Vec<String>,
    tags: Vec<String>,
    you_pay: AmountCoin,
    you_receive: AmountCoin,
    min_receive: AmountCoin,
    fees: QuoteFees,
    eta_seconds: Option<EtaSeconds>,
    tx_hint: Option<String>,
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteFees {
    network: Vec<NetworkFeeItem>,
    platform: PlatformFee,
    total_platform_bps: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EtaSeconds {
    inbound: Option<u64>,
    swap: Option<u64>,
    outbound: Option<u64>,
    total: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteListResp {
    routes: Vec<QuoteRouteResp>,
    provider_errors: Option<Value>,
}

fn house_route(from: Coin, to: Coin, from_amount: u128, quote: &shared::SwapQuote) -> QuoteRouteResp {
    let fee_human = format_amount(quote.fee_amount, from);
    QuoteRouteResp {
        route_id: format!("house:{}:{}:{}", from.as_str(), to.as_str(), from_amount),
        provider: "SatsPay Liquidity".into(),
        providers: vec!["SATSPAY_LIQUIDITY".into()],
        tags: vec!["INSTANT".into()],
        you_pay: AmountCoin {
            amount: from_amount.to_string(),
            amount_human: format_amount(from_amount, from),
            coin: from.as_str().into(),
        },
        you_receive: AmountCoin {
            amount: quote.to_amount.to_string(),
            amount_human: format_amount(quote.to_amount, to),
            coin: to.as_str().into(),
        },
        min_receive: AmountCoin {
            amount: quote.to_amount.to_string(),
            amount_human: format_amount(quote.to_amount, to),
            coin: to.as_str().into(),
        },
        fees: QuoteFees {
            network: vec![],
            platform: PlatformFee {
                bps: quote.fee_bps,
                amount: quote.fee_amount.to_string(),
                amount_human: fee_human,
                asset: from.as_str().into(),
                label: "Taxa SatsPay".into(),
            },
            total_platform_bps: quote.fee_bps,
        },
        eta_seconds: Some(EtaSeconds {
            inbound: Some(0),
            swap: Some(0),
            outbound: Some(0),
            total: Some(0),
        }),
        tx_hint: None,
        source: "house".into(),
    }
}

fn map_dex_route(client: &swapkit::SwapKitClient, from: Coin, to: Coin, from_amount: u128, route: &swapkit::QuoteRoute) -> Option<QuoteRouteResp> {
    let expected_human = route.expected_buy_amount.as_deref()?;
    let expected = parse_human_to_ledger(expected_human, to)?;
    let min_human = route
        .expected_buy_amount_max_slippage
        .as_deref()
        .unwrap_or(expected_human);
    let min_amt = parse_human_to_ledger(min_human, to).unwrap_or(expected);

    let providers = route.providers.clone();
    let provider = providers.first().cloned().unwrap_or_else(|| "UNKNOWN".into());
    let bps = route
        .platform_fee_bps
        .unwrap_or_else(|| client.platform_fee_bps(&providers, from, to));
    let platform_fee_amount = from_amount.saturating_mul(bps as u128) / 10_000;

    let network: Vec<NetworkFeeItem> = route
        .fees
        .iter()
        .filter(|f| {
            let t = f.fee_type.to_lowercase();
            t != "affiliate" && !t.is_empty()
        })
        .map(|f| NetworkFeeItem {
            fee_type: f.fee_type.clone(),
            amount: f.amount.clone(),
            asset: f.asset.clone(),
        })
        .collect();

    let mut tags = Vec::new();
    if let Some(meta) = &route.meta {
        if let Some(arr) = meta.get("tags").and_then(|t| t.as_array()) {
            for t in arr {
                if let Some(s) = t.as_str() {
                    tags.push(s.to_string());
                }
            }
        }
    }

    Some(QuoteRouteResp {
        route_id: route.route_id.clone(),
        provider,
        providers,
        tags,
        you_pay: AmountCoin {
            amount: from_amount.to_string(),
            amount_human: format_amount(from_amount, from),
            coin: from.as_str().into(),
        },
        you_receive: AmountCoin {
            amount: expected.to_string(),
            amount_human: format_amount(expected, to),
            coin: to.as_str().into(),
        },
        min_receive: AmountCoin {
            amount: min_amt.to_string(),
            amount_human: format_amount(min_amt, to),
            coin: to.as_str().into(),
        },
        fees: QuoteFees {
            network,
            platform: PlatformFee {
                bps,
                amount: platform_fee_amount.to_string(),
                amount_human: format_amount(platform_fee_amount, from),
                asset: from.as_str().into(),
                label: "Taxa SatsPay".into(),
            },
            total_platform_bps: bps,
        },
        eta_seconds: route.estimated_time.as_ref().map(|e| EtaSeconds {
            inbound: e.inbound,
            swap: e.swap,
            outbound: e.outbound,
            total: e.total,
        }),
        tx_hint: route.tx_hint.clone(),
        source: "swapkit".into(),
    })
}

async fn build_quotes<R: AuthRepo>(state: &AppState<R>, from_coin: Coin, to_coin: Coin, from_amount: u128) -> Result<QuoteListResp, String> {
    if from_coin == to_coin {
        return Err("fromCoin and toCoin must differ".into());
    }
    if from_amount == 0 {
        return Err("fromAmount must be > 0".into());
    }
    if !is_swap_l2_pair(from_coin, to_coin) {
        return Err("swap temporarily limited to Polygon L2 pairs (POL, USDT, USDC)".into());
    }

    let mut routes: Vec<QuoteRouteResp> = Vec::new();
    let mut provider_errors: Option<Value> = None;

    let src = resolve_hot_address(from_coin, state.hot_mnemonic.as_deref()).ok();
    let dst = resolve_hot_address(to_coin, state.hot_mnemonic.as_deref()).ok();

    if swapkit::asset_id(from_coin).is_some() && swapkit::asset_id(to_coin).is_some() && state.swapkit.is_configured() {
        match state
            .swapkit
            .quote(from_coin, to_coin, from_amount, src.as_deref(), dst.as_deref())
            .await
        {
            Ok(q) => {
                provider_errors = q.provider_errors;
                let mut mapped: Vec<_> = q
                    .routes
                    .iter()
                    .filter_map(|r| map_dex_route(&state.swapkit, from_coin, to_coin, from_amount, r))
                    .collect();
                mapped.sort_by_key(|r| route_priority(r.tx_hint.as_deref()));
                routes.extend(mapped);
            }
            Err(SwapKitError::NoRoutes) => {}
            Err(e) => {
                tracing::warn!(error = %e, "swapkit quote failed");
                provider_errors = Some(json!([{ "provider": "SWAPKIT", "message": e.to_string() }]));
            }
        }
    }

    // HOUSE only when explicitly enabled (default off — no liquidity capital).
    if house_enabled() {
        match db::swap::quote(&state.pool, from_coin, to_coin, from_amount, state.settings.price_max_stale).await {
            Ok(q) => routes.push(house_route(from_coin, to_coin, from_amount, &q)),
            Err(e) => {
                if routes.is_empty() {
                    return Err(e.to_string());
                }
                tracing::warn!(error = %e, "HOUSE quote unavailable");
            }
        }
    }

    if routes.is_empty() {
        if !state.swapkit.is_configured() {
            return Err("DEX not configured (set SWAPKIT_ENABLED + SWAPKIT_API_KEY)".into());
        }
        return Err("no routes available".into());
    }
    Ok(QuoteListResp { routes, provider_errors })
}

async fn quote_get<R: AuthRepo>(State(state): State<AppState<R>>, Query(q): Query<QuoteQuery>) -> Response {
    let (Ok(from_coin), Ok(to_coin), Ok(from_amount)) = (
        q.from_coin.parse::<Coin>(),
        q.to_coin.parse::<Coin>(),
        q.from_amount.parse::<u128>(),
    ) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid query" }))).into_response();
    };
    match build_quotes(&state, from_coin, to_coin, from_amount).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn quote_post<R: AuthRepo>(State(state): State<AppState<R>>, Json(body): Json<QuoteBody>) -> Response {
    let (Ok(from_coin), Ok(to_coin), Ok(from_amount)) = (
        body.from_coin.parse::<Coin>(),
        body.to_coin.parse::<Coin>(),
        body.from_amount.parse::<u128>(),
    ) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match build_quotes(&state, from_coin, to_coin, from_amount).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteRequest {
    from_coin: String,
    to_coin: String,
    from_amount: String,
    min_to_amount: Option<String>,
    idempotency_key: String,
    route_id: Option<String>,
    provider: Option<String>,
    expected_to_amount: Option<String>,
    platform_fee_bps: Option<u32>,
    source: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteResponse {
    id: String,
    from_amount: String,
    to_amount: String,
    fee_amount: String,
    status: String,
    provider: String,
    source: String,
}

async fn execute<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    user: AuthUser,
    Json(body): Json<ExecuteRequest>,
) -> Response {
    let (Ok(from_coin), Ok(to_coin), Ok(from_amount)) = (
        body.from_coin.parse::<Coin>(),
        body.to_coin.parse::<Coin>(),
        body.from_amount.parse::<u128>(),
    ) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    if let Some(resp) = reject_non_l2(from_coin, to_coin) {
        return resp;
    }
    let min_to_amount = match body.min_to_amount.as_ref().map(|s| s.parse::<u128>()) {
        Some(Ok(v)) => Some(v),
        Some(Err(_)) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid minToAmount" }))).into_response()
        }
        None => None,
    };

    let source = body.source.as_deref().unwrap_or("");
    let provider = body.provider.as_deref().unwrap_or("");
    let is_house = source == "house"
        || provider.eq_ignore_ascii_case("SatsPay Liquidity")
        || provider.eq_ignore_ascii_case("SATSPAY_LIQUIDITY")
        || body.route_id.as_deref().map(|r| r.starts_with("house:")).unwrap_or(false);

    // HOUSE path only when explicitly enabled. Otherwise DEX-only.
    if is_house {
        if !house_enabled() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "HOUSE swap disabled; use a DEX route", "code": "SWAP_HOUSE_DISABLED" })),
            )
                .into_response();
        }
        return execute_house(&state, user, ip, from_coin, to_coin, from_amount, min_to_amount, &body.idempotency_key).await;
    }

    if !state.swapkit.is_configured() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "DEX not configured (set SWAPKIT_ENABLED + SWAPKIT_API_KEY)" })),
        )
            .into_response();
    }

    let route_id = match &body.route_id {
        Some(r) if !r.is_empty() => r.clone(),
        _ => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "routeId required for DEX swap" }))).into_response(),
    };

    let expected_to = match body.expected_to_amount.as_ref().map(|s| s.parse::<u128>()) {
        Some(Ok(v)) => v,
        _ => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": "expectedToAmount required" }))).into_response()
        }
    };

    let bps = body.platform_fee_bps.unwrap_or(state.swapkit.fee_bps_cross());
    let platform_fee_amount = from_amount.saturating_mul(bps as u128) / 10_000;

    let source_address = match resolve_hot_address(from_coin, state.hot_mnemonic.as_deref()) {
        Ok(a) => a,
        Err(e) => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": e }))).into_response(),
    };
    let destination_address = match resolve_hot_address(to_coin, state.hot_mnemonic.as_deref()) {
        Ok(a) => a,
        Err(e) => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": e }))).into_response(),
    };

    // Pre-flight SwapKit /v3/swap (AML + deposit channel). Fail before debit if rejected.
    let swap_resp = match state.swapkit.swap(&route_id, &source_address, &destination_address).await {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": format!("swapkit: {e}") }))).into_response();
        }
    };
    let deposit = match swap_resp.deposit_address() {
        Some(a) => a.to_string(),
        None if swap_resp.tx_hint.as_deref() == Some("contractCall")
            || swap_resp.tx_type.as_deref().map(|t| t.to_lowercase().contains("contract")).unwrap_or(false) =>
        {
            // Contract-call routes need calldata broadcast — mark for worker.
            String::new()
        }
        None => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": "swapkit did not return a deposit address" }))).into_response();
        }
    };

    let providers = if swap_resp.providers.is_empty() {
        vec![provider.to_string()]
    } else {
        swap_resp.providers.clone()
    };
    let primary = providers.first().cloned().unwrap_or_else(|| provider.to_string());
    let fees_json = json!([]);
    let swap_payload = serde_json::to_value(&swap_resp).unwrap_or(json!({}));

    let input = db::dex_swap::LockDexSwapInput {
        user_id: user.id,
        from_coin,
        to_coin,
        from_amount,
        expected_to_amount: expected_to,
        min_to_amount,
        provider: &primary,
        providers: &providers,
        route_id: Some(&route_id),
        quote_id: None,
        platform_fee_bps: bps,
        platform_fee_amount,
        fees_json,
        eta_seconds: None,
        tx_hint: swap_resp.tx_hint.as_deref(),
        destination_address: Some(&destination_address),
        source_address: Some(&source_address),
        swap_payload: Some(swap_payload.clone()),
        idempotency_key: &body.idempotency_key,
    };

    match db::dex_swap::lock_and_create(&state.pool, input).await {
        Ok((row, created)) => {
            if created {
                let _ = db::dex_swap::mark_broadcasting(
                    &state.pool,
                    row.id,
                    if deposit.is_empty() { destination_address.as_str() } else { &deposit },
                    swap_resp.memo_str(),
                    &swap_payload,
                )
                .await;

                if let Err(e) = queue::enqueue(&state.pool, "dex_swap_broadcast", &json!({ "dexSwapId": row.id })).await {
                    tracing::error!(swap_id = %row.id, error = %e, "failed to enqueue dex_swap_broadcast");
                }

                let pool = state.pool.clone();
                let uid = user.id;
                let c_str = from_coin.as_str().to_string();
                let fee_dec = bigdecimal::BigDecimal::from(platform_fee_amount);
                let comm_amt = &fee_dec / bigdecimal::BigDecimal::from(10);
                tokio::spawn(async move {
                    let _ = db::airdrop::award_airdrop_points(&pool, uid, 100, 0, "SWAP_EXECUTE").await;
                    let _ = db::referral::record_referral_commission(
                        &pool,
                        uid,
                        "SWAP_FEE",
                        &c_str,
                        comm_amt,
                        bigdecimal::BigDecimal::from(0),
                    )
                    .await;
                });

                db::audit::record_log_spawned(
                    state.pool.clone(),
                    Some(user.id),
                    "DEX_SWAP_LOCK".into(),
                    "DexSwap".into(),
                    Some(row.id),
                    Some(ip),
                    Some(json!({
                        "fromCoin": from_coin.as_str(),
                        "toCoin": to_coin.as_str(),
                        "provider": primary,
                        "routeId": route_id,
                    })),
                );
            }

            Json(ExecuteResponse {
                id: row.id.to_string(),
                from_amount: row.from_amount,
                to_amount: row.expected_to_amount,
                fee_amount: row.platform_fee_amount,
                status: row.status,
                provider: row.provider,
                source: "swapkit".into(),
            })
            .into_response()
        }
        Err(e) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "DEX_SWAP_FAILED".into(),
                "DexSwap".into(),
                None,
                Some(ip),
                Some(json!({ "reason": e.to_string() })),
            );
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response()
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_house<R: AuthRepo>(
    state: &AppState<R>,
    user: AuthUser,
    ip: String,
    from_coin: Coin,
    to_coin: Coin,
    from_amount: u128,
    min_to_amount: Option<u128>,
    idempotency_key: &str,
) -> Response {
    match db::swap::execute(
        &state.pool,
        user.id,
        from_coin,
        to_coin,
        from_amount,
        min_to_amount,
        idempotency_key,
        state.settings.price_max_stale,
    )
    .await
    {
        Ok((swap, _created)) => {
            let pool = state.pool.clone();
            let uid = user.id;
            let c_str = from_coin.as_str().to_string();
            let fee_dec = bigdecimal::BigDecimal::from(swap.fee_amount);
            let comm_amt = &fee_dec / bigdecimal::BigDecimal::from(10);
            tokio::spawn(async move {
                let _ = db::airdrop::award_airdrop_points(&pool, uid, 100, 0, "SWAP_EXECUTE").await;
                let _ = db::referral::record_referral_commission(
                    &pool,
                    uid,
                    "SWAP_FEE",
                    &c_str,
                    comm_amt,
                    bigdecimal::BigDecimal::from(0),
                )
                .await;
            });
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "SWAP_EXECUTE".into(),
                "Swap".into(),
                Some(swap.id),
                Some(ip),
                Some(json!({
                    "fromCoin": from_coin.as_str(),
                    "toCoin": to_coin.as_str(),
                    "provider": "SatsPay Liquidity",
                })),
            );
            Json(ExecuteResponse {
                id: swap.id.to_string(),
                from_amount: swap.from_amount.to_string(),
                to_amount: swap.to_amount.to_string(),
                fee_amount: swap.fee_amount.to_string(),
                status: "COMPLETED".into(),
                provider: "SatsPay Liquidity".into(),
                source: "house".into(),
            })
            .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swapkit::{EstimatedTime, ProviderFee, QuoteRoute, SwapKitClient};

    fn sample_route() -> QuoteRoute {
        QuoteRoute {
            route_id: "dex-route-1".into(),
            providers: vec!["THORCHAIN".into()],
            sell_asset: Some("BTC.BTC".into()),
            buy_asset: Some("LTC.LTC".into()),
            sell_amount: Some("0.01".into()),
            expected_buy_amount: Some("2.5".into()),
            expected_buy_amount_max_slippage: Some("2.4".into()),
            fees: vec![
                ProviderFee {
                    fee_type: "inbound".into(),
                    amount: "0.0001".into(),
                    asset: "BTC.BTC".into(),
                    chain: None,
                    protocol: None,
                },
                ProviderFee {
                    fee_type: "affiliate".into(),
                    amount: "0.00001".into(),
                    asset: "BTC.BTC".into(),
                    chain: None,
                    protocol: None,
                },
                ProviderFee {
                    fee_type: "".into(),
                    amount: "0".into(),
                    asset: "BTC.BTC".into(),
                    chain: None,
                    protocol: None,
                },
            ],
            estimated_time: Some(EstimatedTime {
                inbound: Some(30),
                swap: Some(60),
                outbound: Some(30),
                total: Some(120),
            }),
            total_slippage_bps: Some(50),
            tx_hint: Some("transferWithMemo".into()),
            meta: Some(json!({ "tags": ["RECOMMENDED", 1, true] })),
            platform_fee_bps: Some(50),
        }
    }

    #[test]
    fn map_dex_route_covers_fees_tags_eta() {
        let client = SwapKitClient::from_env();
        let mapped = map_dex_route(&client, Coin::Btc, Coin::Ltc, 1_000_000, &sample_route()).expect("mapped");
        assert_eq!(mapped.source, "swapkit");
        assert_eq!(mapped.provider, "THORCHAIN");
        assert!(mapped.tags.contains(&"RECOMMENDED".into()));
        assert_eq!(mapped.fees.network.len(), 1);
        assert_eq!(mapped.fees.platform.bps, 50);
        assert_eq!(mapped.eta_seconds.as_ref().unwrap().total, Some(120));
    }

    #[test]
    fn map_dex_route_none_without_expected_amount() {
        let client = SwapKitClient::from_env();
        let mut route = sample_route();
        route.expected_buy_amount = None;
        assert!(map_dex_route(&client, Coin::Btc, Coin::Ltc, 1_000_000, &route).is_none());
    }

    #[test]
    fn map_dex_route_unknown_provider_and_client_bps() {
        let client = SwapKitClient::from_env();
        let mut route = sample_route();
        route.providers.clear();
        route.platform_fee_bps = None;
        route.expected_buy_amount_max_slippage = None;
        let mapped = map_dex_route(&client, Coin::Btc, Coin::Ltc, 2_000_000, &route).unwrap();
        assert_eq!(mapped.provider, "UNKNOWN");
        assert!(mapped.fees.platform.bps > 0);
    }

    #[test]
    fn house_route_shapes_response() {
        let quote = shared::SwapQuote {
            from_amount: 1_000_000,
            to_amount: 50_000_000,
            fee_amount: 5_000,
            fee_bps: 50,
            price_from: 1,
            price_to: 1,
            price_decimals: 8,
        };
        let r = house_route(Coin::Btc, Coin::Ltc, 1_000_000, &quote);
        assert_eq!(r.source, "house");
        assert!(r.route_id.starts_with("house:"));
    }

    #[test]
    fn resolve_hot_address_from_mnemonic_and_missing() {
        use std::sync::Mutex;
        static LOCK: Mutex<()> = Mutex::new(());
        let _g = LOCK.lock().unwrap();
        std::env::remove_var("HOT_MNEMONIC");
        std::env::remove_var("HOT_WALLET_PRIVATE_KEY");
        std::env::remove_var("POL_HOT_WALLET_KEY");
        std::env::remove_var("HOT_WALLET_WIF");
        assert!(resolve_hot_address(Coin::Btc, None).is_err());

        std::env::set_var(
            "HOT_MNEMONIC",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        );
        std::env::set_var("CHAIN_NETWORK", "mainnet");
        assert!(resolve_hot_address(Coin::Btc, None).is_ok());
        std::env::set_var("CHAIN_NETWORK", "testnet");
        let _ = resolve_hot_address(Coin::Btc, None); // exercise testnet branch
        std::env::remove_var("HOT_MNEMONIC");
        std::env::remove_var("CHAIN_NETWORK");
    }
}
