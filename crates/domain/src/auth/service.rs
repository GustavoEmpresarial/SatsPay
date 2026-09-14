use super::repo::{AuthRepo, RepoError, UserRow};
use chrono::{Duration, Utc};
use crypto::{hash_password, random_token, verify_password, JwtService, SecretsService};
use serde::{Deserialize, Serialize};
use shared::COINS;
use std::sync::Arc;
use thiserror::Error;

// Precomputed dummy argon2id hash, used to equalize verify() timing when the
// user does not exist — mirrors legacy `DUMMY_ARGON2_HASH`.
const DUMMY_ARGON2_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$YnJvd25pZXNyb2Nrcw$q9DkQxAsAJDpLu1lVn6RGaXwzp/wCr8dGKZlbJnMxT8";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("this email cannot self-register")]
    Forbidden,
    #[error("unable to register with the provided credentials")]
    Conflict,
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("invalid or expired email code")]
    Invalid2fa,
    #[error("invalid refresh token")]
    Unauthorized,
    #[error("refresh token reuse detected; session revoked")]
    RefreshReuseDetected,
    #[error("please wait before requesting another code")]
    RateLimited,
    #[error("user not found")]
    NotFound,
    #[error("passwords do not match")]
    PasswordMismatch,
    #[error("you must accept the terms and conditions")]
    TermsNotAccepted,
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub role: String,
    #[serde(rename = "twoFactor")]
    pub two_factor: bool,
    pub iss: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, Clone)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug)]
pub enum LoginResult {
    Ok { user: UserRow, tokens: AuthTokens },
    CodeSent { email: String },
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_access_ttl_secs: i64,
    pub jwt_refresh_ttl_secs: i64,
    pub admin_emails: Vec<String>,
    pub house_email: String,
    pub otp_code_ttl_secs: i64,
    pub otp_resend_cooldown_secs: i64,
    pub otp_max_attempts: i32,
    /// Seconds after a refresh-token claim during which a parallel presenter
    /// is treated as a multi-tab race (new tokens) instead of theft. `0`
    /// restores strict single-use + family revoke (used in unit tests).
    pub refresh_reuse_grace_secs: i64,
}

impl AuthConfig {
    fn is_admin_email(&self, email: &str) -> bool {
        self.admin_emails.iter().any(|e| e.eq_ignore_ascii_case(email))
    }
}

pub struct AuthService<R: AuthRepo> {
    repo: Arc<R>,
    jwt: JwtService,
    secrets: Arc<SecretsService>,
    config: AuthConfig,
    email_sender: Arc<dyn super::EmailSender>,
}

impl<R: AuthRepo> AuthService<R> {
    pub fn new(
        repo: Arc<R>,
        jwt: JwtService,
        secrets: Arc<SecretsService>,
        config: AuthConfig,
        email_sender: Arc<dyn super::EmailSender>,
    ) -> Self {
        Self {
            repo,
            jwt,
            secrets,
            config,
            email_sender,
        }
    }

    fn sign_access_token(&self, user: &UserRow) -> Result<String, AuthError> {
        let now = Utc::now();
        let claims = AccessTokenClaims {
            sub: user.id.to_string(),
            role: user.role.clone(),
            two_factor: user.two_factor_enabled,
            iss: self.jwt.config.issuer.clone(),
            aud: self.jwt.config.audience.clone(),
            iat: now.timestamp(),
            exp: (now + Duration::seconds(self.config.jwt_access_ttl_secs)).timestamp(),
        };
        self.jwt.sign(&claims).map_err(|_| AuthError::Unauthorized)
    }

    async fn issue_tokens(&self, user: &UserRow) -> Result<AuthTokens, AuthError> {
        let access_token = self.sign_access_token(user)?;
        let refresh_token = random_token(48);
        let token_hash = self.secrets.refresh_token_hash(&refresh_token);
        let expires_at = Utc::now() + Duration::seconds(self.config.jwt_refresh_ttl_secs);
        self.repo.create_refresh_token(user.id, &token_hash, expires_at).await?;
        Ok(AuthTokens { access_token, refresh_token })
    }

