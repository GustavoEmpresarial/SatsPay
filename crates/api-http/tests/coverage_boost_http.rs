//! Coverage boost: oauth edges, admin 403 sweep, treasury+hot, deposits history map, auth variants.

mod common;

use axum::body::Body;
use base64::Engine;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const HOT_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// `std::env` is process-global, but `#[sqlx::test]` runs every test in this
/// binary in parallel. Tests that set `HOT_MNEMONIC` / `CHAIN_NETWORK` /
/// `SWAP_HOUSE_ENABLED` must therefore not overlap: otherwise one test's
/// mnemonic is read while another builds its `AppState`, and the treasury /
/// swap assertions fail at random. Hold the guard for the whole test, since
/// the chain registry reads these variables lazily.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn oneshot(
    state: api_http::AppState<db::auth::PgAuthRepo>,
    req: axum::http::Request<Body>,
) -> (axum::http::StatusCode, serde_json::Value, axum::http::HeaderMap) {
    let response = api_http::app_without_metrics(state).oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, v, headers)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn oauth_edges_and_basic_auth(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "oedg").await;

    // Discovery / well-known
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/.well-known/openid-configuration")
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert!(body["token_endpoint"].as_str().is_some());

    // Create app
    let (st, created, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/apps")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(
                serde_json::json!({
                    "name": "Edge App",
                    "redirect_uris": ["https://edge.example/cb", "https://edge.example/cb2"]
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{created}");
    let client_id = created["client_id"]
        .as_str()
        .or_else(|| created["app"]["client_id"].as_str())
        .unwrap()
        .to_string();
    let client_secret = created["client_secret"]
        .as_str()
        .or_else(|| created["app"]["client_secret"].as_str())
        .unwrap()
        .to_string();
    let app_id = created["id"]
        .as_str()
        .or_else(|| created["app"]["id"].as_str())
        .unwrap()
        .to_string();

    // Empty name rejected
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/apps")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(serde_json::json!({ "name": "  ", "redirect_uris": ["https://x.example/cb"] }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    // Bad redirect_uris
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/apps")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(serde_json::json!({ "name": "Bad", "redirect_uris": ["not-a-url"] }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    // authorize/info — invalid client
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/oauth/authorize/info?client_id=nope")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // authorize/info — bad redirect_uri
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri(format!(
                "/v1/oauth/authorize/info?client_id={client_id}&redirect_uri=https://evil.example/cb"
            ))
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    // authorize/info — default first redirect + user preview
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri(format!("/v1/oauth/authorize/info?client_id={client_id}"))
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert!(body["user"]["email"].as_str().is_some());

    // authorize submit — invalid client / bad redirect / bad PKCE
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(
                serde_json::json!({
                    "client_id": "missing",
                    "redirect_uri": "https://edge.example/cb",
                    "decision": "approve"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(
                serde_json::json!({
                    "client_id": client_id,
                    "redirect_uri": "https://evil.example/x",
                    "decision": "approve"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(
                serde_json::json!({
                    "client_id": client_id,
                    "redirect_uri": "https://edge.example/cb",
                    "decision": "approve",
                    "code_challenge": "short",
                    "code_challenge_method": "S256"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    // Token: unsupported grant + missing params + Basic auth decode path + invalid grant
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from("grant_type=client_credentials"))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from("grant_type=authorization_code"))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let basic = base64::engine::general_purpose::STANDARD.encode(format!("{client_id}:{client_secret}"));
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/oauth/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("authorization", format!("Basic {basic}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(
                "grant_type=authorization_code&code=deadbeef&redirect_uri=https://edge.example/cb&code_verifier=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // userinfo bad bearer form
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/oauth/userinfo")
            .header("authorization", "Token nope")
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/oauth/userinfo")
            .header("authorization", "Bearer dead-token")
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED);

    // Update + rotate + list authorized
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("PUT")
            .uri(format!("/v1/oauth/apps/{app_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::from(
                serde_json::json!({
                    "name": "Edge App 2",
                    "description": "updated",
                    "redirect_uris": ["https://edge.example/cb"]
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/oauth/apps/{app_id}/rotate-secret"))
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(st.is_success(), "rotate={st}");

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/oauth/authorized-apps")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // Delete unknown app → error arm
    let (st, _, _) = oneshot(
        state,
        axum::http::Request::builder()
            .method("DELETE")
            .uri(format!("/v1/oauth/apps/{}", Uuid::new_v4()))
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.120")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(st.is_server_error() || st == axum::http::StatusCode::NOT_FOUND || st == axum::http::StatusCode::BAD_REQUEST || st == axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_forbidden_sweep_and_treasury_hot(pool: PgPool) {
    let _env = ENV_LOCK.lock().await;
    common::set_hot_addresses(HOT_MNEMONIC);
    std::env::set_var("CHAIN_NETWORK", "mainnet");

    let (state, user_token, user_id, _) = common::register_user(pool.clone(), "afbd").await;
    let (_a, admin_token) = common::admin_login(pool.clone()).await;
    let uid = Uuid::parse_str(&user_id).unwrap();

    // Ensure a deposit address exists so treasury deposit rows spawn onchain checks
    let _ = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/address/POL")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-real-ip", "203.0.113.121")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    let fake = Uuid::new_v4();
    let forbidden_gets = [
        "/v1/admin/stats",
        "/v1/admin/treasury-wallets",
        "/v1/admin/pending-withdrawals",
        "/v1/admin/withdrawals",
        "/v1/admin/merchants",
        "/v1/admin/faucetlist",
        "/v1/admin/audit-logs",
        "/v1/admin/rewards/programs",
        "/v1/admin/telemetry/overview",
        "/v1/admin/telemetry/metrics-history?hours=1",
        "/v1/admin/telemetry/errors",
    ];
    for path in forbidden_gets {
        let (st, body, _) = oneshot(
            state.clone(),
            axum::http::Request::builder()
                .uri(path)
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-real-ip", "203.0.113.121")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{path} {body}");
    }

    let forbidden_posts = [
        format!("/v1/admin/withdrawals/{fake}/approve"),
        format!("/v1/admin/withdrawals/{fake}/reject"),
        format!("/v1/admin/merchants/{uid}/approve"),
        format!("/v1/admin/merchants/{uid}/suspend"),
        format!("/v1/admin/faucetlist/{fake}/approve"),
        format!("/v1/admin/faucetlist/{fake}/suspend"),
        "/v1/admin/house/fund".to_string(),
        "/v1/admin/lend-pool/fund".to_string(),
        "/v1/admin/telemetry/test-error".to_string(),
        "/v1/admin/telemetry/errors/resolve-all".to_string(),
        "/v1/admin/telemetry/errors/clear".to_string(),
    ];
    for path in &forbidden_posts {
        let body = if path.contains("house/fund") || path.contains("lend-pool/fund") {
            serde_json::json!({ "coin": "BTC", "amount": "1" }).to_string()
        } else if path.contains("reject") && path.contains("faucet") {
            serde_json::json!({ "reason": "x" }).to_string()
        } else {
            "{}".into()
        };
        let (st, _, _) = oneshot(
            state.clone(),
            axum::http::Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-real-ip", "203.0.113.121")
                .body(Body::from(body))
                .unwrap(),
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{path}");
    }

    // Reject faucet needs JSON body — also gate with user
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/admin/faucetlist/{fake}/reject"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {user_token}"))
            .header("x-real-ip", "203.0.113.121")
            .body(Body::from(serde_json::json!({ "reason": "nope" }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN);

    // Treasury with hot mnemonic (covers resolve_hot_address Ok + onchain spawn)
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/admin/treasury-wallets")
            .header("authorization", format!("Bearer {admin_token}"))
            .header("x-real-ip", "203.0.113.121")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert!(body["wallets"].as_array().map(|a| !a.is_empty()).unwrap_or(false));

    // Invalid fund payloads
    for path in ["/v1/admin/house/fund", "/v1/admin/lend-pool/fund"] {
        let (st, _, _) = oneshot(
            state.clone(),
            axum::http::Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-real-ip", "203.0.113.121")
                .body(Body::from(serde_json::json!({ "coin": "NOPE", "amount": "x" }).to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{path}");
    }

    // Approve now always demands a step-up OTP first, so a JWT-only call is
    // answered with `codeSent` and never reaches the withdrawal at all.
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/admin/withdrawals/{fake}/approve"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {admin_token}"))
            .header("x-real-ip", "203.0.113.121")
            .body(Body::from(serde_json::json!({}).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["codeSent"], true);

    // With the code, it gets through the gate and hits the real conflict arm.
    let admin_id: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = 'admin@bitcosats.test'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let otp_hash = crypto::hash_password("123456").unwrap();
    sqlx::query(
        "INSERT INTO email_otps (user_id, purpose, code_hash, expires_at) \
         VALUES ($1, 'LOGIN', $2, now() + interval '10 minutes')",
    )
    .bind(admin_id)
    .bind(&otp_hash)
    .execute(&pool)
    .await
    .unwrap();

    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/admin/withdrawals/{fake}/approve"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {admin_token}"))
            .header("x-real-ip", "203.0.113.121")
            .body(Body::from(serde_json::json!({ "emailCode": "123456" }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT, "{body}");

    let (st, _, _) = oneshot(
        state,
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/admin/withdrawals/{fake}/reject"))
            .header("authorization", format!("Bearer {admin_token}"))
            .header("x-real-ip", "203.0.113.121")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT);

    common::clear_hot_addresses();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn deposits_history_maps_rows(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "depmap").await;
    let uid = Uuid::parse_str(&user_id).unwrap();

    // Ensure wallet + address
    let addr = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/deposits/address/POL")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.122")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(addr.status(), axum::http::StatusCode::OK);

    let wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = 'POL'::coin AND kind = 'PERSONAL'",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO deposits (wallet_id, tx_hash, vout, amount, confirmations, status)
         VALUES ($1, $2, 0, 100000, 1, 'PENDING'::deposit_status)",
    )
    .bind(wallet_id)
    .bind(format!("tx{}", Uuid::new_v4().as_simple()))
    .execute(&pool)
    .await
    .unwrap();

    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/deposits/history?coin=POL&limit=10")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.122")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert!(!body["deposits"].as_array().unwrap().is_empty());

    // Unknown coin on address
    let (st, _, _) = oneshot(
        state,
        axum::http::Request::builder()
            .uri("/v1/deposits/address/ZZZ")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.122")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn auth_register_validation_and_admin_2fa(pool: PgPool) {
    let state = common::test_state(pool.clone());

    // Password mismatch
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/auth/register")
            .header("content-type", "application/json")
            .header("x-real-ip", "203.0.113.123")
            .body(Body::from(
                serde_json::json!({
                    "email": format!("mm-{}@bitcosats.test", Uuid::new_v4()),
                    "username": format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]),
                    "password": "Password1234",
                    "confirmPassword": "Password9999",
                    "acceptTerms": true,
                    "captchaToken": "dev-bypass"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // Terms not accepted
    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/auth/register")
            .header("content-type", "application/json")
            .header("x-real-ip", "203.0.113.123")
            .body(Body::from(
                serde_json::json!({
                    "email": format!("tt-{}@bitcosats.test", Uuid::new_v4()),
                    "username": format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]),
                    "password": "Password1234",
                    "confirmPassword": "Password1234",
                    "acceptTerms": false,
                    "captchaToken": "dev-bypass"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // Admin user with 2FA → codeSent on admin login (must be allowlisted email)
    let admin_email = "admin@bitcosats.test";
    sqlx::query(
        "INSERT INTO users (email, password_hash, username, role, two_factor_enabled)
         VALUES ($1, $2, $3, 'ADMIN', true)
         ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash,
           role = 'ADMIN', two_factor_enabled = true",
    )
    .bind(admin_email)
    .bind(crypto::hash_password("Password1234").unwrap())
    .bind(format!("h{}", &Uuid::new_v4().as_simple().to_string()[..12]))
    .execute(&pool)
    .await
    .unwrap();

    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/auth/admin/login")
            .header("content-type", "application/json")
            .header("x-real-ip", "203.0.113.123")
            .header("user-agent", "cov-admin-2fa")
            .body(Body::from(
                serde_json::json!({
                    "email": admin_email,
                    "password": "Password1234",
                    "captchaToken": "dev-bypass"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["codeSent"], true);

    // Admin login fail
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/auth/admin/login")
            .header("content-type", "application/json")
            .header("x-real-ip", "203.0.113.123")
            .body(Body::from(
                serde_json::json!({
                    "email": admin_email,
                    "password": "WrongPassword99",
                    "captchaToken": "dev-bypass"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED);

    // Username conflict
    let (state2, token, _, _) = common::register_user(pool.clone(), "unamec").await;
    let (st, _, _) = oneshot(
        state2,
        axum::http::Request::builder()
            .method("PATCH")
            .uri("/v1/auth/username")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.123")
            .body(Body::from(serde_json::json!({ "username": "ab" }).to_string()))
            .unwrap(),
    )
    .await;
    // short username → validation BAD_REQUEST
    assert!(
        st == axum::http::StatusCode::BAD_REQUEST || st == axum::http::StatusCode::CONFLICT,
        "username update={st}"
    );

    // Refresh missing cookie
    let (st, _, _) = oneshot(
        state,
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/auth/refresh")
            .header("content-type", "application/json")
            .header("origin", "http://localhost:5173")
            .header("x-real-ip", "203.0.113.123")
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn swap_dex_swapkit_api_errors(pool: PgPool) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let _env = ENV_LOCK.lock().await;
    common::set_hot_addresses(HOT_MNEMONIC);
    common::seed_price_cache(&pool).await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/swap"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let state = common::test_state_with_swapkit(pool.clone(), &server.uri());
    let email = format!("dexerr2-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let (st, reg, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/auth/register")
            .header("content-type", "application/json")
            .header("x-real-ip", "203.0.113.124")
            .body(Body::from(
                serde_json::json!({
                    "email": email,
                    "username": username,
                    "password": "Password1234",
                    "confirmPassword": "Password1234",
                    "acceptTerms": true,
                    "captchaToken": "dev-bypass"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{reg}");
    let token = reg["tokens"]["accessToken"].as_str().unwrap();
    let uid = Uuid::parse_str(reg["user"]["id"].as_str().unwrap()).unwrap();
    common::credit_personal(&pool, uid, Coin::Pol, 5_000_000).await;

    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/swap")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.124")
            .body(Body::from(
                serde_json::json!({
                    "fromCoin": "POL",
                    "toCoin": "USDT",
                    "fromAmount": "100000",
                    "expectedToAmount": "1",
                    "idempotencyKey": "dex-api-fail",
                    "source": "swapkit",
                    "provider": "THORCHAIN",
                    "routeId": "r-fail",
                    "platformFeeBps": 50
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // No deposit address in swap response
    let server2 = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/swap"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"routeId":"r2","providers":["THORCHAIN"]}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server2)
        .await;
    let state2 = common::test_state_with_swapkit(pool.clone(), &server2.uri());
    let (st, body, _) = oneshot(
        state2,
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/swap")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.124")
            .body(Body::from(
                serde_json::json!({
                    "fromCoin": "POL",
                    "toCoin": "USDT",
                    "fromAmount": "100000",
                    "expectedToAmount": "1",
                    "idempotencyKey": "dex-no-addr",
                    "source": "swapkit",
                    "provider": "THORCHAIN",
                    "routeId": "r2",
                    "platformFeeBps": 50
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");

    // Contract-call swap response (no deposit address, txType contract)
    let server3 = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/swap"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"routeId":"r3","providers":[],"txType":"contractCall","txHint":"contractCall"}"#,
                )
                .insert_header("content-type", "application/json"),
        )
        .mount(&server3)
        .await;
    let state3 = common::test_state_with_swapkit(pool.clone(), &server3.uri());
    common::credit_personal(&pool, uid, Coin::Pol, 5_000_000).await;
    let (st, body, _) = oneshot(
        state3,
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/swap")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.124")
            .body(Body::from(
                serde_json::json!({
                    "fromCoin": "POL",
                    "toCoin": "USDT",
                    "fromAmount": "100000",
                    "expectedToAmount": "1",
                    "idempotencyKey": "dex-contract",
                    "source": "swapkit",
                    "provider": "UNISWAP",
                    "routeId": "r3",
                    "platformFeeBps": 25
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert!(
        st.is_success() || st.is_client_error(),
        "contract path={st} {body}"
    );

    common::clear_hot_addresses();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn wallet_ledger_and_swap_quote_edges(pool: PgPool) {
    let _env = ENV_LOCK.lock().await;
    std::env::set_var("SWAP_HOUSE_ENABLED", "true");
    common::seed_price_cache(&pool).await;
    let (state, token, user_id, _) = common::register_user(pool.clone(), "wledg").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, Coin::Btc, 3_000_000).await;

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/wallet?kind=DEVELOPER")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/wallet/transfer")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::from(
                serde_json::json!({ "coin": "BTC", "amount": "nope", "toDeveloper": true }).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/wallet/transfer")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::from(
                serde_json::json!({ "coin": "BTC", "amount": "1000", "toDeveloper": true }).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert!(st == axum::http::StatusCode::NO_CONTENT || st == axum::http::StatusCode::BAD_REQUEST);

    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/wallet/ledger?kind=PERSONAL&coin=BTC&take=20")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    assert!(!body["entries"].as_array().unwrap().is_empty());

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/swap/quote?fromCoin=POL&toCoin=POL&fromAmount=1")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri("/v1/swap/quote?fromCoin=POL&toCoin=USDT&fromAmount=0")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/swap/quote")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::from(
                serde_json::json!({ "fromCoin": "POL",
                    "toCoin": "USDT", "fromAmount": "100000" }).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .uri(format!("/v1/swap/orders/{}", Uuid::new_v4()))
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/stake")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::from(serde_json::json!({ "coin": "NOPE", "amount": "x", "lockDays": 7 }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let fake = Uuid::new_v4();
    for path in [
        format!("/v1/stake/{fake}/claim"),
        format!("/v1/stake/{fake}/cancel"),
    ] {
        let (st, _, _) = oneshot(
            state.clone(),
            axum::http::Request::builder()
                .method("POST")
                .uri(&path)
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.125")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{path}");
    }

    let (_a, admin) = common::admin_login(pool).await;
    let (st, _, _) = oneshot(
        state,
        axum::http::Request::builder()
            .uri("/v1/swap/telemetry")
            .header("authorization", format!("Bearer {admin}"))
            .header("x-real-ip", "203.0.113.125")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn faucet_edges_and_rewards_map(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let (state, token, user_id, _) = common::register_user(pool.clone(), "fedg").await;
    let uid = Uuid::parse_str(&user_id).unwrap();

    // claim body: missing coin + unknown coin
    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/faucet/claim")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.130")
            .body(Body::from(serde_json::json!({ "captchaToken": "dev-bypass" }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/faucet/claim")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.130")
            .body(Body::from(serde_json::json!({ "coin": "ZZZ", "captchaToken": "dev-bypass" }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/faucet/claim/NOPE")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.130")
            .body(Body::from(serde_json::json!({ "captchaToken": "dev-bypass" }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);

    // Fund + claim then cooldown branch
    let admin: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, username, role) VALUES ($1,$2,$3,'ADMIN') RETURNING id",
    )
    .bind(format!("fad-{}@bitcosats.test", Uuid::new_v4()))
    .bind(crypto::hash_password("x").unwrap())
    .bind(format!("h{}", &Uuid::new_v4().as_simple().to_string()[..12]))
    .fetch_one(&pool)
    .await
    .unwrap();
    db::admin::fund_house(&pool, Coin::Btc, 5_000_000_000, admin).await.unwrap();

    let (st, _, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/faucet/claim/BTC")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.130")
            .body(Body::from(serde_json::json!({ "captchaToken": "dev-bypass" }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    let (st, body, _) = oneshot(
        state.clone(),
        axum::http::Request::builder()
            .method("POST")
            .uri("/v1/faucet/claim")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.130")
            .body(Body::from(serde_json::json!({ "coin": "BTC", "captchaToken": "dev-bypass" }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::TOO_MANY_REQUESTS, "{body}");

    // Seed reward program + accrual so /v1/rewards maps rows
    if let Ok(prog) = db::rewards::create_program(
        &pool,
        admin,
        Coin::Ltc,
        Coin::Btc,
        "SUPPLY",
        1_000_000,
        None,
        None,
    )
    .await
    {
        let _ = sqlx::query(
            "INSERT INTO reward_accruals (user_id, program_id, claimed_total) VALUES ($1, $2, 12345)
             ON CONFLICT DO NOTHING",
        )
        .bind(uid)
        .bind(prog.id)
        .execute(&pool)
        .await;
    }

    let (st, body, _) = oneshot(
        state,
        axum::http::Request::builder()
            .uri("/v1/rewards")
            .header("authorization", format!("Bearer {token}"))
            .header("x-real-ip", "203.0.113.130")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
}
