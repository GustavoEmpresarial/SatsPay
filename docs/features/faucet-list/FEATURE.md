# FEATURE — Faucet List

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `faucet-list` |
| Título | Faucet List |
| Componente | `FaucetListPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`faucet list FaucetListPage /faucetlist /faucetlist  user`

## Rotas

- `/faucetlist`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/faucetlist` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/FaucetListPage.tsx`

## Comportamento (bruto)

Página React `FaucetListPage`. Chama 1 endpoint(s) via `api()`.

## Notas de overview legado

_sem overview em docs/pages_

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
