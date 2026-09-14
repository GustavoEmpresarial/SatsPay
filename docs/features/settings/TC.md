# TC — Settings

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-settings-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-settings-02 | auth | Gate user: anônimo / usuário / admin conforme esperado | [ ] |
| TC-settings-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-settings-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-settings-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-settings-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-settings-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-settings-08 | structure | `client/tests/unit/pages/settings/` structure test se página SPA | [ ] |
| TC-settings-09 | ui-tab | Aba/seção «Perfil & Conta» carrega e exibe empty/loading/data | [ ] |
| TC-settings-10 | ui-tab | Aba/seção «Segurança & 2FA» carrega e exibe empty/loading/data | [ ] |
| TC-settings-11 | ui-tab | Aba/seção «Sessões & Atividades» carrega e exibe empty/loading/data | [ ] |
| TC-settings-12 | ui-tab | Aba/seção «Aplicações Conectadas» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/settings/` |
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
