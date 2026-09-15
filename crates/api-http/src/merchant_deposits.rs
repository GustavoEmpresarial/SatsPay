//! SatsPay deposit gateway: merchant invoices + the public `/pay/:id`
//! checkout. The contract these handlers expose is the one documented on
//! `/docs` (`client/src/pages/ApiDocsPage.tsx`) — change one and change the
//! other, with `client/tests/unit/contract/` guarding the pair.

use crate::middleware::AuthUser;
use crate::notify_email::send_best_effort;
use crate::public_api::{authenticate_api_request, split_request};
use crate::state::AppState;
use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{async_trait, Json, Router};
use bigdecimal::BigDecimal;
use db::merchant_deposits::{
    create_invoice, get_invoice_by_id, get_invoice_by_order_id, list_invoices_by_merchant,
    pay_invoice_with_balance, CreateDepositInvoiceInput, MerchantDepositError, MerchantDepositInvoice,
};
use domain::auth::AuthRepo;
use serde::Deserialize;
use serde_json::json;
use std::str::FromStr;
use uuid::Uuid;

/// API-key scope required to use the deposit gateway. Matches the scope the
/// dashboard offers when issuing a key (`client/src/pages/ApiKeysPage.tsx`).
const MERCHANT_SCOPE: &str = "deposits";

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        // Merchant API endpoints (Site X integration).
        // `POST /v1/merchant/deposits` is the documented spelling; `/create`
        // and `/invoices` are older aliases kept working for integrations
        // that already ship them.
        .route(
            "/v1/merchant/deposits",
            get(list_deposits_handler::<R>).post(create_deposit_handler::<R>),
        )
        .route("/v1/merchant/deposits/create", post(create_deposit_handler::<R>))
        .route("/v1/merchant/deposits/:id", get(get_deposit_handler::<R>))
        .route("/v1/merchant/deposits/:id/test-webhook", post(test_webhook_handler::<R>))
        .route(
            "/v1/merchant/invoices",
            get(list_deposits_handler::<R>).post(create_deposit_handler::<R>),
        )
        .route("/v1/merchant/invoices/:id", get(get_deposit_handler::<R>))
        .route("/v1/merchant/webhook-signing-secret", get(webhook_signing_secret_handler::<R>))
        // Public checkout routes (/pay/:id)
        .route("/v1/public/pay/:id", get(get_public_invoice_handler::<R>))
        .route("/v1/public/pay/:id/balance", post(pay_with_balance_handler::<R>))
        .route("/public/pay/:id", get(get_public_invoice_handler::<R>))
        .route("/public/pay/:id/balance", post(pay_with_balance_handler::<R>))
}

#[derive(Deserialize)]
pub struct CreateDepositReq {
    pub coin: String,
    pub amount: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "siteUserId")]
    pub site_user_id: Option<String>,
    #[serde(rename = "siteName")]
    pub site_name: Option<String>,
    #[serde(rename = "callbackUrl")]
    pub callback_url: String,
    #[serde(rename = "successUrl")]
    pub success_url: Option<String>,
    #[serde(rename = "cancelUrl")]
    pub cancel_url: Option<String>,
    #[serde(rename = "customerEmail")]
    pub customer_email: Option<String>,
    #[serde(rename = "customerName")]
    pub customer_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "expiryMinutes")]
    pub expiry_minutes: Option<i64>,
}

#[derive(Deserialize)]
pub struct ListDepositsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Error body shape every gateway failure uses: a human `error` plus a stable
/// `code` the client (`lib/api.ts::adaptRustError`) and the published docs
/// can both rely on.
fn fail(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({ "error": message, "code": code }))).into_response()
}

/// A rejection before a `Response` exists — keeps validation helpers cheap to
/// return from and testable without building HTTP responses.
#[derive(Debug)]
struct Rejected {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl Rejected {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Rejected { status, code, message: message.into() }
    }

    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Rejected::new(StatusCode::BAD_REQUEST, code, message)
    }
}

