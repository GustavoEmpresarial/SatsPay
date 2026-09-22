//! Swap routes — custodial DEX (SwapKit), Relay bridge, and ChangeNOW L1.
//! HOUSE liquidity is off unless `SWAP_HOUSE_ENABLED=true`.

use crate::client_ip::ClientIp;
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use changenow::ChangeNowEstimate;
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::{format_amount, is_swap_pair, Coin};
use relay::{RelayClient, RelayQuote};
use swapkit::{parse_human_to_ledger, route_priority, SwapKitClient, SwapKitError};
use uuid::Uuid;

fn house_enabled() -> bool {
    std::env::var("SWAP_HOUSE_ENABLED")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn reject_unsupported_pair(from: Coin, to: Coin) -> Option<Response> {
    if is_swap_pair(from, to) {
        return None;
    }
    Some(
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "swap limited to L1 (BTC, LTC, DOGE, BCH, DGB) via ChangeNOW and Polygon L2 (POL, USDT, USDC) + SOL",
                "code": "SWAP_PAIR_UNSUPPORTED",
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
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn telemetry<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    if let Err(r) = crate::middleware::require_admin(&user) {
        return *r;
    }
    match db::dex_swap::telemetry_snapshot(&state.pool).await {
        Ok(t) => Json(t).into_response(),
        Err(e) => crate::http_error::internal_error(&e),
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
        Err(e) => crate::http_error::internal_error(&e),
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

fn map_relay_route(
    client: &SwapKitClient,
    from: Coin,
    to: Coin,
    from_amount: u128,
    quote: &RelayQuote,
) -> QuoteRouteResp {
    let bps = client.platform_fee_bps(&["RELAY".into()], from, to);
    let platform_fee_amount = from_amount.saturating_mul(bps as u128) / 10_000;
    let expected = quote.expected_out_ledger;
    let mut tags = vec!["RELAY".to_string()];
    if quote.is_bridge {
        tags.push("BRIDGE".into());
    }
    QuoteRouteResp {
        route_id: quote.route_id.clone(),
        provider: "RELAY".into(),
        providers: vec!["RELAY".into()],
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
            amount: expected.to_string(),
            amount_human: format_amount(expected, to),
            coin: to.as_str().into(),
        },
        fees: QuoteFees {
            network: quote
                .network_fees
                .iter()
                .map(|f| NetworkFeeItem {
                    fee_type: f.fee_type.clone(),
                    amount: f.amount.clone(),
                    asset: f.asset.clone(),
                })
                .collect(),
            platform: PlatformFee {
                bps,
                amount: platform_fee_amount.to_string(),
                amount_human: format_amount(platform_fee_amount, from),
                asset: from.as_str().into(),
                label: "Taxa SatsPay".into(),
            },
            total_platform_bps: bps,
        },
        eta_seconds: Some(EtaSeconds {
            inbound: Some(0),
            swap: Some(if quote.is_bridge { 60 } else { 30 }),
            outbound: Some(if quote.is_bridge { 30 } else { 0 }),
            total: Some(if quote.is_bridge { 90 } else { 30 }),
        }),
        tx_hint: Some(quote.tx_hint.into()),
        source: "relay".into(),
    }
}

fn map_changenow_route(
    client: &SwapKitClient,
    from: Coin,
    to: Coin,
    from_amount: u128,
    estimate: &ChangeNowEstimate,
) -> Option<QuoteRouteResp> {
    let bps = client.platform_fee_bps(&["CHANGENOW".into()], from, to);
    let platform_fee_amount = from_amount.saturating_mul(bps as u128) / 10_000;
    // Single SatsPay cut on destination (CN already embeds deposit/spread).
    let cn_out = estimate.to_amount_ledger;
    let expected = cn_out
        .saturating_sub(cn_out.saturating_mul(bps as u128) / 10_000)
        .max(1);

    // Fixed ChangeNOW deposit fees destroy dust swaps (e.g. 0.0006 SOL fee on 0.0014 SOL).
    // Refuse the route when deposit fee alone exceeds 15% of input.
    if let Some(fee_h) = estimate.deposit_fee_human.as_deref() {
        if let Some(fee_l) = changenow::human_to_ledger(fee_h, from) {
            if from_amount > 0 && fee_l.saturating_mul(100) / from_amount > 15 {
                tracing::info!(
                    %from_amount,
                    fee = fee_l,
                    from = %from.as_str(),
                    to = %to.as_str(),
                    "changenow route skipped: deposit fee >15% of input"
                );
                return None;
            }
        }
    }

    let mut network = Vec::new();
    if let Some(fee) = &estimate.deposit_fee_human {
        network.push(NetworkFeeItem {
            fee_type: "deposit".into(),
            amount: fee.clone(),
            asset: from.as_str().into(),
        });
    }
    Some(QuoteRouteResp {
        route_id: format!(
            "changenow:{}:{}:{}",
            from.as_str(),
            to.as_str(),
            from_amount
        ),
        provider: "CHANGENOW".into(),
        providers: vec!["CHANGENOW".into()],
        tags: vec!["CHANGENOW".into(), "L1".into()],
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
            amount: expected.to_string(),
            amount_human: format_amount(expected, to),
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
        eta_seconds: Some(EtaSeconds {
            inbound: Some(600),
            swap: Some(300),
            outbound: Some(600),
            total: Some(1_800),
        }),
        tx_hint: Some("simpleTransfer".into()),
        source: "changenow".into(),
    })
}

fn merge_provider_errors(existing: Option<Value>, provider: &str, message: String) -> Value {
    match existing {
        Some(Value::Array(mut arr)) => {
            arr.push(json!({ "provider": provider, "message": message }));
            Value::Array(arr)
        }
        Some(other) => json!([other, { "provider": provider, "message": message }]),
        None => json!([{ "provider": provider, "message": message }]),
    }
}

async fn build_quotes<R: AuthRepo>(state: &AppState<R>, from_coin: Coin, to_coin: Coin, from_amount: u128) -> Result<QuoteListResp, String> {
    if from_coin == to_coin {
        return Err("fromCoin and toCoin must differ".into());
    }
    if from_amount == 0 {
        return Err("fromAmount must be > 0".into());
    }
    if !is_swap_pair(from_coin, to_coin) {
        return Err(
            "swap limited to L1 (BTC, LTC, DOGE, BCH, DGB) via ChangeNOW and Polygon L2 (POL, USDT, USDC) + SOL"
                .into(),
        );
    }

    let mut routes: Vec<QuoteRouteResp> = Vec::new();
    let mut provider_errors: Option<Value> = None;

    let src = resolve_hot_address(from_coin, state.hot_mnemonic.as_deref()).ok();
    let dst = resolve_hot_address(to_coin, state.hot_mnemonic.as_deref()).ok();

    // SwapKit: Polygon same-chain only (do not mix SOL bridge into SwapKit).
    if relay::is_polygon_l2(from_coin)
        && relay::is_polygon_l2(to_coin)
        && swapkit::asset_id(from_coin).is_some()
        && swapkit::asset_id(to_coin).is_some()
        && state.swapkit.is_configured()
    {
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

    if RelayClient::supports_pair(from_coin, to_coin) && state.relay.is_configured() {
        if let (Some(src), Some(dst)) = (src.as_deref(), dst.as_deref()) {
            match state.relay.quote(from_coin, to_coin, from_amount, src, dst).await {
                Ok(q) => routes.push(map_relay_route(&state.swapkit, from_coin, to_coin, from_amount, &q)),
                Err(e) => {
                    tracing::warn!(error = %e, "relay quote failed");
                    provider_errors = Some(merge_provider_errors(
                        provider_errors,
                        "RELAY",
                        e.to_string(),
                    ));
                }
            }
        } else {
            provider_errors = Some(merge_provider_errors(
                provider_errors,
                "RELAY",
                "hot wallet address unavailable for relay quote".into(),
            ));
        }
    }

    // ChangeNOW: any mapped pair (L1↔L1, L1↔L2/SOL, and L2 fallback).
    if changenow::supports_pair(from_coin, to_coin) && state.changenow.is_configured() {
        match state.changenow.estimate(from_coin, to_coin, from_amount).await {
            Ok(est) => {
                if let Some(route) =
                    map_changenow_route(&state.swapkit, from_coin, to_coin, from_amount, &est)
                {
                    routes.push(route);
                } else {
                    provider_errors = Some(merge_provider_errors(
                        provider_errors,
                        "CHANGENOW",
                        "valor pequeno demais: taxa fixa de depósito do ChangeNOW >15% do envio".into(),
                    ));
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "changenow estimate failed");
                provider_errors = Some(merge_provider_errors(
                    provider_errors,
                    "CHANGENOW",
                    e.to_string(),
                ));
            }
        }
    }

    routes.sort_by_key(|r| route_priority(r.tx_hint.as_deref()));

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
        if !state.swapkit.is_configured()
            && !state.relay.is_configured()
            && !state.changenow.is_configured()
        {
            return Err(
                "DEX not configured (set SWAPKIT_ENABLED, RELAY_ENABLED, and/or CHANGENOW_ENABLED)"
                    .into(),
            );
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
    if let Some(resp) = reject_unsupported_pair(from_coin, to_coin) {
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

    let is_relay = source == "relay"
        || provider.eq_ignore_ascii_case("RELAY")
        || body.route_id.as_deref().map(|r| r.starts_with("relay:")).unwrap_or(false);
    if is_relay {
        return execute_relay(
            &state,
            user,
            ip,
            from_coin,
            to_coin,
            from_amount,
            min_to_amount,
            &body,
        )
        .await;
    }

    let is_changenow = source == "changenow"
        || provider.eq_ignore_ascii_case("CHANGENOW")
        || body
            .route_id
            .as_deref()
            .map(|r| r.starts_with("changenow:"))
            .unwrap_or(false);
    if is_changenow {
        return execute_changenow(
            &state,
            user,
            ip,
            from_coin,
            to_coin,
            from_amount,
            min_to_amount,
            &body,
        )
        .await;
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
            finalize_dex_lock(
                &state,
                user,
                ip,
                from_coin,
                &route_id,
                &primary,
                row,
                created,
                &deposit,
                &destination_address,
                swap_resp.memo_str(),
                &swap_payload,
                platform_fee_amount,
                "swapkit",
            )
            .await
        }
        Err(e) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "DEX_SWAP_FAILED".into(),
                "DexSwap".into(),
                None,
                Some(state.secrets.ip_fingerprint(&ip)),
                Some(json!({ "reason": e.to_string() })),
            );
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response()
        }
    }
}

async fn finalize_dex_lock<R: AuthRepo>(
    state: &AppState<R>,
    user: AuthUser,
    ip: String,
    from_coin: Coin,
    route_id: &str,
    primary: &str,
    row: db::dex_swap::DexSwapRow,
    created: bool,
    deposit: &str,
    destination_address: &str,
    memo: Option<&str>,
    swap_payload: &Value,
    platform_fee_amount: u128,
    source_label: &str,
) -> Response {
    if created {
        let _ = db::dex_swap::mark_broadcasting(
            &state.pool,
            row.id,
            if deposit.is_empty() {
                destination_address
            } else {
                deposit
            },
            memo,
            swap_payload,
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
        let amount_usd = db::pricing::usd_from_ledger_amount(&state.pool, from_coin, &comm_amt).await;
        match db::airdrop::award_airdrop_points(&pool, uid, 100, 0, "SWAP_EXECUTE").await {
            Ok(db::airdrop::AwardResult::Awarded) => {}
            Ok(db::airdrop::AwardResult::NoActiveSeason) => {
                tracing::info!(user_id = %uid, "swap: airdrop season inactive, points not awarded");
            }
            Err(e) => {
                tracing::warn!(user_id = %uid, error = %e, "airdrop SWAP_EXECUTE award failed");
            }
        }
        if let Err(e) = db::referral::record_referral_commission(
            &pool,
            uid,
            "SWAP_FEE",
            &c_str,
            comm_amt,
            amount_usd,
        )
        .await
        {
            tracing::warn!(user_id = %uid, error = %e, "referral commission on swap failed");
        }

        db::audit::record_log_spawned(
            state.pool.clone(),
            Some(user.id),
            "DEX_SWAP_LOCK".into(),
            "DexSwap".into(),
            Some(row.id),
            Some(state.secrets.ip_fingerprint(&ip)),
            Some(json!({
                "fromCoin": from_coin.as_str(),
                "provider": primary,
                "routeId": route_id,
                "source": source_label,
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
        source: source_label.to_string(),
    })
    .into_response()
}

#[allow(clippy::too_many_arguments)]
async fn execute_relay<R: AuthRepo>(
    state: &AppState<R>,
    user: AuthUser,
    ip: String,
    from_coin: Coin,
    to_coin: Coin,
    from_amount: u128,
    min_to_amount: Option<u128>,
    body: &ExecuteRequest,
) -> Response {
    if !state.relay.is_configured() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "Relay not configured (set RELAY_ENABLED=true)" })),
        )
            .into_response();
    }

    let _route_id = match &body.route_id {
        Some(r) if !r.is_empty() => r.clone(),
        _ => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "routeId required for Relay swap" }))).into_response(),
    };

    let source_address = match resolve_hot_address(from_coin, state.hot_mnemonic.as_deref()) {
        Ok(a) => a,
        Err(e) => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": e }))).into_response(),
    };
    let destination_address = match resolve_hot_address(to_coin, state.hot_mnemonic.as_deref()) {
        Ok(a) => a,
        Err(e) => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": e }))).into_response(),
    };

    let relay_quote = match state
        .relay
        .quote(from_coin, to_coin, from_amount, &source_address, &destination_address)
        .await
    {
        Ok(q) => q,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": format!("relay: {e}") }))).into_response()
        }
    };
    // Fresh quote always gets a new requestId — keep client's routeId only as selection
    // marker; persist the live quote's route_id / requestId for the worker.
    let live_route_id = relay_quote.route_id.clone();

    let expected_to = relay_quote.expected_out_ledger;
    if let Some(min) = min_to_amount {
        if expected_to < min {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "relay output below minToAmount", "code": "SLIPPAGE" })),
            )
                .into_response();
        }
    }

    let bps = body
        .platform_fee_bps
        .unwrap_or_else(|| state.swapkit.platform_fee_bps(&["RELAY".into()], from_coin, to_coin));
    let platform_fee_amount = from_amount.saturating_mul(bps as u128) / 10_000;
    let swap_payload = relay_quote.to_swap_payload();
    let providers = vec!["RELAY".to_string()];
    let fees_json = json!([]);
    let tx_hint = relay_quote.tx_hint;
    let eta = if relay_quote.is_bridge { Some(90) } else { Some(30) };

    let input = db::dex_swap::LockDexSwapInput {
        user_id: user.id,
        from_coin,
        to_coin,
        from_amount,
        expected_to_amount: expected_to,
        min_to_amount,
        provider: "RELAY",
        providers: &providers,
        route_id: Some(&live_route_id),
        quote_id: relay_quote.request_id.as_deref(),
        platform_fee_bps: bps,
        platform_fee_amount,
        fees_json,
        eta_seconds: eta,
        tx_hint: Some(tx_hint),
        destination_address: Some(&destination_address),
        source_address: Some(&source_address),
        swap_payload: Some(swap_payload.clone()),
        idempotency_key: &body.idempotency_key,
    };

    match db::dex_swap::lock_and_create(&state.pool, input).await {
        Ok((row, created)) => {
            finalize_dex_lock(
                state,
                user,
                ip,
                from_coin,
                &live_route_id,
                "RELAY",
                row,
                created,
                "",
                &destination_address,
                None,
                &swap_payload,
                platform_fee_amount,
                "relay",
            )
            .await
        }
        Err(e) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "DEX_SWAP_FAILED".into(),
                "DexSwap".into(),
                None,
                Some(state.secrets.ip_fingerprint(&ip)),
                Some(json!({ "reason": e.to_string(), "source": "relay" })),
            );
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response()
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_changenow<R: AuthRepo>(
    state: &AppState<R>,
    user: AuthUser,
    ip: String,
    from_coin: Coin,
    to_coin: Coin,
    from_amount: u128,
    min_to_amount: Option<u128>,
    body: &ExecuteRequest,
) -> Response {
    if !state.changenow.is_configured() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "ChangeNOW not configured (set CHANGENOW_ENABLED + CHANGENOW_API_KEY)" })),
        )
            .into_response();
    }
    if !changenow::supports_pair(from_coin, to_coin) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "pair not supported on ChangeNOW" })),
        )
            .into_response();
    }

    let source_address = match resolve_hot_address(from_coin, state.hot_mnemonic.as_deref()) {
        Ok(a) => a,
        Err(e) => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": e }))).into_response(),
    };
    let destination_address = match resolve_hot_address(to_coin, state.hot_mnemonic.as_deref()) {
        Ok(a) => a,
        Err(e) => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": e }))).into_response(),
    };

    let exchange = match state
        .changenow
        .create_exchange(
            from_coin,
            to_coin,
            from_amount,
            &destination_address,
            &source_address,
        )
        .await
    {
        Ok(e) => e,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": format!("changenow: {e}") })))
                .into_response()
        }
    };

    let bps = body
        .platform_fee_bps
        .unwrap_or_else(|| state.swapkit.platform_fee_bps(&["CHANGENOW".into()], from_coin, to_coin));
    let platform_fee_amount = from_amount.saturating_mul(bps as u128) / 10_000;

    let mut expected_to = if exchange.to_amount_ledger > 0 {
        exchange.to_amount_ledger
    } else {
        body.expected_to_amount
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    let cut = expected_to.saturating_mul(bps as u128) / 10_000;
    expected_to = expected_to.saturating_sub(cut);
    if expected_to == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "changenow estimated output too small" })),
        )
            .into_response();
    }
    if let Some(min) = min_to_amount {
        if expected_to < min {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "changenow output below minToAmount", "code": "SLIPPAGE" })),
            )
                .into_response();
        }
    }

    if exchange.payin_extra_id.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "changenow requires memo/extraId for this pair — not supported yet",
                "code": "CHANGENOW_MEMO_UNSUPPORTED",
            })),
        )
            .into_response();
    }

    let live_route_id = exchange.route_id();
    let deposit = exchange.payin_address.clone();
    let swap_payload = exchange.to_swap_payload();
    let providers = vec!["CHANGENOW".to_string()];
    let fees_json = json!([]);

    let input = db::dex_swap::LockDexSwapInput {
        user_id: user.id,
        from_coin,
        to_coin,
        from_amount,
        expected_to_amount: expected_to,
        min_to_amount,
        provider: "CHANGENOW",
        providers: &providers,
        route_id: Some(&live_route_id),
        quote_id: Some(exchange.id.as_str()),
        platform_fee_bps: bps,
        platform_fee_amount,
        fees_json,
        eta_seconds: Some(1_800),
        tx_hint: Some("simpleTransfer"),
        destination_address: Some(&destination_address),
        source_address: Some(&source_address),
        swap_payload: Some(swap_payload.clone()),
        idempotency_key: &body.idempotency_key,
    };

    match db::dex_swap::lock_and_create(&state.pool, input).await {
        Ok((row, created)) => {
            finalize_dex_lock(
                state,
                user,
                ip,
                from_coin,
                &live_route_id,
                "CHANGENOW",
                row,
                created,
                &deposit,
                &destination_address,
                None,
                &swap_payload,
                platform_fee_amount,
                "changenow",
            )
            .await
        }
        Err(e) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "DEX_SWAP_FAILED".into(),
                "DexSwap".into(),
                None,
                Some(state.secrets.ip_fingerprint(&ip)),
                Some(json!({ "reason": e.to_string(), "source": "changenow" })),
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
            let amount_usd = db::pricing::usd_from_ledger_amount(&state.pool, from_coin, &comm_amt).await;
            match db::airdrop::award_airdrop_points(&pool, uid, 100, 0, "SWAP_EXECUTE").await {
                Ok(db::airdrop::AwardResult::Awarded) => {}
                Ok(db::airdrop::AwardResult::NoActiveSeason) => {
                    tracing::info!(user_id = %uid, "house swap: airdrop season inactive, points not awarded");
                }
                Err(e) => {
                    tracing::warn!(user_id = %uid, error = %e, "airdrop SWAP_EXECUTE award failed");
                }
            }
            if let Err(e) = db::referral::record_referral_commission(
                &pool,
                uid,
                "SWAP_FEE",
                &c_str,
                comm_amt,
                amount_usd,
            )
            .await
            {
                tracing::warn!(user_id = %uid, error = %e, "referral commission on swap failed");
            }
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "SWAP_EXECUTE".into(),
                "Swap".into(),
                Some(swap.id),
                Some(state.secrets.ip_fingerprint(&ip)),
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
