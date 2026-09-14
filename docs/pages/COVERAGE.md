# Meta de cobertura automatizada

## Escopo medido

**Módulo de página** = 1 pasta em `docs/pages/<slug>` + testes em `client/tests/unit/pages/<slug>/` (ou lote 1 equivalente).

| Meta | Alvo | Status |
|------|------|--------|
| Docs + structure test por página | ≥ 80% | **100%** (46/46) |
| Ideal docs+structure | 97–98% | **100%** |
| Lógica pura (money/auth/admin helpers) | 100% lines | **100%** — `npm run test:logic` (ver `docs/testing/LOGIC_COVERAGE.md`) |
| API/smoke fluxos $$ + ledger | deposit/withdraw/swap/faucet/auth | **Docker smokes** (`npm run test:smoke`) |
| E2E Playwright | shell + oauth | `e2e/landing-login.spec.ts` + oauth (gated) |
| Rust domain/shared | auth + swap math | `cargo test -p domain -p shared -p crypto` |

## Como regenerar matriz

```bash
node scripts/generate_page_coverage.mjs
cd client && npm run test:unit -- tests/unit/pages
```

## Próximo nível

1. Playwright autenticado: login → dashboard → deposit QR (`SATSPAY_E2E=1`)
2. Coverage em pages React (component mount) — separado da gate de lógica pura
3. API tests auth negativos (captcha, lockout) além dos smokes atuais
