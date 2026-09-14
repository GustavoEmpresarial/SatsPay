# Estratégia de testes

Ver também: [`README.md`](README.md) (mapa de pastas) e [`types-catalog.md`](types-catalog.md) (tipos de teste).  
Documentação por página: [`../pages/`](../pages/).

## O que existe hoje (atualizado)

- **Client unit pyramid**: property/fuzz (`fast-check`), contracts, acceptance, i18n parity, architecture boundaries, security headers, micro perf, logic coverage gate.
- **API live**: `tests/api` contra prod (`/healthz`, auth 401, login 4xx, swap prices contract).
- **Docker PG smokes**: ledger concurrency, migrations, auth/ledger invariants, withdrawal reverse, faucet, idempotency.
- **E2E**: landing/login shell + a11y shell + oauth (gated).
- **Rust**: domain auth (incl. refresh concurrency) + shared money math + crypto.
- **CI**: unit + logic coverage + smoke-db + audit + secrets hygiene.

Ver [`types-catalog.md`](types-catalog.md) para o mapa completo dos ~70 tipos.

## Por que smoke tests manuais em vez de `sqlx::test`

Dado o volume de módulos portados numa única sessão, os smoke tests contra o cluster k3d real (não um Postgres efêmero de CI) foram o jeito mais rápido de validar contra o schema/migrations reais, incluindo pegar bugs que só aparecem com Postgres de verdade (ex.: o bug do lock de wallet só apareceu rodando swap de verdade). **Isso não substitui uma suíte de CI** — é um gap conhecido.

## TODO (não feito ainda)

- Migrar os smoke tests pra `#[sqlx::test]` (transação isolada por teste, roda em CI sem precisar de cluster externo).
- Testes de paridade numérica contra o legado TypeScript (mesmos inputs, mesmo output) — não foi possível nesta sessão por não haver runtime Node disponível pra rodar o legado lado a lado.
- Testes de carga/fuzzing no ledger sob alta concorrência (o smoke test de saque testa 2 chamadas simultâneas; não testa 50).
- CI real: `cargo clippy -D warnings`, `cargo fmt --check`, `cargo test --workspace`, dry-run de migration — nenhum pipeline de CI foi configurado nesta sessão.
