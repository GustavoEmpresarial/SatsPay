# Stake — Checklist de testes

## Automatizado (lote coverage)

| Tipo | Local |
|------|-------|
| Unit — estrutura | `client/tests/unit/pages/stake/stake.structure.test.ts` |

## Planejado (aprofundar cobertura)

| Tipo | Caso |
|------|------|
| Unit | Regras de negócio / validação locais |
| API / smoke | Happy path + negativos |
| E2E | Fluxo crítico na UI |
| Security | Authz, IDOR, input validation |
| Observability | Erros reportados sem vazamento de segredo |

## Aceite

- [ ] Rota(s) registradas em `App.tsx`
- [ ] Página existe e exporta componente
- [ ] Docs overview + API atualizados
- [ ] Teste de estrutura passando
