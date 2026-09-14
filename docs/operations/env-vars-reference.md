# Referência de Variáveis de Ambiente

Este documento cataloga todas as variáveis de ambiente utilizadas pelos serviços **`api-server`**, **`worker`**, **`client`** e **`ln-bridge`**.

---

## 1. Banco de Dados e Infraestrutura

| Variável | Serviços | Obrigatória? | Exemplo / Padrão | Descrição |
|---|---|---|---|---|
| `DATABASE_URL` | `api-server`, `worker` | Sim | `postgresql://user:pass@host:5432/bitcosats` | URL de conexão com o PostgreSQL |
| `BIND_ADDR` | `api-server` | Não | `127.0.0.1:4000` | Endereço IP e porta para o servidor HTTP |
| `NODE_ENV` | `api-server`, `client` | Não | `development` / `production` | Ambiente de execução |
| `KAFKA_BOOTSTRAP_SERVERS` | `worker` | Não | `127.0.0.1:9092` | Endereço dos brokers Kafka para publicação do outbox |

---

## 2. Segurança, Chaves e Autenticação

| Variável | Serviços | Obrigatória? | Exemplo | Descrição |
|---|---|---|---|---|
| `JWT_ACCESS_SECRET` | `api-server` | Sim | `segredo-longo-e-aleatorio-32-bytes` | Segredo para assinatura de Access Tokens JWT |
| `ENCRYPTION_KEY` | `api-server` | Sim | Hex 64 chars (32 bytes) | Chave mestre AES-256-GCM para criptografia de segredos e TOTP |
| `ADMIN_EMAILS` | `api-server` | Não | `admin@bitcosats.com,op@...` | Lista de e-mails autorizados para papel `ADMIN` |
| `TURNSTILE_SECRET` | `api-server` | Não | `0x4AAAAAA...` / `disabled_in_dev` | Chave secreta do Cloudflare Turnstile |

---

## 3. Blockchain e Clientes On-Chain

| Variável | Serviços | Obrigatória? | Exemplo / Padrão | Descrição |
|---|---|---|---|---|
| `USE_REAL_CHAIN_CLIENTS` | `api-server`, `worker` | Não | `false` / `true` | Ativa clientes de blockchain reais (Bitcore/Polygon) |
| `ALLOW_STUB_CHAIN` | `api-server`, `worker` | Não | `true` / `false` | Permite o uso de stubs de chain (proibido em produção) |
| `CHAIN_NETWORK` | `api-server`, `worker` | Se real=true | `mainnet` / `testnet` | Rede blockchain alvo |
| `BTC_XPUB`, `LTC_XPUB`, ... | `api-server` | Se real=true | `xpub6...` | Chaves públicas estendidas para geração de endereços |
| `BTC_HOT_WIF`, `POL_PRIVATE_KEY` | `worker` | Se real=true | WIF / Hex Key | Chaves privadas para assinatura e broadcast de saques |
| `POL_RPC_URL` | `api-server`, `worker` | Se real=true | `https://polygon-rpc.com` | Endpoint JSON-RPC da rede Polygon |

---

## 4. Intervalos e Jobs do Worker

