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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub tx_hash: Option<String>,
    pub confirmations: i32,
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
    pub callback_url: String,
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub description: Option<String>,
    pub expiry_minutes: Option<i64>,
}

pub async fn create_invoice(
    pool: &PgPool,
    input: CreateDepositInvoiceInput,
) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let expiry_mins = input.expiry_minutes.unwrap_or(60).max(5).min(1440);
    let expires_at = Utc::now() + chrono::Duration::minutes(expiry_mins);
    
    // Default gateway fee: 0.5% (can be 0 or customizable)
    let fee_pct = BigDecimal::from(5) / BigDecimal::from(1000); // 0.005
    let fee_amount = &input.amount * &fee_pct;
    let net_amount = &input.amount - &fee_amount;

    let row = sqlx::query(
        "INSERT INTO merchant_deposit_invoices (
            merchant_id, api_key_id, site_user_id, order_id, site_name,
            coin, amount, fee_amount, net_amount, deposit_address,
            callback_url, success_url, cancel_url, customer_email, customer_name,
            description, status, expires_at
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6::coin, $7, $8, $9, $10,
            $11, $12, $13, $14, $15,
            $16, 'PENDING'::deposit_invoice_status, $17
        ) RETURNING 
            id, merchant_id, api_key_id, site_user_id, order_id, site_name,
            coin::text as coin, amount, fee_amount, net_amount, deposit_address,
            tx_hash, confirmations, callback_url, success_url, cancel_url,
            customer_email, customer_name, description, status::text as status,
            expires_at, paid_at, webhook_delivered, webhook_status_code,
            webhook_attempts, webhook_last_error, webhook_last_attempt_at, created_at"
    )
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
    .fetch_one(pool)
    .await?;

    Ok(MerchantDepositInvoice {
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
        tx_hash: row.get("tx_hash"),
        confirmations: row.get("confirmations"),
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
        created_at: row.get("created_at"),
    })
}

pub async fn get_invoice_by_id(pool: &PgPool, id: Uuid) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let row = sqlx::query(
        "SELECT 
            id, merchant_id, api_key_id, site_user_id, order_id, site_name,
            coin::text as coin, amount, fee_amount, net_amount, deposit_address,
            tx_hash, confirmations, callback_url, success_url, cancel_url,
            customer_email, customer_name, description, status::text as status,
            expires_at, paid_at, webhook_delivered, webhook_status_code,
            webhook_attempts, webhook_last_error, webhook_last_attempt_at, created_at
        FROM merchant_deposit_invoices WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or(MerchantDepositError::NotFound)?;
    Ok(MerchantDepositInvoice {
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
        tx_hash: row.get("tx_hash"),
        confirmations: row.get("confirmations"),
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
        created_at: row.get("created_at"),
    })
}

pub async fn list_invoices_by_merchant(
    pool: &PgPool,
    merchant_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<MerchantDepositInvoice>, MerchantDepositError> {
    let rows = sqlx::query(
        "SELECT 
            id, merchant_id, api_key_id, site_user_id, order_id, site_name,
            coin::text as coin, amount, fee_amount, net_amount, deposit_address,
            tx_hash, confirmations, callback_url, success_url, cancel_url,
            customer_email, customer_name, description, status::text as status,
            expires_at, paid_at, webhook_delivered, webhook_status_code,
            webhook_attempts, webhook_last_error, webhook_last_attempt_at, created_at
        FROM merchant_deposit_invoices 
        WHERE merchant_id = $1 
        ORDER BY created_at DESC 
        LIMIT $2 OFFSET $3"
    )
    .bind(merchant_id)
    .bind(limit.max(1).min(100))
    .bind(offset.max(0))
    .fetch_all(pool)
    .await?;

    let list = rows.into_iter().map(|row| MerchantDepositInvoice {
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
        tx_hash: row.get("tx_hash"),
        confirmations: row.get("confirmations"),
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
        created_at: row.get("created_at"),
    }).collect();

    Ok(list)
}

/// Confirm an invoice: updates status to CONFIRMED and credits the merchant's MERCHANT wallet in ledger.
pub async fn confirm_invoice(
    pool: &PgPool,
    invoice_id: Uuid,
    tx_hash: Option<&str>,
) -> Result<MerchantDepositInvoice, MerchantDepositError> {
    let mut tx = pool.begin().await?;

    let inv = get_invoice_by_id(pool, invoice_id).await?;
    if inv.status == "CONFIRMED" {
        return Ok(inv);
    }
    if inv.status == "CANCELLED" || inv.status == "EXPIRED" {
        return Err(MerchantDepositError::InvalidStatus);
    }

    let _coin: shared::Coin = inv.coin.parse().map_err(|_| MerchantDepositError::NotFound)?;

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
    let ref_key = format!("merchant_dep:{}:{}", inv.id, tx_hash.unwrap_or("internal"));
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

    let inv = get_invoice_by_id(pool, invoice_id).await?;
    if inv.status != "PENDING" && inv.status != "DETECTED" {
        return Err(MerchantDepositError::InvalidStatus);
    }
    if inv.expires_at < Utc::now() {
        return Err(MerchantDepositError::InvalidStatus);
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
    let credit_ref = format!("merchant_dep:{}:internal", inv.id);
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
