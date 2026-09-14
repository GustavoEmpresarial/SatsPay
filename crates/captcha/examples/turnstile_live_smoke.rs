//! Manual smoke test — verifies real requests against Cloudflare's live
//! Turnstile `siteverify` endpoint using Cloudflare's own published test
//! secrets (always-pass / always-fail), plus the Postgres anti-replay path.
//! Not run in CI — `cargo run -p captcha --example turnstile_live_smoke`.

use captcha::{TurnstileConfig, TurnstileVerifier, VerifyOptions};
use std::time::Duration;

// Cloudflare's own published Turnstile testing secrets (documented at
// https://developers.cloudflare.com/turnstile/troubleshooting/testing/) —
// not invented here. "1x0000..." always passes, "2x0000..." always fails.
const ALWAYS_PASS_SECRET: &str = "1x0000000000000000000000000000000AA";
const ALWAYS_FAIL_SECRET: &str = "2x0000000000000000000000000000000AA";

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(&database_url).await.expect("connect");

    // --- Always-pass secret: real network round-trip to Cloudflare ---
    let verifier = TurnstileVerifier::new(TurnstileConfig { secret: ALWAYS_PASS_SECRET.to_string(), node_env: "development".to_string(), timeout: Duration::from_secs(5), max_token_age: Duration::from_secs(300), siteverify_url: None });
    let token_1 = "smoke-test-token-unique-1";
    let ok = verifier.verify(&pool, token_1, Some("203.0.113.5"), VerifyOptions { expected_action: None, expected_hostnames: &[] }).await.expect("verify call");
    println!("always-pass secret, fresh token: {ok}");
    assert!(ok, "Cloudflare's always-pass test secret must succeed against the real API");

    // --- Replay of the same token must be rejected locally (before even
    // hitting Cloudflare again) — real Postgres anti-replay table. ---
    let replay_ok = verifier.verify(&pool, token_1, Some("203.0.113.5"), VerifyOptions { expected_action: None, expected_hostnames: &[] }).await.expect("verify call");
    println!("always-pass secret, REPLAYED token: {replay_ok}");
    assert!(!replay_ok, "a replayed token must be rejected by the Postgres anti-replay cache");

    // --- Always-fail secret: real network round-trip, real failure ---
    let fail_verifier = TurnstileVerifier::new(TurnstileConfig { secret: ALWAYS_FAIL_SECRET.to_string(), node_env: "development".to_string(), timeout: Duration::from_secs(5), max_token_age: Duration::from_secs(300), siteverify_url: None });
    let token_2 = "smoke-test-token-unique-2";
    let fail_ok = fail_verifier.verify(&pool, token_2, Some("203.0.113.5"), VerifyOptions { expected_action: None, expected_hostnames: &[] }).await.expect("verify call");
    println!("always-fail secret, fresh token: {fail_ok}");
    assert!(!fail_ok, "Cloudflare's always-fail test secret must fail against the real API");

    // --- disabled_in_dev bypass, outside production ---
    let dev_verifier = TurnstileVerifier::new(TurnstileConfig { secret: "disabled_in_dev".to_string(), node_env: "development".to_string(), timeout: Duration::from_secs(5), max_token_age: Duration::from_secs(300), siteverify_url: None });
    let dev_ok = dev_verifier.verify(&pool, "anything", None, VerifyOptions { expected_action: None, expected_hostnames: &[] }).await.expect("verify call");
    assert!(dev_ok, "disabled_in_dev must bypass verification outside production");

    // --- disabled_in_dev must hard-fail in production ---
    let prod_verifier = TurnstileVerifier::new(TurnstileConfig { secret: "disabled_in_dev".to_string(), node_env: "production".to_string(), timeout: Duration::from_secs(5), max_token_age: Duration::from_secs(300), siteverify_url: None });
    let prod_result = prod_verifier.verify(&pool, "anything", None, VerifyOptions { expected_action: None, expected_hostnames: &[] }).await;
    assert!(prod_result.is_err(), "disabled_in_dev must be a hard error in production");
    println!("production fail-closed check: {:?}", prod_result.err());

    println!("\nALL LIVE TURNSTILE ASSERTIONS PASSED");
}
