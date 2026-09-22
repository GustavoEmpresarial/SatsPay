//! Multi-coin checkout: which coins get offered, one address per (invoice, coin),
//! and the lock that stops a customer switching coins under a payment in flight.

mod common;

use bigdecimal::BigDecimal;
use db::merchant_deposits as inv_db;
use db::merchant_multicoin as mc;
use shared::Coin;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

const FRESH: Duration = Duration::from_secs(600);

async fn invoice(pool: &PgPool, prefix: &str) -> inv_db::MerchantDepositInvoice {
    let merchant = common::insert_user(pool, prefix).await;
    inv_db::create_invoice(
        pool,
        inv_db::CreateDepositInvoiceInput {
            merchant_id: merchant,
            api_key_id: None,
            site_user_id: None,
            order_id: format!("ORD-{}", Uuid::new_v4().simple()),
            site_name: None,
            coin: Coin::Usdt,
            amount: BigDecimal::from(2_500_000_000u64),
            deposit_address: format!("0x{}", Uuid::new_v4().simple()),
            hd_index: Some(1),
            callback_url: "https://merchant.example/hook".into(),
            success_url: None,
            cancel_url: None,
            customer_email: None,
            customer_name: None,
            description: None,
            expiry_minutes: Some(60),
            accepted_coins: vec![Coin::Usdt, Coin::Pol, Coin::Sol],
            price_usd_scaled: Some(BigDecimal::from(2_500_000_000u64)),
            price_decimals: Some(8),
            quote_price_scaled: None,
        },
    )
    .await
    .unwrap()
}

