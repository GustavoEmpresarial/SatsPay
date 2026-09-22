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
use bigdecimal::{BigDecimal, ToPrimitive};
use db::merchant_deposits::{
    create_invoice, get_invoice_by_id, get_invoice_by_order_id, list_invoices_by_merchant,
    pay_invoice_with_balance, reveal_invoice_pii, seal_invoice_pii, CreateDepositInvoiceInput,
    MerchantDepositError, MerchantDepositInvoice,
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
        .route(
            "/v1/merchant/settings",
            get(get_settings_handler::<R>).put(put_settings_handler::<R>),
        )
        // Public checkout routes (/pay/:id)
        .route("/v1/public/pay/:id", get(get_public_invoice_handler::<R>))
        .route("/v1/public/pay/:id/balance", post(pay_with_balance_handler::<R>))
        .route("/v1/public/pay/:id/select-coin", post(select_coin_handler::<R>))
        .route("/public/pay/:id", get(get_public_invoice_handler::<R>))
        .route("/public/pay/:id/balance", post(pay_with_balance_handler::<R>))
        .route("/public/pay/:id/select-coin", post(select_coin_handler::<R>))
}

#[derive(Deserialize)]
pub struct CreateDepositReq {
    /// Required for a crypto-priced invoice; omitted when pricing in USD.
    pub coin: Option<String>,
    /// Integer ledger units (1e-8). Required alongside `coin`.
    pub amount: Option<String>,
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
    /// Price in USD, as a decimal string ("25.00"). Alternative to
    /// `coin` + `amount`: the customer picks the coin at the checkout.
    /// Unlike `amount`, this is real money and therefore decimal.
    #[serde(rename = "amountUsd")]
    pub amount_usd: Option<String>,
    /// Coins the customer may choose from. Defaults to the merchant's
    /// configured list. Only meaningful alongside `amountUsd`.
    #[serde(rename = "acceptedCoins")]
    pub accepted_coins: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct MerchantSettingsReq {
    #[serde(rename = "acceptedCoins")]
    pub accepted_coins: Vec<String>,
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

/// USD price as a decimal string ("25.00"), scaled to `decimals`.
///
/// Unlike `amount`, this one **is** decimal: it is fiat money, not a ledger
/// integer. Two spellings of the same rule would be confusing, so the error
/// says which field it is talking about.
fn parse_usd(raw: &str, decimals: u32) -> Result<BigDecimal, Rejected> {
    let t = raw.trim().replace(',', ".");
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == '.') || t.matches('.').count() > 1 {
        return Err(Rejected::bad_request("INVALID_AMOUNT_USD", "amountUsd must be a positive decimal like \"25.00\""));
    }
    let (whole, frac) = t.split_once('.').unwrap_or((t.as_str(), ""));
    if frac.len() > decimals as usize {
        return Err(Rejected::bad_request(
            "INVALID_AMOUNT_USD",
            format!("amountUsd supports at most {decimals} decimal places"),
        ));
    }
    let digits = format!("{}{}", if whole.is_empty() { "0" } else { whole }, format_args!("{:0<width$}", frac, width = decimals as usize));
    let scaled = BigDecimal::from_str(&digits)
        .map_err(|_| Rejected::bad_request("INVALID_AMOUNT_USD", "amountUsd must be a positive decimal like \"25.00\""))?;
    if scaled <= BigDecimal::from(0) {
        return Err(Rejected::bad_request("INVALID_AMOUNT_USD", "amountUsd must be greater than 0"));
    }
    Ok(scaled)
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

/// Payment URI for a wallet to scan. UTXO coins stay BIP21
/// (`btc:addr?amount=0.001`). EVM wallets dump `pol:` and `ethereum:0x…@137?value=`
/// as raw text, so POL/USDT/USDC/PEPE encode only the `0x` address. The
/// amount stays on the checkout page.
pub(crate) fn payment_uri(coin: &str, address: &str, ledger_amount: &BigDecimal) -> String {
    let network = match std::env::var("CHAIN_NETWORK").as_deref() {
        Ok("testnet") => chain::ChainNetwork::Testnet,
        _ => chain::ChainNetwork::Mainnet,
    };
    payment_uri_for(coin, address, ledger_amount, network)
}

fn payment_uri_for(
    coin: &str,
    address: &str,
    ledger_amount: &BigDecimal,
    network: chain::ChainNetwork,
) -> String {
    let Some(parsed) = coin.parse::<shared::Coin>().ok() else {
        return format!("{}:{address}?amount={}", coin.to_lowercase(), ledger_amount);
    };
    let params = chain::params_for(parsed, network);
    if params.evm_chain_id.is_some() {
        return address.to_string();
    }
    format!("{}:{address}?amount={}", coin.to_lowercase(), human_amount(coin, ledger_amount))
}

/// Ledger units (1e-8) rendered as the decimal quantity of coins a human — or
/// a wallet — expects: `2500000000` → `25`.
fn human_amount(coin: &str, ledger_amount: &BigDecimal) -> String {
    coin.parse::<shared::Coin>()
        .ok()
        .and_then(|c| ledger_amount.to_u128().map(|units| shared::format_amount(units, c)))
        .unwrap_or_else(|| ledger_amount.to_string())
}

/// The create/get response body. One builder so the idempotent replay and the
/// fresh insert can never answer with different shapes.
fn invoice_json(inv: &MerchantDepositInvoice, base_url: &str) -> serde_json::Value {
    let qr_code = payment_uri(&inv.coin, &inv.deposit_address, &inv.amount);
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
        // Multi-coin: what the customer may pick, and whether they still can.
        "acceptedCoins": inv.accepted_coins,
        "amountUsd": usd_display(inv),
        "coinLocked": inv.coin_locked_at.is_some(),
    })
}

