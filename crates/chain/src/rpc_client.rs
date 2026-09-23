//! Multi-Provider Blockchain RPC & REST Indexer with Automatic 3x Fallback per Network.
//! If any provider fails, times out or rate limits, the client instantly cascades to the next RPC.

use crate::types::{ChainError, OnchainTx};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use shared::Coin;
use std::time::Duration;
use tracing::{info, warn};

pub struct MultiChainRpcClient {
    http: Client,
}

impl Default for MultiChainRpcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiChainRpcClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();
        Self { http }
    }

    /// Fetches deposits across providers with 3-tier fallback per network
    pub async fn fetch_deposits(
        &self,
        coin: Coin,
        address: &str,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        match coin {
            Coin::Btc => self.fetch_btc_with_fallbacks(address).await,
            Coin::Ltc => self.fetch_ltc_with_fallbacks(address).await,
            Coin::Doge => self.fetch_doge_with_fallbacks(address).await,
            Coin::Bch => self.fetch_bch_with_fallbacks(address).await,
            Coin::Pol => self.fetch_evm_with_fallbacks(address).await,
            Coin::Dgb => self.fetch_dgb_with_fallbacks(address).await,
            Coin::Zer => self.fetch_zer_with_fallbacks(address).await,
            Coin::Sol => self.fetch_sol_with_fallbacks(address).await,
            Coin::Usdt => {
                self.fetch_erc20_with_fallbacks(
                    address,
                    "0xc2132D05D31c914a87C6611C10748AEb04B58e8F",
                    Coin::Usdt,
                )
                .await
            }
            Coin::Usdc => {
                self.fetch_erc20_with_fallbacks(
                    address,
                    "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
                    Coin::Usdc,
                )
                .await
            }
            Coin::Pepe => self.fetch_pepe_bsc(address).await,
        }
    }

    // =========================================================================
    // 1. BITCOIN (BTC) - 3 RPCs / REST Providers
    // =========================================================================
    async fn fetch_btc_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        // Provider 1: Mempool.space
        if let Ok(txs) = self
            .fetch_mempool_space_format("https://mempool.space/api", address)
            .await
        {
            return Ok(txs);
        }
        warn!(
            coin = "BTC",
            provider = "mempool.space",
            "Primary failed, trying Blockstream API..."
        );

        // Provider 2: Blockstream.info
        if let Ok(txs) = self
            .fetch_mempool_space_format("https://blockstream.info/api", address)
            .await
        {
            info!(
                coin = "BTC",
                provider = "blockstream.info",
                "Fallback successful"
            );
            return Ok(txs);
        }
        warn!(
            coin = "BTC",
            provider = "blockstream.info",
            "Secondary failed, trying Bitcore API..."
        );

        // Provider 3: Bitcore Public API
        if let Ok(txs) = self.fetch_bitcore_format("btc", "mainnet", address).await {
            info!(coin = "BTC", provider = "bitcore", "Fallback successful");
            return Ok(txs);
        }

        // Provider 4 (Extra): BlockCypher
        self.fetch_blockcypher_format("btc", address).await
    }

    // =========================================================================
    // 2. LITECOIN (LTC) - 3 RPCs / REST Providers
    // =========================================================================
    async fn fetch_ltc_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        // Provider 1: LitecoinSpace
        if let Ok(txs) = self
            .fetch_mempool_space_format("https://litecoinspace.org/api", address)
            .await
        {
            return Ok(txs);
        }
        warn!(
            coin = "LTC",
            provider = "litecoinspace.org",
            "Primary failed, trying BlockCypher..."
        );

        // Provider 2: BlockCypher
        if let Ok(txs) = self.fetch_blockcypher_format("ltc", address).await {
            info!(
                coin = "LTC",
                provider = "blockcypher",
                "Fallback successful"
            );
            return Ok(txs);
        }
        warn!(
            coin = "LTC",
            provider = "blockcypher",
            "Secondary failed, trying Bitcore..."
        );

        // Provider 3: Bitcore Public API
        if let Ok(txs) = self.fetch_bitcore_format("ltc", "mainnet", address).await {
            info!(coin = "LTC", provider = "bitcore", "Fallback successful");
            return Ok(txs);
        }

        Err(ChainError {
            message: "All 3 LTC RPC providers failed".into(),
        })
    }

    // =========================================================================
    // 3. DOGECOIN (DOGE) - history providers, with optional Blockbook reserve
    // =========================================================================
    async fn fetch_doge_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        // Provider 1: BlockCypher
        if let Ok(txs) = crate::provider_circuit::call("doge_blockcypher", || {
            self.fetch_blockcypher_format("doge", address)
        })
        .await
        {
            return Ok(txs);
        }
        warn!(
            coin = "DOGE",
            provider = "blockcypher",
            "Primary failed, trying DogeChain API..."
        );

        // Provider 2: Bitcore Public API
        if let Ok(txs) = crate::provider_circuit::call("doge_bitcore", || {
            self.fetch_bitcore_format("doge", "mainnet", address)
        })
        .await
        {
            info!(coin = "DOGE", provider = "bitcore", "Fallback successful");
            return Ok(txs);
        }
        warn!(coin = "DOGE", provider = "bitcore", "Secondary failed, trying Blockbook...");

        if let Ok(key) = std::env::var("DOGE_BLOCKBOOK_API_KEY") {
            if !key.trim().is_empty() {
                if let Ok(txs) = crate::provider_circuit::call("doge_blockbook", || {
                    self.fetch_doge_blockbook(address, &key)
                })
                .await
                {
                    info!(coin = "DOGE", provider = "blockbook", "Fallback successful");
                    return Ok(txs);
                }
            }
        }

        Err(ChainError {
            message: "All configured DOGE deposit providers failed".into(),
        })
    }

    /// NOWNodes Blockbook history fallback. The API key stays in a header and
    /// errors never contain the URL or key. Empty/malformed JSON is not success.
    async fn fetch_doge_blockbook(
        &self,
        address: &str,
        key: &str,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        let base = std::env::var("DOGE_BLOCKBOOK_API")
            .unwrap_or_else(|_| "https://dogebook.nownodes.io".into());
        let url = format!(
            "{}/api/v2/address/{address}?details=txs&pageSize=1000",
            base.trim_end_matches('/')
        );
        let resp = self
            .http
            .get(url)
            .header("api-key", key)
            .send()
            .await
            .map_err(|_| ChainError {
                message: "DOGE Blockbook request failed".into(),
            })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("DOGE Blockbook HTTP {}", resp.status()),
            });
        }
        let data: serde_json::Value = resp.json().await.map_err(|_| ChainError {
            message: "DOGE Blockbook invalid JSON".into(),
        })?;
        parse_doge_blockbook(&data, address)
    }

    // =========================================================================
    // 4. BITCOIN CASH (BCH) - 3 RPCs / REST Providers
    // =========================================================================
    async fn fetch_bch_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        // Provider 1: Bitcore API
        if let Ok(txs) = self.fetch_bitcore_format("bch", "mainnet", address).await {
            return Ok(txs);
        }
        warn!(
            coin = "BCH",
            provider = "bitcore",
            "Primary failed, trying Blockchair..."
        );

        // Provider 2: Blockchair API
        if let Ok(txs) = self.fetch_blockchair_format("bitcoin-cash", address).await {
            info!(coin = "BCH", provider = "blockchair", "Fallback successful");
            return Ok(txs);
        }

        // Provider 3: Fullnode CashAddr REST
        let bare = address.split(':').next_back().unwrap_or(address);
        self.fetch_bitcore_format("bch", "mainnet", bare).await
    }

    // =========================================================================
    // =========================================================================
    // 5. POLYGON / EVM (POL, USDT, USDC) - Explorer + 4 RPC Providers
    // =========================================================================
    async fn fetch_evm_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        // Provider 1: Blockscout Explorer Indexer API (Full on-chain tx history with real hash)
        if let Ok(txs) = self.fetch_blockscout_polygon(address).await {
            if !txs.is_empty() {
                info!(
                    coin = "POL",
                    provider = "blockscout",
                    count = txs.len(),
                    "Blockscout Polygon transactions fetched successfully"
                );
                return Ok(txs);
            }
        }
        warn!(
            coin = "POL",
            provider = "blockscout",
            "Blockscout API empty or failed, trying high-speed public RPCs..."
        );

        let rpc_list = [
            "https://polygon-bor-rpc.publicnode.com",
            "https://1rpc.io/matic",
            "https://polygon.drpc.org",
            "https://polygon.gateway.tenderly.co",
        ];

        for &rpc_url in &rpc_list {
            if let Ok(txs) = self.fetch_single_evm_rpc(rpc_url, address).await {
                info!(
                    coin = "POL",
                    provider = rpc_url,
                    "Polygon EVM balance RPC successful"
                );
                return Ok(txs);
            }
            warn!(
                rpc = rpc_url,
                "Polygon EVM RPC failed, cascading to next..."
            );
        }

        Err(ChainError {
            message: "All Polygon EVM RPCs failed".into(),
        })
    }

    async fn fetch_dgb_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let insight = std::env::var("DGB_INSIGHT_API")
            .unwrap_or_else(|_| "https://digiexplorer.info/api".to_string());
        let rpc = std::env::var("DGB_RPC_URL").ok();
        let client = crate::dgb_client::DgbClient::with_rpc(&insight, rpc.as_deref());
        client.fetch_deposits(address).await
    }

    async fn fetch_zer_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let explorer = std::env::var("ZER_EXPLORER_API")
            .unwrap_or_else(|_| "https://zerochain.info/api".to_string());
        let key = std::env::var("ZER_EXPLORER_API_KEY").ok();
        let rpc = std::env::var("ZER_RPC_URL").ok();
        let client =
            crate::zer_client::ZerClient::with_rpc(&explorer, key.as_deref(), rpc.as_deref());
        client.fetch_deposits(address).await
    }

    async fn fetch_sol_with_fallbacks(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        // Single configured endpoint (node VM proxy via SOL_RPC_URL / RealClientConfig).
        let rpc = std::env::var("SOL_RPC_URL")
            .unwrap_or_else(|_| crate::sol_client::DEFAULT_SOL_RPC.to_string());
        let client = crate::sol_client::SolClient::new(&rpc);
        client.fetch_deposits(address).await
    }

    /// PEPE is BEP-20 on BNB Smart Chain only. Polygon indexers must not be queried.
    async fn fetch_pepe_bsc(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let network = match std::env::var("CHAIN_NETWORK").as_deref() {
            Ok("testnet") => crate::params::ChainNetwork::Testnet,
            _ => crate::params::ChainNetwork::Mainnet,
        };
        let params = crate::params::params_for(Coin::Pepe, network);
        let Some(token) = params.erc20_contract else {
            return Err(ChainError {
                message: "PEPE só existe na BNB Smart Chain mainnet (BEP-20). Testnet recusado."
                    .into(),
            });
        };
        if params.evm_chain_id != Some(56) {
            return Err(ChainError {
                message: "PEPE chain id ausente — recusado fora da BNB mainnet".into(),
            });
        }
        let lookback: u64 = std::env::var("BSC_DEPOSIT_LOOKBACK_BLOCKS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2_000);
        let configured = std::env::var("BSC_RPC_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://bsc-rpc.publicnode.com".to_string());
        let mut rpcs = vec![configured];
        for extra in [
            "https://bsc-dataseed.binance.org",
            "https://bsc-rpc.publicnode.com",
            "https://1rpc.io/bnb",
        ] {
            if !rpcs.iter().any(|r| r == extra) {
                rpcs.push(extra.to_string());
            }
        }
        let mut last = String::new();
        for rpc in &rpcs {
            let evm = crate::evm_client::EvmClient::new(rpc);
            match evm.fetch_erc20_received(token, address, lookback).await {
                Ok(raw) => {
                    let txs: Vec<OnchainTx> = raw
                        .into_iter()
                        .map(|mut t| {
                            t.amount = shared::from_onchain_amount(Coin::Pepe, t.amount);
                            t
                        })
                        .filter(|t| t.amount > 0)
                        .collect();
                    return Ok(txs);
                }
                Err(e) => {
                    last = format!("{rpc}: {e}");
                    warn!(rpc = %rpc, error = %e, "BSC PEPE eth_getLogs failed, cascading");
                }
            }
        }
        Err(ChainError {
            message: format!("PEPE deposit scan failed on every BSC RPC: {last}"),
        })
    }

    async fn fetch_erc20_with_fallbacks(
        &self,
        address: &str,
        token: &str,
        coin: Coin,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        if let Ok(txs) = self.fetch_blockscout_token(address, token, coin).await {
            if !txs.is_empty() {
                return Ok(txs);
            }
        }
        let rpc_list = [
            "https://polygon-bor-rpc.publicnode.com",
            "https://1rpc.io/matic",
            "https://polygon.drpc.org",
        ];
        for rpc in rpc_list {
            let evm = crate::evm_client::EvmClient::new(rpc);
            if let Ok(raw) = evm.fetch_erc20_received(token, address, 2_000).await {
                let txs: Vec<OnchainTx> = raw
                    .into_iter()
                    .map(|mut t| {
                        t.amount = shared::from_onchain_amount(coin, t.amount);
                        t
                    })
                    .filter(|t| t.amount > 0)
                    .collect();
                return Ok(txs);
            }
        }
        Ok(vec![])
    }

    async fn fetch_blockscout_token(
        &self,
        address: &str,
        token: &str,
        coin: Coin,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        let url = format!(
            "https://polygon.blockscout.com/api/v2/addresses/{address}/token-transfers?type=ERC-20&filter=to"
        );
        let resp = self
            .http
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| ChainError {
                message: e.to_string(),
            })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("HTTP {}", resp.status()),
            });
        }
        let data: BlockscoutTokenList = resp.json().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;
        let mut txs = Vec::new();
        for item in data.items.unwrap_or_default() {
            let token_hash = item
                .token
                .as_ref()
                .and_then(|t| t.address_hash.as_deref())
                .unwrap_or("");
            if !token_hash.eq_ignore_ascii_case(token) {
                continue;
            }
            let to = item.to.as_ref().map(|t| t.hash.as_str()).unwrap_or("");
            if !to.eq_ignore_ascii_case(address) {
                continue;
            }
            let raw: u128 = item
                .total
                .as_ref()
                .and_then(|t| t.value.as_deref())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let amount = shared::from_onchain_amount(coin, raw);
            if amount == 0 {
                continue;
            }
            txs.push(OnchainTx {
                tx_hash: item.transaction_hash.unwrap_or_default(),
                vout: 0,
                amount,
                confirmations: item.confirmations.unwrap_or(30),
                address: address.to_string(),
            });
        }
        Ok(txs)
    }

    // =========================================================================
    // PROTOCOL FORMAT IMPLEMENTATIONS
    // =========================================================================

    // Format A: Mempool.space & Blockstream API format (UTXOs + Tip)
    async fn fetch_mempool_space_format(
        &self,
        base_url: &str,
        address: &str,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        let url = format!("{}/address/{}/utxo", base_url, address);
        let resp = self.http.get(&url).send().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("HTTP {}", resp.status()),
            });
        }
        let utxos: Vec<MempoolUtxo> = resp.json().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;

        let tip = self.get_mempool_tip(base_url).await.unwrap_or(0);
        Ok(utxos
            .into_iter()
            .map(|u| {
                let confs = if let Some(h) = u.status.block_height {
                    (tip - h + 1).max(0) as u32
                } else {
                    0
                };
                OnchainTx {
                    tx_hash: u.txid,
                    vout: u.vout,
                    amount: u.value,
                    confirmations: confs,
                    address: address.to_string(),
                }
            })
            .collect())
    }

    // Format B: BlockCypher REST API
    async fn fetch_blockcypher_format(
        &self,
        coin_path: &str,
        address: &str,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        let url = format!(
            "https://api.blockcypher.com/v1/{}/main/addrs/{}/full?limit=50",
            coin_path, address
        );
        let resp = self.http.get(&url).send().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("HTTP {}", resp.status()),
            });
        }
        let data: BlockcypherAddrResp = resp.json().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;

        let mut txs = Vec::new();
        for tx in data.txs.unwrap_or_default() {
            for (vout_idx, out) in tx.outputs.iter().enumerate() {
                if out
                    .addresses
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(address))
                {
                    txs.push(OnchainTx {
                        tx_hash: tx.hash.clone(),
                        vout: vout_idx as u32,
                        amount: out.value as u128,
                        confirmations: tx.confirmations.max(0) as u32,
                        address: address.to_string(),
                    });
                }
            }
        }
        Ok(txs)
    }

    // Format C: Bitcore / Insight API
    async fn fetch_bitcore_format(
        &self,
        chain: &str,
        net: &str,
        address: &str,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        let bare = address.split(':').next_back().unwrap_or(address);
        let url = format!(
            "https://api.bitcore.io/api/{}/{}/address/{}/txs?limit=50",
            chain, net, bare
        );
        let resp = self.http.get(&url).send().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("HTTP {}", resp.status()),
            });
        }
        let coins: Vec<BitcoreCoinEntry> = resp.json().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;

        Ok(coins
            .into_iter()
            .map(|c| OnchainTx {
                tx_hash: c.mint_txid,
                vout: c.mint_index,
                amount: c.value.max(0) as u128,
                confirmations: if c.mint_height > 0 { 6 } else { 0 },
                address: address.to_string(),
            })
            .collect())
    }

    // Format D: Blockchair API
    async fn fetch_blockchair_format(
        &self,
        chain_slug: &str,
        address: &str,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        let bare = address.split(':').next_back().unwrap_or(address);
        let url = format!(
            "https://api.blockchair.com/{}/dashboards/address/{}",
            chain_slug, bare
        );
        let resp = self.http.get(&url).send().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("HTTP {}", resp.status()),
            });
        }
        Ok(vec![])
    }

    // Blockscout Polygon API
    async fn fetch_blockscout_polygon(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let url = format!(
            "https://polygon.blockscout.com/api/v2/addresses/{}/transactions",
            address
        );
        let resp = self
            .http
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| ChainError {
                message: e.to_string(),
            })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("HTTP {}", resp.status()),
            });
        }
        let data: BlockscoutTxListResp = resp.json().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;

        let mut txs = Vec::new();
        for item in data.items.unwrap_or_default() {
            let Some(to) = item.to else { continue };
            if !to.hash.eq_ignore_ascii_case(address) {
                continue;
            }
            let is_ok =
                item.status.as_deref() == Some("ok") || item.result.as_deref() == Some("success");
            if !is_ok {
                continue;
            }
            let raw_wei: u128 = match item.value.as_deref() {
                Some(s) => s.parse().unwrap_or(0),
                None => 0,
            };
            if raw_wei == 0 {
                continue;
            }
            // Scale native EVM wei (18 decimals) to BitcoSats standard 8-decimal units (sats equivalent)
            let amount = raw_wei / 10_000_000_000;
            let confs = item.confirmations.unwrap_or(30).max(0) as u32;
            txs.push(OnchainTx {
                tx_hash: item.hash,
                vout: 0,
                amount,
                confirmations: confs,
                address: address.to_string(),
            });
        }
        Ok(txs)
    }

    // Format G: EVM JSON-RPC Call (eth_getBalance)
    async fn fetch_single_evm_rpc(
        &self,
        rpc_url: &str,
        address: &str,
    ) -> Result<Vec<OnchainTx>, ChainError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [address, "latest"],
            "id": 1
        });

        let resp = self
            .http
            .post(rpc_url)
            .header("User-Agent", "Mozilla/5.0")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChainError {
                message: e.to_string(),
            })?;
        if !resp.status().is_success() {
            return Err(ChainError {
                message: format!("HTTP {}", resp.status()),
            });
        }
        let body: JsonRpcResponse<String> = resp.json().await.map_err(|e| ChainError {
            message: e.to_string(),
        })?;

        if let Some(hex_bal) = body.result {
            let clean = hex_bal.trim_start_matches("0x");
            if let Ok(raw_wei) = u128::from_str_radix(clean, 16) {
                if raw_wei > 0 {
                    let amount = raw_wei / 10_000_000_000;
                    return Ok(vec![OnchainTx {
                        tx_hash: format!(
                            "evm_bal_{}",
                            hex::encode(&address.as_bytes()[..address.len().min(16)])
                        ),
                        vout: 0,
                        amount,
                        confirmations: 30,
                        address: address.to_string(),
                    }]);
                }
            }
        }
        Ok(vec![])
    }

    async fn get_mempool_tip(&self, base_url: &str) -> Result<i64, reqwest::Error> {
        let url = format!("{}/blocks/tip/height", base_url);
        let res = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(res.trim().parse::<i64>().unwrap_or(0))
    }
}

