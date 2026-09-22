# FEATURE — Admin Merchants

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-merchants` |
| Título | Admin Merchants |
| Componente | `AdminMerchantsPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin merchants AdminMerchantsPage /admin/merchants /admin/economics /admin/merchants /admin/merchants/:id/approve /admin/merchants/:id/suspend /admin/merchants/stats Visão geral Estatísticas Comerciantes admin`

## Rotas

- `/admin/merchants`

## Abas / seções internas

- Visão geral
- Estatísticas
- Comerciantes

## APIs usadas (client → `/v1…`)

- `/admin/economics` (prefixo `/v1` no servidor)
- `/admin/merchants` (prefixo `/v1` no servidor)
- `/admin/merchants/:id/approve` (prefixo `/v1` no servidor)
- `/admin/merchants/:id/suspend` (prefixo `/v1` no servidor)
- `/admin/merchants/stats` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminMerchantsPage.tsx`
- `docs/pages/admin-merchants/`

## Comportamento (bruto)

Página React `AdminMerchantsPage`. Chama 5 endpoint(s) via `api()`. Abas/labels: Visão geral, Estatísticas, Comerciantes. 3 abas: Visão geral · Estatísticas (funil, série 14d, volume, top, faturas recentes) · Comerciantes (moderação). UI admin sempre pt-BR.

## Notas de overview legado

# Admin Merchants — Overview

## Papel

Página **Admin Merchants** (`AdminMerchantsPage.tsx`).

- Auth: **admin** (`RequireAdmin`)
- Rota: `/admin/merchants`
- UI: pt-BR

## Abas

| Aba | Função |
|-----|--------|
| Visão geral | Economia gateway/faucet + KPIs contas + CTA estatísticas |
| Estatísticas | Funil, série 14d, volumes, top 10, faturas recentes, períodos |
| Comerciantes | Lista + aprovar/suspender |

Doc densa AI: [`../../features/admin-merchants/FEATURE.md`](../../features/admin-merchants/FEATURE.md)

## APIs

Ver `routes-and-api.md` e FEATURE.md (inclui `/admin/merchants/stats` expandido).


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
