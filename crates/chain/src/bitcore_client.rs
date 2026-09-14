//! Real HTTP client for Bitpay's public Bitcore/Insight API
//! (`https://api.bitcore.io`), which serves BTC/LTC/DOGE/BCH from one
//! uniform REST surface without requiring an API key for read access —
//! verified live against mainnet during development (see `docs/decisions/`).
//! Broadcast (`/tx/send`) is the one write endpoint and needs no key either,
//! but obviously needs a *signed* transaction, which needs the hot wallet's
//! private key (see `signing.rs`).

use crate::types::OnchainTx;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum BitcoreError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected response shape: {0}")]
    UnexpectedShape(String),
}

pub struct BitcoreClient {
    http: reqwest::Client,
    base_url: String,
    chain: &'static str,
    network: &'static str,
}

#[derive(Debug, Deserialize)]
struct BalanceResponse {
    confirmed: i64,
    #[allow(dead_code)]
    unconfirmed: i64,
}

#[derive(Debug, Deserialize)]
struct CoinEntry {
    #[serde(rename = "mintTxid")]
    mint_txid: String,
    #[serde(rename = "mintIndex")]
    mint_index: u32,
    #[serde(rename = "mintHeight")]
    mint_height: i64,
    value: i64,
    address: String,
}

#[derive(Debug, Deserialize)]
struct BlockTip {
    height: i64,
}

impl BitcoreClient {
    /// `base_url` defaults to `https://api.bitcore.io` in production; tests
    /// and self-hosted deployments can point it elsewhere via config —
    /// never hardcoded past this one configurable constructor argument.
    pub fn new(base_url: &str, chain: &'static str, network: &'static str) -> Self {
        Self { http: reqwest::Client::new(), base_url: base_url.trim_end_matches('/').to_string(), chain, network }
    }

    fn api_path(&self, suffix: &str) -> String {
        format!("{}/api/{}/{}{}", self.base_url, self.chain, self.network, suffix)
    }

    /// Bitcore's address endpoints reject BCH's `"bitcoincash:"`-prefixed
    /// CashAddr form (returns an empty/zero result instead of erroring —
    /// found by comparing a live prefixed vs. bare query against the same
    /// known-funded address during development). None of BTC/LTC/DOGE's
    /// address formats ever contain a colon, so stripping everything up to
    /// and including the first `:` is safe across all four coins this
    /// client serves.
    fn bare_address(address: &str) -> &str {
        address.split(':').next_back().unwrap_or(address)
    }

    pub async fn tip_height(&self) -> Result<i64, BitcoreError> {
        let tip: BlockTip = self.http.get(self.api_path("/block/tip")).send().await?.error_for_status()?.json().await?;
        Ok(tip.height)
    }

    /// Confirmed balance, in the coin's smallest unit.
    pub async fn get_balance(&self, address: &str) -> Result<u128, BitcoreError> {
        let resp: BalanceResponse = self.http.get(self.api_path(&format!("/address/{}/balance", Self::bare_address(address)))).send().await?.error_for_status()?.json().await?;
        Ok(resp.confirmed.max(0) as u128)
    }

    /// Every UTXO ever received at `address` (spent or not), with
    /// confirmations computed against the current chain tip — the API's own
    /// `confirmations` field on this endpoint isn't reliably populated, so
    /// this recomputes it from `mintHeight`, which is always present.
    pub async fn fetch_received(&self, address: &str) -> Result<Vec<OnchainTx>, BitcoreError> {
        let tip = self.tip_height().await?;
        let coins: Vec<CoinEntry> = self.http.get(self.api_path(&format!("/address/{}/txs?limit=100", Self::bare_address(address)))).send().await?.error_for_status()?.json().await?;
        Ok(coins
            .into_iter()
            .map(|c| {
                let confirmations = if c.mint_height > 0 { (tip - c.mint_height + 1).max(0) as u32 } else { 0 };
                OnchainTx { tx_hash: c.mint_txid, vout: c.mint_index, amount: c.value.max(0) as u128, confirmations, address: c.address }
            })
            .collect())
    }

    /// Raw unspent-output entries for `address` — deserialized by the
    /// caller into whatever shape it needs (see `real_client::fetch_spendable_utxos`),
    /// since the fields used differ from `fetch_received`'s `OnchainTx`.
    pub async fn get_unspent_raw<T: for<'de> Deserialize<'de>>(&self, address: &str) -> Result<Vec<T>, BitcoreError> {
        Ok(self.http.get(self.api_path(&format!("/address/{}/?unspent=true&limit=1000", Self::bare_address(address)))).send().await?.error_for_status()?.json().await?)
    }

    /// Broadcasts a raw signed transaction (hex-encoded). Returns the txid.
    pub async fn broadcast(&self, raw_tx_hex: &str) -> Result<String, BitcoreError> {
        #[derive(serde::Serialize)]
        struct SendReq<'a> {
            #[serde(rename = "rawTx")]
            raw_tx: &'a str,
        }
        #[derive(Deserialize)]
        struct SendResp {
            txid: String,
        }
        let resp: SendResp = self.http.post(self.api_path("/tx/send")).json(&SendReq { raw_tx: raw_tx_hex }).send().await?.error_for_status()?.json().await?;
        Ok(resp.txid)
    }
}
