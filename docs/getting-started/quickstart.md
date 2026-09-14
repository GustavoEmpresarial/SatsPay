# Guia de Início Rápido (Quickstart)

Este guia cobre a configuração do ambiente de desenvolvimento local para o ecossistema **BitcoSats**, permitindo rodar o backend Rust (`api-server` e `worker`), o frontend SPA (`client`), o banco de dados PostgreSQL e o barramento Kafka.

---

## 1. Pré-requisitos

Certifique-se de ter instalado em seu sistema:

- **Rust**: Versão estável (1.80+ recomendado) via `rustup`.
- **Node.js**: Versão 20+ LTS e `npm` (ou `pnpm`/`yarn`) para o frontend.
- **Docker & Docker Compose**: Para rodar os serviços de infraestrutura (Postgres, Kafka).
- **PostgreSQL Client (`psql`)**: Opcional, para inspeção direta do banco.
- **k3d & kubectl**: Opcional, caso queira rodar a topologia Kubernetes local completa.
- **OpenSSL**: Para geração de chaves criptográficas (`openssl rand -hex 32`).

---

## 2. Estrutura do Workspace

```text
current/
├── Cargo.toml               # Workspace Cargo com 12 crates
├── crates/                  # Módulos Rust do backend
│   ├── shared/              # Definições de moedas, tipos base e matemática financeira
│   ├── domain/              # Modelos de domínio e traits de repositório (Auth, Ledger)
│   ├── db/                  # SQLx, migrations e operações transacionais com Postgres
│   ├── crypto/              # AES-256-GCM, Argon2id, JWT e HMAC
│   ├── chain/               # Clientes de blockchain (RPCs reais e Stubs de teste)
│   ├── events/              # Eventos de domínio e integração Kafka (Outbox / Consumer)
│   ├── queue/               # Fila interna de jobs via PostgreSQL (SKIP LOCKED)
│   ├── notify/              # Envio de e-mails transacionais (SMTP / Mock)
│   ├── captcha/             # Validação de Cloudflare Turnstile com anti-replay
│   ├── pricing/             # Cotações de mercado via CoinGecko com cache
│   ├── api-http/            # Camada de transporte HTTP (Axum, rotas, middlewares)
│   ├── worker/              # Binário do daemon de background (jobs e outbox relay)
│   └── api-server/          # Binário do servidor HTTP principal
├── client/                  # Frontend SPA em React 18, Vite, TypeScript e TailwindCSS
├── deploy/                  # Configurações de Docker Compose e manifests Kubernetes (k8s)
├── docs/                    # Documentação técnica de ponta a ponta
└── storage/                 # Armazenamento local de imagens, backups, temporários e logs
```

---

## 3. Configuração Rápida com Docker Compose

A forma mais rápida de subir toda a infraestrutura e a aplicação localmente é através do `docker-compose.yml`:

```bash
cd deploy/docker
docker compose up -d postgres kafka
```

Isso inicializa:
- **PostgreSQL 15** na porta `5432` (banco `bitcosats`, usuário `bitcosats`, senha `bitcosats_dev_password`).
- **Apache Kafka (KRaft)** na porta `9092`.

---

## 4. Executando o Backend via Cargo

### 4.1 Configurar Variáveis de Ambiente

Crie um arquivo de ambiente ou exporte as variáveis no terminal:

