//! Manual smoke test for merchant + admin + public-api against a real
//! Postgres (not run in CI — `cargo run -p db --example merchant_admin_pubapi_smoke`).

use bigdecimal::BigDecimal;
use chain::ChainRegistry;
use shared::Coin;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    db::house::ensure_house_inventory(&pool).await.expect("ensure house inventory");
    let secrets = crypto::SecretsService::from_hex(&"ab".repeat(32)).unwrap();

    let email = format!("merchant-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let password_hash = crypto::hash_password("irrelevant").unwrap();
    let user_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&email).bind(&password_hash).fetch_one(&pool).await.unwrap();
    let admin_email = format!("admin-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let admin_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (email, password_hash, role) VALUES ($1, $2, 'ADMIN') RETURNING id").bind(&admin_email).bind(&password_hash).fetch_one(&pool).await.unwrap();

    // --- Merchant ---
    let status = db::merchant::get_status(&pool, None, user_id).await.unwrap();
    assert_eq!(status.status, "NONE");

    let applied = db::merchant::apply(&pool, None, user_id, "Acme Faucet", "https://acme.example", "A faucet site").await.unwrap();
    assert_eq!(applied.status, "PENDING");
    println!("merchant applied status={}", applied.status);

    // Double apply while PENDING must fail.
    let double_apply = db::merchant::apply(&pool, None, user_id, "Acme Faucet 2", "https://acme2.example", "desc").await;
    assert!(double_apply.is_err());
    println!("double-apply correctly rejected: {:?}", double_apply.err());

    db::merchant::approve(&pool, user_id, admin_id).await.unwrap();
    let after_approve = db::merchant::get_status(&pool, None, user_id).await.unwrap();
    assert_eq!(after_approve.status, "APPROVED");
    println!("merchant approved status={}", after_approve.status);

    // --- Admin: withdrawal approve/reject ---
    let btc_wallet: Uuid =
        sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id: btc_wallet, amount: BigDecimal::from(100_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let registry = ChainRegistry::build("development", true).unwrap();
    let client = registry.get(Coin::Btc);
    // Amount >= BTC approval_threshold (1_000_000) forces PENDING (requires_approval).
    let (withdrawal, _created) = db::withdrawals::request_withdrawal(&pool, user_id, Coin::Btc, "bc1qsomevaliddestinationaddressxxxxxxxxxxxxxxx", BigDecimal::from(2_000_000u64), client.as_ref(), "127.0.0.1", None, None).await.unwrap();
    assert_eq!(withdrawal.status, "PENDING");
    assert!(withdrawal.requires_approval);
    println!("withdrawal requires approval, status={}", withdrawal.status);

    let pending = db::admin::list_pending_withdrawals(&pool, None).await.unwrap();
    assert!(pending.iter().any(|w| w.id == withdrawal.id));

    db::admin::approve_withdrawal(&pool, withdrawal.id, admin_id, Some("127.0.0.1")).await.unwrap();
    let status_after_approve: String = sqlx::query_scalar("SELECT status::text FROM withdrawals WHERE id = $1").bind(withdrawal.id).fetch_one(&pool).await.unwrap();
    assert_eq!(status_after_approve, "APPROVED");
    println!("withdrawal approved by admin, status={status_after_approve}");

    // Re-approving an already-approved withdrawal must fail (CAS).
    let reapprove = db::admin::approve_withdrawal(&pool, withdrawal.id, admin_id, None).await;
    assert!(reapprove.is_err());
    println!("re-approve correctly rejected: {:?}", reapprove.err());

    // Process the broadcast (worker's job) then confirm settlement.
    db::withdrawals::process_broadcast(&pool, withdrawal.id, &registry, None).await.unwrap();
    let final_status: String = sqlx::query_scalar("SELECT status::text FROM withdrawals WHERE id = $1").bind(withdrawal.id).fetch_one(&pool).await.unwrap();
    assert_eq!(final_status, "CONFIRMED");
    println!("withdrawal broadcast+confirmed, status={final_status}");

    // --- Admin: fund HOUSE ---
    db::admin::fund_house(&pool, Coin::Ltc, 5_000_000_000, admin_id).await.unwrap();
    println!("HOUSE LTC funded by admin");

    // --- Public API: issue key, send funds ---
    let user2_email = format!("recipient-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let user2_id: Uuid =
        sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&user2_email).bind(&password_hash).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'PERSONAL')").bind(user2_id).execute(&pool).await.unwrap();
    let dev_wallet: Uuid = sqlx::query_scalar("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'BTC', 'DEVELOPER') RETURNING id").bind(user_id).fetch_one(&pool).await.unwrap();
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput { reference_key: None, wallet_id: dev_wallet, amount: BigDecimal::from(10_000_000u64), ledger_type: "ADJUSTMENT", reference_id: None, reference_type: Some("SmokeSeed"), memo: Some("seed dev wallet") },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let issued = db::public_api::issue_api_key(&pool, &secrets, user_id, "smoke-key", &["send"], &[], None, false).await.unwrap();
    println!("issued api key id={} prefix={}", issued.id, issued.prefix);

    let authed = db::public_api::authenticate_by_hash(&pool, &secrets, &issued.key, "203.0.113.9").await.unwrap();
    assert_eq!(authed.id, issued.id);
    db::public_api::require_scope(&authed, "send").unwrap();

    let send_ref = db::public_api::send_to_user(&pool, &secrets, user_id, issued.id, Coin::Btc, &user2_email, BigDecimal::from(1_000_000u64), "smoke-send-1", 100).await.unwrap();
    println!("sent via public API, reference={send_ref}");

    // Replay with the same idempotency key must not double-send.
    let send_ref2 = db::public_api::send_to_user(&pool, &secrets, user_id, issued.id, Coin::Btc, &user2_email, BigDecimal::from(1_000_000u64), "smoke-send-1", 100).await.unwrap();
    assert_eq!(send_ref, send_ref2);
    let dev_balance: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id = $1").bind(dev_wallet).fetch_one(&pool).await.unwrap();
    println!("dev wallet balance after ONE send (replay must not double-debit): {dev_balance}");
    assert_eq!(dev_balance, BigDecimal::from(9_000_000u64));

    println!("\nALL ASSERTIONS PASSED");
}
