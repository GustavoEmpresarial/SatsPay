# FEATURE — Admin Faucet Sites

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-faucet-sites` |
| Título | Admin Faucet Sites |
| Componente | `AdminFaucetSitesPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin faucet sites AdminFaucetSitesPage /admin/faucet-sites /admin/faucetlist /admin/faucetlist/:id/approve /admin/faucetlist/:id/reject /admin/faucetlist/:id/suspend  admin`

## Rotas

- `/admin/faucet-sites`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/admin/faucetlist` (prefixo `/v1` no servidor)
- `/admin/faucetlist/:id/approve` (prefixo `/v1` no servidor)
- `/admin/faucetlist/:id/reject` (prefixo `/v1` no servidor)
- `/admin/faucetlist/:id/suspend` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminFaucetSitesPage.tsx`
- `docs/pages/admin-faucet-sites/`

## Comportamento (bruto)

Página React `AdminFaucetSitesPage`. Chama 4 endpoint(s) via `api()`. UI admin sempre pt-BR.

## Notas de overview legado

# Admin Faucet Sites — Overview

## Papel

Página **Admin Faucet Sites** (`AdminFaucetSitesPage.tsx`).

- Auth gate: **admin**
- Rotas: `/admin/faucet-sites`
- Nota: Nested under /admin

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
