use captcha::{TurnstileConfig, TurnstileVerifier, VerifyOptions};
use chrono::Utc;
use sqlx::PgPool;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn verifier(url: String) -> TurnstileVerifier {
    TurnstileVerifier::new(TurnstileConfig {
        secret: "test-secret".into(),
        node_env: "test".into(),
        timeout: Duration::from_secs(5),
        max_token_age: Duration::from_secs(600),
        siteverify_url: Some(url),
    })
}

#[sqlx::test(migrations = "../db/migrations")]
async fn verify_success_reserves_token(pool: PgPool) {
    let server = MockServer::start().await;
    let body = format!(
        r#"{{"success":true,"error-codes":[],"action":"login","challenge_ts":"{}","hostname":"localhost"}}"#,
        Utc::now().to_rfc3339()
    );
    Mock::given(method("POST"))
        .and(path("/siteverify"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    let v = verifier(format!("{}/siteverify", server.uri()));
    let hosts = vec!["localhost".into()];
    assert!(v
        .verify(
            &pool,
            "token-one",
            Some("203.0.113.1"),
            VerifyOptions {
                expected_action: Some("login"),
                expected_hostnames: &hosts,
            },
        )
        .await
        .unwrap());
    assert!(!v
        .verify(
            &pool,
            "token-one",
            Some("203.0.113.1"),
            VerifyOptions {
                expected_action: Some("login"),
                expected_hostnames: &hosts,
            },
        )
        .await
        .unwrap());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn verify_http_error_fails_closed(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/siteverify"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let v = verifier(format!("{}/siteverify", server.uri()));
    assert!(!v
        .verify(
            &pool,
            "tok",
            None,
            VerifyOptions {
                expected_action: None,
                expected_hostnames: &[],
            },
        )
        .await
        .unwrap());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn verify_success_false_fails_closed(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/siteverify"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"success":false,"error-codes":["invalid-input-response"]}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let v = verifier(format!("{}/siteverify", server.uri()));
    assert!(!v
        .verify(
            &pool,
            "bad",
            None,
            VerifyOptions {
                expected_action: None,
                expected_hostnames: &[],
            },
        )
        .await
        .unwrap());
}
