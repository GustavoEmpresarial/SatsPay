# FEATURE — Referral

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `referral` |
| Título | Referral |
| Componente | `ReferralPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`referral ReferralPage /referrals /referral/commissions /referral/stats /referral/users  user`

## Rotas

- `/referrals`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/referral/commissions` (prefixo `/v1` no servidor)
- `/referral/stats` (prefixo `/v1` no servidor)
- `/referral/users` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/ReferralPage.tsx`

## Comportamento (bruto)

Página React `ReferralPage`. Chama 3 endpoint(s) via `api()`.

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
