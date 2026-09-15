//! Public catalogue + demo checkout: everything a merchant's page or a
//! customer's browser can call with no credentials at all.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

async fn get_json(pool: PgPool, uri: &str) -> (axum::http::StatusCode, serde_json::Value) {
    let state = common::test_state(pool);
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .header("x-real-ip", "203.0.113.200")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

/// Prices were only reachable at `/v1/swap/prices` and icons were not served
/// at all, so a merchant had no way to render a coin.
#[sqlx::test(migrations = "../db/migrations")]
async fn coins_catalogue_is_public_and_complete(pool: PgPool) {
    let (st, body) = get_json(pool, "/v1/public/coins").await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let coins = body["coins"].as_array().expect("coins array");
    assert_eq!(coins.len(), shared::COINS.len());
    assert_eq!(body["amountDecimals"], 8, "amounts are integers of 1e-8");

    for coin in shared::COINS {
        let entry = coins
            .iter()
            .find(|c| c["symbol"] == coin.as_str())
            .unwrap_or_else(|| panic!("missing {}", coin.as_str()));
        assert!(entry["name"].as_str().is_some_and(|n| !n.is_empty()));
        assert_eq!(entry["decimals"], 8);
        assert!(entry["minConfirmations"].as_u64().is_some());
        // The icon must come from this platform, not a third-party CDN.
        let logo = entry["logoUrl"].as_str().expect("logoUrl");
        assert!(logo.starts_with("https://www.satspay.pro/sdk/coins/"), "{logo}");
        assert!(logo.ends_with(&format!("{}.svg", coin.as_str().to_lowercase())), "{logo}");
        // Paused coins are advertised as such rather than silently failing later.
        assert_eq!(
            entry["depositsEnabled"].as_bool(),
            Some(!shared::is_deposit_withdraw_paused(coin)),
            "{} depositsEnabled",
            coin.as_str()
        );
    }
}

/// `/demo` used to redirect to the user's own deposit page. It now renders a
/// real checkout backed by a synthetic invoice: no row, no money, no webhook.
#[sqlx::test(migrations = "../db/migrations")]
async fn demo_invoice_renders_a_real_checkout_without_money(pool: PgPool) {
    let (st, body) = get_json(pool, "/v1/public/pay/demo").await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    assert_eq!(body["demo"], true, "the page must be able to label it a demo");
    assert_eq!(body["id"], "demo");
    assert_eq!(body["status"], "PENDING");
    assert_eq!(body["coin"], "USDT");
    // Ledger integer for the API, coin quantity for the human.
    assert_eq!(body["amount"], "2500000000");
    assert_eq!(body["amountDisplay"], "25");
    // A wallet scanning this must be asked for 25 USDT, not 2.5 billion.
    assert_eq!(body["qrCode"], "usdt:0x71C6705624342490cf03323decB0C392A8892A88?amount=25");
    assert!(body["expiresAt"].as_str().is_some());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn demo_alias_without_v1_also_works(pool: PgPool) {
    let (st, body) = get_json(pool, "/public/pay/demo").await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["demo"], true);
}

/// The static `demo` segment must not shadow real invoice lookups.
#[sqlx::test(migrations = "../db/migrations")]
async fn a_real_invoice_id_still_resolves_normally(pool: PgPool) {
    let (st, body) = get_json(pool, "/v1/public/pay/550e8400-e29b-41d4-a716-446655440000").await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "INVOICE_NOT_FOUND");
}