impl From<Rejected> for Response {
    fn from(r: Rejected) -> Response {
        fail(r.status, r.code, &r.message)
    }
}

/// An authenticated merchant plus the buffered request body.
///
/// Both auth modes land here: the dashboard's JWT session and an API key
/// (plain `x-api-key` or an HMAC-signed request). API-key auth is delegated
/// to [`authenticate_api_request`], the same path `/v1/public/*` uses — the
/// gateway previously had its own copy that authenticated against a
/// hardcoded `0.0.0.0` source IP, which made every IP-allowlisted key fail
/// here and made the documented IP allowlist meaningless for this surface.
struct MerchantAuth {
    merchant_id: Uuid,
    api_key_id: Option<Uuid>,
    body: axum::body::Bytes,
}

#[async_trait]
impl<R: AuthRepo + 'static> FromRequest<AppState<R>> for MerchantAuth {
    type Rejection = Response;

    async fn from_request(req: Request, state: &AppState<R>) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = split_request(req).await.map_err(IntoResponse::into_response)?;

        if let Ok(user) = AuthUser::from_request_parts(&mut parts, state).await {
            return Ok(MerchantAuth { merchant_id: user.id, api_key_id: None, body });
        }

        let record = authenticate_api_request(&parts, &body, state)
            .await
            .map_err(IntoResponse::into_response)?;
        if db::public_api::require_scope(&record, MERCHANT_SCOPE).is_err() {
            return Err(fail(
                StatusCode::FORBIDDEN,
                "MISSING_SCOPE",
                &format!("missing required scope: {MERCHANT_SCOPE}"),
            ));
        }
        Ok(MerchantAuth { merchant_id: record.user_id, api_key_id: Some(record.id), body })
    }
}

/// `amount` is an integer count of ledger units (1e-8), exactly like
/// `/v1/public/send` — never a decimal quantity of coins.
///
/// A merchant writing `"25.00"` for USDT means 25 dollars but would be
/// charged 25 × 1e-8 USDT, so a fractional-looking amount is rejected
/// outright instead of being silently reinterpreted. Note the value is
/// numerically an integer, so a numeric `is_integer()` check would not catch
/// it — the decimal point in the *input* is the signal.
fn parse_amount(raw: &str, coin: shared::Coin) -> Result<BigDecimal, Rejected> {
    let invalid = || Rejected::bad_request("INVALID_AMOUNT", "invalid amount");
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(invalid());
    }
    // Only call it a unit mistake when the input actually looks numeric —
    // "not-a-number" contains an 'e' but is plain garbage.
    let numeric_shape = raw.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | 'e' | 'E' | '+' | '-'));
    if numeric_shape && raw.contains(['.', ',', 'e', 'E']) {
        return Err(Rejected::bad_request(
            "AMOUNT_NOT_INTEGER",
            "amount must be an integer number of ledger units (1e-8). 25 USDT = \"2500000000\", not \"25.00\"",
        ));
    }
    if !raw.chars().all(|c| c.is_ascii_digit()) || raw.len() > 30 {
        return Err(invalid());
    }
    let units: u128 = raw.parse().map_err(|_| invalid())?;
    if units == 0 {
        return Err(Rejected::bad_request("INVALID_AMOUNT", "amount must be greater than 0"));
    }
    // An amount that rounds to zero on-chain can never be paid on-chain —
    // e.g. USDT has 6 decimals, so anything below 100 ledger units is dust.
    if shared::to_onchain_amount(coin, units) == 0 {
        return Err(Rejected::bad_request(
            "AMOUNT_BELOW_MINIMUM",
            format!("amount is below the smallest {} unit payable on-chain", coin.as_str()),
        ));
    }
    BigDecimal::from_str(raw).map_err(|_| invalid())
}

