//! `outbox_relay`: polls pending `outbox_events` rows and publishes them to
//! Kafka, marking `published_at` only after the broker ACKs. At-least-once
//! delivery — if the process crashes between publish and the `UPDATE`, the
//! row is republished on the next poll, so Kafka consumers of these topics
//! must dedupe by `event_id` (the outbox row id, sent as the message key's
//! sibling header — see `producer::EventProducer::publish`).

use crate::domain::DomainEvent;
use crate::producer::EventProducer;
use crate::topics::topic_for;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Producer(#[from] crate::producer::ProducerError),
    #[error(transparent)]
    Deserialize(#[from] serde_json::Error),
}

struct PendingRow {
    id: Uuid,
    aggregate_id: Uuid,
    payload: serde_json::Value,
}

/// Runs one poll-and-publish batch. Intended to be called in a loop from the
/// `worker` binary's `outbox_relay` task, on a short interval (e.g. 500ms).
/// Returns the number of events published.
pub async fn relay_once(pool: &PgPool, producer: &EventProducer, batch_size: i64) -> Result<usize, RelayError> {
    let mut tx = pool.begin().await?;

    let rows = sqlx::query(
        r#"
        SELECT id, aggregate_id, payload
        FROM outbox_events
        WHERE published_at IS NULL
        ORDER BY created_at
        LIMIT $1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(batch_size)
    .fetch_all(&mut *tx)
    .await?;

    let pending: Vec<PendingRow> = rows
        .into_iter()
        .map(|r| PendingRow {
            id: r.get("id"),
            aggregate_id: r.get("aggregate_id"),
            payload: r.get("payload"),
        })
        .collect();

    let mut published = 0usize;
    for row in &pending {
        let event: DomainEvent = serde_json::from_value(row.payload.clone())?;
        let topic = topic_for(&event);
        // Envelope carries `event_id` (the outbox row id) alongside the raw
        // event payload, so consumers can dedupe at-least-once redelivery by
        // that id instead of guessing identity from the payload shape.
        let envelope = serde_json::json!({ "event_id": row.id, "event": row.payload });
        let payload_bytes = serde_json::to_vec(&envelope)?;

        producer.publish(topic, &row.aggregate_id.to_string(), &payload_bytes).await?;

        sqlx::query("UPDATE outbox_events SET published_at = now() WHERE id = $1")
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        published += 1;
    }

    tx.commit().await?;
    Ok(published)
}
