# Checklist de fases

| Fase | O que | Status |
|---|---|---|
| 0 | Infra k8s base (k3d, CloudNativePG, Strimzi) | ✅ Completa |
| 1 | Fundações (workspace, shared, db, crypto, events) | ✅ Completa |
| 2 | Ledger + Auth | ✅ Completa |
| 3 | Wallet + Deposits + chain stub + outbox_relay | ✅ Completa |
| 4 | Withdrawals (state machine + reconciler) | ✅ Completa |
| 5 | Swap, Faucet, Faucetlist | ✅ Completa (faucetlist só superfície pública) |
| 6 | Stake, Lend, Rewards | ✅ Completa (stake, lend incl. liquidação, rewards/liquidity mining) |
| 7 | Merchant, Public API, Admin | ✅ Completa (public-api com `x-api-key` e HMAC assinado) |
| 8 | Hardening e corte | ✅ Completa — ver detalhe abaixo |
| 9 | Frontend (React + Vite) migrado de `legacy/apps/web` → `current/client` | ✅ Build passa; `@bitcosats/shared` inlinado em `client/src/shared`; sem pnpm workspace. Deployado na VM (`bitcosats-client` @ :4500) substituindo `bitcosats-web`. Pasta `legacy/` deletada. |

## Fase 8 em detalhe

| Item | Status |
|---|---|
| Integração on-chain real | ✅ Leitura (5) + broadcast BTC/LTC/DOGE/BCH/POL; smoke de assinatura offline; fundos reais ainda não exercitados |
| Envio de e-mail real (SMTP) | ✅ OTP/2FA + best-effort (merchant / faucetlist / public-api send) |
| Captcha real (Turnstile) | ✅ |
| Liquidação de lend | ✅ |
| Programa de rewards | ✅ |
| Feed de preço real (CoinGecko) | ✅ |
| Assinatura HMAC da API pública | ✅ |
| IP real do cliente | ✅ `X-Forwarded-For` / `X-Real-IP` / `ConnectInfo` |
| Deployments reais no k8s | ✅ |
| Backup/PITR do Postgres | ✅ mecanismo + `scripts/exercise-pitr-restore.sh` — restore contra bucket real ainda não rodado |
| TLS | ✅ manifests — hostname via `prod.env` |
| Secrets via External Secrets Operator | ✅ manifests — Vault via `prod.env` |
| Overlay staging dual-run | ✅ `deploy/k8s/overlays/staging` |
| CI | ✅ `.github/workflows/ci.yml` |
| Smoke ledger `#[sqlx::test]` | ✅ `crates/db/tests/ledger_smoke.rs` |

## O que ainda falta pra "corte" de produção de verdade

Ver `security/threat-model-and-gaps.md`. Curto: (1) broadcast com hot wallet fundada, (2) rodar `scripts/exercise-pitr-restore.sh` contra S3 real, (3) preencher `overlays/prod/prod.env` (domínio/Vault/registry/S3) — zero placeholders.
