# TC — Integração on-chain

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-domain-chain-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-domain-chain-02 | auth | Gate mixed: anônimo / usuário / admin conforme esperado | [ ] |
| TC-domain-chain-03 | api | Happy path das APIs listadas retorna 2xx com payload válido | [ ] |
| TC-domain-chain-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError | [ ] |
| TC-domain-chain-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-domain-chain-06 | obs | Erros inesperados reportados; ruído esperado filtrado | [ ] |
| TC-domain-chain-07 | security | Sem IDOR; sem vazar secrets em UI/logs | [ ] |
| TC-domain-chain-08 | structure | `client/tests/unit/pages/domain-chain/` structure test se página SPA | [ ] |

## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/domain-chain/` |
| API HTTP (se admin/core) | `crates/api-http/tests/` |
| SQLx | `crates/db/tests/` |

## Dados / fixtures

- Preferir `client/tests/helpers/apiMock.ts` para unit.
- Integração: `DATABASE_URL` de teste + migrations.

## Critérios de aceite

- [ ] DOGE e DGB saltam provedores após três falhas, retomam após 60 s e não tratam HTML 200 como sucesso.
- [ ] Blockbook DOGE credenciado detecta saída destinada ao endereço correto, sem repassar chave em URL/log.
- [ ] ZER recusa RPC público de assinatura; `zerod` local assina enquanto leitura e broadcast usam provedor público.
- [ ] ZeroChain `rawtx` recebe apenas hex já assinado; rejeição HTTP não vaza a chave de API e resposta sem txid não é sucesso.
- [ ] Histórico ZeroChain usa `vout[].valueSat` e endereço da saída para depósitos; a busca de UTXOs percorre as páginas e exclui saídas gastas. Saldo sem campo válido falha em vez de somar depósitos históricos.
- [ ] Falha de detecção não impede a tentativa de sweep, mas gera alerta persistente após três ciclos.

- [ ] Rotas documentadas batem com `App.tsx`
- [ ] APIs documentadas batem com chamadas `api()` / handlers Axum
- [ ] Sem regressão de hooks (Rules of Hooks)
- [ ] Docs FEATURE.md + TC.md atualizados nesta pasta
