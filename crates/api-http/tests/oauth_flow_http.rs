//! OAuth PKCE approve → token → userinfo + deny + revoke.

mod common;

use axum::body::Body;
use base64::Engine;
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;

fn pkce_pair() -> (String, String) {
    // 43-char verifier (charset unreserved)
    let verifier = "a".repeat(43);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    (verifier, challenge)
}

#[sqlx::test(migrations = "../db/migrations")]
async fn oauth_approve_token_userinfo_deny_revoke(pool: PgPool) {
    let (state, token, _, _) = common::register_user(pool, "oflow").await;

    let create = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/apps")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.81")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Flow App",
                        "redirect_uris": ["https://app.example/cb"]
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
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // OauthAppCreated is flattened or nested
    let client_id = created["client_id"]
        .as_str()
        .or_else(|| created["app"]["client_id"].as_str())
        .expect("client_id")
        .to_string();
    let client_secret = created["client_secret"]
        .as_str()
        .or_else(|| created["app"]["client_secret"].as_str())
        .expect("client_secret")
        .to_string();
    let app_id = created["id"]
        .as_str()
        .or_else(|| created["app"]["id"].as_str())
        .expect("app id")
        .to_string();

    let (verifier, challenge) = pkce_pair();
    let redirect = "https://app.example/cb";

    // Deny
    let deny = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/authorize")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.81")
                .body(Body::from(
                    serde_json::json!({
                        "client_id": client_id,
                        "redirect_uri": redirect,
                        "decision": "deny",
                        "state": "s1",
                        "code_challenge": challenge,
                        "code_challenge_method": "S256"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = deny.status();
    let bytes = deny.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(st, axum::http::StatusCode::OK);
    let d: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        d["redirect_url"]
            .as_str()
            .unwrap_or("")
            .contains("access_denied"),
        "{d}"
    );

    // Approve
    let approve = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/authorize")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.81")
                .body(Body::from(
                    serde_json::json!({
                        "client_id": client_id,
                        "redirect_uri": redirect,
                        "decision": "approve",
                        "scope": "openid profile email",
                        "state": "s2",
                        "code_challenge": challenge,
                        "code_challenge_method": "S256"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = approve.status();
    let bytes = approve.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let a: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let redirect_url = a["redirect_url"].as_str().expect("redirect_url");
    let code = redirect_url
        .split("code=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .expect("code in redirect");

    // Token exchange (form)
    let form = format!(
        "grant_type=authorization_code&code={code}&client_id={client_id}&client_secret={client_secret}&redirect_uri={}&code_verifier={verifier}",
        urlencoding_simple(redirect)
    );
    let tok = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/oauth/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("x-real-ip", "203.0.113.81")
                .body(Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = tok.status();
    let bytes = tok.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "token={}",
        String::from_utf8_lossy(&bytes)
    );
    let tr: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let access = tr["access_token"].as_str().expect("access_token");

    let userinfo = api_http::app_without_metrics(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/oauth/userinfo")
                .header("authorization", format!("Bearer {access}"))
                .header("x-real-ip", "203.0.113.81")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        userinfo.status(),
        axum::http::StatusCode::OK,
        "userinfo"
    );

    let revoked = api_http::app_without_metrics(state)
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/oauth/authorized-apps/{app_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("x-real-ip", "203.0.113.81")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        revoked.status().is_success() || revoked.status() == axum::http::StatusCode::NO_CONTENT,
        "revoke={}",
        revoked.status()
    );
}

fn urlencoding_simple(s: &str) -> String {
    s.replace(':', "%3A").replace('/', "%2F")
}
