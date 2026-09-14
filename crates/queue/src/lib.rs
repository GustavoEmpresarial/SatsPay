//! Internal job queue over the Postgres `internal_jobs` table (migration
//! `0003_internal_jobs.sql`). Same `SELECT ... FOR UPDATE SKIP LOCKED`
//! pattern as the ledger — deliberately not Redis/BullMQ, so job state has a
//! single source of truth. See
//! `docs/decisions/internal-job-queue-postgres-skip-locked.md`.
//!
//! Reconstructed from its call sites (`worker::withdrawal_reconciler`,
//! `api-http::withdrawals`/`admin`) and the table DDL. The workspace `sqlx`
//! has no `json` feature, so the `jsonb` payload is round-tripped as text
//! (`$n::jsonb` on write, `payload::text` on read).

use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("malformed job payload json: {0}")]
    Payload(#[from] serde_json::Error),
}

/// A claimed job. `payload` is the JSON the producer enqueued.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: Uuid,
    pub job_type: String,
    pub payload: Value,
    pub attempts: i32,
}

/// Enqueues a `PENDING` job, runnable immediately. Returns its id.
pub async fn enqueue(pool: &PgPool, job_type: &str, payload: &Value) -> Result<Uuid, QueueError> {
    let row = sqlx::query("INSERT INTO internal_jobs (job_type, payload) VALUES ($1, $2::jsonb) RETURNING id")
        .bind(job_type)
        .bind(payload.to_string())
        .fetch_one(pool)
        .await?;
    Ok(row.get::<Uuid, _>("id"))
}

/// Atomically claims the oldest runnable job of `job_type`, flipping it to
/// `RUNNING`, bumping `attempts` and stamping `locked_by`/`locked_at`.
/// `FOR UPDATE SKIP LOCKED` guarantees two workers never claim the same row.
/// Returns `None` when nothing is runnable.
pub async fn claim_next(pool: &PgPool, job_type: &str, locked_by: &str) -> Result<Option<Job>, QueueError> {
    let row = sqlx::query(
        r#"
        UPDATE internal_jobs
        SET status = 'RUNNING',
            attempts = attempts + 1,
            locked_by = $2,
            locked_at = now(),
            updated_at = now()
        WHERE id = (
            SELECT id FROM internal_jobs
            WHERE job_type = $1
              AND status = 'PENDING'
              AND run_after <= now()
            ORDER BY run_after ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING id, job_type, payload::text AS payload, attempts
        "#,
    )
    .bind(job_type)
    .bind(locked_by)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(None),
        Some(r) => {
            let payload_text: String = r.get("payload");
            Ok(Some(Job {
                id: r.get("id"),
                job_type: r.get("job_type"),
                payload: serde_json::from_str(&payload_text)?,
                attempts: r.get("attempts"),
            }))
        }
    }
}

/// Marks a claimed job `DONE`.
pub async fn complete(pool: &PgPool, job_id: Uuid) -> Result<(), QueueError> {
    sqlx::query("UPDATE internal_jobs SET status = 'DONE', updated_at = now() WHERE id = $1")
        .bind(job_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Records a failed attempt. Re-queues the job (`PENDING`, backed off by
/// `30s * attempts`) while attempts remain; otherwise parks it `FAILED`.
/// Clears the lock either way.
pub async fn fail(pool: &PgPool, job_id: Uuid, error: &str) -> Result<(), QueueError> {
    sqlx::query(
        r#"
        UPDATE internal_jobs
        SET status = CASE
                WHEN attempts >= max_attempts THEN 'FAILED'::internal_job_status
                ELSE 'PENDING'::internal_job_status
            END,
            run_after = now() + make_interval(secs => 30 * attempts),
            last_error = $2,
            locked_by = NULL,
            locked_at = NULL,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}
