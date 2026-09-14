use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::Coin;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub password_hash: String,
    pub role: String, // "USER" | "ADMIN"
    pub two_factor_enabled: bool,
    pub merchant_status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct EmailOtpRow {
    pub id: Uuid,
    pub code_hash: String,
    pub attempts: i32,
}

#[derive(Debug, thiserror::Error)]
#[error("auth repository error: {0}")]
pub struct RepoError(pub String);

/// Mirrors legacy `authRepository` 1:1 (see `auth.repository.ts`). Kept as a
/// trait so `domain::auth::AuthService` can be tested against an in-memory
/// fake without Postgres.
#[async_trait]
pub trait AuthRepo: Send + Sync {
    async fn find_user_by_email(&self, email: &str) -> Result<Option<UserRow>, RepoError>;
    async fn find_user_by_username(&self, username: &str) -> Result<Option<UserRow>, RepoError>;
    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<UserRow>, RepoError>;
    /// Creates the user AND its PERSONAL+DEVELOPER wallets for every `Coin`
    /// in one transaction — mirrors the legacy `prisma.$transaction` in
    /// `register()`.
    async fn create_user_with_wallets(
        &self,
        email: &str,
        username: &str,
        password_hash: &str,
        coins: &[Coin],
    ) -> Result<UserRow, RepoError>;
    async fn update_user_role(&self, user_id: Uuid, role: &str) -> Result<(), RepoError>;
    async fn update_last_login(&self, user_id: Uuid) -> Result<(), RepoError>;
    async fn update_two_factor_enabled(&self, user_id: Uuid, enabled: bool) -> Result<(), RepoError>;
    async fn update_username(&self, user_id: Uuid, username: &str) -> Result<UserRow, RepoError>;

    async fn create_refresh_token(&self, user_id: Uuid, token_hash: &str, expires_at: DateTime<Utc>) -> Result<(), RepoError>;
    /// Atomically flips `revoked_at` from NULL for a non-expired token.
    /// Returns the number of rows affected (0 or 1) — the caller uses this
    /// as a single-use claim to prevent concurrent-refresh double-issue.
    async fn claim_refresh_token(&self, token_hash: &str, now: DateTime<Utc>) -> Result<u64, RepoError>;
    async fn find_refresh_token_with_user(&self, token_hash: &str) -> Result<Option<(RefreshTokenRow, UserRow)>, RepoError>;
    async fn find_refresh_token(&self, token_hash: &str) -> Result<Option<RefreshTokenRow>, RepoError>;
    async fn revoke_all_user_refresh_tokens(&self, user_id: Uuid, now: DateTime<Utc>) -> Result<(), RepoError>;
    async fn revoke_refresh_token_by_hash(&self, token_hash: &str, now: DateTime<Utc>) -> Result<(), RepoError>;

    async fn find_recent_otp(&self, user_id: Uuid, purpose: &str, since: DateTime<Utc>) -> Result<Option<EmailOtpRow>, RepoError>;
    /// Consumes any still-active OTP of this purpose, then inserts the new one.
    async fn rotate_otp(&self, user_id: Uuid, purpose: &str, code_hash: &str, expires_at: DateTime<Utc>, now: DateTime<Utc>) -> Result<(), RepoError>;
    async fn find_active_otp(&self, user_id: Uuid, purpose: &str, now: DateTime<Utc>) -> Result<Option<EmailOtpRow>, RepoError>;
    async fn increment_otp_attempts(&self, otp_id: Uuid) -> Result<(), RepoError>;
    /// Atomic single-use claim, same pattern as `claim_refresh_token`.
    async fn claim_otp(&self, otp_id: Uuid, now: DateTime<Utc>) -> Result<u64, RepoError>;
}
