//! API entrypoint — paridade com `legacy/apps/api/src/server.ts`.
//! Roda como um dos dois binários obrigatórios (o outro é `worker`).

use chain::ChainRegistry;
use db::auth::PgAuthRepo;
use domain::auth::{AuthConfig, AuthService, EmailSender, NoopEmailSender};
use std::sync::Arc;

fn required_env<T: std::str::FromStr>(name: &str) -> T {
    let raw = std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"));
    raw.parse().unwrap_or_else(|_| panic!("{name} has an invalid value: {raw:?}"))
}

/// `SMTP_ENABLED=true` wires a real `SmtpSender`; otherwise emails are
/// dropped with a warning log (`NoopEmailSender`) — same fail-visible
/// philosophy as `ALLOW_STUB_CHAIN`/`USE_REAL_CHAIN_CLIENTS`, so dev/test can
/// run without a mail server but production must opt in explicitly.
fn build_email_sender() -> Arc<dyn EmailSender> {
    let enabled = std::env::var("SMTP_ENABLED").map(|v| v == "true").unwrap_or(false);
    if !enabled {
        tracing::warn!("SMTP_ENABLED is not true — emails (OTP codes, notifications) will NOT be delivered");
        return Arc::new(NoopEmailSender);
    }
    let config = notify::SmtpConfig {
        host: std::env::var("SMTP_HOST").expect("SMTP_HOST must be set when SMTP_ENABLED=true"),
        port: required_env("SMTP_PORT"),
        username: std::env::var("SMTP_USERNAME").expect("SMTP_USERNAME must be set when SMTP_ENABLED=true"),
        password: std::env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set when SMTP_ENABLED=true"),
        from_address: std::env::var("SMTP_FROM_ADDRESS").expect("SMTP_FROM_ADDRESS must be set when SMTP_ENABLED=true"),
    };
    Arc::new(notify::SmtpSender::new(config).expect("failed to build SMTP transport"))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    db::telemetry::install_panic_hook("api-server");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::connect(&database_url).await.expect("failed to connect to Postgres");
    db::run_migrations(&pool).await.expect("failed to run migrations");
    db::house::ensure_house_inventory(&pool).await.expect("failed to ensure house inventory");
    db::lend::ensure_lend_reserves(&pool).await.expect("failed to ensure lend reserves");

    let jwt_secret = std::env::var("JWT_ACCESS_SECRET").expect("JWT_ACCESS_SECRET must be set");
    let admin_emails: Vec<String> = std::env::var("ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let auth_config = AuthConfig {
        jwt_access_ttl_secs: required_env("JWT_ACCESS_TTL_SECS"),
        jwt_refresh_ttl_secs: required_env("JWT_REFRESH_TTL_SECS"),
        admin_emails,
        house_email: db::house::HOUSE_EMAIL.to_string(),
        otp_code_ttl_secs: required_env("OTP_CODE_TTL_SECS"),
        otp_resend_cooldown_secs: required_env("OTP_RESEND_COOLDOWN_SECS"),
        otp_max_attempts: required_env("OTP_MAX_ATTEMPTS"),
        refresh_reuse_grace_secs: std::env::var("REFRESH_REUSE_GRACE_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60),
    };

    let auth_repo = Arc::new(PgAuthRepo::new(pool.clone()));
    let jwt = crypto::JwtService::new(&jwt_secret).expect("JWT_ACCESS_SECRET must be ≥32 bytes");
    let encryption_key = std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set (64 hex chars)");
    let secrets = Arc::new(
        crypto::SecretsService::from_hex(&encryption_key).expect("ENCRYPTION_KEY must be 64 hex chars"),
    );
    let hot_mnemonic = crypto::bootstrap_hot_mnemonic(&secrets)
        .expect("hot mnemonic bootstrap failed")
        .map(Arc::<str>::from);
    // Inject decrypted mnemonic for ChainRegistry / resolve helpers that still
    // read HOT_MNEMONIC from the process environment (never log this value).
    if let Some(ref m) = hot_mnemonic {
        std::env::set_var("HOT_MNEMONIC", m.as_ref());
    }
    let email_sender = build_email_sender();
    let auth_service = Arc::new(AuthService::new(
        auth_repo,
        jwt,
        secrets.clone(),
        auth_config,
        email_sender.clone(),
    ));

    let chain_registry = Arc::new(ChainRegistry::from_env(pool.clone()).expect("failed to build chain registry"));

    let node_env = std::env::var("NODE_ENV").unwrap_or_else(|_| "development".to_string());
    let captcha_secret = match std::env::var("TURNSTILE_SECRET") {
        Ok(s) if !s.trim().is_empty() => s,
        _ if node_env == "production" => {
            panic!("TURNSTILE_SECRET must be set in production (use a real Cloudflare secret)");
        }
        _ => "disabled_in_dev".to_string(),
    };
    let captcha_config = captcha::TurnstileConfig {
        secret: captcha_secret,
        node_env: node_env.clone(),
        timeout: std::time::Duration::from_secs(
            std::env::var("TURNSTILE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
        ),
        max_token_age: std::time::Duration::from_secs(
            std::env::var("TURNSTILE_MAX_TOKEN_AGE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600),
        ),
        siteverify_url: None,
    };
    let captcha_verifier = Arc::new(captcha::TurnstileVerifier::new(captcha_config));
    let mut captcha_expected_hostnames: Vec<String> = std::env::var("CORS_ORIGIN")
        .unwrap_or_default()
        .split(',')
        .filter_map(|o| url::Url::parse(o.trim()).ok().and_then(|u| u.host_str().map(str::to_string)))
        .collect();
    for host in &["satspay.pro", "www.satspay.pro", "blockminer.space", "127.0.0.1", "localhost"] {
        let h_str = host.to_string();
        if !captcha_expected_hostnames.contains(&h_str) {
            captcha_expected_hostnames.push(h_str);
        }
    }

    let settings = api_http::AppSettings {
        faucet_cooldown_minutes: required_env("FAUCET_COOLDOWN_MINUTES"),
        public_api_daily_send_limit: required_env("PUBLIC_API_DAILY_SEND_LIMIT"),
        captcha_expected_hostnames,
        lend_close_factor_bps: required_env("LEND_CLOSE_FACTOR_BPS"),
        price_max_stale: std::time::Duration::from_secs(required_env("PRICE_MAX_STALE_SECS")),
        public_api_signature_max_skew: std::time::Duration::from_secs(required_env("PUBLIC_API_SIGNATURE_MAX_SKEW_SECS")),
    };

    let state = api_http::AppState {
        auth: auth_service,
        email: email_sender,
        pool: pool.clone(),
        chain_registry,
        secrets,
        hot_mnemonic,
        captcha: captcha_verifier,
        settings,
        swapkit: Arc::new(swapkit::SwapKitClient::from_env()),
    };
    let app = api_http::app(state);
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:4000".to_string());
    tracing::info!(addr, "api-server listening");

    let listener = tokio::net::TcpListener::bind(&addr).await.expect("failed to bind");
    // ConnectInfo is required so `client_ip::ClientIp` can fall back to the
    // TCP peer when proxy headers are absent.
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await
        .expect("server error");
}
