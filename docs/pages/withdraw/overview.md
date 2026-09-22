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
