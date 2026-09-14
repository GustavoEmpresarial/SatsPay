//! HS256 access tokens with hard issuer/audience/expiry validation.
//!
//! Financial-grade defaults:
//! - Secret must be ≥ 32 bytes of high entropy
//! - `iss` + `aud` required and validated
//! - `exp` required; `iat` recorded by callers

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// Default issuer claim for BitcoSats access tokens.
pub const DEFAULT_JWT_ISSUER: &str = "bitcosats";
/// Default audience for end-user access tokens.
pub const DEFAULT_JWT_AUDIENCE: &str = "bitcosats-api";

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("JWT secret must be at least 32 bytes")]
    WeakSecret,
    #[error("failed to encode token")]
    EncodeFailed,
    #[error("invalid or expired token")]
    InvalidToken,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub issuer: String,
    pub audience: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            issuer: DEFAULT_JWT_ISSUER.to_string(),
            audience: DEFAULT_JWT_AUDIENCE.to_string(),
        }
    }
}

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    pub config: JwtConfig,
}

impl JwtService {
    pub fn new(secret: &str) -> Result<Self, JwtError> {
        Self::with_config(secret, JwtConfig::default())
    }

    pub fn with_config(secret: &str, config: JwtConfig) -> Result<Self, JwtError> {
        if secret.as_bytes().len() < 32 {
            return Err(JwtError::WeakSecret);
        }
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[config.issuer.clone()]);
        validation.set_audience(&[config.audience.clone()]);
        validation.validate_exp = true;
        validation.required_spec_claims.insert("exp".to_string());
        validation.required_spec_claims.insert("iss".to_string());
        validation.required_spec_claims.insert("aud".to_string());

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            validation,
            config,
        })
    }

    pub fn sign<T: Serialize>(&self, claims: &T) -> Result<String, JwtError> {
        let header = Header::new(Algorithm::HS256);
        encode(&header, claims, &self.encoding_key).map_err(|_| JwtError::EncodeFailed)
    }

    pub fn verify<T: DeserializeOwned>(&self, token: &str) -> Result<T, JwtError> {
        decode::<T>(token, &self.decoding_key, &self.validation)
            .map(|data| data.claims)
            .map_err(|_| JwtError::InvalidToken)
    }
}
