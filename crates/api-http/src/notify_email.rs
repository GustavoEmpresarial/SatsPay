//! Best-effort notification emails from the HTTP layer.
//!
//! DB modules stay free of SMTP/`EmailSender` deps; handlers call these
//! helpers after a successful commit and never fail the HTTP success path
//! if delivery fails.

use domain::auth::EmailSender;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn user_email(pool: &PgPool, user_id: Uuid) -> Option<String> {
    match sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1").bind(user_id).fetch_optional(pool).await {
        Ok(email) => email,
        Err(e) => {
            tracing::warn!(%user_id, error = %e, "failed to look up user email for notification");
            None
        }
    }
}

pub async fn faucet_site_owner_email(pool: &PgPool, site_id: Uuid) -> Option<String> {
    match sqlx::query_scalar::<_, String>(
        "SELECT u.email FROM faucet_sites fs JOIN users u ON u.id = fs.owner_id WHERE fs.id = $1",
    )
    .bind(site_id)
    .fetch_optional(pool)
    .await
    {
        Ok(email) => email,
        Err(e) => {
            tracing::warn!(%site_id, error = %e, "failed to look up faucet site owner email for notification");
            None
        }
    }
}

pub async fn send_best_effort(sender: &dyn EmailSender, to: &str, subject: &str, body: &str) {
    if let Err(e) = sender.send(to, subject, body).await {
        tracing::warn!(to, subject, error = %e, "best-effort notification email failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::auth::NoopEmailSender;

    struct FailEmailSender;

    #[async_trait]
    impl EmailSender for FailEmailSender {
        async fn send(&self, _to: &str, _subject: &str, _body: &str) -> Result<(), String> {
            Err("smtp down".into())
        }
    }

    #[tokio::test]
    async fn send_best_effort_swallows_errors() {
        send_best_effort(&FailEmailSender, "a@b.c", "subj", "body").await;
        send_best_effort(&NoopEmailSender, "a@b.c", "subj", "body").await;
    }

    #[tokio::test]
    async fn user_email_missing_returns_none() {
        // Lazy: no live pool — skip if DATABASE_URL unset would panic on connect.
        // Covered by SQL Err path via closed pool when available.
        let url = std::env::var("DATABASE_URL").ok();
        let Some(url) = url else { return };
        let pool = sqlx::PgPool::connect(&url).await.expect("pool");
        let missing = user_email(&pool, Uuid::nil()).await;
        assert!(missing.is_none());
        let none_site = faucet_site_owner_email(&pool, Uuid::nil()).await;
        assert!(none_site.is_none());
        pool.close().await;
        assert!(user_email(&pool, Uuid::nil()).await.is_none());
        assert!(faucet_site_owner_email(&pool, Uuid::nil()).await.is_none());
    }
}
