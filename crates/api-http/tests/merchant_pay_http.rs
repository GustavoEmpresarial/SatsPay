//! Merchant pay-with-balance, simulate, test-webhook, x-api-key create.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use shared::Coin;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn merchant_pay_simulate_webhook_apikey(pool: PgPool) {
    let (state, merch_token, merch_id, _) = common::register_user(pool.clone(), "mpay").await;
    let merch_uid = Uuid::parse_str(&merch_id).unwrap();

    // Ensure MERCHANT wallet exists for credits
    let _: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, 'POL', 'MERCHANT') \
         ON CONFLICT (user_id, coin, kind) DO UPDATE SET updated_at = now() RETURNING id",
    )
    .bind(merch_uid)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| Uuid::nil());
    let _: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = 'POL' AND kind = 'MERCHANT'",
    )
    .bind(merch_uid)
    .fetch_optional(&state.pool)
    .await
    .unwrap()
    .unwrap_or(Uuid::nil());

    let create = |state: api_http::AppState<db::auth::PgAuthRepo>,
                  token: String,
                  order: String| async move {
        let r = api_http::app_without_metrics(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/merchant/deposits/create")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-real-ip", "203.0.113.86")
                    .body(Body::from(
                        serde_json::json!({
                            "coin": "POL",
                            "amount": "500000",
                            "orderId": order,
                            "callbackUrl": "https://merchant.example/hook",
                            "siteName": "Shop"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let st = r.status();
        let bytes = r.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            st,
            axum::http::StatusCode::CREATED,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        inv["id"].as_str().unwrap().to_string()
    };

    let inv1 = create(state.clone(), merch_token.clone(), format!("o-{}", Uuid::new_v4())).await;

    // Payer with balance
    let (state_p, payer_token, payer_id, _) = common::register_user(pool.clone(), "payer").await;
    let payer_uid = Uuid::parse_str(&payer_id).unwrap();
    common::credit_personal(&state_p.pool, payer_uid, Coin::Pol, 5_000_000).await;

    let pay = api_http::app_without_metrics(state_p.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/public/pay/{inv1}/balance"))
                .header("authorization", format!("Bearer {payer_token}"))
                .header("x-real-ip", "203.0.113.87")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = pay.status();
    let bytes = pay.into_body().collect().await.unwrap().to_bytes();
    assert!(
        st.is_success(),
        "pay balance={} {}",
        st,
        String::from_utf8_lossy(&bytes)
    );

    let inv2 = create(state.clone(), merch_token.clone(), format!("o-{}", Uuid::new_v4())).await;
    let sim = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/public/pay/{inv2}/simulate-payment"))
                .header("x-real-ip", "203.0.113.86")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = sim.status();
    let bytes = sim.into_body().collect().await.unwrap().to_bytes();
    assert!(
        st.is_success(),
        "simulate={} {}",
        st,
        String::from_utf8_lossy(&bytes)
    );

    let inv3 = create(state.clone(), merch_token.clone(), format!("o-{}", Uuid::new_v4())).await;
    let hook = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/v1/merchant/deposits/{inv3}/test-webhook"))
                .header("authorization", format!("Bearer {merch_token}"))
                .header("x-real-ip", "203.0.113.86")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        hook.status().is_success() || hook.status().is_client_error(),
        "webhook={}",
        hook.status()
    );

    // Create via x-api-key
    let issue = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/api-keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {merch_token}"))
                .header("x-real-ip", "203.0.113.86")
                .body(Body::from(
                    serde_json::json!({
                        "label": "merch-key",
                        "scopes": ["*"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = issue.into_body().collect().await.unwrap().to_bytes();
    let issued: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let api_key = issued["key"].as_str().expect("key");

    let via_key = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/merchant/deposits/create")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .header("x-real-ip", "203.0.113.86")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "POL",
                        "amount": "100000",
                        "orderId": format!("okey-{}", Uuid::new_v4()),
                        "callbackUrl": "https://merchant.example/hook2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = via_key.status();
    let bytes = via_key.into_body().collect().await.unwrap().to_bytes();
    assert!(
        st.is_success() || st == axum::http::StatusCode::CREATED,
        "apikey create={} {}",
        st,
        String::from_utf8_lossy(&bytes)
    );
}
