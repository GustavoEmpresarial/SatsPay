//! Telemetry error log + overview against real Postgres.

use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn record_list_resolve_and_overview(pool: PgPool) {
    let payload = db::telemetry::NewErrorPayload {
        service: "api-server".into(),
        level: "ERROR".into(),
        message: "boom line\nstack".into(),
        stack_trace: Some("trace".into()),
        endpoint: Some("/v1/x".into()),
        method: Some("GET".into()),
        status_code: Some(500),
        user_id: None,
        ip_address: Some("203.0.113.99".into()),
        request_payload: None,
        user_agent: Some("test".into()),
    };
    let payload2 = db::telemetry::NewErrorPayload {
        service: "api-server".into(),
        level: "ERROR".into(),
        message: "boom line\nother".into(),
        stack_trace: Some("trace".into()),
        endpoint: Some("/v1/x".into()),
        method: Some("GET".into()),
        status_code: Some(500),
        user_id: None,
        ip_address: None,
        request_payload: None,
        user_agent: None,
    };
    assert_eq!(
        db::telemetry::compute_fingerprint(&payload),
        db::telemetry::compute_fingerprint(&payload2)
    );
    let id1 = db::telemetry::record_error(&pool, payload).await.unwrap();
    let id2 = db::telemetry::record_error(&pool, payload2).await.unwrap();
    assert_eq!(id1.id, id2.id);
    assert!(id1.is_new);
    assert!(!id2.is_new);
    assert_eq!(id2.occurrences, 2);

    // Different stack frame → new group
    let id3 = db::telemetry::record_error(
        &pool,
        db::telemetry::NewErrorPayload {
            service: "api-server".into(),
            level: "ERROR".into(),
            message: "boom line\nother".into(),
            stack_trace: Some("at other.rs:99".into()),
            endpoint: Some("/v1/x".into()),
            method: Some("GET".into()),
            status_code: Some(500),
            user_id: None,
            ip_address: None,
            request_payload: None,
            user_agent: None,
        },
    )
    .await
    .unwrap();
    assert_ne!(id1.id, id3.id);
    assert!(id3.is_new);

    let listed = db::telemetry::list_errors(&pool, None, None, None, None, 50)
        .await
        .unwrap();
    assert!(!listed.is_empty());

    db::telemetry::resolve_error(&pool, id1.id).await.unwrap();
    db::telemetry::ignore_error(
        &pool,
        db::telemetry::record_error(
            &pool,
            db::telemetry::NewErrorPayload {
                service: "worker".into(),
                level: "WARN".into(),
                message: "unique-warn".into(),
                stack_trace: None,
                endpoint: None,
                method: None,
                status_code: None,
                user_id: None,
                ip_address: None,
                request_payload: None,
                user_agent: None,
            },
        )
        .await
        .unwrap()
        .id,
    )
    .await
    .unwrap();

    let n = db::telemetry::batch_resolve_errors(&pool, &[Uuid::new_v4()]).await.unwrap();
    assert_eq!(n, 0);
    let _ = db::telemetry::resolve_all_open_errors(&pool).await.unwrap();
    let _ = db::telemetry::clear_resolved_errors(&pool).await.unwrap();

    let overview = db::telemetry::get_telemetry_overview(&pool).await.unwrap();
    assert!(overview.total_users >= 0);

    let snap = db::telemetry::capture_metrics_snapshot(&pool).await.unwrap();
    assert_ne!(snap, Uuid::nil());
    let hist = db::telemetry::get_metrics_history(&pool, 24).await.unwrap();
    assert!(!hist.is_empty());

    db::telemetry::record_worker_error(&pool, "ERROR", "job", "fail", None).await;
    db::telemetry::record_security_alert(&pool, "brute", "ERROR", "msg", None, None, None).await;

    let _ = db::telemetry::list_errors(
        &pool,
        Some("OPEN"),
        Some("api-server"),
        Some("ERROR"),
        Some("/v1"),
        10,
    )
    .await
    .unwrap();
}
