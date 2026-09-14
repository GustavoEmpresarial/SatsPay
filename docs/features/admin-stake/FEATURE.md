# FEATURE — Admin Stake / Tesouraria

> Doc bruta. Rota UI: `/admin/stake`. Nav label: **Tesouraria** (não “Tesouraria & Hot”).

## Keywords

`tesouraria treasury hot custody solvency fee_margin network_fees P&L break-even runway buffer FEE_MARGIN_HARD_BLOCK DGB Cryptoid`

## O que mostra

1. Tabela hot on-chain × custódia ledger × depósitos × saques × endereço hot  
2. Saúde financeira (solvência, fluxo, receita/custo, atividade 24h, charts)  
3. **9 painéis operacionais** via `GET /v1/admin/treasury-health`:  
   - Resultado (USD) via `price_cache`  
   - Equilíbrio do saque (taxa cobrada × média rede)  
   - Autonomia HOUSE (runway faucet)  
   - Passivos pendentes (saques na fila)  
   - Varreduras pendentes (depósitos não swept)  
   - Reserva da hot (buffer alvo)  
   - Trava de margem (`FEE_MARGIN_HARD_BLOCK`)  
   - Taxas × rede (série 7d)  
   - DGB saldo (fallback Cryptoid se Insight 404)  

## APIs

- `GET /v1/admin/treasury-wallets`  
- `GET /v1/admin/treasury-health`  
- `GET /v1/admin/economics`  

## Backend

- `crates/db/src/treasury_health.rs`  
- `crates/db/src/network_fees.rs` + migration `0024_network_fee_events.sql`  
- Hard block: faucet + withdrawals → `FEE_MARGIN_NEGATIVE`  
- Domínio: [`../domain-treasury-health/FEATURE.md`](../domain-treasury-health/FEATURE.md)  

## Labels pt-BR (não reverter para EN)

Resultado (USD), Equilíbrio do saque, Autonomia da HOUSE, Varreduras pendentes, Reserva da hot, diferença (+/−), histórico (não all-time), tipos rede: saque / varredura / depósito DEX  

## Arquivos

- `client/src/pages/AdminStakePage.tsx`  
- `client/src/components/AdminTreasuryMonitor.tsx`  
- `docs/pages/admin-stake/`  
