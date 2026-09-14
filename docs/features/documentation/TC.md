# TC — Documentation

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-documentation-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-documentation-02 | auth | Gate public: anônimo / usuário / admin conforme esperado | [ ] |
| TC-documentation-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-documentation-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-documentation-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-documentation-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-documentation-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-documentation-08 | structure | `client/tests/unit/pages/documentation/` structure test se página SPA | [ ] |
| TC-documentation-09 | ui-tab | Aba/seção «Visão Geral & Moedas» carrega e exibe empty/loading/data | [ ] |
| TC-documentation-10 | ui-tab | Aba/seção «Login com SatsPay (SSO)» carrega e exibe empty/loading/data | [ ] |
| TC-documentation-11 | ui-tab | Aba/seção «Comerciantes & APIs» carrega e exibe empty/loading/data | [ ] |
| TC-documentation-12 | ui-tab | Aba/seção «Ledger Contábil» carrega e exibe empty/loading/data | [ ] |
| TC-documentation-13 | ui-tab | Aba/seção «Faucet & Faucetlist» carrega e exibe empty/loading/data | [ ] |
| TC-documentation-14 | ui-tab | Aba/seção «Câmbio (Swap)» carrega e exibe empty/loading/data | [ ] |
| TC-documentation-15 | ui-tab | Aba/seção «Staking» carrega e exibe empty/loading/data | [ ] |
| TC-documentation-16 | ui-tab | Aba/seção «Mercado de Empréstimos» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/documentation/` |
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
