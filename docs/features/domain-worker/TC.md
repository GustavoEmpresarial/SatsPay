# TC — Worker / jobs em background

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-domain-worker-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-domain-worker-02 | auth | Gate mixed: anônimo / usuário / admin conforme esperado | [ ] |
| TC-domain-worker-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-domain-worker-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-domain-worker-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-domain-worker-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-domain-worker-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-domain-worker-08 | structure | `client/tests/unit/pages/domain-worker/` structure test se página SPA | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/domain-worker/` |
| API HTTP (se admin/core) | `crates/api-http/tests/` |
| SQLx | `crates/db/tests/` |

## Dados / fixtures

- Preferir `client/tests/helpers/apiMock.ts` para unit.
- Integração: `DATABASE_URL` de teste + migrations.

## Critérios de aceite

- [ ] Fila de saque atrasada por cinco minutos ou `FAILED` alerta e não duplica processamento.
- [ ] Pool SOL vazio e depósito creditado sem lançamento igual no ledger alertam sem alterar saldo.
- [ ] Três sweeps falhos consecutivos alertam; sucesso posterior resolve o incidente.

- [ ] Rotas documentadas batem com `App.tsx`
- [ ] APIs documentadas batem com chamadas `api()` / handlers Axum
- [ ] Sem regressão de hooks (Rules of Hooks)
- [ ] Docs FEATURE.md + TC.md atualizados nesta pasta
