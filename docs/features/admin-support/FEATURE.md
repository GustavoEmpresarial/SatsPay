# FEATURE — Admin Support

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-support` |
| Título | Admin Support |
| Componente | `AdminSupportPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin support AdminSupportPage /admin/support /admin/support/tickets/:id /admin/support/tickets/:id/messages /admin/support/tickets/:id/status /admin/support/tickets:id  admin`

## Rotas

- `/admin/support`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/admin/support/tickets/:id` (prefixo `/v1` no servidor)
- `/admin/support/tickets/:id/messages` (prefixo `/v1` no servidor)
- `/admin/support/tickets/:id/status` (prefixo `/v1` no servidor)
- `/admin/support/tickets:id` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminSupportPage.tsx`

## Comportamento (bruto)

Página React `AdminSupportPage`. Chama 4 endpoint(s) via `api()`. UI admin sempre pt-BR.

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
