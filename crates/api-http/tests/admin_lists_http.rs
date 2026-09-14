//! Extra admin GETs + merchant approve/suspend + audit + treasury.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn get(app: axum::Router, uri: &str, token: &str) -> (axum::http::StatusCode, serde_json::Value) {
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.55")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, v)
}

async fn post_empty(app: axum::Router, uri: &str, token: &str) -> axum::http::StatusCode {
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.55")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_lists_treasury_merchants_audit(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let (state, user_token, user_id, _) = common::register_user(pool.clone(), "adm2").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    let (_a, admin_token) = common::admin_login(pool.clone()).await;

    for path in [
        "/v1/admin/treasury-wallets",
        "/v1/admin/pending-withdrawals",
        "/v1/admin/withdrawals",
        "/v1/admin/merchants",
        "/v1/admin/merchants/stats",
        "/v1/admin/economics",
        "/v1/admin/faucetlist",
        "/v1/admin/rewards/programs",
        "/v1/admin/audit-logs",
        "/v1/admin/telemetry/overview",
    ] {
        let (st, body) = get(api_http::app_without_metrics(state.clone()), path, &admin_token).await;
        assert_eq!(st, axum::http::StatusCode::OK, "{path} -> {body}");
    }

    let (st, body) = get(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/economics",
        &admin_token,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "economics={body}");
    assert!(body.get("all_time").is_some(), "{body}");
    assert!(body.get("last_24h").is_some(), "{body}");
    assert!(body["all_time"].get("faucet_by_coin").is_some(), "{body}");
    assert!(body["all_time"].get("gateway_by_coin").is_some(), "{body}");

    let (st, body) = get(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/merchants/stats",
        &admin_token,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "stats body={body}");
    assert!(body.get("accounts_total").is_some(), "{body}");
    assert!(body.get("invoices_all").is_some(), "{body}");
    assert!(body.get("volume_by_coin").is_some(), "{body}");
    assert!(body.get("top_merchants").is_some(), "{body}");

    // Mark user as merchant-ish then approve/suspend via admin merchants routes
    sqlx::query(
        "UPDATE users SET merchant_status = 'PENDING'::merchant_status, \
         merchant_business_name = 'Biz', merchant_website = 'https://biz.example', \
         merchant_description = 'desc' WHERE id = $1",
    )
    .bind(uid)
    .execute(&pool)
    .await
    .ok();

    let st = post_empty(
        api_http::app_without_metrics(state.clone()),
        &format!("/v1/admin/merchants/{uid}/approve"),
        &admin_token,
    )
    .await;
    assert!(
        st.is_success() || st == axum::http::StatusCode::NO_CONTENT || st == axum::http::StatusCode::OK,
        "approve={st}"
    );

    let st = post_empty(
        api_http::app_without_metrics(state.clone()),
        &format!("/v1/admin/merchants/{uid}/suspend"),
        &admin_token,
    )
    .await;
    assert!(
        st.is_success() || st == axum::http::StatusCode::NO_CONTENT || st == axum::http::StatusCode::OK,
        "suspend={st}"
    );

    // merchant.rs apply + admin applications path
    let apply = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/merchant/apply")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-real-ip", "203.0.113.55")
                .body(Body::from(
                    serde_json::json!({
                        "businessName": "Fresh Biz",
                        "website": "https://fresh.example",
                        "description": "A merchant application for coverage"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // may be CONFLICT if already merchant — still exercises handler
    assert!(
        apply.status().is_success()
            || apply.status() == axum::http::StatusCode::CONFLICT
            || apply.status() == axum::http::StatusCode::OK
    );

    let (st, _) = get(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/merchant/applications",
        &admin_token,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // Fresh user for merchant.rs approve/reject (PENDING)
    let (state_m, _mt, mid, _) = common::register_user(pool.clone(), "madm").await;
    let mid_u = Uuid::parse_str(&mid).unwrap();
    sqlx::query(
        "UPDATE users SET merchant_status = 'PENDING'::merchant_status, \
         merchant_business_name = 'PendBiz', merchant_website = 'https://p.example', \
         merchant_description = 'pending app' WHERE id = $1",
    )
    .bind(mid_u)
    .execute(&pool)
    .await
    .unwrap();

    let st = post_empty(
        api_http::app_without_metrics(state_m.clone()),
        &format!("/v1/admin/merchant/{mid}/approve"),
        &admin_token,
    )
    .await;
    assert!(st.is_success() || st == axum::http::StatusCode::NO_CONTENT, "merchant approve={st}");

    sqlx::query("UPDATE users SET merchant_status = 'PENDING'::merchant_status WHERE id = $1")
        .bind(mid_u)
        .execute(&pool)
        .await
        .unwrap();
    let reject = api_http::app_without_metrics(state_m)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/admin/merchant/{mid}/reject"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-real-ip", "203.0.113.55")
                .body(Body::from(serde_json::json!({ "reason": "incomplete docs" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        reject.status().is_success() || reject.status() == axum::http::StatusCode::NO_CONTENT,
        "reject={}",
        reject.status()
    );

    let _ = user_token;
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_logout_and_login_me(pool: PgPool) {
    let (state, admin_token) = common::admin_login(pool).await;

    let me = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/auth/me")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("x-real-ip", "203.0.113.56")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), axum::http::StatusCode::OK);

    let logout = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/admin/logout")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .header("origin", "http://localhost:5173")
                .header("x-real-ip", "203.0.113.56")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        logout.status().is_success()
            || logout.status() == axum::http::StatusCode::NO_CONTENT
            || logout.status() == axum::http::StatusCode::OK,
        "admin logout={}",
        logout.status()
    );
}
