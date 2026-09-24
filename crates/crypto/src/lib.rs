pub mod ct;
pub mod jwt;
pub mod password;
pub mod pii;
pub mod secret_bootstrap;
pub mod secrets;

pub use ct::{ct_eq, ct_eq_str};
pub use secret_bootstrap::{
    assert_no_signer_env, bootstrap_signer_secrets, encrypt_secret, read_env_or_file, SecretBootstrapError, SecretSpec,
    SecretValue, SignerSecrets,
};
pub use jwt::{JwtConfig, JwtError, JwtService, DEFAULT_JWT_AUDIENCE, DEFAULT_JWT_ISSUER};
pub use password::{hash_password, production_argon2, verify_password, PasswordError};
pub use pii::{validate_api_scopes, ALLOWED_API_SCOPES, PII_PREFIX};
pub use secrets::{random_token, sha256_hex, CryptoError, SecretsService};

#[cfg(test)]
mod tests;
