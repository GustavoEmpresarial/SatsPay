//! Full-path coverage for `db::support` (in-app tickets).

mod common;

use db::support::SupportError;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn support_full_lifecycle_and_branches(pool: PgPool) {
    let user = common::insert_user(&pool, "sup-user").await;
    let staff = common::insert_user(&pool, "sup-staff").await;
    let other = common::insert_user(&pool, "sup-other").await;

    // Invalid topic / empty / too long
    assert!(matches!(
        db::support::create_ticket(&pool, user, "nope", "hi").await,
        Err(SupportError::InvalidTopic)
    ));
    assert!(matches!(
        db::support::create_ticket(&pool, user, "deposit", "   ").await,
        Err(SupportError::EmptyMessage)
    ));
    assert!(matches!(
        db::support::create_ticket(&pool, user, "deposit", &"x".repeat(4001)).await,
        Err(SupportError::EmptyMessage)
    ));

    // All topic subjects (including fallback "other")
    for topic in ["deposit", "withdraw", "account", "security", "other"] {
        let d = db::support::create_ticket(&pool, user, topic, &format!("msg-{topic}"))
            .await
            .unwrap();
        assert_eq!(d.ticket.topic, topic);
        assert!(d.ticket.subject.starts_with("[SatsPay]"));
        assert_eq!(d.ticket.status, "OPEN");
        assert_eq!(d.messages.len(), 1);
        assert_eq!(d.messages[0].author_role, "USER");
    }

    let listed = db::support::list_tickets_for_user(&pool, user).await.unwrap();
    assert!(listed.len() >= 5);
    assert!(listed.iter().all(|t| t.user_email.is_none()));

    let ticket_id = listed[0].id;
    let detail = db::support::get_ticket_for_user(&pool, user, ticket_id)
        .await
        .unwrap();
    assert_eq!(detail.ticket.id, ticket_id);

    // IDOR → NotFound
    assert!(matches!(
        db::support::get_ticket_for_user(&pool, other, ticket_id).await,
        Err(SupportError::NotFound)
    ));
    assert!(matches!(
        db::support::get_ticket_for_user(&pool, user, Uuid::new_v4()).await,
        Err(SupportError::NotFound)
    ));

    // User reply → WAITING_STAFF
    let after_user = db::support::add_user_message(&pool, user, ticket_id, "follow-up")
        .await
        .unwrap();
    assert_eq!(after_user.ticket.status, "WAITING_STAFF");
    assert!(after_user.messages.len() >= 2);

    assert!(matches!(
        db::support::add_user_message(&pool, user, ticket_id, "").await,
        Err(SupportError::EmptyMessage)
    ));
    assert!(matches!(
        db::support::add_user_message(&pool, other, ticket_id, "steal").await,
        Err(SupportError::NotFound)
    ));

    // Admin list: all + filter + invalid status
    let all_admin = db::support::list_tickets_admin(&pool, None).await.unwrap();
    assert!(!all_admin.is_empty());
    assert!(all_admin.iter().any(|t| t.user_email.is_some()));

    let filtered = db::support::list_tickets_admin(&pool, Some("WAITING_STAFF"))
        .await
        .unwrap();
    assert!(filtered.iter().any(|t| t.id == ticket_id));

    assert!(matches!(
        db::support::list_tickets_admin(&pool, Some("NOPE")).await,
        Err(SupportError::InvalidStatus)
    ));

    // Admin get + staff reply → WAITING_USER
    let admin_detail = db::support::get_ticket_admin(&pool, ticket_id).await.unwrap();
    assert_eq!(admin_detail.ticket.id, ticket_id);
    assert!(admin_detail.ticket.user_email.is_some());

    assert!(matches!(
        db::support::get_ticket_admin(&pool, Uuid::new_v4()).await,
        Err(SupportError::NotFound)
    ));

    assert!(matches!(
        db::support::add_staff_message(&pool, staff, ticket_id, "  ").await,
        Err(SupportError::EmptyMessage)
    ));
    assert!(matches!(
        db::support::add_staff_message(&pool, staff, Uuid::new_v4(), "hi").await,
        Err(SupportError::NotFound)
    ));

    let after_staff = db::support::add_staff_message(&pool, staff, ticket_id, "we are looking")
        .await
        .unwrap();
    assert_eq!(after_staff.ticket.status, "WAITING_USER");
    assert!(after_staff
        .messages
        .iter()
        .any(|m| m.author_role == "STAFF"));

    // Invalid / missing status updates
    assert!(matches!(
        db::support::set_ticket_status(&pool, ticket_id, "BAD").await,
        Err(SupportError::InvalidStatus)
    ));
    assert!(matches!(
        db::support::set_ticket_status(&pool, Uuid::new_v4(), "CLOSED").await,
        Err(SupportError::NotFound)
    ));

    // Resolve → user cannot reply; staff can still post (status stays RESOLVED)
    let resolved = db::support::set_ticket_status(&pool, ticket_id, "RESOLVED")
        .await
        .unwrap();
    assert_eq!(resolved.ticket.status, "RESOLVED");
    assert!(matches!(
        db::support::add_user_message(&pool, user, ticket_id, "after resolve").await,
        Err(SupportError::Closed)
    ));
    let staff_on_resolved =
        db::support::add_staff_message(&pool, staff, ticket_id, "closing note")
            .await
            .unwrap();
    assert_eq!(staff_on_resolved.ticket.status, "RESOLVED");

    // CLOSED path for user reply
    let closed_ticket = db::support::create_ticket(&pool, user, "account", "close me")
        .await
        .unwrap();
    db::support::set_ticket_status(&pool, closed_ticket.ticket.id, "CLOSED")
        .await
        .unwrap();
    assert!(matches!(
        db::support::add_user_message(&pool, user, closed_ticket.ticket.id, "again").await,
        Err(SupportError::Closed)
    ));

    // Remaining statuses for validate_status coverage via set_ticket_status
    let t2 = db::support::create_ticket(&pool, user, "security", "status sweep")
        .await
        .unwrap();
    for status in ["OPEN", "WAITING_USER", "WAITING_STAFF", "RESOLVED", "CLOSED"] {
        let d = db::support::set_ticket_status(&pool, t2.ticket.id, status)
            .await
            .unwrap();
        assert_eq!(d.ticket.status, status);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn support_db_error_on_closed_pool(pool: PgPool) {
    let user = common::insert_user(&pool, "sup-db-err").await;
    pool.close().await;
    let err = db::support::create_ticket(&pool, user, "deposit", "should fail")
        .await
        .unwrap_err();
    assert!(matches!(err, SupportError::Db(_)));
    assert!(err.to_string().len() > 0);
}