/// True when a re-POST for the same `orderId` describes the same charge and
/// the original invoice is still payable — the definition of an idempotent
/// retry for this endpoint.
fn is_idempotent_replay(
    existing: &MerchantDepositInvoice,
    coin: shared::Coin,
    amount: &BigDecimal,
    price_usd_scaled: Option<&BigDecimal>,
) -> bool {
    if !matches!(existing.status.as_str(), "PENDING" | "DETECTED") || existing.expires_at <= chrono::Utc::now() {
        return false;
    }
    match price_usd_scaled {
        // USD-priced: the charge is the dollar amount. The coin may already
        // have changed because the customer picked one, and that is still the
        // same order.
        Some(usd) => existing.price_usd_scaled.as_ref() == Some(usd),
        None => existing.coin == coin.as_str() && &existing.amount == amount,
    }
}

async fn create_deposit_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    auth: MerchantAuth,
) -> Response {
    let Ok(body) = serde_json::from_slice::<CreateDepositReq>(&auth.body) else {
        return fail(StatusCode::BAD_REQUEST, "INVALID_BODY", "invalid request body");
    };

    // Two pricing modes, never both: `coin` + `amount` prices in crypto (the
    // original contract), `amountUsd` prices in dollars and lets the customer
    // pick the coin. Accepting both would leave it ambiguous which one the
    // merchant meant to charge.
    let prices_in_usd = body.amount_usd.is_some();
    if prices_in_usd && (body.coin.is_some() || body.amount.is_some()) {
        return fail(
            StatusCode::BAD_REQUEST,
            "AMBIGUOUS_AMOUNT",
            "send either coin + amount, or amountUsd — not both",
        );
    }
    if !prices_in_usd && (body.coin.is_none() || body.amount.is_none()) {
        return fail(
            StatusCode::BAD_REQUEST,
            "INVALID_AMOUNT",
            "send coin + amount (ledger units), or amountUsd",
        );
    }

    // Which coins the customer may pick from. Explicit list wins; otherwise
    // the merchant's saved setting. Paused coins are filtered out of both.
    let accepted: Vec<shared::Coin> = if prices_in_usd {
        let requested: Option<Vec<shared::Coin>> = match &body.accepted_coins {
            Some(raw) => {
                let mut parsed = Vec::with_capacity(raw.len());
                for c in raw {
                    match c.parse::<shared::Coin>() {
                        Ok(p) => parsed.push(p),
                        Err(_) => return fail(StatusCode::BAD_REQUEST, "UNKNOWN_COIN", &format!("unknown coin: {c}")),
                    }
                }
                Some(parsed)
            }
            None => None,
        };
        let resolved = match requested {
            Some(list) => db::merchant_settings::live_coins(&list),
            None => match db::merchant_settings::accepted_coins(&state.pool, auth.merchant_id).await {
                Ok(list) => list,
                Err(e) => return crate::http_error::internal_error(&e),
            },
        };
        if resolved.is_empty() {
            return fail(
                StatusCode::BAD_REQUEST,
                "NO_USABLE_COIN",
                "none of the accepted coins currently take deposits",
            );
        }
        resolved
    } else {
        Vec::new()
    };

    let Ok(coin) = body.coin.clone().unwrap_or_else(|| accepted[0].as_str().to_string()).parse::<shared::Coin>() else {
        return fail(StatusCode::BAD_REQUEST, "UNKNOWN_COIN", "unknown coin");
    };
    // Same pause list as personal deposits — merchant invoice addresses for
    // BTC/LTC/DOGE/BCH/DGB stay blocked until DEPOSIT_WITHDRAW_PAUSED_COINS is cleared.
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

    // Crypto-priced: the merchant states the exact ledger amount.
    // USD-priced: quote every accepted coin now, and open the invoice on the
    // first one that has a fresh price. A coin without one is simply not
    // offered — never quoted at an invented rate.
    let (coin, amount, price_usd_scaled, price_decimals, quote_price) = if prices_in_usd {
        let decimals = match db::pricing::list_cached_prices(&state.pool).await {
            Ok((d, _)) => d,
            Err(e) => return fail(StatusCode::SERVICE_UNAVAILABLE, "PRICE_UNAVAILABLE", &e.to_string()),
        };
        let usd = match parse_usd(body.amount_usd.as_deref().unwrap_or(""), decimals) {
            Ok(v) => v,
            Err(rejected) => return rejected.into(),
        };
        let options = db::merchant_multicoin::price_options(
            &state.pool,
            &accepted,
            &usd,
            state.settings.price_max_stale,
        )
        .await;
        let Some(first) = options.first() else {
            return fail(
                StatusCode::SERVICE_UNAVAILABLE,
                "PRICE_UNAVAILABLE",
                "no accepted coin has a fresh price right now",
            );
        };
        (first.coin, first.amount.clone(), Some(usd), Some(decimals as i32), Some(first.price_scaled.clone()))
    } else {
        let parsed = match parse_amount(body.amount.as_deref().unwrap_or(""), coin) {
            Ok(a) => a,
            Err(rejected) => return rejected.into(),
        };
        (coin, parsed, None, None, None)
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
            return if is_idempotent_replay(&existing, coin, &amount, price_usd_scaled.as_ref()) {
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
        Err(e) => return crate::http_error::internal_error(&e),
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

    let mut input = CreateDepositInvoiceInput {
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
        accepted_coins: accepted.clone(),
        price_usd_scaled: price_usd_scaled.clone(),
        price_decimals,
        quote_price_scaled: quote_price.clone(),
    };
    seal_invoice_pii(&state.secrets, &mut input);

    match create_invoice(&state.pool, input).await {
        Ok(inv) => (StatusCode::CREATED, Json(invoice_json(&inv, &state.settings.public_base_url))).into_response(),
        // Lost the race against a concurrent POST with the same orderId:
        // resolve it exactly like the pre-insert check above.
        Err(MerchantDepositError::DuplicateOrderId) => {
            match get_invoice_by_order_id(&state.pool, auth.merchant_id, &body.order_id).await {
                Ok(existing) if is_idempotent_replay(&existing, coin, &amount, price_usd_scaled.as_ref()) => {
                    (StatusCode::OK, Json(invoice_json(&existing, &state.settings.public_base_url))).into_response()
                }
                _ => fail(
                    StatusCode::CONFLICT,
                    "DUPLICATE_ORDER_ID",
                    "orderId already used for a different invoice",
                ),
            }
        }
        Err(_) => fail(StatusCode::BAD_REQUEST, "INVOICE_CREATE_FAILED", "could not create invoice"),
    }
}

async fn list_deposits_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    Query(query): Query<ListDepositsQuery>,
    auth: MerchantAuth,
) -> Response {
    match list_invoices_by_merchant(&state.pool, auth.merchant_id, query.limit.unwrap_or(50), query.offset.unwrap_or(0)).await {
        Ok(list) => {
            let opened: Vec<_> = list.iter().map(|inv| reveal_invoice_pii(&state.secrets, inv)).collect();
            Json(json!({ "invoices": opened })).into_response()
        }
        Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "internal error"),
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
            Json(reveal_invoice_pii(&state.secrets, &inv)).into_response()
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
        Ok(inv) => public_invoice_json(&state, &inv).await,
        Err(_) => fail(StatusCode::NOT_FOUND, "INVOICE_NOT_FOUND", "invoice not found"),
    }
}

