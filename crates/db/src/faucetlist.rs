//! Port of legacy `apps/api/src/modules/faucetlist/{services,repositories}/faucetlist.*.ts`.

use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum FaucetlistError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("faucet site not found")]
    NotFound,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FaucetSitePublic {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub description: String,
    pub coins: Vec<String>,
    #[serde(rename = "rewardInfo")]
    pub reward_info: Option<String>,
    pub status: String,
    #[serde(rename = "rejectionReason")]
    pub rejection_reason: Option<String>,
    pub clicks: i32,
}

/// Public faucetlist — approved sites only, most-clicked first.
pub async fn list_approved(pool: &PgPool) -> Result<Vec<FaucetSitePublic>, FaucetlistError> {
    let rows = sqlx::query(
        "SELECT id, name, url, description, coins::text[] as coins, reward_info, status::text as status, rejection_reason, clicks \
         FROM faucet_sites WHERE status = 'APPROVED' ORDER BY clicks DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| FaucetSitePublic {
            id: r.get("id"),
            name: r.get("name"),
            url: r.get("url"),
            description: r.get("description"),
            coins: r.get("coins"),
            reward_info: r.get("reward_info"),
            status: r.get("status"),
            rejection_reason: r.get("rejection_reason"),
            clicks: r.get("clicks"),
        })
        .collect())
}

/// Registers a click and returns the destination URL — only for approved sites.
pub async fn register_click(pool: &PgPool, id: Uuid) -> Result<String, FaucetlistError> {
    let url: Option<String> = sqlx::query_scalar("SELECT url FROM faucet_sites WHERE id = $1 AND status = 'APPROVED'").bind(id).fetch_optional(pool).await?;
    let url = url.ok_or(FaucetlistError::NotFound)?;
    sqlx::query("UPDATE faucet_sites SET clicks = clicks + 1 WHERE id = $1").bind(id).execute(pool).await?;
    Ok(url)
}

pub async fn list_mine(pool: &PgPool, owner_id: Uuid) -> Result<Vec<FaucetSitePublic>, FaucetlistError> {
    let rows = sqlx::query(
        "SELECT id, name, url, description, coins::text[] as coins, reward_info, status::text as status, rejection_reason, clicks \
         FROM faucet_sites WHERE owner_id = $1 ORDER BY created_at DESC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| FaucetSitePublic {
            id: r.get("id"),
            name: r.get("name"),
            url: r.get("url"),
            description: r.get("description"),
            coins: r.get("coins"),
            reward_info: r.get("reward_info"),
            status: r.get("status"),
            rejection_reason: r.get("rejection_reason"),
            clicks: r.get("clicks"),
        })
        .collect())
}

/// New sites are automatically created as APPROVED and listed immediately.
pub async fn create_site(pool: &PgPool, owner_id: Uuid, name: &str, url: &str, description: &str, coins: &[&str], reward_info: Option<&str>) -> Result<Uuid, FaucetlistError> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO faucet_sites (owner_id, name, url, description, coins, reward_info, status) \
         VALUES ($1, $2, $3, $4, $5::coin[], $6, 'APPROVED') RETURNING id",
    )
    .bind(owner_id)
    .bind(name)
    .bind(url)
    .bind(description)
    .bind(coins)
    .bind(reward_info)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn delete_site(pool: &PgPool, owner_id: Uuid, id: Uuid) -> Result<(), FaucetlistError> {
    let res = sqlx::query("DELETE FROM faucet_sites WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(owner_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(FaucetlistError::NotFound);
    }
    Ok(())
}