async fn address(pool: &PgPool, inv: Uuid, coin: Coin, addr: &str, amount: u64) -> mc::InvoiceAddress {
    mc::upsert_invoice_address(pool, inv, coin, addr, Some(7), &BigDecimal::from(amount), Some(&BigDecimal::from(45_000_000u64)))
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn only_freshly_priced_coins_are_offered(pool: PgPool) {
    common::seed_price_cache(&pool).await;
    sqlx::query("UPDATE price_cache SET fetched_at = now() - interval '2 hours' WHERE coin = 'SOL'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM price_cache WHERE coin = 'LTC'").execute(&pool).await.unwrap();

    let usd = BigDecimal::from(2_500_000_000u64); // US$ 25
    let opts = mc::price_options(&pool, &[Coin::Usdt, Coin::Pol, Coin::Sol, Coin::Ltc], &usd, FRESH).await;
    let coins: Vec<Coin> = opts.iter().map(|o| o.coin).collect();
    assert_eq!(coins, vec![Coin::Usdt, Coin::Pol], "stale SOL and unpriced LTC are not offered");

    let pol = opts.iter().find(|o| o.coin == Coin::Pol).unwrap();
    assert_eq!(pol.amount, BigDecimal::from(5_555_555_556u64), "25 / 0.45 POL, rounded up for the merchant");
    assert_eq!(pol.price_scaled, BigDecimal::from(45_000_000u64));

    // A zero price is never quoted.
    sqlx::query("UPDATE price_cache SET price_scaled = 0 WHERE coin = 'USDT'").execute(&pool).await.unwrap();
    let opts = mc::price_options(&pool, &[Coin::Usdt], &usd, FRESH).await;
    assert!(opts.is_empty());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn one_address_per_invoice_and_coin(pool: PgPool) {
    let inv = invoice(&pool, "mc-addr").await;

    let first = address(&pool, inv.id, Coin::Pol, "0xfirst", 100).await;
    // A refresh with a freshly derived address must not replace the one shown.
    let again = address(&pool, inv.id, Coin::Pol, "0xsecond", 999).await;
    assert_eq!(again.id, first.id);
    assert_eq!(again.address, "0xfirst");
    assert_eq!(again.amount, BigDecimal::from(100));

    address(&pool, inv.id, Coin::Sol, "SoLaddr", 200).await;
    let found = mc::find_invoice_address(&pool, inv.id, Coin::Pol).await.unwrap().unwrap();
    assert_eq!(found.id, first.id);
    // The address shown at creation is recorded too, so the watcher still scans it.
    let initial = mc::find_invoice_address(&pool, inv.id, Coin::Usdt).await.unwrap().unwrap();
    assert_eq!(initial.address, inv.deposit_address);
    assert!(mc::find_invoice_address(&pool, inv.id, Coin::Btc).await.unwrap().is_none());

    let all = mc::list_invoice_addresses(&pool, inv.id).await.unwrap();
    assert_eq!(all.iter().map(|a| a.coin.as_str()).collect::<Vec<_>>(), vec!["USDT", "POL", "SOL"]);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn selecting_a_coin_reprices_until_locked(pool: PgPool) {
    let inv = invoice(&pool, "mc-select").await;
    let pol = address(&pool, inv.id, Coin::Pol, "0xpol", 10_000).await;
    let sol = address(&pool, inv.id, Coin::Sol, "SoLaddr", 20_000).await;

    let switched = mc::select_coin(&pool, inv.id, Coin::Pol, &pol, false).await.unwrap();
    assert_eq!(switched.coin, "POL");
    assert_eq!(switched.deposit_address, "0xpol");
    assert_eq!(&switched.fee_amount + &switched.net_amount, BigDecimal::from(10_000));
    assert!(switched.coin_locked_at.is_none());

    // Customer may still switch, and choosing with lock=true freezes it.
    let locked = mc::select_coin(&pool, inv.id, Coin::Sol, &sol, true).await.unwrap();
    assert_eq!(locked.coin, "SOL");
    assert!(locked.coin_locked_at.is_some());

    let blocked = mc::select_coin(&pool, inv.id, Coin::Pol, &pol, false).await;
    assert!(matches!(blocked, Err(mc::MultiCoinError::CoinLocked)), "{blocked:?}");
    let still = inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap();
    assert_eq!(still.coin, "SOL", "a refused switch changes nothing");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn payment_seen_locks_the_coin(pool: PgPool) {
    let inv = invoice(&pool, "mc-lock").await;
    let pol = address(&pool, inv.id, Coin::Pol, "0xpol", 10_000).await;

    mc::lock_coin(&pool, inv.id).await.unwrap();
    let first_lock = inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap().coin_locked_at;
    mc::lock_coin(&pool, inv.id).await.unwrap();
    assert_eq!(inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap().coin_locked_at, first_lock, "lock time is kept");

    assert!(matches!(mc::select_coin(&pool, inv.id, Coin::Pol, &pol, false).await, Err(mc::MultiCoinError::CoinLocked)));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn expired_invoice_is_not_payable(pool: PgPool) {
    let inv = invoice(&pool, "mc-exp").await;
    let pol = address(&pool, inv.id, Coin::Pol, "0xpol", 10_000).await;
    sqlx::query("UPDATE merchant_deposit_invoices SET expires_at = now() - interval '1 minute' WHERE id = $1")
        .bind(inv.id)
        .execute(&pool)
        .await
        .unwrap();

    let r = mc::select_coin(&pool, inv.id, Coin::Pol, &pol, false).await;
    assert!(matches!(r, Err(mc::MultiCoinError::NotPayable)), "{r:?}");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn money_on_an_abandoned_address_repoints_the_invoice(pool: PgPool) {
    let inv = invoice(&pool, "mc-point").await;
    let pol = address(&pool, inv.id, Coin::Pol, "0xpol", 10_000).await;
    let sol = address(&pool, inv.id, Coin::Sol, "SoLaddr", 20_000).await;
    mc::select_coin(&pool, inv.id, Coin::Sol, &sol, true).await.unwrap();

    // Customer had paid the POL address before switching: the invoice follows the money.
    mc::point_selection_at(&pool, inv.id, Coin::Pol, &pol).await.unwrap();
    let got = inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap();
    assert_eq!((got.coin.as_str(), got.deposit_address.as_str()), ("POL", "0xpol"));
    assert_eq!(&got.fee_amount + &got.net_amount, BigDecimal::from(10_000));

    // Once confirmed, the settled coin is final.
    inv_db::confirm_invoice(&pool, inv.id, Some("0xpaid")).await.unwrap();
    mc::point_selection_at(&pool, inv.id, Coin::Sol, &sol).await.unwrap();
    assert_eq!(inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap().coin, "POL");
}
