//! Port of legacy `apps/api/src/jobs/withdrawalReconciler.ts` +
//! `withdrawal.queue.ts`. Drains `withdrawal_broadcast` internal jobs and
//! periodically re-enqueues QUEUED/APPROVED withdrawals that never reached
//! the worker (missed job, worker downtime).

use chain::ChainRegistry;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
struct BroadcastPayload {
    #[serde(rename = "withdrawalId")]
    withdrawal_id: uuid::Uuid,
}

/// Drains all currently-pending `withdrawal_broadcast` jobs.
pub async fn drain_broadcast_queue(pool: &PgPool, registry: &ChainRegistry, locked_by: &str) {
    loop {
        let claimed = match queue::claim_next(pool, "withdrawal_broadcast", locked_by).await {
            Ok(Some(job)) => job,
            Ok(None) => return,
            Err(e) => {
                tracing::error!(error = %e, "withdrawal_reconciler: failed to claim job");
                return;
            }
        };

        let payload: Result<BroadcastPayload, _> = serde_json::from_value(claimed.payload.clone());
        let outcome = match payload {
            Ok(p) => db::withdrawals::process_broadcast(pool, p.withdrawal_id, registry).await.map_err(|e| e.to_string()),
            Err(e) => Err(format!("malformed job payload: {e}")),
        };

        match outcome {
            Ok(()) => {
                if let Err(e) = queue::complete(pool, claimed.id).await {
                    tracing::error!(job_id = %claimed.id, error = %e, "withdrawal_reconciler: failed to mark job complete");
                }
            }
            Err(err) => {
                tracing::error!(job_id = %claimed.id, error = %err, "withdrawal_reconciler: job failed");
                if let Err(e) = queue::fail(pool, claimed.id, &err).await {
                    tracing::error!(job_id = %claimed.id, error = %e, "withdrawal_reconciler: failed to mark job failed");
                }
            }
        }
    }
}

/// Re-enqueues QUEUED/APPROVED withdrawals with no corresponding pending job
/// — recovers from a missed enqueue or worker downtime.
pub async fn requeue_eligible(pool: &PgPool) {
    let ids = match db::withdrawals::list_eligible_for_requeue(pool, 100).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!(error = %e, "withdrawal_reconciler: failed to list eligible withdrawals");
            return;
        }
    };
    for id in ids {
        if let Err(e) = queue::enqueue(pool, "withdrawal_broadcast", &serde_json::json!({ "withdrawalId": id })).await {
            tracing::error!(withdrawal_id = %id, error = %e, "withdrawal_reconciler: failed to requeue");
        }
    }
}
