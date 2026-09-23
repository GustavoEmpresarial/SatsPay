//! 2FA toggle (`/v1/auth/2fa/*`), "sign out other devices"
//! (`/v1/auth/sessions/revoke-others`), `CLIENT_IP_UNAVAILABLE` and the
//! `X-Request-Id` correlation header.
//!
//! Before these routes existed the Settings page's 2FA tab got a 404 and its
//! "disconnect other devices" button only faked success on the client.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

struct Resp {
    status: StatusCode,
    body: Value,
    cookie: String,
    request_id: Option<String>,
}

fn refresh_cookie(res: &axum::response::Response) -> String {
    res.headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|sc| {
            let pair = sc.split(';').next()?.trim();
            let is_refresh = pair.starts_with("refresh_token=") || pair.starts_with("__Host-refresh_token=");
            (is_refresh && !pair.ends_with('=')).then(|| pair.to_string())
        })
        .collect::<Vec<_>>()
        .join("; ")
}

async fn send(
    state: &api_http::AppState<db::auth::PgAuthRepo>,
    method: &str,
    uri: &str,
    token: Option<&str>,
    cookie: Option<&str>,
    origin: Option<&str>,
    body: Option<Value>,
) -> Resp {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-real-ip", "203.0.113.77");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    if let Some(c) = cookie {
        b = b.header("cookie", c);
    }
    if let Some(o) = origin {
        b = b.header("origin", o);
    }
    let body = body.map(|v| v.to_string()).unwrap_or_else(|| "{}".into());
    let res = api_http::app_without_metrics(state.clone())
        .oneshot(b.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let cookie = refresh_cookie(&res);
    let request_id = res.headers().get("x-request-id").and_then(|v| v.to_str().ok()).map(str::to_string);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Resp { status, body, cookie, request_id }
}

async fn seed_otp(pool: &PgPool, user_id: Uuid, purpose: &str, code: &str) {
    let hash = crypto::hash_password(code).unwrap();
    sqlx::query(
        "INSERT INTO email_otps (user_id, purpose, code_hash, expires_at) \
         VALUES ($1, $2::otp_purpose, $3, now() + interval '10 minutes')",
    )
    .bind(user_id)
    .bind(purpose)
    .bind(&hash)
    .execute(pool)
    .await
    .unwrap();
}

