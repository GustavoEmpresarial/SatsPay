//! Thin axum routes for auth — parse request, call `domain::auth::AuthService`,
//! serialize response. Port of legacy `auth.controller.ts`/`auth.routes.ts`.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use crate::client_ip::ClientIp;
use crate::middleware::AuthUser;
use crate::state::AppState;
use domain::auth::{AuthError, AuthRepo, LoginResult};
use serde::{Deserialize, Serialize};

/// Legacy cookie name (pre-`__Host-`). Still accepted on read / cleared on logout.
const REFRESH_COOKIE_LEGACY: &str = "refresh_token";
/// OWASP ASVS: `__Host-` requires Secure, Path=/, no Domain — blocks subdomain cookie forging.
const REFRESH_COOKIE_HOST: &str = "__Host-refresh_token";
/// 30 days — mirrors a typical refresh TTL; the DB row is the real authority.
const REFRESH_COOKIE_MAX_AGE_SECS: i64 = 60 * 60 * 24 * 30;

/// Must match the Turnstile widget `action` on LoginPage / RegisterPage.
pub const LOGIN_TURNSTILE_ACTION: &str = "login";
pub const REGISTER_TURNSTILE_ACTION: &str = "register";

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/v1/auth/register", post(register::<R>))
        .route("/v1/auth/login", post(login::<R>))
        .route("/v1/auth/admin/login", post(admin_login::<R>))
        .route("/v1/auth/admin/logout", post(logout::<R>))
        .route("/v1/auth/refresh", post(refresh::<R>))
        .route("/v1/auth/logout", post(logout::<R>))
        .route("/v1/auth/me", get(me::<R>))
        .route("/v1/auth/username", patch(update_username::<R>))
        .route("/v1/auth/security-logs", get(security_logs::<R>))
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    username: String,
    password: String,
    #[serde(rename = "confirmPassword")]
    confirm_password: String,
    #[serde(rename = "acceptTerms")]
    accept_terms: bool,
    #[serde(rename = "referralCode")]
    referral_code: Option<String>,
    #[serde(rename = "captchaToken")]
    captcha_token: Option<String>,
}

/// Access token only — refresh lives exclusively in the HttpOnly cookie (never JSON).
#[derive(Serialize)]
struct TokensResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
}

#[derive(Serialize)]
struct UserResponse {
    id: String,
    email: String,
    username: String,
    #[serde(rename = "twoFactorEnabled")]
    two_factor_enabled: bool,
    #[serde(rename = "merchantStatus")]
    merchant_status: String,
    #[serde(rename = "createdAt")]
    created_at: String,
}

fn user_response(user: &domain::auth::UserRow) -> UserResponse {
    UserResponse {
        id: user.id.to_string(),
        email: user.email.clone(),
        username: user.username.clone(),
        two_factor_enabled: user.two_factor_enabled,
        merchant_status: user.merchant_status.clone(),
        created_at: user.created_at.to_rfc3339(),
    }
}

fn extract_user_agent(headers: &HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

fn cookie_secure_enabled() -> bool {
    let forced = std::env::var("COOKIE_SECURE")
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "yes" || t == "on"
        })
        .unwrap_or(false);
    let prod = std::env::var("NODE_ENV").unwrap_or_default() == "production";
    forced || prod
}

/// Production → `__Host-refresh_token` (+ Secure). Local HTTP → legacy name (browsers reject `__Host-` without Secure).
fn refresh_cookie_name() -> &'static str {
    if cookie_secure_enabled() {
        REFRESH_COOKIE_HOST
    } else {
        REFRESH_COOKIE_LEGACY
    }
}

fn set_refresh_cookie(token: &str) -> HeaderValue {
    let name = refresh_cookie_name();
    let secure = if cookie_secure_enabled() { "; Secure" } else { "" };
    let value = format!(
        "{name}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={REFRESH_COOKIE_MAX_AGE_SECS}{secure}"
    );
    HeaderValue::from_str(&value).expect("refresh cookie header is ASCII")
}

fn clear_cookie_header(name: &str, secure: bool) -> HeaderValue {
    let secure_attr = if secure { "; Secure" } else { "" };
    let value = format!("{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_attr}");
    HeaderValue::from_str(&value).expect("clear refresh cookie header is ASCII")
}

