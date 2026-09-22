# FEATURE — Admin Stake

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-stake` |
| Título | Admin Stake |
| Componente | `AdminStakePage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin stake AdminStakePage /admin/stake /admin/economics /admin/treasury-health /admin/treasury-wallets Depósitos user 24h Gateway pago 24h Saques 24h Claims de faucet 24h admin`

## Rotas

- `/admin/stake`

## Abas / seções internas

- Depósitos user 24h
- Gateway pago 24h
- Saques 24h
- Claims de faucet 24h

## APIs usadas (client → `/v1…`)

- `/admin/economics` (prefixo `/v1` no servidor)
- `/admin/treasury-health` (prefixo `/v1` no servidor)
- `/admin/treasury-wallets` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminStakePage.tsx`
- `docs/pages/admin-stake/`

## Comportamento (bruto)

Página React `AdminStakePage`. Chama 3 endpoint(s) via `api()`. Abas/labels: Depósitos user 24h, Gateway pago 24h, Saques 24h, Claims de faucet 24h. Tesouraria: hot vs custódia, economia, 9 painéis health, labels pt-BR (Resultado USD, Equilíbrio do saque, Autonomia HOUSE, Reserva da hot). UI admin sempre pt-BR.

## Notas de overview legado

# Admin Stake — Overview

## Papel

Página **Admin Stake** (`AdminStakePage.tsx`).

- Auth gate: **admin**
- Rotas: `/admin/stake`
- Nota: Nested under /admin

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `admin` (RequireAuth / RequireAdmin / público).
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
