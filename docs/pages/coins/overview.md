# Coins — Overview

## Papel

Página **Coins** (`CoinsPage.tsx`).

- Auth gate: **public**
- Rotas: `/coins`
- Nota: Lista COINS (11, inclui ZER — só `t1` transparente — e PEPE BEP-20 na BNB)

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `public` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.