fn append_clear_refresh_cookies(res: &mut Response) {
    // Always clear both names so upgrades from legacy → __Host- leave no orphans.
    res.headers_mut()
        .append(header::SET_COOKIE, clear_cookie_header(REFRESH_COOKIE_LEGACY, cookie_secure_enabled()));
    res.headers_mut()
        .append(header::SET_COOKIE, clear_cookie_header(REFRESH_COOKIE_HOST, true));
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').map(str::trim).find_map(|part| {
        let (k, v) = part.split_once('=')?;
        (k == name).then_some(v)
    })
}

fn extract_refresh_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, REFRESH_COOKIE_HOST)
        .or_else(|| cookie_value(headers, REFRESH_COOKIE_LEGACY))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn json_with_refresh_cookie(status: StatusCode, body: serde_json::Value, refresh_token: &str) -> Response {
    let mut res = (status, Json(body)).into_response();
    if cookie_secure_enabled() {
        // Drop legacy cookie if present so only `__Host-` remains.
        res.headers_mut()
            .append(header::SET_COOKIE, clear_cookie_header(REFRESH_COOKIE_LEGACY, true));
    }
    res.headers_mut()
        .append(header::SET_COOKIE, set_refresh_cookie(refresh_token));
    res
}

async fn register<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> Response {
    let ua = extract_user_agent(&headers);

    // Verify Cloudflare Turnstile Captcha (action pinned to registration widget)
    let captcha_token = body.captcha_token.as_deref().unwrap_or("");
    let captcha_result = state
        .captcha
        .verify(
            &state.pool,
            captcha_token,
            Some(&ip),
            captcha::VerifyOptions {
                expected_action: Some(REGISTER_TURNSTILE_ACTION),
                expected_hostnames: &state.settings.captcha_expected_hostnames,
            },
        )
        .await;
    let captcha_ok = match captcha_result {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "code": "CAPTCHA_MISCONFIGURED",
                    "message": e.to_string()
                })),
            )
                .into_response();
        }
    };

    if !captcha_ok {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "code": "CAPTCHA_REQUIRED",
                "message": "Falha na verificação de segurança (Cloudflare Turnstile). Por favor, tente novamente."
            })),
        )
            .into_response();
    }

    match state
        .auth
        .register(
            &body.email,
            &body.username,
            &body.password,
            &body.confirm_password,
            body.accept_terms,
        )
        .await
    {
        Ok((user, tokens)) => {
            if let Some(ref code) = body.referral_code {
                let pool = state.pool.clone();
                let ref_code = code.clone();
                let user_id = user.id;
                tokio::spawn(async move {
                    let _ = db::referral::link_referred_user(&pool, &ref_code, user_id).await;
                });
            }
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "AUTH_REGISTER".into(),
                "User".into(),
                Some(user.id),
                Some(ip),
                Some(serde_json::json!({
                    "email": body.email,
                    "username": body.username,
                    "referralCode": body.referral_code,
                    "userAgent": ua
                })),
            );
            let payload = serde_json::json!({
                "user": user_response(&user),
                "tokens": TokensResponse { access_token: tokens.access_token.clone() },
            });
            json_with_refresh_cookie(StatusCode::CREATED, payload, &tokens.refresh_token)
        }
        Err(err) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                None,
                "AUTH_REGISTER_FAILED".into(),
                "User".into(),
                None,
                Some(ip),
                Some(serde_json::json!({
                    "attemptedEmail": body.email,
                    "attemptedUsername": body.username,
                    "reason": err.to_string(),
                    "userAgent": ua
                })),
            );
            auth_error_response(err)
        }
    }
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
    #[serde(rename = "emailCode")]
    email_code: Option<String>,
    #[serde(rename = "captchaToken")]
    captcha_token: Option<String>,
}

