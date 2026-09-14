# TC — Api Keys

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-api-keys-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-api-keys-02 | auth | Gate user: anônimo / usuário / admin conforme esperado | [ ] |
| TC-api-keys-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-api-keys-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-api-keys-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-api-keys-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-api-keys-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-api-keys-08 | structure | `client/tests/unit/pages/api-keys/` structure test se página SPA | [ ] |
| TC-api-keys-09 | ui-tab | Aba/seção «Gateway de Depósitos (Invoicing)» carrega e exibe empty/loading/data | [ ] |
| TC-api-keys-10 | ui-tab | Aba/seção «Envios & Payouts (Saques)» carrega e exibe empty/loading/data | [ ] |
| TC-api-keys-11 | ui-tab | Aba/seção «Consulta de Saldos» carrega e exibe empty/loading/data | [ ] |
| TC-api-keys-12 | ui-tab | Aba/seção «Histórico & Extratos» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/api-keys/` |
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
