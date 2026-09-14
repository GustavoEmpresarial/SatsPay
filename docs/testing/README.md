# Pasta de testes — mapa

## Client (`client/tests/`)

```
client/tests/
  helpers/           # Postgres, contracts, asserts
  unit/
    lib/ shared/ pages/ components/
    property/        # fast-check invariants + fuzz
    contract/        # JSON shape contracts
    acceptance/      # business rules as tests
    security/ i18n/ architecture/ observability/ performance/
  api/               # live HTTP negatives + healthz
  smoke/             # Docker Postgres (ledger, migrations, concurrency…)
client/e2e/          # Playwright (shell, a11y, oauth)
```

```bash
cd client && npm run test:unit
cd client && npm run test:logic      # coverage gate
cd client && npm run test:property
cd client && npm run test:contract
cd client && npm run test:api
cd client && npm run test:security
cd client && npm run test:smoke
cd client && npm run test:e2e
```

## Backend (Rust)

```bash
cargo test --workspace
cargo run -p db --example swap_faucet_smoke
```

## Docs

- [`types-catalog.md`](types-catalog.md) — mapa dos ~70 tipos
- [`LOGIC_COVERAGE.md`](LOGIC_COVERAGE.md) — gate 100% lógica pura
- [`test-strategy.md`](test-strategy.md) — histórico/estratégia
- Por página: [`../pages/`](../pages/)
