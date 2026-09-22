//! Balance checkout guards, settlement refusals on dead invoices, sealed
//! invoice PII, and the watcher's bookkeeping helpers.

mod common;

use bigdecimal::BigDecimal;
use crypto::{SecretsService, PII_PREFIX};
use db::merchant_deposits::{self as inv_db, MerchantDepositError};
use shared::Coin;
use sqlx::PgPool;
use uuid::Uuid;

fn input(merchant: Uuid, order_id: &str, amount: u64) -> inv_db::CreateDepositInvoiceInput {
    inv_db::CreateDepositInvoiceInput {
        merchant_id: merchant,
        api_key_id: None,
        site_user_id: Some("user-42".into()),
        order_id: order_id.to_string(),
        site_name: None,
        coin: Coin::Pol,
        amount: BigDecimal::from(amount),
        deposit_address: format!("0x{}", Uuid::new_v4().simple()),
        hd_index: Some(3),
        callback_url: "https://merchant.example/hook".into(),
        success_url: Some("https://merchant.example/ok".into()),
        cancel_url: Some("https://merchant.example/cancel".into()),
        customer_email: Some("buyer@example.com".into()),
        customer_name: Some("Buyer".into()),
        description: None,
        expiry_minutes: Some(60),
        accepted_coins: vec![],
        price_usd_scaled: None,
        price_decimals: None,
        quote_price_scaled: None,
    }
}

