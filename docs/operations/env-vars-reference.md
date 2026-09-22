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
| `ENCRYPTION_KEY` | `api-server`, `worker` | Sim | Hex 64 chars (32 bytes) | Chave mestre AES-256-GCM (PII, API keys, TOTP, HMAC e-mail/IP). Backup offline obrigatório. Rotação: [`vm-security-runbook.md`](vm-security-runbook.md) §5 — sem dual-key, re-selar tudo antes de trocar |
| `PII_BLANK_EMAIL` | `api-server`, `worker` | Não | `true` | Deploy 4: substitui `users.email` por placeholder após HMAC+`email_enc` estarem verdes. HOUSE não é tocado |
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
| `DGB_RPC_URL` | `api-server`, `worker` | Não | `http://user:pass@host:14022` | Node DigiByte próprio (`scantxoutset` / `sendrawtransaction`). Sem valor, cai no Insight. |
| `DGB_INSIGHT_API` | `api-server`, `worker` | Não | `https://digiexplorer.info/api` | Fallback indexer DGB quando o node RPC falha |
| `ZER_RPC_URL` | `api-server`, `worker` | Prod (saque) | `http://user:pass@host:23801` | Node `zerod` (`scantxoutset` / `createrawtransaction` / `signrawtransactionwithkey` / `sendrawtransaction`). Sem URL, depósito cai no explorer; saque falha fechado. |
| `ZER_EXPLORER_API` | `api-server`, `worker` | Não | `https://zerochain.info/api` | Fallback de saldo/txs para ZER |
| `ZER_EXPLORER_API_KEY` | `api-server`, `worker` | Não | — | Key pedida pelos paths públicos `addressinfo` / `txs` do zerochain.info |
| `BSC_RPC_URL` | `api-server`, `worker` | Prod (PEPE) | `https://bsc-rpc.publicnode.com` | JSON-RPC da BNB Smart Chain (chain id 56). Só PEPE. Nunca reusar `EVM_RPC_URL` (Polygon). |
| `BSC_DEPOSIT_LOOKBACK_BLOCKS` | `worker` | Não | `2000` | Blocos BSC varridos no `eth_getLogs` do contrato PEPE (~100 min). |

A hot de PEPE é o mesmo endereço `0x` da hot de POL (`m/44'/60'/0'/0/0`). O gas do `transfer` BEP-20 e do bridge Relay (approve+deposit) é **BNB** nativo nessa carteira — sem saldo de BNB o saque/swap PEPE→* falha fechado (`BNB_GAS_REQUIRED`, mínimo ~0,005 BNB) e o ledger é revertido.

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
| `RELAY_ENABLED` | `api-server`, `worker` | Não | `false` | Liga cotação/execução Relay (`POST /quote/v2`) como 2º provedor Polygon |
| `RELAY_API_KEY` | `api-server`, `worker` | Não | — | API key Relay (opcional) |
| `RELAY_BASE_URL` | `api-server`, `worker` | Não | `https://api.relay.link` | Base URL Relay Protocol |
| `CHANGENOW_ENABLED` | `api-server`, `worker` | Não | `false` | Liga swaps L1 via ChangeNOW (deposit-address) |
| `CHANGENOW_API_KEY` | `api-server`, `worker` | Se habilitado | — | API key Partner ChangeNOW (`x-changenow-api-key`) |
| `CHANGENOW_BASE_URL` | `api-server`, `worker` | Não | `https://api.changenow.io/v2` | Base URL ChangeNOW API v2 |
| `SWAP_PLATFORM_FEE_BPS_SAME` | `api-server`, `worker` | Não | `25` | Taxa SatsPay same-chain (bps) — 0,25% diferencial |
| `SWAP_PLATFORM_FEE_BPS_CROSS` | `api-server`, `worker` | Não | `25` | Taxa SatsPay cross-chain / ChangeNOW / Relay (bps) — 0,25% |
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
| `WITHDRAWAL_APPROVAL_THRESHOLD_ZER` | `10000000000000` | 100 000 ZER |

Saques `>=` limiar ficam `PENDING` (`requires_approval=true`) até `POST /v1/admin/withdrawals/:id/approve`.

---

## 7. Deploy (`scripts/deploy_to_vm.py`)

Sem senha no código. É obrigatório `DEPLOY_SSH_KEY` **ou** `DEPLOY_SSH_PASSWORD` no ambiente do operador. Preferir chave; senha só até o SSH da VM ser key-only ([runbook](vm-security-runbook.md)).

| Variável | Obrigatória? | Exemplo | Descrição |
|---|---|---|---|
| `DEPLOY_SSH_HOST` | Não | `203.0.113.10` | IP/hostname da VM (tem default no script) |
| `DEPLOY_SSH_USER` | Não | `root` | Usuário SSH (default `root`) |
| `DEPLOY_SSH_KEY` | Uma das duas | `~/.ssh/id_ed25519` | Caminho da chave privada (preferido) |
| `DEPLOY_SSH_PASSWORD` | Uma das duas | — | Senha SSH; **nunca** commitar. Rotacionar se já esteve no git |

```bash
export DEPLOY_SSH_HOST=…
export DEPLOY_SSH_KEY=~/.ssh/id_ed25519
python3 scripts/deploy_to_vm.py --check-auth   # só valida env
python3 scripts/deploy_to_vm.py                # client
python3 scripts/deploy_to_vm.py --backend      # + api/worker
```

Rotacionar a senha de root se ela já esteve no repositório. Preferir chave e `PasswordAuthentication no`.

