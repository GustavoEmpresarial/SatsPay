use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Form, Json, Router};
use domain::auth::AuthRepo;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        // Standard OIDC Discovery
        .route("/.well-known/openid-configuration", get(openid_configuration))
        // OAuth 2.0 Flow Endpoints
        .route("/v1/oauth/authorize/info", get(authorize_info::<R>))
        .route("/v1/oauth/authorize", axum::routing::post(authorize_submit::<R>))
        .route("/v1/oauth/token", axum::routing::post(token_exchange::<R>))
        .route("/v1/oauth/userinfo", get(userinfo::<R>))
        // Developer App Management (Protected)
        .route("/v1/oauth/apps", get(list_apps::<R>).post(create_app::<R>))
        .route(
            "/v1/oauth/apps/:id",
            axum::routing::put(update_app::<R>).delete(delete_app::<R>),
        )
        .route("/v1/oauth/apps/:id/rotate-secret", axum::routing::post(rotate_secret::<R>))
        // User Authorized Apps / SSO Consents (Protected)
        .route("/v1/oauth/authorized-apps", get(list_authorized_apps::<R>))
        .route("/v1/oauth/authorized-apps/:id", axum::routing::delete(revoke_authorized_app::<R>))
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

// ---------------------------------------------------------------------------
// OpenID Connect Discovery
// ---------------------------------------------------------------------------

async fn openid_configuration() -> impl IntoResponse {
    let issuer = "https://www.satspay.pro";
    let config = serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/oauth/authorize"),
        "token_endpoint": format!("{issuer}/v1/oauth/token"),
        "userinfo_endpoint": format!("{issuer}/v1/oauth/userinfo"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["HS256"],
        "scopes_supported": ["openid", "profile", "email"],
        "token_endpoint_auth_methods_supported": ["client_secret_post", "client_secret_basic"],
        "claims_supported": ["sub", "name", "preferred_username", "email", "email_verified", "picture"]
    });

    (
        [(header::CONTENT_TYPE, "application/json")],
        Json(config),
    )
}

// ---------------------------------------------------------------------------
// OAuth 2.0 Authorize Flow
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AuthorizeInfoQuery {
    client_id: String,
    redirect_uri: Option<String>,
    scope: Option<String>,
}

#[derive(Serialize)]
struct AuthorizeInfoResponse {
    app_id: String,
    app_name: String,
    description: Option<String>,
    website_url: Option<String>,
    logo_url: Option<String>,
    redirect_uri: String,
    scopes: Vec<String>,
    user: Option<CurrentUserPreview>,
}

#[derive(Serialize)]
struct CurrentUserPreview {
    id: String,
    username: String,
    email: String,
}

async fn authorize_info<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth_user: Option<AuthUser>,
    Query(params): Query<AuthorizeInfoQuery>,
) -> Result<Json<AuthorizeInfoResponse>, (StatusCode, Json<serde_json::Value>)> {
    let app = db::oauth::get_application_by_client_id(&state.pool, &params.client_id)
        .await
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid_client", "error_description": "Application not found or inactive" })),
            )
        })?;

    // Determine redirect_uri: if provided, must be in allowed list; otherwise use first registered
    let redirect_uri = match params.redirect_uri {
        Some(ref uri) if !uri.is_empty() => {
            crate::oauth_redirect::assert_redirect_uri_allowed(&app.redirect_uris, uri).map_err(
                |msg| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": "invalid_redirect_uri", "error_description": msg })),
                    )
                },
            )?;
            uri.clone()
        }
        _ => {
            if app.redirect_uris.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "invalid_redirect_uri", "error_description": "application has no registered redirect_uris" })),
                ));
            }
            app.redirect_uris.first().cloned().unwrap()
        }
    };

    let scope_str = params.scope.unwrap_or_else(|| "openid profile email".to_string());
    let scopes = scope_str
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let mut user_preview = None;
    if let Some(au) = auth_user {
        let user_row = sqlx::query("SELECT id, username, email FROM users WHERE id = $1")
            .bind(au.id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();

        if let Some(u) = user_row {
            let id: Uuid = u.get("id");
            let username: String = u.get("username");
            let email: String = u.get("email");
            user_preview = Some(CurrentUserPreview {
                id: id.to_string(),
                username,
                email,
            });
        }
    }

    Ok(Json(AuthorizeInfoResponse {
        app_id: app.id.to_string(),
        app_name: app.name,
        description: app.description,
        website_url: app.website_url,
        logo_url: app.logo_url,
        redirect_uri,
        scopes,
        user: user_preview,
    }))
}