/// The create/get response body. One builder so the idempotent replay and the
/// fresh insert can never answer with different shapes.
fn invoice_json(inv: &MerchantDepositInvoice, base_url: &str) -> serde_json::Value {
    let qr_code = format!("{}:{}?amount={}", inv.coin.to_lowercase(), inv.deposit_address, inv.amount);
    let pay_url = format!("/pay/{}", inv.id);
    json!({
        "id": inv.id,
        "status": inv.status,
        "coin": inv.coin,
        "amount": inv.amount.to_string(),
        "feeAmount": inv.fee_amount.to_string(),
        "netAmount": inv.net_amount.to_string(),
        "depositAddress": inv.deposit_address,
        "payUrl": pay_url,
        "checkoutUrl": format!("{base_url}{pay_url}"),
        "qrCode": qr_code,
        "orderId": inv.order_id,
        "expiresAt": inv.expires_at,
        "createdAt": inv.created_at,
    })
}

/// True when a re-POST for the same `orderId` describes the same charge and
/// the original invoice is still payable — the definition of an idempotent
/// retry for this endpoint.
fn is_idempotent_replay(existing: &MerchantDepositInvoice, coin: shared::Coin, amount: &BigDecimal) -> bool {
    existing.coin == coin.as_str()
        && &existing.amount == amount
        && matches!(existing.status.as_str(), "PENDING" | "DETECTED")
        && existing.expires_at > chrono::Utc::now()
}

async fn create_deposit_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    auth: MerchantAuth,
) -> Response {
    let Ok(body) = serde_json::from_slice::<CreateDepositReq>(&auth.body) else {
        return fail(StatusCode::BAD_REQUEST, "INVALID_BODY", "invalid request body");
    };

    let Ok(coin) = body.coin.parse::<shared::Coin>() else {
        return fail(StatusCode::BAD_REQUEST, "UNKNOWN_COIN", "unknown coin");
    };
    // Same pause list as personal deposits — merchant invoice addresses for
    // BTC/LTC/DOGE/DGB stay blocked until DEPOSIT_WITHDRAW_PAUSED_COINS is cleared.
    // `/v1/public/send` (ledger payout to users) is intentionally NOT paused.
    if shared::is_deposit_withdraw_paused(coin) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": format!("{} deposits are temporarily paused", coin.as_str()),
                "code": "DEPOSIT_PAUSED",
                "coin": coin.as_str(),
            })),
        )
            .into_response();
    }

    let amount = match parse_amount(&body.amount, coin) {
        Ok(a) => a,
        Err(rejected) => return rejected.into(),
    };

    if body.order_id.trim().is_empty() || body.order_id.len() > 128 {
        return fail(StatusCode::BAD_REQUEST, "INVALID_ORDER_ID", "orderId must be 1-128 characters");
    }

    // The webhook is an outbound request this server makes on the merchant's
    // instruction — reject unreachable/internal targets at creation time
    // rather than discovering it at delivery.
    if let Err(e) = webhooks::validate_callback_url(&body.callback_url) {
        return fail(StatusCode::BAD_REQUEST, "INVALID_CALLBACK_URL", &e.to_string());
    }

    // Idempotency: a retried POST for the same orderId returns the original
    // invoice instead of minting a second payable address for one order.
    match get_invoice_by_order_id(&state.pool, auth.merchant_id, &body.order_id).await {
        Ok(existing) => {
            return if is_idempotent_replay(&existing, coin, &amount) {
                (StatusCode::OK, Json(invoice_json(&existing, &state.settings.public_base_url))).into_response()
            } else {
                fail(
                    StatusCode::CONFLICT,
                    "DUPLICATE_ORDER_ID",
                    "orderId already used for a different invoice",
                )
            };
        }
        Err(MerchantDepositError::NotFound) => {}
        Err(e) => return fail(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", &e.to_string()),
    }

    // Every invoice needs its OWN address: the watcher attributes an on-chain
    // payment to an invoice by the address it landed on. The old fallback here
    // reused the merchant's shared personal deposit address, which would let a
    // single payment confirm two different invoices — so a failure to derive a
    // fresh address now fails the request instead.
    let client = state.chain_registry.get(coin);
    let (deposit_address, hd_index) = match client.generate_address(&auth.merchant_id.to_string()).await {
        Ok(addr) => (addr.address, addr.hd_index.map(|i| i as i64)),
        Err(e) => {
            tracing::error!(merchant_id = %auth.merchant_id, coin = %coin.as_str(), error = %e.message, "gateway: could not derive an invoice address");
            return fail(
                StatusCode::SERVICE_UNAVAILABLE,
                "ADDRESS_UNAVAILABLE",
                "could not generate a deposit address for this coin right now",
            );
        }
    };

    let input = CreateDepositInvoiceInput {
        merchant_id: auth.merchant_id,
        api_key_id: auth.api_key_id,
        site_user_id: body.site_user_id,
        order_id: body.order_id.clone(),
        site_name: body.site_name,
        coin,
        amount: amount.clone(),
        deposit_address,
        hd_index,
        callback_url: body.callback_url,
        success_url: body.success_url,
        cancel_url: body.cancel_url,
        customer_email: body.customer_email,
        customer_name: body.customer_name,
        description: body.description,
        expiry_minutes: body.expiry_minutes,
    };

    match create_invoice(&state.pool, input).await {
        Ok(inv) => (StatusCode::CREATED, Json(invoice_json(&inv, &state.settings.public_base_url))).into_response(),
        // Lost the race against a concurrent POST with the same orderId:
        // resolve it exactly like the pre-insert check above.
        Err(MerchantDepositError::DuplicateOrderId) => {
            match get_invoice_by_order_id(&state.pool, auth.merchant_id, &body.order_id).await {
                Ok(existing) if is_idempotent_replay(&existing, coin, &amount) => {
                    (StatusCode::OK, Json(invoice_json(&existing, &state.settings.public_base_url))).into_response()
                }
                _ => fail(
                    StatusCode::CONFLICT,
                    "DUPLICATE_ORDER_ID",
                    "orderId already used for a different invoice",
                ),
            }
        }
        Err(e) => fail(StatusCode::BAD_REQUEST, "INVOICE_CREATE_FAILED", &e.to_string()),
    }
}

