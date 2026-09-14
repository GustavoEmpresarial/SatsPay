# FEATURE — Admin Telemetry

## Keywords

`telemetria APM erros system_error_logs resolve ignore clear client-frontend reportClientError fingerprint TELEMETRY_ALERT_WEBHOOK_URL`

## Abas

1. **Saúde** — stack (API/worker/Postgres), load, overview  
2. **Desempenho** — vazão/latência snapshots worker (ex-APM)  
3. **Erros** — console estilo operações: Sev / Impacto / Cat / Ciclo / Código / Origem / HTTP / Qtd / Amostra / Path / error_id / Último; segredos redigidos (client + ingest) 

UI pt-BR: Autoatualização, Vazão, Cliente (frontend), etc.

## APIs

- `GET /v1/admin/telemetry/overview`  
- `GET /v1/admin/telemetry/metrics-history?hours=`  
- `GET /v1/admin/telemetry/errors?…`  
- `POST …/errors/:id/resolve|ignore`  
- `POST …/errors/resolve-all` · `POST …/errors/clear`  
- `GET /v1/admin/stats` (recursos servidor)  

## Cliente → telemetria

- `client/src/lib/reportError.ts` — fila, throttle, `isExternalNoise`, `isExpectedApiNoise`  
- Ruído filtrado: inventory faucet, login 400, removeChild, React #311 legado, ícones CDN  
- Ingest `POST /v1/telemetry/client-errors` — rate-limit **telemetry-ingest** (60/min/IP, shared PG)

## Fingerprint + alertas

- Fingerprint: service + level + method + status + endpoint normalizado (`:id`) + kind + msg/stack sem UUIDs/números voláteis  
- Env opcional `TELEMETRY_ALERT_WEBHOOK_URL` — webhook best-effort em **novo** CRITICAL/FATAL (e security://) e milestones de spike (10/50/100/…)

## Bug histórico

React #311 em admin: `useAdminStore(…) || useAuthStore(…)` em `RequireAdmin` — **proibido**. Sempre chamar os dois hooks.

## Arquivos

- `client/src/pages/AdminTelemetryPage.tsx`  
- `client/src/lib/errorConsole.ts` — classificação + `redactSecrets`  
- `crates/db/src/telemetry.rs`  
- `docs/quality/error-observability.md`  
- [`../domain-observability/FEATURE.md`](../domain-observability/FEATURE.md)  
