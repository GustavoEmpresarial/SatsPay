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

## Carteira de origem — página é PERSONAL-only

O saque on-chain debita **sempre a carteira `PERSONAL`**, e esta página **não
toca a infraestrutura de comerciante em nada**: não consulta `/wallet?kind=MERCHANT`,
não exibe saldo de caixa e não oferece caminho para movê-lo. O caixa (`MERCHANT`,
onde caem os créditos líquidos de invoice) é gerenciado exclusivamente no painel
de Comerciante, que já tem a transferência para a pessoal.

Motivo: envio em blockchain é irreversível, e o caixa é capital de giro do
negócio. A página já selecionou `MERCHANT` sozinha quando a pessoal estava
zerada, o que fez usuário mandar dinheiro do negócio para fora sem perceber.
Depois disso o caixa ainda apareceu aqui como aviso com botão de transferência —
também removido: lado pessoal e lado comerciante ficam separados, sem ponte.

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
