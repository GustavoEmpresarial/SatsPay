# Documentation (in-app) — Overview

## Papel

Página **Documentation (in-app)** (`DocumentationPage.tsx`).

- Auth gate: **mixed**
- Rotas: `/documentation`
- Nota: Público + autenticado

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `mixed` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.