async fn list_deposits_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    Query(query): Query<ListDepositsQuery>,
    auth: MerchantAuth,
) -> Response {
    match list_invoices_by_merchant(&state.pool, auth.merchant_id, query.limit.unwrap_or(50), query.offset.unwrap_or(0)).await {
        Ok(list) => Json(json!({ "invoices": list })).into_response(),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", &e.to_string()),
    }
}

async fn get_deposit_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    Path(id): Path<Uuid>,
    auth: MerchantAuth,
) -> Response {
    match get_invoice_by_id(&state.pool, id).await {
        Ok(inv) => {
            if inv.merchant_id != auth.merchant_id {
                return fail(StatusCode::FORBIDDEN, "INVOICE_FORBIDDEN", "forbidden");
            }
            Json(inv).into_response()
        }
        Err(_) => fail(StatusCode::NOT_FOUND, "INVOICE_NOT_FOUND", "invoice not found"),
    }
}

/// Public endpoint for `/pay/:id` checkout page. Deliberately narrower than
/// the merchant view: no `callbackUrl`, `siteUserId` or webhook state.
async fn get_public_invoice_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    Path(id): Path<Uuid>,
) -> Response {
    match get_invoice_by_id(&state.pool, id).await {
        Ok(inv) => {
            let qr_code = format!("{}:{}?amount={}", inv.coin.to_lowercase(), inv.deposit_address, inv.amount);
            Json(json!({
                "id": inv.id,
                "status": inv.status,
                "coin": inv.coin,
                "amount": inv.amount.to_string(),
                "depositAddress": inv.deposit_address,
                "orderId": inv.order_id,
                "siteName": inv.site_name,
                "description": inv.description,
                "customerEmail": inv.customer_email,
                "successUrl": inv.success_url,
                "cancelUrl": inv.cancel_url,
                "qrCode": qr_code,
                "expiresAt": inv.expires_at,
                "paidAt": inv.paid_at,
                "txHash": inv.tx_hash,
            }))
            .into_response()
        }
        Err(_) => fail(StatusCode::NOT_FOUND, "INVOICE_NOT_FOUND", "invoice not found"),
    }
}

