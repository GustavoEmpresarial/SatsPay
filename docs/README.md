# BitcoSats — Documentação Técnica Completa

Bem-vindo à documentação técnica oficial do **BitcoSats**, uma plataforma financeira moderna de custódia e serviços para múltiplos criptoativos (**Bitcoin, Litecoin, Dogecoin, Bitcoin Cash e Polygon**), construída em **Rust (Axum + SQLx)**, **PostgreSQL 15+**, **Apache Kafka** e **React 18 / TypeScript**.

---

## 🗺️ Mapa da Documentação

### 1. Início Rápido e Desenvolvimento
- [`getting-started/quickstart.md`](getting-started/quickstart.md): Guia passo a passo para subir todo o ambiente local via Docker Compose / Cargo e executar os primeiros fluxos.
- [`getting-started/development-workflow.md`](getting-started/development-workflow.md): Padrões de código, organização das 12 crates do workspace, criação de migrations e execução de testes.

### 2. Arquitetura do Sistema
- [`architecture/overview.md`](architecture/overview.md): Visão geral da topologia de crates, isolamento de camadas e fluxo de requests/jobs.
- [`architecture/ledger.md`](architecture/ledger.md): Especificação do motor contábil, partidas dobradas, tipos de lançamentos e modelo de concorrência.
- [`architecture/events-and-jobs.md`](architecture/events-and-jobs.md): Padrão Transactional Outbox, streaming com Apache Kafka e fila interna com PostgreSQL `SKIP LOCKED`.
- [`architecture/chain-integration.md`](architecture/chain-integration.md): Clientes on-chain (UTXO/EVM), derivação hierárquica HD (BIP32/44) e máquina de estados de saques.
- [`architecture/lightning.md`](architecture/lightning.md): Arquitetura de integração com Bitcoin Lightning Network via microserviço `ln-bridge` e isolamento mTLS.

### 3. Referência de APIs
- [`api/http-api-reference.md`](api/http-api-reference.md): Documentação exaustiva de todos os endpoints REST (Auth, Wallets, Deposits, Withdrawals, Swap, Faucet, Staking, Lending, Admin).
- [`api/public-api-hmac.md`](api/public-api-hmac.md): Guia de integração B2B para desenvolvedores externos, emissão de API Keys, assinatura HMAC-SHA256 e exemplos de código.

### 4. Banco de Dados e Modelagem
- [`database/schema-and-models.md`](database/schema-and-models.md): Dicionário de dados, relacionamentos, enums customizados, índices e histórico de migrations.
- [`database/ledger-invariants.md`](database/ledger-invariants.md): Invariantes matemáticos e relacionais para garantia de saldo não-negativo e prevenção de race conditions.

### 5. Frontend & Aplicação Web
- [`client/frontend-architecture.md`](client/frontend-architecture.md): Single Page Application em React 18, Vite, TypeScript, TailwindCSS, interceptor de refresh token e build em Nginx.
- [`pages/`](pages/): Documentação **por página** (landing, login, register…) — overview, rotas/API e checklist de testes.
- **[`features/`](features/README.md): FEATURE.md + TC.md por aba/domínio — índice bruto para busca por IA** (`rg keyword docs/features`). Gerador: `python3 scripts/generate_feature_docs.py`.

### 6. Processamento em Segundo Plano
- [`worker/background-jobs.md`](worker/background-jobs.md): Tarefas autônomas do daemon worker (watcher de depósitos, reconciliador de saques, outbox relay, cotações e recompensas).

### 7. Segurança e Auditoria
- [`security/BALANCE_SECURITY.md`](security/BALANCE_SECURITY.md): O modelo formal de segurança de saldo (por que não existem colunas mutáveis de saldo).
- [`security/threat-model-and-gaps.md`](security/threat-model-and-gaps.md): Vetores T1–T12 e checklist mainnet (prod = compose VM).
- [`security/SAST.md`](security/SAST.md): CodeQL CI + regras Semgrep de domínio (BitcoSats).
- [`security/audit-2026-09-01.md`](security/audit-2026-09-01.md): Addendum 2026-09-16 + snapshot histórico 01/09.

