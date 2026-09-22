//! Backfill / blank / retention + sealed merchant / ticket / withdrawal round-trip.

mod common;

use crypto::{SecretsService, PII_PREFIX};
use sqlx::PgPool;
use uuid::Uuid;

fn secrets() -> SecretsService {
    SecretsService::from_hex(&"ab".repeat(32)).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn backfill_seals_plaintext_and_is_idempotent(pool: PgPool) {
    let secrets = secrets();
    let user = common::insert_user(&pool, "pii-bf").await;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query(
        "UPDATE users SET merchant_business_name = 'Acme', merchant_website = 'https://acme.example', \
         merchant_description = 'loja' WHERE id = $1",
    )
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();

    let ticket_id = Uuid::new_v4();
    sqlx::query("INSERT INTO support_tickets (id, user_id, topic, subject, status) VALUES ($1, $2, 'other', 'hello', 'OPEN')")
        .bind(ticket_id)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    let msg_id = Uuid::new_v4();
    sqlx::query("INSERT INTO support_messages (id, ticket_id, author_id, author_role, body) VALUES ($1, $2, $3, 'USER', 'plain body')")
        .bind(msg_id)
        .bind(ticket_id)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, ip) VALUES ($1, 'TEST', 'User', '203.0.113.9')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();

    let first = db::privacy::backfill_pii(&pool, &secrets).await.unwrap();
    assert!(first.users >= 1);
    assert!(first.merchants >= 1);
    assert!(first.tickets >= 1);
    assert!(first.messages >= 1);
    assert!(first.ips >= 1);

    let hmac: Option<String> = sqlx::query_scalar("SELECT email_hmac FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    let want_hmac = secrets.email_index(&email);
    assert_eq!(hmac.as_deref(), Some(want_hmac.as_str()));
    let enc: Option<String> = sqlx::query_scalar("SELECT email_enc FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(enc.as_deref().unwrap_or("").starts_with(PII_PREFIX));

    let name: Option<String> = sqlx::query_scalar("SELECT merchant_business_name FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(name.as_deref().unwrap_or("").starts_with(PII_PREFIX));
    let status = db::merchant::get_status(&pool, Some(&secrets), user).await.unwrap();
    assert_eq!(status.business_name.as_deref(), Some("Acme"));

    let ticket = db::support::get_ticket_for_user(&pool, Some(&secrets), user, ticket_id)
        .await
        .unwrap();
    assert_eq!(ticket.ticket.subject, "hello");
    assert_eq!(ticket.messages[0].body, "plain body");
    let raw_subj: String = sqlx::query_scalar("SELECT subject FROM support_tickets WHERE id = $1")
        .bind(ticket_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(raw_subj.starts_with(PII_PREFIX));

    let ip: Option<String> = sqlx::query_scalar("SELECT ip FROM audit_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    let want_ip = secrets.ip_fingerprint("203.0.113.9");
    assert_eq!(ip.as_deref(), Some(want_ip.as_str()));

    let second = db::privacy::backfill_pii(&pool, &secrets).await.unwrap();
    assert_eq!(second.users, 0);
    assert_eq!(second.merchants, 0);
    assert_eq!(second.tickets, 0);
    assert_eq!(second.messages, 0);

    let blanked = db::privacy::blank_plaintext_emails(&pool).await.unwrap();
    assert!(blanked >= 1);
    let stored_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored_email.starts_with("sealed+"));
    assert!(!stored_email.contains('@') || stored_email.ends_with("@invalid.local"));

    let found = db::privacy::find_user_id_by_email(&pool, &secrets, &email)
        .await
        .unwrap();
    assert_eq!(found, Some(user));

    let exported = db::privacy::export_user_data(&pool, &secrets, user).await.unwrap();
    assert_eq!(exported["user"]["email"], email);
}

#[sqlx::test(migrations = "./migrations")]
async fn withdrawal_and_label_seal_then_reveal(pool: PgPool) {
    let secrets = secrets();
    let user = common::insert_user(&pool, "pii-wd").await;
    let wallet = common::insert_personal_wallet(&pool, user, shared::Coin::Btc).await;
    common::credit_wallet(&pool, wallet, 5_000_000, "seed").await;

    let registry = chain::ChainRegistry::build("development", true).unwrap();
    let client = registry.get(shared::Coin::Btc);
    let (wd, created) = db::withdrawals::request_withdrawal(
        &pool,
        user,
        shared::Coin::Btc,
        "bc1qsomevaliddestinationaddressxxxxxxxxxxxxxxx",
        bigdecimal::BigDecimal::from(50_000u64),
        client.as_ref(),
        "198.51.100.10",
        None,
        Some(&secrets),
    )
    .await
    .unwrap();
    assert!(created);

    let stored_addr: String = sqlx::query_scalar("SELECT to_address FROM withdrawals WHERE id = $1")
        .bind(wd.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored_addr.starts_with(PII_PREFIX));
    let stored_ip: Option<String> = sqlx::query_scalar("SELECT requested_ip FROM withdrawals WHERE id = $1")
        .bind(wd.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let want_wd_ip = secrets.ip_fingerprint("198.51.100.10");
    assert_eq!(stored_ip.as_deref(), Some(want_wd_ip.as_str()));

    let hist = db::withdrawals::list_user_withdrawals(&pool, user, None, 10, Some(&secrets))
        .await
        .unwrap();
    assert_eq!(hist[0].to_address, "bc1qsomevaliddestinationaddressxxxxxxxxxxxxxxx");

    let issued = db::public_api::issue_api_key(&pool, &secrets, user, "loja principal", &["balance"], &[], None, false)
        .await
        .unwrap();
    let raw_label: String = sqlx::query_scalar("SELECT label FROM api_keys WHERE id = $1")
        .bind(issued.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(raw_label.starts_with(PII_PREFIX));
    let listed = db::public_api::list_api_keys_by_user(&pool, Some(&secrets), user)
        .await
        .unwrap();
    assert_eq!(listed[0].label, "loja principal");
}

#[sqlx::test(migrations = "./migrations")]
async fn retain_expired_pii_deletes_old_rows(pool: PgPool) {
    let user = common::insert_user(&pool, "pii-ret").await;
    sqlx::query(
        "INSERT INTO email_otps (user_id, purpose, code_hash, expires_at) \
         VALUES ($1, 'LOGIN'::otp_purpose, 'x', now() - interval '10 days')",
    )
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    let report = db::privacy::retain_expired_pii(&pool).await.unwrap();
    assert!(report.otps >= 1);
}

#[test]
fn looks_like_ip_detects_v4_v6_not_hmac() {
    assert!(db::privacy::looks_like_ip("203.0.113.9"));
    assert!(db::privacy::looks_like_ip("2001:db8::1"));
    assert!(!db::privacy::looks_like_ip("aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"));
    assert!(!db::privacy::looks_like_ip("enc:v1:nope"));
}
