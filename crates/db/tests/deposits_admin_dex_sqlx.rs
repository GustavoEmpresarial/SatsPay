//! Deposits + admin lists/funds + dex_swap FSM + faucetlist + merchant invoice.

mod common;

use bigdecimal::BigDecimal;
use chain::StubClient;
use shared::Coin;
use sqlx::PgPool;
use uuid::Uuid;

async fn insert_admin(pool: &PgPool) -> Uuid {
    let email = format!("adm-{}@bitcosats.test", Uuid::new_v4());
    let password_hash = crypto::hash_password("x").unwrap();
    let username = format!("a{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(&username)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn deposits_address_credit_list(pool: PgPool) {
    let user = common::insert_user(&pool, "dep").await;
    let client = StubClient::new(Coin::Btc);
    let addr = db::deposits::get_or_create_address(&pool, user, Coin::Btc, &client)
        .await
        .unwrap();
    assert!(addr.starts_with("bc1q"));
    let again = db::deposits::get_or_create_address(&pool, user, Coin::Btc, &client)
        .await
        .unwrap();
    assert_eq!(addr, again);

    let wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = 'BTC' AND kind = 'PERSONAL'",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();

    db::deposits::credit_deposit(
        &pool,
        wallet_id,
        Coin::Btc,
        "txid-dep-1",
        0,
        BigDecimal::from(50_000u64),
        99,
    )
    .await
    .unwrap();
    // idempotent
    db::deposits::credit_deposit(
        &pool,
        wallet_id,
        Coin::Btc,
        "txid-dep-1",
        0,
        BigDecimal::from(50_000u64),
        99,
    )
    .await
    .unwrap();
    assert!(common::wallet_balance(&pool, wallet_id).await >= BigDecimal::from(50_000u64));

    let listed = db::deposits::list_user_deposits(&pool, user, None, 20)
        .await
        .unwrap();
    assert!(!listed.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_stats_funds_and_lists(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    db::lend::ensure_lend_reserves(&pool).await.unwrap();
    let admin = insert_admin(&pool).await;
    let user = common::insert_user(&pool, "adm-user").await;
    // auto-approved merchant; suspend/approve cycle
    db::admin::suspend_merchant(&pool, user, admin).await.unwrap();
    db::admin::approve_merchant(&pool, user, admin).await.unwrap();

    let stats = db::admin::get_dashboard_stats(&pool).await.unwrap();
    assert!(stats.total_users >= 1);

    db::admin::fund_house(&pool, Coin::Ltc, 1_000_000, admin)
        .await
        .unwrap();
    db::admin::fund_lend_pool(&pool, Coin::Btc, 1_000_000, admin)
        .await
        .unwrap();

    let _ = db::admin::list_pending_withdrawals(&pool).await.unwrap();
    let _ = db::admin::list_all_withdrawals(&pool, None, 20).await.unwrap();
    let merchants = db::admin::list_all_merchants(&pool).await.unwrap();
    assert!(merchants.iter().any(|m| m.email.contains("adm-user")));
    let mstats = db::admin::get_merchant_platform_stats(&pool).await.unwrap();
    assert!(mstats.accounts_total >= 1);
    assert!(mstats.conversion_pct >= 0.0);
    let econ = db::admin::get_platform_economics(&pool).await.unwrap();
    assert!(econ.all_time.faucet_claims >= 0);
    assert!(econ.last_24h.gateway_paid >= 0);

    // Network fee telemetry + fee margin aggregation
    let recorded = db::network_fees::record_network_fee(
        &pool,
        db::network_fees::RecordNetworkFeeInput {
            coin: Coin::Btc,
            kind: db::network_fees::NetworkFeeKind::Withdrawal,
            amount: 500,
            tx_hash: Some("txid-net-fee-1"),
            reference_id: None,
            reference_type: Some("Withdrawal"),
        },
    )
    .await
    .unwrap();
    assert!(recorded);
    let again = db::network_fees::record_network_fee(
        &pool,
        db::network_fees::RecordNetworkFeeInput {
            coin: Coin::Btc,
            kind: db::network_fees::NetworkFeeKind::Withdrawal,
            amount: 500,
            tx_hash: Some("txid-net-fee-1"),
            reference_id: None,
            reference_type: Some("Withdrawal"),
        },
    )
    .await
    .unwrap();
    assert!(!again, "duplicate tx_hash must be idempotent");

    let econ = db::admin::get_platform_economics(&pool).await.unwrap();
    let btc_margin = econ
        .all_time
        .fee_margin_by_coin
        .iter()
        .find(|m| m.coin == "BTC")
        .expect("BTC fee margin row expected after network fee");
    assert_eq!(btc_margin.network_paid, "500");
    // Only network cost so far → fee_margin negative → not healthy
    assert_eq!(btc_margin.fees_earned, "0");
    assert!(!btc_margin.healthy);
    let net_kind = econ
        .all_time
        .network_by_kind
        .iter()
        .find(|n| n.coin == "BTC" && n.kind == "WITHDRAWAL")
        .expect("WITHDRAWAL network kind");
    assert_eq!(net_kind.amount, "500");

    let hot = std::collections::HashMap::new();
    let custody = std::collections::HashMap::new();
    let health = db::treasury_health::get_treasury_health(&pool, &hot, &custody, vec![])
        .await
        .unwrap();
    assert!(health.fee_margin_block.enabled);
    assert!(health.break_even.iter().any(|b| b.coin == "BTC"));
    assert!(
        db::treasury_health::fee_margin_blocks_coin(&pool, Coin::Btc)
            .await
            .unwrap(),
        "BTC should hard-block when network > earned"
    );

    let _ = db::admin::list_all_faucet_sites(&pool).await.unwrap();

    // create_site inserts APPROVED (legacy auto-approve); cover suspend + re-approve.
    let site = db::faucetlist::create_site(
        &pool,
        user,
        "My Faucet",
        "https://faucet.example",
        "desc",
        &["BTC", "LTC"],
        Some("100 sats"),
    )
    .await
    .unwrap();
    let approved = db::faucetlist::list_approved(&pool).await.unwrap();
    assert!(approved.iter().any(|s| s.id == site));
    let _ = db::faucetlist::register_click(&pool, site).await.unwrap();
    let mine = db::faucetlist::list_mine(&pool, user).await.unwrap();
    assert!(!mine.is_empty());
    db::admin::suspend_faucet_site(&pool, site, admin).await.unwrap();
    db::admin::approve_faucet_site(&pool, site, admin).await.unwrap();
    db::admin::reject_faucet_site(&pool, site, admin, "spam").await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn dex_swap_lock_complete_and_refund_paths(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let user = common::insert_user(&pool, "dex").await;
    let btc = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    let _ltc = common::insert_personal_wallet(&pool, user, Coin::Ltc).await;
    common::credit_wallet(&pool, btc, 10_000_000, "seed").await;

    let providers = vec!["THORCHAIN".to_string()];
    let (row, created) = db::dex_swap::lock_and_create(
        &pool,
        db::dex_swap::LockDexSwapInput {
            user_id: user,
            from_coin: Coin::Btc,
            to_coin: Coin::Ltc,
            from_amount: 1_000_000,
            expected_to_amount: 50_000_000,
            min_to_amount: Some(45_000_000),
            provider: "THORCHAIN",
            providers: &providers,
            route_id: Some("r1"),
            quote_id: Some("q1"),
            platform_fee_bps: 50,
            platform_fee_amount: 1000,
            fees_json: serde_json::json!({}),
            eta_seconds: Some(60),
            tx_hint: Some("simpleTransfer"),
            destination_address: Some("ltc1qdest"),
            source_address: Some("bc1qsrc"),
            swap_payload: Some(serde_json::json!({"x":1})),
            idempotency_key: "dex-key-1",
        },
    )
    .await
    .unwrap();
    assert!(created);
    assert_eq!(row.status, "LOCKED");

    let (again, created2) = db::dex_swap::lock_and_create(
        &pool,
        db::dex_swap::LockDexSwapInput {
            user_id: user,
            from_coin: Coin::Btc,
            to_coin: Coin::Ltc,
            from_amount: 1_000_000,
            expected_to_amount: 50_000_000,
            min_to_amount: Some(45_000_000),
            provider: "THORCHAIN",
            providers: &providers,
            route_id: Some("r1"),
            quote_id: Some("q1"),
            platform_fee_bps: 50,
            platform_fee_amount: 1000,
            fees_json: serde_json::json!({}),
            eta_seconds: Some(60),
            tx_hint: Some("simpleTransfer"),
            destination_address: Some("ltc1qdest"),
            source_address: Some("bc1qsrc"),
            swap_payload: None,
            idempotency_key: "dex-key-1",
        },
    )
    .await
    .unwrap();
    assert!(!created2);
    assert_eq!(again.id, row.id);

    db::dex_swap::mark_broadcasting(&pool, row.id, "bc1qin", Some("memo"), &serde_json::json!({"ok":true}))
        .await
        .unwrap();
    db::dex_swap::mark_in_flight(&pool, row.id, "in-tx-1")
        .await
        .unwrap();
    db::dex_swap::credit_and_complete(&pool, row.id, 50_000_000, Some("out-tx-1"))
        .await
        .unwrap();

    let done = db::dex_swap::get_by_id(&pool, row.id).await.unwrap().unwrap();
    assert_eq!(done.status, "COMPLETED");
    let listed = db::dex_swap::list_user(&pool, user, 10).await.unwrap();
    assert!(!listed.is_empty());
    let _ = db::dex_swap::list_active(&pool, 10).await.unwrap();
    let _ = db::dex_swap::telemetry_snapshot(&pool).await.unwrap();

    // Second swap → fail + refund
    let (row2, _) = db::dex_swap::lock_and_create(
        &pool,
        db::dex_swap::LockDexSwapInput {
            user_id: user,
            from_coin: Coin::Btc,
            to_coin: Coin::Ltc,
            from_amount: 500_000,
            expected_to_amount: 20_000_000,
            min_to_amount: None,
            provider: "THORCHAIN",
            providers: &providers,
            route_id: None,
            quote_id: None,
            platform_fee_bps: 50,
            platform_fee_amount: 100,
            fees_json: serde_json::json!({}),
            eta_seconds: None,
            tx_hint: None,
            destination_address: None,
            source_address: None,
            swap_payload: None,
            idempotency_key: "dex-key-2",
        },
    )
    .await
    .unwrap();
    db::dex_swap::mark_failed(&pool, row2.id, "provider down")
        .await
        .unwrap();
    db::dex_swap::refund(&pool, row2.id, "provider down")
        .await
        .unwrap();
    let refunded = db::dex_swap::get_by_id(&pool, row2.id).await.unwrap().unwrap();
    assert_eq!(refunded.status, "REFUNDED");
}

#[sqlx::test(migrations = "./migrations")]
async fn merchant_invoice_pay_confirm_webhook(pool: PgPool) {
    let merchant = common::insert_user(&pool, "merch-inv").await;
    let payer = common::insert_user(&pool, "payer-inv").await;
    let _m_btc = common::insert_personal_wallet(&pool, merchant, Coin::Btc).await;
    let _m_merchant: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'MERCHANT') RETURNING id",
    )
    .bind(merchant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let p_btc = common::insert_personal_wallet(&pool, payer, Coin::Btc).await;
    common::credit_wallet(&pool, p_btc, 10_000_000, "payer seed").await;

    let inv = db::merchant_deposits::create_invoice(
        &pool,
        db::merchant_deposits::CreateDepositInvoiceInput {
            merchant_id: merchant,
            api_key_id: None,
            site_user_id: Some("u1".into()),
            order_id: format!("ord-{}", Uuid::new_v4()),
            site_name: Some("Shop".into()),
            coin: Coin::Btc,
            amount: BigDecimal::from(1_000_000u64),
            deposit_address: "bc1qinvoice".into(),
            hd_index: Some(7),
            callback_url: "https://shop.example/cb".into(),
            success_url: None,
            cancel_url: None,
            customer_email: Some("c@x.test".into()),
            customer_name: Some("Cust".into()),
            description: Some("order".into()),
            expiry_minutes: Some(60),
            accepted_coins: vec![],
            price_usd_scaled: None,
            price_decimals: None,
            quote_price_scaled: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(inv.status, "PENDING");

    let got = db::merchant_deposits::get_invoice_by_id(&pool, inv.id)
        .await
        .unwrap();
    assert_eq!(got.id, inv.id);
    let listed = db::merchant_deposits::list_invoices_by_merchant(&pool, merchant, 20, 0)
        .await
        .unwrap();
    assert!(!listed.is_empty());

    db::merchant_deposits::pay_invoice_with_balance(&pool, inv.id, payer)
        .await
        .unwrap();
    let paid = db::merchant_deposits::get_invoice_by_id(&pool, inv.id)
        .await
        .unwrap();
    assert_eq!(paid.status, "CONFIRMED");

    // Fresh invoice for on-chain confirm path
    let inv2 = db::merchant_deposits::create_invoice(
        &pool,
        db::merchant_deposits::CreateDepositInvoiceInput {
            merchant_id: merchant,
            api_key_id: None,
            site_user_id: None,
            order_id: format!("ord-{}", Uuid::new_v4()),
            site_name: None,
            coin: Coin::Btc,
            amount: BigDecimal::from(500_000u64),
            deposit_address: "bc1qin2".into(),
            hd_index: None,
            callback_url: "https://shop.example/cb".into(),
            success_url: None,
            cancel_url: None,
            customer_email: None,
            customer_name: None,
            description: None,
            expiry_minutes: Some(30),
            accepted_coins: vec![],
            price_usd_scaled: None,
            price_decimals: None,
            quote_price_scaled: None,
        },
    )
    .await
    .unwrap();
    db::merchant_deposits::confirm_invoice(&pool, inv2.id, Some("tx-merch-1"))
        .await
        .unwrap();
    db::merchant_deposits::record_webhook_delivery(&pool, inv2.id, true, Some(200), None)
        .await
        .unwrap();
}
