//! Merchant deposit invoices + public pay GET.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_invoice_create_list_get_public(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "minv").await;

    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/merchant/deposits/create")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.85")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "amount": "100000",
                        "orderId": "order-oneshot-1",
                        "callbackUrl": "https://merchant.example/hook",
                        "siteName": "Oneshot Shop",
                        "description": "test invoice"
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
        axum::http::StatusCode::CREATED,
        "create={}",
        String::from_utf8_lossy(&bytes)
    );
    let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = inv["id"].as_str().expect("invoice id");

    let list = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/merchant/deposits")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.85")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), axum::http::StatusCode::OK);

    let get = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/merchant/deposits/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.85")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), axum::http::StatusCode::OK);

    let public = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/v1/public/pay/{id}"))
                .header("x-real-ip", "203.0.113.85")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = public.status();
    let bytes = public.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "public={}",
        String::from_utf8_lossy(&bytes)
    );

    // invoices alias
    let alias = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/merchant/invoices")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.85")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(alias.status(), axum::http::StatusCode::OK);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_deposits_unauthorized(pool: PgPool) {
    let state = common::test_state(pool);
    // POST create without auth — `Option<AuthUser>` is None → authenticate_merchant 401.
    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/merchant/deposits/create")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.86")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "BTC",
                        "amount": "1000",
                        "orderId": "x",
                        "callbackUrl": "https://merchant.example/hook"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
}