async fn login<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Response {
    let ua = extract_user_agent(&headers);

    // Verify Cloudflare Turnstile Captcha (action pinned to login widget)
    let captcha_token = body.captcha_token.as_deref().unwrap_or("");
    let captcha_result = state
        .captcha
        .verify(
            &state.pool,
            captcha_token,
            Some(&ip),
            captcha::VerifyOptions {
                expected_action: Some(LOGIN_TURNSTILE_ACTION),
                expected_hostnames: &state.settings.captcha_expected_hostnames,
            },
        )
        .await;
    let captcha_ok = match captcha_result {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "code": "CAPTCHA_MISCONFIGURED",
                    "message": e.to_string()
                })),
            )
                .into_response();
        }
    };

    if !captcha_ok {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "code": "CAPTCHA_REQUIRED",
                "message": "Falha na verificação de segurança (Cloudflare Turnstile). Por favor, tente novamente."
            })),
        )
            .into_response();
    }
    match state.auth.login(&body.email, &body.password, body.email_code.as_deref(), false).await {
        Ok(LoginResult::Ok { user, tokens }) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "AUTH_LOGIN_SUCCESS".into(),
                "User".into(),
                Some(user.id),
                Some(ip),
                Some(serde_json::json!({
                    "email": user.email,
                    "twoFactor": user.two_factor_enabled,
                    "userAgent": ua
                })),
            );
            let payload = serde_json::json!({
                "kind": "ok",
                "user": user_response(&user),
                "tokens": TokensResponse { access_token: tokens.access_token.clone() },
            });
            json_with_refresh_cookie(StatusCode::OK, payload, &tokens.refresh_token)
        }
        Ok(LoginResult::CodeSent { email }) => {
            let user_id = lookup_user_id_by_email(&state.pool, &body.email).await;
            db::audit::record_log_spawned(
                state.pool.clone(),
                user_id,
                "AUTH_2FA_CODE_SENT".into(),
                "User".into(),
                user_id,
                Some(ip),
                Some(serde_json::json!({
                    "email": body.email,
                    "userAgent": ua
                })),
            );
            (StatusCode::OK, Json(serde_json::json!({ "kind": "code_sent", "email": email }))).into_response()
        }
        Err(err) => {
            let user_id = lookup_user_id_by_email(&state.pool, &body.email).await;
            db::audit::record_log_spawned(
                state.pool.clone(),
                user_id,
                "AUTH_LOGIN_FAILED".into(),
                "User".into(),
                user_id,
                Some(ip),
                Some(serde_json::json!({
                    "attemptedEmail": body.email,
                    "reason": err.to_string(),
                    "userAgent": ua
                })),
            );
            auth_error_response(err)
        }
    }
}

async fn lookup_user_id_by_email(pool: &sqlx::PgPool, email: &str) -> Option<uuid::Uuid> {
    sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = lower($1)")
        .bind(email)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn admin_login<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Response {
    let ua = extract_user_agent(&headers);
    match state.auth.login(&body.email, &body.password, body.email_code.as_deref(), true).await {
        Ok(LoginResult::Ok { user, tokens }) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "AUTH_ADMIN_LOGIN_SUCCESS".into(),
                "Admin".into(),
                Some(user.id),
                Some(ip),
                Some(serde_json::json!({
                    "email": user.email,
                    "role": user.role,
                    "userAgent": ua
                })),
            );
            let payload = serde_json::json!({
                "user": {
                    "id": user.id,
                    "email": user.email,
                    "role": user.role
                },
                "accessToken": tokens.access_token.clone(),
            });
            json_with_refresh_cookie(StatusCode::OK, payload, &tokens.refresh_token)
        }
        Ok(LoginResult::CodeSent { email }) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                None,
                "AUTH_ADMIN_2FA_CODE_SENT".into(),
                "Admin".into(),
                None,
                Some(ip),
                Some(serde_json::json!({
                    "email": body.email,
                    "userAgent": ua
                })),
            );
            (StatusCode::OK, Json(serde_json::json!({ "codeSent": true, "email": email }))).into_response()
        }
        Err(err) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                None,
                "AUTH_ADMIN_LOGIN_FAILED".into(),
                "Admin".into(),
                None,
                Some(ip),
                Some(serde_json::json!({
                    "attemptedEmail": body.email,
                    "reason": err.to_string(),
                    "userAgent": ua
                })),
            );
            auth_error_response(err)
        }
    }
}

#[derive(Deserialize, Default)]
struct RefreshRequest {
    /// Deprecated: refresh is cookie-only. Accepted only to ignore leftover clients.
    #[serde(rename = "refreshToken", alias = "refresh_token", default)]
    _refresh_token: Option<String>,
}

async fn refresh<R: AuthRepo>(State(state): State<AppState<R>>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(err) = crate::csrf::assert_browser_csrf(&headers, &state.settings.captcha_expected_hostnames) {
        return err.into_response();
    }
    let _ = serde_json::from_slice::<RefreshRequest>(&body);
    let Some(refresh_token) = extract_refresh_cookie(&headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "missing refresh token" }))).into_response();
    };

    match state.auth.refresh(&refresh_token).await {
        Ok(tokens) => {
            let payload = serde_json::json!(TokensResponse {
                access_token: tokens.access_token.clone(),
            });
            json_with_refresh_cookie(StatusCode::OK, payload, &tokens.refresh_token)
        }
        Err(err) => auth_error_response(err),
    }
}

