use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::ledger::{apply_ledger_entry, get_wallet_balance, lock_wallet, LedgerCreditInput, LedgerError};

#[derive(Debug, thiserror::Error)]
pub enum MerchantDepositError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error("invoice not found")]
    NotFound,
    #[error("invoice already paid or expired")]
    InvalidStatus,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("merchant wallet not found")]
    MerchantWalletNotFound,
    #[error("payer wallet not found")]
    PayerWalletNotFound,
    /// The logged-in payer is the merchant who owns the invoice. Checkout is
    /// for a different customer. Moving your own funds is the wallet transfer,
    /// not this path.
    #[error("cannot pay your own invoice")]
    PayerIsMerchant,
    #[error("orderId already used for a different invoice")]
    DuplicateOrderId,
}

/// True for a Postgres unique-constraint violation (SQLSTATE 23505).
fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

/// Serialized straight to JSON by `GET /v1/merchant/deposits[/:id]`, so the
/// field names are part of the public contract. camelCase matches every
/// hand-built response in this domain (and what the merchant dashboard has
/// always read) — without it the list returned `order_id`/`fee_amount` and
/// the dashboard rendered blanks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantDepositInvoice {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub site_user_id: Option<String>,
    pub order_id: String,
    pub site_name: Option<String>,
    pub coin: String,
    pub amount: BigDecimal,
    pub fee_amount: BigDecimal,
    pub net_amount: BigDecimal,
    pub deposit_address: String,
    /// Derivation index of `deposit_address` — needed to sweep the funds.
    pub hd_index: Option<i64>,
    pub tx_hash: Option<String>,
    pub confirmations: i32,
    /// Ledger units (1e-8) actually seen at `deposit_address` so far.
    pub received_amount: BigDecimal,
    pub callback_url: String,
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub description: Option<String>,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub webhook_delivered: bool,
    pub webhook_status_code: Option<i32>,
    pub webhook_attempts: i32,
    pub webhook_last_error: Option<String>,
    pub webhook_last_attempt_at: Option<DateTime<Utc>>,
    pub webhook_next_retry_at: Option<DateTime<Utc>>,
    /// Coins the customer may switch to. One entry means no choice is offered.
    pub accepted_coins: Vec<String>,
    /// What the merchant asked for in USD, scaled by `price_decimals`.
    /// `None` on an invoice priced directly in crypto.
    pub price_usd_scaled: Option<BigDecimal>,
    pub price_decimals: Option<i32>,
    /// Set once the coin is final — the customer confirmed, or money arrived.
    /// While `None`, the selection can still change.
    pub coin_locked_at: Option<DateTime<Utc>>,
    /// Coin price used for the current selection, for reconciliation.
    pub quote_price_scaled: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDepositInvoiceInput {
    pub merchant_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub site_user_id: Option<String>,
    pub order_id: String,
    pub site_name: Option<String>,
    pub coin: shared::Coin,
    pub amount: BigDecimal,
    pub deposit_address: String,
    pub hd_index: Option<i64>,
    pub callback_url: String,
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub description: Option<String>,
    pub expiry_minutes: Option<i64>,
    /// Coins the customer may pick from. Empty or single = no choice offered,
    /// and the coin is locked at creation.
    pub accepted_coins: Vec<shared::Coin>,
    /// USD the merchant asked for, scaled by `price_decimals`.
    pub price_usd_scaled: Option<BigDecimal>,
    pub price_decimals: Option<i32>,
    /// Coin price used for the initial selection.
    pub quote_price_scaled: Option<BigDecimal>,
}

pub fn invoice_pii_key(merchant_id: Uuid, order_id: &str) -> String {
    crypto::SecretsService::invoice_row_key(&merchant_id.to_string(), order_id)
}

