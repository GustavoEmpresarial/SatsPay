# TC — Dashboard

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-dashboard-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-dashboard-02 | auth | Gate user: anônimo / usuário / admin conforme esperado | [ ] |
| TC-dashboard-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-dashboard-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-dashboard-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-dashboard-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-dashboard-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-dashboard-08 | structure | `client/tests/unit/pages/dashboard/` structure test se página SPA | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/dashboard/` |
| API HTTP (se admin/core) | `crates/api-http/tests/` |
| SQLx | `crates/db/tests/` |

## Dados / fixtures

- Preferir `client/tests/helpers/apiMock.ts` para unit.
- Integração: `DATABASE_URL` de teste + migrations.

## Critérios de aceite

- [ ] Rotas documentadas batem com `App.tsx`
- [ ] APIs documentadas batem com chamadas `api()` / handlers Axum
- [ ] Sem regressão de hooks (Rules of Hooks)
- [ ] Docs FEATURE.md + TC.md atualizados nesta pasta
