# FEATURE — Analytics

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `analytics` |
| Título | Analytics |
| Componente | `AnalyticsPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`analytics AnalyticsPage /analytics /swap/prices /wallet /wallet/ledger  user`

## Rotas

- `/analytics`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/swap/prices` (prefixo `/v1` no servidor)
- `/wallet` (prefixo `/v1` no servidor)
- `/wallet/ledger` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AnalyticsPage.tsx`
- `docs/pages/analytics/`

## Comportamento (bruto)

Página React `AnalyticsPage`. Chama 3 endpoint(s) via `api()`.

## Notas de overview legado

# Analytics — Overview

## Papel

Página **Analytics** (`AnalyticsPage.tsx`).

- Auth gate: **user**
- Rotas: `/analytics`


## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `user` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.


## Bugs / armadilhas conhecidas

- “Por operação” **não** inclui `ADJUSTMENT` (seed/manual admin) — inflava USD sem ser atividade do usuário.
- Estorno de saque (`WITHDRAWAL_REVERSAL`) liquida o volume de `WITHDRAWAL` (não soma as duas barras).
- Dust faucet/depósito → `formatPortfolioUsd` mostra `< $0.01` quando há movimento real.
- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