/// The checkout payload. When the coin is still open, it carries every option
/// the customer may pick — priced, named and with an icon — so the page needs
/// exactly one request to render the picker.
async fn public_invoice_json<R: AuthRepo>(state: &AppState<R>, inv: &MerchantDepositInvoice) -> Response {
    let qr_code = payment_uri(&inv.coin, &inv.deposit_address, &inv.amount);
    let base = state.settings.public_base_url.trim_end_matches('/');

    // Options are only meaningful while the coin can still change.
    let can_switch = inv.coin_locked_at.is_none()
        && inv.status == "PENDING"
        && inv.expires_at > chrono::Utc::now()
        && inv.accepted_coins.len() > 1;

    let mut options: Vec<serde_json::Value> = Vec::new();
    if can_switch {
        if let Some(price_usd) = inv.price_usd_scaled.clone() {
            let coins: Vec<shared::Coin> = inv
                .accepted_coins
                .iter()
                .filter_map(|c| c.parse::<shared::Coin>().ok())
                .filter(|c| !shared::is_deposit_withdraw_paused(*c))
                .collect();
            for option in db::merchant_multicoin::price_options(
                &state.pool,
                &coins,
                &price_usd,
                state.settings.price_max_stale,
            )
            .await
            {
                options.push(json!({
                    "coin": option.coin.as_str(),
                    "name": shared::coin_config(option.coin).name,
                    "amount": option.amount.to_string(),
                    "amountDisplay": human_amount(option.coin.as_str(), &option.amount),
                    "logoUrl": crate::public_catalog::coin_logo_url(base, option.coin.as_str()),
                    "minConfirmations": shared::coin_config(option.coin).min_confirmations,
                }));
            }
        }
    }

    Json(json!({
        "id": inv.id,
        "status": inv.status,
        "coin": inv.coin,
        "amount": inv.amount.to_string(),
        "amountDisplay": human_amount(&inv.coin, &inv.amount),
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
        "logoUrl": crate::public_catalog::coin_logo_url(base, &inv.coin),
        // Multi-coin: empty while there is nothing to choose between.
        "coinOptions": options,
        "coinLocked": !can_switch,
        "amountUsd": usd_display(inv),
    }))
    .into_response()
}

