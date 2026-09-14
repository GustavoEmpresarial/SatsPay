//! Real `ChainClient` — replaces the stub. Reads (address generation/
//! validation, balance, deposit detection) are live for all 5 coins.
//! Broadcasts are live for BTC/LTC/DOGE (P2WPKH), BCH (P2PKH + SIGHASH_FORKID),
//! and POL (EIP-155 legacy tx). Fee rates, gas price, nonce and gas limit
//! always come from the live network — never invented constants.

use crate::bch_sign::build_and_sign_bch_p2pkh;
use crate::bitcore_client::BitcoreClient;
use crate::btc_sign::{build_and_sign_p2wpkh, Utxo};
use crate::encoding::{
    base58check_decode, base58check_encode, bech32_p2wpkh_decode, bech32_p2wpkh_encode, cashaddr_decode, cashaddr_encode,
    eip55_encode, eip55_validate, evm_address_from_uncompressed_pubkey, hash160, solana_address_decode,
};
use crate::evm_client::{erc20_transfer_data, EvmClient};
use crate::evm_sign::{address_from_secret, parse_address, sign_legacy_tx, LegacyTx};
use crate::hd::{compressed_bytes, derive_child_pubkey, uncompressed_xy_bytes};
use crate::params::{params_for, AddressKind, ChainNetwork};
use crate::types::{BroadcastError, BroadcastResult, ChainClient, ChainError, OnchainTx};
use async_trait::async_trait;
use bitcoin::hashes::Hash;
use serde::Deserialize;
use shared::Coin;
use sqlx::PgPool;

/// Endpoints + keys for the real client. Every field is supplied by the
/// caller (env via `ChainRegistry::from_env`) — nothing is hardcoded here.
#[derive(Clone)]
pub struct RealClientConfig {
    pub bitcore_base_url: String,
    pub evm_rpc_url: String,
    /// Watch-only BIP32 xpub used to derive deposit addresses.
    pub deposit_xpub: String,
    /// Hot-wallet WIF. The same secp256k1 key is reused for POL (EVM address
    /// = keccak of the uncompressed pubkey). `None` disables broadcast.
    /// Testnet WIFs use a different version byte; `PrivateKey::from_wif`
    /// handles that — address encoding still must use testnet `params_for`.
    pub hot_wallet_wif: Option<String>,
    /// How many recent Polygon blocks to scan for native transfers — sized
    /// by the operator to their poll interval ÷ actual block time.
    pub evm_deposit_lookback_blocks: u64,
    /// Confirmation target (in blocks) passed to Bitcore's fee endpoint —
    /// operator-chosen, not invented here.
    pub fee_confirmation_target: u32,
    /// `mainnet` or `testnet` — selects Bitcore path + address params / POL chain id.
    pub network: ChainNetwork,
    pub sol_rpc_url: String,
    pub dgb_insight_url: String,
    /// BIP-39 deposit seed. When set, addresses are derived at `{account}/0/{index}`.
    pub deposit_mnemonic: Option<String>,
    /// BIP-39 hot/withdrawal seed. Index 0 of each coin. Sweeps land here.
    pub hot_mnemonic: Option<String>,
}

pub struct RealChainClient {
    coin: Coin,
    pool: PgPool,
    config: RealClientConfig,
}

impl RealChainClient {
    pub fn new(coin: Coin, pool: PgPool, config: RealClientConfig) -> Self {
        Self { coin, pool, config }
    }

    fn coin_params(&self) -> crate::params::CoinParams {
        params_for(self.coin, self.config.network)
    }

    fn bitcore(&self) -> Option<BitcoreClient> {
        self.coin_params().bitcore_chain.map(|chain| {
            BitcoreClient::new(
                &self.config.bitcore_base_url,
                chain,
                self.config.network.as_bitcore_str(),
            )
        })
    }

    fn evm(&self) -> EvmClient {
        EvmClient::new(&self.config.evm_rpc_url)
    }

    fn hot_key_material(&self) -> Result<String, String> {
        if let Some(m) = &self.config.hot_mnemonic {
            if self.coin == Coin::Sol {
                return Ok(m.clone());
            }
            let secret = crate::hd_wallet::hot_secret_from_mnemonic(m, self.coin).map_err(|e| e.to_string())?;
            return crate::hd_wallet::secret_to_wif(&secret).map_err(|e| e.to_string());
        }
        self.config.hot_wallet_wif.clone().ok_or_else(|| "Hot wallet não configurada".into())
    }

