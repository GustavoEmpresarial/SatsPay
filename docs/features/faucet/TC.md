# TC — Faucet

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-faucet-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-faucet-02 | auth | Gate user: anônimo / usuário / admin conforme esperado | [ ] |
| TC-faucet-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-faucet-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-faucet-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-faucet-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-faucet-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-faucet-08 | structure | `client/tests/unit/pages/faucet/` structure test se página SPA | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/faucet/` |
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
