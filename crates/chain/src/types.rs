//! Port 1:1 of legacy `apps/api/src/integrations/types.ts`.

use async_trait::async_trait;
use shared::Coin;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct OnchainTx {
    pub tx_hash: String,
    pub vout: u32,
    pub amount: u128,
    pub confirmations: u32,
    pub address: String,
}

#[derive(Debug, Clone)]
pub struct GeneratedAddress {
    pub address: String,
    pub hd_index: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct BroadcastResult {
    pub tx_hash: String,
    /// Network fee paid by the platform hot wallet (not debited from the user).
    pub fee_amount: u128,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ChainError {
    pub message: String,
}

/// `safe_to_reverse` must be true ONLY when the client is certain that no
/// transaction left the hot wallet (e.g. pre-flight validation failure).
/// Timeouts, RPC disconnects, and ambiguous node errors MUST leave it false
/// so the app does not re-credit the user while funds may still confirm on-chain.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct BroadcastError {
    pub message: String,
    pub safe_to_reverse: bool,
}

/// Contract for a chain-specific client. Implement one per supported coin.
/// The stub bundled here (`stub::StubClient`) returns deterministic
/// placeholder data — swap for real node/API calls before touching mainnet
/// funds (see `registry::build_client`).
#[async_trait]
pub trait ChainClient: Send + Sync {
    fn coin(&self) -> Coin;
    async fn generate_address(&self, user_id: &str) -> Result<GeneratedAddress, ChainError>;
    fn validate_address(&self, address: &str) -> bool;
    async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError>;
    async fn broadcast_withdrawal(&self, to_address: &str, amount: u128) -> Result<BroadcastResult, BroadcastError>;
    async fn get_balance(&self, address: &str) -> Result<u128, ChainError>;
    /// Move all spendable funds at `hd_index` to the hot/withdrawal wallet.
    /// `Ok(None)` means nothing to sweep (dust, unsupported, or no key).
    async fn sweep_deposit_to_hot(&self, hd_index: u32) -> Result<Option<BroadcastResult>, BroadcastError> {
        let _ = hd_index;
        Ok(None)
    }
}
