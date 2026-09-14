//! Admin POSTs + remaining telemetry GETs via ADMIN login (`accessToken` top-level).

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
                .header("x-real-ip", "203.0.113.50")
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

async fn post_json(
    app: axum::Router,
    uri: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut b = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-real-ip", "203.0.113.50");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let response = app
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, v)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_telemetry_and_fund_actions(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.expect("house");
    db::lend::ensure_lend_reserves(&pool).await.expect("lend");

    let (state, token) = common::admin_login(pool.clone()).await;

    for path in [
        "/v1/admin/telemetry/metrics-history?hours=6",
        "/v1/admin/telemetry/errors",
    ] {
        let (st, body) = get(api_http::app_without_metrics(state.clone()), path, &token).await;
        assert_eq!(st, axum::http::StatusCode::OK, "{path} -> {body}");
    }

    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/telemetry/test-error",
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    let err_id = body["errorId"].as_str().expect("errorId");

    let (st, _) = post_json(
        api_http::app_without_metrics(state.clone()),
        &format!("/v1/admin/telemetry/errors/{err_id}/resolve"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);

    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/telemetry/test-error",
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    let err_id2 = body["errorId"].as_str().unwrap();

    let (st, _) = post_json(
        api_http::app_without_metrics(state.clone()),
        &format!("/v1/admin/telemetry/errors/{err_id2}/ignore"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);

    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/telemetry/test-error",
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
    let err_id3 = Uuid::parse_str(body["errorId"].as_str().unwrap()).unwrap();
    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/telemetry/errors/batch-resolve",
        Some(&token),
        serde_json::json!({ "ids": [err_id3] }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/telemetry/errors/resolve-all",
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/telemetry/errors/clear",
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let (st, _) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/house/fund",
        Some(&token),
        serde_json::json!({ "coin": "LTC", "amount": "1000000000" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);

    let (st, _) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/lend-pool/fund",
        Some(&token),
        serde_json::json!({ "coin": "LTC", "amount": "500000000" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);

    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/admin/rewards/programs",
        Some(&token),
        serde_json::json!({
            "rewardCoin": "LTC",
            "marketCoin": "BTC",
            "side": "SUPPLY",
            "emissionPerDay": "1000000"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED, "{body}");
    let prog_id = body["id"].as_str().or_else(|| body.get("id").and_then(|v| v.as_str()));
    if let Some(id) = prog_id {
        let (st, _) = post_json(
            api_http::app_without_metrics(state.clone()),
            &format!("/v1/admin/rewards/programs/{id}/active"),
            Some(&token),
            serde_json::json!({ "active": false }),
        )
        .await;
        assert_eq!(st, axum::http::StatusCode::NO_CONTENT);
    }

    // Public client-error collectors (no admin required).
    let (st, _) = post_json(
        api_http::app_without_metrics(state.clone()),
        "/v1/telemetry/client-error",
        None,
        serde_json::json!({
            "message": "oneshot client boom",
            "url": "/test",
            "kind": "js"
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);

    let (st, _) = post_json(
        api_http::app_without_metrics(state),
        "/v1/telemetry/client-errors",
        Some(&token),
        serde_json::json!({
            "errors": [{ "message": "batch a" }, { "message": "batch b" }]
        }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_faucet_site_lifecycle(pool: PgPool) {
    let (state, user_token, user_id, _) = common::register_user(pool.clone(), "fown").await;
    let uid = Uuid::parse_str(&user_id).unwrap();

    // Public create (auto-APPROVED in db) — covers faucetlist POST + mine.
    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/faucetlist")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-real-ip", "203.0.113.51")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Test Faucet Site",
                        "url": "https://faucet.example/claim",
                        "description": "A longer description for the faucet site listing.",
                        "coins": ["BTC", "LTC"],
                        "rewardInfo": "100 sats"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), axum::http::StatusCode::CREATED);
    let bytes = create.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let approved_id = v["id"].as_str().expect("site id");

    let mine = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/faucetlist/mine")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-real-ip", "203.0.113.51")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mine.status(), axum::http::StatusCode::OK);

    let (_astate, admin_token) = common::admin_login(pool.clone()).await;

    // PENDING row for approve path (API create is auto-APPROVED).
    let pending_id: Uuid = sqlx::query_scalar(
        "INSERT INTO faucet_sites (owner_id, name, url, description, coins, status) \
         VALUES ($1, 'Pending Site', 'https://pending.example', 'description long enough', \
                 ARRAY['BTC']::coin[], 'PENDING') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .expect("insert pending site");

    let (st, _) = post_json(
        api_http::app_without_metrics(state.clone()),
        &format!("/v1/admin/faucetlist/{pending_id}/approve"),
        Some(&admin_token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);

    let reject_id: Uuid = sqlx::query_scalar(
        "INSERT INTO faucet_sites (owner_id, name, url, description, coins, status) \
         VALUES ($1, 'Reject Me', 'https://reject.example', 'description long enough', \
                 ARRAY['BTC']::coin[], 'PENDING') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (st, _) = post_json(
        api_http::app_without_metrics(state.clone()),
        &format!("/v1/admin/faucetlist/{reject_id}/reject"),
        Some(&admin_token),
        serde_json::json!({ "reason": "spam" }),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);

    let (st, _) = post_json(
        api_http::app_without_metrics(state),
        &format!("/v1/admin/faucetlist/{approved_id}/suspend"),
        Some(&admin_token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_withdrawal_approve_reject(pool: PgPool) {
    db::house::ensure_house_inventory(&pool).await.unwrap();
    let (state, user_token, user_id, _) = common::register_user(pool.clone(), "wdadm").await;
    let uid = Uuid::parse_str(&user_id).unwrap();
    common::credit_personal(&pool, uid, shared::Coin::Pol, 50_000_000).await;

    let wd = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/withdrawals")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.52")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "toAddress": "0x1111111111111111111111111111111111111111",
                        "amount": "2000000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = wd.status();
    let bytes = wd.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::CREATED,
        "wd={}",
        String::from_utf8_lossy(&bytes)
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wd_id = v["id"].as_str().unwrap();

    let (_a, admin_token) = common::admin_login(pool.clone()).await;

    // Create a second withdrawal to reject
    common::credit_personal(&pool, uid, shared::Coin::Pol, 50_000_000).await;
    let wd2 = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/withdrawals")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-real-ip", "203.0.113.52")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "toAddress": "0x2222222222222222222222222222222222222222",
                        "amount": "2000000",
                        "idempotencyKey": "reject-me-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = wd2.into_body().collect().await.unwrap().to_bytes();
    let v2: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wd2_id = v2["id"].as_str().expect("wd2 id");

    let (st, body) = post_json(
        api_http::app_without_metrics(state.clone()),
        &format!("/v1/admin/withdrawals/{wd_id}/approve"),
        Some(&admin_token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");

    let (st, body) = post_json(
        api_http::app_without_metrics(state),
        &format!("/v1/admin/withdrawals/{wd2_id}/reject"),
        Some(&admin_token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "{body}");
}
