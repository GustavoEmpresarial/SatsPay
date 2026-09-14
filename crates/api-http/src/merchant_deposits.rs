use crate::middleware::AuthUser;
use crate::notify_email::send_best_effort;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bigdecimal::BigDecimal;
use chrono::Utc;
use db::merchant_deposits::{
    create_invoice, get_invoice_by_id, list_invoices_by_merchant,
    pay_invoice_with_balance, record_webhook_delivery, CreateDepositInvoiceInput,
    MerchantDepositInvoice,
};
use domain::auth::AuthRepo;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        // Merchant API endpoints (Site X integration)
        .route("/v1/merchant/deposits/create", post(create_deposit_handler::<R>))
        .route("/v1/merchant/deposits", get(list_deposits_handler::<R>))
        .route("/v1/merchant/deposits/:id", get(get_deposit_handler::<R>))
        .route("/v1/merchant/deposits/:id/test-webhook", post(test_webhook_handler::<R>))
        // Aliases for invoices
        .route("/v1/merchant/invoices", get(list_deposits_handler::<R>).post(create_deposit_handler::<R>))
        .route("/v1/merchant/invoices/:id", get(get_deposit_handler::<R>))
        // Public checkout routes (/pay/:id)
        .route("/v1/public/pay/:id", get(get_public_invoice_handler::<R>))
        .route("/v1/public/pay/:id/balance", post(pay_with_balance_handler::<R>))
        .route("/v1/public/pay/:id/simulate-payment", post(simulate_demo_payment_handler::<R>))
        .route("/public/pay/:id", get(get_public_invoice_handler::<R>))
        .route("/public/pay/:id/balance", post(pay_with_balance_handler::<R>))
        .route("/public/pay/:id/simulate-payment", post(simulate_demo_payment_handler::<R>))
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

/// Helper to authenticate merchant either via `x-api-key` header or via JWT session (dashboard)
async fn authenticate_merchant<R: AuthRepo>(
    state: &AppState<R>,
    headers: &HeaderMap,
    req_user: Option<AuthUser>,
) -> Result<(Uuid, Option<Uuid>), (StatusCode, Response)> {
    if let Some(user) = req_user {
        return Ok((user.id, None));
    }

    if let Some(api_key_header) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        if api_key_header.len() == 64 && api_key_header.chars().all(|c| c.is_ascii_hexdigit()) {
            let record = db::public_api::authenticate_by_hash(&state.pool, &state.secrets, api_key_header, "0.0.0.0")
                .await
                .map_err(|e| (StatusCode::UNAUTHORIZED, Json(json!({ "error": e.to_string() })).into_response()))?;
            return Ok((record.user_id, Some(record.id)));
        }
    }

    Err((StatusCode::UNAUTHORIZED, Json(json!({ "error": "missing or invalid API key / authentication" })).into_response()))
}

fn auth_err((status, body): (StatusCode, Response)) -> Response {
    let mut out = body;
    *out.status_mut() = status;
    out
}

