//! In-app support tickets (user ↔ staff). No email.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SupportError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("invalid topic")]
    InvalidTopic,
    #[error("empty message")]
    EmptyMessage,
    #[error("ticket not found")]
    NotFound,
    #[error("ticket closed")]
    Closed,
    #[error("invalid status")]
    InvalidStatus,
}

const TOPICS: &[&str] = &["deposit", "withdraw", "account", "security", "other"];
const STATUSES: &[&str] = &["OPEN", "WAITING_USER", "WAITING_STAFF", "RESOLVED", "CLOSED"];

fn validate_topic(topic: &str) -> Result<(), SupportError> {
    if TOPICS.contains(&topic) {
        Ok(())
    } else {
        Err(SupportError::InvalidTopic)
    }
}

fn validate_status(status: &str) -> Result<(), SupportError> {
    if STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(SupportError::InvalidStatus)
    }
}

fn topic_subject(topic: &str) -> &'static str {
    match topic {
        "deposit" => "Depósito",
        "withdraw" => "Saque",
        "account" => "Conta",
        "security" => "Segurança",
        _ => "Outro",
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct SupportTicketSummary {
    pub id: Uuid,
    pub topic: String,
    pub subject: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: i64,
    pub user_email: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SupportMessage {
    pub id: Uuid,
    pub author_role: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SupportTicketDetail {
    pub ticket: SupportTicketSummary,
    pub messages: Vec<SupportMessage>,
}

pub async fn create_ticket(
    pool: &PgPool,
    user_id: Uuid,
    topic: &str,
    body: &str,
) -> Result<SupportTicketDetail, SupportError> {
    validate_topic(topic)?;
    let body = body.trim();
    if body.is_empty() || body.len() > 4000 {
        return Err(SupportError::EmptyMessage);
    }
    let subject = format!("[SatsPay] {}", topic_subject(topic));

    let mut tx = pool.begin().await?;
    let ticket_id: Uuid = sqlx::query_scalar(
        "INSERT INTO support_tickets (user_id, topic, subject, status) \
         VALUES ($1, $2, $3, 'OPEN') RETURNING id",
    )
    .bind(user_id)
    .bind(topic)
    .bind(&subject)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO support_messages (ticket_id, author_id, author_role, body) \
         VALUES ($1, $2, 'USER', $3)",
    )
    .bind(ticket_id)
    .bind(user_id)
    .bind(body)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    get_ticket_for_user(pool, user_id, ticket_id).await
}

pub async fn list_tickets_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<SupportTicketSummary>, SupportError> {
    let rows = sqlx::query(
        "SELECT t.id, t.topic, t.subject, t.status::text as status, t.created_at, t.updated_at, \
                (SELECT COUNT(*) FROM support_messages m WHERE m.ticket_id = t.id)::bigint as message_count \
         FROM support_tickets t \
         WHERE t.user_id = $1 \
         ORDER BY t.updated_at DESC \
         LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SupportTicketSummary {
            id: r.get("id"),
            topic: r.get("topic"),
            subject: r.get("subject"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            message_count: r.get("message_count"),
            user_email: None,
        })
        .collect())
}

pub async fn get_ticket_for_user(
    pool: &PgPool,
    user_id: Uuid,
    ticket_id: Uuid,
) -> Result<SupportTicketDetail, SupportError> {
    let row = sqlx::query(
        "SELECT t.id, t.topic, t.subject, t.status::text as status, t.created_at, t.updated_at, \
                (SELECT COUNT(*) FROM support_messages m WHERE m.ticket_id = t.id)::bigint as message_count \
         FROM support_tickets t \
         WHERE t.id = $1 AND t.user_id = $2",
    )
    .bind(ticket_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(SupportError::NotFound)?;

    let ticket = SupportTicketSummary {
        id: row.get("id"),
        topic: row.get("topic"),
        subject: row.get("subject"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        message_count: row.get("message_count"),
        user_email: None,
    };
    let messages = list_messages(pool, ticket_id).await?;
    Ok(SupportTicketDetail { ticket, messages })
}

async fn list_messages(pool: &PgPool, ticket_id: Uuid) -> Result<Vec<SupportMessage>, SupportError> {
    let rows = sqlx::query(
        "SELECT id, author_role::text as author_role, body, created_at \
         FROM support_messages WHERE ticket_id = $1 ORDER BY created_at ASC",
    )
    .bind(ticket_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| SupportMessage {
            id: r.get("id"),
            author_role: r.get("author_role"),
            body: r.get("body"),
            created_at: r.get("created_at"),
        })
        .collect())
}

pub async fn add_user_message(
    pool: &PgPool,
    user_id: Uuid,
    ticket_id: Uuid,
    body: &str,
) -> Result<SupportTicketDetail, SupportError> {
    let body = body.trim();
    if body.is_empty() || body.len() > 4000 {
        return Err(SupportError::EmptyMessage);
    }
    let status: String = sqlx::query_scalar(
        "SELECT status::text FROM support_tickets WHERE id = $1 AND user_id = $2",
    )
    .bind(ticket_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(SupportError::NotFound)?;

    if status == "CLOSED" || status == "RESOLVED" {
        return Err(SupportError::Closed);
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO support_messages (ticket_id, author_id, author_role, body) \
         VALUES ($1, $2, 'USER', $3)",
    )
    .bind(ticket_id)
    .bind(user_id)
    .bind(body)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE support_tickets SET status = 'WAITING_STAFF', updated_at = now() WHERE id = $1",
    )
    .bind(ticket_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    get_ticket_for_user(pool, user_id, ticket_id).await
}

pub async fn list_tickets_admin(
    pool: &PgPool,
    status_filter: Option<&str>,
) -> Result<Vec<SupportTicketSummary>, SupportError> {
    if let Some(s) = status_filter {
        validate_status(s)?;
    }
    let rows = if let Some(status) = status_filter {
        sqlx::query(
            "SELECT t.id, t.topic, t.subject, t.status::text as status, t.created_at, t.updated_at, \
                    u.email as user_email, \
                    (SELECT COUNT(*) FROM support_messages m WHERE m.ticket_id = t.id)::bigint as message_count \
             FROM support_tickets t \
             JOIN users u ON u.id = t.user_id \
             WHERE t.status = $1::support_ticket_status \
             ORDER BY t.updated_at DESC \
             LIMIT 100",
        )
        .bind(status)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT t.id, t.topic, t.subject, t.status::text as status, t.created_at, t.updated_at, \
                    u.email as user_email, \
                    (SELECT COUNT(*) FROM support_messages m WHERE m.ticket_id = t.id)::bigint as message_count \
             FROM support_tickets t \
             JOIN users u ON u.id = t.user_id \
             ORDER BY t.updated_at DESC \
             LIMIT 100",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|r| SupportTicketSummary {
            id: r.get("id"),
            topic: r.get("topic"),
            subject: r.get("subject"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            message_count: r.get("message_count"),
            user_email: Some(r.get("user_email")),
        })
        .collect())
}

pub async fn get_ticket_admin(
    pool: &PgPool,
    ticket_id: Uuid,
) -> Result<SupportTicketDetail, SupportError> {
    let row = sqlx::query(
        "SELECT t.id, t.topic, t.subject, t.status::text as status, t.created_at, t.updated_at, \
                u.email as user_email, \
                (SELECT COUNT(*) FROM support_messages m WHERE m.ticket_id = t.id)::bigint as message_count \
         FROM support_tickets t \
         JOIN users u ON u.id = t.user_id \
         WHERE t.id = $1",
    )
    .bind(ticket_id)
    .fetch_optional(pool)
    .await?
    .ok_or(SupportError::NotFound)?;

    let ticket = SupportTicketSummary {
        id: row.get("id"),
        topic: row.get("topic"),
        subject: row.get("subject"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        message_count: row.get("message_count"),
        user_email: Some(row.get("user_email")),
    };
    let messages = list_messages(pool, ticket_id).await?;
    Ok(SupportTicketDetail { ticket, messages })
}

pub async fn add_staff_message(
    pool: &PgPool,
    staff_id: Uuid,
    ticket_id: Uuid,
    body: &str,
) -> Result<SupportTicketDetail, SupportError> {
    let body = body.trim();
    if body.is_empty() || body.len() > 4000 {
        return Err(SupportError::EmptyMessage);
    }
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM support_tickets WHERE id = $1")
            .bind(ticket_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Err(SupportError::NotFound);
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO support_messages (ticket_id, author_id, author_role, body) \
         VALUES ($1, $2, 'STAFF', $3)",
    )
    .bind(ticket_id)
    .bind(staff_id)
    .bind(body)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE support_tickets SET status = 'WAITING_USER', updated_at = now() WHERE id = $1 \
         AND status NOT IN ('CLOSED', 'RESOLVED')",
    )
    .bind(ticket_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    get_ticket_admin(pool, ticket_id).await
}

pub async fn set_ticket_status(
    pool: &PgPool,
    ticket_id: Uuid,
    status: &str,
) -> Result<SupportTicketDetail, SupportError> {
    validate_status(status)?;
    let n = sqlx::query(
        "UPDATE support_tickets SET status = $2::support_ticket_status, updated_at = now() \
         WHERE id = $1",
    )
    .bind(ticket_id)
    .bind(status)
    .execute(pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(SupportError::NotFound);
    }
    get_ticket_admin(pool, ticket_id).await
}
