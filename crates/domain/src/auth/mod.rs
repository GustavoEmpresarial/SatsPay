//! Port of legacy `apps/api/src/modules/auth/services/auth.service.ts` +
//! `otp.service.ts`. Business rules only — `AuthRepo` is implemented against
//! Postgres by `db::auth::PgAuthRepo`; tests can supply an in-memory fake.

mod email;
mod repo;
mod service;

pub use email::{EmailSender, NoopEmailSender};
pub use repo::{AuthRepo, EmailOtpRow, RefreshTokenRow, RepoError, UserRow};
pub use service::{AuthConfig, AuthError, AuthService, AuthTokens, LoginResult};

#[cfg(test)]
mod tests;