pub fn seal_invoice_pii(secrets: &crypto::SecretsService, input: &mut CreateDepositInvoiceInput) {
    let key = invoice_pii_key(input.merchant_id, &input.order_id);
    input.callback_url = secrets.seal_pii("invoice.callback", &key, &input.callback_url);
    if let Some(v) = input.success_url.take() {
        input.success_url = Some(secrets.seal_pii("invoice.success", &key, &v));
    }
    if let Some(v) = input.cancel_url.take() {
        input.cancel_url = Some(secrets.seal_pii("invoice.cancel", &key, &v));
    }
    if let Some(v) = input.customer_email.take() {
        input.customer_email = Some(secrets.seal_pii("invoice.customer_email", &key, &v));
    }
    if let Some(v) = input.customer_name.take() {
        input.customer_name = Some(secrets.seal_pii("invoice.customer_name", &key, &v));
    }
    if let Some(v) = input.site_user_id.take() {
        input.site_user_id = Some(secrets.seal_pii("invoice.site_user", &key, &v));
    }
}

pub fn reveal_invoice_pii(secrets: &crypto::SecretsService, inv: &MerchantDepositInvoice) -> MerchantDepositInvoice {
    let key = invoice_pii_key(inv.merchant_id, &inv.order_id);
    let mut out = inv.clone();
    out.callback_url = secrets.open_pii("invoice.callback", &key, &inv.callback_url);
    out.success_url = secrets.open_pii_opt("invoice.success", &key, inv.success_url.as_deref());
    out.cancel_url = secrets.open_pii_opt("invoice.cancel", &key, inv.cancel_url.as_deref());
    out.customer_email = secrets.open_pii_opt("invoice.customer_email", &key, inv.customer_email.as_deref());
    out.customer_name = secrets.open_pii_opt("invoice.customer_name", &key, inv.customer_name.as_deref());
    out.site_user_id = secrets.open_pii_opt("invoice.site_user", &key, inv.site_user_id.as_deref());
    out
}

/// Every column [`row_to_invoice`] reads, in one place — the three read
/// paths (create/get/list) used to keep three hand-maintained copies of this
/// list and of the mapping below.
const INVOICE_COLUMNS: &str = "id, merchant_id, api_key_id, site_user_id, order_id, site_name, \
     coin::text as coin, amount, fee_amount, net_amount, deposit_address, hd_index, \
     tx_hash, confirmations, received_amount, callback_url, success_url, cancel_url, \
     customer_email, customer_name, description, status::text as status, \
     expires_at, paid_at, webhook_delivered, webhook_status_code, \
     webhook_attempts, webhook_last_error, webhook_last_attempt_at, webhook_next_retry_at, \
     accepted_coins::text[] as accepted_coins, price_usd_scaled, price_decimals, \
     coin_locked_at, quote_price_scaled, created_at";

fn row_to_invoice(row: &sqlx::postgres::PgRow) -> MerchantDepositInvoice {
    MerchantDepositInvoice {
        id: row.get("id"),
        merchant_id: row.get("merchant_id"),
        api_key_id: row.get("api_key_id"),
        site_user_id: row.get("site_user_id"),
        order_id: row.get("order_id"),
        site_name: row.get("site_name"),
        coin: row.get("coin"),
        amount: row.get("amount"),
        fee_amount: row.get("fee_amount"),
        net_amount: row.get("net_amount"),
        deposit_address: row.get("deposit_address"),
        hd_index: row.get("hd_index"),
        tx_hash: row.get("tx_hash"),
        confirmations: row.get("confirmations"),
        received_amount: row.get("received_amount"),
        callback_url: row.get("callback_url"),
        success_url: row.get("success_url"),
        cancel_url: row.get("cancel_url"),
        customer_email: row.get("customer_email"),
        customer_name: row.get("customer_name"),
        description: row.get("description"),
        status: row.get("status"),
        expires_at: row.get("expires_at"),
        paid_at: row.get("paid_at"),
        webhook_delivered: row.get("webhook_delivered"),
        webhook_status_code: row.get("webhook_status_code"),
        webhook_attempts: row.get("webhook_attempts"),
        webhook_last_error: row.get("webhook_last_error"),
        webhook_last_attempt_at: row.get("webhook_last_attempt_at"),
        webhook_next_retry_at: row.get("webhook_next_retry_at"),
        accepted_coins: row.get("accepted_coins"),
        price_usd_scaled: row.get("price_usd_scaled"),
        price_decimals: row.get("price_decimals"),
        coin_locked_at: row.get("coin_locked_at"),
        quote_price_scaled: row.get("quote_price_scaled"),
        created_at: row.get("created_at"),
    }
}

