# FEATURE — Features

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `features` |
| Título | Features |
| Componente | `FeaturesPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`features FeaturesPage /features   public`

## Rotas

- `/features`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/FeaturesPage.tsx`
- `docs/pages/features/`

## Comportamento (bruto)

Página React `FeaturesPage`. Chama 0 endpoint(s) via `api()`.

## Notas de overview legado

# Features — Overview

## Papel

Página **Features** (`FeaturesPage.tsx`).

- Auth gate: **public**
- Rotas: `/features`
- Nota: MarketingLayout

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
