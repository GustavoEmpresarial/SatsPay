# FEATURE — Api Docs

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `api-docs` |
| Título | Api Docs |
| Componente | `ApiDocsPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`api docs ApiDocsPage /docs  loja-producao public`

## Rotas

- `/docs`

## Abas / seções internas

- loja-producao

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/ApiDocsPage.tsx`
- `docs/pages/api-docs/`

## Comportamento (bruto)

Página React `ApiDocsPage`. Chama 0 endpoint(s) via `api()`. Abas/labels: loja-producao. Página pública do contrato da API. Tem de bater com `crates/api-http/src/merchant_deposits.rs` e `crates/webhooks/src/lib.rs` — guardado por `client/tests/unit/contract/merchantGateway.contract.test.ts`. Host único em `API_BASE`; tabela de moedas vem de `client/src/shared/coins.ts`. `GET /v1/me/export` e `POST /v1/me/erase` são LGPD do usuário logado (Settings), não superfície HMAC — ficam no `http-api-reference.md`, não nesta página.

## Notas de overview legado

# API Docs — Overview

## Papel

Página **API Docs** (`ApiDocsPage.tsx`).

- Auth gate: **mixed**
- Rotas: `/api`, `/docs`, `/api-docs`
- Nota: Redirect /api-docs→/docs

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `mixed` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.
- `POST /v1/public/send`: `toEmail` digitado ou `email` do userinfo. Falha com `{ "error", "code" }`: `TARGET_INELIGIBLE`, `SEND_TO_SELF` (não é saldo), `DAILY_LIMIT_REACHED`, `WALLET_NOT_FOUND`. Checkout da própria fatura: `CANNOT_PAY_OWN_INVOICE`. Nada debitado.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
