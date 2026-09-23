//! Port of legacy `apps/api/src/integrations/registry.ts`, extended with a
//! real client path (`build_real`) now that `RealChainClient` exists.
//!
//! `build` (stub) still fails closed in production via `assert_stub_client_allowed`.
//! `build_real` is the production path: real HD address derivation, real
//! balance/deposit queries, and signed broadcasts for BTC/LTC/DOGE/BCH/POL.
use crate::params::ChainNetwork;
use crate::policy::{assert_stub_client_allowed, StubPolicyError};
use crate::real_client::{RealChainClient, RealClientConfig};
use crate::stub::StubClient;
use crate::types::ChainClient;
use crate::wallet_config::{PublicWalletConfig, SignerKeys, WalletConfigError};
use shared::{Coin, COINS};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ChainRegistry {
    clients: HashMap<&'static str, Arc<dyn ChainClient>>,
    wallet: PublicWalletConfig,
}

impl ChainRegistry {
    /// `node_env` should be `"production"` / `"development"` / `"test"`,
    /// mirroring `env.NODE_ENV` in the legacy service.
    pub fn build(node_env: &str, allow_stub_chain: bool) -> Result<Self, StubPolicyError> {
        let mut clients: HashMap<&'static str, Arc<dyn ChainClient>> = HashMap::new();
        for &coin in COINS.iter() {
            let client = build_stub_client(coin, node_env, allow_stub_chain)?;
            clients.insert(coin.as_str(), client);
        }
        Ok(Self { clients, wallet: PublicWalletConfig::default() })
    }

    /// Real-client registry — one `RealChainClient` per coin, sharing the
    /// same `pool` (for HD-index sequences) and `config` (endpoints/keys
    /// read from env by the caller, never hardcoded here).
    pub fn build_real(pool: PgPool, config: RealClientConfig) -> Self {
        let mut clients: HashMap<&'static str, Arc<dyn ChainClient>> = HashMap::new();
        for &coin in COINS.iter() {
            clients.insert(coin.as_str(), Arc::new(RealChainClient::new(coin, pool.clone(), config.clone())) as Arc<dyn ChainClient>);
        }
        Self { clients, wallet: config.wallet }
    }

    /// Replaces the public wallet config (hot addresses) — tests and stub setups.
    pub fn with_wallet(mut self, wallet: PublicWalletConfig) -> Self {
        self.wallet = wallet;
        self
    }

    /// Public hot address for `coin` from `HOT_ADDRESS_<COIN>`. The api-server
    /// never derives it: it holds no key to derive from.
    pub fn hot_address(&self, coin: Coin) -> Result<String, String> {
        self.wallet
            .hot_address(coin)
            .map(str::to_string)
            .ok_or_else(|| format!("HOT_ADDRESS_{} not configured", coin.as_str()))
    }

    /// Swaps in `client` for the coin it reports. For tests that need a
    /// broadcast outcome the stub can't produce (success, ambiguous failure).
    pub fn with_client(mut self, client: Arc<dyn ChainClient>) -> Self {
        self.clients.insert(client.coin().as_str(), client);
        self
    }

    pub fn get(&self, coin: Coin) -> Arc<dyn ChainClient> {
        self.clients
            .get(coin.as_str())
            .cloned()
            .expect("ChainRegistry::build/build_real populates every Coin variant")
    }

    /// api-server registry: watch-only. Reads **no** key material — deposit
    /// addresses come from `DEPOSIT_XPUB_<COIN>` (SOL from the worker's pool),
    /// and every signing path fails with `SIGNER_NOT_AVAILABLE`.
    pub fn from_env(pool: PgPool) -> Result<Self, String> {
        Self::from_env_inner(pool, SignerKeys::default())
    }

    /// Worker registry: same public config plus the signing keys, handed in
    /// already decrypted (never read from env here).
    pub fn from_env_with_signer(pool: PgPool, keys: SignerKeys) -> Result<Self, String> {
        Self::from_env_inner(pool, keys)
    }

