# TC — Admin Faucet Sites

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-admin-faucet-sites-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-admin-faucet-sites-02 | auth | Gate admin: anônimo / usuário / admin conforme esperado | [ ] |
| TC-admin-faucet-sites-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-admin-faucet-sites-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-admin-faucet-sites-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-admin-faucet-sites-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-admin-faucet-sites-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-admin-faucet-sites-08 | structure | `client/tests/unit/pages/admin-faucet-sites/` structure test se página SPA | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/admin-faucet-sites/` |
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
