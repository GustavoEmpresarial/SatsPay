# FEATURE — Telemetria / erros cliente + servidor

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `domain-observability` |
| Título | Telemetria / erros cliente + servidor |
| Componente | `—` |
| Auth | **mixed** — pública ou autenticada conforme contexto |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`system_error_logs telemetry reportClientError APM client-frontend FEE_MARGIN`

## Rotas

- (sem rota SPA — domínio backend)

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `POST /v1/telemetry/client-errors` (prefixo `/v1` no servidor)
- `GET /v1/admin/telemetry/overview` (prefixo `/v1` no servidor)
- `GET /v1/admin/telemetry/errors` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/lib/reportError.ts`
- `client/src/pages/AdminTelemetryPage.tsx`
- `docs/quality/error-observability.md`
- `crates/db/migrations/0014_telemetry_and_error_logs.sql`

## Comportamento (bruto)

Filtros de ruído: inventory faucet, login 400, React #311 legado, CDN icons.
Fingerprint fino + rate-limit `telemetry-ingest` (60/min).
Alerta opcional: `TELEMETRY_ALERT_WEBHOOK_URL` (CRITICAL/FATAL novos + spikes).

## Notas de overview legado

_sem overview em docs/pages_

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.
- Endpoint de ingest é público (auth opcional) — depende de rate-limit + filtros cliente.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
