# TC — Admin Withdrawals

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-admin-withdrawals-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-admin-withdrawals-02 | auth | Gate admin: anônimo / usuário / admin conforme esperado | [ ] |
| TC-admin-withdrawals-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-admin-withdrawals-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-admin-withdrawals-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-admin-withdrawals-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-admin-withdrawals-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-admin-withdrawals-08 | structure | `client/tests/unit/pages/admin-withdrawals/` structure test se página SPA | [ ] |
| TC-admin-withdrawals-09 | ui-tab | Aba/seção «Todos» carrega e exibe empty/loading/data | [ ] |
| TC-admin-withdrawals-10 | ui-tab | Aba/seção «Aprovar» carrega e exibe empty/loading/data | [ ] |
| TC-admin-withdrawals-11 | ui-tab | Aba/seção «Fila» carrega e exibe empty/loading/data | [ ] |
| TC-admin-withdrawals-12 | ui-tab | Aba/seção «Transmitindo» carrega e exibe empty/loading/data | [ ] |
| TC-admin-withdrawals-13 | ui-tab | Aba/seção «Transmitidos» carrega e exibe empty/loading/data | [ ] |
| TC-admin-withdrawals-14 | ui-tab | Aba/seção «Confirmados» carrega e exibe empty/loading/data | [ ] |
| TC-admin-withdrawals-15 | ui-tab | Aba/seção «Falhas» carrega e exibe empty/loading/data | [ ] |
| TC-admin-withdrawals-16 | ui-tab | Aba/seção «Cancelados» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/admin-withdrawals/` |
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
