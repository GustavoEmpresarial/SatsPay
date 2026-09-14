//! Postgres implementation of `domain::auth::AuthRepo` — port of legacy
//! `apps/api/src/modules/auth/repositories/auth.repository.ts`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::auth::{AuthRepo, EmailOtpRow, RefreshTokenRow, RepoError, UserRow};
use shared::Coin;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgAuthRepo {
    pool: PgPool,
}

impl PgAuthRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn map_err(e: sqlx::Error) -> RepoError {
    RepoError(e.to_string())
}

fn row_to_user(row: &sqlx::postgres::PgRow) -> UserRow {
    UserRow {
        id: row.get("id"),
        email: row.get("email"),
        username: row.get("username"),
        password_hash: row.get("password_hash"),
        role: row.get("role"),
        two_factor_enabled: row.get("two_factor_enabled"),
        merchant_status: row.get("merchant_status"),
        created_at: row.get("created_at"),
    }
}

const USER_COLUMNS: &str = "id, email, username, password_hash, role::text as role, two_factor_enabled, merchant_status::text as merchant_status, created_at";

#[async_trait]
impl AuthRepo for PgAuthRepo {
    async fn find_user_by_email(&self, email: &str) -> Result<Option<UserRow>, RepoError> {
        let row = sqlx::query(&format!("SELECT {USER_COLUMNS} FROM users WHERE email = $1"))
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(row.as_ref().map(row_to_user))
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<UserRow>, RepoError> {
        let row = sqlx::query(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(row.as_ref().map(row_to_user))
    }

    async fn find_user_by_username(&self, username: &str) -> Result<Option<UserRow>, RepoError> {
        let row = sqlx::query(&format!("SELECT {USER_COLUMNS} FROM users WHERE lower(username) = lower($1)"))
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(row.as_ref().map(row_to_user))
    }

    async fn create_user_with_wallets(
        &self,
        email: &str,
        username: &str,
        password_hash: &str,
        coins: &[Coin],
    ) -> Result<UserRow, RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;

        let row = sqlx::query(&format!(
            "INSERT INTO users (email, username, password_hash) VALUES ($1, $2, $3) RETURNING {USER_COLUMNS}"
        ))
        .bind(email)
        .bind(username)
        .bind(password_hash)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_err)?;
        let user = row_to_user(&row);

        for coin in coins {
            for kind in ["PERSONAL", "DEVELOPER"] {
                sqlx::query("INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, $3::wallet_kind)")
                    .bind(user.id)
                    .bind(coin.as_str())
                    .bind(kind)
                    .execute(&mut *tx)
                    .await
                    .map_err(map_err)?;
            }
        }

        tx.commit().await.map_err(map_err)?;
        Ok(user)
    }

