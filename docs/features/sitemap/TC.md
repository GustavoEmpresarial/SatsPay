# TC — Sitemap

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-sitemap-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-sitemap-02 | auth | Gate public: anônimo / usuário / admin conforme esperado | [ ] |
| TC-sitemap-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-sitemap-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-sitemap-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-sitemap-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-sitemap-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-sitemap-08 | structure | `client/tests/unit/pages/sitemap/` structure test se página SPA | [ ] |
| TC-sitemap-09 | ui-tab | Aba/seção «Home / Dashboard» carrega e exibe empty/loading/data | [ ] |
| TC-sitemap-10 | ui-tab | Aba/seção «Welcome / Landing» carrega e exibe empty/loading/data | [ ] |
| TC-sitemap-11 | ui-tab | Aba/seção «Sign In» carrega e exibe empty/loading/data | [ ] |
| TC-sitemap-12 | ui-tab | Aba/seção «Sign Up» carrega e exibe empty/loading/data | [ ] |
| TC-sitemap-13 | ui-tab | Aba/seção «Wallets» carrega e exibe empty/loading/data | [ ] |
| TC-sitemap-14 | ui-tab | Aba/seção «Faucet» carrega e exibe empty/loading/data | [ ] |
| TC-sitemap-15 | ui-tab | Aba/seção «Faucet Directory» carrega e exibe empty/loading/data | [ ] |
| TC-sitemap-16 | ui-tab | Aba/seção «Swap» carrega e exibe empty/loading/data | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/sitemap/` |
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