async fn two_factor_enabled(pool: &PgPool, user_id: Uuid) -> bool {
    sqlx::query_scalar("SELECT two_factor_enabled FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Audit rows are written by a spawned task; poll briefly.
async fn audit_count(pool: &PgPool, user_id: Uuid, action: &str) -> i64 {
    for _ in 0..40 {
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_logs WHERE user_id = $1 AND action = $2")
            .bind(user_id)
            .bind(action)
            .fetch_one(pool)
            .await
            .unwrap();
        if n > 0 {
            return n;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    0
}

#[sqlx::test(migrations = "../db/migrations")]
async fn twofa_requires_auth(pool: PgPool) {
    let state = common::test_state(pool);
    for path in ["/v1/auth/2fa/request", "/v1/auth/2fa/enable", "/v1/auth/2fa/disable", "/v1/auth/sessions/revoke-others"] {
        let r = send(&state, "POST", path, None, None, None, Some(json!({ "purpose": "ENABLE_2FA", "code": "123456" }))).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{path} {}", r.body);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn twofa_request_validates_purpose_and_state(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool.clone(), "2fa-req").await;

    // Only the two toggle purposes are accepted here.
    for bad in ["WITHDRAWAL", "LOGIN", "", "enable_2fa"] {
        let r = send(&state, "POST", "/v1/auth/2fa/request", Some(&token), None, None, Some(json!({ "purpose": bad }))).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{bad}: {}", r.body);
        assert_eq!(r.body["code"], "VALIDATION_ERROR");
    }

    // Disabling something that is off is a no-op conflict, and sends no mail.
    let r = send(&state, "POST", "/v1/auth/2fa/request", Some(&token), None, None, Some(json!({ "purpose": "DISABLE_2FA" }))).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert_eq!(r.body["code"], "TWO_FACTOR_NOT_ENABLED");

    let r = send(&state, "POST", "/v1/auth/2fa/request", Some(&token), None, None, Some(json!({ "purpose": "ENABLE_2FA" }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["codeSent"], true);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn twofa_enable_and_disable_round_trip(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "2fa-flow").await;
    let user_id: Uuid = user_id.parse().unwrap();
    assert!(!two_factor_enabled(&pool, user_id).await);

    // Wrong code: stays off, failure is audited.
    seed_otp(&pool, user_id, "ENABLE_2FA", "123456").await;
    let r = send(&state, "POST", "/v1/auth/2fa/enable", Some(&token), None, None, Some(json!({ "code": "000000" }))).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "INVALID_2FA");
    assert!(!two_factor_enabled(&pool, user_id).await);
    assert!(audit_count(&pool, user_id, "AUTH_2FA_ENABLE_FAILED").await >= 1);

    // A code minted for another purpose does not enable 2FA.
    seed_otp(&pool, user_id, "WITHDRAWAL", "654321").await;
    let r = send(&state, "POST", "/v1/auth/2fa/enable", Some(&token), None, None, Some(json!({ "code": "654321" }))).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.body);
    assert!(!two_factor_enabled(&pool, user_id).await);

    // Right code (whitespace tolerated): on.
    let r = send(&state, "POST", "/v1/auth/2fa/enable", Some(&token), None, None, Some(json!({ "code": " 123456 " }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["twoFactorEnabled"], true);
    assert!(two_factor_enabled(&pool, user_id).await);
    assert!(audit_count(&pool, user_id, "AUTH_2FA_ENABLED").await >= 1);

    // Replaying the enable is a state conflict, not a second toggle.
    let r = send(&state, "POST", "/v1/auth/2fa/enable", Some(&token), None, None, Some(json!({ "code": "123456" }))).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.body);
    assert_eq!(r.body["code"], "TWO_FACTOR_ALREADY_ENABLED");

    // The ENABLE code was single-use: it cannot disable either.
    let r = send(&state, "POST", "/v1/auth/2fa/disable", Some(&token), None, None, Some(json!({ "code": "123456" }))).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.body);
    assert!(two_factor_enabled(&pool, user_id).await);

    seed_otp(&pool, user_id, "DISABLE_2FA", "111222").await;
    let r = send(&state, "POST", "/v1/auth/2fa/disable", Some(&token), None, None, Some(json!({ "code": "111222" }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["twoFactorEnabled"], false);
    assert!(!two_factor_enabled(&pool, user_id).await);
    assert!(audit_count(&pool, user_id, "AUTH_2FA_DISABLED").await >= 1);

    // /auth/me reflects it.
    let r = send(&state, "GET", "/v1/auth/me", Some(&token), None, None, None).await;
    assert_eq!(r.body["user"]["twoFactorEnabled"], false);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn twofa_code_is_locked_after_max_attempts(pool: PgPool) {
    let (state, token, user_id, _) = common::register_user(pool.clone(), "2fa-brute").await;
    let user_id: Uuid = user_id.parse().unwrap();
    seed_otp(&pool, user_id, "ENABLE_2FA", "123456").await;
    for _ in 0..10 {
        let _ = send(&state, "POST", "/v1/auth/2fa/enable", Some(&token), None, None, Some(json!({ "code": "999999" }))).await;
    }
    // Brute force burned the code: even the right one fails now.
    let r = send(&state, "POST", "/v1/auth/2fa/enable", Some(&token), None, None, Some(json!({ "code": "123456" }))).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.body);
    assert!(!two_factor_enabled(&pool, user_id).await);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn revoke_others_kills_other_refresh_tokens_and_keeps_caller(pool: PgPool) {
    // Production grace window: this is where a *revoked* (instead of deleted)
    // token would still be honoured for an attacker.
    let state = common::test_state_with_reuse_grace(pool.clone(), 60);
    let email = format!("rev-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);
    let reg = send(
        &state,
        "POST",
        "/v1/auth/register",
        None,
        None,
        None,
        Some(json!({
            "email": email, "username": username, "password": "Password1234",
            "confirmPassword": "Password1234", "acceptTerms": true, "captchaToken": "dev-bypass"
        })),
    )
    .await;
    assert_eq!(reg.status, StatusCode::CREATED, "{}", reg.body);
    let token_a = reg.body["tokens"]["accessToken"].as_str().unwrap().to_string();
    let user_id: Uuid = reg.body["user"]["id"].as_str().unwrap().parse().unwrap();
    let cookie_a = reg.cookie.clone();
    assert!(!cookie_a.is_empty());

    // Second device.
    let login = send(
        &state,
        "POST",
        "/v1/auth/login",
        None,
        None,
        None,
        Some(json!({ "email": email, "password": "Password1234", "captchaToken": "dev-bypass" })),
    )
    .await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    let cookie_b = login.cookie.clone();
    assert!(!cookie_b.is_empty() && cookie_b != cookie_a);

    // Cross-site origin is refused by the CSRF gate (the call re-issues a cookie).
    let r = send(&state, "POST", "/v1/auth/sessions/revoke-others", Some(&token_a), Some(&cookie_a), Some("https://evil.example"), None).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.body);
    let r = send(&state, "POST", "/v1/auth/refresh", None, Some(&cookie_b), Some("http://localhost:5173"), None).await;
    assert_eq!(r.status, StatusCode::OK, "rejected CSRF call must not have revoked anything: {}", r.body);
    let cookie_b = r.cookie; // refresh rotated it

    let r = send(&state, "POST", "/v1/auth/sessions/revoke-others", Some(&token_a), Some(&cookie_a), Some("http://localhost:5173"), None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["revoked"], true);
    assert!(r.body["accessToken"].as_str().is_some_and(|t| !t.is_empty()));
    assert!(r.body.get("refreshToken").is_none(), "refresh token never travels in JSON");
    let cookie_c = r.cookie.clone();
    assert!(!cookie_c.is_empty() && cookie_c != cookie_a);
    assert!(audit_count(&pool, user_id, "AUTH_SESSIONS_REVOKED").await >= 1);

    // The other device (e.g. an attacker who stole it) refreshes right away,
    // inside the grace window: it must NOT get a session.
    let r = send(&state, "POST", "/v1/auth/refresh", None, Some(&cookie_b), Some("http://localhost:5173"), None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "old device must be out even inside the grace window: {}", r.body);
    assert!(r.cookie.is_empty(), "no session may be minted for the old device");
    // …and its attempt must not look like theft that cascades to the caller.
    assert_ne!(
        r.body["error"]["message"], "refresh token reuse detected; session revoked",
        "a user-initiated sign-out must not trip reuse detection"
    );

    // The caller survives the old device's attempt and keeps refreshing.
    let r = send(&state, "POST", "/v1/auth/refresh", None, Some(&cookie_c), Some("http://localhost:5173"), None).await;
    assert_eq!(r.status, StatusCode::OK, "caller must stay logged in after the old device retries: {}", r.body);
    let cookie_c = r.cookie;
    let r = send(&state, "POST", "/v1/auth/refresh", None, Some(&cookie_c), Some("http://localhost:5173"), None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    // Exactly one live refresh token remains: the caller's.
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(live, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn missing_client_ip_is_400_and_every_response_has_request_id(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "noip").await;

    // PATCH /auth/username takes ClientIp; strip every IP source.
    let res = api_http::app_without_metrics(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/auth/username")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-request-id", "support-ticket-42")
                .body(Body::from(json!({ "username": "whatever1" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(res.headers()["x-request-id"], "support-ticket-42");
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "CLIENT_IP_UNAVAILABLE");

    let r = send(&state, "GET", "/v1/auth/me", Some(&token), None, None, None).await;
    assert!(r.request_id.is_some_and(|id| id.starts_with("req_")));
}