| Variável | Serviços | Obrigatória? | Padrão | Descrição |
|---|---|---|---|---|
| `DEPOSIT_WATCHER_INTERVAL_SECS` | `worker` | Sim | `10` | Frequência de varredura de depósitos on-chain |
| `WITHDRAWAL_BROADCAST_INTERVAL_SECS` | `worker` | Sim | `5` | Frequência de envio de saques enfileirados |
| `WITHDRAWAL_REQUEUE_INTERVAL_SECS` | `worker` | Sim | `30` | Intervalo de verificação de saques pendentes de re-enfileiramento |
| `OUTBOX_RELAY_INTERVAL_MS` | `worker` | Se Kafka ativo | `500` | Frequência em ms do outbox relay |
| `OUTBOX_RELAY_BATCH_SIZE` | `worker` | Se Kafka ativo | `50` | Quantidade de eventos por lote publicado |
| `PRICE_REFRESH_INTERVAL_SECS` | `worker` | Sim | `60` | Frequência de atualização de preços via CoinGecko |
| `PRICE_DECIMALS` | `api-server`, `worker` | Sim | `8` | Precisão decimal utilizada para cotações |
| `PRICE_MAX_STALE_SECS` | `api-server`, `worker` | Sim | `300` | Idade máxima aceitável de um preço antes de considerá-lo obsoleto |
| `SWAPKIT_ENABLED` | `api-server`, `worker` | Não | `false` | Liga rotas DEX SwapKit (`true` + API key) |
| `SWAPKIT_API_KEY` | `api-server`, `worker` | Não | — | API key Partner SwapKit (sem key = só pool HOUSE) |
| `SWAPKIT_BASE_URL` | `api-server`, `worker` | Não | `https://api.swapkit.dev` | Base URL SwapKit |
| `SWAP_PLATFORM_FEE_BPS_SAME` | `api-server`, `worker` | Não | `25` | Taxa SatsPay same-chain (bps) |
| `SWAP_PLATFORM_FEE_BPS_CROSS` | `api-server`, `worker` | Não | `50` | Taxa SatsPay cross-chain (bps) |
| `SWAP_SLIPPAGE_PCT` | `api-server`, `worker` | Não | `2` | Slippage máximo nas quotes SwapKit (%) |
| `DEX_SWAP_INTERVAL_SECS` | `worker` | Não | `15` | Intervalo do worker de broadcast/track DEX |
| `COINGECKO_API_BASE_URL` | `worker` | Sim | `https://api.coingecko.com/api/v3` | URL da API do CoinGecko |
| `COINGECKO_TIMEOUT_SECS` | `worker` | Sim | `10` | Timeout de requisições ao CoinGecko |
| `REWARDS_TICK_INTERVAL_SECS` | `worker` | Sim | `60` | Frequência de cálculo de recompensas de liquidez |

---

## 5. E-mail Transacional (SMTP)

| Variável | Serviços | Obrigatória? | Exemplo | Descrição |
|---|---|---|---|---|
| `SMTP_ENABLED` | `api-server` | Não | `true` / `false` | Liga `SmtpSender`; `false` = `NoopEmailSender` (OTP não sai) |
| `SMTP_HOST` | `api-server` | Se enabled | `smtp.hostinger.com` | Host do servidor SMTP |
| `SMTP_PORT` | `api-server` | Se enabled | `587` | Porta SMTP (STARTTLS) |
| `SMTP_USERNAME` | `api-server` | Se enabled | `no-reply@…` | Usuário SMTP (`SMTP_USER` legado não é lido) |
| `SMTP_PASSWORD` | `api-server` | Se enabled | — | Senha SMTP |
| `SMTP_FROM_ADDRESS` | `api-server` | Se enabled | `SatsPay <no-reply@…>` | Remetente (deve casar com mailbox autorizada) |

---

## 6. Limiares de aprovação de saque

Valores em **unidade mínima** da moeda. Se a env estiver vazia/ausente, vale o default em `shared::coin_config` (~USD 1 000).

| Variável | Default (atomic) | Aprox. humano |
|---|---:|---|
| `WITHDRAWAL_APPROVAL_THRESHOLD_BTC` | `1500000` | 0.015 BTC |
| `WITHDRAWAL_APPROVAL_THRESHOLD_LTC` | `1000000000` | 10 LTC |
| `WITHDRAWAL_APPROVAL_THRESHOLD_DOGE` | `600000000000` | 6 000 DOGE |
| `WITHDRAWAL_APPROVAL_THRESHOLD_BCH` | `250000000` | 2.5 BCH |
| `WITHDRAWAL_APPROVAL_THRESHOLD_POL` | `250000000000` | 2 500 POL |
| `WITHDRAWAL_APPROVAL_THRESHOLD_DGB` | `12000000000000` | 120 000 DGB |
| `WITHDRAWAL_APPROVAL_THRESHOLD_SOL` | `600000000` | 6 SOL |
| `WITHDRAWAL_APPROVAL_THRESHOLD_USDT` | `100000000000` | 1 000 USDT |
| `WITHDRAWAL_APPROVAL_THRESHOLD_USDC` | `100000000000` | 1 000 USDC |

Saques `>=` limiar ficam `PENDING` (`requires_approval=true`) até `POST /v1/admin/withdrawals/:id/approve`.

