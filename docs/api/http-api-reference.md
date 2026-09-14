# Referência da API HTTP REST

A API HTTP do **BitcoSats** é implementada em Rust com o framework **Axum** (`crates/api-http/`). Todas as respostas seguem o formato JSON e utilizam códigos HTTP semânticos.

---

## 1. Endpoints de Sistema e Saúde

### `GET /healthz`
- **Autenticação**: Pública
- **Descrição**: Checagem de disponibilidade do serviço (Liveness/Readiness probe).
- **Resposta**: `200 OK` (`"ok"`)

### `GET /metrics`
- **Autenticação**: Pública (uso interno por Prometheus)
- **Descrição**: Métricas operacionais em formato OpenMetrics / Prometheus.

---

## 2. Autenticação e Usuários (`/v1/auth`)

### `POST /v1/auth/register`
- **Autenticação**: Pública
- **Body**:
  ```json
  {
    "email": "user@example.com",
    "username": "satoshi",
    "password": "StrongPassword123!",
    "confirmPassword": "StrongPassword123!",
    "acceptTerms": true
  }
  ```
- **Resposta `201 Created`**:
  ```json
  {
    "id": "uuid",
    "email": "user@example.com",
    "username": "satoshi",
    "role": "USER"
  }
  ```

### `POST /v1/auth/login`
- **Autenticação**: Pública
- **Body**:
  ```json
  {
    "email": "user@example.com",
    "password": "StrongPassword123!"
  }
  ```
- **Resposta `200 OK`**:
  ```json
  {
    "accessToken": "eyJhbGciOi...",
    "refreshToken": "raw_refresh_token_value",
    "user": {
      "id": "uuid",
      "email": "user@example.com",
      "role": "USER",
      "twoFactorEnabled": false
    }
  }
  ```
  *(Define também o cookie `refresh_token` HttpOnly)*.

### `POST /v1/auth/refresh`
- **Autenticação**: Cookie `refresh_token` ou body `{"refreshToken": "..."}`
- **Descrição**: Rotação do refresh token com detecção de reuso.
- **Resposta `200 OK`**: Novos pares de `accessToken` e `refreshToken`.

### `POST /v1/auth/logout`
- **Autenticação**: Bearer JWT ou Cookie de Refresh
- **Descrição**: Revoga a sessão e o refresh token atual.

### `GET /v1/auth/me`
- **Autenticação**: Bearer JWT
- **Descrição**: Retorna o perfil do usuário autenticado.

### `PATCH /v1/auth/username`
- **Autenticação**: Bearer JWT
- **Body**: `{"username": "novo_nome"}`
- **Descrição**: Altera o display username do usuário.

---

## 3. Carteiras e Razão (`/v1/wallet`)

### `GET /v1/wallet`
- **Autenticação**: Bearer JWT
- **Descrição**: Lista todas as carteiras do usuário com seus saldos derivados do ledger.
- **Resposta `200 OK`**:
  ```json
  [
    {
      "coin": "BTC",
      "address": "bc1q...",
      "balance": "150000",
      "kind": "PERSONAL"
    }
  ]
  ```

### `POST /v1/wallet/transfer`
- **Autenticação**: Bearer JWT
- **Body**:
  ```json
  {
    "toEmail": "destinatario@example.com",
    "coin": "BTC",
    "amount": "10000",
    "memo": "Pagamento de almoço"
  }
  ```
- **Descrição**: Transferência instantânea e interna entre usuários da plataforma (sem taxa de rede).

### `GET /v1/wallet/ledger`
- **Autenticação**: Bearer JWT
- **Query Params**: `?coin=BTC&limit=50&offset=0`
- **Descrição**: Extrato detalhado de todos os lançamentos contábeis da carteira.

---

## 4. Depósitos e Saques (`/v1/deposits` e `/v1/withdrawals`)

### `GET /v1/deposits/address/:coin`
- **Autenticação**: Bearer JWT
- **Descrição**: Obtém ou gera o endereço determinístico de depósito para a moeda solicitada.

### `POST /v1/withdrawals`
- **Autenticação**: Bearer JWT (Exige código 2FA/OTP se habilitado)
- **Body**:
  ```json
  {
    "coin": "BTC",
    "toAddress": "bc1q...",
    "amount": "50000",
    "emailCode": "123456",
    "idempotencyKey": "unique-client-uuid"
  }
  ```
- **Resposta `200 OK`**:
  ```json
  {
    "id": "uuid",
    "status": "PENDING",
    "requiresApproval": false
  }
  ```

---

## 5. Câmbio Instantâneo (`/v1/swap`)

### `GET /v1/swap/prices`
- **Autenticação**: Bearer JWT
- **Descrição**: Cotações de mercado atuais de todas as moedas.

### `GET /v1/swap/quote`
- **Autenticação**: Bearer JWT
- **Query Params**: `?fromCoin=BTC&toCoin=LTC&fromAmount=10000`
- **Descrição**: Simulação de conversão com cálculo de taxa de corretagem (*spread*).

### `POST /v1/swap`
- **Autenticação**: Bearer JWT
- **Body**:
  ```json
  {
    "fromCoin": "BTC",
    "toCoin": "LTC",
    "fromAmount": "10000",
    "idempotencyKey": "swap-uuid"
  }
  ```
