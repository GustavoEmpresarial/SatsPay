//! Argon2id password hashing with explicit production parameters.
//!
//! OWASP / PHC guidance for interactive logins (not the crate `default()`,
//! which is too weak for a financial system):
//! - Algorithm: Argon2id
//! - Memory: 64 MiB
//! - Iterations: 3
//! - Parallelism: 4
//! - Output: 32 bytes

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("failed to hash password")]
    HashFailed,
    #[error("stored hash is malformed")]
    InvalidHash,
}

/// Production Argon2id hasher — never use `Argon2::default()` for new hashes.
pub fn production_argon2() -> Argon2<'static> {
    let params = Params::new(
        65_536, // 64 MiB
        3,      // iterations
        4,      // lanes / parallelism
        Some(32),
    )
    .expect("Argon2id production params are within crate limits");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub fn hash_password(plaintext: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    production_argon2()
        .hash_password(plaintext.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| PasswordError::HashFailed)
}

pub fn verify_password(plaintext: &str, stored_hash: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(stored_hash).map_err(|_| PasswordError::InvalidHash)?;
    // Verify with the same algorithm params encoded in the PHC string.
    // Argon2::default() can still verify old hashes; production_argon2()
    // also verifies any Argon2id PHC as long as params decode.
    Ok(production_argon2()
        .verify_password(plaintext.as_bytes(), &parsed)
        .is_ok())
}
