use captcha::TurnstileVerifier;
use chain::ChainRegistry;
use crypto::SecretsService;
use domain::auth::{AuthRepo, AuthService, EmailSender};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use swapkit::SwapKitClient;

/// App-level settings that would otherwise show up as magic numbers scattered
/// through route handlers — all sourced from env at boot (`api-server::main`),
/// never invented inline.
#[derive(Clone)]
pub struct AppSettings {
    pub faucet_cooldown_minutes: i64,
    pub public_api_daily_send_limit: i32,
    pub captcha_expected_hostnames: Vec<String>,
    /// Max fraction of a borrower's debt a single liquidation may repay —
    /// legacy hardcoded 5000 (50%); kept tunable here.
    pub lend_close_factor_bps: u32,
    /// Max age of a cached CoinGecko price still accepted for a swap quote —
    /// beyond this, swap fails closed instead of using a stale/invented price.
    pub price_max_stale: Duration,
    /// Max clock skew accepted between an HMAC-signed public API request's
    /// `x-timestamp` and server time — legacy fixed 5 minutes, kept tunable.
    pub public_api_signature_max_skew: Duration,
    /// `SMTP_ENABLED=true` — when set, user withdrawals require a fresh email
    /// OTP even if the account has not enabled 2FA. Admin withdrawal approve
    /// always requires step-up OTP regardless of this flag.
    pub smtp_enabled: bool,
    /// Public origin the hosted checkout is reachable at (no trailing slash) —
    /// used to build the absolute `checkoutUrl` merchants redirect to. From
    /// `PUBLIC_BASE_URL`, else the first `CORS_ORIGIN` entry.
    pub public_base_url: String,
}

/// Shared axum state — auth service (generic over its repo) plus raw
/// dependencies the wallet/deposits routes call `db::*`/`chain::*` with
/// directly, since those modules are transactional orchestration living in
/// `db`, not behind a `domain` trait (see `db::wallet`/`db::deposits` docs).
pub struct AppState<R: AuthRepo> {
    pub auth: Arc<AuthService<R>>,
    /// Same sender wired into `AuthService` for OTP — also used for
    /// best-effort merchant / faucetlist / public-API notification emails
    /// from HTTP handlers (db stays free of SMTP deps).
    pub email: Arc<dyn EmailSender>,
    pub pool: PgPool,
    pub chain_registry: Arc<ChainRegistry>,
    pub secrets: Arc<SecretsService>,
    /// Decrypted hot mnemonic held only in process memory (from
    /// `HOT_MNEMONIC_ENC` in production). Never log or serialize this.
    pub hot_mnemonic: Option<Arc<str>>,
    pub captcha: Arc<TurnstileVerifier>,
    pub settings: AppSettings,
    pub swapkit: Arc<SwapKitClient>,
}

// Manual impl: `#[derive(Clone)]` would incorrectly require `R: Clone` even
// though `R` only ever appears behind an `Arc`.
impl<R: AuthRepo> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            auth: self.auth.clone(),
            email: self.email.clone(),
            pool: self.pool.clone(),
            chain_registry: self.chain_registry.clone(),
            secrets: self.secrets.clone(),
            hot_mnemonic: self.hot_mnemonic.clone(),
            captcha: self.captcha.clone(),
            settings: self.settings.clone(),
            swapkit: self.swapkit.clone(),
        }
    }
}
