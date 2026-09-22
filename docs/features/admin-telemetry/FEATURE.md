# FEATURE — Admin Telemetry

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-telemetry` |
| Título | Admin Telemetry |
| Componente | `AdminTelemetryPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin telemetry AdminTelemetryPage /admin/telemetry /admin/stats /admin/telemetry/errors /admin/telemetry/errors/:id/ignore /admin/telemetry/errors/:id/resolve /admin/telemetry/errors/clear /admin/telemetry/errors/resolve-all /admin/telemetry/metrics-history /admin/telemetry/overview Saúde Desempenho Erros 5s 10s 30s Off Recentes Mais frequentes admin`

## Rotas

- `/admin/telemetry`

## Abas / seções internas

- Saúde
- Desempenho
- Erros
- 5s
- 10s
- 30s
- Off
- Recentes
- Mais frequentes

## APIs usadas (client → `/v1…`)

- `/admin/stats` (prefixo `/v1` no servidor)
- `/admin/telemetry/errors` (prefixo `/v1` no servidor)
- `/admin/telemetry/errors/:id/ignore` (prefixo `/v1` no servidor)
- `/admin/telemetry/errors/:id/resolve` (prefixo `/v1` no servidor)
- `/admin/telemetry/errors/clear` (prefixo `/v1` no servidor)
- `/admin/telemetry/errors/resolve-all` (prefixo `/v1` no servidor)
- `/admin/telemetry/metrics-history` (prefixo `/v1` no servidor)
- `/admin/telemetry/overview` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminTelemetryPage.tsx`
- `docs/pages/admin-telemetry/`

## Comportamento (bruto)

Página React `AdminTelemetryPage`. Chama 8 endpoint(s) via `api()`. Abas/labels: Saúde, Desempenho, Erros, 5s, 10s, 30s, Off, Recentes, Mais frequentes. Abas Saúde / Desempenho / Erros; autoatualização; resolve/ignore/clear. UI admin sempre pt-BR.

## Notas de overview legado

# Admin Telemetry — Overview

## Papel

Página **Admin Telemetry** (`AdminTelemetryPage.tsx`).

- Auth gate: **admin**
- Rotas: `/admin/telemetry`
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
