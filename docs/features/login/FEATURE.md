# FEATURE — Login

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `login` |
| Título | Login |
| Componente | `LoginPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`login LoginPage /login /auth/login  public`

## Rotas

- `/login`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/auth/login` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/LoginPage.tsx`
- `docs/pages/login/`

## Comportamento (bruto)

Página React `LoginPage`. Chama 1 endpoint(s) via `api()`.

## Notas de overview legado

# Login — Overview

Fluxo: email+senha(+captcha) → `POST /v1/auth/login` → OTP (`codeSent`) ou `setSession` → `navigate(returnTo)`.

Segurança client: `resolveReturnTo`, Turnstile action `login`, `reportAuthFailure` sem senha, tokens não no localStorage.


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