    async fn update_user_role(&self, user_id: Uuid, role: &str) -> Result<(), RepoError> {
        sqlx::query("UPDATE users SET role = $2::user_role, updated_at = now() WHERE id = $1")
            .bind(user_id)
            .bind(role)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn update_last_login(&self, user_id: Uuid) -> Result<(), RepoError> {
        sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn update_two_factor_enabled(&self, user_id: Uuid, enabled: bool) -> Result<(), RepoError> {
        sqlx::query("UPDATE users SET two_factor_enabled = $2, updated_at = now() WHERE id = $1")
            .bind(user_id)
            .bind(enabled)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn update_username(&self, user_id: Uuid, username: &str) -> Result<UserRow, RepoError> {
        let row = sqlx::query(&format!(
            "UPDATE users SET username = $2, updated_at = now() WHERE id = $1 RETURNING {USER_COLUMNS}"
        ))
        .bind(user_id)
        .bind(username)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(row_to_user(&row))
    }

    async fn create_refresh_token(&self, user_id: Uuid, token_hash: &str, expires_at: DateTime<Utc>) -> Result<(), RepoError> {
        sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(token_hash)
            .bind(expires_at)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn claim_refresh_token(&self, token_hash: &str, now: DateTime<Utc>) -> Result<u64, RepoError> {
        let result = sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = $2 WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > $2",
        )
        .bind(token_hash)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(result.rows_affected())
    }

    async fn find_refresh_token_with_user(&self, token_hash: &str) -> Result<Option<(RefreshTokenRow, UserRow)>, RepoError> {
        let row = sqlx::query(
            "SELECT rt.id, rt.user_id, rt.token_hash, rt.expires_at, rt.revoked_at,
                    u.id as user_row_id, u.email, u.username, u.password_hash, u.role::text as role,
                    u.two_factor_enabled, u.merchant_status::text as merchant_status, u.created_at as user_created_at
             FROM refresh_tokens rt JOIN users u ON u.id = rt.user_id
             WHERE rt.token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?;

        let Some(row) = row else { return Ok(None) };
        let refresh = RefreshTokenRow {
            id: row.get("id"),
            user_id: row.get("user_id"),
            token_hash: row.get("token_hash"),
            expires_at: row.get("expires_at"),
            revoked_at: row.get("revoked_at"),
        };
        let user = UserRow {
            id: row.get("user_row_id"),
            email: row.get("email"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            role: row.get("role"),
            two_factor_enabled: row.get("two_factor_enabled"),
            merchant_status: row.get("merchant_status"),
            created_at: row.get("user_created_at"),
        };
        Ok(Some((refresh, user)))
    }

    async fn find_refresh_token(&self, token_hash: &str) -> Result<Option<RefreshTokenRow>, RepoError> {
        let row = sqlx::query("SELECT id, user_id, token_hash, expires_at, revoked_at FROM refresh_tokens WHERE token_hash = $1")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(row.map(|r| RefreshTokenRow {
            id: r.get("id"),
            user_id: r.get("user_id"),
            token_hash: r.get("token_hash"),
            expires_at: r.get("expires_at"),
            revoked_at: r.get("revoked_at"),
        }))
    }

    async fn revoke_all_user_refresh_tokens(&self, user_id: Uuid, now: DateTime<Utc>) -> Result<(), RepoError> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = $2 WHERE user_id = $1 AND revoked_at IS NULL")
            .bind(user_id)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn revoke_refresh_token_by_hash(&self, token_hash: &str, now: DateTime<Utc>) -> Result<(), RepoError> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = $2 WHERE token_hash = $1 AND revoked_at IS NULL")
            .bind(token_hash)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn find_recent_otp(&self, user_id: Uuid, purpose: &str, since: DateTime<Utc>) -> Result<Option<EmailOtpRow>, RepoError> {
        let row = sqlx::query(
            "SELECT id, code_hash, attempts FROM email_otps
             WHERE user_id = $1 AND purpose = $2::otp_purpose AND created_at > $3
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(purpose)
        .bind(since)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(row.map(|r| EmailOtpRow { id: r.get("id"), code_hash: r.get("code_hash"), attempts: r.get("attempts") }))
    }

    async fn rotate_otp(&self, user_id: Uuid, purpose: &str, code_hash: &str, expires_at: DateTime<Utc>, now: DateTime<Utc>) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        sqlx::query("UPDATE email_otps SET consumed_at = $3 WHERE user_id = $1 AND purpose = $2::otp_purpose AND consumed_at IS NULL")
            .bind(user_id)
            .bind(purpose)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_err)?;
        sqlx::query("INSERT INTO email_otps (user_id, purpose, code_hash, expires_at) VALUES ($1, $2::otp_purpose, $3, $4)")
            .bind(user_id)
            .bind(purpose)
            .bind(code_hash)
            .bind(expires_at)
            .execute(&mut *tx)
            .await
            .map_err(map_err)?;
        tx.commit().await.map_err(map_err)?;
        Ok(())
    }

    async fn find_active_otp(&self, user_id: Uuid, purpose: &str, now: DateTime<Utc>) -> Result<Option<EmailOtpRow>, RepoError> {
        let row = sqlx::query(
            "SELECT id, code_hash, attempts FROM email_otps
             WHERE user_id = $1 AND purpose = $2::otp_purpose AND consumed_at IS NULL AND expires_at > $3
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(purpose)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(row.map(|r| EmailOtpRow { id: r.get("id"), code_hash: r.get("code_hash"), attempts: r.get("attempts") }))
    }

    async fn increment_otp_attempts(&self, otp_id: Uuid) -> Result<(), RepoError> {
        sqlx::query("UPDATE email_otps SET attempts = attempts + 1 WHERE id = $1")
            .bind(otp_id)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn claim_otp(&self, otp_id: Uuid, now: DateTime<Utc>) -> Result<u64, RepoError> {
        let result = sqlx::query("UPDATE email_otps SET consumed_at = $2 WHERE id = $1 AND consumed_at IS NULL")
            .bind(otp_id)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(result.rows_affected())
    }
}
