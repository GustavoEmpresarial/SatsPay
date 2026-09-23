//! axum routes/controllers/middlewares — thin: parse request, call
//! `domain::<mod>::service` or `db::<mod>` (for transactional orchestration
//! that doesn't fit the repository-trait pattern), serialize response.

pub mod access_log;
pub mod admin;
pub mod airdrop;
pub mod auth;
pub mod client_ip;
pub mod csrf;
pub mod deposits;
pub mod faucet;
pub mod http_error;
pub mod lend;
pub mod merchant;
pub mod merchant_deposits;
pub mod middleware;
pub mod notify_email;
pub mod oauth;
pub mod oauth_pkce;
pub mod oauth_redirect;
pub mod public_api;
pub mod public_catalog;
pub mod rate_limit;
pub mod referral;
pub mod rewards;
pub mod stake;
pub mod state;
pub mod status;
pub mod support;
pub mod swap;
pub mod wallet;
pub mod withdrawals;

pub use state::{AppSettings, AppState};

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use domain::auth::AuthRepo;
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;

/// Matches `TimeoutLayer` below and the previous outer timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

const SKIP_ERROR_RECORD_PATHS: &[&str] = &[
    "/healthz",
    "/metrics",
    "/v1/telemetry/client-error",
    "/v1/telemetry/client-errors",
];

/// Production router (Prometheus + `/metrics`).
pub fn app<R: AuthRepo + 'static>(state: AppState<R>) -> Router {
    let (prometheus_layer, metric_handle) = axum_prometheus::PrometheusMetricLayer::pair();
    let metrics_pool = state.pool.clone();
    finish_router(
        route_tree::<R>()
            .route(
                "/metrics",
                get(move || async move {
                    let mut output = metric_handle.render();
                    if let Ok(extra) =
                        db::telemetry::render_operational_metrics(&metrics_pool).await
                    {
                        output.push_str(&extra);
                    }
                    output
                }),
            )
            .layer(prometheus_layer),
        state,
    )
}

/// Same routes/middleware as [`app`], without Prometheus.
///
/// Use in Axum oneshot tests: `PrometheusMetricLayer::pair()` installs a process-global
/// metrics recorder and panics on the second call in the same binary.
pub fn app_without_metrics<R: AuthRepo + 'static>(state: AppState<R>) -> Router {
    finish_router(route_tree::<R>(), state)
}

fn route_tree<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::<AppState<R>>::new()
        .route("/healthz", get(healthz))
        .merge(auth::routes::<R>())
        .merge(wallet::routes::<R>())
        .merge(deposits::routes::<R>())
        .merge(withdrawals::routes::<R>())
        .merge(swap::routes::<R>())
        .merge(faucet::routes::<R>())
        .merge(stake::routes::<R>())
        .merge(lend::routes::<R>())
        .merge(merchant::routes::<R>())
        .merge(merchant_deposits::routes::<R>())
        .merge(admin::routes::<R>())
        .merge(public_api::routes::<R>())
        .merge(public_catalog::routes::<R>())
        .merge(rewards::routes::<R>())
        .merge(referral::routes::<R>())
        .merge(airdrop::routes::<R>())
        .merge(oauth::routes::<R>())
        .merge(status::routes::<R>())
        .merge(support::routes::<R>())
}

fn finish_router<R: AuthRepo + 'static>(router: Router<AppState<R>>, state: AppState<R>) -> Router {
    router
        // Innermost: normalize axum extractor rejections (bad JSON / wrong type)
        // into the stable `{ error, code }` contract before anything else sees them.
        .layer(axum::middleware::from_fn(normalize_extractor_rejections))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            record_server_errors::<R>,
        ))
        // Rate limiting sits outside the metrics layer on purpose: rejected
        // floods should not inflate the per-route histograms.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit::layer::<R>,
        ))
        // Outermost: no request gets to hold a connection (and a Postgres
        // pool slot) indefinitely.
        .layer(TimeoutLayer::new(REQUEST_TIMEOUT))
        // Outside everything else so 408/429/5xx get an access line and every
        // log below carries the request's correlation id.
        .layer(axum::middleware::from_fn(access_log::layer))
        .with_state(state)
}

