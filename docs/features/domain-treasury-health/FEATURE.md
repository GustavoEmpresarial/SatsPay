# FEATURE — Tesouraria & saúde financeira (admin)

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `domain-treasury-health` |
| Título | Tesouraria & saúde financeira (admin) |
| Componente | `—` |
| Auth | **mixed** — pública ou autenticada conforme contexto |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`treasury-health solvency hot custody fee margin P&L break-even runway buffer FEE_MARGIN_HARD_BLOCK`

## Rotas

- (sem rota SPA — domínio backend)

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `GET /v1/admin/treasury-wallets` (prefixo `/v1` no servidor)
- `GET /v1/admin/treasury-health` (prefixo `/v1` no servidor)
- `GET /v1/admin/economics` (prefixo `/v1` no servidor)

## Arquivos-chave

- `crates/db/src/treasury_health.rs`
- `crates/db/src/network_fees.rs`
- `client/src/pages/AdminStakePage.tsx`
- `docs/pages/admin-stake/`

## Comportamento (bruto)

9 painéis: P&L USD, break-even, runway HOUSE, passivos, sweeps, buffer hot, trava margem, série 7d, DGB OK.

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
