//! Auth middleware: the `AuthUser` extractor (verifies the `Authorization:
//! Bearer <access token>` header) and the `require_admin` gate.
//!
//! Reconstructed from its call sites across `api-http` — every route handler
//! takes `user: AuthUser` (or `_user: AuthUser` to require auth without
//! reading identity), and admin handlers open with
//! `if let Err(r) = require_admin(&user) { return *r; }`.

use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use domain::auth::AuthRepo;
use serde_json::json;
use uuid::Uuid;

/// The authenticated caller, resolved from a verified JWT access token.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub role: String,
    /// `twoFactor` claim — whether the session was minted after a 2FA step.
    #[allow(dead_code)]
    pub two_factor: bool,
}

fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({ "error": "unauthorized" }))).into_response()
}

#[axum::async_trait]
impl<R> FromRequestParts<AppState<R>> for AuthUser
where
    R: AuthRepo + Send + Sync + 'static,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState<R>) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(unauthorized)?;

        let token = raw
            .strip_prefix("Bearer ")
            .or_else(|| raw.strip_prefix("bearer "))
            .unwrap_or(raw)
            .trim();
        if token.is_empty() {
            return Err(unauthorized());
        }

        let claims = state.auth.verify_access_token(token).map_err(|_| unauthorized())?;
        let id = Uuid::parse_str(&claims.sub).map_err(|_| unauthorized())?;

        // Re-read the account instead of trusting the token's claims. Access
        // tokens live for `JWT_ACCESS_TTL_SECS` and cannot be revoked, so a
        // stale `role` claim would keep a demoted admin privileged, and a
        // deleted account would keep working, until the token expired.
        // `get_user_by_id` / `find_user_by_id` is `WHERE erased_at IS NULL`:
        // an erased account is `Ok(None)` → 401 on every request (no jti deny-list).
        match state.auth.get_user_by_id(id).await {
            Ok(Some(user)) => Ok(AuthUser { id, role: user.role, two_factor: user.two_factor_enabled }),
            Ok(None) => Err(unauthorized()),
            Err(e) => {
                tracing::error!(error = %e, "failed to load account while authenticating");
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal error" }))).into_response())
            }
        }
    }
}

/// `Err(Box<Response>)` (403) unless `user` is an admin. Boxed so the common
/// success path stays a thin `Ok(())`; handlers do
/// `if let Err(r) = require_admin(&user) { return *r; }`.
/// Best-effort identity for public collectors (client errors, logout).
/// Missing or invalid tokens become `None` — never 401.
pub struct OptionalAuthUser(pub Option<AuthUser>);

#[axum::async_trait]
impl<R> FromRequestParts<AppState<R>> for OptionalAuthUser
where
    R: AuthRepo + Send + Sync + 'static,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState<R>) -> Result<Self, Self::Rejection> {
        match AuthUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalAuthUser(Some(user))),
            Err(_) => Ok(OptionalAuthUser(None)),
        }
    }
}

pub fn require_admin(user: &AuthUser) -> Result<(), Box<Response>> {
    if user.role.eq_ignore_ascii_case("ADMIN") {
        Ok(())
    } else {
        Err(Box::new(
            (StatusCode::FORBIDDEN, Json(json!({ "error": "admin access required" }))).into_response(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: &str) -> AuthUser {
        AuthUser { id: Uuid::new_v4(), role: role.into(), two_factor: false }
    }

    #[test]
    fn require_admin_allows_admin_case_insensitive() {
        assert!(require_admin(&user("ADMIN")).is_ok());
        assert!(require_admin(&user("admin")).is_ok());
        assert!(require_admin(&user("Admin")).is_ok());
    }

    #[test]
    fn require_admin_rejects_user_and_merchant() {
        assert!(require_admin(&user("USER")).is_err());
        assert!(require_admin(&user("MERCHANT")).is_err());
        assert!(require_admin(&user("")).is_err());
    }
}
