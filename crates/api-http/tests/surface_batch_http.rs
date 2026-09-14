//! Batch oneshots: swap prices, faucetlist, status, admin stats, auth extras.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn register_login(pool: PgPool, prefix: &str) -> (api_http::AppState<db::auth::PgAuthRepo>, String, String) {
    let state = common::test_state(pool);
    let email = format!("{prefix}-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let password = "Password1234";
    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.40")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "username": username,
                        "password": password,
                        "confirmPassword": password,
                        "acceptTerms": true,
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status(), axum::http::StatusCode::CREATED, "register failed");
    let bytes = reg.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["tokens"]["accessToken"].as_str().unwrap().to_string();
    (state, token, email)
}

async fn get(app: axum::Router, uri: &str, token: Option<&str>) -> (axum::http::StatusCode, serde_json::Value) {
    let mut b = axum::http::Request::builder().uri(uri).header("x-real-ip", "203.0.113.40");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let response = app.oneshot(b.body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, v)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_prices_and_faucetlist_and_status(pool: PgPool) {
    // price ticker is fail-open on empty cache
    for (coin, price) in [("BTC", 100i64), ("LTC", 10), ("DOGE", 1), ("BCH", 20), ("POL", 1), ("DGB", 1), ("SOL", 50), ("USDT", 100), ("USDC", 100)] {
        sqlx::query(
            "INSERT INTO price_cache (coin, price_scaled, price_decimals, fetched_at) \
             VALUES ($1::coin, $2, 8, now()) ON CONFLICT (coin) DO UPDATE SET price_scaled = EXCLUDED.price_scaled, fetched_at = now()",
        )
        .bind(coin)
        .bind(bigdecimal::BigDecimal::from(price * 100_000_000))
        .execute(&pool)
        .await
        .unwrap();
    }

    let (state, token, _) = register_login(pool, "surf").await;

    let (st, prices) = get(api_http::app_without_metrics(state.clone()), "/v1/swap/prices", Some(&token)).await;
    assert_eq!(st, axum::http::StatusCode::OK, "{prices}");

    let (st, list) = get(api_http::app_without_metrics(state.clone()), "/v1/faucetlist", None).await;
    assert_eq!(st, axum::http::StatusCode::OK, "{list}");

    let (st, nodes) = get(api_http::app_without_metrics(state.clone()), "/v1/status/nodes", None).await;
    assert!(
        st == axum::http::StatusCode::OK || st.is_success() || st.is_server_error(),
        "nodes={nodes} st={st}"
    );

    let (st, ledger) = get(
        api_http::app_without_metrics(state.clone()),
        "/v1/wallet/ledger?kind=PERSONAL",
        Some(&token),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{ledger}");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn auth_username_security_logs_logout(pool: PgPool) {
    let (state, token, _) = register_login(pool, "authx").await;
    let new_name = format!("n{}", &Uuid::new_v4().as_simple().to_string()[..12]);

    let response = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("PATCH")
                .uri("/v1/auth/username")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.40")
                .body(Body::from(serde_json::json!({ "username": new_name }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let (st, logs) = get(
        api_http::app_without_metrics(state.clone()),
        "/v1/auth/security-logs",
        Some(&token),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{logs}");

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/logout")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.40")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success() || response.status() == axum::http::StatusCode::NO_CONTENT || response.status() == axum::http::StatusCode::OK);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_stats_as_admin(pool: PgPool) {
    let email = "admin@bitcosats.test".to_string();
    let username = format!("a{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let password = "Password1234";
    let hash = crypto::hash_password(password).unwrap();
    sqlx::query(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1, $2, $3, 'ADMIN') \
         ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash, role = 'ADMIN'",
    )
    .bind(&email)
    .bind(&hash)
    .bind(&username)
    .execute(&pool)
    .await
    .unwrap();

    let state = common::test_state(pool);
    let login = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/admin/login")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.41")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "password": password,
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = login.status();
    let bytes = login.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK, "admin login={}", String::from_utf8_lossy(&bytes));
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["accessToken"]
        .as_str()
        .or_else(|| v["tokens"]["accessToken"].as_str())
        .expect("accessToken");

    for path in [
        "/v1/admin/stats",
        "/v1/admin/treasury-wallets",
        "/v1/admin/pending-withdrawals",
        "/v1/admin/withdrawals",
        "/v1/admin/merchants",
        "/v1/admin/faucetlist",
        "/v1/admin/audit-logs",
        "/v1/admin/telemetry/overview",
        "/v1/admin/rewards/programs",
    ] {
        let (st, body) = get(api_http::app_without_metrics(state.clone()), path, Some(token)).await;
        assert_eq!(st, axum::http::StatusCode::OK, "{path} -> {st} {body}");
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn referral_stats_authenticated(pool: PgPool) {
    let (state, token, _) = register_login(pool, "ref").await;
    let (st, body) = get(api_http::app_without_metrics(state), "/v1/referral/stats", Some(&token)).await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
}
