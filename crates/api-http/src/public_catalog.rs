//! Unauthenticated catalogue endpoints a merchant's page can call directly.
//!
//! Everything here is public on purpose: a checkout button, a price label or a
//! coin picker runs in the customer's browser, where no API key may exist. It
//! exposes only facts already visible on the hosted checkout — never anything
//! tied to an account.

use crate::middleware::OptionalAuthUser;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bigdecimal::{BigDecimal, ToPrimitive};
use domain::auth::AuthRepo;
use serde_json::json;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/public/coins", get(coins_handler::<R>))
        // Static segment, so it is matched ahead of `/v1/public/pay/:id`.
        .route("/v1/public/pay/demo", get(demo_invoice_handler::<R>))
        .route("/public/pay/demo", get(demo_invoice_handler::<R>))
        .route("/v1/public/pay/demo/select-coin", post(demo_select_coin_handler::<R>))
        .route("/public/pay/demo/select-coin", post(demo_select_coin_handler::<R>))
}

/// Path under `PUBLIC_BASE_URL` where the vendored coin icons are served.
/// PEPE is PNG: browsers do not paint an image nested inside an SVG used as `<img>`.
pub(crate) fn coin_logo_url(base: &str, symbol: &str) -> String {
    let ext = if symbol.eq_ignore_ascii_case("pepe") { "png" } else { "svg" };
    format!(
        "{}/sdk/coins/{}.{}",
        base.trim_end_matches('/'),
        symbol.to_ascii_lowercase(),
        ext
    )
}

fn logo_url(base: &str, coin: shared::Coin) -> String {
    coin_logo_url(base, coin.as_str())
}

/// Coin catalogue: everything needed to render a payment UI — symbol, name,
/// the ledger scale amounts use, how many confirmations a deposit needs,
/// whether the gateway currently accepts it, an icon served from this domain,
/// and the latest cached price.
///
/// Prices were only reachable at `/v1/swap/prices`, which no merchant would
/// think to call, and icons were not served at all — the checkout pulled them
/// from a third-party CDN.
async fn coins_handler<R: AuthRepo>(State(state): State<AppState<R>>) -> Response {
    let base = state.settings.public_base_url.trim_end_matches('/');
    let (price_decimals, prices) = db::pricing::list_cached_prices(&state.pool)
        .await
        .unwrap_or((8, Default::default()));

    let coins: Vec<serde_json::Value> = shared::COINS
        .into_iter()
        .map(|coin| {
            let cfg = shared::coin_config(coin);
            let paused = shared::is_deposit_withdraw_paused(coin);
            json!({
                "symbol": coin.as_str(),
                "name": cfg.name,
                // Ledger scale: every `amount` in this API is an integer of 1e-8.
                "decimals": cfg.decimals,
                "onchainDecimals": coin.onchain_decimals(),
                "minConfirmations": cfg.min_confirmations,
                "depositsEnabled": !paused,
                "logoUrl": logo_url(base, coin),
                // Scaled by `priceDecimals`; absent from the cache means unpriced.
                "priceUsd": prices.get(&coin).map(|p| p.to_string()),
            })
        })
        .collect();

    Json(json!({
        "priceDecimals": price_decimals,
        "amountDecimals": shared::coin_config(shared::Coin::Btc).decimals,
        "coins": coins,
    }))
    .into_response()
}

/// Deposit address shown for each coin on the demo invoice.
///
/// Deliberately **not spendable**: every one of these is malformed for its
/// network, so a transaction to it cannot be broadcast. The previous demo used
/// `0x71C6705624342490cf03323decB0C392A8892A88` — a well-formed EVM address
/// nobody controls — and its QR was scannable, so anyone ignoring the banner
/// could have burned real coins on a page whose whole point is that nothing is
/// real.
fn demo_address(coin: shared::Coin) -> &'static str {
    // Every one embeds `-DEMO-`. A hyphen and capitals appear in none of the
    // alphabets these networks use — base58, bech32, cashaddr or hex — so each
    // string fails validation everywhere while still reading as an address to
    // a human looking at the screen.
    match coin {
        shared::Coin::Btc => "bc1q-DEMO-nao-envie-nada-para-este-endereco",
        shared::Coin::Ltc => "ltc1q-DEMO-nao-envie-nada-para-este-endereco",
        shared::Coin::Doge => "D-DEMO-nao-envie-nada-para-este-endereco",
        shared::Coin::Bch => "bitcoincash:q-DEMO-nao-envie-nada-para-este-endereco",
        shared::Coin::Dgb => "dgb1q-DEMO-nao-envie-nada-para-este-endereco",
        shared::Coin::Zer => "t1-DEMO-nao-envie-nada-para-este-endereco",
        shared::Coin::Sol => "DEMO-nao-envie-nada-para-este-endereco",
        shared::Coin::Pol | shared::Coin::Usdt | shared::Coin::Usdc | shared::Coin::Pepe => {
            "0x-DEMO-nao-envie-nada-para-este-endereco"
        }
    }
}

/// US$ 25, scaled the way the price cache scales everything.
const DEMO_USD_SCALED: u64 = 25 * 100_000_000;

/// A fake invoice so `/pay/demo` renders the *real* checkout with no money, no
/// database row and no webhook. `/demo` used to be a dead redirect to the
/// user's own deposit page — it advertised a demo and delivered something else.
///
/// It offers every coin, priced from the live cache, so it demonstrates the
/// coin picker instead of hiding it. Signed in, it offers the coins **that
/// merchant** accepts: a merchant who configures three coins and then sees one
/// on the demo concludes, reasonably, that the feature is broken.
async fn demo_invoice_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    OptionalAuthUser(user): OptionalAuthUser,
) -> Response {
    let coins = demo_coins(&state, user.as_ref().map(|u| u.id)).await;
    let usd = BigDecimal::from(DEMO_USD_SCALED);
    let options =
        db::merchant_multicoin::price_options(&state.pool, &coins, &usd, state.settings.price_max_stale).await;

    let selected = options.first();
    demo_payload(&state, selected, &options).into_response()
}

