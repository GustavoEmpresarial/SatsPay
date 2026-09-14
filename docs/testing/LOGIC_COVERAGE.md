# Real logic coverage (client + Rust + Docker smokes)

## Commands

```bash
# Pure money/auth/admin helpers — gate: 100% lines/funcs/stmts, ≥95% branches
cd client && npm run test:logic

# Ephemeral Docker Postgres (or DATABASE_URL test DB) — ledger/auth invariants
cd client && npm run test:smoke

# Domain auth + shared money math (Rust)
cargo test -p domain -p shared -p crypto

# Playwright public shell (landing/login)
cd client && npx playwright test e2e/landing-login.spec.ts
```

## What “100% logic” means here

| Layer | Scope | Gate |
|-------|--------|------|
| Client pure logic | `amountInput`, `authValidation`, `coins`, `admin` helpers, explorers, `returnTo`, `formatError`, … | Vitest coverage thresholds in `vitest.config.ts` |
| DB invariants | Unique email, ledger SUM balance, reference dedup | `tests/smoke/authUsersAndLedger.smoke.test.ts` (+ existing smokes) |
| Rust domain | Register/login/refresh/session reuse | `crates/domain` unit tests |
| E2E shell | Landing brand + login form fields | `e2e/landing-login.spec.ts` |

Not gated at 100% line coverage (by design): React page components, `api.ts` network client, `reportError` browser telemetry. Those stay under structure tests (`npm run test:pages`) + smokes/E2E.
