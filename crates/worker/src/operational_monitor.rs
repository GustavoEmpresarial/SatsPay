//! Cheap, read-only checks for failures that otherwise only appear in logs.
use sqlx::PgPool;

async fn report(pool: &PgPool, code: &str, count: i64) {
    if count > 0 {
        tracing::error!(code, count, "operational check failed");
        db::telemetry::record_worker_error(pool, "CRITICAL", code, code, None).await;
    } else if let Err(e) = db::telemetry::resolve_worker_alert(pool, code).await {
        tracing::warn!(code, error = %e, "could not resolve operational alert");
    }
}

pub async fn run_once(pool: &PgPool) -> Result<(), sqlx::Error> {
    let five_xx: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM http_failure_events WHERE occurred_at >= now() - interval '1 minute'",
    )
    .fetch_one(pool)
    .await?;
    report(
        pool,
        "HTTP_5XX_SPIKE",
        if five_xx >= 5 { five_xx } else { 0 },
    )
    .await;
    let empty_sol: i64 = sqlx::query_scalar(
        "SELECT (CASE WHEN EXISTS (SELECT 1 FROM deposit_address_pool WHERE coin = 'SOL' AND claimed_at IS NULL) THEN 0 ELSE 1 END)::bigint",
    ).fetch_one(pool).await?;
    report(pool, "DEPOSIT_ADDRESS_POOL_EMPTY", empty_sol).await;

    let stalled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM internal_jobs WHERE job_type = 'withdrawal_broadcast' AND \
         ((status = 'RUNNING' AND locked_at < now() - interval '5 minutes') OR \
          (status = 'PENDING' AND run_after < now() - interval '5 minutes') OR status = 'FAILED')",
    )
    .fetch_one(pool)
    .await?;
    report(pool, "WITHDRAWAL_QUEUE_STALLED", stalled).await;

    // A credited deposit must have exactly one matching ledger credit. Check
    // amount and wallet as well as presence; the query changes no balances.
    let divergent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM deposits d WHERE d.status = 'CREDITED' AND NOT EXISTS (\
           SELECT 1 FROM ledger_entries l WHERE l.reference_type = 'Deposit' \
           AND l.reference_id = d.id AND l.type = 'DEPOSIT' AND l.wallet_id = d.wallet_id AND l.amount = d.amount)",
    ).fetch_one(pool).await?;
    report(pool, "LEDGER_DEPOSIT_DIVERGENCE", divergent).await;
    let withdrawal_divergent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM withdrawals w WHERE NOT EXISTS (\
           SELECT 1 FROM ledger_entries l WHERE l.reference_type = 'Withdrawal' \
           AND l.reference_id = w.id AND l.type = 'WITHDRAWAL' AND l.wallet_id = w.wallet_id AND l.amount = -w.amount)",
    ).fetch_one(pool).await?;
    report(pool, "LEDGER_WITHDRAWAL_DIVERGENCE", withdrawal_divergent).await;
    sqlx::query("DELETE FROM http_failure_events WHERE occurred_at < now() - interval '7 days'")
        .execute(pool)
        .await?;
    Ok(())
}