/// Gateway fee in basis points: 0.25%. Single rate for every merchant —
/// there is no per-merchant plan, so changing it here changes it for all.
/// Documented on /docs and in docs/api/http-api-reference.md; keep those in
/// step with this constant.
pub const GATEWAY_FEE_BPS: u32 = 25;

/// Splits `amount` into (fee, net) in **whole** ledger units.
///
/// The ledger stores integer units of 1e-8, so the fee cannot carry a
/// fractional part: 0.25% of 250001 is 625.0025, which would credit a
/// fraction of the smallest representable unit and leave
/// `fee + net != amount`. Truncating the fee keeps the identity exact and
/// rounds in the merchant's favour by at most one unit.
pub(crate) fn split_fee(amount: &BigDecimal) -> (BigDecimal, BigDecimal) {
    let fee = (amount * BigDecimal::from(GATEWAY_FEE_BPS) / BigDecimal::from(10_000)).with_scale(0);
    let net = amount - &fee;
    (fee, net)
}

pub async fn create_invoice(
    pool: &PgPool,
    input: CreateDepositInvoiceInput,
) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let expiry_mins = input.expiry_minutes.unwrap_or(60).max(5).min(1440);
    let expires_at = Utc::now() + chrono::Duration::minutes(expiry_mins);
    
    let (fee_amount, net_amount) = split_fee(&input.amount);

    // The selection offered at creation is always among the accepted coins,
    // so the checkout never shows a coin the merchant did not agree to.
    let mut accepted = input.accepted_coins.clone();
    if accepted.is_empty() {
        accepted.push(input.coin);
    }
    if !accepted.contains(&input.coin) {
        accepted.push(input.coin);
    }
    let accepted_text: Vec<String> = accepted.iter().map(|c| c.as_str().to_string()).collect();
    let locked_at: Option<DateTime<Utc>> = if accepted.len() <= 1 { Some(Utc::now()) } else { None };
    let coin_for_address = input.coin;
    let amount_for_address = input.amount.clone();
    let quote_for_address = input.quote_price_scaled.clone();
    let hd_for_address = input.hd_index;
    let address_for_row = input.deposit_address.clone();

    let row = sqlx::query(&format!(
        "INSERT INTO merchant_deposit_invoices (
            merchant_id, api_key_id, site_user_id, order_id, site_name,
            coin, amount, fee_amount, net_amount, deposit_address,
            callback_url, success_url, cancel_url, customer_email, customer_name,
            description, status, expires_at, hd_index,
            accepted_coins, price_usd_scaled, price_decimals, quote_price_scaled, coin_locked_at
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6::coin, $7, $8, $9, $10,
            $11, $12, $13, $14, $15,
            $16, 'PENDING'::deposit_invoice_status, $17, $18,
            $19::text[]::coin[], $20, $21, $22, $23
        ) RETURNING {INVOICE_COLUMNS}"
    ))
    .bind(input.merchant_id)
    .bind(input.api_key_id)
    .bind(input.site_user_id)
    .bind(input.order_id)
    .bind(input.site_name)
    .bind(input.coin.as_str())
    .bind(input.amount)
    .bind(fee_amount)
    .bind(net_amount)
    .bind(input.deposit_address)
    .bind(input.callback_url)
    .bind(input.success_url)
    .bind(input.cancel_url)
    .bind(input.customer_email)
    .bind(input.customer_name)
    .bind(input.description)
    .bind(expires_at)
    .bind(input.hd_index)
    .bind(&accepted_text)
    .bind(input.price_usd_scaled.as_ref())
    .bind(input.price_decimals)
    .bind(input.quote_price_scaled.as_ref())
    // A single-coin invoice is final the moment it exists; a multi-coin one
    // stays open until the customer picks.
    .bind(locked_at)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        // Unique (merchant_id, order_id) — the caller decides whether this is
        // an idempotent replay or a genuine conflict.
        if is_unique_violation(&e) {
            MerchantDepositError::DuplicateOrderId
        } else {
            MerchantDepositError::Db(e)
        }
    })?;

    let invoice = row_to_invoice(&row);

    // Register the first address in the child table, so the watcher reaches
    // every invoice through one path whether or not the coin ever changes.
    crate::merchant_multicoin::upsert_invoice_address(
        pool,
        invoice.id,
        coin_for_address,
        &address_for_row,
        hd_for_address,
        &amount_for_address,
        quote_for_address.as_ref(),
    )
    .await
    .map_err(|e| MerchantDepositError::Db(sqlx::Error::Protocol(e.to_string())))?;

    Ok(invoice)
}

