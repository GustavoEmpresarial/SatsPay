//! Thin axum routes for public-api — port of legacy
//! `public-api/{controllers,utils}/*.ts`. Key management is JWT-gated (the
//! dashboard); `/v1/public/*` accepts either the simple `x-api-key` header
//! or an HMAC-signed request (`x-key-id`/`x-timestamp`/`x-signature`) — see
//! `db::public_api::verify_signed_request` for the signing scheme.

use crate::client_ip::resolve_client_ip;
use crate::middleware::AuthUser;
use crate::notify_email::{send_best_effort, user_email};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{FromRequest, Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{async_trait, Json, Router};
use bigdecimal::BigDecimal;
use db::public_api::{ApiKeyRecord, PublicApiError, VerifySignedRequestInput};
use domain::auth::AuthRepo;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;


pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        // API key management (support /v1/api-keys, /v1/public/keys, /public/keys, /api-keys)
        .route("/v1/api-keys", get(list_keys::<R>).post(issue_key::<R>))
        .route("/v1/api-keys/:id/rotate", post(rotate_key::<R>))
        .route("/v1/api-keys/:id", delete(disable_key::<R>))
        .route("/v1/public/keys", get(list_keys::<R>).post(issue_key::<R>))
        .route("/v1/public/keys/:id/rotate", post(rotate_key::<R>))
        .route("/v1/public/keys/:id", delete(disable_key::<R>))
        .route("/public/keys", get(list_keys::<R>).post(issue_key::<R>))
        .route("/public/keys/:id/rotate", post(rotate_key::<R>))
        .route("/public/keys/:id", delete(disable_key::<R>))
        .route("/api-keys", get(list_keys::<R>).post(issue_key::<R>))
        .route("/api-keys/:id/rotate", post(rotate_key::<R>))
        .route("/api-keys/:id", delete(disable_key::<R>))
        // Merchant Public API endpoints
        .route("/v1/public/send", post(send::<R>))
        .route("/v1/public/balance", get(balance::<R>))
        .route("/public/send", post(send::<R>))
        .route("/public/balance", get(balance::<R>))
}

#[derive(Deserialize)]
struct IssueKeyRequest {
    label: String,
    scopes: Vec<String>,
    #[serde(rename = "allowedIps", default)]
    allowed_ips: Vec<String>,
    #[serde(rename = "expiresInDays")]
    expires_in_days: Option<i64>,
    #[serde(rename = "requireSignature", default)]
    require_signature: bool,
}

