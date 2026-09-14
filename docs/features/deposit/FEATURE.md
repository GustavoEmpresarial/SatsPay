# FEATURE — Deposit

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `deposit` |
| Título | Deposit |
| Componente | `DepositPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`deposit DepositPage /deposit /deposits/address/:id /deposits/history DEPOSIT_PAUSED DEPOSIT_WITHDRAW_PAUSED_COINS user`

## Rotas

- `/deposit`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/deposits/address/:id` (prefixo `/v1` no servidor)
- `/deposits/history` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/DepositPage.tsx`
- `docs/pages/deposit/`

## Comportamento (bruto)

Página React `DepositPage`. Chama 2 endpoint(s) via `api()`. Endereço HD por coin; watcher no worker credita ledger.

**Pausa temporária:** `BTC` / `LTC` / `DOGE` / `DGB` — UI lista com badge «Pausado»; `GET /v1/deposits/address/:coin` → `503` `DEPOSIT_PAUSED`. Lista: `shared::DEPOSIT_WITHDRAW_PAUSED_COINS` / `client/src/shared/coins.ts`. `/v1/public/send` **não** pausa.

## Notas de overview legado

# Deposit — Overview

## Papel

Página **Deposit** (`DepositPage.tsx`).

- Auth gate: **user**
- Rotas: `/deposit`
- Nota: HD address

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
