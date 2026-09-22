# TC — Api Docs

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-api-docs-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-api-docs-02 | auth | Gate public: anônimo / usuário / admin conforme esperado | [ ] |
| TC-api-docs-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-api-docs-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-api-docs-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-api-docs-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-api-docs-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-api-docs-08 | structure | `client/tests/unit/pages/api-docs/` structure test se página SPA | [ ] |
| TC-api-docs-09 | ui-tab | Aba/seção «loja-producao» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/api-docs/` |
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