async fn create_deposit_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    req_user: Option<AuthUser>,
    Json(body): Json<CreateDepositReq>,
) -> Response {
    let (merchant_id, api_key_id) = match authenticate_merchant(&state, &headers, req_user).await {
        Ok(v) => v,
        Err(e) => return auth_err(e),
    };

    let Ok(coin) = body.coin.parse::<shared::Coin>() else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "unknown coin" }))).into_response();
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
    let Ok(amount) = BigDecimal::from_str(&body.amount) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid amount" }))).into_response();
    };
    if amount <= BigDecimal::from(0) {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "amount must be greater than 0" }))).into_response();
    }

    // Generate dedicated deposit address
    let client = state.chain_registry.get(coin);
    let deposit_address = match client.generate_address(&merchant_id.to_string()).await {
        Ok(addr) => addr.address,
        Err(_) => {
            // Fallback to merchant deposit address
            match db::deposits::get_or_create_address(&state.pool, merchant_id, coin, client.as_ref()).await {
                Ok(addr) => addr,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
            }
        }
    };

    let input = CreateDepositInvoiceInput {
        merchant_id,
        api_key_id,
        site_user_id: body.site_user_id,
        order_id: body.order_id,
        site_name: body.site_name,
        coin,
        amount,
        deposit_address: deposit_address.clone(),
        callback_url: body.callback_url,
        success_url: body.success_url,
        cancel_url: body.cancel_url,
        customer_email: body.customer_email,
        customer_name: body.customer_name,
        description: body.description,
        expiry_minutes: body.expiry_minutes,
    };

    match create_invoice(&state.pool, input).await {
        Ok(inv) => {
            let qr_code = format!("{}:{}?amount={}", coin.as_str().to_lowercase(), inv.deposit_address, inv.amount);
            let pay_url = format!("/pay/{}", inv.id);
            (
                StatusCode::CREATED,
                Json(json!({
                    "id": inv.id,
                    "status": inv.status,
                    "coin": inv.coin,
                    "amount": inv.amount.to_string(),
                    "feeAmount": inv.fee_amount.to_string(),
                    "netAmount": inv.net_amount.to_string(),
                    "depositAddress": inv.deposit_address,
                    "payUrl": pay_url,
                    "qrCode": qr_code,
                    "orderId": inv.order_id,
                    "expiresAt": inv.expires_at,
                    "createdAt": inv.created_at,
                })),
            )
                .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_deposits_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    req_user: Option<AuthUser>,
    Query(query): Query<ListDepositsQuery>,
) -> Response {
    let (merchant_id, _) = match authenticate_merchant(&state, &headers, req_user).await {
        Ok(v) => v,
        Err(e) => return auth_err(e),
    };

    match list_invoices_by_merchant(&state.pool, merchant_id, query.limit.unwrap_or(50), query.offset.unwrap_or(0)).await {
        Ok(list) => Json(json!({ "invoices": list })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn get_deposit_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    req_user: Option<AuthUser>,
    Path(id): Path<Uuid>,
) -> Response {
    let (merchant_id, _) = match authenticate_merchant(&state, &headers, req_user).await {
        Ok(v) => v,
        Err(e) => return auth_err(e),
    };

    match get_invoice_by_id(&state.pool, id).await {
        Ok(inv) => {
            if inv.merchant_id != merchant_id {
                return (StatusCode::FORBIDDEN, Json(json!({ "error": "forbidden" }))).into_response();
            }
            Json(inv).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, Json(json!({ "error": "invoice not found" }))).into_response(),
    }
}

/// Public endpoint for `/pay/:id` checkout page
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
        Err(_) => (StatusCode::NOT_FOUND, Json(json!({ "error": "invoice not found" }))).into_response(),
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
            tokio::spawn(async move {
                dispatch_webhook(&pool_clone, &inv_clone).await;
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
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
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
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "invoice not found" }))).into_response(),
    };
    if inv.merchant_id != user.id {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "forbidden" }))).into_response();
    }

    let (delivered, status_code, err_msg) = dispatch_webhook(&state.pool, &inv).await;
    Json(json!({
        "delivered": delivered,
        "statusCode": status_code,
        "error": err_msg,
    }))
    .into_response()
}

/// Dispatch signed HTTP POST webhook to merchant's callback_url
pub async fn dispatch_webhook(pool: &sqlx::PgPool, inv: &MerchantDepositInvoice) -> (bool, Option<i32>, Option<String>) {
    let payload = json!({
        "event": "deposit.confirmed",
        "invoiceId": inv.id,
        "orderId": inv.order_id,
        "siteUserId": inv.site_user_id,
        "coin": inv.coin,
        "amount": inv.amount.to_string(),
        "fee": inv.fee_amount.to_string(),
        "netAmount": inv.net_amount.to_string(),
        "txHash": inv.tx_hash.as_deref().unwrap_or("internal_satspay"),
        "status": inv.status,
        "paidAt": inv.paid_at.unwrap_or_else(Utc::now),
        "customerEmail": inv.customer_email,
    });

    let payload_str = payload.to_string();

    // Create HMAC-SHA256 signature
    let secret_key = "satspay_secret_default";
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload_str.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            let err = e.to_string();
            record_webhook_delivery(pool, inv.id, false, None, Some(&err)).await.ok();
            return (false, None, Some(err));
        }
    };

    match client
        .post(&inv.callback_url)
        .header("Content-Type", "application/json")
        .header("X-SatsPay-Signature", format!("sha256={signature}"))
        .body(payload_str)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16() as i32;
            let delivered = resp.status().is_success();
            let err = if delivered { None } else { Some(format!("HTTP status {}", status)) };
            record_webhook_delivery(pool, inv.id, delivered, Some(status), err.as_deref()).await.ok();
            (delivered, Some(status), err)
        }
        Err(e) => {
            let err = e.to_string();
            record_webhook_delivery(pool, inv.id, false, None, Some(&err)).await.ok();
            (false, None, Some(err))
        }
    }
}

/// Simulate blockchain payment reception for demo / sandbox
async fn simulate_demo_payment_handler<R: AuthRepo>(
    State(state): State<AppState<R>>,
    Path(id): Path<Uuid>,
) -> Response {
    let tx_hash = format!("0x{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    match db::merchant_deposits::confirm_invoice(&state.pool, id, Some(&tx_hash)).await {
        Ok(inv) => {
            let pool_clone = state.pool.clone();
            let inv_clone = inv.clone();
            tokio::spawn(async move {
                dispatch_webhook(&pool_clone, &inv_clone).await;
            });
            Json(json!({
                "success": true,
                "message": "Pagamento simulado com sucesso na blockchain!",
                "txHash": tx_hash,
                "invoice": inv,
            }))
            .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
