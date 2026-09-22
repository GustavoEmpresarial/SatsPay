# FEATURE — Admin Overview

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-overview` |
| Título | Admin Overview |
| Componente | `AdminOverviewPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin overview AdminOverviewPage /admin /admin/economics /admin/stats  admin`

## Rotas

- `/admin`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/admin/economics` (prefixo `/v1` no servidor)
- `/admin/stats` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminOverviewPage.tsx`
- `docs/pages/admin-overview/`

## Comportamento (bruto)

Página React `AdminOverviewPage`. Chama 2 endpoint(s) via `api()`. UI admin sempre pt-BR.

## Notas de overview legado

# Admin Overview — Overview

## Papel

Página **Admin Overview** (`AdminOverviewPage.tsx`).

- Auth gate: **admin**
- Rotas: `/admin`
- Nota: index route

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `admin` (RequireAuth / RequireAdmin / público).
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
