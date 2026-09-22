//! Extra db::support edges for 100% region coverage.

mod common;

use db::support::SupportError;
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn support_topic_subjects_and_staff_on_closed(pool: PgPool) {
    let user = common::insert_user(&pool, "sup2-u").await;
    let staff = common::insert_user(&pool, "sup2-s").await;

    // whitespace-only after trim on staff
    let t = db::support::create_ticket(&pool, None, user, "other", "hello other")
        .await
        .unwrap();
    assert!(t.ticket.subject.contains("Outro"));

    db::support::set_ticket_status(&pool, None, t.ticket.id, "CLOSED")
        .await
        .unwrap();

    // staff can still append note on CLOSED (status stays CLOSED)
    let after = db::support::add_staff_message(&pool, None, staff, t.ticket.id, "nota final")
        .await
        .unwrap();
    assert_eq!(after.ticket.status, "CLOSED");
    assert!(after.messages.iter().any(|m| m.author_role == "STAFF"));

    // list empty for stranger
    let empty = db::support::list_tickets_for_user(&pool, None, staff).await.unwrap();
    assert!(empty.is_empty() || empty.iter().all(|x| x.id != t.ticket.id));

    // Display all error variants (Display coverage)
    let errs: Vec<String> = vec![
        SupportError::InvalidTopic.to_string(),
        SupportError::EmptyMessage.to_string(),
        SupportError::NotFound.to_string(),
        SupportError::Closed.to_string(),
        SupportError::InvalidStatus.to_string(),
        SupportError::Db(sqlx::Error::RowNotFound).to_string(),
    ];
    assert!(errs.iter().all(|s| !s.is_empty()));
}
