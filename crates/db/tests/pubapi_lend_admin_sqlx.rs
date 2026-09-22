//! Public API HMAC + lend views + admin reject_withdrawal.

mod common;

use bigdecimal::BigDecimal;
use chrono::Utc;
use db::public_api::{sign_request, verify_signed_request, VerifySignedRequestInput};
use shared::Coin;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn public_api_hmac_verify_and_replay(pool: PgPool) {
    let secrets = crypto::SecretsService::from_hex(&"ab".repeat(32)).unwrap();
    let user = common::insert_user(&pool, "hmac").await;
    let issued = db::public_api::issue_api_key(
        &pool,
        &secrets,
        user,
        "hmac",
        &["send"],
        &[],
        None,
        true,
    )
    .await
    .unwrap();
    assert!(db::public_api::authenticate_by_hash(&pool, &secrets, &issued.key, "127.0.0.1")
        .await
        .is_err());

    let method = "POST";
    let path = "/v1/public/send";
    let body = br#"{"coin":"BTC"}"#;
    let timestamp = Utc::now().timestamp().to_string();
    let signature = sign_request(&issued.key, &timestamp, method, path, body);
    let max_skew = Duration::from_secs(300);

    let record = verify_signed_request(
        &pool,
        &secrets,
        VerifySignedRequestInput {
            key_id: issued.id,
            timestamp: &timestamp,
            signature: &signature,
            method,
            path,
            raw_body: body,
            source_ip: "127.0.0.1",
        },
        max_skew,
    )
    .await
    .unwrap();
    assert_eq!(record.id, issued.id);

    assert!(verify_signed_request(
        &pool,
        &secrets,
        VerifySignedRequestInput {
            key_id: issued.id,
            timestamp: &timestamp,
            signature: &signature,
            method,
            path,
            raw_body: body,
            source_ip: "127.0.0.1",
        },
        max_skew,
    )
    .await
    .is_err());

    let old = (Utc::now().timestamp() - 3600).to_string();
    let old_sig = sign_request(&issued.key, &old, method, path, body);
    assert!(verify_signed_request(
        &pool,
        &secrets,
        VerifySignedRequestInput {
            key_id: issued.id,
            timestamp: &old,
            signature: &old_sig,
            method,
            path,
            raw_body: body,
            source_ip: "127.0.0.1",
        },
        max_skew,
    )
    .await
    .is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn lend_markets_and_positions(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    db::lend::ensure_lend_reserves(&pool).await.unwrap();
    common::seed_price_cache(&pool).await;

    let markets = db::lend::get_markets(&pool).await.unwrap();
    assert!(!markets.is_empty());

    let user = common::insert_user(&pool, "lend-view").await;
    let btc = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    common::credit_wallet(&pool, btc, 20_000_000, "seed").await;

    // seed LEND_POOL so supply works
    let mut tx = pool.begin().await.unwrap();
    let pool_id = db::house::get_lend_pool_wallet_id(&mut tx, Coin::Btc)
        .await
        .unwrap();
    db::ledger::apply_ledger_entry(
        &mut tx,
        db::ledger::LedgerCreditInput {
            reference_key: None,
            wallet_id: pool_id,
            amount: BigDecimal::from(1_000_000_000u64),
            ledger_type: "ADJUSTMENT",
            reference_id: None,
            reference_type: Some("SqlxTestSeed"),
            memo: Some("pool"),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    db::lend::supply(&pool, user, Coin::Btc, 5_000_000)
        .await
        .unwrap();
    let view = db::lend::get_user_positions(&pool, user, Duration::from_secs(86_400))
        .await
        .unwrap();
    assert!(!view.positions.is_empty());
    let _ = db::lend::list_liquidatable_positions(&pool, Duration::from_secs(86_400))
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_reject_withdrawal(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let registry = chain::ChainRegistry::build("development", true).unwrap();
    let admin_email = format!("adm-{}@bitcosats.test", Uuid::new_v4());
    let admin: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
    )
    .bind(&admin_email)
    .bind(crypto::hash_password("x").unwrap())
    .bind(format!("a{}", Uuid::new_v4().simple()))
    .fetch_one(&pool)
    .await
    .unwrap();

    let user = common::insert_user(&pool, "wd-rej").await;
    let w = common::insert_personal_wallet(&pool, user, Coin::Btc).await;
    common::credit_wallet(&pool, w, 10_000_000, "seed").await;

    let client = registry.get(Coin::Btc);
    let (wd, _) = db::withdrawals::request_withdrawal(
        &pool,
        user,
        Coin::Btc,
        "bc1qrejectdestaddressxxxxxxxxxxxxxxxxxxxx",
        BigDecimal::from(2_000_000u64),
        client.as_ref(),
        "127.0.0.1",
        None,
        None,
    )
    .await
    .unwrap();
    // Force PENDING if auto-queued for small amounts
    sqlx::query("UPDATE withdrawals SET status = 'PENDING'::withdrawal_status, requires_approval = true WHERE id = $1")
        .bind(wd.id)
        .execute(&pool)
        .await
        .unwrap();

    db::admin::reject_withdrawal(&pool, wd.id, admin, Some("127.0.0.1"))
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status::text FROM withdrawals WHERE id = $1")
        .bind(wd.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "CANCELED");
    assert!(common::wallet_balance(&pool, w).await >= BigDecimal::from(9_000_000u64));

    let _ = db::admin::list_all_withdrawals(&pool, Some("CANCELED"), 20, None)
        .await
        .unwrap();
    let _ = db::audit::list_recent_logs(&pool, 10).await.unwrap();
}