pub async fn get_invoice_by_id(pool: &PgPool, id: Uuid) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let row = sqlx::query(&format!("SELECT {INVOICE_COLUMNS} FROM merchant_deposit_invoices WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;

    let row = row.ok_or(MerchantDepositError::NotFound)?;
    Ok(row_to_invoice(&row))
}

/// Row-locks the invoice inside `tx` and returns it. Both settlement paths
/// (watcher `confirm_invoice`, checkout `pay_invoice_with_balance`) read the
/// status through here, so they serialize and only the first one credits.
async fn lock_invoice(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let row = sqlx::query(&format!("SELECT {INVOICE_COLUMNS} FROM merchant_deposit_invoices WHERE id = $1 FOR UPDATE"))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;

    let row = row.ok_or(MerchantDepositError::NotFound)?;
    Ok(row_to_invoice(&row))
}

/// Ledger key for the merchant credit. One per invoice, whatever settled it,
/// so the ledger's unique index is a second guard against a double credit.
fn merchant_credit_ref(invoice_id: Uuid) -> String {
    format!("merchant_dep:{invoice_id}")
}

/// Looks up an invoice by its merchant-supplied `order_id` — the idempotency
/// key of the create endpoint.
pub async fn get_invoice_by_order_id(
    pool: &PgPool,
    merchant_id: Uuid,
    order_id: &str,
) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let row = sqlx::query(&format!(
        "SELECT {INVOICE_COLUMNS} FROM merchant_deposit_invoices WHERE merchant_id = $1 AND order_id = $2"
    ))
    .bind(merchant_id)
    .bind(order_id)
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or(MerchantDepositError::NotFound)?;
    Ok(row_to_invoice(&row))
}

pub async fn list_invoices_by_merchant(
    pool: &PgPool,
    merchant_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<MerchantDepositInvoice>, MerchantDepositError> {
    let rows = sqlx::query(&format!(
        "SELECT {INVOICE_COLUMNS} FROM merchant_deposit_invoices \
         WHERE merchant_id = $1 \
         ORDER BY created_at DESC \
         LIMIT $2 OFFSET $3"
    ))
    .bind(merchant_id)
    .bind(limit.clamp(1, 100))
    .bind(offset.max(0))
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(row_to_invoice).collect())
}