    pub async fn register(
        &self,
        email: &str,
        username: &str,
        password: &str,
        password_confirm: &str,
        accept_terms: bool,
    ) -> Result<(UserRow, AuthTokens), AuthError> {
        if !accept_terms {
            return Err(AuthError::TermsNotAccepted);
        }
        if password != password_confirm {
            return Err(AuthError::PasswordMismatch);
        }
        validate_password(password)?;
        let username = validate_username(username)?;

        if self.config.is_admin_email(email) || email == self.config.house_email {
            return Err(AuthError::Forbidden);
        }
        if self.repo.find_user_by_email(email).await?.is_some() {
            return Err(AuthError::Conflict);
        }
        if self.repo.find_user_by_username(&username).await?.is_some() {
            return Err(AuthError::Conflict);
        }

        let password_hash = hash_password(password).map_err(|_| AuthError::Conflict)?;
        let user = self
            .repo
            .create_user_with_wallets(email, &username, &password_hash, &COINS)
            .await?;
        let tokens = self.issue_tokens(&user).await?;
        Ok((user, tokens))
    }

    /// `email_code` is the OTP the client already collected from a prior
    /// `CodeSent` response. `admin_gate` mirrors the legacy admin-login
    /// endpoint: on that gate, a non-admin gets the exact same generic error
    /// as a bad password, so the endpoint can't be used to enumerate admins.
    pub async fn login(&self, email: &str, password: &str, email_code: Option<&str>, admin_gate: bool) -> Result<LoginResult, AuthError> {
        if email == self.config.house_email {
            let _ = verify_password(password, DUMMY_ARGON2_HASH);
            return Err(AuthError::InvalidCredentials);
        }

        let Some(mut user) = self.repo.find_user_by_email(email).await? else {
            let _ = verify_password(password, DUMMY_ARGON2_HASH);
            return Err(AuthError::InvalidCredentials);
        };

        let valid = verify_password(password, &user.password_hash).unwrap_or(false);
        if !valid {
            return Err(AuthError::InvalidCredentials);
        }

        // Auto-sync admin role from the env allowlist.
        let should_be_admin = self.config.is_admin_email(&user.email);
        if should_be_admin && user.role != "ADMIN" {
            self.repo.update_user_role(user.id, "ADMIN").await?;
            user.role = "ADMIN".to_string();
        } else if !should_be_admin && user.role == "ADMIN" {
            self.repo.update_user_role(user.id, "USER").await?;
            user.role = "USER".to_string();
        }

        if admin_gate && user.role != "ADMIN" {
            return Err(AuthError::InvalidCredentials);
        }

        // Se 2FA estiver ativado na conta, exige o código OTP por e-mail
        let require_2fa = user.two_factor_enabled;
        if require_2fa {
            match email_code {
                None => {
                    self.request_email_otp(user.id, "LOGIN").await?;
                    return Ok(LoginResult::CodeSent { email: user.email });
                }
                Some(code) => {
                    if !self.verify_email_otp(user.id, "LOGIN", code).await? {
                        return Err(AuthError::Invalid2fa);
                    }
                }
            }
        }

        self.repo.update_last_login(user.id).await?;
        let tokens = self.issue_tokens(&user).await?;
        Ok(LoginResult::Ok { user, tokens })
    }

