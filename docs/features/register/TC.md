# TC — Register

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-register-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-register-02 | auth | Gate public: anônimo / usuário / admin conforme esperado | [ ] |
| TC-register-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-register-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-register-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-register-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-register-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-register-08 | structure | `client/tests/unit/pages/register/` structure test se página SPA | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/register/` |
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
