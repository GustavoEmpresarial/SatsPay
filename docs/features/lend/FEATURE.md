# FEATURE — Lend

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `lend` |
| Título | Lend |
| Componente | `LendPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`lend LendPage /lend /lend/action /lend/markets /lend/positions /wallet  user`

## Rotas

- `/lend`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/lend/action` (prefixo `/v1` no servidor)
- `/lend/markets` (prefixo `/v1` no servidor)
- `/lend/positions` (prefixo `/v1` no servidor)
- `/wallet` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/LendPage.tsx`
- `docs/pages/lend/`

## Comportamento (bruto)

Página React `LendPage`. Chama 4 endpoint(s) via `api()`.

## Notas de overview legado

# Lend — Overview

## Papel

Página **Lend** (`LendPage.tsx`).

- Auth gate: **user**
- Rotas: `/lend`


## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `user` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
