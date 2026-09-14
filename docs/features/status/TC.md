# TC — Status

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-status-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-status-02 | auth | Gate public: anônimo / usuário / admin conforme esperado | [ ] |
| TC-status-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-status-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-status-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-status-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-status-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-status-08 | structure | `client/tests/unit/pages/status/` structure test se página SPA | [ ] |
| TC-status-09 | ui-tab | Aba/seção «Operacional» carrega e exibe empty/loading/data | [ ] |
| TC-status-10 | ui-tab | Aba/seção «Degradado» carrega e exibe empty/loading/data | [ ] |
| TC-status-11 | ui-tab | Aba/seção «Fora do ar» carrega e exibe empty/loading/data | [ ] |
| TC-status-12 | ui-tab | Aba/seção «Verificando…» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/status/` |
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
