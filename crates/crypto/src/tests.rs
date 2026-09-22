use crate::{
    bootstrap_hot_mnemonic, ct_eq_str, encrypt_hot_mnemonic, hash_password, sha256_hex,
    verify_password, HotMnemonicError, JwtService, SecretsService,
};
use serde::{Deserialize, Serialize};

fn test_key() -> &'static str {
    // 64 hex chars = 32 raw bytes.
    "0000000000000000000000000000000000000000000000000000000000000000"
}

fn jwt_secret() -> &'static str {
    "test-jwt-secret-32bytes-minimum!!"
}

#[test]
fn secrets_encrypt_decrypt_round_trip() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    let plaintext = "super secret withdrawal address";
    let ct = svc.encrypt(plaintext);
    assert_ne!(ct, plaintext);
    let decrypted = svc.decrypt(&ct).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn secrets_decrypt_rejects_tampered_ciphertext() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    let mut ct = svc.encrypt("hello").into_bytes();
    let last = ct.len() - 1;
    ct[last] ^= 0xFF;
    let corrupted = String::from_utf8_lossy(&ct).to_string();
    assert!(svc.decrypt(&corrupted).is_err());
}

#[test]
fn hmac_is_deterministic_and_key_scoped() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    assert_eq!(svc.hmac_hex("api-key-123"), svc.hmac_hex("api-key-123"));
    assert_ne!(svc.hmac_hex("api-key-123"), sha256_hex("api-key-123"));
    assert!(svc.verify_hmac_hex("api-key-123", &svc.hmac_hex("api-key-123")));
    assert!(!svc.verify_hmac_hex("api-key-123", &svc.hmac_hex("other")));
}

#[test]
fn refresh_token_hash_is_keyed_and_prefixed() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    let h = svc.refresh_token_hash("refresh-raw");
    assert!(h.starts_with("v2:"));
    assert_ne!(h, format!("v2:{}", sha256_hex("refresh-raw")));
    assert_ne!(h, format!("v2:{}", svc.hmac_hex("refresh-raw")));
    let [v2, legacy] = svc.refresh_token_lookup_hashes("refresh-raw");
    assert_eq!(v2, h);
    assert_eq!(legacy, sha256_hex("refresh-raw"));
}

#[test]
fn aad_binds_ciphertext_and_legacy_empty_still_opens() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    let a = b"api_key:11111111-1111-1111-1111-111111111111";
    let b = b"api_key:22222222-2222-2222-2222-222222222222";
    let bound = svc.encrypt_with_aad("raw-secret", a);
    assert_eq!(svc.decrypt_with_aad(&bound, a).unwrap(), "raw-secret");
    assert!(svc.decrypt_with_aad(&bound, b).is_err());
    // New ciphertext must not open with empty AAD via decrypt() (no fallback).
    assert!(svc.decrypt(&bound).is_err());

    let legacy = svc.encrypt("legacy-secret");
    assert_eq!(svc.decrypt(&legacy).unwrap(), "legacy-secret");
    assert_eq!(svc.decrypt_with_aad(&legacy, a).unwrap(), "legacy-secret");
}

#[test]
fn webhook_signing_secret_is_stable_per_merchant() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    let a = "11111111-1111-1111-1111-111111111111";
    let b = "22222222-2222-2222-2222-222222222222";
    assert_eq!(svc.webhook_signing_secret(a), svc.webhook_signing_secret(a));
    assert_ne!(svc.webhook_signing_secret(a), svc.webhook_signing_secret(b));
    assert_ne!(svc.webhook_signing_secret(a), svc.hmac_hex(&format!("merchant:{a}")));
}

#[test]
fn pii_seal_binds_aad_and_legacy_plaintext_opens() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    let sealed = svc.seal_pii("invoice.callback", "m1:ord1", "https://shop.example/hook");
    assert!(sealed.starts_with(crate::PII_PREFIX));
    assert_eq!(svc.open_pii("invoice.callback", "m1:ord1", &sealed), "https://shop.example/hook");
    assert_ne!(svc.open_pii("invoice.callback", "m2:ord1", &sealed), "https://shop.example/hook");
    assert_eq!(svc.open_pii("invoice.callback", "x", "https://plain.example"), "https://plain.example");
    assert_eq!(svc.email_index("A@B.com"), svc.email_index(" a@b.com "));
    assert_ne!(svc.email_index("a@b.com"), svc.hmac_hex("a@b.com"));
    assert!(crate::validate_api_scopes(&["deposits".into(), "balance".into()]).is_ok());
    assert!(crate::validate_api_scopes(&["nfts".into()]).is_err());
    let ip = svc.ip_fingerprint("203.0.113.9");
    assert_ne!(ip, "203.0.113.9");
    assert_eq!(ip, svc.ip_fingerprint("203.0.113.9"));
    assert_ne!(ip, svc.ip_fingerprint("203.0.113.10"));
}

