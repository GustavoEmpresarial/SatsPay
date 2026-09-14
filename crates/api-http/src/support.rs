//! In-app support tickets (user ↔ staff). No mailto.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use db::support::SupportError;
use domain::auth::AuthRepo;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::middleware::{require_admin, AuthUser};
use crate::AppState;

pub fn routes<R: AuthRepo + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route(
            "/v1/support/tickets",
            get(list_tickets::<R>).post(create_ticket::<R>),
        )
        .route("/v1/support/tickets/:id", get(get_ticket::<R>))
        .route(
            "/v1/support/tickets/:id/messages",
            post(reply_ticket::<R>),
        )
        .route(
            "/v1/admin/support/tickets",
            get(admin_list_tickets::<R>),
        )
        .route(
            "/v1/admin/support/tickets/:id",
            get(admin_get_ticket::<R>),
        )
        .route(
            "/v1/admin/support/tickets/:id/messages",
            post(admin_reply_ticket::<R>),
        )
        .route(
            "/v1/admin/support/tickets/:id/status",
            post(admin_set_status::<R>),
        )
}

#[derive(Deserialize)]
struct CreateTicketBody {
    topic: String,
    message: String,
}

#[derive(Deserialize)]
struct MessageBody {
    message: String,
}

#[derive(Deserialize)]
struct StatusBody {
    status: String,
}

#[derive(Deserialize)]
struct AdminListQuery {
    status: Option<String>,
}

fn map_err(e: SupportError) -> Response {
    let (code, msg) = match &e {
        SupportError::InvalidTopic => (StatusCode::BAD_REQUEST, "invalid_topic"),
        SupportError::EmptyMessage => (StatusCode::BAD_REQUEST, "empty_message"),
        SupportError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        SupportError::Closed => (StatusCode::CONFLICT, "ticket_closed"),
        SupportError::InvalidStatus => (StatusCode::BAD_REQUEST, "invalid_status"),
        SupportError::Db(err) => {
            tracing::error!(error = %err, "support db error");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
        }
    };
    (code, Json(json!({ "error": msg }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn err_body(e: SupportError) -> (StatusCode, String) {
        let res = map_err(e);
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, v["error"].as_str().unwrap().to_string())
    }

    #[tokio::test]
    async fn map_err_covers_all_variants() {
        let cases = [
            (SupportError::InvalidTopic, StatusCode::BAD_REQUEST, "invalid_topic"),
            (SupportError::EmptyMessage, StatusCode::BAD_REQUEST, "empty_message"),
            (SupportError::NotFound, StatusCode::NOT_FOUND, "not_found"),
            (SupportError::Closed, StatusCode::CONFLICT, "ticket_closed"),
            (SupportError::InvalidStatus, StatusCode::BAD_REQUEST, "invalid_status"),
            (
                SupportError::Db(sqlx::Error::RowNotFound),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
            ),
        ];
        for (err, want_status, want_msg) in cases {
            let (st, msg) = err_body(err).await;
            assert_eq!(st, want_status);
            assert_eq!(msg, want_msg);
        }
    }
}

async fn create_ticket<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Json(body): Json<CreateTicketBody>,
) -> Response {
    match db::support::create_ticket(&state.pool, user.id, &body.topic, &body.message).await {
        Ok(detail) => (StatusCode::CREATED, Json(detail)).into_response(),
        Err(e) => map_err(e),
    }
}

async fn list_tickets<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
) -> Response {
    match db::support::list_tickets_for_user(&state.pool, user.id).await {
        Ok(tickets) => Json(json!({ "tickets": tickets })).into_response(),
        Err(e) => map_err(e),
    }
}

async fn get_ticket<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Response {
    match db::support::get_ticket_for_user(&state.pool, user.id, id).await {
        Ok(detail) => Json(detail).into_response(),
        Err(e) => map_err(e),
    }
}

async fn reply_ticket<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MessageBody>,
) -> Response {
    match db::support::add_user_message(&state.pool, user.id, id, &body.message).await {
        Ok(detail) => Json(detail).into_response(),
        Err(e) => map_err(e),
    }
}

async fn admin_list_tickets<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Query(q): Query<AdminListQuery>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    let filter = q.status.as_deref().filter(|s| *s != "ALL" && !s.is_empty());
    match db::support::list_tickets_admin(&state.pool, filter).await {
        Ok(tickets) => Json(json!({ "tickets": tickets })).into_response(),
        Err(e) => map_err(e),
    }
}

async fn admin_get_ticket<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::support::get_ticket_admin(&state.pool, id).await {
        Ok(detail) => Json(detail).into_response(),
        Err(e) => map_err(e),
    }
}

async fn admin_reply_ticket<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MessageBody>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::support::add_staff_message(&state.pool, user.id, id, &body.message).await {
        Ok(detail) => Json(detail).into_response(),
        Err(e) => map_err(e),
    }
}

async fn admin_set_status<R: AuthRepo>(
    State(state): State<AppState<R>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<StatusBody>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return *r;
    }
    match db::support::set_ticket_status(&state.pool, id, &body.status).await {
        Ok(detail) => Json(detail).into_response(),
        Err(e) => map_err(e),
    }
}