#[derive(Deserialize)]
struct AuthorizeSubmitRequest {
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
    decision: String, // "approve" or "deny"
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
}

#[derive(Serialize)]
struct AuthorizeSubmitResponse {
    redirect_url: String,
}

async fn authorize_submit<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
    Json(payload): Json<AuthorizeSubmitRequest>,
) -> Result<Json<AuthorizeSubmitResponse>, (StatusCode, Json<serde_json::Value>)> {
    let app = db::oauth::get_application_by_client_id(&state.pool, &payload.client_id)
        .await
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid_client", "error_description": "Application not found" })),
            )
        })?;

    crate::oauth_redirect::assert_redirect_uri_allowed(&app.redirect_uris, &payload.redirect_uri)
        .map_err(|msg| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid_redirect_uri", "error_description": msg })),
            )
        })?;

    let separator = if payload.redirect_uri.contains('?') { "&" } else { "?" };
    let state_param = payload
        .state
        .as_ref()
        .map(|s| format!("&state={}", url_encode(s)))
        .unwrap_or_default();

    if payload.decision != "approve" {
        let redirect_url = format!(
            "{}{}{}error=access_denied&error_description={}{}",
            payload.redirect_uri,
            separator,
            state_param,
            url_encode("User denied authorization"),
            ""
        );
        return Ok(Json(AuthorizeSubmitResponse { redirect_url }));
    }

    let scope = payload.scope.unwrap_or_else(|| "openid profile email".to_string());
    let pkce = crate::oauth_pkce::normalize_challenge(
        payload.code_challenge.as_deref(),
        payload.code_challenge_method.as_deref(),
    )
    .map_err(|msg| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid_request", "error_description": msg })),
        )
    })?;
    let (challenge, method) = pkce;
    let code = db::oauth::create_authorization_code(
        &state.pool,
        app.id,
        auth.id,
        &payload.redirect_uri,
        &scope,
        payload.state.as_deref(),
        Some(&challenge),
        Some(&method),
    )
    .await
    .map_err(|e| {
            crate::http_error::internal_error_parts(&e)
    })?;

    let redirect_url = format!(
        "{}{}code={}{}",
        payload.redirect_uri, separator, code, state_param
    );

    Ok(Json(AuthorizeSubmitResponse { redirect_url }))
}

// ---------------------------------------------------------------------------
// OAuth 2.0 Token Exchange
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
}

#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    scope: String,
}

async fn token_exchange<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    body: Option<Form<TokenRequest>>,
) -> Result<Json<TokenResponse>, (StatusCode, Json<serde_json::Value>)> {
    let mut req = body.map(|b| b.0).unwrap_or_default();

    // Check Basic Auth header: Authorization: Basic base64(client_id:client_secret)
    if let Some(auth_hdr) = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        if let Some(b64) = auth_hdr.strip_prefix("Basic ") {
            if let Ok(decoded_bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim()) {
                if let Ok(decoded) = String::from_utf8(decoded_bytes) {
                    if let Some((cid, csec)) = decoded.split_once(':') {
                        req.client_id = Some(cid.to_string());
                        req.client_secret = Some(csec.to_string());
                    }
                }
            }
        }
    }

    let grant_type = req.grant_type.as_deref().unwrap_or("");
    if grant_type != "authorization_code" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "unsupported_grant_type", "error_description": "Only grant_type=authorization_code is supported" })),
        ));
    }

    let code = req.code.as_deref().unwrap_or("");
    let client_id = req.client_id.as_deref().unwrap_or("");
    let client_secret = req.client_secret.as_deref().unwrap_or("");
    let redirect_uri = req.redirect_uri.as_deref().unwrap_or("");
    let code_verifier = req.code_verifier.as_deref();

    if code.is_empty() || client_id.is_empty() || client_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid_request", "error_description": "Missing required parameters (code, client_id, client_secret)" })),
        ));
    }

    let (token, expires_in, scope) = db::oauth::exchange_authorization_code(
        &state.pool,
        &state.secrets,
        code,
        client_id,
        client_secret,
        redirect_uri,
        code_verifier,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid_grant", "error_description": e.to_string() })),
        )
    })?;

    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "Bearer",
        expires_in,
        scope,
    }))
}

