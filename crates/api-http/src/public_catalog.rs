//! Unauthenticated catalogue endpoints a merchant's page can call directly.
//!
//! Everything here is public on purpose: a checkout button, a price label or a
//! coin picker runs in the customer's browser, where no API key may exist. It
//! exposes only facts already visible on the hosted checkout — never anything
//! tied to an account.

use crate::state::AppState;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use domain::auth::AuthRepo;
use serde_json::json;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/public/coins", get(coins_handler::<R>))
        // Static segment, so it is matched ahead of `/v1/public/pay/:id`.
        .route("/v1/public/pay/demo", get(demo_invoice_handler::<R>))
        .route("/public/pay/demo", get(demo_invoice_handler::<R>))
}

/// Path under `PUBLIC_BASE_URL` where the vendored coin icons are served.
fn logo_url(base: &str, coin: shared::Coin) -> String {
    format!("{base}/sdk/coins/{}.svg", coin.as_str().to_lowercase())
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

/// A fixed, fake invoice so `/pay/demo` renders the real checkout with no
/// money, no database row and no webhook. `/demo` used to be a dead redirect
/// to the user's own deposit page — it advertised a demo and delivered
/// something else.
async fn demo_invoice_handler<R: AuthRepo>(State(state): State<AppState<R>>) -> Response {
    let coin = shared::Coin::Usdt;
    let units: u128 = 2_500_000_000; // 25 USDT in ledger units
    let amount = units.to_string();
    let display = shared::format_amount(units, coin);
    let address = "0x71C6705624342490cf03323decB0C392A8892A88";
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(30);

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
        "qrCode": format!("{}:{address}?amount={display}", coin.as_str().to_lowercase()),
        "expiresAt": expires_at,
        "paidAt": serde_json::Value::Null,
        "txHash": serde_json::Value::Null,
        "logoUrl": logo_url(state.settings.public_base_url.trim_end_matches('/'), coin),
        "coinOptions": [],
        "coinLocked": true,
        "amountUsd": serde_json::Value::Null,
    }))
    .into_response()
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
