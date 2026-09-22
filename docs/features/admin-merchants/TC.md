# TC — Admin Merchants

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-admin-merchants-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-admin-merchants-02 | auth | Gate admin: anônimo / usuário / admin conforme esperado | [ ] |
| TC-admin-merchants-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-admin-merchants-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-admin-merchants-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-admin-merchants-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-admin-merchants-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-admin-merchants-08 | structure | `client/tests/unit/pages/admin-merchants/` structure test se página SPA | [ ] |
| TC-admin-merchants-09 | ui-tab | Aba/seção «Visão geral» carrega e exibe empty/loading/data | [ ] |
| TC-admin-merchants-10 | ui-tab | Aba/seção «Estatísticas» carrega e exibe empty/loading/data | [ ] |
| TC-admin-merchants-11 | ui-tab | Aba/seção «Comerciantes» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/admin-merchants/` |
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
