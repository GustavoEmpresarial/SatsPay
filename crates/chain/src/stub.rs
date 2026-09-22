//! Port 1:1 of legacy `apps/api/src/integrations/stubClient.ts`.
//!
//! Deterministic stub — replace with real RPC clients (Bitcoin Core,
//! Electrum, an EVM provider, ...) before going to production. Never move
//! real funds with this.

use crate::types::{BroadcastResult, ChainClient, ChainError, OnchainTx};
use async_trait::async_trait;
use rand::RngCore;
use regex::Regex;
use sha2::{Digest, Sha256};
use shared::Coin;

// base58/base32-safe body derived from a hex digest (maps to lowercase
// letters valid in every alphabet checked below, so generated addresses
// pass their own validator).
const SAFE_ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz"; // no 0/1/l/o ambiguity

fn safe_body(hex: &str, len: usize) -> String {
    let hex_bytes: Vec<u8> = hex.bytes().collect();
    (0..len)
        .map(|i| {
            let c = hex_bytes[i % hex_bytes.len()] as char;
            let nibble = c.to_digit(16).unwrap_or(0) as usize;
            SAFE_ALPHABET[nibble % SAFE_ALPHABET.len()] as char
        })
        .collect()
}

fn validator_for(coin: Coin) -> Regex {
    let pattern = match coin {
        Coin::Btc => r"^bc1q[0-9a-z]{20,60}$",
        Coin::Ltc => r"^ltc1q[0-9a-z]{20,60}$",
        Coin::Doge => r"^D[1-9A-HJ-NP-Za-km-z]{25,40}$",
        Coin::Bch => r"^bitcoincash:q[0-9a-z]{38,50}$",
        Coin::Pol | Coin::Usdt | Coin::Usdc | Coin::Pepe => r"^0x[0-9a-fA-F]{40}$",
        Coin::Dgb => r"^dgb1q[0-9a-z]{20,60}$",
        Coin::Sol => r"^[1-9A-HJ-NP-Za-km-z]{32,44}$",
        Coin::Zer => r"^t1[1-9A-HJ-NP-Za-km-z]{20,}$",
    };
    Regex::new(pattern).expect("static regex is valid")
}

pub struct StubClient {
    coin: Coin,
}

impl StubClient {
    pub fn new(coin: Coin) -> Self {
        Self { coin }
    }
}

#[async_trait]
impl ChainClient for StubClient {
    fn coin(&self) -> Coin {
        self.coin
    }

    async fn generate_address(&self, user_id: &str) -> Result<crate::GeneratedAddress, ChainError> {
        let mut nonce = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut nonce);
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}:{}", self.coin.as_str(), user_id, hex::encode(nonce)));
        let h = hex::encode(hasher.finalize());

        let address = match self.coin {
            Coin::Btc => format!("bc1q{}", &h[..38]),
            Coin::Ltc => format!("ltc1q{}", &h[..38]),
            Coin::Doge => format!("D{}", safe_body(&h, 33)),
            Coin::Bch => format!("bitcoincash:q{}", safe_body(&h, 41)),
            Coin::Pol | Coin::Usdt | Coin::Usdc | Coin::Pepe => format!("0x{}", &h[..40]),
            Coin::Dgb => format!("dgb1q{}", &h[..38]),
            Coin::Zer => format!("t1{}", safe_body(&h, 33)),
            Coin::Sol => {
                let mut pk = [0u8; 32];
                let raw = hex::decode(&h).unwrap_or_default();
                for (i, b) in raw.iter().take(32).enumerate() {
                    pk[i] = *b;
                }
                bs58::encode(pk).into_string()
            }
        };
        Ok(crate::GeneratedAddress {
            address,
            hd_index: Some(0),
        })
    }

    fn validate_address(&self, address: &str) -> bool {
        address.len() <= 128 && validator_for(self.coin).is_match(address)
    }

    async fn fetch_deposits(&self, address: &str) -> Result<Vec<OnchainTx>, ChainError> {
        let multi_rpc = crate::rpc_client::MultiChainRpcClient::new();
        multi_rpc.fetch_deposits(self.coin, address).await
    }

    async fn broadcast_withdrawal(
        &self,
        _to_address: &str,
        _amount: u128,
    ) -> Result<BroadcastResult, crate::types::BroadcastError> {
        Err(crate::types::BroadcastError {
            message: format!("Transmissão on-chain indisponível no cliente stub para {}. Configure USE_REAL_CHAIN_CLIENTS=true e HOT_WALLET_WIF.", self.coin.as_str()),
            safe_to_reverse: true,
        })
    }

    async fn get_balance(&self, address: &str) -> Result<u128, ChainError> {
        let multi_rpc = crate::rpc_client::MultiChainRpcClient::new();
        let deposits = multi_rpc.fetch_deposits(self.coin, address).await?;
        let total = deposits.into_iter().map(|d| d.amount).sum();
        Ok(total)
    }
}
