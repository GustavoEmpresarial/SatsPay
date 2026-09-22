# TC — Admin Telemetry

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-admin-telemetry-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-admin-telemetry-02 | auth | Gate admin: anônimo / usuário / admin conforme esperado | [ ] |
| TC-admin-telemetry-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-admin-telemetry-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-admin-telemetry-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-admin-telemetry-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-admin-telemetry-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-admin-telemetry-08 | structure | `client/tests/unit/pages/admin-telemetry/` structure test se página SPA | [ ] |
| TC-admin-telemetry-09 | ui-tab | Aba/seção «Saúde» carrega e exibe empty/loading/data | [ ] |
| TC-admin-telemetry-10 | ui-tab | Aba/seção «Desempenho» carrega e exibe empty/loading/data | [ ] |
| TC-admin-telemetry-11 | ui-tab | Aba/seção «Erros» carrega e exibe empty/loading/data | [ ] |
| TC-admin-telemetry-12 | ui-tab | Aba/seção «5s» carrega e exibe empty/loading/data | [ ] |
| TC-admin-telemetry-13 | ui-tab | Aba/seção «10s» carrega e exibe empty/loading/data | [ ] |
| TC-admin-telemetry-14 | ui-tab | Aba/seção «30s» carrega e exibe empty/loading/data | [ ] |
| TC-admin-telemetry-15 | ui-tab | Aba/seção «Off» carrega e exibe empty/loading/data | [ ] |
| TC-admin-telemetry-16 | ui-tab | Aba/seção «Recentes» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/admin-telemetry/` |
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