    /// Chooses `build` (stub) vs `build_real` based on `USE_REAL_CHAIN_CLIENTS`
    /// and reads every endpoint setting from env. Shared by `api-server`
    /// and `worker` so the two binaries can't drift on how this decision is made.
    fn from_env_inner(pool: PgPool, keys: SignerKeys) -> Result<Self, String> {
        let node_env = std::env::var("NODE_ENV").unwrap_or_else(|_| "development".to_string());
        let use_real = std::env::var("USE_REAL_CHAIN_CLIENTS").map(|v| v == "true").unwrap_or(true);
        let production = node_env == "production" && use_real;
        let wallet = PublicWalletConfig::from_env(production).map_err(|e: WalletConfigError| format!("{}: {e}", e.code()))?;

        if !use_real {
            let allow_stub_chain = std::env::var("ALLOW_STUB_CHAIN").map(|v| v == "true").unwrap_or(false);
            return Self::build(&node_env, allow_stub_chain).map(|r| r.with_wallet(wallet)).map_err(|e| e.to_string());
        }

        let network = match std::env::var("CHAIN_NETWORK") {
            Ok(v) => ChainNetwork::parse(&v)?,
            Err(_) => ChainNetwork::Mainnet,
        };

        if keys.deposit_mnemonic.is_some() || keys.hot_mnemonic.is_some() || keys.hot_wallet_key.is_some() {
            crate::wallet_config::verify_signer_matches(&wallet, &keys, network).map_err(|e| format!("{}: {e}", e.code()))?;
        }

        let bitcore_base_url = std::env::var("BITCORE_API_BASE_URL").unwrap_or_else(|_| "https://api.bitcore.io".to_string());
        let evm_rpc_url = std::env::var("EVM_RPC_URL").unwrap_or_else(|_| "https://polygon-bor-rpc.publicnode.com".to_string());
        let bsc_rpc_url = std::env::var("BSC_RPC_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://bsc-rpc.publicnode.com".to_string());

        let evm_deposit_lookback_blocks: u64 = std::env::var("EVM_DEPOSIT_LOOKBACK_BLOCKS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        let fee_confirmation_target: u32 = std::env::var("FEE_CONFIRMATION_TARGET")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        Ok(Self::build_real(
            pool,
            RealClientConfig {
                bitcore_base_url,
                evm_rpc_url,
                bsc_rpc_url,
                wallet,
                hot_wallet_wif: keys.hot_wallet_key,
                evm_deposit_lookback_blocks,
                fee_confirmation_target,
                network,
                sol_rpc_url: std::env::var("SOL_RPC_URL").unwrap_or_else(|_| crate::sol_client::DEFAULT_SOL_RPC.to_string()),
                dgb_insight_url: std::env::var("DGB_INSIGHT_API").unwrap_or_else(|_| "https://digiexplorer.info/api".to_string()),
                dgb_rpc_url: std::env::var("DGB_RPC_URL").ok().filter(|s| !s.trim().is_empty()),
                zer_explorer_url: std::env::var("ZER_EXPLORER_API").unwrap_or_else(|_| "https://zerochain.info/api".to_string()),
                zer_explorer_api_key: std::env::var("ZER_EXPLORER_API_KEY").ok().filter(|s| !s.trim().is_empty()),
                zer_rpc_url: std::env::var("ZER_RPC_URL").ok().filter(|s| !s.trim().is_empty()),
                deposit_mnemonic: keys.deposit_mnemonic,
                hot_mnemonic: keys.hot_mnemonic,
            },
        ))
    }
}

fn build_stub_client(coin: Coin, node_env: &str, allow_stub_chain: bool) -> Result<Arc<dyn ChainClient>, StubPolicyError> {
    assert_stub_client_allowed(node_env, allow_stub_chain, coin.as_str())?;
    tracing::warn!(coin = coin.as_str(), "using STUB chain client — no real funds are moved");
    Ok(Arc::new(StubClient::new(coin)))
}
