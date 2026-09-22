# FEATURE — Auth, sessão, 2FA, admin login

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `domain-auth` |
| Título | Auth, sessão, 2FA, admin login |
| Componente | `—` |
| Auth | **mixed** — pública ou autenticada conforme contexto |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`JWT refresh cookie login register 2FA admin RequireAuth RequireAdmin Turnstile`

## Rotas

- (sem rota SPA — domínio backend)

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `POST /v1/auth/login` (prefixo `/v1` no servidor)
- `POST /v1/auth/register` (prefixo `/v1` no servidor)
- `GET /v1/auth/me` (prefixo `/v1` no servidor)
- `POST /v1/auth/admin/login` (prefixo `/v1` no servidor)

## Arquivos-chave

- `crates/api-http/src/auth.rs`
- `client/src/lib/api.ts`
- `client/src/stores/auth.ts`
- `client/src/stores/admin.ts`

## Comportamento (bruto)

Access token em memória; refresh HttpOnly. RequireAdmin: NUNCA short-circuit useStore (React #311).

Login/register: Turnstile + rate-limit 10/5min (Postgres compartilhado). E-mail indexado por HMAC; `PII_BLANK_EMAIL` troca `users.email` por placeholder. `AuthUser` relê `role` e exige `erased_at IS NULL` a cada request (sem denylist de access JWT; TTL ~900s).

`SMTP_ENABLED=false`: `/admin/login` pede OTP que ninguém recebe. **Admin operacional** = login de usuário + `ADMIN_EMAILS`. Não fingir 2FA por e-mail até SMTP real.

Audit metadata de auth **não** grava e-mail (`user_id` basta).

Doc gerado à mão — `scripts/generate_feature_docs.py` **não** sobrescreve este FEATURE.

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
