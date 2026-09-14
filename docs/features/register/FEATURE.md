# FEATURE — Register

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `register` |
| Título | Register |
| Componente | `RegisterPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`register RegisterPage /register /auth/register  public`

## Rotas

- `/register`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `/auth/register` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/RegisterPage.tsx`
- `docs/pages/register/`

## Comportamento (bruto)

Página React `RegisterPage`. Chama 1 endpoint(s) via `api()`.

## Notas de overview legado

# Register — Overview

Validação client: username 3–24, senha forte, confirm, terms, Turnstile `register`, referral opcional.
Sucesso → `setSession` → `returnTo`.


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
