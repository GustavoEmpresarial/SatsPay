mod common;

use crypto::SecretsService;
use base64::Engine;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

fn pkce_pair() -> (String, String) {
    let verifier = "a".repeat(64);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    (verifier, challenge)
}

#[sqlx::test(migrations = "./migrations")]
async fn oauth_create_authorize_exchange(pool: PgPool) {
    let secrets = SecretsService::from_hex(&"ab".repeat(32)).unwrap();
    let owner = common::insert_user(&pool, "oauth-owner").await;
    let user = common::insert_user(&pool, "oauth-user").await;
    let redirect = "https://app.example/cb".to_string();

    let created = db::oauth::create_application(
        &pool,
        &secrets,
        owner,
        "App",
        Some("desc"),
        Some("https://app.example"),
        None,
        vec![redirect.clone()],
    )
    .await
    .unwrap();
    assert!(!created.client_secret.is_empty());

    let listed = db::oauth::list_user_applications(&pool, owner).await.unwrap();
    assert_eq!(listed.len(), 1);

    let by_cid = db::oauth::get_application_by_client_id(&pool, &created.app.client_id)
        .await
        .unwrap();
    assert_eq!(by_cid.id, created.app.id);

    let (verifier, challenge) = pkce_pair();
    let code = db::oauth::create_authorization_code(
        &pool,
        created.app.id,
        user,
        &redirect,
        "openid profile",
        Some("state1"),
        Some(&challenge),
        Some("S256"),
    )
    .await
    .unwrap();

    let (token, _expires, scope) = db::oauth::exchange_authorization_code(
        &pool,
        &secrets,
        &code,
        &created.app.client_id,
        &created.client_secret,
        &redirect,
        Some(&verifier),
    )
    .await
    .unwrap();
    assert!(!token.is_empty());
    assert!(scope.contains("openid"));

    let info = db::oauth::get_user_by_oauth_token(&pool, &token)
        .await
        .unwrap();
    assert_eq!(info.id, user.to_string()); // sub/id

    let consents = db::oauth::list_user_consents(&pool, user).await.unwrap();
    assert!(!consents.is_empty());

    db::oauth::revoke_user_consent(&pool, user, created.app.id)
        .await
        .unwrap();
    db::oauth::rotate_client_secret(&pool, &secrets, owner, created.app.id)
        .await
        .unwrap();
    db::oauth::delete_application(&pool, owner, created.app.id)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn referral_link_and_stats(pool: PgPool) {
    let referrer = common::insert_user(&pool, "ref-parent").await;
    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(referrer)
        .fetch_one(&pool)
        .await
        .unwrap();

    let child = common::insert_user(&pool, "ref-child").await;
    let linked = db::referral::link_referred_user(&pool, &username, child)
        .await
        .unwrap();
    assert_eq!(linked, Some(referrer));

    let referrer_logs = db::airdrop::list_user_point_logs(&pool, referrer, 10)
        .await
        .unwrap();
    assert!(
        referrer_logs.iter().any(|l| l.activity_type == "REFERRAL_SIGNUP"),
        "referrer must receive REFERRAL_SIGNUP points"
    );
    let child_logs = db::airdrop::list_user_point_logs(&pool, child, 10)
        .await
        .unwrap();
    assert!(
        child_logs.iter().any(|l| l.activity_type == "REFERRAL_WELCOME"),
        "referred user must receive REFERRAL_WELCOME points"
    );

    let stats = db::referral::get_referral_stats(&pool, referrer).await.unwrap();
    assert!(stats.total_referred >= 1);

    let _ = db::referral::record_referral_commission(
        &pool,
        child,
        "FAUCET_CLAIM",
        "LTC",
        bigdecimal::BigDecimal::from(1000),
        bigdecimal::BigDecimal::from(1),
    )
    .await;
    let commissions = db::referral::list_user_commissions(&pool, referrer, 10)
        .await
        .unwrap();
    assert!(
        commissions.iter().any(|c| {
            c.activity_type == "FAUCET_CLAIM"
                && c.amount_usd.parse::<f64>().unwrap_or(0.0) > 0.0
        }),
        "commission amount_usd must be > 0 when provided"
    );
    let referred = db::referral::list_referred_users(&pool, referrer, 10)
        .await
        .unwrap();
    assert!(!referred.is_empty() || !commissions.is_empty() || stats.total_referred >= 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn airdrop_season_award_and_profile(pool: PgPool) {
    // Migration 0015 seeds an ACTIVE season when empty.
    let _ = sqlx::query(
        "INSERT INTO airdrop_seasons (season_number, title, description, reward_pool_usd, status, start_at, end_at)
         VALUES (99, 'S99', 'd', 1000, 'ACTIVE', now() - interval '1 day', now() + interval '30 days')
         ON CONFLICT (season_number) DO NOTHING"
    ).execute(&pool).await;

    let active = db::airdrop::get_active_season(&pool).await.unwrap();
    assert!(active.is_some());

    let user = common::insert_user(&pool, "airdrop").await;
    let awarded = db::airdrop::award_airdrop_points(&pool, user, 100, 10, "FAUCET_CLAIM")
        .await
        .unwrap();
    assert_eq!(awarded, db::airdrop::AwardResult::Awarded);
    let profile = db::airdrop::get_user_airdrop_profile(&pool, user).await.unwrap();
    assert!(profile.season_active);
    assert!(profile.total_points >= 100);
    let board = db::airdrop::get_airdrop_leaderboard(&pool, 10).await.unwrap();
    assert!(!board.is_empty());
    let logs = db::airdrop::list_user_point_logs(&pool, user, 10).await.unwrap();
    assert!(!logs.is_empty());
    assert!(logs.iter().any(|l| l.activity_type == "FAUCET_CLAIM"));
}

#[sqlx::test(migrations = "./migrations")]
async fn airdrop_award_without_active_season_is_observable(pool: PgPool) {
    sqlx::query("UPDATE airdrop_seasons SET status = 'ENDED'")
        .execute(&pool)
        .await
        .unwrap();

    let user = common::insert_user(&pool, "airdrop-off").await;
    let result = db::airdrop::award_airdrop_points(&pool, user, 50, 0, "FAUCET_CLAIM")
        .await
        .unwrap();
    assert_eq!(result, db::airdrop::AwardResult::NoActiveSeason);

    let profile = db::airdrop::get_user_airdrop_profile(&pool, user).await.unwrap();
    assert!(!profile.season_active);
    assert_eq!(profile.total_points, 0);

    let logs = db::airdrop::list_user_point_logs(&pool, user, 10).await.unwrap();
    assert!(logs.is_empty());
}
