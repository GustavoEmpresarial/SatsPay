//! Merchant apply/approve + admin withdrawal approve + public API send.

mod common;

use bigdecimal::BigDecimal;
use chain::ChainRegistry;
use shared::Coin;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn merchant_admin_and_public_api(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    let secrets = crypto::SecretsService::from_hex(&"ab".repeat(32)).unwrap();

    let user_id = common::insert_user(&pool, "merchant").await;
    let admin_id = {
        let email = format!("admin-{}@bitcosats.test", Uuid::new_v4());
        let password_hash = crypto::hash_password("irrelevant").unwrap();
        let username = format!("a{}", Uuid::new_v4().simple());
        sqlx::query_scalar(
            "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
        )
        .bind(&email)
        .bind(&password_hash)
        .bind(&username)
        .fetch_one(&pool)
        .await
        .unwrap()
    };

    // Migration 0013 defaults new users to APPROVED (no apply gate).
    assert_eq!(db::merchant::get_status(&pool, user_id).await.unwrap().status, "APPROVED");
    db::merchant::reject(&pool, user_id, admin_id, "policy")
        .await
        .unwrap();
    assert_eq!(db::merchant::get_status(&pool, user_id).await.unwrap().status, "REJECTED");

    let applied = db::merchant::apply(&pool, user_id, "Acme Faucet", "https://acme.example", "A faucet site")
        .await
        .unwrap();
    assert_eq!(applied.status, "PENDING");
    assert!(
        db::merchant::apply(&pool, user_id, "Acme 2", "https://acme2.example", "d")
            .await
            .is_err()
    );

    db::merchant::approve(&pool, user_id, admin_id).await.unwrap();
    assert_eq!(db::merchant::get_status(&pool, user_id).await.unwrap().status, "APPROVED");
    let _ = db::merchant::list_applications(&pool, None).await.unwrap();
    let _ = db::merchant::list_applications(&pool, Some("APPROVED")).await.unwrap();

    let btc_wallet = common::insert_personal_wallet(&pool, user_id, Coin::Btc).await;
    common::credit_wallet(&pool, btc_wallet, 100_000_000, "seed").await;

    let registry = ChainRegistry::build("development", true).unwrap();
    let client = registry.get(Coin::Btc);
    let (withdrawal, _) = db::withdrawals::request_withdrawal(
        &pool,
        user_id,
        Coin::Btc,
        "bc1qsomevaliddestinationaddressxxxxxxxxxxxxxxx",
        BigDecimal::from(2_000_000u64),
        client.as_ref(),
        "127.0.0.1",
        None,
    )
    .await
    .unwrap();
    assert_eq!(withdrawal.status, "PENDING");
    assert!(withdrawal.requires_approval);

    let pending = db::admin::list_pending_withdrawals(&pool).await.unwrap();
    assert!(pending.iter().any(|w| w.id == withdrawal.id));

    db::admin::approve_withdrawal(&pool, withdrawal.id, admin_id, Some("127.0.0.1"))
        .await
        .unwrap();
    let status_after: String =
        sqlx::query_scalar("SELECT status::text FROM withdrawals WHERE id = $1")
            .bind(withdrawal.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status_after, "APPROVED");
    assert!(
        db::admin::approve_withdrawal(&pool, withdrawal.id, admin_id, None)
            .await
            .is_err()
    );

    // Stub chain refuses broadcast → FAILED + reverse (see withdrawal_sqlx).
    db::withdrawals::process_broadcast(&pool, withdrawal.id, &registry)
        .await
        .unwrap();
    let final_status: String =
        sqlx::query_scalar("SELECT status::text FROM withdrawals WHERE id = $1")
            .bind(withdrawal.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_status, "FAILED");

    db::admin::fund_house(&pool, Coin::Ltc, 5_000_000_000, admin_id)
        .await
        .unwrap();

    let user2 = common::insert_user(&pool, "recipient").await;
    let _ = common::insert_personal_wallet(&pool, user2, Coin::Btc).await;
    let user2_email: String =
        sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
            .bind(user2)
            .fetch_one(&pool)
            .await
            .unwrap();

    let dev_wallet: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'DEVELOPER') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    common::credit_wallet(&pool, dev_wallet, 10_000_000, "dev seed").await;

    let issued = db::public_api::issue_api_key(
        &pool,
        &secrets,
        user_id,
        "sqlx-key",
        &["send"],
        &[],
        None,
        false,
    )
    .await
    .unwrap();
    let authed = db::public_api::authenticate_by_hash(&pool, &secrets, &issued.key, "203.0.113.9")
        .await
        .unwrap();
    assert_eq!(authed.id, issued.id);
    db::public_api::require_scope(&authed, "send").unwrap();

    let send_ref = db::public_api::send_to_user(
        &pool,
        user_id,
        issued.id,
        Coin::Btc,
        &user2_email,
        BigDecimal::from(1_000_000u64),
        "sqlx-send-1",
        100,
    )
    .await
    .unwrap();
    let send_ref2 = db::public_api::send_to_user(
        &pool,
        user_id,
        issued.id,
        Coin::Btc,
        &user2_email,
        BigDecimal::from(1_000_000u64),
        "sqlx-send-1",
        100,
    )
    .await
    .unwrap();
    assert_eq!(send_ref, send_ref2);
    assert_eq!(
        common::wallet_balance(&pool, dev_wallet).await,
        BigDecimal::from(9_000_000u64)
    );
}