#[derive(serde::Deserialize)]
struct DemoSelectCoinReq {
    coin: String,
}

/// Picking a coin on the demo invoice.
///
/// A static segment, so axum matches it ahead of `/v1/public/pay/:id/...`,
/// whose `Path<Uuid>` would reject `"demo"` outright.
///
/// Writes nothing and derives no address: the demo must never consume an HD
/// index or leave a row behind, however many times someone clicks through it.
async fn demo_select_coin_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    OptionalAuthUser(user): OptionalAuthUser,
    Json(body): Json<DemoSelectCoinReq>,
) -> Response {
    let Ok(coin) = body.coin.parse::<shared::Coin>() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "unknown coin", "code": "UNKNOWN_COIN" })),
        )
            .into_response();
    };

    let coins = demo_coins(&state, user.as_ref().map(|u| u.id)).await;
    let usd = BigDecimal::from(DEMO_USD_SCALED);
    let options =
        db::merchant_multicoin::price_options(&state.pool, &coins, &usd, state.settings.price_max_stale).await;

    let Some(selected) = options.iter().find(|o| o.coin == coin) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "this coin is not accepted for this invoice",
                "code": "COIN_NOT_ACCEPTED",
            })),
        )
            .into_response();
    };

    demo_payload(&state, Some(selected), &options).into_response()
}

/// Coins the demo offers: the signed-in merchant's own list when there is a
/// session, otherwise everything currently live.
async fn demo_coins<R: AuthRepo>(state: &AppState<R>, merchant_id: Option<uuid::Uuid>) -> Vec<shared::Coin> {
    if let Some(id) = merchant_id {
        if let Ok(list) = db::merchant_settings::accepted_coins(&state.pool, id).await {
            if !list.is_empty() {
                return list;
            }
        }
    }
    db::merchant_settings::live_coins(&[])
}

fn demo_payload<R: AuthRepo>(
    state: &AppState<R>,
    selected: Option<&db::merchant_multicoin::CoinOption>,
    options: &[db::merchant_multicoin::CoinOption],
) -> Json<serde_json::Value> {
    let base = state.settings.public_base_url.trim_end_matches('/');
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(30);

    // No coin has a fresh price: show the invoice rather than an error page,
    // with nothing to pick.
    let (coin, amount, display, amount_dec) = match selected {
        Some(o) => (
            o.coin,
            o.amount.to_string(),
            o.amount
                .to_u128()
                .map(|u| shared::format_amount(u, o.coin))
                .unwrap_or_else(|| o.amount.to_string()),
            o.amount.clone(),
        ),
        None => (
            shared::Coin::Usdt,
            DEMO_USD_SCALED.to_string(),
            shared::format_amount(DEMO_USD_SCALED as u128, shared::Coin::Usdt),
            BigDecimal::from(DEMO_USD_SCALED),
        ),
    };
    let address = demo_address(coin);

    let coin_options: Vec<serde_json::Value> = options
        .iter()
        .map(|o| {
            json!({
                "coin": o.coin.as_str(),
                "name": shared::coin_config(o.coin).name,
                "amount": o.amount.to_string(),
                "amountDisplay": o.amount
                    .to_u128()
                    .map(|u| shared::format_amount(u, o.coin))
                    .unwrap_or_else(|| o.amount.to_string()),
                "logoUrl": logo_url(base, o.coin),
                "minConfirmations": shared::coin_config(o.coin).min_confirmations,
            })
        })
        .collect();

    Json(json!({
        "id": "demo",
        "demo": true,
        "status": "PENDING",
        "coin": coin.as_str(),
        "amount": amount,
        "amountDisplay": display,
        "depositAddress": address,
        "orderId": "ORD-DEMO-1",
        "siteName": "Loja de Demonstração",
        "description": "Fatura de demonstração — nenhum pagamento é processado.",
        "customerEmail": serde_json::Value::Null,
        "successUrl": serde_json::Value::Null,
        "cancelUrl": serde_json::Value::Null,
        "qrCode": crate::merchant_deposits::payment_uri(coin.as_str(), address, &amount_dec),
        "expiresAt": expires_at,
        "paidAt": serde_json::Value::Null,
        "txHash": serde_json::Value::Null,
        "logoUrl": logo_url(base, coin),
        "coinOptions": coin_options,
        // Free to switch while there is more than one option — the demo has no
        // money in flight, ever.
        "coinLocked": options.len() <= 1,
        "amountUsd": "25.00",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_url_is_served_from_our_own_domain() {
        let url = logo_url("https://www.satspay.pro", shared::Coin::Usdt);
        assert_eq!(url, "https://www.satspay.pro/sdk/coins/usdt.svg");
        // POL uses its own symbol, not the upstream project's "matic".
        assert_eq!(logo_url("https://x.test", shared::Coin::Pol), "https://x.test/sdk/coins/pol.svg");
    }

    #[test]
    fn every_coin_has_a_vendored_icon() {
        for coin in shared::COINS {
            let file = format!(
                "{}/../../client/public/sdk/coins/{}.svg",
                env!("CARGO_MANIFEST_DIR"),
                coin.as_str().to_lowercase()
            );
            assert!(
                std::path::Path::new(&file).exists(),
                "missing vendored icon for {} ({file})",
                coin.as_str()
            );
        }
    }
}
