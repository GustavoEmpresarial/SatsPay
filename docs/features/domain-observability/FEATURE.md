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
- `crates/api-http/src/access_log.rs`
- `crates/api-http/src/http_error.rs`
- `client/nginx.conf`

## Comportamento (bruto)

Filtros de ruído: inventory faucet, login 400, React #311 legado, CDN icons. Log de acesso: 1 linha JSON por request (`http_access`: rota template, status, duration_ms, request_id), sem query/corpo/IP. `X-Request-Id` em toda resposta; 500 devolve o mesmo `requestId`. nginx: log JSON com `$uri` (sem query) e `X-Request-Id: $request_id` para a API. RUST_LOG padrão = info.

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

## Operação (2026-09-23)

- `/metrics` mantém histogramas HTTP e acrescenta `satspay_http_requests_total{module,version,status_class}`, `satspay_http_5xx_last_minute{module,version}`, `satspay_sol_deposit_pool_available` e `satspay_withdrawal_jobs_stalled`. `APP_VERSION` deve ser o SHA do deploy. A rota deve ficar acessível só pela rede interna/loopback.
- O worker lê eventos 5xx recentes e verifica a fila de saques, o pool SOL, depósitos `CREDITED` sem crédito correspondente e saques sem débito correspondente no ledger. Cinco ou mais 5xx/min disparam alerta. Divergência e pool vazio disparam imediatamente; fila `RUNNING`/`PENDING` atrasada por cinco minutos ou `FAILED` também.
- Alertas `CRITICAL` são agrupados por fingerprint no banco e enviados ao Telegram com `TELEGRAM_BOT_TOKEN` e `TELEGRAM_CHAT_ID` apenas no worker. Recuperação resolve o grupo e envia aviso. `error_id` aparece nas respostas 500; `requestId` e `X-Request-Id` seguem para correlação. Não registrar corpo, token, WIF ou seed.
- `http_failure_events` guarda apenas módulo, versão, status e horário por sete dias. O worker remove eventos antigos.
