# FEATURE — Tesouraria & saúde financeira (domínio)

## Keywords

`treasury-health hot custody solvency fee_margin network_fee_events price_cache break-even runway buffer FEE_MARGIN_HARD_BLOCK`

## Endpoints

- `GET /v1/admin/treasury-wallets` — hot + deposit wallets on-chain vs ledger  
- `GET /v1/admin/treasury-health` — 9 painéis agregados  
- `GET /v1/admin/economics` — janelas all_time / last_24h + fee_margin_by_coin  

## Persistência de custo de rede

- Tabela via migration `0024_network_fee_events.sql`  
- Grava em: broadcast saque, sweep depósito, DEX deposit  
- `crates/db/src/network_fees.rs`  

## Hard block

Env `FEE_MARGIN_HARD_BLOCK` (default ON): bloqueia faucet/saque se margem taxas−rede negativa → código `FEE_MARGIN_NEGATIVE`.

## UI

[`../admin-stake/FEATURE.md`](../admin-stake/FEATURE.md)

## Arquivos

- `crates/db/src/treasury_health.rs`  
- `crates/api-http/src/admin.rs`  
- `crates/chain/src/dgb_client.rs` (fallback Cryptoid)  
