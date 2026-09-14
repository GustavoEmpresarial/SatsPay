//! Auth repo + audit + aave cache + pricing cache + deposit reconcile + pubapi extras.

mod common;

use chrono::{Duration as ChronoDuration, Utc};
use domain::auth::AuthRepo;
use shared::Coin;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn auth_repo_user_refresh_otp_flow(pool: PgPool) {
    let repo = db::auth::PgAuthRepo::new(pool.clone());
    let email = format!("auth-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let hash = crypto::hash_password("x").unwrap();

    let user = repo
        .create_user_with_wallets(&email, &username, &hash, &[Coin::Btc, Coin::Ltc])
        .await
        .unwrap();
    assert_eq!(user.email, email);
    assert!(repo.find_user_by_email(&email).await.unwrap().is_some());
    assert!(repo.find_user_by_id(user.id).await.unwrap().is_some());
    assert!(repo.find_user_by_username(&username).await.unwrap().is_some());

    repo.update_user_role(user.id, "ADMIN").await.unwrap();
    repo.update_last_login(user.id).await.unwrap();
    repo.update_two_factor_enabled(user.id, true).await.unwrap();
    let renamed = format!("n{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let updated = repo.update_username(user.id, &renamed).await.unwrap();
    assert_eq!(updated.username, renamed);

    let now = Utc::now();
    let token_hash = format!("th-{}", Uuid::new_v4());
    repo.create_refresh_token(user.id, &token_hash, now + ChronoDuration::hours(1))
        .await
        .unwrap();
    assert!(repo.find_refresh_token(&token_hash).await.unwrap().is_some());
    assert!(repo
        .find_refresh_token_with_user(&token_hash)
        .await
        .unwrap()
        .is_some());
    assert_eq!(repo.claim_refresh_token(&token_hash, now).await.unwrap(), 1);
    assert_eq!(repo.claim_refresh_token(&token_hash, now).await.unwrap(), 0);

    let token2 = format!("th2-{}", Uuid::new_v4());
    repo.create_refresh_token(user.id, &token2, now + ChronoDuration::hours(1))
        .await
        .unwrap();
    repo.revoke_refresh_token_by_hash(&token2, now).await.unwrap();
    let token3 = format!("th3-{}", Uuid::new_v4());
    repo.create_refresh_token(user.id, &token3, now + ChronoDuration::hours(1))
        .await
        .unwrap();
    repo.revoke_all_user_refresh_tokens(user.id, now).await.unwrap();

    repo.rotate_otp(user.id, "LOGIN", "otp-hash", now + ChronoDuration::minutes(10), now)
        .await
        .unwrap();
    assert!(repo
        .find_recent_otp(user.id, "LOGIN", now - ChronoDuration::minutes(1))
        .await
        .unwrap()
        .is_some());
    let otp = repo.find_active_otp(user.id, "LOGIN", now).await.unwrap().unwrap();
    repo.increment_otp_attempts(otp.id).await.unwrap();
    assert_eq!(repo.claim_otp(otp.id, now).await.unwrap(), 1);
    assert_eq!(repo.claim_otp(otp.id, now).await.unwrap(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn audit_record_and_list(pool: PgPool) {
    let user = common::insert_user(&pool, "audit").await;
    let id = db::audit::record_log(
        &pool,
        Some(user),
        "LOGIN_OK",
        "User",
        Some(user),
        Some("127.0.0.1"),
        Some(serde_json::json!({"ua":"t"})),
    )
    .await
    .unwrap();
    assert!(!id.is_nil());
    db::audit::record_log_spawned(
        pool.clone(),
        Some(user),
        "LOGOUT".into(),
        "User".into(),
        Some(user),
        Some("127.0.0.1".into()),
        None,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mine = db::audit::list_user_logs(&pool, user, 20).await.unwrap();
    assert!(!mine.is_empty());
    let recent = db::audit::list_recent_logs(&pool, 20).await.unwrap();
    assert!(!recent.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn aave_upsert_and_get(pool: PgPool) {
    db::aave_sync::upsert_aave_rate(&pool, "BTC", 300, 500, 4000, 8000, 8500, 1_000_000.0, "aave-v3")
        .await
        .unwrap();
    db::aave_sync::upsert_aave_rate(&pool, "BTC", 350, 550, 4100, 8000, 8500, 1_100_000.0, "aave-v3")
        .await
        .unwrap();
    let rates = db::aave_sync::get_aave_rates(&pool).await.unwrap();
    let btc = rates.get(&Coin::Btc).expect("BTC rate");
    assert_eq!(btc.supply_apy_bps, 350);
    assert_eq!(btc.protocol, "aave-v3");
}

#[sqlx::test(migrations = "./migrations")]
async fn pricing_cache_get_list_table(pool: PgPool) {
    common::seed_price_cache(&pool).await;
    let (scaled, decimals) = db::pricing::get_price(&pool, Coin::Btc, Duration::from_secs(3600))
        .await
        .unwrap();
    assert!(scaled > 0);
    assert_eq!(decimals, 8);
    sqlx::query("UPDATE price_cache SET fetched_at = now() - interval '2 hours' WHERE coin = 'BTC'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(db::pricing::get_price(&pool, Coin::Btc, Duration::from_secs(60))
        .await
        .is_err());
    // restore for load_price_table
    sqlx::query("UPDATE price_cache SET fetched_at = now()")
        .execute(&pool)
        .await
        .unwrap();
    let (dec, map) = db::pricing::list_cached_prices(&pool).await.unwrap();
    assert_eq!(dec, 8);
    assert!(map.contains_key(&Coin::Btc));

    let mut tx = pool.begin().await.unwrap();
    let table = db::pricing::load_price_table(&mut tx, Duration::from_secs(3600))
        .await
        .unwrap();
    assert!(table.price_scaled(Coin::Btc) > 0);
    assert!(table.value_usd(Coin::Btc, 100_000_000, 8) > 0);
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn deposit_reconcile_reversal_and_orphan(pool: PgPool) {
    let user = common::insert_user(&pool, "reorg").await;
    let wallet = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    db::deposits::credit_deposit(
        &pool,
        wallet,
        Coin::Btc,
        "txid-reorg-1",
        0,
        bigdecimal::BigDecimal::from(25_000u64),
        99,
    )
    .await
    .unwrap();
    assert!(common::wallet_balance(&pool, wallet).await >= bigdecimal::BigDecimal::from(25_000u64));

    db::deposits::reconcile_deposit(&pool, wallet, Coin::Btc, "txid-reorg-1", 0, None)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status::text FROM deposits WHERE tx_hash = $1")
        .bind("txid-reorg-1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "ORPHANED");

    // Pending (under-conf) then orphan without credit
    db::deposits::credit_deposit(
        &pool,
        wallet,
        Coin::Btc,
        "txid-pending-1",
        0,
        bigdecimal::BigDecimal::from(10_000u64),
        0,
    )
    .await
    .unwrap();
    db::deposits::reconcile_deposit(&pool, wallet, Coin::Btc, "txid-pending-1", 0, Some(0))
        .await
        .unwrap();
    let listed = db::deposits::list_user_deposits(&pool, user, Some(Coin::Btc), 50)
        .await
        .unwrap();
    assert!(listed.iter().any(|d| d.tx_hash == "txid-reorg-1"));
}

#[sqlx::test(migrations = "./migrations")]
async fn public_api_rotate_disable_balance(pool: PgPool) {
    let secrets = crypto::SecretsService::from_hex(&"ab".repeat(32)).unwrap();
    let user = common::insert_user(&pool, "pubapi").await;
    let _w = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    let dev: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'DEVELOPER') RETURNING id",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    common::credit_wallet(&pool, dev, 5_000_000, "seed").await;

    let issued = db::public_api::issue_api_key(
        &pool,
        &secrets,
        user,
        "k1",
        &["send", "balance"],
        &[],
        Some(30),
        false,
    )
    .await
    .unwrap();
    let keys = db::public_api::list_api_keys_by_user(&pool, user).await.unwrap();
    assert!(!keys.is_empty());

    let rotated = db::public_api::rotate_api_key(&pool, &secrets, user, issued.id)
        .await
        .unwrap();
    assert_ne!(rotated.key, issued.key);
    let bal = db::public_api::get_balance_for_api_key(&pool, user).await.unwrap();
    assert!(bal.iter().any(|(c, _)| *c == Coin::Btc));

    db::public_api::disable_api_key(&pool, user, issued.id)
        .await
        .unwrap();
    assert!(db::public_api::authenticate_by_hash(&pool, &secrets, &rotated.key, "1.2.3.4")
        .await
        .is_err());
}
