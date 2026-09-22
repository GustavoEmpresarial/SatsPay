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

/// Unsigned EVM contract-call fields from a DEX aggregator (SwapKit / 1inch / …).
#[derive(Debug, Clone)]
pub struct EvmContractCall {
    pub to: String,
    pub data_hex: String,
    pub value_wei: u128,
    pub gas_limit_hint: Option<u64>,
    pub gas_price_hint: Option<u128>,
    /// Expected `from` (hot wallet). Rejected if it does not match our key.
    pub from_hint: Option<String>,
    /// On-chain sell amount (token units) when spending an ERC-20 — used for approve.
    pub erc20_sell_amount: Option<u128>,
    /// SwapKit `meta.approvalAddress` — the spender that must be approved (required for ERC-20).
    pub approval_address: Option<String>,
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
    /// Sign + broadcast an EVM contract call (DEX router). Default: unsupported.
    async fn broadcast_evm_contract_call(&self, call: EvmContractCall) -> Result<BroadcastResult, BroadcastError> {
        let _ = call;
        Err(BroadcastError {
            message: format!("contractCall not supported for {}", self.coin().as_str()),
            safe_to_reverse: true,
        })
    }

    /// Sign + broadcast a Relay Solana step (`instructions` + ALTs). Default: unsupported.
    async fn broadcast_solana_relay_tx(&self, solana_tx: &serde_json::Value) -> Result<BroadcastResult, BroadcastError> {
        let _ = solana_tx;
        Err(BroadcastError {
            message: format!("solanaRelay not supported for {}", self.coin().as_str()),
            safe_to_reverse: true,
        })
    }
}
