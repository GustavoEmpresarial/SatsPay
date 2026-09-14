# FEATURE — OAuth Apps

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `o-auth-apps` |
| Título | OAuth Apps |
| Componente | `OAuthAppsPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`o auth apps OAuthAppsPage /developer/apps /developer/oauth /oauth/apps /oauth/apps /oauth/apps/:id /oauth/apps/:id/rotate-secret Clean Light Dark Obsidian Bitcoin Gold Entrar com… Continuar com… Sign in with… Redirect (recomendado) Popup HTML / SDK Node.js Python cURL user`

## Rotas

- `/developer/apps`
- `/developer/oauth`
- `/oauth/apps`

## Abas / seções internas

- Clean Light
- Dark Obsidian
- Bitcoin Gold
- Entrar com…
- Continuar com…
- Sign in with…
- Redirect (recomendado)
- Popup
- HTML / SDK
- Node.js
- Python
- cURL

## APIs usadas (client → `/v1…`)

- `/oauth/apps` (prefixo `/v1` no servidor)
- `/oauth/apps/:id` (prefixo `/v1` no servidor)
- `/oauth/apps/:id/rotate-secret` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/OAuthAppsPage.tsx`

## Comportamento (bruto)

Página React `OAuthAppsPage`. Chama 3 endpoint(s) via `api()`. Abas/labels: Clean Light, Dark Obsidian, Bitcoin Gold, Entrar com…, Continuar com…, Sign in with…, Redirect (recomendado), Popup, HTML / SDK, Node.js, Python, cURL.

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
