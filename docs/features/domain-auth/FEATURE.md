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

## 2FA e sessões (HTTP)

- `POST /v1/auth/2fa/request {purpose: ENABLE_2FA|DISABLE_2FA}` → `{codeSent}`; `POST /v1/auth/2fa/enable|disable {code}` → `{twoFactorEnabled}`.
  Antes dessas rotas a aba Segurança do Settings sempre recebia 404 (o domínio tinha `enable_2fa`/`disable_2fa`, mas nada no HTTP).
- `409 TWO_FACTOR_ALREADY_ENABLED` / `TWO_FACTOR_NOT_ENABLED`; `401 INVALID_2FA`; `429 RATE_LIMITED`. Auditoria `AUTH_2FA_*`.
- `POST /v1/auth/sessions/revoke-others` revoga todos os refresh tokens e reemite o do chamador (CSRF gate). O botão
  "Desconectar outros aparelhos" era falso (só esperava 600 ms e mostrava sucesso).
- `ClientIp` ausente → `400 CLIENT_IP_UNAVAILABLE` (era 500).
- Detecção de reuso é agressiva: depois do revoke-others, se o aparelho antigo tentar `/auth/refresh` com o token já revogado, isso é tratado como roubo e revoga **todas** as sessões (inclusive a atual). Produção suaviza com `refresh_reuse_grace_secs > 0`; nos testes é 0.

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