- **Descrição**: Executa a conversão atômica debitando da carteira do usuário e creditando via liquidez da carteira `HOUSE`.

---

## 6. Faucet e Diretório Faucetlist (`/v1/faucet` e `/v1/faucetlist`)

### `POST /v1/faucet/claim/:coin`
- **Autenticação**: Bearer JWT
- **Body**: `{"captchaToken": "token_cloudflare_turnstile"}`
- **Descrição**: Resgata recompensa periódica do faucet para a moeda especificada.

### `GET /v1/faucetlist`
- **Autenticação**: Pública
- **Descrição**: Lista de sites parceiros de faucet aprovados pela curadoria.

### `POST /v1/faucetlist/click/:id`
- **Autenticação**: Pública
- **Descrição**: Registra clique e redirecionamento para o site parceiro.

---

## 7. Rendimento e Contratos de Staking (`/v1/stake`)

### `GET /v1/stake`
- **Autenticação**: Bearer JWT
- **Descrição**: Lista todos os contratos de staking ativos ou finalizados do usuário.

### `POST /v1/stake`
- **Autenticação**: Bearer JWT
- **Body**:
  ```json
  {
    "coin": "BTC",
    "principal": "100000",
    "lockDays": 30
  }
  ```

### `POST /v1/stake/:id/claim`
- **Autenticação**: Bearer JWT
- **Descrição**: Resgata o principal + rendimento após a data de maturação.

### `POST /v1/stake/:id/cancel`
- **Autenticação**: Bearer JWT
- **Descrição**: Cancela um contrato de staking antes do prazo (sem rendimento).

---

## 8. Mercado Monetário e Empréstimos (`/v1/lend`)

### `GET /v1/lend/markets`
- **Autenticação**: Bearer JWT
- **Descrição**: Estatísticas de liquidez, taxas de depósito (APY) e taxas de empréstimo (Borrow APY) de cada reserva.

### `GET /v1/lend/positions`
- **Autenticação**: Bearer JWT
- **Descrição**: Posições de depósito colateralizado, empréstimos ativos e índice de saúde financeira (Health Factor) do usuário.

### `POST /v1/lend/supply/:coin`
- **Autenticação**: Bearer JWT
- **Body**: `{"amount": "50000"}`
- **Descrição**: Fornece liquidez ao mercado de empréstimos.

### `POST /v1/lend/withdraw/:coin`
- **Autenticação**: Bearer JWT
- **Body**: `{"amount": "50000"}`
- **Descrição**: Retira liquidez fornecida (desde que não comprometa a saúde dos empréstimos ativos).

### `POST /v1/lend/borrow/:coin`
- **Autenticação**: Bearer JWT
- **Body**: `{"amount": "20000"}`
- **Descrição**: Toma empréstimo utilizando as posições de supply como colateral.

### `POST /v1/lend/repay/:coin`
- **Autenticação**: Bearer JWT
- **Body**: `{"amount": "20000"}`
- **Descrição**: Amortiza ou quita a dívida de empréstimo.

---

## 9. Recompensas de Mineração de Liquidez (`/v1/rewards`)

### `GET /v1/rewards`
- **Autenticação**: Bearer JWT
- **Descrição**: Retorna o total de recompensas acumuladas e recebidas pelo usuário em programas de incentivo.

---

## 10. Módulo Merchant (`/v1/merchant`)

### `GET /v1/merchant/status`
- **Autenticação**: Bearer JWT
- **Descrição**: Consulta o status de credenciamento do usuário como comerciante.

### `POST /v1/merchant/apply`
- **Autenticação**: Bearer JWT
- **Body**:
  ```json
  {
    "businessName": "Loja Cripto",
    "website": "https://lojacripto.com",
    "description": "Comércio eletrônico aceitando pagamentos"
  }
  ```

---

## 11. Painel Administrativo (`/v1/admin`)

*Todas as rotas exigem usuário com `role = "ADMIN"`.*

### `GET /v1/admin/pending-withdrawals`
- **Descrição**: Lista saques que aguardam aprovação manual de conformidade.

### `POST /v1/admin/withdrawals/:id/approve`
- **Descrição**: Aprova o saque e libera o envio para a fila de broadcast.

### `POST /v1/admin/withdrawals/:id/reject`
- **Descrição**: Rejeita o saque e estorna os valores no ledger contábil.

### `POST /v1/admin/house/fund`
- **Body**: `{"coin": "BTC", "amount": "1000000"}`
- **Descrição**: Aporta liquidez na carteira de tesouraria `HOUSE`.

### `POST /v1/admin/lend-pool/fund`
- **Body**: `{"coin": "BTC", "amount": "1000000"}`
- **Descrição**: Aporta liquidez na carteira de empréstimos `LEND_POOL`.

### `POST /v1/admin/rewards/programs`
- **Body**:
  ```json
  {
    "rewardCoin": "BTC",
    "marketCoin": "POL",
    "side": "SUPPLY",
    "emissionPerDay": "100000",
    "startAt": "2026-09-01T00:00:00Z"
  }
  ```
- **Descrição**: Cria um novo programa de mineração de liquidez.
