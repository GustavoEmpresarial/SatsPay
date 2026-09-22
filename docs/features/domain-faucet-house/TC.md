# TC — Faucet + inventário HOUSE

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-domain-faucet-house-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-domain-faucet-house-02 | auth | Gate mixed: anônimo / usuário / admin conforme esperado | [ ] |
| TC-domain-faucet-house-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-domain-faucet-house-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-domain-faucet-house-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-domain-faucet-house-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-domain-faucet-house-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-domain-faucet-house-08 | structure | Domínio backend (sem SPA) — N/A; cobrir via `faucet` / sqlx HOUSE | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | N/A (domínio backend; ver `docs/features/faucet/`) |
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
