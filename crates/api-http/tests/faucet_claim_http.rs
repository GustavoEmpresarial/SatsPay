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

    let (state, token, _, _) = common::register_user(pool, "fclaim").await;

    let response = api_http::app_without_metrics(state)
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
    assert!(body["amount"].as_str().unwrap().parse::<u128>().unwrap() > 0);
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
