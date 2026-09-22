//! Invoice lifecycle the on-chain watcher drives: detection, confirmation,
//! expiry, webhook retry scheduling, and `orderId` uniqueness.

mod common;

use bigdecimal::BigDecimal;
use db::merchant_deposits as inv_db;
use shared::Coin;
use sqlx::PgPool;
use uuid::Uuid;

fn input(merchant: Uuid, order_id: &str, amount: u64) -> inv_db::CreateDepositInvoiceInput {
    inv_db::CreateDepositInvoiceInput {
        merchant_id: merchant,
        api_key_id: None,
        site_user_id: None,
        order_id: order_id.to_string(),
        site_name: None,
        coin: Coin::Pol,
        amount: BigDecimal::from(amount),
        deposit_address: format!("0x{}", Uuid::new_v4().simple()),
        hd_index: Some(3),
        callback_url: "https://merchant.example/hook".into(),
        success_url: None,
        cancel_url: None,
        customer_email: None,
        customer_name: None,
        description: None,
        expiry_minutes: Some(60),
        accepted_coins: vec![],
        price_usd_scaled: None,
        price_decimals: None,
        quote_price_scaled: None,
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn order_id_is_unique_per_merchant(pool: PgPool) {
    let a = common::insert_user(&pool, "inv-uniq-a").await;
    let b = common::insert_user(&pool, "inv-uniq-b").await;

    inv_db::create_invoice(&pool, input(a, "ORD-1", 250_000)).await.unwrap();

    let dup = inv_db::create_invoice(&pool, input(a, "ORD-1", 250_000)).await;
    assert!(
        matches!(dup, Err(inv_db::MerchantDepositError::DuplicateOrderId)),
        "second invoice for the same orderId must be rejected, got {dup:?}"
    );

    // Different merchants own separate orderId namespaces.
    inv_db::create_invoice(&pool, input(b, "ORD-1", 250_000)).await.unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn lookup_by_order_id_round_trips(pool: PgPool) {
    let merchant = common::insert_user(&pool, "inv-lookup").await;
    let created = inv_db::create_invoice(&pool, input(merchant, "ORD-LOOK", 400_000)).await.unwrap();

    let found = inv_db::get_invoice_by_order_id(&pool, merchant, "ORD-LOOK").await.unwrap();
    assert_eq!(found.id, created.id);
    assert_eq!(found.hd_index, Some(3), "hd_index must survive the round trip");
    assert_eq!(found.received_amount, BigDecimal::from(0));

    let missing = inv_db::get_invoice_by_order_id(&pool, merchant, "ORD-NOPE").await;
    assert!(matches!(missing, Err(inv_db::MerchantDepositError::NotFound)));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn detection_records_partial_payment_without_confirming(pool: PgPool) {
    let merchant = common::insert_user(&pool, "inv-detect").await;
    common::insert_personal_wallet(&pool, merchant, Coin::Pol).await;
    let created = inv_db::create_invoice(&pool, input(merchant, "ORD-DET", 500_000)).await.unwrap();

    inv_db::mark_invoice_detected(&pool, created.id, BigDecimal::from(200_000u64), 1, Some("0xpartial"))
        .await
        .unwrap();

    let got = inv_db::get_invoice_by_id(&pool, created.id).await.unwrap();
    assert_eq!(got.status, "DETECTED");
    assert_eq!(got.received_amount, BigDecimal::from(200_000u64));
    assert_eq!(got.confirmations, 1);
    assert_eq!(got.tx_hash.as_deref(), Some("0xpartial"));
    assert!(got.paid_at.is_none(), "an underpayment must not look paid");

    // Still open for the watcher to revisit.
    let open = inv_db::list_open_invoices(&pool, 10).await.unwrap();
    assert!(open.iter().any(|i| i.id == created.id));
}

async fn merchant_wallet(pool: &PgPool, user_id: Uuid, coin: Coin) -> Uuid {
    sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'MERCHANT'")
        .bind(user_id)
        .bind(coin.as_str())
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn merchant_credit_count(pool: &PgPool, invoice_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM ledger_entries WHERE type = 'MERCHANT_DEPOSIT' AND reference_id = $1")
        .bind(invoice_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn confirming_twice_credits_once(pool: PgPool) {
    let merchant = common::insert_user(&pool, "inv-confirm").await;
    let personal = common::insert_personal_wallet(&pool, merchant, Coin::Pol).await;
    let created = inv_db::create_invoice(&pool, input(merchant, "ORD-CONF", 500_000)).await.unwrap();

    inv_db::confirm_invoice(&pool, created.id, Some("0xdeadbeef")).await.unwrap();
    let wallet = merchant_wallet(&pool, merchant, Coin::Pol).await;
    let after_first = common::wallet_balance(&pool, wallet).await;

    // A repeated watcher tick must not credit the merchant a second time,
    // not even when it reports a different tx hash.
    inv_db::confirm_invoice(&pool, created.id, Some("0xdeadbeef")).await.unwrap();
    inv_db::confirm_invoice(&pool, created.id, Some("0xother")).await.unwrap();
    let after_second = common::wallet_balance(&pool, wallet).await;

    assert_eq!(after_first, created.net_amount, "merchant cash is credited net of the fee");
    assert_eq!(after_first, after_second, "second confirmation must be a no-op");
    assert_eq!(
        common::wallet_balance(&pool, personal).await,
        BigDecimal::from(0),
        "gateway income lands in the merchant wallet, never the personal one"
    );
    assert_eq!(merchant_credit_count(&pool, created.id).await, 1);

    let got = inv_db::get_invoice_by_id(&pool, created.id).await.unwrap();
    assert_eq!(got.status, "CONFIRMED");
    assert!(got.paid_at.is_some());

    // Confirmed invoices leave the watcher's queue.
    let open = inv_db::list_open_invoices(&pool, 10).await.unwrap();
    assert!(!open.iter().any(|i| i.id == created.id));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn overdue_invoices_expire_and_paid_ones_do_not(pool: PgPool) {
    let merchant = common::insert_user(&pool, "inv-expire").await;
    common::insert_personal_wallet(&pool, merchant, Coin::Pol).await;

    let stale = inv_db::create_invoice(&pool, input(merchant, "ORD-STALE", 100_000)).await.unwrap();
    let paid = inv_db::create_invoice(&pool, input(merchant, "ORD-PAID", 100_000)).await.unwrap();
    inv_db::confirm_invoice(&pool, paid.id, Some("0xpaid")).await.unwrap();

    sqlx::query("UPDATE merchant_deposit_invoices SET expires_at = now() - interval '1 minute'")
        .execute(&pool)
        .await
        .unwrap();

    let expired = inv_db::expire_due_invoices(&pool).await.unwrap();
    assert_eq!(expired, 1, "only the unpaid invoice expires");

    assert_eq!(inv_db::get_invoice_by_id(&pool, stale.id).await.unwrap().status, "EXPIRED");
    assert_eq!(inv_db::get_invoice_by_id(&pool, paid.id).await.unwrap().status, "CONFIRMED");
    assert!(inv_db::list_open_invoices(&pool, 10).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn webhook_retry_queue_respects_schedule_and_attempt_cap(pool: PgPool) {
    let merchant = common::insert_user(&pool, "inv-hook").await;
    common::insert_personal_wallet(&pool, merchant, Coin::Pol).await;
    let created = inv_db::create_invoice(&pool, input(merchant, "ORD-HOOK", 100_000)).await.unwrap();
    inv_db::confirm_invoice(&pool, created.id, Some("0xhook")).await.unwrap();

    // A failed delivery with no schedule yet is not picked up.
    inv_db::record_webhook_delivery(&pool, created.id, false, Some(500), Some("boom")).await.unwrap();
    assert!(inv_db::list_webhook_retries(&pool, 8, 10).await.unwrap().is_empty());

    // Scheduled in the future — still not due.
    inv_db::schedule_webhook_retry(&pool, created.id, Some(chrono::Utc::now() + chrono::Duration::minutes(5)))
        .await
        .unwrap();
    assert!(inv_db::list_webhook_retries(&pool, 8, 10).await.unwrap().is_empty());

    // Due now — the watcher must see it.
    inv_db::schedule_webhook_retry(&pool, created.id, Some(chrono::Utc::now() - chrono::Duration::seconds(1)))
        .await
        .unwrap();
    let due = inv_db::list_webhook_retries(&pool, 8, 10).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, created.id);
    assert_eq!(due[0].webhook_attempts, 1);

    // Attempt cap parks it permanently.
    assert!(inv_db::list_webhook_retries(&pool, 1, 10).await.unwrap().is_empty());

    // A successful delivery clears the queue.
    inv_db::record_webhook_delivery(&pool, created.id, true, Some(200), None).await.unwrap();
    inv_db::schedule_webhook_retry(&pool, created.id, None).await.unwrap();
    assert!(inv_db::list_webhook_retries(&pool, 8, 10).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn concurrent_confirmations_credit_once(pool: PgPool) {
    let merchant = common::insert_user(&pool, "inv-race").await;
    let created = inv_db::create_invoice(&pool, input(merchant, "ORD-RACE", 500_000)).await.unwrap();

    // Two watcher ticks racing with different hashes (e.g. a reorg-replaced tx).
    let (a, b) = tokio::join!(
        inv_db::confirm_invoice(&pool, created.id, Some("0xaaa")),
        inv_db::confirm_invoice(&pool, created.id, Some("0xbbb")),
    );
    a.unwrap();
    b.unwrap();

    let wallet = merchant_wallet(&pool, merchant, Coin::Pol).await;
    assert_eq!(common::wallet_balance(&pool, wallet).await, created.net_amount);
    assert_eq!(merchant_credit_count(&pool, created.id).await, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn watcher_and_balance_checkout_racing_credit_once(pool: PgPool) {
    let merchant = common::insert_user(&pool, "inv-race-m").await;
    let payer = common::insert_user(&pool, "inv-race-p").await;
    let payer_wallet = common::insert_personal_wallet(&pool, payer, Coin::Pol).await;
    common::credit_wallet(&pool, payer_wallet, 1_000_000, "seed").await;
    let created = inv_db::create_invoice(&pool, input(merchant, "ORD-RACE2", 500_000)).await.unwrap();

    let (onchain, checkout) = tokio::join!(
        inv_db::confirm_invoice(&pool, created.id, Some("0xchain")),
        inv_db::pay_invoice_with_balance(&pool, created.id, payer),
    );
    onchain.unwrap();

    let wallet = merchant_wallet(&pool, merchant, Coin::Pol).await;
    assert_eq!(common::wallet_balance(&pool, wallet).await, created.net_amount);
    assert_eq!(merchant_credit_count(&pool, created.id).await, 1, "exactly one settlement wins");

    // The payer is charged only if their checkout was the one that settled it.
    let payer_balance = common::wallet_balance(&pool, payer_wallet).await;
    match checkout {
        Ok(_) => assert_eq!(payer_balance, BigDecimal::from(500_000u64)),
        Err(inv_db::MerchantDepositError::InvalidStatus) => {
            assert_eq!(payer_balance, BigDecimal::from(1_000_000u64), "losing checkout must not debit")
        }
        Err(e) => panic!("unexpected checkout error: {e:?}"),
    }
}
