# FEATURE — Landing

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `landing` |
| Título | Landing |
| Componente | `LandingPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`landing LandingPage /welcome   public`

## Rotas

- `/welcome`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/LandingPage.tsx`
- `docs/pages/landing/`

## Comportamento (bruto)

Página React `LandingPage`. Chama 0 endpoint(s) via `api()`.

## Notas de overview legado

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


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
