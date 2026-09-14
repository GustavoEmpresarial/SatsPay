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

`api docs ApiDocsPage /docs  cURL Node.js Python PHP Go Rust public`

## Rotas

- `/docs`

## Abas / seções internas

- cURL
- Node.js
- Python
- PHP
- Go
- Rust

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/ApiDocsPage.tsx`
- `docs/pages/api-docs/`

## Comportamento (bruto)

Página React `ApiDocsPage`. Chama 0 endpoint(s) via `api()`. Abas/labels: cURL, Node.js, Python, PHP, Go, Rust.

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

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
