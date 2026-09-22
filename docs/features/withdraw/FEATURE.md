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

`withdraw WithdrawPage /withdraw /wallet /wallet/transfer /withdrawals /withdrawals/addresses /withdrawals/addresses/:id /withdrawals/history  user`

## Rotas

- `/withdraw`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/wallet` (prefixo `/v1` no servidor)
- `/wallet/transfer` (prefixo `/v1` no servidor)
- `/withdrawals` (prefixo `/v1` no servidor)
- `/withdrawals/addresses` (prefixo `/v1` no servidor)
- `/withdrawals/addresses/:id` (prefixo `/v1` no servidor)
- `/withdrawals/history` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/WithdrawPage.tsx`
- `docs/pages/withdraw/`

## Comportamento (bruto)

Página React `WithdrawPage`. Chama 6 endpoint(s) via `api()`. 2FA / fee / min withdrawal; status PENDING→BROADCAST→CONFIRMED.

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

## Carteira de origem

O saque on-chain debita **sempre a carteira `PERSONAL`**. O caixa do comerciante
(`MERCHANT`, onde caem os créditos líquidos de invoice) aparece na página só como
aviso informativo, com um botão que chama `POST /v1/wallet/transfer`
(`toDeveloper: false`) para mover o saldo para a pessoal.

Motivo: envio em blockchain é irreversível, e o caixa é capital de giro do
negócio — gastá-lo tem que ser um passo deliberado e separado. A página já
selecionou `MERCHANT` sozinha quando a pessoal estava zerada, o que fez usuário
mandar dinheiro do negócio para fora sem perceber.

A trava é no servidor: `walletKind: "MERCHANT"` devolve `403`
`WITHDRAWAL_MERCHANT_BLOCKED` sem debitar nada (um SPA em cache ainda manda esse
campo). O bloqueio roda antes do step-up de OTP, então tentativa bloqueada não
dispara e-mail de código, e grava `WITHDRAWAL_MERCHANT_BLOCKED` em `audit_logs`.

## Segurança

- Respeitar gate `user` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.
- Nunca reintroduzir seleção automática de carteira de origem — origem de
  dinheiro é escolha explícita do usuário.


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
