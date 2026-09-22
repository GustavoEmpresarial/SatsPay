# FEATURE — Admin Users

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-users` |
| Título | Admin Users |
| Componente | `AdminUsersPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin users AdminUsersPage /admin/users /admin/users  admin`

## Rotas

- `/admin/users`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/admin/users` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminUsersPage.tsx`

## Comportamento (bruto)

Página React `AdminUsersPage`. Chama 1 endpoint(s) via `api()`. UI admin sempre pt-BR.

Lista contas (`GET /v1/admin/users`) com filtros `role`, `q` (e-mail/username/UUID), `include_erased`, `limit`. Exclui HOUSE. E-mail revelado via `email_enc` para admin. Sem ações destrutivas nesta aba (só consulta).

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
