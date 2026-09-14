# FEATURE — Ledger contábil (partidas dobradas)

## Keywords

`ledger wallet_entries double-entry balance HOUSE PERSONAL DEVELOPER FAUCET WITHDRAWAL DEPOSIT SWAP`

## Regra de ouro

**Não existe coluna de saldo mutável confiável.** Saldo = `SUM(wallet_entries.amount)` por wallet.

## Onde

- `crates/db/src/ledger.rs`  
- `docs/architecture/ledger.md`  
- `docs/security/BALANCE_SECURITY.md`  
- `docs/database/ledger-invariants.md`  
- ADR: `docs/decisions/0002-ledger-contabil-partidas-dobradas.md`  

## Concorrência

Lock pessimista na linha `wallets` (não no aggregate do ledger).  
Ver ADR `0006-lock-wallet-row-not-ledger-aggregate.md`.

## HOUSE

Conta plataforma (`house::HOUSE_EMAIL`) — faucet debita HOUSE; fees creditam.  

## Testes

- Invariantes em `docs/database/ledger-invariants.md`  
- SQLx suites em `crates/db/tests/`  
