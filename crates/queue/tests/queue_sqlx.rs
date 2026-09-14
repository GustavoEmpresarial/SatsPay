//! Enqueue → claim → complete / fail against real Postgres.

use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "../db/migrations")]
async fn enqueue_claim_complete(pool: PgPool) {
    let id = queue::enqueue(&pool, "TEST_JOB", &json!({"n": 1}))
        .await
        .unwrap();
    let job = queue::claim_next(&pool, "TEST_JOB", "worker-a")
        .await
        .unwrap()
        .expect("claimed");
    assert_eq!(job.id, id);
    assert_eq!(job.job_type, "TEST_JOB");
    assert_eq!(job.payload["n"], 1);
    assert_eq!(job.attempts, 1);
    assert!(queue::claim_next(&pool, "TEST_JOB", "worker-b")
        .await
        .unwrap()
        .is_none());
    queue::complete(&pool, id).await.unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn fail_requeues_then_parks(pool: PgPool) {
    let id = queue::enqueue(&pool, "FAIL_JOB", &json!({}))
        .await
        .unwrap();
    let job = queue::claim_next(&pool, "FAIL_JOB", "w")
        .await
        .unwrap()
        .unwrap();
    queue::fail(&pool, job.id, "boom").await.unwrap();
    assert!(queue::claim_next(&pool, "FAIL_JOB", "w")
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE internal_jobs SET run_after = now() - interval '1 second' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let again = queue::claim_next(&pool, "FAIL_JOB", "w")
        .await
        .unwrap()
        .expect("requeued");
    assert!(again.attempts >= 2);
}
