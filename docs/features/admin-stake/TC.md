# TC — Admin Stake

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-admin-stake-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-admin-stake-02 | auth | Gate admin: anônimo / usuário / admin conforme esperado | [ ] |
| TC-admin-stake-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-admin-stake-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-admin-stake-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-admin-stake-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-admin-stake-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-admin-stake-08 | structure | `client/tests/unit/pages/admin-stake/` structure test se página SPA | [ ] |
| TC-admin-stake-09 | ui-tab | Aba/seção «Depósitos user 24h» carrega e exibe empty/loading/data | [ ] |
| TC-admin-stake-10 | ui-tab | Aba/seção «Gateway pago 24h» carrega e exibe empty/loading/data | [ ] |
| TC-admin-stake-11 | ui-tab | Aba/seção «Saques 24h» carrega e exibe empty/loading/data | [ ] |
| TC-admin-stake-12 | ui-tab | Aba/seção «Claims de faucet 24h» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/admin-stake/` |
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