async fn issue_key<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Json(body): Json<IssueKeyRequest>) -> Response {
    let scopes: Vec<&str> = body.scopes.iter().map(String::as_str).collect();
    let allowed_ips: Vec<&str> = body.allowed_ips.iter().map(String::as_str).collect();
    match db::public_api::issue_api_key(&state.pool, &state.secrets, user.id, &body.label, &scopes, &allowed_ips, body.expires_in_days, body.require_signature).await {
        Ok(key) => (StatusCode::CREATED, Json(key)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn rotate_key<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    match db::public_api::rotate_api_key(&state.pool, &state.secrets, user.id, id).await {
        Ok(key) => Json(key).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn disable_key<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser, Path(id): Path<Uuid>) -> Response {
    match db::public_api::disable_api_key(&state.pool, user.id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_keys<R: AuthRepo>(State(state): State<AppState<R>>, user: AuthUser) -> Response {
    match db::public_api::list_api_keys_by_user(&state.pool, user.id).await {
        Ok(keys) => Json(json!({ "keys": keys })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

/// Extracts and authenticates a `/v1/public/*` request via either auth mode:
/// the simple `x-api-key` header, or an HMAC-signed request (`x-key-id` +
/// `x-timestamp` + `x-signature`, HMAC-SHA256 over method+path+body-hash).
/// A `require_signature=true` key rejects the `x-api-key` path — matching
/// legacy's `assertKeyUsable` gate. Consumes the body (needed for the
/// signature's body hash) and hands it back as `Bytes` for handlers to parse.
struct ApiKeyAuth(ApiKeyRecord, Bytes);

/// Rejection carrying a stable machine-readable `code` alongside the human
/// message — the shape `client/src/lib/api.ts::adaptRustError` reads.
pub struct ApiKeyRejection(StatusCode, &'static str, String);

impl IntoResponse for ApiKeyRejection {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.2, "code": self.1 }))).into_response()
    }
}

fn unauthorized(code: &'static str, msg: &str) -> ApiKeyRejection {
    ApiKeyRejection(StatusCode::UNAUTHORIZED, code, msg.to_string())
}

/// Stable error code for every `PublicApiError` variant. Documented on
/// `/docs`; never invent a message-only error for these.
pub(crate) fn public_api_error_code(e: &PublicApiError) -> &'static str {
    match e {
        PublicApiError::Db(_) => "INTERNAL_ERROR",
        PublicApiError::KeyNotFound | PublicApiError::InvalidKey => "INVALID_API_KEY",
        PublicApiError::KeyExpired => "API_KEY_EXPIRED",
        PublicApiError::IpNotAllowed => "IP_NOT_ALLOWED",
        PublicApiError::RequiresSignature => "KEY_REQUIRES_SIGNATURE",
        PublicApiError::MissingScope(_) => "MISSING_SCOPE",
        PublicApiError::IneligibleTarget => "TARGET_INELIGIBLE",
        PublicApiError::SendToSelf => "SEND_TO_SELF",
        PublicApiError::WalletNotFound => "WALLET_NOT_FOUND",
        PublicApiError::DailyLimitReached => "DAILY_LIMIT_REACHED",
        PublicApiError::BadTimestamp => "BAD_TIMESTAMP",
        PublicApiError::TimestampOutOfWindow => "TIMESTAMP_OUT_OF_WINDOW",
        PublicApiError::SignatureMismatch => "SIGNATURE_MISMATCH",
        PublicApiError::SignatureReplay => "SIGNATURE_REPLAY",
    }
}

pub(crate) fn key_rejection(e: &PublicApiError) -> ApiKeyRejection {
    let code = public_api_error_code(e);
    let status = if code == "INTERNAL_ERROR" { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNAUTHORIZED };
    ApiKeyRejection(status, code, e.to_string())
}

/// Largest request body the API-key paths buffer. The signature covers a
/// hash of the raw body, so it has to be read fully before auth can run —
/// cap it so an unauthenticated caller cannot make the server buffer
/// unbounded bytes.
const MAX_SIGNED_BODY_BYTES: usize = 1024 * 1024;

/// Splits a request into parts + fully-buffered body, which both API-key auth
/// modes need (the HMAC signature covers a hash of the raw body).
pub(crate) async fn split_request(req: Request) -> Result<(axum::http::request::Parts, Bytes), ApiKeyRejection> {
    let (parts, body_stream) = req.into_parts();
    let body = axum::body::to_bytes(body_stream, MAX_SIGNED_BODY_BYTES)
        .await
        .map_err(|_| ApiKeyRejection(StatusCode::PAYLOAD_TOO_LARGE, "BODY_TOO_LARGE", "request body too large or unreadable".to_string()))?;
    Ok((parts, body))
}

/// Authenticates an API-key request from already-split `parts` + raw `body`,
/// accepting either auth mode. Shared by the `/v1/public/*` extractor and the
/// merchant gateway (`merchant_deposits`), so both apply the same IP,
/// expiry, `require_signature` and replay gates — a merchant-only copy of
/// this logic is how the gateway ended up authenticating against a hardcoded
/// `0.0.0.0` source IP.
pub(crate) async fn authenticate_api_request<R: AuthRepo + 'static>(
    parts: &axum::http::request::Parts,
    body: &Bytes,
    state: &AppState<R>,
) -> Result<ApiKeyRecord, ApiKeyRejection> {
    let source_ip = resolve_client_ip(parts)
        .ok_or_else(|| ApiKeyRejection(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "client IP unavailable".into()))?;
    let method = parts.method.to_string();
    let path_and_query = parts.uri.path_and_query().map(|pq| pq.as_str().to_string()).unwrap_or_default();
    let key_id = parts.headers.get("x-key-id").and_then(|v| v.to_str().ok());
    let timestamp = parts.headers.get("x-timestamp").and_then(|v| v.to_str().ok());
    let signature = parts.headers.get("x-signature").and_then(|v| v.to_str().ok());
    let api_key_header = parts.headers.get("x-api-key").and_then(|v| v.to_str().ok());

    if let (Some(key_id), Some(timestamp), Some(signature)) = (key_id, timestamp, signature) {
        let key_id = key_id.parse::<Uuid>().map_err(|_| unauthorized("INVALID_KEY_ID", "invalid x-key-id"))?;
        return db::public_api::verify_signed_request(
            &state.pool,
            &state.secrets,
            VerifySignedRequestInput { key_id, timestamp, signature, method: &method, path: &path_and_query, raw_body: body, source_ip: &source_ip },
            state.settings.public_api_signature_max_skew,
        )
        .await
        .map_err(|e| key_rejection(&e));
    }

    let key = api_key_header.ok_or_else(|| unauthorized("INVALID_API_KEY", "missing or invalid API key"))?;
    if key.len() != 64 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(unauthorized("INVALID_API_KEY", "missing or invalid API key"));
    }
    db::public_api::authenticate_by_hash(&state.pool, &state.secrets, key, &source_ip)
        .await
        .map_err(|e| key_rejection(&e))
}

#[async_trait]
impl<R: AuthRepo + 'static> FromRequest<AppState<R>> for ApiKeyAuth {
    type Rejection = ApiKeyRejection;

    async fn from_request(req: Request, state: &AppState<R>) -> Result<Self, Self::Rejection> {
        let (parts, body) = split_request(req).await?;
        let record = authenticate_api_request(&parts, &body, state).await?;
        Ok(ApiKeyAuth(record, body))
    }
}