/// Confirm an invoice: updates status to CONFIRMED and credits the merchant's MERCHANT wallet in ledger.
pub async fn confirm_invoice(
    pool: &PgPool,
    invoice_id: Uuid,
    tx_hash: Option<&str>,
) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let mut tx = pool.begin().await?;

    let inv = lock_invoice(&mut tx, invoice_id).await?;
    if inv.status == "CONFIRMED" {
        return Ok(inv);
    }
    if inv.status == "CANCELLED" || inv.status == "EXPIRED" {
        return Err(MerchantDepositError::InvalidStatus);
    }

    let _coin: shared::Coin = inv.coin.parse().map_err(|_| MerchantDepositError::NotFound)?;

    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, 'MERCHANT') \
         ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(inv.merchant_id)
    .bind(inv.coin.as_str())
    .execute(&mut *tx)
    .await?;

    // 1. Locate or ensure merchant wallet exists
    let merchant_wallet_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'MERCHANT'"
    )
    .bind(inv.merchant_id)
    .bind(inv.coin.as_str())
    .fetch_optional(&mut *tx)
    .await?;

    let merchant_wallet_id = match merchant_wallet_id {
        Some(id) => id,
        None => {
            // Fallback to personal wallet if merchant wallet not initialized
            let personal_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'"
            )
            .bind(inv.merchant_id)
            .bind(inv.coin.as_str())
            .fetch_optional(&mut *tx)
            .await?;
            personal_id.ok_or(MerchantDepositError::MerchantWalletNotFound)?
        }
    };

    // 2. Lock merchant wallet and credit net amount
    lock_wallet(&mut tx, merchant_wallet_id).await?;
    let ref_key = merchant_credit_ref(inv.id);
    let memo_str = format!("Deposit from Site X (Order: {})", inv.order_id);

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            wallet_id: merchant_wallet_id,
            amount: inv.net_amount.clone(),
            ledger_type: "MERCHANT_DEPOSIT",
            reference_id: Some(inv.id),
            reference_type: Some("MERCHANT_DEPOSIT"),
            reference_key: Some(&ref_key),
            memo: Some(&memo_str),
        },
    )
    .await?;

    // 3. Mark invoice as CONFIRMED
    sqlx::query(
        "UPDATE merchant_deposit_invoices 
         SET status = 'CONFIRMED'::deposit_invoice_status,
             tx_hash = COALESCE($2, tx_hash),
             paid_at = now(),
             updated_at = now()
         WHERE id = $1"
    )
    .bind(invoice_id)
    .bind(tx_hash)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    get_invoice_by_id(pool, invoice_id).await
}

/// Pay invoice using internal SatsPay account balance (1-click instant transfer)
pub async fn pay_invoice_with_balance(
    pool: &PgPool,
    invoice_id: Uuid,
    payer_user_id: Uuid,
) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let mut tx = pool.begin().await?;

    let inv = lock_invoice(&mut tx, invoice_id).await?;
    if inv.status != "PENDING" && inv.status != "DETECTED" {
        return Err(MerchantDepositError::InvalidStatus);
    }
    if inv.expires_at < Utc::now() {
        return Err(MerchantDepositError::InvalidStatus);
    }
    if payer_user_id == inv.merchant_id {
        return Err(MerchantDepositError::PayerIsMerchant);
    }

    // 1. Get payer wallet
    let payer_wallet_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'"
    )
    .bind(payer_user_id)
    .bind(inv.coin.as_str())
    .fetch_optional(&mut *tx)
    .await?;

    let payer_wallet_id = payer_wallet_id.ok_or(MerchantDepositError::PayerWalletNotFound)?;

    // 2. Lock and check balance
    lock_wallet(&mut tx, payer_wallet_id).await?;
    let balance = get_wallet_balance(&mut tx, payer_wallet_id).await?;
    if balance < inv.amount {
        return Err(MerchantDepositError::InsufficientBalance);
    }

    // 3. Debit payer wallet (negative amount)
    let debit_ref = format!("pay_invoice:{}", inv.id);
    let debit_memo = format!("Payment to merchant (Order: {})", inv.order_id);
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            wallet_id: payer_wallet_id,
            amount: -&inv.amount,
            ledger_type: "MERCHANT_CHECKOUT",
            reference_id: Some(inv.id),
            reference_type: Some("MERCHANT_CHECKOUT"),
            reference_key: Some(&debit_ref),
            memo: Some(&debit_memo),
        },
    )
    .await?;

    // 4. Locate merchant wallet
    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, 'MERCHANT') \
         ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(inv.merchant_id)
    .bind(inv.coin.as_str())
    .execute(&mut *tx)
    .await?;
    let merchant_wallet_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'MERCHANT'"
    )
    .bind(inv.merchant_id)
    .bind(inv.coin.as_str())
    .fetch_optional(&mut *tx)
    .await?;

    let merchant_wallet_id = match merchant_wallet_id {
        Some(id) => id,
        None => {
            let p_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'"
            )
            .bind(inv.merchant_id)
            .bind(inv.coin.as_str())
            .fetch_optional(&mut *tx)
            .await?;
            p_id.ok_or(MerchantDepositError::MerchantWalletNotFound)?
        }
    };

    // 5. Lock and credit merchant wallet
    lock_wallet(&mut tx, merchant_wallet_id).await?;
    let credit_ref = merchant_credit_ref(inv.id);
    let credit_memo = format!("Deposit from Site X (Order: {})", inv.order_id);
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput {
            wallet_id: merchant_wallet_id,
            amount: inv.net_amount.clone(),
            ledger_type: "MERCHANT_DEPOSIT",
            reference_id: Some(inv.id),
            reference_type: Some("MERCHANT_DEPOSIT"),
            reference_key: Some(&credit_ref),
            memo: Some(&credit_memo),
        },
    )
    .await?;

    // 6. Update invoice
    sqlx::query(
        "UPDATE merchant_deposit_invoices 
         SET status = 'CONFIRMED'::deposit_invoice_status,
             tx_hash = 'internal_satspay',
             received_amount = amount,
             paid_at = now(),
             updated_at = now()
         WHERE id = $1"
    )
    .bind(invoice_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    get_invoice_by_id(pool, invoice_id).await
}

