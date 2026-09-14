# Fluxo de Desenvolvimento e Boas Práticas

Este documento detalha o padrão de desenvolvimento do projeto **BitcoSats**, estrutura das crates Rust, convenções de código, gestão de migrations e boas práticas para novas contribuições.

---

## 1. Padrão de Organização das Crates

O backend adota uma arquitetura modular baseada em responsabilidades claras:

| Crate | Responsabilidade | Dependências Externas I/O |
|---|---|---|
| `shared` | Tipos puros, moedas (`BTC`, `LTC`, `DOGE`, `BCH`, `POL`), unidades e matemática financeira | Nenhuma (sem I/O, puro) |
| `domain` | Interfaces de repositório e regras centrais de autenticação/ledger | Nenhuma (traits puros e fake repos) |
| `db` | Implementações PostgreSQL com SQLx, queries transacionais e controle de concorrência | PostgreSQL / SQLx |
| `crypto` | Criptografia simétrica (AES-256-GCM com AAD), hashing Argon2id, JWT HS256 e HMAC-SHA256 | ring / argon2 / hmac |
| `chain` | Traits de clientes de blockchain, derivação HD (BIP32/44) e builders de transação/assinatura | HTTP / RPCs de nós |
| `events` | Definição de eventos de domínio, escrita de outbox transacional e produtores/consumidores Kafka | rdkafka / Kafka |
| `queue` | Fila interna de tarefas em Postgres usando `SKIP LOCKED` | SQLx / PostgreSQL |
| `notify` | Sistema de e-mails transacionais (OTP, alertas) via SMTP | lettre |
| `captcha` | Verificador do Cloudflare Turnstile com anti-replay em Postgres | reqwest / SQLx |
| `pricing` | Cliente e cache de cotações de mercado via CoinGecko | reqwest / SQLx |
| `api-http` | Servidor HTTP Axum, middlewares (auth, timeout, rate limit), controllers e serialização | Axum / Tower |
| `worker` | Daemon de tarefas em segundo plano (watcher de depósito, reconciliador de saque, outbox relay) | Tokio / SQLx / Kafka |
| `api-server` | Entrypoint binário do serviço HTTP | Tokio / Axum |

---

## 2. Padrões de Código e Transações

### 2.1 Regra de Ouro do Ledger
- **Nunca** crie colunas de saldo (`balance`, `current_balance`, etc.) em nenhuma tabela.
- O saldo é sempre a soma calculada dos lançamentos no ledger: `SELECT COALESCE(SUM(amount), 0) FROM ledger_entries WHERE wallet_id = $1`.
- Toda alteração financeira deve travar explicitamente a linha da carteira (`SELECT id FROM wallets WHERE id = $1 FOR UPDATE`) antes de qualquer cálculo ou inserção.

### 2.2 Tratamento de Erros e Tipos
- Erros de domínio e de infraestrutura devem ser modelados via `thiserror`.
- Em respostas HTTP, **nunca** vaze mensagens internas do banco de dados (`sqlx::Error`) diretamente ao cliente em produção. Use mensagens padronizadas e logue o erro com `tracing::error!`.

### 2.3 Idempotência em Todas as Operações Críticas
- Toda operação financeira (saque, swap, transferências via API pública) exige uma `idempotency_key` única.
- As deduplicações são reforçadas a nível de banco de dados por índices únicos (ex: `uq_ledger_reference_type_dedup` e `uq_ledger_reference_key_dedup`).

---

## 3. Gestão de Migrations com SQLx

As migrations estão localizadas em `crates/db/migrations/`.

### 3.1 Criando uma Nova Migration
Adicione um novo arquivo sequencial com o formato `XXXX_descricao.sql`:
```bash
# Exemplo:
crates/db/migrations/0010_adicionar_tabela_exemplo.sql
```

### 3.2 Execução Automática
Ao rodar `api-server` ou `worker`, a função `db::run_migrations(&pool)` executa automaticamente todas as migrations pendentes de forma idempotente.

---

## 4. Testes e Validações

### 4.1 Testes Unitários
```bash
cargo test --workspace
```

### 4.2 Testes de Integração com Postgres
Para rodar a suíte de testes de integração financeira que utiliza transações reais no Postgres:
```bash
cargo test -p db --test ledger_smoke
```

### 4.3 Smoke Tests Manuais
Exemplos executáveis em `crates/db/examples/` cobrem fluxos completos:
```bash
# Teste do fluxo completo de saques e reconciliação
cargo run -p db --example withdrawal_smoke

# Teste de swap e faucet com validação de limites
cargo run -p db --example swap_faucet_smoke

# Teste de staking e mercado de lending
cargo run -p db --example stake_lend_smoke

# Teste de aprovação merchant e API pública com HMAC
cargo run -p db --example merchant_admin_pubapi_smoke
```

---

## 5. Pipeline de CI / CD

Toda alteração enviada para o repositório é verificada pelo GitHub Actions (`.github/workflows/ci.yml`), que executa:
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. Build das imagens Docker (`Dockerfile.api-server` e `Dockerfile.worker`)
5. Validação dos manifests Kubernetes com Kustomize (`kubectl kustomize deploy/k8s/overlays/dev`)
