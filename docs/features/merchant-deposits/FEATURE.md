# FEATURE — Merchant Deposits

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `merchant-deposits` |
| Título | Merchant Deposits |
| Componente | `MerchantDepositsPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`merchant deposits MerchantDepositsPage /merchant/deposits /merchant/deposits /merchant/deposits/:id/test-webhook  user`

## Rotas

- `/merchant/deposits`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/merchant/deposits` (prefixo `/v1` no servidor)
- `/merchant/deposits/:id/test-webhook` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/MerchantDepositsPage.tsx`
- `docs/pages/merchant-deposits/`

## Comportamento (bruto)

Página React `MerchantDepositsPage`. Chama 2 endpoint(s) via `api()`. Endereço HD por coin; watcher no worker credita ledger.

Confirmação de invoice **somente** via watcher on-chain ou `POST /v1/public/pay/:id/balance` (debita o pagador). Não existe sandbox público `simulate-payment`. HMAC do webhook é derivado por `merchant_id` (`GET /v1/merchant/webhook-signing-secret`).

**Pausa temporária:** gateway `POST /v1/merchant/deposits` (e aliases `/deposits/create`, `/invoices`) para `BTC`/`LTC`/`DOGE`/`DGB` → `503` `DEPOSIT_PAUSED`. Envio ledger `/v1/public/send` **não** pausa.

## Notas de overview legado

# Merchant Deposits — Overview

## Papel

Página **Merchant Deposits** (`MerchantDepositsPage.tsx`).

- Auth gate: **user**
- Rotas: `/merchant/deposits`


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
