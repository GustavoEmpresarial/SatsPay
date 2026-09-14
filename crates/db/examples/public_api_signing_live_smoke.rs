//! Live smoke test for HMAC-signed public API requests against a real
//! Postgres (not run in CI — `cargo run -p db --example
//! public_api_signing_live_smoke`).

use chrono::Utc;
use crypto::SecretsService;
use db::public_api::{sign_request, verify_signed_request, VerifySignedRequestInput};
use std::time::Duration;
use uuid::Uuid;

const MAX_SKEW: Duration = Duration::from_secs(300);

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let secrets = SecretsService::from_hex(&"ab".repeat(32)).expect("secrets service");

    let email = format!("hmac-smoke-{}@bitcosats.dev", Uuid::new_v4());
    let user_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id").bind(&email).bind(crypto::hash_password("x").unwrap()).fetch_one(&pool).await.unwrap();

    let issued = db::public_api::issue_api_key(&pool, &secrets, user_id, "hmac smoke key", &["send"], &[], None, true).await.expect("issue_api_key with require_signature=true");
    println!("issued key id={} require_signature=true", issued.id);

    // require_signature=true must reject the simpler x-api-key path.
    let x_api_key_attempt = db::public_api::authenticate_by_hash(&pool, &secrets, &issued.key, "127.0.0.1").await;
    assert!(x_api_key_attempt.is_err(), "require_signature key must reject x-api-key auth");
    println!("x-api-key path correctly rejected for a signature-required key: {:?}", x_api_key_attempt.err());

    let method = "POST";
    let path = "/v1/public/send?foo=bar";
    let body = br#"{"coin":"BTC","toEmail":"someone@example.com","amount":"1000","idempotencyKey":"abc"}"#;
    let timestamp = Utc::now().timestamp().to_string();
    let signature = sign_request(&issued.key, &timestamp, method, path, body);

    // --- Happy path: valid signature, fresh timestamp, first use ---
    let record = verify_signed_request(&pool, &secrets, VerifySignedRequestInput { key_id: issued.id, timestamp: &timestamp, signature: &signature, method, path, raw_body: body, source_ip: "127.0.0.1" }, MAX_SKEW)
        .await
        .expect("valid signed request must verify");
    assert_eq!(record.id, issued.id);
    println!("valid signature verified for key {}", record.id);

    // --- Replay: the exact same signature must be rejected the second time ---
    let replay = verify_signed_request(&pool, &secrets, VerifySignedRequestInput { key_id: issued.id, timestamp: &timestamp, signature: &signature, method, path, raw_body: body, source_ip: "127.0.0.1" }, MAX_SKEW).await;
    assert!(replay.is_err(), "replaying the same signature must fail");
    println!("replay correctly rejected: {:?}", replay.err());

    // --- Tampered body must invalidate the signature ---
    let tampered_body = br#"{"coin":"BTC","toEmail":"someone@example.com","amount":"999999999","idempotencyKey":"abc"}"#;
    let ts2 = (Utc::now().timestamp() + 1).to_string();
    let sig_for_original = sign_request(&issued.key, &ts2, method, path, body);
    let tampered = verify_signed_request(&pool, &secrets, VerifySignedRequestInput { key_id: issued.id, timestamp: &ts2, signature: &sig_for_original, method, path, raw_body: tampered_body, source_ip: "127.0.0.1" }, MAX_SKEW).await;
    assert!(tampered.is_err(), "signature computed over a different body must not verify the tampered one");
    println!("tampered body correctly rejected: {:?}", tampered.err());

    // --- Stale timestamp must be rejected even with a correct signature ---
    let old_timestamp = (Utc::now().timestamp() - 3600).to_string();
    let old_sig = sign_request(&issued.key, &old_timestamp, method, path, body);
    let stale = verify_signed_request(&pool, &secrets, VerifySignedRequestInput { key_id: issued.id, timestamp: &old_timestamp, signature: &old_sig, method, path, raw_body: body, source_ip: "127.0.0.1" }, MAX_SKEW).await;
    assert!(stale.is_err(), "a timestamp far outside the skew window must be rejected");
    println!("stale timestamp correctly rejected: {:?}", stale.err());

    println!("\nALL ASSERTIONS PASSED");
}