pub async fn record_webhook_delivery(
    pool: &PgPool,
    invoice_id: Uuid,
    delivered: bool,
    status_code: Option<i32>,
    error_msg: Option<&str>,
) -> Result<(), MerchantDepositError> {
    sqlx::query(
        "UPDATE merchant_deposit_invoices 
         SET webhook_delivered = $2,
             webhook_status_code = $3,
             webhook_attempts = webhook_attempts + 1,
             webhook_last_error = $4,
             webhook_last_attempt_at = now(),
             updated_at = now()
         WHERE id = $1"
    )
    .bind(invoice_id)
    .bind(delivered)
    .bind(status_code)
    .bind(error_msg)
    .execute(pool)
    .await?;

    Ok(())
}

/// Invoices the on-chain watcher still has to look at: not yet paid and not
/// yet past `expires_at`.
pub async fn list_open_invoices(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<MerchantDepositInvoice>, MerchantDepositError> {
    let rows = sqlx::query(&format!(
        "SELECT {INVOICE_COLUMNS} FROM merchant_deposit_invoices \
         WHERE status IN ('PENDING'::deposit_invoice_status, 'DETECTED'::deposit_invoice_status) \
           AND expires_at > now() \
         ORDER BY expires_at ASC \
         LIMIT $1"
    ))
    .bind(limit.clamp(1, 1000))
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(row_to_invoice).collect())
}

/// Records what the chain shows for an invoice that is not yet payable in
/// full (or not yet deeply enough confirmed). Never moves a CONFIRMED
/// invoice back — confirmation is terminal for the credit.
pub async fn mark_invoice_detected(
    pool: &PgPool,
    invoice_id: Uuid,
    received: BigDecimal,
    confirmations: i32,
    tx_hash: Option<&str>,
) -> Result<(), MerchantDepositError> {
    sqlx::query(
        "UPDATE merchant_deposit_invoices
         SET status = 'DETECTED'::deposit_invoice_status,
             received_amount = $2,
             confirmations = $3,
             tx_hash = COALESCE($4, tx_hash),
             updated_at = now()
         WHERE id = $1
           AND status IN ('PENDING'::deposit_invoice_status, 'DETECTED'::deposit_invoice_status)",
    )
    .bind(invoice_id)
    .bind(received)
    .bind(confirmations)
    .bind(tx_hash)
    .execute(pool)
    .await?;

    Ok(())
}

/// Expires invoices whose window closed without a confirmed payment.
/// Returns how many were expired. DETECTED (underpaid/unconfirmed) invoices
/// expire too — the funds stay recorded in `received_amount` for support.
pub async fn expire_due_invoices(pool: &PgPool) -> Result<u64, MerchantDepositError> {
    let done = sqlx::query(
        "UPDATE merchant_deposit_invoices
         SET status = 'EXPIRED'::deposit_invoice_status, updated_at = now()
         WHERE status IN ('PENDING'::deposit_invoice_status, 'DETECTED'::deposit_invoice_status)
           AND expires_at <= now()",
    )
    .execute(pool)
    .await?;

    Ok(done.rows_affected())
}

