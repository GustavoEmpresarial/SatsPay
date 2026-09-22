//! Stable 5xx bodies. Never put `e.to_string()` (sqlx, IO, provider) in JSON.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use uuid::Uuid;

/// Log `err` server-side and return `{ code: INTERNAL, requestId }` with no debug text.
pub fn internal_error(err: impl std::fmt::Display) -> Response {
    internal_error_parts(err).into_response()
}

/// Same body as [`internal_error`] for handlers that return `Result<_, (StatusCode, Json<Value>)>`.
pub fn internal_error_parts(err: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    let request_id = Uuid::new_v4();
    tracing::error!(%request_id, error = %err, "internal error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "code": "INTERNAL",
            "requestId": request_id.to_string(),
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn internal_error_hides_debug_and_returns_request_id() {
        let resp = internal_error("sqlx: relation \"secret_table\" does not exist");
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = to_bytes(resp.into_body(), 1024).await.expect("body");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(v["code"], "INTERNAL");
        assert!(v["requestId"].as_str().unwrap().len() >= 32);
        let dumped = bytes.to_vec();
        let text = String::from_utf8(dumped).unwrap();
        assert!(!text.contains("secret_table"));
        assert!(!text.contains("sqlx"));
    }
}