```bash
# Banco de Dados
export DATABASE_URL="postgresql://bitcosats:bitcosats_dev_password@127.0.0.1:5432/bitcosats"

# Segurança e Criptografia
export JWT_ACCESS_SECRET="dev-jwt-access-secret-32-chars-long!"
export ENCRYPTION_KEY="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
export ADMIN_EMAILS="admin@bitcosats.local,operador@bitcosats.local"

# Blockchain & Chains (Modo Stub para desenvolvimento local)
export ALLOW_STUB_CHAIN="true"
export USE_REAL_CHAIN_CLIENTS="false"

# Servidor HTTP
export BIND_ADDR="127.0.0.1:4000"
export NODE_ENV="development"
export TURNSTILE_SECRET="disabled_in_dev"

# Kafka & Outbox Relay
export KAFKA_BOOTSTRAP_SERVERS="127.0.0.1:9092"
export OUTBOX_RELAY_INTERVAL_MS="500"
export OUTBOX_RELAY_BATCH_SIZE="50"

# Intervalos do Worker
export DEPOSIT_WATCHER_INTERVAL_SECS="10"
export WITHDRAWAL_BROADCAST_INTERVAL_SECS="5"
export WITHDRAWAL_REQUEUE_INTERVAL_SECS="30"
export PRICE_REFRESH_INTERVAL_SECS="60"
export PRICE_DECIMALS="8"
export PRICE_MAX_STALE_SECS="300"
export COINGECKO_API_BASE_URL="https://api.coingecko.com/api/v3"
export COINGECKO_TIMEOUT_SECS="10"
export REWARDS_TICK_INTERVAL_SECS="60"
```

### 4.2 Rodar as Migrations e Iniciar o `api-server`

O binário `api-server` executa automaticamente as migrations do SQLx ao iniciar:

```bash
cargo run -p api-server
```

O servidor estará respondendo em: `http://127.0.0.1:4000`
- Health check: `GET http://127.0.0.1:4000/healthz`
- Métricas Prometheus: `GET http://127.0.0.1:4000/metrics`

### 4.3 Iniciar o `worker`

Em outro terminal com as mesmas variáveis de ambiente:

```bash
cargo run -p worker
```

O worker inicializa as carteiras `HOUSE` e `LEND_POOL`, processa depósitos/saques e dispara o `outbox_relay` para o Kafka.

---

## 5. Executando o Frontend (`client`)

Em um terceiro terminal:

```bash
cd client
npm install
npm run dev
```

A interface estará acessível em: `http://localhost:5173`
As requisições `/v1/*` e `/healthz` são automaticamente redirecionadas via proxy do Vite para `http://127.0.0.1:4000`.

---

## 6. Fluxos Iniciais de Teste

### 6.1 Criar Conta e Autenticar
1. Acesse `http://localhost:5173/register` ou use o endpoint:
   ```bash
   curl -X POST http://127.0.0.1:4000/v1/auth/register \
     -H "Content-Type: application/json" \
     -d '{
       "email": "user@example.com",
       "username": "satoshi",
       "password": "Password123!",
       "confirmPassword": "Password123!",
       "acceptTerms": true
     }'
   ```
2. Faça login para receber o Access Token e o Cookie de Refresh:
   ```bash
   curl -X POST http://127.0.0.1:4000/v1/auth/login \
     -H "Content-Type: application/json" \
     -d '{
       "email": "user@example.com",
       "password": "Password123!"
     }'
   ```

### 6.2 Testar o Faucet
Em modo `development`, o captcha é aceito automaticamente com `"disabled_in_dev"`:
```bash
curl -X POST http://127.0.0.1:4000/v1/faucet/claim/BTC \
  -H "Authorization: Bearer <SEU_ACCESS_TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{"captchaToken": "disabled_in_dev"}'
```

### 6.3 Consultar o Saldo das Carteiras
```bash
curl -X GET http://127.0.0.1:4000/v1/wallet \
  -H "Authorization: Bearer <SEU_ACCESS_TOKEN>"
```

---

## 7. Execução de Testes e Checagens de Qualidade

```bash
# Executar todos os testes unitários da workspace
cargo test --workspace

# Executar os testes de integração do banco de dados (requer Postgres rodando)
cargo test -p db --test ledger_smoke

# Executar linter sem warnings
cargo clippy --workspace --all-targets -- -D warnings

# Verificar formatação de código
cargo fmt --all -- --check
```