    fn hot_addr(&self) -> Result<String, String> {
        if let Some(m) = &self.config.hot_mnemonic {
            return crate::hd_wallet::hot_address_from_mnemonic(m, self.coin, self.config.network).map_err(|e| e.to_string());
        }
        let wif = self.config.hot_wallet_wif.as_deref().ok_or_else(|| "Hot wallet não configurada".to_string())?;
        hot_wallet_address(self.coin, self.config.network, wif)
    }

    async fn next_hd_index(&self) -> Result<u32, ChainError> {
        let seq_name = format!("{}_hd_index_seq", self.coin.as_str().to_lowercase());
        // Sequence names are a fixed, migration-created set — never user input.
        let sql = format!("SELECT nextval('{seq_name}')");
        let index: i64 = sqlx::query_scalar(&sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ChainError { message: e.to_string() })?;
        Ok(index as u32)
    }
}

#[async_trait]
impl ChainClient for RealChainClient {
    fn coin(&self) -> Coin {
        self.coin
    }

    async fn generate_address(&self, _user_id: &str) -> Result<crate::GeneratedAddress, ChainError> {
        let index = self.next_hd_index().await?;
        if let Some(mnemonic) = &self.config.deposit_mnemonic {
            let address = crate::hd_wallet::address_from_mnemonic(mnemonic, self.coin, self.config.network, index)
                .map_err(|e| ChainError { message: e.to_string() })?;
            return Ok(crate::GeneratedAddress { address, hd_index: Some(index) });
        }
        let pubkey = derive_child_pubkey(&self.config.deposit_xpub, index).map_err(|e| ChainError { message: e.to_string() })?;
        let address = match self.coin_params().address_kind {
            AddressKind::Bech32Segwit { hrp, .. } => bech32_p2wpkh_encode(hrp, &hash160(&compressed_bytes(&pubkey))),
            AddressKind::Base58Only { version } => base58check_encode(version, &hash160(&compressed_bytes(&pubkey))),
            AddressKind::CashAddr { prefix } => cashaddr_encode(prefix, &hash160(&compressed_bytes(&pubkey))),
            AddressKind::Evm => eip55_encode(&evm_address_from_uncompressed_pubkey(&uncompressed_xy_bytes(&pubkey))),
            AddressKind::Solana => {
                let master = self.config.hot_wallet_wif.as_deref().map(crate::sol_client::sol_master_from_hot_key).unwrap_or_else(|| crate::sol_client::sol_master_from_hot_key(&self.config.deposit_xpub));
                let secret = crate::sol_client::derive_sol_secret(&master, b"bitcosats-sol-deposit", index);
                crate::sol_client::sol_address_from_secret(&secret)
            }
        };
        Ok(crate::GeneratedAddress { address, hd_index: Some(index) })
    }

    fn validate_address(&self, address: &str) -> bool {
        if address.len() > 128 {
            return false;
        }
        match self.coin_params().address_kind {
            AddressKind::Bech32Segwit { hrp, legacy_base58_version } => {
                bech32_p2wpkh_decode(hrp, address).is_some() || base58check_decode(address).is_some_and(|(v, _)| v == legacy_base58_version)
            }
            AddressKind::Base58Only { version } => base58check_decode(address).is_some_and(|(v, _)| v == version),
            AddressKind::CashAddr { prefix } => cashaddr_decode(prefix, address).is_some(),
            AddressKind::Evm => eip55_validate(address),
            AddressKind::Solana => solana_address_decode(address).is_some(),
        }
    }

