//! Merchant deposit invoices + public pay GET.

mod common;

use api_http::AppState;
use axum::body::Body;
use db::auth::PgAuthRepo;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

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
    // POST create without a session and without an API key → 401.
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

/// `POST /v1/merchant/deposits` — the spelling the public docs have always
/// shown. It used to be GET-only, so every integration written from the docs
/// got a 405 on its very first call.
#[sqlx::test(migrations = "../db/migrations")]
async fn documented_create_path_and_checkout_url(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "doc-path").await;

    let (st, body) = create(
        state.clone(),
        &token,
        serde_json::json!({
            "coin": "POL",
            "amount": "250000",
            "orderId": "ORD-DOC-1",
            "callbackUrl": "https://merchant.example/hook"
        }),
    )
    .await;

    assert_eq!(st, axum::http::StatusCode::CREATED, "{body}");
    let id = body["id"].as_str().expect("invoice id");
    assert_eq!(body["payUrl"], format!("/pay/{id}"));
    assert_eq!(body["checkoutUrl"], format!("https://www.satspay.pro/pay/{id}"));
    // The 0.25% gateway fee is charged and both sides of it are reported —
    // the docs page did not mention the fee at all.
    assert_eq!(body["feeAmount"], "625");
    assert_eq!(body["netAmount"], "249375");
    assert_eq!(body["status"], "PENDING");
}

