//! Authenticated PERSONAL → DEVELOPER transfer.

mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn transfer_personal_to_developer(pool: PgPool) {
    let state = common::test_state(pool.clone());
    let email = format!("xfer-{}@bitcosats.test", Uuid::new_v4());
    let username = format!("u{}", &Uuid::new_v4().as_simple().to_string()[..12]);

    let reg = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .header("x-real-ip", "203.0.113.20")
                .body(Body::from(
                    serde_json::json!({
                        "email": email,
                        "username": username,
                        "password": "Password1234",
                        "confirmPassword": "Password1234",
                        "acceptTerms": true,
                        "captchaToken": "dev-bypass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status(), axum::http::StatusCode::CREATED);
    let bytes = reg.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = v["tokens"]["accessToken"].as_str().unwrap().to_string();
    let user_id = Uuid::parse_str(v["user"]["id"].as_str().unwrap()).unwrap();

    let wallet_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets WHERE user_id = $1 AND coin = 'BTC' AND kind = 'PERSONAL'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    {
        let mut tx = pool.begin().await.unwrap();
        db::ledger::apply_ledger_entry(
            &mut tx,
            db::ledger::LedgerCreditInput {
                reference_key: None,
                wallet_id,
                amount: bigdecimal::BigDecimal::from(5_000_000u64),
                ledger_type: "ADJUSTMENT",
                reference_id: None,
                reference_type: Some("TestSeed"),
                memo: Some("seed"),
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let response = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/wallet/transfer")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.20")
                .body(Body::from(
                    serde_json::json!({
                        "coin": "BTC",
                        "amount": "1000000",
                        "toDeveloper": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT, "body={}", String::from_utf8_lossy(&bytes));
}