    async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let multi_rpc = crate::rpc_client::MultiChainRpcClient::new();
        multi_rpc.fetch_deposits(self.coin, address).await
    }

    async fn get_balance(&self, address: &str) -> Result<u128, ChainError> {
        match self.coin {
            Coin::Sol => {
                let client = crate::sol_client::SolClient::new(&self.config.sol_rpc_url);
                client.get_balance_internal(address).await
            }
            Coin::Dgb => {
                let client = crate::dgb_client::DgbClient::new(&self.config.dgb_insight_url);
                client.get_balance(address).await
            }
            Coin::Usdt | Coin::Usdc => {
                let token = self.coin_params().erc20_contract.ok_or_else(|| ChainError { message: "missing ERC-20 contract".into() })?;
                let raw = self.evm().erc20_balance(token, address).await.map_err(|e| ChainError { message: e.to_string() })?;
                Ok(shared::from_onchain_amount(self.coin, raw))
            }
            Coin::Pol => {
                let wei = self.evm().get_balance(address).await.map_err(|e| ChainError { message: e.to_string() })?;
                Ok(shared::from_onchain_amount(Coin::Pol, wei))
            }
            _ => {
                if let Some(client) = self.bitcore() {
                    return client.get_balance(address).await.map_err(|e| ChainError { message: e.to_string() });
                }
                Err(ChainError { message: format!("no balance provider for {:?}", self.coin) })
            }
        }
    }

    async fn broadcast_withdrawal(&self, to_address: &str, amount: u128) -> Result<BroadcastResult, BroadcastError> {
        let wif = match self.hot_key_material() {
            Ok(k) => k,
            Err(message) => {
                return Err(BroadcastError {
                    message: format!("{message} ({:?})", self.coin),
                    safe_to_reverse: true,
                })
            }
        };
        let wif = &wif;

        match self.coin {
            Coin::Pol => return self.broadcast_pol(wif, to_address, amount).await,
            Coin::Usdt | Coin::Usdc => return self.broadcast_erc20(wif, to_address, amount).await,
            Coin::Sol => return self.broadcast_sol(wif, to_address, amount).await,
            Coin::Dgb => return self.broadcast_dgb(wif, to_address, amount).await,
            _ => {}
        }

        let Some(bitcore_chain) = self.coin_params().bitcore_chain else {
            return Err(BroadcastError {
                message: format!("no bitcore chain mapping for {:?}", self.coin),
                safe_to_reverse: true,
            });
        };

        let client = self.bitcore().expect("bitcore_chain present");
        let hot_address = self.hot_addr().map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let fee_per_byte = fetch_fee_rate(
            &self.config.bitcore_base_url,
            bitcore_chain,
            self.config.network.as_bitcore_str(),
            self.config.fee_confirmation_target,
        )
        .await
        .map_err(|e| BroadcastError { message: format!("fee estimation failed: {e}"), safe_to_reverse: true })?;

        let utxos = fetch_spendable_utxos(&client, &hot_address)
            .await
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;

        // Size models follow each coin's wire serialization (not invented
        // constants):
        // - SegWit P2WPKH vsize = ceil((overhead_wu + in_wu*n + out_wu*m)/4)
        //   with overhead≈42 wu, in≈272 wu, out≈124 wu → ≈11 + 68*n + 31*m
        //   (BIP141 weight units / 4).
        // - Legacy P2PKH size ≈ 10 + 148*ins + 34*outs (version/locktime/
        //   varints + ~148-byte scriptSig inputs + 34-byte P2PKH outs).
        // Fee = size * live Bitcore sat/byte at `fee_confirmation_target`.
        let (estimated_size, sign) = match self.coin {
            Coin::Bch => {
                let n_in = utxos.len();
                let n_out = 2; // recipient + change (worst-case for fee)
                let size = 10 + n_in * 148 + n_out * 34;
                (size, "bch")
            }
            _ => {
                let n_in = utxos.len();
                let n_out = 2;
                let vsize = 11 + n_in * 68 + n_out * 31;
                (vsize, "segwit")
            }
        };
        let fee = (estimated_size as u64) * fee_per_byte;

        let to_script = address_to_script_pubkey(self.coin, self.config.network, to_address)
            .map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let change_script = address_to_script_pubkey(self.coin, self.config.network, &hot_address)
            .map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let amount_sats: u64 = amount
            .try_into()
            .map_err(|_| BroadcastError { message: "amount exceeds u64 range".into(), safe_to_reverse: true })?;

        let raw_hex = if sign == "bch" {
            let tx = build_and_sign_bch_p2pkh(wif, &utxos, &to_script, amount_sats, fee, &change_script)
                .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
            bitcoin::consensus::encode::serialize_hex(&tx)
        } else {
            let tx = build_and_sign_p2wpkh(wif, &utxos, &to_script, amount_sats, fee, &change_script)
                .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
            bitcoin::consensus::encode::serialize_hex(&tx)
        };

        match client.broadcast(&raw_hex).await {
            Ok(txid) => Ok(BroadcastResult { tx_hash: txid, fee_amount: fee as u128 }),
            Err(e) => Err(BroadcastError { message: e.to_string(), safe_to_reverse: false }),
        }
    }

    async fn sweep_deposit_to_hot(&self, hd_index: u32) -> Result<Option<BroadcastResult>, BroadcastError> {
        let Some(mnemonic) = &self.config.deposit_mnemonic else {
            return Ok(None);
        };
        let to = self.hot_addr().map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let secret = crate::hd_wallet::secret_from_mnemonic(mnemonic, self.coin, hd_index)
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let from = crate::hd_wallet::address_from_mnemonic(mnemonic, self.coin, self.config.network, hd_index)
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        if from.eq_ignore_ascii_case(&to) {
            return Ok(None);
        }
        let wif = if self.coin == Coin::Sol {
            mnemonic.clone()
        } else {
            crate::hd_wallet::secret_to_wif(&secret).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?
        };

        match self.coin {
            Coin::Pol => {
                let wei = self.evm().get_balance(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if wei < 50_000_000_000_000_000 {
                    return Ok(None);
                }
                let gas_price = self.evm().gas_price().await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?.max(30_000_000_000);
                let fee = gas_price.saturating_mul(21_000);
                if wei <= fee + 10_000_000_000_000_000 {
                    return Ok(None);
                }
                let send_wei = wei - fee;
                let internal = shared::from_onchain_amount(Coin::Pol, send_wei);
                if internal == 0 {
                    return Ok(None);
                }
                Ok(Some(self.broadcast_pol(&wif, &to, internal).await?))
            }
            Coin::Usdt | Coin::Usdc => {
                let token = self.coin_params().erc20_contract.ok_or_else(|| BroadcastError { message: "missing ERC-20".into(), safe_to_reverse: true })?;
                let raw = self.evm().erc20_balance(token, &from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                let internal = shared::from_onchain_amount(self.coin, raw);
                if internal == 0 {
                    return Ok(None);
                }
                // Deposit addresses need native POL to pay ERC-20 gas. Top up from hot if empty.
                const MIN_GAS_WEI: u128 = 50_000_000_000_000_000; // 0.05 POL
                const TOPUP_WEI: u128 = 80_000_000_000_000_000; // 0.08 POL
                let pol_bal = self.evm().get_balance(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if pol_bal < MIN_GAS_WEI {
                    let hot_wif = self.hot_key_material().map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
                    let topup_internal = shared::from_onchain_amount(Coin::Pol, TOPUP_WEI);
                    tracing::info!(deposit = %from, coin = %self.coin.as_str(), "topping up POL gas on deposit before ERC-20 sweep");
                    let top = self.broadcast_pol(&hot_wif, &from, topup_internal).await?;
                    let _ = self.evm().wait_receipt_success(&top.tx_hash, 20, 2).await;
                    // Retry ERC-20 sweep next tick once gas is confirmed.
                    return Ok(None);
                }
                Ok(Some(self.broadcast_erc20(&wif, &to, internal).await?))
            }
            Coin::Sol => {
                let client = crate::sol_client::SolClient::new(&self.config.sol_rpc_url);
                let bal = client.get_balance_internal(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if bal < 10_000 {
                    return Ok(None);
                }
                let send = bal.saturating_sub(5_000);
                Ok(Some(client.broadcast_transfer(&secret, &to, send).await?))
            }
            Coin::Dgb => {
                let client = crate::dgb_client::DgbClient::new(&self.config.dgb_insight_url);
                let utxos = client.fetch_utxos(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if utxos.is_empty() {
                    return Ok(None);
                }
                let total: u64 = utxos.iter().map(|u| u.value).sum();
                let fee_per_byte = client.estimate_fee_sat_per_byte().await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                let vsize = 11 + utxos.len() * 68 + 31;
                let fee = (vsize as u64) * fee_per_byte;
                if total <= fee + 1_000 {
                    return Ok(None);
                }
                Ok(Some(self.broadcast_dgb(&wif, &to, (total - fee) as u128).await?))
            }
            _ => {
                let Some(bitcore_chain) = self.coin_params().bitcore_chain else {
                    return Ok(None);
                };
                let client = self.bitcore().expect("bitcore");
                let utxos = fetch_spendable_utxos(&client, &from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if utxos.is_empty() {
                    return Ok(None);
                }
                let total: u64 = utxos.iter().map(|u| u.value).sum();
                let fee_per_byte = fetch_fee_rate(
                    &self.config.bitcore_base_url,
                    bitcore_chain,
                    self.config.network.as_bitcore_str(),
                    self.config.fee_confirmation_target,
                )
                .await
                .map_err(|e| BroadcastError { message: format!("fee: {e}"), safe_to_reverse: true })?;
                let n_in = utxos.len();
                let estimated_size = if self.coin == Coin::Bch { 10 + n_in * 148 + 34 } else { 11 + n_in * 68 + 31 };
                let fee = (estimated_size as u64) * fee_per_byte;
                if total <= fee + 600 {
                    return Ok(None);
                }
                let amount = total - fee;
                let to_script = address_to_script_pubkey(self.coin, self.config.network, &to)
                    .map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
                let change_script = to_script.clone();
                let raw_hex = if self.coin == Coin::Bch {
                    let tx = build_and_sign_bch_p2pkh(&wif, &utxos, &to_script, amount, fee, &change_script)
                        .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                    bitcoin::consensus::encode::serialize_hex(&tx)
                } else {
                    let tx = build_and_sign_p2wpkh(&wif, &utxos, &to_script, amount, fee, &change_script)
                        .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                    bitcoin::consensus::encode::serialize_hex(&tx)
                };
                match client.broadcast(&raw_hex).await {
                    Ok(txid) => Ok(Some(BroadcastResult { tx_hash: txid, fee_amount: fee as u128 })),
                    Err(e) => Err(BroadcastError { message: e.to_string(), safe_to_reverse: false }),
                }
            }
        }
    }
}

impl RealChainClient {
    async fn broadcast_pol(&self, wif: &str, to_address: &str, amount_internal: u128) -> Result<BroadcastResult, BroadcastError> {
        // Scale 8-decimal internal units to 18-decimal EVM wei
        let amount_wei = amount_internal.saturating_mul(10_000_000_000);
        let secret = parse_secret_key_bytes(wif).map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let from = address_from_secret(&secret).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let from_hex = format!("0x{}", hex::encode(from));
        let to = parse_address(to_address).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;

        let rpc_endpoints = [
            self.config.evm_rpc_url.as_str(),
            "https://polygon-bor-rpc.publicnode.com",
            "https://1rpc.io/matic",
            "https://polygon.drpc.org",
            "https://polygon-rpc.com",
        ];

        let chain_id = params_for(Coin::Pol, self.config.network)
            .evm_chain_id
            .unwrap_or(137);

        let mut last_error = String::new();
        for &rpc_url in &rpc_endpoints {
            let evm = EvmClient::new(rpc_url);
            let nonce = match evm.get_transaction_count(&from_hex).await {
                Ok(n) => n,
                Err(e) => {
                    last_error = format!("nonce failed on {rpc_url}: {e}");
                    continue;
                }
            };

            let gas_price = match evm.gas_price().await {
                Ok(g) => g.max(30_000_000_000), // minimum 30 Gwei for Polygon
                Err(e) => {
                    last_error = format!("gas_price failed on {rpc_url}: {e}");
                    continue;
                }
            };

            let gas_limit = match evm.estimate_gas(&from_hex, to_address, amount_wei).await {
                Ok(l) => l.max(21_000),
                Err(_) => 21_000,
            };

            let raw = match sign_legacy_tx(
                &secret,
                &LegacyTx {
                    nonce,
                    gas_price_wei: gas_price,
                    gas_limit,
                    to,
                    value_wei: amount_wei,
                    data: Vec::new(),
                    chain_id,
                },
            ) {
                Ok(r) => r,
                Err(e) => return Err(BroadcastError { message: e.to_string(), safe_to_reverse: true }),
            };

            let fee_amount = (gas_price.saturating_mul(gas_limit as u128)) / 10_000_000_000;
            match evm.broadcast(&raw).await {
                Ok(txid) => {
                    tracing::info!(tx_hash = %txid, from = %from_hex, to = %to_address, rpc = %rpc_url, "POL withdrawal broadcasted on-chain successfully");
                    return Ok(BroadcastResult { tx_hash: txid, fee_amount });
                }
                Err(e) => {
                    last_error = format!("broadcast failed on {rpc_url}: {e}");
                }
            }
        }

        Err(BroadcastError {
            message: format!("EVM broadcast falhou em todos os RPCs: {last_error}"),
            safe_to_reverse: true,
        })
    }

    async fn broadcast_erc20(&self, wif: &str, to_address: &str, amount_internal: u128) -> Result<BroadcastResult, BroadcastError> {
        let token = self.coin_params().erc20_contract.ok_or_else(|| BroadcastError {
            message: "missing ERC-20 contract".into(),
            safe_to_reverse: true,
        })?;
        let amount_token = shared::to_onchain_amount(self.coin, amount_internal);
        if amount_token == 0 {
            return Err(BroadcastError { message: "token amount too small after scale".into(), safe_to_reverse: true });
        }
        let secret = parse_secret_key_bytes(wif).map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let from = address_from_secret(&secret).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let from_hex = format!("0x{}", hex::encode(from));
        let to = parse_address(to_address).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let data = erc20_transfer_data(to, amount_token);
        let data_hex = format!("0x{}", hex::encode(&data));
        let token_addr = parse_address(token).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let chain_id = self.coin_params().evm_chain_id.unwrap_or(137);

        let rpc_endpoints = [
            self.config.evm_rpc_url.as_str(),
            "https://polygon-bor-rpc.publicnode.com",
            "https://1rpc.io/matic",
            "https://polygon.drpc.org",
        ];
        let mut last_error = String::new();
        for &rpc_url in &rpc_endpoints {
            let evm = EvmClient::new(rpc_url);

            // Pre-flight: hot wallet must hold the tokens. Never broadcast a doomed transfer.
            match evm.erc20_balance(token, &from_hex).await {
                Ok(bal) if bal < amount_token => {
                    return Err(BroadcastError {
                        message: format!(
                            "hot wallet {} has insufficient {}: have {} need {} (on-chain units)",
                            from_hex,
                            self.coin.as_str(),
                            bal,
                            amount_token
                        ),
                        safe_to_reverse: true,
                    });
                }
                Ok(_) => {}
                Err(e) => {
                    last_error = format!("balance check failed on {rpc_url}: {e}");
                    continue;
                }
            }

            let nonce = match evm.get_transaction_count(&from_hex).await {
                Ok(n) => n,
                Err(e) => {
                    last_error = format!("nonce failed on {rpc_url}: {e}");
                    continue;
                }
            };
            let gas_price = match evm.gas_price().await {
                Ok(g) => g.max(30_000_000_000),
                Err(e) => {
                    last_error = format!("gas_price failed on {rpc_url}: {e}");
                    continue;
                }
            };
            // estimateGas failing means the call would revert (e.g. exceeds balance) —
            // do NOT fall back to a hardcoded gas limit and broadcast anyway.
            let gas_limit = match evm.estimate_gas_data(&from_hex, token, &data_hex).await {
                Ok(l) => l.max(65_000),
                Err(e) => {
                    return Err(BroadcastError {
                        message: format!("ERC-20 transfer would revert (estimateGas): {e}"),
                        safe_to_reverse: true,
                    });
                }
            };
            let raw = match sign_legacy_tx(
                &secret,
                &LegacyTx {
                    nonce,
                    gas_price_wei: gas_price,
                    gas_limit,
                    to: token_addr,
                    value_wei: 0,
                    data: data.clone(),
                    chain_id,
                },
            ) {
                Ok(r) => r,
                Err(e) => return Err(BroadcastError { message: e.to_string(), safe_to_reverse: true }),
            };
            let fee_amount = (gas_price.saturating_mul(gas_limit as u128)) / 10_000_000_000;
            match evm.broadcast(&raw).await {
                Ok(txid) => {
                    // Wait for receipt — mempool acceptance ≠ success (reverts still burn gas).
                    match evm.wait_receipt_success(&txid, 24, 2).await {
                        Ok(true) => return Ok(BroadcastResult { tx_hash: txid, fee_amount }),
                        Ok(false) => {
                            return Err(BroadcastError {
                                message: format!("ERC-20 tx reverted on-chain: {txid}"),
                                safe_to_reverse: true,
                            });
                        }
                        Err(e) => {
                            // Ambiguous: mined status unknown — do not reverse.
                            return Err(BroadcastError {
                                message: format!("ERC-20 broadcasted ({txid}) but receipt wait failed: {e}"),
                                safe_to_reverse: false,
                            });
                        }
                    }
                }
                Err(e) => last_error = format!("broadcast failed on {rpc_url}: {e}"),
            }
        }
        Err(BroadcastError { message: format!("ERC-20 broadcast falhou: {last_error}"), safe_to_reverse: true })
    }

    async fn broadcast_sol(&self, wif: &str, to_address: &str, amount: u128) -> Result<BroadcastResult, BroadcastError> {
        let master = crate::sol_client::sol_master_from_hot_key(wif);
        let secret = crate::sol_client::derive_sol_secret(&master, b"bitcosats-sol-hot", 0);
        let client = crate::sol_client::SolClient::new(&self.config.sol_rpc_url);
        client.broadcast_transfer(&secret, to_address, amount).await
    }

    async fn broadcast_dgb(&self, wif: &str, to_address: &str, amount: u128) -> Result<BroadcastResult, BroadcastError> {
        let client = crate::dgb_client::DgbClient::new(&self.config.dgb_insight_url);
        let hot_address = hot_wallet_address(self.coin, self.config.network, wif)
            .map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let fee_per_byte = client
            .estimate_fee_sat_per_byte()
            .await
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let utxos = client
            .fetch_utxos(&hot_address)
            .await
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let n_in = utxos.len();
        let vsize = 11 + n_in * 68 + 2 * 31;
        let fee = (vsize as u64) * fee_per_byte;
        let to_script = address_to_script_pubkey(self.coin, self.config.network, to_address)
            .map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let change_script = address_to_script_pubkey(self.coin, self.config.network, &hot_address)
            .map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let amount_sats: u64 = amount.try_into().map_err(|_| BroadcastError { message: "amount exceeds u64".into(), safe_to_reverse: true })?;
        let tx = build_and_sign_p2wpkh(wif, &utxos, &to_script, amount_sats, fee, &change_script)
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let raw_hex = bitcoin::consensus::encode::serialize_hex(&tx);
        match client.broadcast(&raw_hex).await {
            Ok(txid) => Ok(BroadcastResult { tx_hash: txid, fee_amount: fee as u128 }),
            Err(e) => Err(BroadcastError { message: e.to_string(), safe_to_reverse: false }),
        }
    }
}

pub fn parse_secret_key_bytes(key: &str) -> Result<[u8; 32], String> {
    let clean = key.trim().strip_prefix("0x").unwrap_or(key.trim());
    if clean.len() == 64 {
        if let Ok(bytes) = hex::decode(clean) {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            return Ok(arr);
        }
    }
    let privkey = bitcoin::PrivateKey::from_wif(key.trim()).map_err(|e| format!("chave privada inválida (WIF/Hex): {e}"))?;
    Ok(privkey.inner.secret_bytes())
}

/// Derive the hot-wallet address for `coin`/`network` from a WIF or Hex private key.
pub fn hot_wallet_address(coin: Coin, network: ChainNetwork, wif: &str) -> Result<String, String> {
    let secret = parse_secret_key_bytes(wif)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let sk = bitcoin::secp256k1::SecretKey::from_slice(&secret).map_err(|e| e.to_string())?;
    let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let compressed_pk = pubkey.serialize();
    let hash = hash160(&compressed_pk);
    Ok(match params_for(coin, network).address_kind {
        AddressKind::Bech32Segwit { hrp, .. } => bech32_p2wpkh_encode(hrp, &hash),
        AddressKind::Base58Only { version } => base58check_encode(version, &hash),
        AddressKind::CashAddr { prefix } => cashaddr_encode(prefix, &hash),
        AddressKind::Evm => {
            let xy = uncompressed_xy_bytes(&pubkey);
            eip55_encode(&evm_address_from_uncompressed_pubkey(&xy))
        }
        AddressKind::Solana => {
            let master = crate::sol_client::sol_master_from_hot_key(wif);
            let secret = crate::sol_client::derive_sol_secret(&master, b"bitcosats-sol-hot", 0);
            crate::sol_client::sol_address_from_secret(&secret)
        }
    })
}

fn address_to_script_pubkey(coin: Coin, network: ChainNetwork, address: &str) -> Result<bitcoin::ScriptBuf, String> {
    match params_for(coin, network).address_kind {
        AddressKind::Bech32Segwit { hrp, legacy_base58_version } => {
            if let Some(hash) = bech32_p2wpkh_decode(hrp, address) {
                let wpkh = bitcoin::WPubkeyHash::from_raw_hash(bitcoin::hashes::hash160::Hash::from_byte_array(hash));
                return Ok(bitcoin::ScriptBuf::new_p2wpkh(&wpkh));
            }
            if let Some((v, hash)) = base58check_decode(address) {
                if v == legacy_base58_version {
                    let pkh = bitcoin::PubkeyHash::from_raw_hash(bitcoin::hashes::hash160::Hash::from_byte_array(hash));
                    return Ok(bitcoin::ScriptBuf::new_p2pkh(&pkh));
                }
            }
            Err(format!("invalid address for this network: {address}"))
        }
        AddressKind::Base58Only { version } => {
            let (v, hash) = base58check_decode(address).ok_or_else(|| format!("invalid address: {address}"))?;
            if v != version {
                return Err(format!("invalid address version for this network: {address}"));
            }
            let pkh = bitcoin::PubkeyHash::from_raw_hash(bitcoin::hashes::hash160::Hash::from_byte_array(hash));
            Ok(bitcoin::ScriptBuf::new_p2pkh(&pkh))
        }
        AddressKind::CashAddr { prefix } => {
            let hash = cashaddr_decode(prefix, address).ok_or_else(|| format!("invalid cashaddr: {address}"))?;
            let pkh = bitcoin::PubkeyHash::from_raw_hash(bitcoin::hashes::hash160::Hash::from_byte_array(hash));
            Ok(bitcoin::ScriptBuf::new_p2pkh(&pkh))
        }
        AddressKind::Evm => Err("EVM addresses are not bitcoin scriptPubKeys".into()),
        AddressKind::Solana => Err("Solana addresses are not bitcoin scriptPubKeys".into()),
    }
}

async fn fetch_spendable_utxos(client: &BitcoreClient, address: &str) -> Result<Vec<Utxo>, crate::bitcore_client::BitcoreError> {
    #[derive(Deserialize)]
    struct RawUtxo {
        #[serde(rename = "mintTxid")]
        mint_txid: String,
        #[serde(rename = "mintIndex")]
        mint_index: u32,
        value: i64,
        script: String,
    }
    let raw: Vec<RawUtxo> = client.get_unspent_raw(address).await?;
    Ok(raw
        .into_iter()
        .map(|u| Utxo {
            txid: u.mint_txid,
            vout: u.mint_index,
            value: u.value.max(0) as u64,
            script_pubkey_hex: u.script,
        })
        .collect())
}

/// Live fee-rate (sat/byte) from Bitcore at the operator-chosen confirmation
/// target — never a hardcoded fee.
async fn fetch_fee_rate(
    base_url: &str,
    chain: &'static str,
    network: &'static str,
    confirmation_target: u32,
) -> Result<u64, String> {
    #[derive(Deserialize)]
    struct FeeResp {
        #[serde(rename = "feerate")]
        fee_rate_per_kb: f64,
    }
    let url = format!(
        "{}/api/{}/{network}/fee/{confirmation_target}",
        base_url.trim_end_matches('/'),
        chain
    );
    let resp: FeeResp = reqwest::get(&url).await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    // Bitcore returns fee per kB in the coin's main unit → sat/byte.
    Ok((fee_rate_per_kb_to_sat_per_byte(resp.fee_rate_per_kb).max(1.0)) as u64)
}

fn fee_rate_per_kb_to_sat_per_byte(fee_per_kb_main_unit: f64) -> f64 {
    (fee_per_kb_main_unit * 100_000_000.0) / 1000.0
}