#[test]
fn webhook_payload_verify_rejects_tamper() {
    let svc = SecretsService::from_hex(test_key()).unwrap();
    let merchant = "11111111-1111-1111-1111-111111111111";
    let payload = r#"{"event":"deposit.confirmed","invoiceId":"inv-1"}"#;
    let sig = svc.sign_webhook_payload(merchant, payload);
    assert!(svc.verify_webhook_payload(merchant, payload, &sig));
    assert!(!svc.verify_webhook_payload(merchant, r#"{"event":"tampered"}"#, &sig));
    assert!(!svc.verify_webhook_payload(
        "22222222-2222-2222-2222-222222222222",
        payload,
        &sig
    ));
}

#[test]
fn password_hash_uses_production_argon2id_params() {
    let hash = hash_password("correct horse battery staple").unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(hash.contains("m=65536"), "expected 64MiB memory: {hash}");
    assert!(hash.contains("t=3"), "expected 3 iterations: {hash}");
    assert!(hash.contains("p=4"), "expected parallelism 4: {hash}");
    assert!(verify_password("correct horse battery staple", &hash).unwrap());
    assert!(!verify_password("wrong password", &hash).unwrap());
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Claims {
    sub: String,
    iss: String,
    aud: String,
    iat: i64,
    exp: i64,
}

#[test]
fn jwt_sign_and_verify_round_trip() {
    let svc = JwtService::new(jwt_secret()).unwrap();
    let claims = Claims {
        sub: "user-1".into(),
        iss: svc.config.issuer.clone(),
        aud: svc.config.audience.clone(),
        iat: 1_700_000_000,
        exp: 9_999_999_999,
    };
    let token = svc.sign(&claims).unwrap();
    let decoded: Claims = svc.verify(&token).unwrap();
    assert_eq!(decoded, claims);
}

#[test]
fn jwt_rejects_weak_secret_wrong_aud_and_expired() {
    assert!(JwtService::new("too-short").is_err());

    let a = JwtService::new(jwt_secret()).unwrap();
    let b = JwtService::new("other-jwt-secret-32bytes-min!!!!").unwrap();
    let claims = Claims {
        sub: "u".into(),
        iss: a.config.issuer.clone(),
        aud: a.config.audience.clone(),
        iat: 1,
        exp: 9_999_999_999,
    };
    let token = a.sign(&claims).unwrap();
    assert!(b.verify::<Claims>(&token).is_err());

    let bad_aud = Claims {
        aud: "wrong-audience".into(),
        ..claims.clone()
    };
    let tok = a.sign(&bad_aud).unwrap();
    assert!(a.verify::<Claims>(&tok).is_err());

    let expired = Claims {
        exp: 1,
        ..claims
    };
    let tok = a.sign(&expired).unwrap();
    assert!(a.verify::<Claims>(&tok).is_err());
    assert!(a.verify::<Claims>("not.a.jwt").is_err());
}

#[test]
fn secrets_from_hex_rejects_bad_length() {
    assert!(SecretsService::from_hex("abcd").is_err());
    assert!(SecretsService::from_hex(test_key()).is_ok());
}

#[test]
fn sha256_hex_is_stable() {
    assert_eq!(
        sha256_hex("bitcosats"),
        "8c1b953bb48f6cb9c60175f65c315a2a0905a9933c353c6272df57015908665c"
    );
    assert_eq!(
        sha256_hex(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn secrets_decrypt_error_paths_and_random_token() {
    use base64::Engine;
    let svc = SecretsService::from_hex(test_key()).unwrap();
    assert!(svc.decrypt("!!!").is_err());
    assert!(svc.decrypt("").is_err());
    let short = base64::engine::general_purpose::STANDARD.encode(b"short");
    assert!(svc.decrypt(&short).is_err());
    let tok = crate::random_token(16);
    assert_eq!(tok.len(), 32);
}

#[test]
fn ct_eq_str_matches_equal_only() {
    assert!(ct_eq_str("abc", "abc"));
    assert!(!ct_eq_str("abc", "abd"));
    assert!(!ct_eq_str("abc", "ab"));
}

#[test]
fn hot_mnemonic_enc_round_trip_and_production_plaintext_forbidden() {
    use std::sync::Mutex;
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap();

    let svc = SecretsService::from_hex(test_key()).unwrap();
    let mnemonic =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let enc = encrypt_hot_mnemonic(&svc, mnemonic);

    std::env::remove_var("HOT_MNEMONIC");
    std::env::remove_var("HOT_MNEMONIC_ENC");
    std::env::set_var("NODE_ENV", "production");
    assert!(matches!(
        bootstrap_hot_mnemonic(&svc),
        Ok(None)
    ));

    std::env::set_var("HOT_MNEMONIC", mnemonic);
    assert!(matches!(
        bootstrap_hot_mnemonic(&svc),
        Err(HotMnemonicError::PlaintextForbiddenInProduction)
    ));
    std::env::remove_var("HOT_MNEMONIC");

    std::env::set_var("HOT_MNEMONIC_ENC", &enc);
    let loaded = bootstrap_hot_mnemonic(&svc).unwrap().unwrap();
    assert_eq!(loaded, mnemonic);

    std::env::remove_var("HOT_MNEMONIC_ENC");
    std::env::set_var("NODE_ENV", "development");
    std::env::set_var("HOT_MNEMONIC", mnemonic);
    let loaded = bootstrap_hot_mnemonic(&svc).unwrap().unwrap();
    assert_eq!(loaded, mnemonic);
    std::env::remove_var("HOT_MNEMONIC");
    std::env::remove_var("NODE_ENV");
}
