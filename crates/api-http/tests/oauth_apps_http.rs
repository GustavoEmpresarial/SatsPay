//! OAuth discovery + app CRUD + authorize info + authorized-apps list.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn oauth_openid_and_apps_lifecycle(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "oauth").await;

    let discovery = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/.well-known/openid-configuration")
                .header("x-real-ip", "203.0.113.80")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(discovery.status(), axum::http::StatusCode::OK);
    let bytes = discovery.into_body().collect().await.unwrap().to_bytes();
    let cfg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(cfg["issuer"].as_str().unwrap().contains("satspay"));

    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/apps")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.80")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Oneshot App",
                        "description": "test oauth app",
                        "website_url": "https://app.example",
                        "redirect_uris": ["https://app.example/cb", "http://localhost:5173/cb"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = create.status();
    let bytes = create.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "create={}",
        String::from_utf8_lossy(&bytes)
    );
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let app_id = created["app"]["id"]
        .as_str()
        .or_else(|| created["id"].as_str())
        .expect("app id");
    let client_id = created["app"]["client_id"]
        .as_str()
        .or_else(|| created["client_id"].as_str())
        .expect("client_id");

    let list = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/oauth/apps")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.80")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), axum::http::StatusCode::OK);

    let info = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri(format!(
                    "/v1/oauth/authorize/info?client_id={client_id}&redirect_uri=https://app.example/cb"
                ))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.80")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = info.status();
    let bytes = info.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "info={}",
        String::from_utf8_lossy(&bytes)
    );

    let rotate = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/oauth/apps/{app_id}/rotate-secret"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.80")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rotate.status(), axum::http::StatusCode::OK);

    let authorized = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/oauth/authorized-apps")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.80")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), axum::http::StatusCode::OK);

    let update = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/v1/oauth/apps/{app_id}"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.80")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Oneshot App Renamed",
                        "redirect_uris": ["https://app.example/cb"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), axum::http::StatusCode::OK);

    let delete = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/oauth/apps/{app_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.80")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), axum::http::StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn oauth_userinfo_missing_token(pool: PgPool) {
    let state = common::test_state(pool);
    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/oauth/userinfo")
                .header("x-real-ip", "203.0.113.81")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
}