### 8. Operações e DevOps
- [`operations/deployment.md`](operations/deployment.md): **Prod = compose VM + Caddy**; overlays k8s são lab/futuro.
- [`operations/vm-security-runbook.md`](operations/vm-security-runbook.md): SSH key-only, backup/`ENCRYPTION_KEY`, LUKS, SMTP/admin, rotação AES.
- [`operations/env-vars-reference.md`](operations/env-vars-reference.md): Tabela de referência completa de todas as variáveis de ambiente.
- [`operations/backup-restore-postgres.md`](operations/backup-restore-postgres.md): Procedimentos de backup contínuo com Barman/S3 e recuperação Point-in-Time (PITR).

### 9. Testes e Qualidade
- [`testing/README.md`](testing/README.md): Mapa das pastas de teste (client unit/smoke/e2e + Rust).
- [`testing/test-strategy.md`](testing/test-strategy.md): Estratégia de testes unitários, integração SQLx e smoke financeiros.
- [`testing/types-catalog.md`](testing/types-catalog.md): Catálogo de tipos de teste (pirâmide SatsPay + mapeamento dos ~70 tipos).
- [`quality/test-types.md`](quality/test-types.md): Catálogo completo dos 70 tipos de teste (regra Claude Code).
- [`quality/security-checklist.md`](quality/security-checklist.md): Checklist de segurança (regra Claude Code).
- [`quality/error-observability.md`](quality/error-observability.md): Sistema de erros e observabilidade (regra Claude Code).

### 10. Registros de Decisão de Arquitetura (ADRs)
- [`decisions/0001-reescrita-rust-axum-sqlx.md`](decisions/0001-reescrita-rust-axum-sqlx.md) — Reescrita do legado Node.js para Rust.
- [`decisions/0002-ledger-contabil-partidas-dobradas.md`](decisions/0002-ledger-contabil-partidas-dobradas.md) — Razão contábil imutável sem coluna de saldo.
- [`decisions/0003-transactional-outbox-kafka.md`](decisions/0003-transactional-outbox-kafka.md) — Publicação de eventos de domínio via outbox.
- [`decisions/0004-fila-interna-postgres-skip-locked.md`](decisions/0004-fila-interna-postgres-skip-locked.md) — Fila de jobs em PostgreSQL sem dependência de Redis.
- [`decisions/0005-derivacao-hd-xpub-sequencia-postgres.md`](decisions/0005-derivacao-hd-xpub-sequencia-postgres.md) — Derivação HD segura usando apenas xpub no servidor web.
- [`decisions/0006-lock-wallet-row-not-ledger-aggregate.md`](decisions/0006-lock-wallet-row-not-ledger-aggregate.md) — Trava pessimista na linha da carteira.
- [`decisions/0007-api-publica-assinatura-hmac.md`](decisions/0007-api-publica-assinatura-hmac.md) — Protocolo de assinatura HMAC e nonces anti-replay.
- [`decisions/0008-real-chain-clients.md`](decisions/0008-real-chain-clients.md) — Integração on-chain real com BTC, LTC, DOGE, BCH e POL.
- [`decisions/0009-gestao-segredos-external-secrets-operator.md`](decisions/0009-gestao-segredos-external-secrets-operator.md) — Centralização de segredos no Vault com ESO.
- [`decisions/0010-feed-precos-coingecko-cache.md`](decisions/0010-feed-precos-coingecko-cache.md) — Feed de cotações com cache e fail-safe.
- [`decisions/0011-lightning-btc-custodia-e-isolamento.md`](decisions/0011-lightning-btc-custodia-e-isolamento.md) — Custódia e isolamento na Lightning Network com `ln-bridge`.
- [`decisions/0012-isolamento-signer-worker.md`](decisions/0012-isolamento-signer-worker.md) — Chaves de carteira só no worker; api-server watch-only.

---

## 📈 Status da Migração
Consulte [`migration/phase-checklist.md`](migration/phase-checklist.md) para verificar o status detalhado de cada fase da reescrita da plataforma.
