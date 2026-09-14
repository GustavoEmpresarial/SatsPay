# TC — OAuth Apps

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-o-auth-apps-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-o-auth-apps-02 | auth | Gate user: anônimo / usuário / admin conforme esperado | [ ] |
| TC-o-auth-apps-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-o-auth-apps-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-o-auth-apps-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-o-auth-apps-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-o-auth-apps-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-o-auth-apps-08 | structure | `client/tests/unit/pages/o-auth-apps/` structure test se página SPA | [ ] |
| TC-o-auth-apps-09 | ui-tab | Aba/seção «Clean Light» carrega e exibe empty/loading/data | [ ] |
| TC-o-auth-apps-10 | ui-tab | Aba/seção «Dark Obsidian» carrega e exibe empty/loading/data | [ ] |
| TC-o-auth-apps-11 | ui-tab | Aba/seção «Bitcoin Gold» carrega e exibe empty/loading/data | [ ] |
| TC-o-auth-apps-12 | ui-tab | Aba/seção «Entrar com…» carrega e exibe empty/loading/data | [ ] |
| TC-o-auth-apps-13 | ui-tab | Aba/seção «Continuar com…» carrega e exibe empty/loading/data | [ ] |
| TC-o-auth-apps-14 | ui-tab | Aba/seção «Sign in with…» carrega e exibe empty/loading/data | [ ] |
| TC-o-auth-apps-15 | ui-tab | Aba/seção «Redirect (recomendado)» carrega e exibe empty/loading/data | [ ] |
| TC-o-auth-apps-16 | ui-tab | Aba/seção «Popup» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/o-auth-apps/` |
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
