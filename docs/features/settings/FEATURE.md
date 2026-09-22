# FEATURE — Settings

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `settings` |
| Título | Settings |
| Componente | `SettingsPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`settings SettingsPage /settings /auth/2fa/request /auth/me /auth/security-logs /auth/username /me/erase /oauth/authorized-apps /oauth/authorized-apps/:id Perfil & Conta Segurança & 2FA Sessões & Atividades Aplicações Conectadas user`

## Rotas

- `/settings`

## Abas / seções internas

- Perfil & Conta
- Segurança & 2FA
- Sessões & Atividades
- Aplicações Conectadas

## APIs usadas (client → `/v1…`)

- `/auth/2fa/request` (prefixo `/v1` no servidor)
- `/auth/me` (prefixo `/v1` no servidor)
- `/auth/security-logs` (prefixo `/v1` no servidor)
- `/auth/username` (prefixo `/v1` no servidor)
- `/me/erase` (prefixo `/v1` no servidor)
- `/oauth/authorized-apps` (prefixo `/v1` no servidor)
- `/oauth/authorized-apps/:id` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/SettingsPage.tsx`
- `docs/pages/settings/`

## Comportamento (bruto)

Página React `SettingsPage`. Chama 7 endpoint(s) via `api()`. Abas/labels: Perfil & Conta, Segurança & 2FA, Sessões & Atividades, Aplicações Conectadas.

## Notas de overview legado

# Settings — Overview

## Papel

Página **Settings** (`SettingsPage.tsx`).

- Auth gate: **user**
- Rotas: `/settings`


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
