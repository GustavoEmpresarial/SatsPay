# Landing — Overview

## Comportamento

1. Visitante abre `/` ou `/welcome`.
2. Se `useAuthStore.user` existir → redirect imediato para `/dashboard`.
3. Caso contrário renderiza marketing: nav, hero, features, coins, FAQ, footer, cookie consent.
4. CTAs principais: **Entrar** → `/login`, **Começar grátis** → `/register`.

## Estrutura visual (ordem)

| Bloco | Conteúdo |
|-------|----------|
| Header sticky | Logo SatsPay, `ThemeToggle`, `LanguageSwitch`, Sign In, Get Started |
| Hero | Badge, título i18n (`landing.hero.*`), CTAs, painel visual |
| Features | 6 itens (`landing.features.*`) |
| Coins | Lista `COINS` + `COIN_CONFIG` + logos |
| FAQ | Array i18n `landing.faq.items` `{ q, a }` |
| Footer | `SiteFooter` |
| Cookies | `CookieConsent` |

## i18n

Chaves sob `landing.*` e `common.signIn` / `common.getStartedFree` em `pt.json` / `en.json`.