    /// Atomic single-use claim on the token row prevents two concurrent
    /// `/auth/refresh` calls from both minting a valid session out of one
    /// cookie. A previously-revoked token presented again is treated as
    /// reuse/theft: every session for that user is revoked (with a 60s grace
    /// window to tolerate parallel SPA / multi-tab refresh races).
    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthTokens, AuthError> {
        let now = Utc::now();
        // Prefer keyed v2 hash; fall back to legacy bare SHA-256 once so old
        // sessions can rotate into v2 without a forced mass logout.
        for token_hash in self.secrets.refresh_token_lookup_hashes(refresh_token) {
            let claimed = self.repo.claim_refresh_token(&token_hash, now).await?;
            if claimed == 1 {
                let Some((_, user)) = self.repo.find_refresh_token_with_user(&token_hash).await? else {
                    return Err(AuthError::Unauthorized);
                };
                return self.issue_tokens(&user).await;
            }

            let existing = self.repo.find_refresh_token(&token_hash).await?;
            if let Some(existing) = existing {
                if let Some(revoked_at) = existing.revoked_at {
                    let grace_period = Duration::seconds(self.config.refresh_reuse_grace_secs.max(0));
                    if self.config.refresh_reuse_grace_secs > 0
                        && now.signed_duration_since(revoked_at) < grace_period
                        && now < existing.expires_at
                    {
                        if let Some((_, user)) = self.repo.find_refresh_token_with_user(&token_hash).await? {
                            return self.issue_tokens(&user).await;
                        }
                    }

                    self.repo.revoke_all_user_refresh_tokens(existing.user_id, now).await?;
                    return Err(AuthError::RefreshReuseDetected);
                }
            }
        }
        Err(AuthError::Unauthorized)
    }

    pub async fn revoke_refresh_token(&self, refresh_token: &str) -> Result<(), AuthError> {
        let now = Utc::now();
        for token_hash in self.secrets.refresh_token_lookup_hashes(refresh_token) {
            self.repo.revoke_refresh_token_by_hash(&token_hash, now).await?;
        }
        Ok(())
    }

    pub async fn get_user_by_id(&self, user_id: uuid::Uuid) -> Result<Option<UserRow>, AuthError> {
        Ok(self.repo.find_user_by_id(user_id).await?)
    }


    pub async fn update_username(&self, user_id: uuid::Uuid, username: &str) -> Result<UserRow, AuthError> {
        let username = validate_username(username)?;
        let Some(current) = self.repo.find_user_by_id(user_id).await? else {
            return Err(AuthError::NotFound);
        };
        if current.username == username {
            return Ok(current);
        }
        if let Some(other) = self.repo.find_user_by_username(&username).await? {
            if other.id != user_id {
                return Err(AuthError::Conflict);
            }
        }
        Ok(self.repo.update_username(user_id, &username).await?)
    }

    /// Verifies an access token from the `Authorization` header, used by the
    /// axum auth middleware to resolve the current user id.
    pub fn verify_access_token(&self, token: &str) -> Result<AccessTokenClaims, AuthError> {
        self.jwt.verify(token).map_err(|_| AuthError::Unauthorized)
    }

    /// Verifies a fresh email OTP for the `WITHDRAWAL` purpose — withdrawals
    /// always require a valid 2FA code regardless of admin gate, mirroring
    /// legacy `verifyEmailOtp(userId, 'WITHDRAWAL', emailCode)`.
    pub async fn verify_withdrawal_otp(&self, user_id: uuid::Uuid, code: &str) -> Result<bool, AuthError> {
        self.verify_email_otp(user_id, "WITHDRAWAL", code).await
    }

    /// Send a fresh email OTP. Used by HTTP step-up (withdraw / admin approve).
    pub async fn request_otp(&self, user_id: uuid::Uuid, purpose: &str) -> Result<(), AuthError> {
        self.request_email_otp(user_id, purpose).await?;
        Ok(())
    }

    /// Verify an email OTP for an arbitrary purpose (`LOGIN`, `WITHDRAWAL`, …).
    pub async fn verify_otp(&self, user_id: uuid::Uuid, purpose: &str, code: &str) -> Result<bool, AuthError> {
        self.verify_email_otp(user_id, purpose, code).await
    }

