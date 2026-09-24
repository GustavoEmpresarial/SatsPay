#[path = "../src/operational_monitor.rs"]
mod operational_monitor;

use sqlx::PgPool;

#[sqlx::test(migrations = "../db/migrations")]
async fn spike_and_empty_pool_alert_then_resolve(pool: PgPool) {
    for _ in 0..5 {
        db::telemetry::record_http_failure(&pool, "wallet", "test", 500)
            .await
            .unwrap();
    }
    operational_monitor::run_once(&pool).await.unwrap();
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_error_logs WHERE service = 'worker' AND status = 'OPEN' AND endpoint IN \
         ('task://worker/HTTP_5XX_SPIKE', 'task://worker/DEPOSIT_ADDRESS_POOL_EMPTY')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 2);

    sqlx::query("DELETE FROM http_failure_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO deposit_address_pool (coin, hd_index, address) VALUES ('SOL', 1, 'sol-test-address')")
        .execute(&pool)
        .await
        .unwrap();
    operational_monitor::run_once(&pool).await.unwrap();
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM system_error_logs WHERE service = 'worker' AND status = 'OPEN'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 0);
}
