# FEATURE — Admin Withdrawals

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `admin-withdrawals` |
| Título | Admin Withdrawals |
| Componente | `AdminWithdrawalsPage` |
| Auth | **admin** — RequireAdmin — role ADMIN ou sessão admin store |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`admin withdrawals AdminWithdrawalsPage /admin/withdrawals /admin/withdrawals /admin/withdrawals/:id/approve /admin/withdrawals/:id/reject Todos Aprovar Fila Transmitindo Transmitidos Confirmados Falhas Cancelados admin`

## Rotas

- `/admin/withdrawals`

## Abas / seções internas

- Todos
- Aprovar
- Fila
- Transmitindo
- Transmitidos
- Confirmados
- Falhas
- Cancelados

## APIs usadas (client → `/v1…`)

- `/admin/withdrawals` (prefixo `/v1` no servidor)
- `/admin/withdrawals/:id/approve` (prefixo `/v1` no servidor)
- `/admin/withdrawals/:id/reject` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/AdminWithdrawalsPage.tsx`
- `docs/pages/admin-withdrawals/`

## Comportamento (bruto)

Página React `AdminWithdrawalsPage`. Chama 3 endpoint(s) via `api()`. Abas/labels: Todos, Aprovar, Fila, Transmitindo, Transmitidos, Confirmados, Falhas, Cancelados. 2FA / fee / min withdrawal; status PENDING→BROADCAST→CONFIRMED. UI admin sempre pt-BR.

`POST /v1/admin/withdrawals/:id/approve`: se `SMTP_ENABLED=true` ou o admin tem 2FA, exige `emailCode` (OTP `LOGIN`). Sem código → `{ "codeSent": true }`. Reject sem step-up.

## Notas de overview legado

# Admin Withdrawals — Overview

## Papel

Página **Admin Withdrawals** (`AdminWithdrawalsPage.tsx`).

- Auth gate: **admin**
- Rotas: `/admin/withdrawals`
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
