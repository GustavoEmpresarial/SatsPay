# FEATURE — Withdraw

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `withdraw` |
| Título | Withdraw |
| Componente | `WithdrawPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`withdraw WithdrawPage /withdraw /wallet /withdrawals /withdrawals/history  user`

## Rotas

- `/withdraw`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/wallet` (prefixo `/v1` no servidor)
- `/withdrawals` (prefixo `/v1` no servidor)
- `/withdrawals/history` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/WithdrawPage.tsx`
- `docs/pages/withdraw/`

## Comportamento (bruto)

Página React `WithdrawPage`. Chama 3 endpoint(s) via `api()`. 2FA / fee / min withdrawal; status PENDING→BROADCAST→CONFIRMED.

OTP de saque: se `SMTP_ENABLED=true` **ou** `two_factor_enabled`, `POST /v1/withdrawals` exige código e-mail (`emailCode`). Sem código → `{ "codeSent": true }` e envio OTP `WITHDRAWAL`. Sem SMTP e sem 2FA, o saque segue sem step-up.

**Pausa temporária:** `BTC` / `LTC` / `DOGE` / `DGB` — `POST /v1/withdrawals` → `503` `WITHDRAWAL_PAUSED` (mesma lista `DEPOSIT_WITHDRAW_PAUSED_COINS`).

## Notas de overview legado

# Withdraw — Overview

## Papel

Página **Withdraw** (`WithdrawPage.tsx`).

- Auth gate: **user**
- Rotas: `/withdraw`
- Nota: Idempotency

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