/// Pay with internal SatsPay balance (1-click)
async fn pay_with_balance_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Response {
    match pay_invoice_with_balance(&state.pool, id, user.id).await {
        Ok(inv) => {
            // Dispatch webhook in background
            let pool_clone = state.pool.clone();
            let inv_clone = inv.clone();
            let secrets = state.secrets.clone();
            tokio::spawn(async move {
                webhooks::dispatch_invoice_webhook(&pool_clone, &inv_clone, &secrets).await;
            });

            // Send notification email to customer if provided
            if let Some(ref email) = inv.customer_email {
                let subject = format!("Pagamento Confirmado - Pedido {}", inv.order_id);
                let body = format!(
                    "Seu pagamento de {} {} para {} foi confirmado com sucesso!\n\nPedido: {}\nStatus: CONFIRMADO\nObrigado por usar SatsPay.",
                    inv.amount, inv.coin, inv.site_name.as_deref().unwrap_or("o comerciante"), inv.order_id
                );
                send_best_effort(state.email.as_ref(), email, &subject, &body).await;
            }

            Json(json!({
                "success": true,
                "message": "Pagamento realizado com sucesso com seu saldo SatsPay!",
                "invoice": inv,
            }))
            .into_response()
        }
        Err(MerchantDepositError::InsufficientBalance) => {
            fail(StatusCode::BAD_REQUEST, "INSUFFICIENT_BALANCE", "insufficient balance")
        }
        Err(MerchantDepositError::InvalidStatus) => {
            fail(StatusCode::BAD_REQUEST, "INVOICE_INVALID_STATE", "invoice already paid or expired")
        }
        Err(e) => fail(StatusCode::BAD_REQUEST, "INVOICE_PAYMENT_FAILED", &e.to_string()),
    }
}

/// Test webhook dispatch manually from merchant dashboard
async fn test_webhook_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Response {
    let inv = match get_invoice_by_id(&state.pool, id).await {
        Ok(i) => i,
        Err(_) => return fail(StatusCode::NOT_FOUND, "INVOICE_NOT_FOUND", "invoice not found"),
    };
    if inv.merchant_id != user.id {
        return fail(StatusCode::FORBIDDEN, "INVOICE_FORBIDDEN", "forbidden");
    }

    let outcome = webhooks::dispatch_invoice_webhook(&state.pool, &inv, &state.secrets).await;
    Json(json!({
        "delivered": outcome.delivered,
        "statusCode": outcome.status_code,
        "error": outcome.error,
    }))
    .into_response()
}

