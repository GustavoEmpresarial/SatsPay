//! Right to erasure, sealed-email lookup, and backfill of the PII columns the
//! first backfill test doesn't reach (invoices, withdrawals, API key labels, IPs).

mod common;

use bigdecimal::BigDecimal;
use crypto::{SecretsService, PII_PREFIX};
use db::privacy::{self, PrivacyError};
use shared::Coin;
use sqlx::PgPool;
use uuid::Uuid;

fn secrets() -> SecretsService {
    SecretsService::from_hex(&"cd".repeat(32)).unwrap()
}

async fn email_of(pool: &PgPool, user: Uuid) -> String {
    sqlx::query_scalar("SELECT email FROM users WHERE id = $1").bind(user).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn erasure_anonymizes_revokes_access_and_keeps_the_ledger(pool: PgPool) {
    let s = secrets();
    let user = common::insert_user(&pool, "erase").await;
    let email = email_of(&pool, user).await;
    privacy::seal_user_email(&pool, &s, user, &email).await.unwrap();
    sqlx::query("UPDATE users SET two_factor_enabled = true, merchant_business_name = 'Acme' WHERE id = $1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, now() + interval '1 day')")
        .bind(user)
        .bind(format!("h{}", Uuid::new_v4()))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO api_keys (user_id, label, key_hash, key_prefix) VALUES ($1, 'shop', $2, 'sk_x')")
        .bind(user)
        .bind(format!("k{}", Uuid::new_v4()))
        .execute(&pool)
        .await
        .unwrap();
    let wallet = common::insert_personal_wallet(&pool, user, Coin::Pol).await;
    common::credit_wallet(&pool, wallet, 1_234, "kept for accounting").await;

    privacy::erase_user(&pool, &s, user).await.unwrap();

    let (stored_email, enc, uname, pw, tfa, biz, erased): (String, Option<String>, String, String, bool, Option<String>, bool) =
        sqlx::query_as(
            "SELECT email, email_enc, username, password_hash, two_factor_enabled, merchant_business_name, erased_at IS NOT NULL \
             FROM users WHERE id = $1",
        )
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_email, format!("erased+{user}@invalid.local"));
    assert!(enc.is_none());
    assert!(uname.starts_with("d_") && !uname.contains(&email));
    assert!(pw.starts_with("!erased!"), "password can no longer verify");
    assert!(!tfa);
    assert!(biz.is_none());
    assert!(erased);

    let live_tokens: i64 = sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    let live_keys: i64 = sqlx::query_scalar("SELECT count(*) FROM api_keys WHERE user_id = $1 AND disabled_at IS NULL")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((live_tokens, live_keys), (0, 0), "every session and key is cut off");
    assert_eq!(common::wallet_balance(&pool, wallet).await, BigDecimal::from(1_234), "ledger is never rewritten");

    assert_eq!(privacy::find_user_id_by_email(&pool, &s, &email).await.unwrap(), None, "old email no longer resolves");
    assert!(matches!(privacy::erase_user(&pool, &s, user).await, Err(PrivacyError::AlreadyErased)));
    assert!(matches!(privacy::erase_user(&pool, &s, Uuid::new_v4()).await, Err(PrivacyError::NotFound)));

    // Sealing an email onto an erased account is refused silently.
    privacy::seal_user_email(&pool, &s, user, "back@example.com").await.unwrap();
    let enc: Option<String> = sqlx::query_scalar("SELECT email_enc FROM users WHERE id = $1").bind(user).fetch_one(&pool).await.unwrap();
    assert!(enc.is_none());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn sealed_email_is_found_and_revealed(pool: PgPool) {
    let s = secrets();
    let user = common::insert_user(&pool, "seal").await;
    let email = email_of(&pool, user).await;
    privacy::seal_user_email(&pool, &s, user, &email).await.unwrap();

    // Lookup works through the HMAC index even once the plaintext column is blanked.
    sqlx::query("UPDATE users SET email = 'blank' WHERE id = $1").bind(user).execute(&pool).await.unwrap();
    assert_eq!(privacy::find_user_id_by_email(&pool, &s, &email.to_uppercase()).await.unwrap(), Some(user));

    let enc: String = sqlx::query_scalar("SELECT email_enc FROM users WHERE id = $1").bind(user).fetch_one(&pool).await.unwrap();
    assert!(enc.starts_with(PII_PREFIX));
    assert_eq!(privacy::reveal_stored_email(Some(&s), user, "blank", Some(&enc)), SecretsService::normalize_email(&email));
    // A sealed value left in the email column itself is opened too.
    assert_eq!(privacy::reveal_stored_email(Some(&s), user, &enc, None), SecretsService::normalize_email(&email));
    // Without the key, or for plaintext / the house account, the stored value is returned as is.
    assert_eq!(privacy::reveal_stored_email(None, user, "plain@x.io", Some(&enc)), "plain@x.io");
    assert_eq!(privacy::reveal_stored_email(Some(&s), user, "plain@x.io", Some("not-sealed")), "plain@x.io");
    assert_eq!(
        privacy::reveal_stored_email(Some(&s), user, db::house::HOUSE_EMAIL, Some(&enc)),
        db::house::HOUSE_EMAIL
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn backfill_seals_invoice_withdrawal_key_and_ip_columns(pool: PgPool) {
    let s = secrets();
    let user = common::insert_user(&pool, "bf2").await;
    let wallet = common::insert_personal_wallet(&pool, user, Coin::Pol).await;

    let inv = db::merchant_deposits::create_invoice(
        &pool,
        db::merchant_deposits::CreateDepositInvoiceInput {
            merchant_id: user,
            api_key_id: None,
            site_user_id: Some("site-user-9".into()),
            order_id: "ORD-BF".into(),
            site_name: None,
            coin: Coin::Pol,
            amount: BigDecimal::from(100_000u64),
            deposit_address: "0xbf".into(),
            hd_index: Some(1),
            callback_url: "https://shop.example/hook".into(),
            success_url: Some("https://shop.example/ok".into()),
            cancel_url: None,
            customer_email: Some("buyer@example.com".into()),
            customer_name: Some("Buyer".into()),
            description: None,
            expiry_minutes: Some(60),
            accepted_coins: vec![],
            price_usd_scaled: None,
            price_decimals: None,
            quote_price_scaled: None,
        },
    )
    .await
    .unwrap();

    let wd_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO withdrawals (id, wallet_id, to_address, amount, fee_amount, status, requires_approval, requested_ip) \
         VALUES ($1, $2, '0xdestination', 1, 0, 'FAILED', false, '198.51.100.7')",
    )
    .bind(wd_id)
    .bind(wallet)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO api_keys (user_id, label, key_hash, key_prefix) VALUES ($1, 'my shop key', $2, 'sk_y')")
        .bind(user)
        .bind(format!("k{}", Uuid::new_v4()))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO faucet_claims (user_id, coin, amount, ip) VALUES ($1, 'POL', 1, '198.51.100.8')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO system_error_logs (fingerprint, message, ip_address, user_agent) VALUES ('fp', 'boom', '2001:db8::1', $1)")
        .bind("Mozilla/".repeat(30))
        .execute(&pool)
        .await
        .unwrap();

    let report = privacy::run_boot_privacy_jobs(&pool, &s).await.unwrap();
    assert!(report.invoices >= 1 && report.withdrawals >= 1 && report.api_key_labels >= 1 && report.ips >= 2, "{report:?}");

    let (cb, email, site_user): (String, String, String) = sqlx::query_as(
        "SELECT callback_url, customer_email, site_user_id FROM merchant_deposit_invoices WHERE id = $1",
    )
    .bind(inv.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!([&cb, &email, &site_user].iter().all(|v| v.starts_with(PII_PREFIX)), "invoice PII sealed");

    let (to, ip): (String, String) = sqlx::query_as("SELECT to_address, requested_ip FROM withdrawals WHERE id = $1")
        .bind(wd_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(to.starts_with(PII_PREFIX));
    assert_eq!(privacy::open_opt(Some(&s), privacy::KIND_WD_TO, &wd_id.to_string(), &to), "0xdestination", "still usable to send");
    assert!(!ip.contains('.'), "raw IP replaced by a fingerprint");

    let label: String = sqlx::query_scalar("SELECT label FROM api_keys WHERE user_id = $1").bind(user).fetch_one(&pool).await.unwrap();
    assert!(label.starts_with(PII_PREFIX));
    let fip: String = sqlx::query_scalar("SELECT ip FROM faucet_claims WHERE user_id = $1").bind(user).fetch_one(&pool).await.unwrap();
    assert!(!fip.contains('.'));
    let (tip, ua): (String, String) = sqlx::query_as("SELECT ip_address, user_agent FROM system_error_logs WHERE fingerprint = 'fp'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(tip, "2001:db8::1", "raw IPv6 replaced");
    assert_eq!(ua.chars().count(), 80);

    // Second pass has nothing left to seal.
    let again = privacy::backfill_pii(&pool, &s).await.unwrap();
    assert_eq!((again.invoices, again.withdrawals, again.api_key_labels, again.ips), (0, 0, 0, 0), "{again:?}");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn export_includes_wallets_invoices_and_decrypted_key_labels(pool: PgPool) {
    let s = secrets();
    let user = common::insert_user(&pool, "export").await;
    let wallet = common::insert_personal_wallet(&pool, user, Coin::Pol).await;
    common::credit_wallet(&pool, wallet, 5_000, "seed").await;
    sqlx::query("INSERT INTO api_keys (user_id, label, key_hash, key_prefix) VALUES ($1, 'export key', $2, 'sk_z')")
        .bind(user)
        .bind(format!("k{}", Uuid::new_v4()))
        .execute(&pool)
        .await
        .unwrap();
    privacy::backfill_pii(&pool, &s).await.unwrap();

    let out = privacy::export_user_data(&pool, &s, user).await.unwrap();
    let wallets = out["wallets"].as_array().unwrap();
    assert!(wallets.iter().any(|w| w["coin"] == "POL" && w["balance"] == "5000"), "{out}");
    assert_eq!(out["apiKeys"][0]["label"], "export key", "label is decrypted for the owner");
    assert!(out["invoices"].as_array().unwrap().is_empty());
}
