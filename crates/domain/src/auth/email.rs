//! `EmailSender` — the trait `AuthService` sends OTP/notification emails
//! through. Kept here (not in a lower-level crate) so `domain` stays free of
//! any concrete transport (SMTP, etc); `crates/notify` provides the real
//! SMTP implementation, tests supply an in-memory fake.

use async_trait::async_trait;

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), String>;
}

/// Drops every email — used where no delivery is configured (matches the
/// pre-item-2 behavior of only logging that a code was generated). Never
/// used by default in production code paths; callers must opt in explicitly.
pub struct NoopEmailSender;

#[async_trait]
impl EmailSender for NoopEmailSender {
    async fn send(&self, to: &str, subject: &str, _body: &str) -> Result<(), String> {
        tracing::warn!(to, subject, "NoopEmailSender: email NOT sent — no real sender configured");
        Ok(())
    }
}
