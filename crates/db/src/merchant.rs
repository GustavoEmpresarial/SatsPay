//! Port 1:1 of legacy `apps/api/src/modules/merchant/{services,repositories}/merchant.*.ts`.
//! Decision emails are sent best-effort by the HTTP layer (`api-http::merchant`)
//! after a successful commit — this module stays free of SMTP/`EmailSender` deps.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum MerchantError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("user not found")]
    NotFound,
    #[error("a merchant application is already pending or approved")]
    AlreadyPending,
    #[error("application is not in a reviewable state")]
    NotReviewable,
}

#[derive(Debug, serde::Serialize)]
pub struct MerchantStatusView {
    pub status: String,
    pub rejection_reason: Option<String>,
    pub business_name: Option<String>,
    pub website: Option<String>,
    pub description: Option<String>,
    pub applied_at: Option<DateTime<Utc>>,
}

pub async fn get_status(pool: &PgPool, user_id: Uuid) -> Result<MerchantStatusView, MerchantError> {
    let row = sqlx::query(
        "SELECT merchant_status::text as status, merchant_rejection_reason, merchant_business_name, \
         merchant_website, merchant_description, merchant_applied_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or(MerchantError::NotFound)?;
    Ok(MerchantStatusView {
        status: row.get("status"),
        rejection_reason: row.get("merchant_rejection_reason"),
        business_name: row.get("merchant_business_name"),
        website: row.get("merchant_website"),
        description: row.get("merchant_description"),
        applied_at: row.get("merchant_applied_at"),
    })
}

/// Apply (or re-apply after rejection). Atomic claim: only succeeds from
/// `NONE`/`REJECTED`, so a second concurrent apply while one is already
/// PENDING/APPROVED is rejected instead of silently overwriting it.
pub async fn apply(pool: &PgPool, user_id: Uuid, business_name: &str, website: &str, description: &str) -> Result<MerchantStatusView, MerchantError> {
    let result = sqlx::query(
        "UPDATE users SET merchant_status = 'PENDING'::merchant_status, merchant_business_name = $2, \
         merchant_website = $3, merchant_description = $4, merchant_applied_at = now(), \
         merchant_rejection_reason = NULL, merchant_reviewed_at = NULL, merchant_reviewed_by_id = NULL \
         WHERE id = $1 AND merchant_status IN ('NONE', 'REJECTED')",
    )
    .bind(user_id)
    .bind(business_name)
    .bind(website)
    .bind(description)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(MerchantError::AlreadyPending);
    }
    get_status(pool, user_id).await
}

#[derive(Debug, serde::Serialize)]
pub struct ApplicationView {
    pub user_id: Uuid,
    pub email: String,
    pub status: String,
    pub business_name: Option<String>,
    pub website: Option<String>,
    pub description: Option<String>,
    pub applied_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
}

pub async fn list_applications(pool: &PgPool, status: Option<&str>) -> Result<Vec<ApplicationView>, MerchantError> {
    let rows = match status {
        Some(s) => {
            sqlx::query(
                "SELECT id, email, merchant_status::text as status, merchant_business_name, merchant_website, \
                 merchant_description, merchant_applied_at, merchant_rejection_reason \
                 FROM users WHERE merchant_status = $1::merchant_status ORDER BY merchant_applied_at ASC",
            )
            .bind(s)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, email, merchant_status::text as status, merchant_business_name, merchant_website, \
                 merchant_description, merchant_applied_at, merchant_rejection_reason \
                 FROM users WHERE merchant_status != 'NONE' ORDER BY merchant_applied_at ASC",
            )
            .fetch_all(pool)
            .await?
        }
    };
    Ok(rows
        .into_iter()
        .map(|r| ApplicationView {
            user_id: r.get("id"),
            email: r.get("email"),
            status: r.get("status"),
            business_name: r.get("merchant_business_name"),
            website: r.get("merchant_website"),
            description: r.get("merchant_description"),
            applied_at: r.get("merchant_applied_at"),
            rejection_reason: r.get("merchant_rejection_reason"),
        })
        .collect())
}

async fn review(pool: &PgPool, user_id: Uuid, admin_id: Uuid, from: &[&str], to: &str, action: &str, rejection_reason: Option<&str>) -> Result<(), MerchantError> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE users SET merchant_status = $2::merchant_status, merchant_reviewed_by_id = $3, \
         merchant_reviewed_at = now(), merchant_rejection_reason = $4 \
         WHERE id = $1 AND merchant_status::text = ANY($5)",
    )
    .bind(user_id)
    .bind(to)
    .bind(admin_id)
    .bind(rejection_reason)
    .bind(from)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(MerchantError::NotReviewable);
    }
    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id) VALUES ($1, $2, $3, $4)")
        .bind(admin_id)
        .bind(action)
        .bind("MerchantApplication")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    // Notification email is sent by the HTTP layer after success (best-effort).
    Ok(())
}

pub async fn approve(pool: &PgPool, user_id: Uuid, admin_id: Uuid) -> Result<(), MerchantError> {
    review(pool, user_id, admin_id, &["PENDING"], "APPROVED", "MERCHANT_APPROVE", None).await
}

pub async fn reject(pool: &PgPool, user_id: Uuid, admin_id: Uuid, reason: &str) -> Result<(), MerchantError> {
    review(pool, user_id, admin_id, &["PENDING", "APPROVED"], "REJECTED", "MERCHANT_REJECT", Some(reason)).await
}
