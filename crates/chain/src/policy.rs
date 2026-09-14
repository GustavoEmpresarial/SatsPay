//! Port 1:1 of legacy `apps/api/src/integrations/stubPolicy.ts`.
//! Guards the placeholder chain implementation from production use. Kept
//! pure so the production policy can be tested without initializing infra.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StubPolicyError {
    #[error(
        "No real chain client configured for {coin}. The stub chain client is forbidden in production, \
         regardless of ALLOW_STUB_CHAIN. Wire a real RPC client before starting production."
    )]
    ForbiddenInProduction { coin: String },
    #[error(
        "No real chain client configured for {coin}. Stub chain clients are disabled; \
         set ALLOW_STUB_CHAIN=true only in development or test."
    )]
    NotExplicitlyAllowed { coin: String },
}

pub fn assert_stub_client_allowed(node_env: &str, allow_stub_chain: bool, coin: &str) -> Result<(), StubPolicyError> {
    if node_env == "production" {
        return Err(StubPolicyError::ForbiddenInProduction { coin: coin.to_string() });
    }
    if !allow_stub_chain {
        return Err(StubPolicyError::NotExplicitlyAllowed { coin: coin.to_string() });
    }
    Ok(())
}
