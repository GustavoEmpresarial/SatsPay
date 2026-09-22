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
use crate::evm_client::{erc20_approve_data, erc20_approve_max_data, erc20_transfer_data, EvmClient};
use crate::evm_sign::{address_from_secret, parse_address, sign_legacy_tx, LegacyTx};
use crate::hd::{compressed_bytes, derive_receive_pubkey, uncompressed_xy_bytes};
use crate::params::{params_for, AddressKind, ChainNetwork};
use crate::types::{BroadcastError, BroadcastResult, ChainClient, ChainError, EvmContractCall, OnchainTx};
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
    /// BNB Smart Chain JSON-RPC. Used only for PEPE (chain id 56). Never the Polygon URL.
    pub bsc_rpc_url: String,
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
    /// Self-hosted DigiByte Core JSON-RPC (`http://user:pass@host:port`).
    /// Empty/`None` falls back to Insight/Blockbook.
    pub dgb_rpc_url: Option<String>,
    pub zer_explorer_url: String,
    pub zer_explorer_api_key: Option<String>,
    /// Self-hosted zerod JSON-RPC. Required for ZER withdraw.
    pub zer_rpc_url: Option<String>,
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
        if self.coin_params().evm_chain_id == Some(56) {
            EvmClient::new(&self.config.bsc_rpc_url)
        } else {
            EvmClient::new(&self.config.evm_rpc_url)
        }
    }

    /// USDT and USDC share the POL HD path (`m/44'/60'/0'/0/{index}`).
    /// Unknown balance is treated as "holds tokens" so a POL sweep cannot
    /// drain the gas those sweeps need.
    async fn polygon_address_holds_stable(&self, address: &str) -> bool {
        for coin in [Coin::Usdt, Coin::Usdc] {
            let Some(token) = params_for(coin, self.config.network).erc20_contract else { continue };
            match self.evm().erc20_balance(token, address).await {
                Ok(raw) if shared::from_onchain_amount(coin, raw) > 0 => return true,
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, address, coin = %coin.as_str(), "ERC-20 balance unread before POL sweep; keeping gas");
                    return true;
                }
            }
        }
        false
    }

    /// JSON-RPC list for the coin's EVM chain. BSC never falls back to Polygon.
    fn token_rpc_urls(&self) -> Vec<String> {
        let mut out = Vec::new();
        let push = |out: &mut Vec<String>, url: &str| {
            let url = url.trim();
            if url.is_empty() || out.iter().any(|u| u == url) {
                return;
            }
            out.push(url.to_string());
        };
        if self.coin_params().evm_chain_id == Some(56) {
            push(&mut out, &self.config.bsc_rpc_url);
            push(&mut out, "https://bsc-dataseed.binance.org");
            push(&mut out, "https://bsc-rpc.publicnode.com");
            push(&mut out, "https://1rpc.io/bnb");
        } else {
            push(&mut out, &self.config.evm_rpc_url);
            push(&mut out, "https://polygon-bor-rpc.publicnode.com");
            push(&mut out, "https://1rpc.io/matic");
            push(&mut out, "https://polygon.drpc.org");
        }
        out
    }

    /// Live gas floor. Polygon stays at 30 gwei; BSC must not inherit that floor.
    fn min_gas_price_wei(&self) -> u128 {
        if self.coin_params().evm_chain_id == Some(56) {
            50_000_000 // 0.05 gwei
        } else {
            30_000_000_000 // 30 gwei
        }
    }

    fn gas_token_label(&self) -> &'static str {
        if self.coin_params().evm_chain_id == Some(56) { "BNB" } else { "POL" }
    }

    /// PEPE exists only as BEP-20 on BSC mainnet. Chapel has no official token.
    fn pepe_mainnet_ready(&self) -> Result<(), String> {
        if self.coin != Coin::Pepe {
            return Ok(());
        }
        let p = self.coin_params();
        if self.config.network != ChainNetwork::Mainnet || p.evm_chain_id != Some(56) || p.erc20_contract.is_none() {
            return Err("PEPE só existe na BNB Smart Chain mainnet (BEP-20). Testnet recusado.".into());
        }
        Ok(())
    }

    fn dgb(&self) -> crate::dgb_client::DgbClient {
        crate::dgb_client::DgbClient::with_rpc(
            &self.config.dgb_insight_url,
            self.config.dgb_rpc_url.as_deref(),
        )
    }

    fn zer(&self) -> crate::zer_client::ZerClient {
        crate::zer_client::ZerClient::with_rpc(
            &self.config.zer_explorer_url,
            self.config.zer_explorer_api_key.as_deref(),
            self.config.zer_rpc_url.as_deref(),
        )
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

    /// BIP44 receive key (`…/0/{index}`). If that address is empty, the legacy
    /// xpub child (`…/{index}`, no change level) is used when it actually holds
    /// funds — those coins were issued on a path the sweeper could not sign.
    async fn resolve_deposit_signer(&self, mnemonic: &str, hd_index: u32) -> Result<(String, String), BroadcastError> {
        let secret = crate::hd_wallet::secret_from_mnemonic(mnemonic, self.coin, hd_index)
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let from = crate::hd_wallet::address_from_mnemonic(mnemonic, self.coin, self.config.network, hd_index)
            .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        if self.coin == Coin::Sol {
            return Ok((mnemonic.to_string(), from));
        }
        let wif = crate::hd_wallet::secret_to_wif(&secret).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let primary = ChainClient::get_balance(self, &from).await.unwrap_or(0);
        if primary > 0 {
            return Ok((wif, from));
        }
        if let Ok(legacy_secret) = crate::hd_wallet::secret_from_account_index(mnemonic, self.coin, hd_index) {
            if let Ok(legacy_from) = crate::hd_wallet::address_from_secret_bytes(self.coin, self.config.network, &legacy_secret) {
                if !legacy_from.eq_ignore_ascii_case(&from) && ChainClient::get_balance(self, &legacy_from).await.unwrap_or(0) > 0 {
                    tracing::warn!(
                        index = hd_index,
                        coin = %self.coin.as_str(),
                        address = %legacy_from,
                        "sweeping legacy xpub path; BIP44 receive path was empty"
                    );
                    let legacy_wif = crate::hd_wallet::secret_to_wif(&legacy_secret)
                        .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                    return Ok((legacy_wif, legacy_from));
                }
            }
        }
        Ok((wif, from))
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
        let pubkey = derive_receive_pubkey(&self.config.deposit_xpub, index).map_err(|e| ChainError { message: e.to_string() })?;
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
            AddressKind::ZcashTransparent { version } => crate::encoding::zcash_t1_encode(version, &hash160(&compressed_bytes(&pubkey))),
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
            AddressKind::ZcashTransparent { version } => crate::encoding::zcash_t1_validate(address, version),
        }
    }

    async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        if self.coin == Coin::Dgb {
            return self.dgb().fetch_deposits(address).await;
        }
        if self.coin == Coin::Zer {
            return self.zer().fetch_deposits(address).await;
        }
        if self.coin == Coin::Pepe {
            self.pepe_mainnet_ready().map_err(|message| ChainError { message })?;
        }
        let multi_rpc = crate::rpc_client::MultiChainRpcClient::new();
        multi_rpc.fetch_deposits(self.coin, address).await
    }

    async fn get_balance(&self, address: &str) -> Result<u128, ChainError> {
        match self.coin {
            Coin::Sol => {
                let client = crate::sol_client::SolClient::new(&self.config.sol_rpc_url);
                client.get_balance_internal(address).await
            }
            Coin::Dgb => self.dgb().get_balance(address).await,
            Coin::Zer => self.zer().get_balance(address).await,
            Coin::Usdt | Coin::Usdc | Coin::Pepe => {
                if self.coin == Coin::Pepe {
                    self.pepe_mainnet_ready().map_err(|message| ChainError { message })?;
                }
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
            Coin::Usdt | Coin::Usdc | Coin::Pepe => {
                if let Err(message) = self.pepe_mainnet_ready() {
                    return Err(BroadcastError { message, safe_to_reverse: true });
                }
                return self.broadcast_erc20(wif, to_address, amount).await;
            }
            Coin::Sol => return self.broadcast_sol(wif, to_address, amount).await,
            Coin::Dgb => return self.broadcast_dgb(wif, to_address, amount).await,
            Coin::Zer => return self.broadcast_zer(wif, to_address, amount).await,
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
        let (wif, from) = self.resolve_deposit_signer(mnemonic, hd_index).await?;
        if from.eq_ignore_ascii_case(&to) {
            return Ok(None);
        }

        match self.coin {
            Coin::Pol => {
                let wei = self.evm().get_balance(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                let gas_price = self.evm().gas_price().await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?.max(30_000_000_000);
                let fee = gas_price.saturating_mul(21_000);
                // Same BIP-44 index as USDT/USDC. Sweeping all POL would take
                // the gas those token sweeps just received.
                let reserve = if self.polygon_address_holds_stable(&from).await { ERC20_GAS_TOPUP_WEI } else { 0 };
                let Some(send_wei) = pol_native_send_wei(wei, fee, reserve) else {
                    return Ok(None);
                };
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
                // Deposit addresses need native POL to pay ERC-20 gas. Top up
                // from hot, wait for the receipt, then send the token in this
                // same call — returning here used to let the POL sweeper take
                // the gas on the next tick.
                let pol_bal = self.evm().get_balance(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if pol_bal < ERC20_MIN_GAS_WEI {
                    let hot_wif = self.hot_key_material().map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
                    let topup_internal = shared::from_onchain_amount(Coin::Pol, ERC20_GAS_TOPUP_WEI);
                    tracing::info!(deposit = %from, coin = %self.coin.as_str(), "topping up POL gas on deposit before ERC-20 sweep");
                    let top = self.broadcast_pol(&hot_wif, &from, topup_internal).await?;
                    self.evm().wait_receipt_success(&top.tx_hash, 20, 2).await.map_err(|e| BroadcastError {
                        message: format!("POL gas top-up {} not confirmed, ERC-20 sweep not sent: {e}", top.tx_hash),
                        safe_to_reverse: true,
                    })?;
                }
                Ok(Some(self.broadcast_erc20(&wif, &to, internal).await?))
            }
            Coin::Pepe => {
                self.pepe_mainnet_ready().map_err(|message| BroadcastError { message, safe_to_reverse: true })?;
                let token = self.coin_params().erc20_contract.ok_or_else(|| BroadcastError { message: "missing PEPE contract".into(), safe_to_reverse: true })?;
                let raw = self.evm().erc20_balance(token, &from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                let internal = shared::from_onchain_amount(self.coin, raw);
                if internal == 0 {
                    return Ok(None);
                }
                let rpc = self.token_rpc_urls().into_iter().next().unwrap_or_else(|| self.config.bsc_rpc_url.clone());
                let evm = EvmClient::new(&rpc);
                let gas_price = evm
                    .gas_price()
                    .await
                    .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?
                    .max(self.min_gas_price_wei());
                let amount_token = shared::to_onchain_amount(self.coin, internal);
                let to_bytes = parse_address(&to).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                let data = erc20_transfer_data(to_bytes, amount_token);
                let data_hex = format!("0x{}", hex::encode(&data));
                // estimateGas can fail when the deposit address has no BNB yet. Size the
                // top-up from a conservative BEP-20 limit; the real send still estimates.
                let gas_limit = evm.estimate_gas_data(&from, token, &data_hex).await.unwrap_or(80_000).max(65_000);
                let needed = gas_price.saturating_mul(u128::from(gas_limit));
                let bnb_bal = evm.get_balance(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if bnb_bal < needed {
                    let hot_wif = self.hot_key_material().map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
                    let topup = needed.saturating_mul(2);
                    tracing::info!(deposit = %from, topup_wei = topup, "topping up BNB gas on deposit before PEPE sweep");
                    let top = self.broadcast_native_wei(&hot_wif, &from, topup).await?;
                    let _ = evm.wait_receipt_success(&top.tx_hash, 20, 3).await;
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
                let secret = crate::hd_wallet::secret_from_mnemonic(mnemonic, Coin::Sol, hd_index)
                    .map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                Ok(Some(client.broadcast_transfer(&secret, &to, send).await?))
            }
            Coin::Zer => {
                if !self.zer().rpc_ready() {
                    return Err(BroadcastError {
                        message: "ZER_RPC_URL required to sweep ZER".into(),
                        safe_to_reverse: true,
                    });
                }
                let bal = self.zer().get_balance(&from).await.map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
                if bal < 1 {
                    return Ok(None);
                }
                Ok(Some(self.broadcast_zer(&wif, &to, bal).await?))
            }
            Coin::Dgb => {
                let client = self.dgb();
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

    async fn broadcast_evm_contract_call(&self, call: EvmContractCall) -> Result<BroadcastResult, BroadcastError> {
        match self.coin {
            Coin::Pol | Coin::Usdt | Coin::Usdc => self.broadcast_polygon_contract_call(call).await,
            other => Err(BroadcastError {
                message: format!("contractCall not supported for {}", other.as_str()),
                safe_to_reverse: true,
            }),
        }
    }

    async fn broadcast_solana_relay_tx(&self, solana_tx: &serde_json::Value) -> Result<BroadcastResult, BroadcastError> {
        if self.coin != Coin::Sol {
            return Err(BroadcastError {
                message: format!("solanaRelay not supported for {}", self.coin.as_str()),
                safe_to_reverse: true,
            });
        }
        let wif = self.hot_key_material().map_err(|e| BroadcastError {
            message: e,
            safe_to_reverse: true,
        })?;
        let master = crate::sol_client::sol_master_from_hot_key(&wif);
        let secret = crate::sol_client::derive_sol_secret(&master, b"bitcosats-sol-hot", 0);
        let client = crate::sol_client::SolClient::new(&self.config.sol_rpc_url);
        client.broadcast_relay_tx(&secret, solana_tx).await
    }
}

impl RealChainClient {
    /// Sign + broadcast SwapKit/1inch-style calldata on Polygon (chain id from params).
    async fn broadcast_polygon_contract_call(&self, call: EvmContractCall) -> Result<BroadcastResult, BroadcastError> {
        let chain_id = self.coin_params().evm_chain_id.ok_or_else(|| BroadcastError {
            message: "missing EVM chain id".into(),
            safe_to_reverse: true,
        })?;
        let wif = self.hot_key_material().map_err(|e| BroadcastError {
            message: e,
            safe_to_reverse: true,
        })?;
        let secret = parse_secret_key_bytes(&wif).map_err(|e| BroadcastError {
            message: e,
            safe_to_reverse: true,
        })?;
        let from = address_from_secret(&secret).map_err(|e| BroadcastError {
            message: e.to_string(),
            safe_to_reverse: true,
        })?;
        let from_hex = format!("0x{}", hex::encode(from));

        if let Some(hint) = call.from_hint.as_deref() {
            let hint_bytes = parse_address(hint).map_err(|e| BroadcastError {
                message: format!("bad from hint: {e}"),
                safe_to_reverse: true,
            })?;
            if hint_bytes != from {
                return Err(BroadcastError {
                    message: format!("contractCall from mismatch: quote={hint} hot={from_hex}"),
                    safe_to_reverse: true,
                });
            }
        }

        let to = parse_address(&call.to).map_err(|e| BroadcastError {
            message: format!("bad contract to: {e}"),
            safe_to_reverse: true,
        })?;
        let data_hex = call.data_hex.trim();
        if !data_hex.starts_with("0x") && !data_hex.starts_with("0X") {
            return Err(BroadcastError {
                message: "contractCall data must be 0x-hex".into(),
                safe_to_reverse: true,
            });
        }
        let data = hex::decode(data_hex.strip_prefix("0x").or_else(|| data_hex.strip_prefix("0X")).unwrap_or(data_hex))
            .map_err(|e| BroadcastError {
                message: format!("bad contractCall data: {e}"),
                safe_to_reverse: true,
            })?;
        if data.is_empty() {
            return Err(BroadcastError {
                message: "contractCall data empty".into(),
                safe_to_reverse: true,
            });
        }

        // ERC-20 sells: approve SwapKit's `approvalAddress` first (required).
        // Only fall back to tx.to / known routers when approvalAddress is absent.
        let mut approve_fee = 0u128;
        if let Some(token) = self.coin_params().erc20_contract {
            let need = call.erc20_sell_amount.unwrap_or(1);
            let mut spenders: Vec<String> = Vec::new();
            if let Some(a) = call.approval_address.as_ref().filter(|s| !s.is_empty()) {
                spenders.push(a.clone());
            }
            // Router itself sometimes pulls; keep as secondary.
            if !spenders.iter().any(|x| x.eq_ignore_ascii_case(&call.to)) {
                spenders.push(call.to.clone());
            }
            // Only add known routers when SwapKit did not tell us the spender —
            // shotgun-approving 6 routers burns gas/nonce and was masking failures.
            if call.approval_address.is_none() {
                for s in polygon_dex_spender_candidates(&data) {
                    if !spenders.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                        spenders.push(s);
                    }
                }
            }
            for (i, spender) in spenders.iter().enumerate() {
                match self
                    .ensure_erc20_allowance(&secret, &from_hex, token, spender, need, chain_id)
                    .await
                {
                    Ok(fee) => approve_fee = approve_fee.saturating_add(fee),
                    Err(e) => {
                        // First spender (approvalAddress or tx.to) is required.
                        if i == 0 {
                            return Err(e);
                        }
                        tracing::warn!(spender, error = %e.message, "optional DEX spender approve failed");
                    }
                }
            }
        }

        let rpc_endpoints = [
            self.config.evm_rpc_url.as_str(),
            "https://polygon-bor-rpc.publicnode.com",
            "https://1rpc.io/matic",
            "https://polygon.drpc.org",
            "https://polygon-mainnet.public.blastapi.io",
        ];

        let mut last_error = String::new();
        let mut seen = std::collections::HashSet::new();
        for &rpc_url in &rpc_endpoints {
            if rpc_url.is_empty() || !seen.insert(rpc_url) {
                continue;
            }
            let evm = EvmClient::new(rpc_url);
            let nonce = match evm.get_transaction_count(&from_hex).await {
                Ok(n) => n,
                Err(e) => {
                    last_error = format!("nonce failed on {rpc_url}: {e}");
                    continue;
                }
            };

            let gas_price = match evm.gas_price().await {
                Ok(g) => g
                    .max(call.gas_price_hint.unwrap_or(0))
                    .max(30_000_000_000),
                Err(e) => {
                    last_error = format!("gas_price failed on {rpc_url}: {e}");
                    continue;
                }
            };

            let gas_limit = match evm
                .estimate_gas_call(&from_hex, &call.to, data_hex, call.value_wei)
                .await
            {
                Ok(l) => l
                    .saturating_mul(12)
                    .saturating_div(10)
                    .max(call.gas_limit_hint.unwrap_or(0))
                    .max(100_000),
                Err(e) => {
                    last_error = format!("estimateGas failed on {rpc_url}: {e}");
                    // Do not fall back to a stale quote gas hint — that caused
                    // on-chain reverts after missing ERC-20 allowance.
                    continue;
                }
            };

            if let Err(e) = evm
                .simulate_call(&from_hex, &call.to, data_hex, call.value_wei)
                .await
            {
                last_error = format!("eth_call simulation reverted on {rpc_url}: {e}");
                continue;
            }

            match evm.get_balance(&from_hex).await {
                Ok(bal) => {
                    let need = call
                        .value_wei
                        .saturating_add(gas_price.saturating_mul(gas_limit as u128));
                    if bal < need {
                        return Err(BroadcastError {
                            message: format!(
                                "hot wallet POL insuficiente para contractCall: bal={bal} need={need}"
                            ),
                            safe_to_reverse: true,
                        });
                    }
                }
                Err(e) => {
                    last_error = format!("balance failed on {rpc_url}: {e}");
                    continue;
                }
            }

            let raw = match sign_legacy_tx(
                &secret,
                &LegacyTx {
                    nonce,
                    gas_price_wei: gas_price,
                    gas_limit,
                    to,
                    value_wei: call.value_wei,
                    data: data.clone(),
                    chain_id,
                },
            ) {
                Ok(r) => r,
                Err(e) => {
                    return Err(BroadcastError {
                        message: e.to_string(),
                        safe_to_reverse: true,
                    })
                }
            };

            let fee_amount = approve_fee
                + (gas_price.saturating_mul(gas_limit as u128)) / 10_000_000_000;
            match evm.broadcast(&raw).await {
                Ok(txid) => {
                    match evm.wait_receipt_success(&txid, 30, 2).await {
                        Ok(true) => {
                            tracing::info!(
                                tx_hash = %txid,
                                from = %from_hex,
                                to = %call.to,
                                rpc = %rpc_url,
                                "EVM contractCall broadcasted + confirmed"
                            );
                            return Ok(BroadcastResult {
                                tx_hash: txid,
                                fee_amount,
                            });
                        }
                        Ok(false) => {
                            return Err(BroadcastError {
                                message: format!("contractCall reverted on-chain ({txid})"),
                                // Revert: ERC-20/native principal stays in hot wallet; only gas spent.
                                safe_to_reverse: true,
                            });
                        }
                        Err(e) => {
                            return Err(BroadcastError {
                                message: format!(
                                    "contractCall broadcasted ({txid}) but receipt wait failed: {e}"
                                ),
                                safe_to_reverse: false,
                            });
                        }
                    }
                }
                Err(e) => {
                    last_error = format!("broadcast failed on {rpc_url}: {e}");
                }
            }
        }

        Err(BroadcastError {
            message: format!("EVM contractCall falhou em todos os RPCs: {last_error}"),
            safe_to_reverse: true,
        })
    }

    /// Ensure `spender` can pull at least `need` of `token` from the hot wallet.
    /// Returns network fee paid (ledger POL units, 8 decimals) for any approve txs.
    async fn ensure_erc20_allowance(
        &self,
        secret: &[u8; 32],
        from_hex: &str,
        token: &str,
        spender: &str,
        need: u128,
        chain_id: u64,
    ) -> Result<u128, BroadcastError> {
        let spender_bytes = parse_address(spender).map_err(|e| BroadcastError {
            message: format!("bad approve spender: {e}"),
            safe_to_reverse: true,
        })?;
        let token_addr = parse_address(token).map_err(|e| BroadcastError {
            message: format!("bad token: {e}"),
            safe_to_reverse: true,
        })?;

        let rpc_endpoints = [
            self.config.evm_rpc_url.as_str(),
            "https://polygon-bor-rpc.publicnode.com",
            "https://1rpc.io/matic",
            "https://polygon.drpc.org",
            "https://polygon-mainnet.public.blastapi.io",
        ];

        let mut last_error = String::new();
        let mut seen = std::collections::HashSet::new();
        for &rpc_url in &rpc_endpoints {
            if rpc_url.is_empty() || !seen.insert(rpc_url) {
                continue;
            }
            let evm = EvmClient::new(rpc_url);
            // If allowance cannot be read, assume 0 and attempt approve (fail-open toward approve).
            let allowance = match evm.erc20_allowance(token, from_hex, spender).await {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(rpc = %rpc_url, error = %e, "ERC-20 allowance read failed; trying approve");
                    last_error = format!("allowance failed on {rpc_url}: {e}");
                    0
                }
            };
            if allowance >= need {
                return Ok(0);
            }

            // Some USDT deployments require approve(0) before a non-zero approve.
            let mut fee_total = 0u128;
            if allowance > 0 {
                match self
                    .broadcast_erc20_approve_once(
                        &evm,
                        secret,
                        from_hex,
                        token_addr,
                        erc20_approve_data(spender_bytes, 0),
                        chain_id,
                        rpc_url,
                    )
                    .await
                {
                    Ok(fee) => fee_total = fee_total.saturating_add(fee),
                    Err(e) => {
                        last_error = e.message;
                        continue;
                    }
                }
            }

            match self
                .broadcast_erc20_approve_once(
                    &evm,
                    secret,
                    from_hex,
                    token_addr,
                    erc20_approve_max_data(spender_bytes),
                    chain_id,
                    rpc_url,
                )
                .await
            {
                Ok(fee) => {
                    tracing::info!(token, spender, need, rpc = %rpc_url, "ERC-20 approve(max) confirmed for DEX router");
                    return Ok(fee_total.saturating_add(fee));
                }
                Err(e) => {
                    last_error = e.message;
                }
            }
        }

        Err(BroadcastError {
            message: format!("ERC-20 approve falhou: {last_error}"),
            safe_to_reverse: true,
        })
    }

    async fn broadcast_erc20_approve_once(
        &self,
        evm: &EvmClient,
        secret: &[u8; 32],
        from_hex: &str,
        token: [u8; 20],
        data: Vec<u8>,
        chain_id: u64,
        rpc_url: &str,
    ) -> Result<u128, BroadcastError> {
        let token_hex = format!("0x{}", hex::encode(token));
        let data_hex = format!("0x{}", hex::encode(&data));
        let nonce = evm.get_transaction_count(from_hex).await.map_err(|e| BroadcastError {
            message: format!("approve nonce failed on {rpc_url}: {e}"),
            safe_to_reverse: true,
        })?;
        let gas_price = evm
            .gas_price()
            .await
            .map_err(|e| BroadcastError {
                message: format!("approve gas_price failed on {rpc_url}: {e}"),
                safe_to_reverse: true,
            })?
            .max(30_000_000_000);
        let gas_limit = evm
            .estimate_gas_call(from_hex, &token_hex, &data_hex, 0)
            .await
            .unwrap_or(60_000)
            .max(50_000);
        let raw = sign_legacy_tx(
            secret,
            &LegacyTx {
                nonce,
                gas_price_wei: gas_price,
                gas_limit,
                to: token,
                value_wei: 0,
                data,
                chain_id,
            },
        )
        .map_err(|e| BroadcastError {
            message: e.to_string(),
            safe_to_reverse: true,
        })?;
        let fee_amount = (gas_price.saturating_mul(gas_limit as u128)) / 10_000_000_000;
        let txid = evm.broadcast(&raw).await.map_err(|e| BroadcastError {
            message: format!("approve broadcast failed on {rpc_url}: {e}"),
            safe_to_reverse: true,
        })?;
        match evm.wait_receipt_success(&txid, 30, 2).await {
            Ok(true) => Ok(fee_amount),
            Ok(false) => Err(BroadcastError {
                message: format!("approve reverted ({txid})"),
                safe_to_reverse: true,
            }),
            Err(e) => Err(BroadcastError {
                message: format!("approve receipt wait failed ({txid}): {e}"),
                safe_to_reverse: false,
            }),
        }
    }

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

    /// Native-value transfer on the coin's EVM chain (POL or BNB). `value_wei` is already on-chain units.
    async fn broadcast_native_wei(&self, wif: &str, to_address: &str, value_wei: u128) -> Result<BroadcastResult, BroadcastError> {
        if value_wei == 0 {
            return Err(BroadcastError { message: "native top-up amount is zero".into(), safe_to_reverse: true });
        }
        let chain_id = self.coin_params().evm_chain_id.ok_or_else(|| BroadcastError {
            message: format!("{} sem chain id EVM", self.coin.as_str()),
            safe_to_reverse: true,
        })?;
        let secret = parse_secret_key_bytes(wif).map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let from = address_from_secret(&secret).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let from_hex = format!("0x{}", hex::encode(from));
        let to = parse_address(to_address).map_err(|e| BroadcastError { message: e.to_string(), safe_to_reverse: true })?;
        let min_gas = self.min_gas_price_wei();
        let gas_label = self.gas_token_label();
        let rpc_endpoints = self.token_rpc_urls();
        let mut last_error = String::new();
        for rpc_url in &rpc_endpoints {
            let evm = EvmClient::new(rpc_url);
            let nonce = match evm.get_transaction_count(&from_hex).await {
                Ok(n) => n,
                Err(e) => {
                    last_error = format!("nonce failed on {rpc_url}: {e}");
                    continue;
                }
            };
            let gas_price = match evm.gas_price().await {
                Ok(g) => g.max(min_gas),
                Err(e) => {
                    last_error = format!("gas_price failed on {rpc_url}: {e}");
                    continue;
                }
            };
            let gas_limit = match evm.estimate_gas(&from_hex, to_address, value_wei).await {
                Ok(l) => l.max(21_000),
                Err(_) => 21_000,
            };
            let gas_cost = gas_price.saturating_mul(u128::from(gas_limit));
            match evm.get_balance(&from_hex).await {
                Ok(native) if native < value_wei.saturating_add(gas_cost) => {
                    return Err(BroadcastError {
                        message: format!(
                            "hot wallet {from_hex} sem {gas_label} para taxa: tem {native} wei, precisa {} wei (valor {value_wei} + gas {gas_cost})",
                            value_wei.saturating_add(gas_cost)
                        ),
                        safe_to_reverse: true,
                    });
                }
                Ok(_) => {}
                Err(e) => {
                    last_error = format!("native balance failed on {rpc_url}: {e}");
                    continue;
                }
            }
            let raw = match sign_legacy_tx(
                &secret,
                &LegacyTx {
                    nonce,
                    gas_price_wei: gas_price,
                    gas_limit,
                    to,
                    value_wei,
                    data: Vec::new(),
                    chain_id,
                },
            ) {
                Ok(r) => r,
                Err(e) => return Err(BroadcastError { message: e.to_string(), safe_to_reverse: true }),
            };
            let fee_amount = gas_cost / 10_000_000_000;
            match evm.broadcast(&raw).await {
                Ok(txid) => return Ok(BroadcastResult { tx_hash: txid, fee_amount }),
                Err(e) => last_error = format!("broadcast failed on {rpc_url}: {e}"),
            }
        }
        Err(BroadcastError {
            message: format!("{gas_label} broadcast falhou em todos os RPCs: {last_error}"),
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
        let chain_id = self.coin_params().evm_chain_id.ok_or_else(|| BroadcastError {
            message: format!("{} sem chain id EVM (testnet recusado ou params incompletos)", self.coin.as_str()),
            safe_to_reverse: true,
        })?;
        let min_gas = self.min_gas_price_wei();
        let gas_label = self.gas_token_label();
        let rpc_endpoints = self.token_rpc_urls();
        if rpc_endpoints.is_empty() {
            return Err(BroadcastError { message: "nenhum RPC EVM configurado".into(), safe_to_reverse: true });
        }
        let mut last_error = String::new();
        for rpc_url in &rpc_endpoints {
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
                Ok(g) => g.max(min_gas),
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
            let gas_cost = gas_price.saturating_mul(u128::from(gas_limit));
            match evm.get_balance(&from_hex).await {
                Ok(native) if native < gas_cost => {
                    return Err(BroadcastError {
                        message: format!(
                            "hot wallet {from_hex} sem {gas_label} para taxa: tem {native} wei, precisa {gas_cost} wei (gas_price {gas_price} × gas_limit {gas_limit})"
                        ),
                        safe_to_reverse: true,
                    });
                }
                Ok(_) => {}
                Err(e) => {
                    last_error = format!("native balance failed on {rpc_url}: {e}");
                    continue;
                }
            }
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
        let client = self.dgb();
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

    async fn broadcast_zer(&self, wif: &str, to_address: &str, amount: u128) -> Result<BroadcastResult, BroadcastError> {
        let client = self.zer();
        if !client.rpc_ready() {
            return Err(BroadcastError {
                message: "ZER_RPC_URL required for ZER withdraw (zerod signraw)".into(),
                safe_to_reverse: true,
            });
        }
        if !client.validate_address_kind(to_address, self.config.network) {
            return Err(BroadcastError {
                message: "ZER withdraw accepts only transparent t1 addresses".into(),
                safe_to_reverse: true,
            });
        }
        let hot_address = hot_wallet_address(self.coin, self.config.network, wif)
            .map_err(|e| BroadcastError { message: e, safe_to_reverse: true })?;
        let amount_sats: u64 = amount.try_into().map_err(|_| BroadcastError {
            message: "ZER amount exceeds u64".into(),
            safe_to_reverse: true,
        })?;
        match client.broadcast_signed(wif, to_address, amount_sats, &hot_address).await {
            Ok((txid, fee)) => Ok(BroadcastResult { tx_hash: txid, fee_amount: fee as u128 }),
            Err(e) => {
                // Pre-broadcast failures (UTXO scan, insight disabled, bad addr)
                // never put funds on-chain — safe to reverse the ledger debit.
                let msg = e.message.to_lowercase();
                let safe = msg.contains("insufficient")
                    || msg.contains("required")
                    || msg.contains("method not found")
                    || msg.contains("scantxoutset")
                    || msg.contains("getaddressutxos")
                    || msg.contains("disabled")
                    || msg.contains("no spendable")
                    || msg.contains("accepts only")
                    || msg.contains("invalid")
                    || msg.contains("missing inputs")
                    || msg.contains("tx-expiring-soon")
                    || msg.contains("expiryheight")
                    || msg.contains("params must be an array");
                Err(BroadcastError { message: e.message, safe_to_reverse: safe })
            }
        }
    }
}

/// Spenders that may `transferFrom` the hot wallet during a Polygon DEX contractCall.
/// Always includes known 1inch AggregationRouters; also any 20-byte address embedded
/// in the calldata that matches that known set (case-insensitive).
pub(crate) fn polygon_dex_spender_candidates(calldata: &[u8]) -> Vec<String> {
    const KNOWN: &[&str] = &[
        // 1inch AggregationRouterV6 (Polygon)
        "0x111111125421cA6dc452D289314280a0f8842A65",
        // 1inch AggregationRouterV5 (Polygon)
        "0x1111111254EEB25477B68fb85Ed929f73A960582",
        // 1inch address seen in SwapKit ONEINCH calldata
        "0x111116053f09d34a7eae8102887004445176ca11",
        // Kyber MetaAggregationRouterV2 (Polygon)
        "0x6131B5fae19EA4f9D964eAc0408E4408b66337b5",
        // Uniswap Universal Router (Polygon)
        "0x1095692A6237d83C6a72F3F5eFEdb9A670C49223",
        // 0x Exchange Proxy (Polygon)
        "0xDef1C0ded9bec7F1a1670819833240f027b25EfF",
    ];
    let mut out: Vec<String> = KNOWN.iter().map(|s| (*s).to_string()).collect();
    // Scan ABI words for known spenders referenced in this specific quote.
    if calldata.len() >= 36 {
        let mut i = 4; // skip selector
        while i + 32 <= calldata.len() {
            let word = &calldata[i..i + 32];
            // address is right-aligned in 32-byte word
            if word[..12].iter().all(|b| *b == 0) {
                let addr = format!("0x{}", hex::encode(&word[12..]));
                if KNOWN.iter().any(|k| k.eq_ignore_ascii_case(&addr))
                    && !out.iter().any(|x| x.eq_ignore_ascii_case(&addr))
                {
                    out.push(addr);
                }
            }
            i += 32;
        }
    }
    out
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
        AddressKind::ZcashTransparent { version } => crate::encoding::zcash_t1_encode(version, &hash),
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
        AddressKind::ZcashTransparent { .. } => Err("ZER spends are signed by zerod, not bitcoin scriptPubKeys".into()),
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

/// POL the hot wallet sends so a deposit address can pay one ERC-20 transfer.
const ERC20_GAS_TOPUP_WEI: u128 = 80_000_000_000_000_000; // 0.08 POL
/// Below this the deposit address cannot pay that transfer.
const ERC20_MIN_GAS_WEI: u128 = 50_000_000_000_000_000; // 0.05 POL

/// Native POL a sweep may move. `token_gas_reserve` stays on the address when
/// it still holds USDT/USDC — those coins share this HD path.
fn pol_native_send_wei(balance_wei: u128, fee_wei: u128, token_gas_reserve: u128) -> Option<u128> {
    const DUST_WEI: u128 = 10_000_000_000_000_000; // 0.01 POL
    const MIN_SWEEP_WEI: u128 = 50_000_000_000_000_000; // 0.05 POL
    if token_gas_reserve == 0 && balance_wei < MIN_SWEEP_WEI {
        return None;
    }
    let keep = fee_wei.saturating_add(DUST_WEI).saturating_add(token_gas_reserve);
    if balance_wei <= keep {
        return None;
    }
    let send = balance_wei - fee_wei - token_gas_reserve;
    if send == 0 { None } else { Some(send) }
}

#[cfg(test)]
mod pol_sweep_tests {
    use super::*;

    #[test]
    fn pol_sweep_leaves_gas_when_address_still_holds_usdt() {
        let fee = 21_000u128 * 30_000_000_000;
        let gas_only = ERC20_GAS_TOPUP_WEI;
        assert_eq!(pol_native_send_wei(gas_only, fee, ERC20_GAS_TOPUP_WEI), None);

        let deposited = 10 * 10u128.pow(18);
        let send = pol_native_send_wei(deposited, fee, ERC20_GAS_TOPUP_WEI).unwrap();
        assert_eq!(send, deposited - fee - ERC20_GAS_TOPUP_WEI);
    }

    #[test]
    fn pol_sweep_sends_native_when_no_token_is_left() {
        let fee = 21_000u128 * 30_000_000_000;
        let deposited = 10 * 10u128.pow(18);
        assert_eq!(pol_native_send_wei(deposited, fee, 0), Some(deposited - fee));
        assert_eq!(pol_native_send_wei(ERC20_MIN_GAS_WEI - 1, fee, 0), None);
    }
}