async fn logout<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    crate::middleware::OptionalAuthUser(user): crate::middleware::OptionalAuthUser,
    body: Bytes,
) -> Response {
    if let Err(err) = crate::csrf::assert_browser_csrf(&headers, &state.settings.captcha_expected_hostnames) {
        return err.into_response();
    }
    let ua = extract_user_agent(&headers);
    let _ = serde_json::from_slice::<RefreshRequest>(&body);
    if let Some(refresh_token) = extract_refresh_cookie(&headers) {
        let _ = state.auth.revoke_refresh_token(&refresh_token).await;
    }

    db::audit::record_log_spawned(
        state.pool.clone(),
        user.as_ref().map(|u| u.id),
        "AUTH_LOGOUT".into(),
        "Session".into(),
        user.as_ref().map(|u| u.id),
        Some(ip),
        Some(serde_json::json!({ "userAgent": ua })),
    );

    let mut res = StatusCode::NO_CONTENT.into_response();
    append_clear_refresh_cookies(&mut res);
    res
}

async fn me<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match state.auth.get_user_by_id(user.id).await {
        Ok(Some(u)) => Json(serde_json::json!({ "user": user_response(&u) })).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "user not found" }))).into_response(),
        Err(err) => auth_error_response(err),
    }
}

#[derive(Deserialize)]
struct UpdateUsernameRequest {
    username: String,
}

async fn update_username<R: AuthRepo>(
    State(state): State<AppState<R>>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    user: AuthUser,
    Json(body): Json<UpdateUsernameRequest>,
) -> Response {
    let ua = extract_user_agent(&headers);
    match state.auth.update_username(user.id, &body.username).await {
        Ok(u) => {
            db::audit::record_log_spawned(
                state.pool.clone(),
                Some(user.id),
                "AUTH_UPDATE_USERNAME".into(),
                "User".into(),
                Some(user.id),
                Some(ip),
                Some(serde_json::json!({
                    "newUsername": body.username,
                    "userAgent": ua
                })),
            );
            Json(serde_json::json!({ "user": user_response(&u) })).into_response()
        }
        Err(err) => auth_error_response(err),
    }
}

