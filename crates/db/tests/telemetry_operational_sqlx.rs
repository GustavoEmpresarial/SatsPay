use db::telemetry::{
    record_error, record_http_failure, render_operational_metrics, NewErrorPayload,
};
use sqlx::PgPool;

fn incident() -> NewErrorPayload {
    NewErrorPayload {
        service: "worker".into(),
        level: "ERROR".into(),
        message: "provider unavailable".into(),
        stack_trace: None,
        endpoint: Some("task://worker/doge".into()),
        method: Some("BACKGROUND".into()),
        status_code: None,
        user_id: None,
        ip_address: None,
        request_payload: None,
        user_agent: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_occurrences_share_one_open_group(pool: PgPool) {
    let first = record_error(&pool, incident());
    let second = record_error(&pool, incident());
    let (a, b) = tokio::join!(first, second);
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.id, b.id);
    let groups: i64 =
        sqlx::query_scalar("SELECT count(*) FROM system_error_logs WHERE service = 'worker'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(groups, 1);
    let occurrences: i32 =
        sqlx::query_scalar("SELECT occurrences_count FROM system_error_logs WHERE id = $1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(occurrences, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn metrics_expose_version_and_operational_gauges(pool: PgPool) {
    record_http_failure(&pool, "wallet", "abc123", 503)
        .await
        .unwrap();
    let output = render_operational_metrics(&pool).await.unwrap();
    assert!(output.contains("module=\"wallet\",version=\"abc123\"} 1"));
    assert!(output.contains("satspay_sol_deposit_pool_available 0"));
    assert!(output.contains("satspay_withdrawal_jobs_stalled 0"));
}

#[sqlx::test(migrations = "./migrations")]
async fn resolved_worker_alert_is_closed_once(pool: PgPool) {
    db::telemetry::record_worker_error(
        &pool,
        "CRITICAL",
        "WITHDRAWAL_QUEUE_STALLED",
        "WITHDRAWAL_QUEUE_STALLED",
        None,
    )
    .await;
    assert!(db::telemetry::resolve_worker_alert(&pool, "WITHDRAWAL_QUEUE_STALLED")
        .await
        .unwrap());
    assert!(!db::telemetry::resolve_worker_alert(&pool, "WITHDRAWAL_QUEUE_STALLED")
        .await
        .unwrap());
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_error_logs WHERE endpoint = 'task://worker/WITHDRAWAL_QUEUE_STALLED' AND status = 'OPEN'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(open, 0);
}
