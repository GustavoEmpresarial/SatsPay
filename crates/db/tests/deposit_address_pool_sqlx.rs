//! ADR 0012 SOL deposit pool: the worker (signer) pre-derives addresses, the
//! keyless api-server claims them. Concurrency, exhaustion and key continuity.

use chain::hd_wallet::address_from_mnemonic;
use chain::{ChainClient, ChainNetwork, PublicWalletConfig, RealChainClient, RealClientConfig, DEPOSIT_ADDRESS_POOL_EMPTY};
use shared::Coin;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;

const WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

fn config(deposit_mnemonic: Option<&str>) -> RealClientConfig {
    RealClientConfig {
        bitcore_base_url: "http://127.0.0.1:9".into(),
        evm_rpc_url: "http://127.0.0.1:9".into(),
        bsc_rpc_url: "http://127.0.0.1:9".into(),
        wallet: PublicWalletConfig::default(),
        hot_wallet_wif: None,
        evm_deposit_lookback_blocks: 10,
        fee_confirmation_target: 2,
        network: ChainNetwork::Mainnet,
        sol_rpc_url: "http://127.0.0.1:9".into(),
        dgb_insight_url: "http://127.0.0.1:9".into(),
        dgb_rpc_url: None,
        zer_explorer_url: "http://127.0.0.1:9".into(),
        zer_explorer_api_key: None,
        zer_rpc_url: None,
        deposit_mnemonic: deposit_mnemonic.map(str::to_string),
        hot_mnemonic: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn keyless_api_claims_distinct_worker_derived_sol_addresses(pool: PgPool) {
    let signer = RealChainClient::new(Coin::Sol, pool.clone(), config(Some(WORDS)));
    let api = Arc::new(RealChainClient::new(Coin::Sol, pool.clone(), config(None)));

    assert_eq!(signer.top_up_deposit_pool(5).await.unwrap(), 5);
    assert_eq!(signer.top_up_deposit_pool(5).await.unwrap(), 0, "already at target");

    let claims = (0..10).map(|_| {
        let api = api.clone();
        tokio::spawn(async move { api.generate_address("user").await })
    });
    let results: Vec<_> = futures_join(claims).await;

    let ok: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let empty = results.iter().filter(|r| r.as_ref().is_err_and(|e| e.message.starts_with(DEPOSIT_ADDRESS_POOL_EMPTY))).count();
    assert_eq!(ok.len(), 5, "exactly the pooled rows are handed out");
    assert_eq!(empty, 5, "the rest fail with a stable code, never a reused address");
    let distinct: HashSet<_> = ok.iter().map(|g| g.address.clone()).collect();
    assert_eq!(distinct.len(), 5);

    // Continuity: the worker can re-derive every claimed address from its index.
    for g in ok {
        let index = g.hd_index.expect("pooled rows carry hd_index");
        assert_eq!(g.address, address_from_mnemonic(WORDS, Coin::Sol, ChainNetwork::Mainnet, index).unwrap());
    }

    assert_eq!(signer.top_up_deposit_pool(3).await.unwrap(), 3, "refills after claims");
}

#[sqlx::test(migrations = "./migrations")]
async fn keyless_client_cannot_fill_pool_or_derive_sol_from_public_data(pool: PgPool) {
    let api = RealChainClient::new(Coin::Sol, pool.clone(), config(None));
    let err = api.top_up_deposit_pool(5).await.unwrap_err();
    assert!(err.message.starts_with(chain::SIGNER_NOT_AVAILABLE), "{}", err.message);

    // Empty pool: no fallback derivation from an xpub or any other public value.
    let err = api.generate_address("user").await.unwrap_err();
    assert!(err.message.starts_with(DEPOSIT_ADDRESS_POOL_EMPTY), "{}", err.message);
    let issued: i64 = sqlx::query_scalar("SELECT count(*) FROM deposit_address_pool").fetch_one(&pool).await.unwrap();
    assert_eq!(issued, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn keyless_client_without_xpub_refuses_utxo_addresses(pool: PgPool) {
    let api = RealChainClient::new(Coin::Btc, pool.clone(), config(None));
    let err = api.generate_address("user").await.unwrap_err();
    assert!(err.message.starts_with("CHAIN_CONFIG_MISSING_XPUB"), "{}", err.message);
}

async fn futures_join<T: Send + 'static>(handles: impl Iterator<Item = tokio::task::JoinHandle<T>>) -> Vec<T> {
    let mut out = Vec::new();
    for h in handles.collect::<Vec<_>>() {
        out.push(h.await.expect("task"));
    }
    out
}