    pub async fn enable_2fa(&self, user_id: uuid::Uuid, code: &str) -> Result<(), AuthError> {
        if !self.verify_email_otp(user_id, "ENABLE_2FA", code).await? {
            return Err(AuthError::Invalid2fa);
        }
        self.repo.update_two_factor_enabled(user_id, true).await?;
        Ok(())
    }

    pub async fn disable_2fa(&self, user_id: uuid::Uuid, code: &str) -> Result<(), AuthError> {
        if !self.verify_email_otp(user_id, "DISABLE_2FA", code).await? {
            return Err(AuthError::Invalid2fa);
        }
        self.repo.update_two_factor_enabled(user_id, false).await?;
        Ok(())
    }

    // --- Email OTP (port of otp.service.ts) ---

    async fn request_email_otp(&self, user_id: uuid::Uuid, purpose: &str) -> Result<String, AuthError> {
        let user = self.repo.find_user_by_id(user_id).await?.ok_or(AuthError::NotFound)?;
        let now = Utc::now();

        let recent = self.repo.find_recent_otp(user_id, purpose, now - Duration::seconds(self.config.otp_resend_cooldown_secs)).await?;
        if recent.is_some() {
            return Err(AuthError::RateLimited);
        }

        let code = generate_otp_code();
        let code_hash = hash_password(&code).map_err(|_| AuthError::RateLimited)?;
        self.repo.rotate_otp(user_id, purpose, &code_hash, now + Duration::seconds(self.config.otp_code_ttl_secs), now).await?;

        let subject = format!("BitcoSats — your {} code", purpose.to_lowercase().replace('_', " "));
        let body = format!("Your BitcoSats verification code is: {code}\n\nThis code expires in {} minutes. If you didn't request this, ignore this email.", self.config.otp_code_ttl_secs / 60);
        if let Err(e) = self.email_sender.send(&user.email, &subject, &body).await {
            tracing::error!(user_id = %user_id, purpose, error = %e, "failed to send OTP email");
        }

        Ok(code)
    }

    async fn verify_email_otp(&self, user_id: uuid::Uuid, purpose: &str, code: &str) -> Result<bool, AuthError> {
        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            return Ok(false);
        }
        let now = Utc::now();
        let Some(otp) = self.repo.find_active_otp(user_id, purpose, now).await? else {
            return Ok(false);
        };
        if otp.attempts >= self.config.otp_max_attempts {
            self.repo.claim_otp(otp.id, now).await?;
            return Ok(false);
        }

        let ok = verify_password(code, &otp.code_hash).unwrap_or(false);
        if !ok {
            self.repo.increment_otp_attempts(otp.id).await?;
            return Ok(false);
        }

        let claimed = self.repo.claim_otp(otp.id, now).await?;
        Ok(claimed == 1)
    }
}

fn validate_username(username: &str) -> Result<String, AuthError> {
    let username = username.trim();
    if username.len() < 3 || username.len() > 24 {
        return Err(AuthError::Validation(
            "username must be between 3 and 24 characters".into(),
        ));
    }
    let mut chars = username.chars();
    let Some(first) = chars.next() else {
        return Err(AuthError::Validation("username is required".into()));
    };
    if !first.is_ascii_alphabetic() {
        return Err(AuthError::Validation(
            "username must start with a letter".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AuthError::Validation(
            "username may only contain letters, numbers and underscore".into(),
        ));
    }
    Ok(username.to_ascii_lowercase())
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < 10 {
        return Err(AuthError::Validation(
            "password must be at least 10 characters".into(),
        ));
    }
    if password.len() > 128 {
        return Err(AuthError::Validation("password is too long".into()));
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AuthError::Validation(
            "password must contain an uppercase letter".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        return Err(AuthError::Validation(
            "password must contain a lowercase letter".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AuthError::Validation(
            "password must contain a digit".into(),
        ));
    }
    Ok(())
}

fn generate_otp_code() -> String {
    use rand::Rng;
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000))
}
