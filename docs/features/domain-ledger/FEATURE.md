# FEATURE — Ledger contábil (partidas dobradas)

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `domain-ledger` |
| Título | Ledger contábil (partidas dobradas) |
| Componente | `—` |
| Auth | **mixed** — pública ou autenticada conforme contexto |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`ledger balance wallet_entries double-entry HOUSE PERSONAL DEVELOPER saldo`

## Rotas

- (sem rota SPA — domínio backend)

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `crates/db/src/ledger.rs`
- `docs/architecture/ledger.md`
- `docs/security/BALANCE_SECURITY.md`
- `docs/database/ledger-invariants.md`

## Comportamento (bruto)

Saldos NÃO vivem em coluna mutável — sempre SUM(wallet_entries). Travas na linha wallets.

## Notas de overview legado

_sem overview em docs/pages_

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