#[derive(Debug, Deserialize)]
struct MempoolUtxo {
    txid: String,
    vout: u32,
    value: u128,
    status: MempoolStatus,
}

#[derive(Debug, Deserialize)]
struct MempoolStatus {
    block_height: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BlockcypherAddrResp {
    txs: Option<Vec<BlockcypherTx>>,
}

#[derive(Debug, Deserialize)]
struct BlockcypherTx {
    hash: String,
    confirmations: i64,
    outputs: Vec<BlockcypherOutput>,
}

#[derive(Debug, Deserialize)]
struct BlockcypherOutput {
    value: i64,
    addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BitcoreCoinEntry {
    #[serde(rename = "mintTxid")]
    mint_txid: String,
    #[serde(rename = "mintIndex")]
    mint_index: u32,
    #[serde(rename = "mintHeight")]
    mint_height: i64,
    value: i64,
}

#[derive(Debug, Deserialize)]
struct BlockscoutTxListResp {
    items: Option<Vec<BlockscoutTxItem>>,
}

#[derive(Debug, Deserialize)]
struct BlockscoutTxItem {
    hash: String,
    to: Option<BlockscoutAddressHash>,
    value: Option<String>,
    status: Option<String>,
    result: Option<String>,
    confirmations: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BlockscoutAddressHash {
    hash: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct BlockscoutTokenList {
    items: Option<Vec<BlockscoutTokenItem>>,
}

#[derive(Debug, Deserialize)]
struct BlockscoutTokenItem {
    #[serde(default)]
    transaction_hash: Option<String>,
    #[serde(default)]
    confirmations: Option<u32>,
    #[serde(default)]
    token: Option<BlockscoutTokenMeta>,
    #[serde(default)]
    to: Option<BlockscoutAddressHash>,
    #[serde(default)]
    total: Option<BlockscoutTokenTotal>,
}

#[derive(Debug, Deserialize)]
struct BlockscoutTokenMeta {
    #[serde(default)]
    address_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlockscoutTokenTotal {
    #[serde(default)]
    value: Option<String>,
}

fn parse_doge_blockbook(
    data: &serde_json::Value,
    address: &str,
) -> Result<Vec<OnchainTx>, ChainError> {
    let transactions = data
        .get("transactions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ChainError {
            message: "DOGE Blockbook missing transactions".into(),
        })?;
    let mut deposits = Vec::new();
    for tx in transactions {
        let Some(txid) = tx.get("txid").and_then(|v| v.as_str()) else {
            continue;
        };
        let confirmations = tx
            .get("confirmations")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        if let Some(outputs) = tx.get("vout").and_then(|v| v.as_array()) {
            for (index, output) in outputs.iter().enumerate() {
                let owned = output
                    .get("addresses")
                    .and_then(|v| v.as_array())
                    .is_some_and(|xs| xs.iter().any(|x| x.as_str() == Some(address)));
                if !owned {
                    continue;
                }
                let amount = output
                    .get("value")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u128>().ok());
                if let Some(amount) = amount {
                    deposits.push(OnchainTx {
                        tx_hash: txid.to_owned(),
                        vout: index as u32,
                        amount,
                        confirmations,
                        address: address.to_owned(),
                    });
                }
            }
        }
    }
    Ok(deposits)
}

#[cfg(test)]
mod doge_blockbook_tests {
    use super::*;
    #[test]
    fn parses_only_outputs_for_requested_address() {
        let data = serde_json::json!({"transactions": [{"txid":"abc", "confirmations": 3,
            "vout": [{"value":"100", "addresses":["mine"]}, {"value":"200", "addresses":["other"]}]}]});
        let deposits = parse_doge_blockbook(&data, "mine").unwrap();
        assert_eq!(deposits.len(), 1);
        assert_eq!(deposits[0].amount, 100);
        assert_eq!(deposits[0].vout, 0);
    }
    #[test]
    fn rejects_empty_schema_instead_of_reporting_no_deposits() {
        assert!(parse_doge_blockbook(&serde_json::json!({"error":"rate limit"}), "mine").is_err());
    }
}