/// `amount` is an integer count of ledger units (1e-8). `"25.00"` reads like
/// 25 USDT but would charge 0.00000025 USDT — the class of bug that broke a
/// real integration, so it is rejected rather than reinterpreted.
#[sqlx::test(migrations = "../db/migrations")]
async fn decimal_amount_is_rejected_with_a_unit_hint(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "amt-unit").await;

    let (st, body) = create(
        state.clone(),
        &token,
        serde_json::json!({
            "coin": "USDT",
            "amount": "25.00",
            "orderId": "ORD-AMT-1",
            "callbackUrl": "https://merchant.example/hook"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "AMOUNT_NOT_INTEGER");

    // Dust that rounds to zero at USDT's 6 on-chain decimals can never be paid.
    let (st, body) = create(
        state.clone(),
        &token,
        serde_json::json!({
            "coin": "USDT",
            "amount": "99",
            "orderId": "ORD-AMT-2",
            "callbackUrl": "https://merchant.example/hook"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "AMOUNT_BELOW_MINIMUM");

    // The correct spelling of "25 USDT".
    let (st, body) = create(
        state,
        &token,
        serde_json::json!({
            "coin": "USDT",
            "amount": "2500000000",
            "orderId": "ORD-AMT-3",
            "callbackUrl": "https://merchant.example/hook"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{body}");
}

/// A retried POST must not mint a second payable address for one order, and
/// reusing an `orderId` for a different charge must conflict.
#[sqlx::test(migrations = "../db/migrations")]
async fn order_id_is_idempotent_then_conflicts(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "idem").await;
    let payload = serde_json::json!({
        "coin": "POL",
        "amount": "300000",
        "orderId": "ORD-IDEM-1",
        "callbackUrl": "https://merchant.example/hook"
    });

    let (st, first) = create(state.clone(), &token, payload.clone()).await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{first}");

    let (st, replay) = create(state.clone(), &token, payload).await;
    assert_eq!(st, axum::http::StatusCode::OK, "{replay}");
    assert_eq!(replay["id"], first["id"], "retry must return the same invoice");
    assert_eq!(replay["depositAddress"], first["depositAddress"]);

    let (st, conflict) = create(
        state,
        &token,
        serde_json::json!({
            "coin": "POL",
            "amount": "999999",
            "orderId": "ORD-IDEM-1",
            "callbackUrl": "https://merchant.example/hook"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["code"], "DUPLICATE_ORDER_ID");
}

/// The webhook is an outbound request the server makes on the merchant's
/// instruction — an internal or plaintext target is refused up front (SSRF).
#[sqlx::test(migrations = "../db/migrations")]
async fn callback_url_must_be_public_https(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "cb-url").await;

    for bad in ["http://merchant.example/hook", "https://127.0.0.1/hook", "https://169.254.169.254/latest/meta-data/"] {
        let (st, body) = create(
            state.clone(),
            &token,
            serde_json::json!({
                "coin": "POL",
                "amount": "250000",
                "orderId": format!("ORD-CB-{}", Uuid::new_v4()),
                "callbackUrl": bad
            }),
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::BAD_REQUEST, "{bad} -> {body}");
        assert_eq!(body["code"], "INVALID_CALLBACK_URL", "{bad}");
    }
}

async fn issue_key(
    state: AppState<PgAuthRepo>,
    token: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/api-keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.90")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Regression: the gateway used to authenticate API keys against a hardcoded
/// `"0.0.0.0"` source IP, so a key restricted to the merchant's own server IP
/// could never call it — while the docs advertised the allowlist as the
/// gateway's protection.
#[sqlx::test(migrations = "../db/migrations")]
async fn api_key_auth_uses_the_real_client_ip(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "key-ip").await;
    let issued = issue_key(
        state.clone(),
        &token,
        serde_json::json!({
            "label": "ip-locked",
            "scopes": ["deposits"],
            "allowedIps": ["203.0.113.90"]
        }),
    )
    .await;
    let key = issued["key"].as_str().expect("issued key");

    let allowed = create_with_key(state.clone(), key, "203.0.113.90", "ORD-IP-1").await;
    assert_eq!(allowed.0, axum::http::StatusCode::CREATED, "{}", allowed.1);

    let blocked = create_with_key(state, key, "198.51.100.7", "ORD-IP-2").await;
    assert_eq!(blocked.0, axum::http::StatusCode::UNAUTHORIZED, "{}", blocked.1);
    assert_eq!(blocked.1["code"], "IP_NOT_ALLOWED");
}

/// A key that requires signed requests was locked out of the gateway
/// entirely: `/v1/merchant/*` only ever accepted the plain `x-api-key` path.
#[sqlx::test(migrations = "../db/migrations")]
async fn signed_requests_work_on_the_gateway(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "key-sig").await;
    let issued = issue_key(
        state.clone(),
        &token,
        serde_json::json!({
            "label": "signed",
            "scopes": ["deposits"],
            "requireSignature": true
        }),
    )
    .await;
    let key_id = issued["id"].as_str().unwrap().to_string();
    let key = issued["key"].as_str().unwrap().to_string();

    let payload = serde_json::json!({
        "coin": "POL",
        "amount": "250000",
        "orderId": "ORD-SIG-1",
        "callbackUrl": "https://merchant.example/hook"
    })
    .to_string();
    let ts = chrono::Utc::now().timestamp().to_string();
    let path = "/v1/merchant/deposits";
    let sig = db::public_api::sign_request(&key, &ts, "POST", path, payload.as_bytes());

    let res = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .header("x-key-id", &key_id)
                .header("x-timestamp", &ts)
                .header("x-signature", &sig)
                .header("x-real-ip", "203.0.113.90")
                .body(Body::from(payload.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::CREATED, "signed={}", String::from_utf8_lossy(&bytes));

    // And the plain header path stays closed for that key, as documented.
    let plain = create_with_key(state, &key, "203.0.113.90", "ORD-SIG-2").await;
    assert_eq!(plain.0, axum::http::StatusCode::UNAUTHORIZED, "{}", plain.1);
    assert_eq!(plain.1["code"], "KEY_REQUIRES_SIGNATURE");
}

/// Regression: the list/detail endpoints serialized the invoice struct
/// directly, which emitted snake_case (`order_id`, `fee_amount`), while the
/// merchant dashboard — and every hand-built response in this domain — uses
/// camelCase. The dashboard's invoice table rendered blanks because of it.
#[sqlx::test(migrations = "../db/migrations")]
async fn list_and_detail_use_the_same_camel_case_as_create(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "casing").await;
    let (st, created) = create(
        state.clone(),
        &token,
        serde_json::json!({
            "coin": "POL",
            "amount": "250000",
            "orderId": "ORD-CASE-1",
            "callbackUrl": "https://merchant.example/hook"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap().to_string();

    let get = fetch(state.clone(), &token, &format!("/v1/merchant/deposits/{id}")).await;
    let list = fetch(state, &token, "/v1/merchant/deposits").await;
    let first = &list["invoices"][0];

    for field in ["orderId", "feeAmount", "netAmount", "depositAddress", "receivedAmount", "callbackUrl", "createdAt", "webhookDelivered", "webhookAttempts"] {
        assert!(!get[field].is_null(), "GET must expose {field} (got {get})");
        assert!(!first[field].is_null(), "list must expose {field} (got {first})");
    }
    // The snake_case spelling must be gone, not merely duplicated.
    for field in ["order_id", "fee_amount", "net_amount", "deposit_address", "received_amount", "created_at"] {
        assert!(get[field].is_null(), "GET still exposes {field}");
    }
    assert_eq!(get["orderId"], "ORD-CASE-1");
    // 0.25% of 250000, in whole ledger units — never a fraction of 1e-8.
    assert_eq!(get["feeAmount"], "625");
    assert_eq!(get["netAmount"], "249375");
}

async fn fetch(
    state: AppState<PgAuthRepo>,
    token: &str,
    uri: &str,
) -> serde_json::Value {
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.90")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

/// The gateway checks the `deposits` scope, like `/v1/public/send` checks `send`.
#[sqlx::test(migrations = "../db/migrations")]
async fn gateway_requires_the_deposits_scope(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "key-scope").await;
    let issued = issue_key(
        state.clone(),
        &token,
        serde_json::json!({ "label": "payouts only", "scopes": ["send"] }),
    )
    .await;
    let key = issued["key"].as_str().unwrap();

    let (st, body) = create_with_key(state, key, "203.0.113.90", "ORD-SCOPE-1").await;
    assert_eq!(st, axum::http::StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "MISSING_SCOPE");
}

async fn create(
    state: AppState<PgAuthRepo>,
    token: &str,
    payload: serde_json::Value,
) -> (axum::http::StatusCode, serde_json::Value) {
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/merchant/deposits")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.90")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

async fn create_with_key(
    state: AppState<PgAuthRepo>,
    key: &str,
    ip: &str,
    order_id: &str,
) -> (axum::http::StatusCode, serde_json::Value) {
    let res = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/merchant/deposits")
                .header("content-type", "application/json")
                .header("x-api-key", key)
                .header("x-real-ip", ip)
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "amount": "250000",
                        "orderId": order_id,
                        "callbackUrl": "https://merchant.example/hook"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}
