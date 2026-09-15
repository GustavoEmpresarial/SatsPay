//! Multi-coin invoices: the merchant configures what they accept, the paying
//! customer picks how to settle.

mod common;

use api_http::AppState;
use axum::body::Body;
use bigdecimal::BigDecimal;
use db::auth::PgAuthRepo;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;

async fn seed_prices(pool: &PgPool) {
    for (coin, price) in [
        (Coin::Usdt, 100_000_000u64), // US$ 1.00
        (Coin::Usdc, 100_000_000),
        (Coin::Pol, 45_000_000),      // US$ 0.45
        (Coin::Sol, 15_000_000_000),
        (Coin::Bch, 45_000_000_000),
    ] {
        sqlx::query(
            "INSERT INTO price_cache (coin, price_scaled, price_decimals, fetched_at) \
             VALUES ($1::coin, $2, 8, now()) \
             ON CONFLICT (coin) DO UPDATE SET price_scaled = EXCLUDED.price_scaled, fetched_at = now()",
        )
        .bind(coin.as_str())
        .bind(BigDecimal::from(price))
        .execute(pool)
        .await
        .expect("seed price");
    }
}

async fn send(
    state: AppState<PgAuthRepo>,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-real-ip", "203.0.113.210");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let res = api_http::app_without_metrics(state)
        .oneshot(req.body(body.map(|b| Body::from(b.to_string())).unwrap_or(Body::empty())).unwrap())
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn usd_invoice(order: &str) -> serde_json::Value {
    serde_json::json!({
        "amountUsd": "25.00",
        "orderId": order,
        "callbackUrl": "https://merchant.example/hook"
    })
}

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_configures_which_coins_are_accepted(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "mc-settings").await;

    // Never configured: everything currently live, and nothing paused.
    let (st, body) = send(state.clone(), "GET", "/v1/merchant/settings", Some(&token), None).await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    let accepted: Vec<String> = serde_json::from_value(body["acceptedCoins"].clone()).unwrap();
    assert!(accepted.contains(&"USDT".to_string()));
    assert!(!accepted.contains(&"BTC".to_string()), "BTC deposits are paused");

    let (st, body) = send(
        state.clone(),
        "PUT",
        "/v1/merchant/settings",
        Some(&token),
        Some(serde_json::json!({ "acceptedCoins": ["USDT", "POL"] })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    let mut saved: Vec<String> = serde_json::from_value(body["acceptedCoins"].clone()).unwrap();
    saved.sort();
    assert_eq!(saved, vec!["POL".to_string(), "USDT".to_string()]);

    // It survives a reload.
    let (_, body) = send(state.clone(), "GET", "/v1/merchant/settings", Some(&token), None).await;
    let reloaded: Vec<String> = serde_json::from_value(body["acceptedCoins"].clone()).unwrap();
    assert_eq!(reloaded.len(), 2);

    // A selection with nothing payable in it is refused, rather than leaving
    // the merchant with a checkout nobody can use.
    let (st, body) = send(
        state,
        "PUT",
        "/v1/merchant/settings",
        Some(&token),
        Some(serde_json::json!({ "acceptedCoins": ["BTC", "LTC"] })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "NO_USABLE_COIN");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn usd_priced_invoice_offers_every_accepted_coin(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "mc-create").await;
    seed_prices(&pool).await;

    send(
        state.clone(),
        "PUT",
        "/v1/merchant/settings",
        Some(&token),
        Some(serde_json::json!({ "acceptedCoins": ["USDT", "POL"] })),
    )
    .await;

    let (st, created) = send(state.clone(), "POST", "/v1/merchant/deposits", Some(&token), Some(usd_invoice("ORD-USD-1"))).await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    assert_eq!(created["amountUsd"], "25.00");
    assert_eq!(created["coinLocked"], false);
    let accepted: Vec<String> = serde_json::from_value(created["acceptedCoins"].clone()).unwrap();
    assert_eq!(accepted.len(), 2);

    let id = created["id"].as_str().unwrap().to_string();
    let (st, public) = send(state, "GET", &format!("/v1/public/pay/{id}"), None, None).await;
    assert_eq!(st, axum::http::StatusCode::OK, "{public}");

    let options = public["coinOptions"].as_array().expect("coinOptions");
    assert_eq!(options.len(), 2, "{public}");
    let pol = options.iter().find(|o| o["coin"] == "POL").expect("POL offered");
    // US$ 25 at US$ 0.45 = 55.555… POL, rounded up so the merchant is whole.
    assert_eq!(pol["amount"], "5555555556");
    assert_eq!(pol["amountDisplay"], "55.55555556");
    assert!(pol["logoUrl"].as_str().unwrap().ends_with("/sdk/coins/pol.svg"));

    let usdt = options.iter().find(|o| o["coin"] == "USDT").expect("USDT offered");
    assert_eq!(usdt["amount"], "2500000000");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn customer_picks_a_coin_and_the_quote_locks(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "mc-pick").await;
    seed_prices(&pool).await;

    let (_, created) = send(
        state.clone(),
        "POST",
        "/v1/merchant/deposits",
        Some(&token),
        Some(serde_json::json!({
            "amountUsd": "25.00",
            "acceptedCoins": ["USDT", "POL", "SOL"],
            "orderId": "ORD-PICK-1",
            "callbackUrl": "https://merchant.example/hook"
        })),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let (st, picked) = send(
        state.clone(),
        "POST",
        &format!("/v1/public/pay/{id}/select-coin"),
        None,
        Some(serde_json::json!({ "coin": "POL" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{picked}");
    assert_eq!(picked["coin"], "POL");
    assert_eq!(picked["amount"], "5555555556");
    assert!(picked["depositAddress"].as_str().unwrap().starts_with("0x"));
    // The wallet URI must carry the coin quantity, not the ledger integer.
    assert!(picked["qrCode"].as_str().unwrap().contains("amount=55.55555556"), "{picked}");

    // Picking again is idempotent: same address, no second HD index burned.
    let (st, again) = send(
        state.clone(),
        "POST",
        &format!("/v1/public/pay/{id}/select-coin"),
        None,
        Some(serde_json::json!({ "coin": "POL" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(again["depositAddress"], picked["depositAddress"]);

    // Switching keeps the abandoned address on file — money may already be
    // on its way there.
    let (st, switched) = send(
        state.clone(),
        "POST",
        &format!("/v1/public/pay/{id}/select-coin"),
        None,
        Some(serde_json::json!({ "coin": "USDT" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{switched}");
    assert_eq!(switched["coin"], "USDT");
    assert_ne!(switched["depositAddress"], picked["depositAddress"]);

    // One row per coin ever shown. The invoice opened on POL (the accepted
    // list is ordered by `shared::COINS`, not by the order the merchant sent),
    // the customer confirmed POL, then switched to USDT — two coins, two rows.
    let kept = db::merchant_multicoin::list_invoice_addresses(&pool, id.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(kept.len(), 2, "one row per coin ever shown: {kept:?}");

    // The abandoned address survives the switch, carrying its own locked
    // amount — money already sent there has to be honoured.
    let abandoned = kept.iter().find(|a| a.coin == "POL").expect("POL address kept after switching away");
    assert_eq!(abandoned.amount, BigDecimal::from(5_555_555_556u64));
    assert_eq!(abandoned.address, picked["depositAddress"].as_str().unwrap());

    // A coin outside the invoice's own list is refused — the client cannot
    // widen what the merchant agreed to.
    let (st, body) = send(
        state.clone(),
        "POST",
        &format!("/v1/public/pay/{id}/select-coin"),
        None,
        Some(serde_json::json!({ "coin": "USDC" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "COIN_NOT_ACCEPTED");

    // Neither is a paused one.
    let (st, body) = send(
        state,
        "POST",
        &format!("/v1/public/pay/{id}/select-coin"),
        None,
        Some(serde_json::json!({ "coin": "BTC" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "COIN_NOT_ACCEPTED");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn the_coin_freezes_once_money_is_in_flight(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "mc-lock").await;
    seed_prices(&pool).await;

    let (_, created) = send(state.clone(), "POST", "/v1/merchant/deposits", Some(&token), Some(usd_invoice("ORD-LOCK-1"))).await;
    let id: uuid::Uuid = created["id"].as_str().unwrap().parse().unwrap();

    // The watcher locks the coin the moment an address shows a payment.
    db::merchant_multicoin::lock_coin(&pool, id).await.unwrap();

    let (st, body) = send(
        state,
        "POST",
        &format!("/v1/public/pay/{id}/select-coin"),
        None,
        Some(serde_json::json!({ "coin": "POL" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "COIN_LOCKED");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn the_two_pricing_modes_cannot_be_mixed(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "mc-modes").await;
    seed_prices(&pool).await;

    let (st, body) = send(
        state.clone(),
        "POST",
        "/v1/merchant/deposits",
        Some(&token),
        Some(serde_json::json!({
            "coin": "USDT",
            "amount": "2500000000",
            "amountUsd": "25.00",
            "orderId": "ORD-MIX-1",
            "callbackUrl": "https://merchant.example/hook"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "AMBIGUOUS_AMOUNT");

    // Neither mode at all.
    let (st, body) = send(
        state.clone(),
        "POST",
        "/v1/merchant/deposits",
        Some(&token),
        Some(serde_json::json!({ "orderId": "ORD-MIX-2", "callbackUrl": "https://merchant.example/hook" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "INVALID_AMOUNT");

    // amountUsd is fiat, so it IS decimal — the ledger-integer rule must not
    // be applied to it.
    let (st, body) = send(
        state.clone(),
        "POST",
        "/v1/merchant/deposits",
        Some(&token),
        Some(usd_invoice("ORD-MIX-3")),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{body}");

    let (st, body) = send(
        state,
        "POST",
        "/v1/merchant/deposits",
        Some(&token),
        Some(serde_json::json!({ "amountUsd": "abc", "orderId": "ORD-MIX-4", "callbackUrl": "https://merchant.example/hook" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "INVALID_AMOUNT_USD");
}

/// The original contract must keep behaving exactly as before.
#[sqlx::test(migrations = "../db/migrations")]
async fn single_coin_invoices_do_not_regress(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "mc-legacy").await;
    seed_prices(&pool).await;

    let (st, created) = send(
        state.clone(),
        "POST",
        "/v1/merchant/deposits",
        Some(&token),
        Some(serde_json::json!({
            "coin": "USDT",
            "amount": "2500000000",
            "orderId": "ORD-LEGACY-1",
            "callbackUrl": "https://merchant.example/hook"
        })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    assert_eq!(created["coin"], "USDT");
    assert_eq!(created["amount"], "2500000000");
    assert_eq!(created["feeAmount"], "6250000");
    assert_eq!(created["coinLocked"], true, "a crypto-priced invoice is final at birth");
    assert!(created["amountUsd"].is_null());

    let id = created["id"].as_str().unwrap().to_string();
    let (_, public) = send(state.clone(), "GET", &format!("/v1/public/pay/{id}"), None, None).await;
    assert_eq!(public["coinOptions"].as_array().unwrap().len(), 0, "no picker on a single-coin invoice");
    assert_eq!(public["coinLocked"], true);

    // And it cannot be switched.
    let (st, body) = send(
        state,
        "POST",
        &format!("/v1/public/pay/{id}/select-coin"),
        None,
        Some(serde_json::json!({ "coin": "POL" })),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT, "{body}");
}
