//! Faucet claim (HOUSE funded) + faucetlist mine/create paths.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_ok(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    // Fund HOUSE so claim can debit.
    let admin = {
        let email = format!("fund-{}@bitcosats.test", Uuid::new_v4());
        let hash = crypto::hash_password("x").unwrap();
        let username = format!("f{}", &Uuid::new_v4().as_simple().to_string()[..12]);
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
        )
        .bind(&email)
        .bind(&hash)
        .bind(&username)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    db::admin::fund_house(&pool, Coin::Btc, 10_000_000_000, admin)
        .await
        .expect("fund house");

    let (state, token, _, _) = common::register_user(pool.clone(), "fclaim").await;
    common::seed_price_cache(&pool).await;

    let response = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/faucet/claim")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "198.51.100.40")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "BTC",
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "claim={}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["coin"], "BTC");
    assert_eq!(body["amount"].as_str().unwrap(), "1");

    // Ledger SUM must reflect the 1-sat FAUCET credit.
    let bal: bigdecimal::BigDecimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM ledger_entries le \
         JOIN wallets w ON w.id = le.wallet_id \
         JOIN users u ON u.id = w.user_id \
         WHERE u.email LIKE 'fclaim-%' AND w.coin = 'BTC' AND w.kind = 'PERSONAL'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bal, bigdecimal::BigDecimal::from(1));

    let faucet_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ledger_entries le \
         JOIN wallets w ON w.id = le.wallet_id \
         JOIN users u ON u.id = w.user_id \
         WHERE u.email LIKE 'fclaim-%' AND le.type = 'FAUCET'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(faucet_rows, 1);

    // System path: wallets + airdrop profile after claim.
    let wallets = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/wallet")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wallets.status(), axum::http::StatusCode::OK);
    let wbytes = wallets.into_body().collect().await.unwrap().to_bytes();
    let wjson: serde_json::Value = serde_json::from_slice(&wbytes).unwrap();
    let list = wjson.as_array().expect("wallet list is array");
    let btc = list.iter().find(|w| w["coin"] == "BTC").expect("BTC wallet");
    assert_eq!(btc["balance"].as_str().unwrap(), "1");

    let status_res = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/faucet/status")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_res.status(), axum::http::StatusCode::OK);
    let sbytes = status_res.into_body().collect().await.unwrap().to_bytes();
    let sjson: serde_json::Value = serde_json::from_slice(&sbytes).unwrap();
    let btc_cd = sjson["coins"]
        .as_array()
        .expect("coins")
        .iter()
        .find(|c| c["coin"] == "BTC")
        .expect("BTC cooldown");
    assert!(btc_cd["nextClaimAt"].as_str().is_some());

    let overview = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/airdrop/overview")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overview.status(), axum::http::StatusCode::OK);
    let obytes = overview.into_body().collect().await.unwrap().to_bytes();
    let ojson: serde_json::Value = serde_json::from_slice(&obytes).unwrap();
    if body["pointsAwarded"] == true {
        assert!(ojson["total_points"].as_i64().unwrap_or(0) >= 50);
        assert_eq!(ojson["season_active"], true);
    } else {
        assert_eq!(body["seasonActive"], false);
        assert_eq!(ojson["season_active"], false);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_no_season_zero_points(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    sqlx::query("UPDATE airdrop_seasons SET status = 'ENDED'")
        .execute(&pool)
        .await
        .unwrap();

    let admin: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
    )
    .bind(format!("fund-ns-{}@bitcosats.test", Uuid::new_v4()))
    .bind(crypto::hash_password("x").unwrap())
    .bind(format!("n{}", &Uuid::new_v4().as_simple().to_string()[..12]))
    .fetch_one(&pool)
    .await
    .unwrap();
    db::admin::fund_house(&pool, Coin::Btc, 10_000_000_000, admin)
        .await
        .unwrap();

    let (state, token, _, _) = common::register_user(pool.clone(), "fclaimns").await;
    let response = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/faucet/claim")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "198.51.100.77")
                .body(Body::from(
                    serde_json::json!({ "coin": "BTC", "captchaToken": "dev-bypass" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["amount"], "1");
    assert_eq!(body["pointsAwarded"], false);
    assert_eq!(body["seasonActive"], false);

    let overview = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/airdrop/overview")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let obytes = overview.into_body().collect().await.unwrap().to_bytes();
    let ojson: serde_json::Value = serde_json::from_slice(&obytes).unwrap();
    assert_eq!(ojson["season_active"], false);
    assert_eq!(ojson["total_points"], 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_double_race_one_wins(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    let admin: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
    )
    .bind(format!("fund-race-{}@bitcosats.test", Uuid::new_v4()))
    .bind(crypto::hash_password("x").unwrap())
    .bind(format!("r{}", &Uuid::new_v4().as_simple().to_string()[..12]))
    .fetch_one(&pool)
    .await
    .unwrap();
    db::admin::fund_house(&pool, Coin::Btc, 10_000_000_000, admin)
        .await
        .unwrap();

    let (state, token, _, _) = common::register_user(pool.clone(), "fclaimrace").await;
    let mk = |state: api_http::AppState<db::auth::PgAuthRepo>, token: String| async move {
        api_http::app_without_metrics(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/faucet/claim")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-real-ip", "198.51.100.88")
                    .body(Body::from(
                        serde_json::json!({ "coin": "BTC", "captchaToken": "dev-bypass" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    };
    let (a, b) = tokio::join!(mk(state.clone(), token.clone()), mk(state, token));
    let statuses = [a.status(), b.status()];
    let oks = statuses
        .iter()
        .filter(|s| **s == axum::http::StatusCode::OK)
        .count();
    let blocked = statuses
        .iter()
        .filter(|s| {
            **s == axum::http::StatusCode::TOO_MANY_REQUESTS
                || **s == axum::http::StatusCode::BAD_REQUEST
                || **s == axum::http::StatusCode::CONFLICT
        })
        .count();
    assert_eq!(oks, 1, "exactly one claim succeeds: {statuses:?}");
    assert_eq!(blocked, 1, "other claim blocked: {statuses:?}");

    let faucet_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ledger_entries le \
         JOIN wallets w ON w.id = le.wallet_id \
         JOIN users u ON u.id = w.user_id \
         WHERE u.email LIKE 'fclaimrace-%' AND le.type = 'FAUCET'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(faucet_rows, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_path_coin_and_unauth(pool: PgPool) {
    let state = common::test_state(pool.clone());
    let unauth = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/faucet/claim/BTC")
                .header("content-type", "application/json")
                .header("x-real-ip", "198.51.100.41")
                .body(Body::from(r#"{"captchaToken":"dev-bypass"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), axum::http::StatusCode::UNAUTHORIZED);

    db::house::ensure_house_inventory(&pool).await.unwrap();
    let admin: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') RETURNING id",
    )
    .bind(format!("fund2-{}@bitcosats.test", Uuid::new_v4()))
    .bind(crypto::hash_password("x").unwrap())
    .bind(format!("g{}", &Uuid::new_v4().as_simple().to_string()[..12]))
    .fetch_one(&pool)
    .await
    .unwrap();
    db::admin::fund_house(&pool, Coin::Ltc, 10_000_000_000, admin)
        .await
        .unwrap();

    let (state, token, _, _) = common::register_user(pool, "fclaim2").await;
    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/faucet/claim/LTC")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "198.51.100.42")
                .body(Body::from(r#"{"captchaToken":"dev-bypass"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "path claim={}",
        String::from_utf8_lossy(&bytes)
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_claim_requires_captcha_token_field(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "fclaimcap").await;
    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/faucet/claim")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "198.51.100.99")
                .body(Body::from(r#"{"coin":"BTC"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // Missing captchaToken → rejection (422/400); production still verifies Turnstile.
    assert!(
        response.status() == axum::http::StatusCode::UNPROCESSABLE_ENTITY
            || response.status() == axum::http::StatusCode::BAD_REQUEST,
        "status={}",
        response.status()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn airdrop_logs_require_auth_and_are_self_scoped(pool: PgPool) {
    let state = common::test_state(pool.clone());
    let unauth = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/airdrop/logs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), axum::http::StatusCode::UNAUTHORIZED);

    let (state, token, _, _) = common::register_user(pool, "airself").await;
    let logs = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/airdrop/logs")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logs.status(), axum::http::StatusCode::OK);
    // No `userId` query param — cannot IDOR another user's point logs.
}
