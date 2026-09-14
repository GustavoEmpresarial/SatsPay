# FEATURE — Sitemap

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `sitemap` |
| Título | Sitemap |
| Componente | `SitemapPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`sitemap SitemapPage /sitemap  Home / Dashboard Welcome / Landing Sign In Sign Up Wallets Faucet Faucet Directory Swap Stake Lend Analytics Documentation API Reference Privacy Policy Terms of Service Cookie Policy Security FAQ public`

## Rotas

- `/sitemap`

## Abas / seções internas

- Home / Dashboard
- Welcome / Landing
- Sign In
- Sign Up
- Wallets
- Faucet
- Faucet Directory
- Swap
- Stake
- Lend
- Analytics
- Documentation
- API Reference
- Privacy Policy
- Terms of Service
- Cookie Policy
- Security
- FAQ

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/SitemapPage.tsx`
- `docs/pages/sitemap/`

## Comportamento (bruto)

Página React `SitemapPage`. Chama 0 endpoint(s) via `api()`. Abas/labels: Home / Dashboard, Welcome / Landing, Sign In, Sign Up, Wallets, Faucet, Faucet Directory, Swap, Stake, Lend, Analytics, Documentation.

## Notas de overview legado

# Sitemap — Overview

## Papel

Página **Sitemap** (`SitemapPage.tsx`).

- Auth gate: **public**
- Rotas: `/sitemap`


## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `public` (RequireAuth / RequireAdmin / público).
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