#[derive(Deserialize)]
struct SendRequest {
    coin: String,
    #[serde(rename = "toEmail")]
    to_email: String,
    amount: String,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
}

async fn send<R: AuthRepo>(State(state): State<AppState<R>>, ApiKeyAuth(key, raw_body): ApiKeyAuth) -> Response {
    if db::public_api::require_scope(&key, "send").is_err() {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "missing required scope: send", "code": "MISSING_SCOPE" }))).into_response();
    }
    let Ok(body) = serde_json::from_slice::<SendRequest>(&raw_body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    let (Ok(coin), Ok(amount)) = (body.coin.parse::<shared::Coin>(), body.amount.parse::<BigDecimal>()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid request" }))).into_response();
    };
    match db::public_api::send_to_user(&state.pool, key.user_id, key.id, coin, &body.to_email, amount.clone(), &body.idempotency_key, state.settings.public_api_daily_send_limit).await {
        Ok(reference) => {
            if let Some(to) = user_email(&state.pool, key.user_id).await {
                let body_text = format!(
                    "Your API key sent {amount} {} to {}.\n\nReference: {reference}",
                    coin.as_str(),
                    body.to_email
                );
                send_best_effort(state.email.as_ref(), &to, "BitcoSats public API send", &body_text).await;
            }
            Json(json!({ "referenceId": reference })).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn balance<R: AuthRepo>(State(state): State<AppState<R>>, ApiKeyAuth(key, _raw_body): ApiKeyAuth) -> Response {
    match db::public_api::get_balance_for_api_key(&state.pool, key.user_id).await {
        Ok(balances) => {
            let map: serde_json::Map<String, serde_json::Value> = balances.into_iter().map(|(c, b)| (c.as_str().to_string(), json!(b.to_string()))).collect();
            Json(map).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
