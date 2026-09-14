//! Audit log persistence and query models for security, login, registration, and user activities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub entity: String,
    pub entity_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Records a security/audit log event asynchronously.
pub async fn record_log(
    pool: &PgPool,
    user_id: Option<Uuid>,
    action: &str,
    entity: &str,
    entity_id: Option<Uuid>,
    ip: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<Uuid, AuditError> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO audit_logs (user_id, action, entity, entity_id, ip, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(user_id)
    .bind(action)
    .bind(entity)
    .bind(entity_id)
    .bind(ip)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Convenience helper: spawn background task to record audit log without blocking request latency.
pub fn record_log_spawned(
    pool: PgPool,
    user_id: Option<Uuid>,
    action: String,
    entity: String,
    entity_id: Option<Uuid>,
    ip: Option<String>,
    metadata: Option<serde_json::Value>,
) {
    tokio::spawn(async move {
        let _ = record_log(
            &pool,
            user_id,
            &action,
            &entity,
            entity_id,
            ip.as_deref(),
            metadata,
        )
        .await;
    });
}

/// Lists recent audit logs for a specific user (security log in frontend).
pub async fn list_user_logs(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<AuditLogEntry>, AuditError> {
    let rows = sqlx::query(
        "SELECT id, user_id, action, entity, entity_id, metadata, ip, created_at \
         FROM audit_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let entries = rows
        .into_iter()
        .map(|r| AuditLogEntry {
            id: r.get("id"),
            user_id: r.get("user_id"),
            action: r.get("action"),
            entity: r.get("entity"),
            entity_id: r.get("entity_id"),
            metadata: r.get("metadata"),
            ip: r.get("ip"),
            created_at: r.get("created_at"),
        })
        .collect();

    Ok(entries)
}

/// Lists global audit logs for admin overview.
pub async fn list_recent_logs(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<AuditLogEntry>, AuditError> {
    let rows = sqlx::query(
        "SELECT id, user_id, action, entity, entity_id, metadata, ip, created_at \
         FROM audit_logs ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let entries = rows
        .into_iter()
        .map(|r| AuditLogEntry {
            id: r.get("id"),
            user_id: r.get("user_id"),
            action: r.get("action"),
            entity: r.get("entity"),
            entity_id: r.get("entity_id"),
            metadata: r.get("metadata"),
            ip: r.get("ip"),
            created_at: r.get("created_at"),
        })
        .collect();

    Ok(entries)
}
