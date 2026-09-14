# FEATURE — Merchant Dashboard

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `merchant-dashboard` |
| Título | Merchant Dashboard |
| Componente | `MerchantDashboardPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`merchant dashboard MerchantDashboardPage /merchant/dashboard /merchant /api-keys /merchant/deposits /swap/prices /wallet  user`

## Rotas

- `/merchant/dashboard`
- `/merchant`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/api-keys` (prefixo `/v1` no servidor)
- `/merchant/deposits` (prefixo `/v1` no servidor)
- `/swap/prices` (prefixo `/v1` no servidor)
- `/wallet` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/MerchantDashboardPage.tsx`
- `docs/pages/merchant-dashboard/`

## Comportamento (bruto)

Página React `MerchantDashboardPage`. Chama 4 endpoint(s) via `api()`.

## Notas de overview legado

# Merchant Dashboard — Overview

## Papel

Página **Merchant Dashboard** (`MerchantDashboardPage.tsx`).

- Auth gate: **user**
- Rotas: `/merchant`, `/merchant/dashboard`


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

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
