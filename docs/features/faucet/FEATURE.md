# FEATURE — Faucet

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `faucet` |
| Título | Faucet |
| Componente | `FaucetPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`faucet FaucetPage /faucet /faucet/claim/:id  user`

## Rotas

- `/faucet`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/faucet/claim/:id` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/FaucetPage.tsx`
- `docs/pages/faucet/`

## Comportamento (bruto)

Página React `FaucetPage`. Chama 1 endpoint(s) via `api()`. Turnstile action `faucet_claim`; inventário HOUSE; cooldown.

## Notas de overview legado

# Faucet — Overview

## Papel

Página **Faucet** (`FaucetPage.tsx`).

- Auth gate: **user**
- Rotas: `/faucet`
- Nota: Cooldown 11h + Turnstile

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