async fn merchant_cash(pool: &PgPool, merchant: Uuid) -> BigDecimal {
    let id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = 'POL' AND kind = 'MERCHANT'")
        .bind(merchant)
        .fetch_optional(pool)
        .await
        .unwrap();
    match id {
        Some(id) => common::wallet_balance(pool, id).await,
        None => BigDecimal::from(0),
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn balance_checkout_moves_money_once(pool: PgPool) {
    let merchant = common::insert_user(&pool, "co-m").await;
    let payer = common::insert_user(&pool, "co-p").await;
    let payer_wallet = common::insert_personal_wallet(&pool, payer, Coin::Pol).await;
    common::credit_wallet(&pool, payer_wallet, 1_000_000, "seed").await;
    let inv = inv_db::create_invoice(&pool, input(merchant, "CO-1", 400_000)).await.unwrap();

    let paid = inv_db::pay_invoice_with_balance(&pool, inv.id, payer).await.unwrap();
    assert_eq!(paid.status, "CONFIRMED");
    assert_eq!(paid.tx_hash.as_deref(), Some("internal_satspay"));
    assert_eq!(paid.received_amount, BigDecimal::from(400_000u64));
    assert_eq!(common::wallet_balance(&pool, payer_wallet).await, BigDecimal::from(600_000u64), "payer pays the gross amount");
    assert_eq!(merchant_cash(&pool, merchant).await, inv.net_amount, "merchant cash gets the net");

    let again = inv_db::pay_invoice_with_balance(&pool, inv.id, payer).await;
    assert!(matches!(again, Err(MerchantDepositError::InvalidStatus)), "{again:?}");
    assert_eq!(common::wallet_balance(&pool, payer_wallet).await, BigDecimal::from(600_000u64));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn balance_checkout_refusals_leave_balances_alone(pool: PgPool) {
    let merchant = common::insert_user(&pool, "cr-m").await;
    let payer = common::insert_user(&pool, "cr-p").await;
    let inv = inv_db::create_invoice(&pool, input(merchant, "CR-1", 400_000)).await.unwrap();

    let own = inv_db::pay_invoice_with_balance(&pool, inv.id, merchant).await;
    assert!(matches!(own, Err(MerchantDepositError::PayerIsMerchant)), "{own:?}");

    let no_wallet = inv_db::pay_invoice_with_balance(&pool, inv.id, payer).await;
    assert!(matches!(no_wallet, Err(MerchantDepositError::PayerWalletNotFound)), "{no_wallet:?}");

    let payer_wallet = common::insert_personal_wallet(&pool, payer, Coin::Pol).await;
    common::credit_wallet(&pool, payer_wallet, 399_999, "one short").await;
    let short = inv_db::pay_invoice_with_balance(&pool, inv.id, payer).await;
    assert!(matches!(short, Err(MerchantDepositError::InsufficientBalance)), "{short:?}");

    let missing = inv_db::pay_invoice_with_balance(&pool, Uuid::new_v4(), payer).await;
    assert!(matches!(missing, Err(MerchantDepositError::NotFound)), "{missing:?}");

    common::credit_wallet(&pool, payer_wallet, 1, "top up").await;
    sqlx::query("UPDATE merchant_deposit_invoices SET expires_at = now() - interval '1 minute' WHERE id = $1")
        .bind(inv.id)
        .execute(&pool)
        .await
        .unwrap();
    let expired = inv_db::pay_invoice_with_balance(&pool, inv.id, payer).await;
    assert!(matches!(expired, Err(MerchantDepositError::InvalidStatus)), "{expired:?}");

    assert_eq!(common::wallet_balance(&pool, payer_wallet).await, BigDecimal::from(400_000u64));
    assert_eq!(merchant_cash(&pool, merchant).await, BigDecimal::from(0));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn dead_invoices_are_never_settled(pool: PgPool) {
    let merchant = common::insert_user(&pool, "dead").await;
    for (order, status) in [("DEAD-1", "EXPIRED"), ("DEAD-2", "CANCELLED")] {
        let inv = inv_db::create_invoice(&pool, input(merchant, order, 100_000)).await.unwrap();
        sqlx::query(&format!("UPDATE merchant_deposit_invoices SET status = '{status}' WHERE id = $1"))
            .bind(inv.id)
            .execute(&pool)
            .await
            .unwrap();
        let r = inv_db::confirm_invoice(&pool, inv.id, Some("0xlate")).await;
        assert!(matches!(r, Err(MerchantDepositError::InvalidStatus)), "{status}: {r:?}");
    }
    assert_eq!(merchant_cash(&pool, merchant).await, BigDecimal::from(0));
    assert!(matches!(inv_db::confirm_invoice(&pool, Uuid::new_v4(), None).await, Err(MerchantDepositError::NotFound)));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn invoice_pii_is_stored_sealed_and_revealed_for_the_merchant(pool: PgPool) {
    let secrets = SecretsService::from_hex(&"ef".repeat(32)).unwrap();
    let merchant = common::insert_user(&pool, "inv-pii").await;
    let mut inp = input(merchant, "PII-1", 100_000);
    inv_db::seal_invoice_pii(&secrets, &mut inp);
    let stored = inv_db::create_invoice(&pool, inp).await.unwrap();

    for v in [
        Some(stored.callback_url.clone()),
        stored.success_url.clone(),
        stored.cancel_url.clone(),
        stored.customer_email.clone(),
        stored.customer_name.clone(),
        stored.site_user_id.clone(),
    ] {
        assert!(v.unwrap().starts_with(PII_PREFIX), "stored sealed");
    }

    let shown = inv_db::reveal_invoice_pii(&secrets, &stored);
    assert_eq!(shown.callback_url, "https://merchant.example/hook");
    assert_eq!(shown.success_url.as_deref(), Some("https://merchant.example/ok"));
    assert_eq!(shown.cancel_url.as_deref(), Some("https://merchant.example/cancel"));
    assert_eq!(shown.customer_email.as_deref(), Some("buyer@example.com"));
    assert_eq!(shown.customer_name.as_deref(), Some("Buyer"));
    assert_eq!(shown.site_user_id.as_deref(), Some("user-42"));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn watcher_bookkeeping_helpers(pool: PgPool) {
    let merchant = common::insert_user(&pool, "wb").await;
    let inv = inv_db::create_invoice(&pool, input(merchant, "WB-1", 100_000)).await.unwrap();

    assert_eq!(inv_db::count_invoices_at_address(&pool, &inv.deposit_address).await.unwrap(), 1);
    assert_eq!(inv_db::count_invoices_at_address(&pool, "0xnobody").await.unwrap(), 0);

    let when = chrono::Utc::now() + chrono::Duration::minutes(5);
    inv_db::schedule_webhook_retry(&pool, inv.id, Some(when)).await.unwrap();
    let got = inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap();
    assert!((got.webhook_next_retry_at.unwrap() - when).num_milliseconds().abs() < 1_000);
    inv_db::schedule_webhook_retry(&pool, inv.id, None).await.unwrap();
    assert!(inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap().webhook_next_retry_at.is_none());

    // Once swept, the derivation index is dropped so the watcher stops re-sweeping.
    inv_db::mark_invoice_swept(&pool, inv.id).await.unwrap();
    assert!(inv_db::get_invoice_by_id(&pool, inv.id).await.unwrap().hd_index.is_none());
}
