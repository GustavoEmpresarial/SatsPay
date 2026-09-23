pub mod bch_sign;
pub mod bitcore_client;
pub mod btc_sign;
pub mod dgb_client;
pub mod encoding;
pub mod evm_client;
pub mod evm_sign;
pub mod hd;
pub mod hd_wallet;
pub mod params;
pub mod policy;
pub mod real_client;
pub mod registry;
pub mod sol_client;
pub mod rpc_client;
pub mod stub;
pub mod types;
pub mod utxo_node_rpc;
pub mod wallet_config;
pub mod zer_client;

pub use params::{params, params_for, AddressKind, ChainNetwork, CoinParams};
pub use policy::{assert_stub_client_allowed, StubPolicyError};
pub use real_client::{
    address_from_pubkey, hot_wallet_address, RealChainClient, RealClientConfig, DEPOSIT_ADDRESS_POOL_EMPTY, SIGNER_NOT_AVAILABLE,
};
pub use registry::ChainRegistry;
pub use rpc_client::MultiChainRpcClient;
pub use stub::StubClient;
pub use wallet_config::{verify_signer_matches, PublicWalletConfig, SignerKeys, WalletConfigError};
pub use types::{
    BroadcastError, BroadcastResult, ChainClient, ChainError, EvmContractCall, GeneratedAddress, OnchainTx,
};

#[cfg(test)]
mod tests;
