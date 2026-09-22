# FEATURE — Privacy

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `privacy` |
| Título | Privacy |
| Componente | `PrivacyPage` |
| Auth | **public** — rota pública |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`privacy PrivacyPage /privacy   public`

## Rotas

- `/privacy`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `client/src/pages/PrivacyPage.tsx`
- `docs/pages/privacy/`

## Comportamento (bruto)

Página React `PrivacyPage`. Chama 0 endpoint(s) via `api()`. Política LGPD (bases art. 7, direitos art. 18, retenção art. 16). Export `GET /v1/me/export` e erase `POST /v1/me/erase` (confirmEmail + APAGAR) na aba Configurações. Ledger não é apagado.

PII em AES-GCM (`enc:v1:`) com AAD por linha: fatura, e-mail (`email_hmac`+`email_enc`), perfil merchant, tickets, `to_address` de saque, label de API key. IP vira HMAC (`ip_fingerprint`). Boot roda `backfill_pii` (idempotente). `PII_BLANK_EMAIL=true` apaga o texto de `users.email` (HOUSE fica). Worker apaga OTP/captcha >7d, refresh >30d, telemetria >90d. Sem isso, dump do Postgres ainda vaza o que não foi selado.

**Username fica em claro.** É handle público (`@user` na UI). Não cifrar. E-mail não volta para `audit_logs.metadata`.

Doc gerado à mão — `scripts/generate_feature_docs.py` **não** sobrescreve este FEATURE.

## Notas de overview legado

# Privacy — Overview

## Papel

Página **Privacy** (`PrivacyPage.tsx`).

- Auth gate: **public**
- Rotas: `/privacy`
- Nota: Legal

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


## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