/// Confirmed invoices whose webhook has not been delivered yet and whose
/// backoff window has elapsed. `max_attempts` caps redelivery so a merchant
/// endpoint that is permanently broken stops consuming the worker.
pub async fn list_webhook_retries(
    pool: &PgPool,
    max_attempts: i32,
    limit: i64,
) -> Result<Vec<MerchantDepositInvoice>, MerchantDepositError> {
    let rows = sqlx::query(&format!(
        "SELECT {INVOICE_COLUMNS} FROM merchant_deposit_invoices \
         WHERE status = 'CONFIRMED'::deposit_invoice_status \
           AND webhook_delivered = FALSE \
           AND webhook_attempts < $1 \
           AND webhook_next_retry_at IS NOT NULL \
           AND webhook_next_retry_at <= now() \
         ORDER BY webhook_next_retry_at ASC \
         LIMIT $2"
    ))
    .bind(max_attempts)
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(row_to_invoice).collect())
}

/// Schedules (or clears) the next webhook redelivery attempt.
pub async fn schedule_webhook_retry(
    pool: &PgPool,
    invoice_id: Uuid,
    next_retry_at: Option<DateTime<Utc>>,
) -> Result<(), MerchantDepositError> {
    sqlx::query("UPDATE merchant_deposit_invoices SET webhook_next_retry_at = $2, updated_at = now() WHERE id = $1")
        .bind(invoice_id)
        .bind(next_retry_at)
        .execute(pool)
        .await?;

    Ok(())
}

/// Marks the invoice address as swept so the watcher does not re-broadcast a
/// sweep every tick once the funds already moved to the hot wallet.
pub async fn mark_invoice_swept(pool: &PgPool, invoice_id: Uuid) -> Result<(), MerchantDepositError> {
    sqlx::query("UPDATE merchant_deposit_invoices SET hd_index = NULL, updated_at = now() WHERE id = $1")
        .bind(invoice_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// How many invoices share `deposit_address`. Must be 1 before an on-chain
/// payment can be attributed to a specific invoice.
pub async fn count_invoices_at_address(pool: &PgPool, address: &str) -> Result<i64, MerchantDepositError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM merchant_deposit_invoices WHERE deposit_address = $1",
    )
    .bind(address)
    .fetch_one(pool)
    .await?;

    Ok(count)
}

#[cfg(test)]
mod fee_tests {
    use super::*;
    use std::str::FromStr;

    /// The ledger holds integer units of 1e-8, so a fee with a fractional
    /// part would credit a fraction of the smallest unit and break the
    /// identity `fee + net == amount`.
    #[test]
    fn fee_is_a_whole_number_of_ledger_units() {
        for raw in ["250000", "250001", "1", "99999999", "2500000000"] {
            let amount = BigDecimal::from_str(raw).unwrap();
            let (fee, net) = split_fee(&amount);
            assert_eq!(fee.fractional_digit_count().max(0), 0, "fee {fee} for amount {raw} is fractional");
            assert_eq!(net.fractional_digit_count().max(0), 0, "net {net} for amount {raw} is fractional");
            assert_eq!(&fee + &net, amount, "fee + net must equal amount for {raw}");
            assert!(fee >= BigDecimal::from(0) && net >= BigDecimal::from(0), "no negative side for {raw}");
        }
    }

    #[test]
    fn fee_is_a_quarter_percent_truncated_down() {
        assert_eq!(GATEWAY_FEE_BPS, 25, "the published rate is 0.25%");
        assert_eq!(split_fee(&BigDecimal::from(250_000)).0, BigDecimal::from(625));
        assert_eq!(split_fee(&BigDecimal::from(250_000)).1, BigDecimal::from(249_375));
        // 0.25% of 250001 is 625.0025 — truncating favours the merchant.
        assert_eq!(split_fee(&BigDecimal::from(250_001)).0, BigDecimal::from(625));
        // 25 USDT in ledger units.
        assert_eq!(split_fee(&BigDecimal::from(2_500_000_000u64)).0, BigDecimal::from(6_250_000));
        // Amounts too small to owe a whole unit of fee owe nothing.
        assert_eq!(split_fee(&BigDecimal::from(1)).0, BigDecimal::from(0));
        assert_eq!(split_fee(&BigDecimal::from(1)).1, BigDecimal::from(1));
    }
}