/// Hex HMAC key derived for this merchant (`ENCRYPTION_KEY` + HKDF `bitcosats:webhook:v1`).
async fn webhook_signing_secret_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    let secret = state.secrets.webhook_signing_secret(&user.id.to_string());
    Json(json!({
        "secret": secret,
        "algorithm": "HMAC-SHA256",
        "header": "X-SatsPay-Signature",
        "format": "sha256=<hex>",
        "event": webhooks::EVENT_DEPOSIT_CONFIRMED,
        "maxAgeSeconds": webhooks::MAX_WEBHOOK_AGE.as_secs(),
        "maxAttempts": webhooks::MAX_WEBHOOK_ATTEMPTS,
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_rejects_decimal_input_even_when_numerically_integral() {
        // "25.00" is numerically 25, so a value-based integer check passes it.
        // The merchant meant 25 USDT; the API would charge 0.00000025 USDT.
        let err = parse_amount("25.00", shared::Coin::Usdt).expect_err("must reject");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert_eq!(err.code, "AMOUNT_NOT_INTEGER");
        assert!(parse_amount("25,00", shared::Coin::Usdt).is_err());
        assert!(parse_amount("2.5e9", shared::Coin::Usdt).is_err());
        assert!(parse_amount("25e8", shared::Coin::Usdt).is_err());
    }

    #[test]
    fn amount_accepts_ledger_units() {
        let ok = parse_amount("2500000000", shared::Coin::Usdt).expect("25 USDT in ledger units");
        assert_eq!(ok, BigDecimal::from(2_500_000_000u64));
    }

    #[test]
    fn amount_rejects_zero_negative_and_garbage() {
        assert_eq!(
            parse_amount("not-a-number", shared::Coin::Pol).expect_err("garbage").code,
            "INVALID_AMOUNT",
            "garbage is not a unit mistake, even though it contains an 'e'"
        );
        assert!(parse_amount("0", shared::Coin::Pol).is_err());
        assert!(parse_amount("-1", shared::Coin::Pol).is_err());
        assert!(parse_amount("not-a-number", shared::Coin::Pol).is_err());
        assert!(parse_amount("", shared::Coin::Pol).is_err());
    }

    #[test]
    fn amount_rejects_dust_that_is_zero_on_chain() {
        // USDT is 6 decimals on-chain: below 100 ledger units it rounds to 0
        // and the invoice could never be paid.
        assert!(parse_amount("99", shared::Coin::Usdt).is_err());
        assert!(parse_amount("100", shared::Coin::Usdt).is_ok());
        // BTC is 8 decimals on-chain — 1 ledger unit is payable.
        assert!(parse_amount("1", shared::Coin::Btc).is_ok());
    }

    #[test]
    fn idempotent_replay_requires_same_charge_and_open_invoice() {
        let inv = sample_invoice("PENDING", chrono::Utc::now() + chrono::Duration::minutes(10));
        assert!(is_idempotent_replay(&inv, shared::Coin::Pol, &BigDecimal::from(500_000)));
        // Different amount for the same orderId is a conflict, not a replay.
        assert!(!is_idempotent_replay(&inv, shared::Coin::Pol, &BigDecimal::from(500_001)));
        assert!(!is_idempotent_replay(&inv, shared::Coin::Usdt, &BigDecimal::from(500_000)));

        let paid = sample_invoice("CONFIRMED", chrono::Utc::now() + chrono::Duration::minutes(10));
        assert!(!is_idempotent_replay(&paid, shared::Coin::Pol, &BigDecimal::from(500_000)));

        let expired = sample_invoice("PENDING", chrono::Utc::now() - chrono::Duration::minutes(1));
        assert!(!is_idempotent_replay(&expired, shared::Coin::Pol, &BigDecimal::from(500_000)));
    }

    fn sample_invoice(status: &str, expires_at: chrono::DateTime<chrono::Utc>) -> MerchantDepositInvoice {
        MerchantDepositInvoice {
            id: Uuid::new_v4(),
            merchant_id: Uuid::new_v4(),
            api_key_id: None,
            site_user_id: None,
            order_id: "ORD-1".into(),
            site_name: None,
            coin: "POL".into(),
            amount: BigDecimal::from(500_000),
            fee_amount: BigDecimal::from(2_500),
            net_amount: BigDecimal::from(497_500),
            deposit_address: "0xabc".into(),
            hd_index: Some(1),
            tx_hash: None,
            confirmations: 0,
            received_amount: BigDecimal::from(0),
            callback_url: "https://merchant.example/hook".into(),
            success_url: None,
            cancel_url: None,
            customer_email: None,
            customer_name: None,
            description: None,
            status: status.into(),
            expires_at,
            paid_at: None,
            webhook_delivered: false,
            webhook_status_code: None,
            webhook_attempts: 0,
            webhook_last_error: None,
            webhook_last_attempt_at: None,
            webhook_next_retry_at: None,
            created_at: chrono::Utc::now(),
        }
    }
}