// ---------------------------------------------------------------------------
// OAuth 2.0 UserInfo (Identity Provider Endpoint)
// ---------------------------------------------------------------------------

async fn userinfo<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<db::oauth::OauthUserInfo>, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "missing_token", "error_description": "Authorization header is required" })),
            )
        })?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
        .map(str::trim)
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "invalid_token", "error_description": "Expected Bearer token" })),
            )
        })?;

    let info = db::oauth::get_user_by_oauth_token(&state.pool, token)
        .await
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "invalid_token", "error_description": "Token expired or invalid" })),
            )
        })?;

    Ok(Json(info))
}

// ---------------------------------------------------------------------------
// Developer App Management
// ---------------------------------------------------------------------------

async fn list_apps<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
) -> Result<Json<Vec<db::oauth::OauthApp>>, (StatusCode, Json<serde_json::Value>)> {
    let apps = db::oauth::list_user_applications(&state.pool, auth.id)
        .await
        .map_err(|e| {
            crate::http_error::internal_error_parts(&e)
        })?;
    Ok(Json(apps))
}

#[derive(Deserialize)]
struct CreateAppRequest {
    name: String,
    description: Option<String>,
    website_url: Option<String>,
    logo_url: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
}

async fn create_app<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
    Json(payload): Json<CreateAppRequest>,
) -> Result<Json<db::oauth::OauthAppCreated>, (StatusCode, Json<serde_json::Value>)> {
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "App name is required" })),
        ));
    }

    let redirect_uris = crate::oauth_redirect::validate_redirect_uris(&payload.redirect_uris)
        .map_err(|msg| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
        })?;

    let created = db::oauth::create_application(
        &state.pool,
        &state.secrets,
        auth.id,
        &payload.name,
        payload.description.as_deref(),
        payload.website_url.as_deref(),
        payload.logo_url.as_deref(),
        redirect_uris,
    )
    .await
    .map_err(|e| crate::http_error::internal_error_parts(&e))?;

    Ok(Json(created))
}

#[derive(Deserialize)]
struct UpdateAppRequest {
    name: String,
    description: Option<String>,
    website_url: Option<String>,
    logo_url: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
}

async fn update_app<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAppRequest>,
) -> Result<Json<db::oauth::OauthApp>, (StatusCode, Json<serde_json::Value>)> {
    let redirect_uris = crate::oauth_redirect::validate_redirect_uris(&payload.redirect_uris)
        .map_err(|msg| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
        })?;

    let updated = db::oauth::update_application(
        &state.pool,
        auth.id,
        id,
        &payload.name,
        payload.description.as_deref(),
        payload.website_url.as_deref(),
        payload.logo_url.as_deref(),
        redirect_uris,
    )
    .await
    .map_err(|e| crate::http_error::internal_error_parts(&e))?;

    Ok(Json(updated))
}

async fn delete_app<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    db::oauth::delete_application(&state.pool, auth.id, id)
        .await
        .map_err(|e| {
            crate::http_error::internal_error_parts(&e)
        })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn rotate_secret<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<db::oauth::OauthAppCreated>, (StatusCode, Json<serde_json::Value>)> {
    let rotated = db::oauth::rotate_client_secret(
        &state.pool,
        &state.secrets,
        auth.id,
        id,
    )
    .await
    .map_err(|e| crate::http_error::internal_error_parts(&e))?;

    Ok(Json(rotated))
}

// ---------------------------------------------------------------------------
// User Authorized Apps / Consents
// ---------------------------------------------------------------------------

async fn list_authorized_apps<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
) -> Result<Json<Vec<db::oauth::UserAuthorizedAppItem>>, (StatusCode, Json<serde_json::Value>)> {
    let list = db::oauth::list_user_consents(&state.pool, auth.id)
        .await
        .map_err(|e| {
            crate::http_error::internal_error_parts(&e)
        })?;

    Ok(Json(list))
}

async fn revoke_authorized_app<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    db::oauth::revoke_user_consent(&state.pool, auth.id, app_id)
        .await
        .map_err(|e| {
            crate::http_error::internal_error_parts(&e)
        })?;

    Ok(StatusCode::NO_CONTENT)
}
