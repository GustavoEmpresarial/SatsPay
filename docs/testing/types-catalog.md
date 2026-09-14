# Catálogo de tipos de teste (SatsPay)

Referência dos ~70 tipos adaptada ao monorepo. **Não** criamos 70 pastas —
agrupamos nas suítes abaixo. Detalhe de lógica pura: [`LOGIC_COVERAGE.md`](LOGIC_COVERAGE.md).

## Suítes do repositório

| Pasta / comando | Tipos cobertos |
|-----------------|----------------|
| `npm run test:unit` | Unit, property, fuzz, contract, acceptance, i18n, a11y helpers, arch, perf micro, security static |
| `npm run test:logic` | Coverage gate 100% em money/auth helpers |
| `npm run test:property` | Property-based (#52) + acceptance (#15/#69) |
| `npm run test:contract` | Consumer contracts frontend↔API (#6/#54) |
| `npm run test:api` | API live negatives + health (#5/#29) — `SATSPAY_API_LIVE=0` skip |
| `npm run test:security` | Headers/CSP/secrets/nginx IP (#10) |
| `npm run test:smoke` | Smoke/DB/concurrency/migrations/idempotency (#11/#13/#24/#25/#31/#41) |
| `npm run test:e2e` | Playwright shell + oauth + a11y (#4/#18) |
| `cargo test --workspace` | Unit domain/shared/crypto + refresh concurrency |
| `crates/db/examples/*_smoke.rs` | System/financial smokes (manual) |
| CI `.github/workflows/ci.yml` | Lint pyramid: unit → logic gate → smoke-db → audit |

## Pirâmide

```
              E2E (Playwright: shell, a11y, oauth)
             /                                    \
      Smoke / System (Docker PG)              Acceptance (businessRules)
           /                                        \
    API live negatives                         Contract (JSON shapes)
         /                                              \
   Property + Fuzz                               Unit (client + Rust)
```

Por fora (automatizado leve): security headers, i18n parity, architecture boundaries,
observability hygiene, micro perf budgets, SCA (`cargo audit` / `npm audit`).

**Não automatizado de propósito (ops / raro):** chaos, k8s, mutation, visual baseline,
soak 6h, GraphQL/WS (N/A).

## Mapa dos 70 tipos → onde vivem

| # | Família | Status | Onde |
|---|---------|--------|------|
| 1 | Unit | ✅ | `tests/unit`, `cargo test` |
| 2 | Integration | ✅ | `tests/smoke` + db examples |
| 3–4 | System / E2E | ✅ parcial | Playwright + smokes |
| 5 | API | ✅ | `tests/api` |
| 6 | Contract | ✅ | `tests/unit/contract` + helpers |
| 7 | Functional | ✅ | acceptance + page checklists |
| 8–9 | Non-func / perf | ✅ leve | `tests/unit/performance` |
| 10 | Security | ✅ | security/nginx/secrets + auth domain |
| 11 | Database | ✅ | migrations + ledger smokes |
| 12–14 | Regression / smoke / sanity | ✅ | CI + smokes |
| 15 | Acceptance | ✅ | `tests/unit/acceptance` |
| 16 | Exploratory | 📋 manual | checklists |
| 17–18 | UX / a11y | ✅ leve | `e2e/a11y-shell` |
| 19–22 | Compat / UI / visual / responsive | 📋 | Playwright devices futuro |
| 23 | i18n | ✅ | `tests/unit/i18n` |
| 24–25 | Concurrency / idempotency | ✅ | ledgerConcurrency + captcha/swap smokes |
| 26–29 | Resilience / chaos / recovery / availability | ✅ parcial | `/healthz` API + ops |
| 30 | Architecture | ✅ | `tests/unit/architecture` |
| 31 | Migration | ✅ | `migrations.smoke` |
| 32–34 | Deploy / CI / config | ✅ | workflow + secrets-hygiene |
| 35–37 | Jobs / events / cache | 📋 | worker examples |
| 38–39 | Files / notifications | 📋 | conforme feature |
| 40–41 | Chain / financial | ✅ | property + smokes + rust shared |
| 42–45 | Data / obs / logging / audit | ✅ parcial | logHygiene + auditActions |
| 46 | Permissions | ✅ parcial | admin gate rust + API 401 |
| 47–50 | Prod / install / upgrade / rollback | 📋 | ops |
| 51 | Mutation | ❌ skip | custo alto |
| 52–53 | Property / fuzz | ✅ | `tests/unit/property` |
| 54 | Data contract | ✅ | contracts helpers |
| 55–61 | GraphQL/WS/k8s… | N/A | — |
| 62–65 | Docs / static / build / deps | ✅ | eslint, tsc, audit |
| 66–67 | API compat / migration | 📋 | |
| 68–69 | Business acceptance / rules | ✅ | acceptance + property |
| 70 | Operational security | ✅ parcial | cookie Host-, secrets CI |

## Pipeline

```
Lint → Typecheck → Unit(+property/contract) → Logic coverage → Smoke(Docker PG)
  → API live (opt) → E2E shell (opt) → Security audit → Deploy → /healthz
```

## Comandos rápidos

```bash
cd client && npm run test:pyramid
cd client && npm run test:smoke
cd client && npm run test:api          # live; SATSPAY_API_LIVE=0 to skip
cd client && npx playwright test e2e/landing-login.spec.ts e2e/a11y-shell.spec.ts
cargo test --workspace --lib
```