async fn security_logs<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    match db::audit::list_user_logs(&state.pool, user.id, 50).await {
        Ok(logs) => Json(serde_json::json!({ "logs": logs })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

fn auth_error_response(err: AuthError) -> Response {
    let status = match &err {
        AuthError::Forbidden => StatusCode::FORBIDDEN,
        AuthError::Conflict => StatusCode::CONFLICT,
        AuthError::InvalidCredentials | AuthError::Invalid2fa => StatusCode::UNAUTHORIZED,
        AuthError::Unauthorized | AuthError::RefreshReuseDetected => StatusCode::UNAUTHORIZED,
        AuthError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        AuthError::NotFound => StatusCode::NOT_FOUND,
        AuthError::PasswordMismatch | AuthError::TermsNotAccepted | AuthError::Validation(_) => {
            StatusCode::BAD_REQUEST
        }
        AuthError::Repo(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let (code, message) = match &err {
        AuthError::PasswordMismatch => ("PASSWORD_MISMATCH", err.to_string()),
        AuthError::TermsNotAccepted => ("TERMS_NOT_ACCEPTED", err.to_string()),
        AuthError::Validation(msg) => ("VALIDATION_ERROR", msg.clone()),
        AuthError::Forbidden => ("FORBIDDEN", err.to_string()),
        AuthError::Conflict => ("CONFLICT", err.to_string()),
        AuthError::InvalidCredentials => ("INVALID_CREDENTIALS", err.to_string()),
        AuthError::Invalid2fa => ("INVALID_2FA", err.to_string()),
        AuthError::Unauthorized | AuthError::RefreshReuseDetected => ("UNAUTHORIZED", err.to_string()),
        AuthError::RateLimited => ("RATE_LIMITED", err.to_string()),
        AuthError::NotFound => ("NOT_FOUND", err.to_string()),
        AuthError::Repo(_) => ("INTERNAL", err.to_string()),
    };
    (
        status,
        Json(serde_json::json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static COOKIE_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn turnstile_actions_match_frontend_widgets() {
        assert_eq!(LOGIN_TURNSTILE_ACTION, "login");
        assert_eq!(REGISTER_TURNSTILE_ACTION, "register");
    }

    #[test]
    fn refresh_cookie_is_httponly_samesite_lax() {
        let _guard = COOKIE_ENV_LOCK.lock().unwrap();
        std::env::remove_var("COOKIE_SECURE");
        std::env::remove_var("NODE_ENV");
        let v = set_refresh_cookie("tok").to_str().unwrap().to_string();
        assert!(v.starts_with("refresh_token="));
        assert!(v.contains("HttpOnly"));
        assert!(v.contains("SameSite=Lax"));
        assert!(!v.contains("Secure"));
        assert!(!v.contains("__Host-"));
    }

    #[test]
    fn refresh_cookie_uses_host_prefix_in_production() {
        let _guard = COOKIE_ENV_LOCK.lock().unwrap();
        std::env::set_var("NODE_ENV", "production");
        std::env::remove_var("COOKIE_SECURE");
        let v = set_refresh_cookie("tok").to_str().unwrap().to_string();
        assert!(v.starts_with("__Host-refresh_token="));
        assert!(v.contains("; Secure"));
        assert!(v.contains("Path=/"));
        assert!(!v.contains("Domain="));
        std::env::remove_var("NODE_ENV");
    }

    #[test]
    fn tokens_response_omits_refresh_token() {
        let json = serde_json::to_value(TokensResponse {
            access_token: "a".into(),
        })
        .unwrap();
        assert_eq!(json["accessToken"], "a");
        assert!(json.get("refreshToken").is_none());
    }

    #[test]
    fn auth_error_response_maps_all_variants() {
        use domain::auth::AuthError;
        let cases: Vec<(AuthError, StatusCode, &str)> = vec![
            (AuthError::PasswordMismatch, StatusCode::BAD_REQUEST, "PASSWORD_MISMATCH"),
            (AuthError::TermsNotAccepted, StatusCode::BAD_REQUEST, "TERMS_NOT_ACCEPTED"),
            (AuthError::Validation("x".into()), StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            (AuthError::Forbidden, StatusCode::FORBIDDEN, "FORBIDDEN"),
            (AuthError::Conflict, StatusCode::CONFLICT, "CONFLICT"),
            (AuthError::InvalidCredentials, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS"),
            (AuthError::Invalid2fa, StatusCode::UNAUTHORIZED, "INVALID_2FA"),
            (AuthError::Unauthorized, StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            (AuthError::RefreshReuseDetected, StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            (AuthError::RateLimited, StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
            (AuthError::NotFound, StatusCode::NOT_FOUND, "NOT_FOUND"),
            (AuthError::Repo(domain::auth::RepoError("boom".into())), StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        ];
        for (err, want_status, want_code) in cases {
            let res = auth_error_response(err);
            assert_eq!(res.status(), want_status, "code={want_code}");
        }
    }

    #[test]
    fn cookie_helpers_cover_secure_and_extract() {
        let _guard = COOKIE_ENV_LOCK.lock().unwrap();
        std::env::set_var("COOKIE_SECURE", "1");
        std::env::remove_var("NODE_ENV");
        assert!(cookie_secure_enabled());
        let set = set_refresh_cookie("abc").to_str().unwrap().to_string();
        assert!(set.contains("Secure"));
        let clear = clear_cookie_header(REFRESH_COOKIE_LEGACY, true).to_str().unwrap().to_string();
        assert!(clear.contains("Max-Age=0"));

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("{REFRESH_COOKIE_HOST}=hosttok; {REFRESH_COOKIE_LEGACY}=legtok")
                .parse()
                .unwrap(),
        );
        assert_eq!(extract_refresh_cookie(&headers).as_deref(), Some("hosttok"));
        headers.clear();
        headers.insert(header::COOKIE, format!("{REFRESH_COOKIE_LEGACY}=only").parse().unwrap());
        assert_eq!(extract_refresh_cookie(&headers).as_deref(), Some("only"));

        let mut res = json_with_refresh_cookie(StatusCode::OK, serde_json::json!({ "ok": true }), "rt");
        assert!(res.headers().get(header::SET_COOKIE).is_some());
        append_clear_refresh_cookies(&mut res);

        std::env::remove_var("COOKIE_SECURE");
    }
}
