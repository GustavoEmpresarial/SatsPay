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

async fn seed_demo_prices(pool: &PgPool) {
    for (coin, price) in [
        (shared::Coin::Usdt, 100_000_000u64),
        (shared::Coin::Usdc, 100_000_000),
        (shared::Coin::Pol, 45_000_000),
        (shared::Coin::Sol, 15_000_000_000),
        (shared::Coin::Bch, 45_000_000_000),
        (shared::Coin::Zer, 1_000_000),
        (shared::Coin::Pepe, 400),
    ] {
        sqlx::query(
            "INSERT INTO price_cache (coin, price_scaled, price_decimals, fetched_at) \
             VALUES ($1::coin, $2, 8, now()) \
             ON CONFLICT (coin) DO UPDATE SET price_scaled = EXCLUDED.price_scaled, fetched_at = now()",
        )
        .bind(coin.as_str())
        .bind(bigdecimal::BigDecimal::from(price))
        .execute(pool)
        .await
        .expect("seed price");
    }
}

/// The demo used to be hardcoded to one coin with `coinOptions: []`, so a
/// merchant who accepted several of them opened it, saw one, and concluded the
/// picker did not work.
#[sqlx::test(migrations = "../db/migrations")]
async fn demo_offers_every_coin_so_the_picker_is_visible(pool: PgPool) {
    seed_demo_prices(&pool).await;
    let (st, body) = get_json(pool, "/v1/public/pay/demo").await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    assert_eq!(body["demo"], true);
    assert_eq!(body["coinLocked"], false, "the demo must let the customer choose: {body}");
    assert_eq!(body["amountUsd"], "25.00");

    let options = body["coinOptions"].as_array().expect("coinOptions");
    assert!(options.len() > 1, "a picker needs more than one option: {body}");

    for o in options {
        assert!(o["amount"].as_str().is_some_and(|a| !a.is_empty()));
        assert!(o["amountDisplay"].as_str().is_some_and(|a| a != "0"));
        assert!(o["logoUrl"].as_str().unwrap().starts_with("https://www.satspay.pro/sdk/coins/"));
    }

    // US$ 25 at US$ 0.45 — the same conversion a real invoice does.
    let pol = options.iter().find(|o| o["coin"] == "POL").expect("POL offered");
    assert_eq!(pol["amount"], "5555555556");
}

/// The literal complaint: configure several coins, open the demo, see one.
#[sqlx::test(migrations = "../db/migrations")]
async fn demo_shows_the_signed_in_merchants_own_coins(pool: PgPool) {
    seed_demo_prices(&pool).await;
    let (state, token, _, _) = common::register_user(pool.clone(), "demo-merch").await;

    let put = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/v1/merchant/settings")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.200")
                .body(Body::from(serde_json::json!({ "acceptedCoins": ["USDT", "POL"] }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(put.status().is_success());

    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/public/pay/demo")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.200")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let mut coins: Vec<String> = body["coinOptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["coin"].as_str().unwrap().to_string())
        .collect();
    coins.sort();
    assert_eq!(coins, vec!["POL".to_string(), "USDT".to_string()], "{body}");
}

/// Clicking through the demo must never consume an HD index or leave a row.
#[sqlx::test(migrations = "../db/migrations")]
async fn choosing_on_the_demo_writes_nothing(pool: PgPool) {
    seed_demo_prices(&pool).await;

    let count = |p: PgPool, table: &'static str| async move {
        sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&p)
            .await
            .unwrap()
    };
    let before_inv = count(pool.clone(), "merchant_deposit_invoices").await;
    let before_addr = count(pool.clone(), "merchant_invoice_addresses").await;

    let state = common::test_state(pool.clone());
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/public/pay/demo/select-coin")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.200")
                .body(Body::from(serde_json::json!({ "coin": "POL" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["coin"], "POL");
    assert_eq!(body["amount"], "5555555556");
    assert_eq!(body["demo"], true);

    assert_eq!(count(pool.clone(), "merchant_deposit_invoices").await, before_inv, "demo created an invoice");
    assert_eq!(count(pool, "merchant_invoice_addresses").await, before_addr, "demo burned an address");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn the_demo_refuses_a_coin_it_does_not_offer(pool: PgPool) {
    seed_demo_prices(&pool).await;
    let state = common::test_state(pool);
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/public/pay/demo/select-coin")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.200")
                // Paused, so it is never offered.
                .body(Body::from(serde_json::json!({ "coin": "BTC" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "COIN_NOT_ACCEPTED");
}

/// The demo QR is scannable. If its address were valid, someone ignoring the
/// banner could send real coins to an address nobody controls.
#[sqlx::test(migrations = "../db/migrations")]
async fn demo_addresses_cannot_receive_money(pool: PgPool) {
    seed_demo_prices(&pool).await;
    let state = common::test_state(pool.clone());
    let registry = state.chain_registry.clone();

    for coin in shared::COINS {
        let res = api_http::app_without_metrics(common::test_state(pool.clone()))
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/public/pay/demo/select-coin")
                    .header("content-type", "application/json")
                    .header("x-real-ip", "203.0.113.200")
                    .body(Body::from(serde_json::json!({ "coin": coin.as_str() }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let Some(address) = body["depositAddress"].as_str() else { continue };

        assert!(
            !registry.get(coin).validate_address(address),
            "{} demo address {address} is a valid address — money sent to it would be lost",
            coin.as_str()
        );
    }
}

/// The static `demo` segment must not shadow real invoice lookups.
/// The static `demo` segment must not shadow real invoice lookups.
#[sqlx::test(migrations = "../db/migrations")]
async fn a_real_invoice_id_still_resolves_normally(pool: PgPool) {
    let (st, body) = get_json(pool, "/v1/public/pay/550e8400-e29b-41d4-a716-446655440000").await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "INVOICE_NOT_FOUND");
}