async fn record_server_errors<R: AuthRepo + 'static>(
    State(state): State<AppState<R>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    // Path only: query strings carry OAuth `code`/`state`, e-mails, callback keys.
    let full_endpoint = path.clone();
    let request_id = access_log::current_request_id();

    // Same trust model as `client_ip::resolve_client_ip` (do not trust CF-* from clients).
    let ip_address = request
        .headers()
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            request
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| {
                    s.split(',')
                        .map(str::trim)
                        .filter(|h| !h.is_empty())
                        .last()
                        .map(str::to_string)
                })
        });

    let user_agent = request
        .headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let response = next.run(request).await;
    let status = response.status();
    if status.is_server_error() && !SKIP_ERROR_RECORD_PATHS.contains(&path.as_str()) {
        let pool = state.pool.clone();
        let status_code = i32::from(status.as_u16());
        tokio::spawn(async move {
            tracing::error!(
                method = %method,
                endpoint = %full_endpoint,
                status = status_code,
                request_id = ?request_id,
                "HTTP 5xx Server Error intercepted"
            );
            let payload = db::telemetry::NewErrorPayload {
                service: "api-server".to_string(),
                level: "CRITICAL".to_string(),
                message: format!("{method} {full_endpoint} -> {status_code}"),
                stack_trace: None,
                endpoint: Some(full_endpoint),
                method: Some(method),
                status_code: Some(status_code),
                user_id: None,
                ip_address: ip_address.map(|ip| state.secrets.ip_fingerprint(&ip)),
                request_payload: request_id
                    .as_ref()
                    .map(|id| serde_json::json!({"request_id": id})),
                user_agent: user_agent.map(|ua| ua.chars().take(80).collect()),
            };
            let module = path
                .split('/')
                .nth(2)
                .filter(|s| !s.is_empty())
                .unwrap_or("unknown");
            let version = std::env::var("APP_VERSION").unwrap_or_else(|_| "unknown".into());
            if let Err(e) =
                db::telemetry::record_http_failure(&pool, module, &version, status_code).await
            {
                tracing::warn!(error = %e, "failed to record 5xx event");
            }
            let _ = db::telemetry::record_error(&pool, payload).await;
        });
    }
    response
}

async fn healthz() -> &'static str {
    "ok"
}

/// axum's built-in `Json`/`Path`/`Query` extractors reject malformed input with
/// a `text/plain` body that describes the parser internals (e.g. "Failed to
/// deserialize the JSON body into the target type: email: invalid type:
/// integer `123`..."). That leaks field/parser detail and, more importantly,
/// breaks the `{ error, code }` contract every other 4xx honors — the client's
/// `adaptRustError` cannot read it. This rewrites only those framework
/// rejections (400/422 with a `text/plain` body from the extractor) into the
/// stable JSON shape. App handlers already emit JSON, so they pass through
/// untouched.
async fn normalize_extractor_rejections(request: Request, next: Next) -> Response {
    use axum::http::{header, StatusCode};
    let response = next.run(request).await;
    let status = response.status();
    if status != StatusCode::BAD_REQUEST
        && status != StatusCode::UNPROCESSABLE_ENTITY
        && status != StatusCode::UNSUPPORTED_MEDIA_TYPE
        && status != StatusCode::LENGTH_REQUIRED
    {
        return response;
    }
    // Only touch plain-text bodies: our own errors are already `application/json`.
    let is_text = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/plain"))
        .unwrap_or(false);
    if !is_text {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    // Extractor rejections are tiny; cap defensively.
    let bytes = match axum::body::to_bytes(body, 16 * 1024).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let raw = String::from_utf8_lossy(&bytes);
    // Order matters: axum's query/form messages also start with "Failed to
    // deserialize", so the specific non-JSON rejections are matched first
    // (texts from axum 0.7 `extract::rejection` / `path`).
    let code = if raw.starts_with("Failed to deserialize query string")
        || raw.starts_with("Failed to deserialize form")
        || raw.starts_with("Form requests must have")
        || raw.starts_with("Invalid URL")
        || raw.starts_with("Cannot parse")
        || raw.starts_with("Wrong number of path arguments")
    {
        "VALIDATION_ERROR"
    } else if raw.starts_with("Failed to parse the request body as JSON")
        || raw.starts_with("Failed to deserialize the JSON body")
        || raw.starts_with("Expected request with `Content-Type: application/json`")
    {
        "INVALID_JSON"
    } else {
        // Some other framework rejection (e.g. body-too-large) — normalize the
        // shape without guessing a specific code.
        "BAD_REQUEST"
    };
    // Do not echo the parser's message (it names fields/types/offsets).
    let payload = serde_json::json!({
        "error": "invalid request body or parameters",
        "code": code,
    });
    parts.headers.remove(header::CONTENT_TYPE);
    parts.headers.remove(header::CONTENT_LENGTH);
    (parts.status, axum::Json(payload)).into_response()
}