/// The merchant's USD price, unscaled, for display. `None` on a crypto-priced
/// invoice.
fn usd_display(inv: &MerchantDepositInvoice) -> Option<String> {
    let scaled = inv.price_usd_scaled.as_ref()?;
    let decimals = inv.price_decimals.unwrap_or(8).max(0) as i64;
    Some((scaled / BigDecimal::from(10i64.pow(decimals as u32))).with_scale(2).to_string())
}

#[derive(Deserialize)]
struct SelectCoinReq {
    coin: String,
}

/// Lets the paying customer pick which coin to settle in.
///
/// Deliberately unauthenticated: the payer is a stranger holding a link, not
/// an account. Everything that could be abused is therefore bounded by the
/// invoice itself rather than by the caller:
///
/// * the coin must be in the invoice's own `accepted_coins` — the client
///   cannot widen the set, and a coin paused since creation is refused;
/// * addresses are unique per `(invoice, coin)`, so repeatedly switching
///   back and forth reuses rows instead of burning HD indices. The worst case
///   for one invoice is one address per accepted coin;
/// * once any address of the invoice has money, the selection is locked and
///   this returns 409 — a switch then would strand a payment in flight;
/// * nothing about another merchant, or about this merchant's other
///   invoices, is readable through it.
async fn select_coin_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    Path(id): Path<Uuid>,
    Json(body): Json<SelectCoinReq>,
) -> Response {
    let Ok(coin) = body.coin.parse::<shared::Coin>() else {
        return fail(StatusCode::BAD_REQUEST, "UNKNOWN_COIN", "unknown coin");
    };

    let inv = match get_invoice_by_id(&state.pool, id).await {
        Ok(i) => i,
        Err(_) => return fail(StatusCode::NOT_FOUND, "INVOICE_NOT_FOUND", "invoice not found"),
    };

    if inv.coin_locked_at.is_some() {
        return fail(
            StatusCode::CONFLICT,
            "COIN_LOCKED",
            "this invoice already has a payment in progress and its coin can no longer change",
        );
    }
    if inv.status != "PENDING" || inv.expires_at <= chrono::Utc::now() {
        return fail(StatusCode::BAD_REQUEST, "INVOICE_INVALID_STATE", "invoice is no longer payable");
    }
    // The accepted set comes from the invoice, never from the request.
    if !inv.accepted_coins.iter().any(|c| c == coin.as_str()) || shared::is_deposit_withdraw_paused(coin) {
        return fail(
            StatusCode::BAD_REQUEST,
            "COIN_NOT_ACCEPTED",
            "this coin is not accepted for this invoice",
        );
    }

    // Reuse the address already minted for this coin, if any: a refresh or a
    // customer flipping between two coins must not mint a second one.
    let existing = match db::merchant_multicoin::find_invoice_address(&state.pool, inv.id, coin).await {
        Ok(found) => found,
        Err(e) => return crate::http_error::internal_error(&e),
    };

    let addr = match existing {
        Some(a) => a,
        None => {
            let Some(price_usd) = inv.price_usd_scaled.clone() else {
                return fail(
                    StatusCode::BAD_REQUEST,
                    "COIN_NOT_ACCEPTED",
                    "this invoice is priced in a single coin and cannot be switched",
                );
            };
            let options = db::merchant_multicoin::price_options(
                &state.pool,
                &[coin],
                &price_usd,
                state.settings.price_max_stale,
            )
            .await;
            let Some(option) = options.into_iter().next() else {
                return fail(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "PRICE_UNAVAILABLE",
                    "no fresh price for this coin right now",
                );
            };

            let client = state.chain_registry.get(coin);
            let generated = match client.generate_address(&inv.merchant_id.to_string()).await {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!(invoice_id = %inv.id, coin = coin.as_str(), error = %e.message, "select-coin: address generation failed");
                    return fail(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "ADDRESS_UNAVAILABLE",
                        "could not generate a deposit address for this coin right now",
                    );
                }
            };

            match db::merchant_multicoin::upsert_invoice_address(
                &state.pool,
                inv.id,
                coin,
                &generated.address,
                generated.hd_index.map(|i| i as i64),
                &option.amount,
                Some(&option.price_scaled),
            )
            .await
            {
                Ok(a) => a,
                Err(e) => return crate::http_error::internal_error(&e),
            }
        }
    };

    // `lock = false`: choosing is not committing. The customer may still
    // change their mind until money actually shows up.
    match db::merchant_multicoin::select_coin(&state.pool, inv.id, coin, &addr, false).await {
        Ok(updated) => public_invoice_json(&state, &updated).await,
        Err(db::merchant_multicoin::MultiCoinError::CoinLocked) => {
            fail(StatusCode::CONFLICT, "COIN_LOCKED", "this invoice already has a payment in progress")
        }
        Err(db::merchant_multicoin::MultiCoinError::NotPayable) => {
            fail(StatusCode::BAD_REQUEST, "INVOICE_INVALID_STATE", "invoice is no longer payable")
        }
        Err(e) => crate::http_error::internal_error(&e),
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
        Err(MerchantDepositError::PayerIsMerchant) => {
            fail(
                StatusCode::BAD_REQUEST,
                "CANNOT_PAY_OWN_INVOICE",
                "this invoice belongs to the same SatsPay account that is paying. the checkout cannot move money from your personal wallet into your own merchant wallet. nothing was debited",
            )
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

/// Coins this merchant accepts on the hosted checkout.
async fn get_settings_handler<R: AuthRepo>(State(state): State<AppState<R>>, auth: MerchantAuth) -> Response {
    match db::merchant_settings::accepted_coins(&state.pool, auth.merchant_id).await {
        Ok(coins) => Json(json!({
            "acceptedCoins": coins.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "availableCoins": shared::COINS
                .into_iter()
                .filter(|c| !shared::is_deposit_withdraw_paused(*c))
                .map(|c| c.as_str())
                .collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(e) => crate::http_error::internal_error(&e),
    }
}

async fn put_settings_handler<R: AuthRepo>(State(state): State<AppState<R>>, auth: MerchantAuth) -> Response {
    let Ok(body) = serde_json::from_slice::<MerchantSettingsReq>(&auth.body) else {
        return fail(StatusCode::BAD_REQUEST, "INVALID_BODY", "invalid request body");
    };
    let mut coins = Vec::with_capacity(body.accepted_coins.len());
    for raw in &body.accepted_coins {
        match raw.parse::<shared::Coin>() {
            Ok(c) => coins.push(c),
            Err(_) => return fail(StatusCode::BAD_REQUEST, "UNKNOWN_COIN", &format!("unknown coin: {raw}")),
        }
    }

    match db::merchant_settings::set_accepted_coins(&state.pool, auth.merchant_id, &coins).await {
        Ok(saved) => Json(json!({ "acceptedCoins": saved.iter().map(|c| c.as_str()).collect::<Vec<_>>() })).into_response(),
        Err(db::merchant_settings::MerchantSettingsError::NoUsableCoin) => fail(
            StatusCode::BAD_REQUEST,
            "NO_USABLE_COIN",
            "at least one accepted coin must currently be enabled for deposits",
        ),
        Err(e) => crate::http_error::internal_error(&e),
    }
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
        assert!(is_idempotent_replay(&inv, shared::Coin::Pol, &BigDecimal::from(500_000), None));
        // Different amount for the same orderId is a conflict, not a replay.
        assert!(!is_idempotent_replay(&inv, shared::Coin::Pol, &BigDecimal::from(500_001), None));
        assert!(!is_idempotent_replay(&inv, shared::Coin::Usdt, &BigDecimal::from(500_000), None));

        let paid = sample_invoice("CONFIRMED", chrono::Utc::now() + chrono::Duration::minutes(10));
        assert!(!is_idempotent_replay(&paid, shared::Coin::Pol, &BigDecimal::from(500_000), None));

        let expired = sample_invoice("PENDING", chrono::Utc::now() - chrono::Duration::minutes(1));
        assert!(!is_idempotent_replay(&expired, shared::Coin::Pol, &BigDecimal::from(500_000), None));
    }

    /// A USD-priced retry is the same order even after the customer has
    /// picked a coin, so matching on the coin would mint a second invoice
    /// for one order.
    #[test]
    fn usd_priced_replay_matches_on_the_dollar_amount() {
        let mut inv = sample_invoice("PENDING", chrono::Utc::now() + chrono::Duration::minutes(10));
        inv.price_usd_scaled = Some(BigDecimal::from(2_500_000_000u64));
        inv.coin = "SOL".into(); // customer already switched away from the default

        let usd = BigDecimal::from(2_500_000_000u64);
        assert!(is_idempotent_replay(&inv, shared::Coin::Pol, &BigDecimal::from(1), Some(&usd)));

        let other = BigDecimal::from(9_900_000_000u64);
        assert!(!is_idempotent_replay(&inv, shared::Coin::Pol, &BigDecimal::from(1), Some(&other)));
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
            accepted_coins: vec!["POL".into()],
            price_usd_scaled: None,
            price_decimals: None,
            coin_locked_at: Some(chrono::Utc::now()),
            quote_price_scaled: None,
            created_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;

    /// The paying customer reads this. A 25 USDT invoice used to render
    /// "2500000000 USDT" and encode the same number into the payment URI,
    /// which a scanning wallet reads as 2.5 billion USDT.
    #[test]
    fn amounts_render_as_coins_not_ledger_units() {
        assert_eq!(human_amount("USDT", &BigDecimal::from(2_500_000_000u64)), "25");
        assert_eq!(human_amount("BTC", &BigDecimal::from(1u64)), "0.00000001");
        assert_eq!(human_amount("LTC", &BigDecimal::from(1_000u64)), "0.00001");
        assert_eq!(human_amount("POL", &BigDecimal::from(100_000_000u64)), "1");
    }

    #[test]
    fn payment_uri_encodes_the_coin_quantity() {
        let net = chain::ChainNetwork::Mainnet;
        let uri = payment_uri_for("USDT", "0xabc", &BigDecimal::from(2_500_000_000u64), net);
        assert_eq!(uri, "0xabc");
        assert!(!uri.contains(':'), "wallet shows the URI as text: {uri}");
        assert_eq!(
            payment_uri_for("BTC", "bc1qtest", &BigDecimal::from(100_000u64), net),
            "btc:bc1qtest?amount=0.001"
        );
    }

    #[test]
    fn pol_qr_is_the_bare_address() {
        let uri = payment_uri_for(
            "POL",
            "0x12E5FCB80D69929Daa4e978C26De73271f9E70A5",
            &BigDecimal::from(190_485_262u64),
            chain::ChainNetwork::Mainnet,
        );
        assert_eq!(uri, "0x12E5FCB80D69929Daa4e978C26De73271f9E70A5");
    }

    #[test]
    fn unknown_coin_falls_back_to_the_raw_value() {
        assert_eq!(human_amount("NOPE", &BigDecimal::from(42u64)), "42");
    }
}
