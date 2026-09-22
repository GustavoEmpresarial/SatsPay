pub mod ct;
pub mod hot_mnemonic;
pub mod jwt;
pub mod password;
pub mod pii;
pub mod secrets;

pub use ct::{ct_eq, ct_eq_str};
pub use hot_mnemonic::{bootstrap_hot_mnemonic, encrypt_hot_mnemonic, HotMnemonicError};
pub use jwt::{JwtConfig, JwtError, JwtService, DEFAULT_JWT_AUDIENCE, DEFAULT_JWT_ISSUER};
pub use password::{hash_password, production_argon2, verify_password, PasswordError};
pub use pii::{validate_api_scopes, ALLOWED_API_SCOPES, PII_PREFIX};
pub use secrets::{random_token, sha256_hex, CryptoError, SecretsService};

#[cfg(test)]
mod tests;
