# Referência da API HTTP REST

A API HTTP do **BitcoSats** é implementada em Rust com o framework **Axum** (`crates/api-http/`). Todas as respostas seguem o formato JSON e utilizam códigos HTTP semânticos.

## 0. Convenções que valem para toda a API

**Quantias são inteiros.** Todo campo `amount` desta API é um inteiro em unidades de
ledger de `1e-8`, em qualquer moeda. `25 USDT` é `"2500000000"`, nunca `"25.00"`.
A única exceção é `amountUsd` no gateway de cobranças, que é **fiat decimal** — e por
isso está marcado como tal onde aparece. Quantias viajam como **string**: `2500000000`
cabe num double, mas `9 007 199 254 740 993` não, e nenhuma API de dinheiro deve
depender de o cliente ter notado a diferença.

**Aliases.** Vários endpoints estão montados em mais de um caminho por compatibilidade
com integrações anteriores. Cada família é documentada **uma vez**, no caminho canônico,
com os aliases listados na entrada:

| Canônico | Aliases (mesmo handler, mesmo comportamento) |
|---|---|
| `/v1/api-keys` | `/v1/public/keys`, `/public/keys`, `/api-keys` |
| `/v1/public/send`, `/v1/public/balance` | `/public/send`, `/public/balance` |
| `/v1/public/pay/*` | `/public/pay/*` |
| `/v1/merchant/deposits` | `/v1/merchant/deposits/create`, `/v1/merchant/invoices` |
| `/v1/faucet/*`, `/v1/faucetlist/*` | `/faucet/*`, `/faucetlist/*` |

**Autenticação.** Três mecanismos, nunca misturados na mesma requisição:
- **Bearer JWT** — sessão de usuário no app. Cookie `HttpOnly` no navegador.
- **`x-api-key`** — chave de servidor, com escopos. Ver §12.
- **HMAC assinado** — chave com `requireSignature`, descrito em
  [`public-api-hmac.md`](public-api-hmac.md).

Rotas marcadas **ADMIN** exigem JWT cujo usuário tenha `role = "ADMIN"`; para um usuário
comum elas respondem `403`, não `404`.

**Erros.** Corpo JSON com `error` (mensagem legível) e, nas superfícies novas, `code`
(constante estável — `AMOUNT_NOT_INTEGER`, `COIN_LOCKED`, `DUPLICATE_ORDER_ID`…).
Integre contra o `code`, nunca contra o texto.

---

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

### `GET /v1/auth/security-logs`
- **Autenticação**: Bearer JWT
- **Descrição**: Histórico de eventos de segurança da própria conta (logins, trocas de
  senha, emissão de chaves), com IP e user-agent. Só os do usuário autenticado.

### `POST /v1/auth/admin/login`
- **Autenticação**: Pública, mas só conclui para usuário com `role = "ADMIN"`.
- **Descrição**: Sessão do painel administrativo, separada da sessão do app. Um JWT de
  usuário comum não vale no painel e vice-versa.

### `POST /v1/auth/admin/logout`
- **Autenticação**: Bearer JWT (admin)
- **Descrição**: Encerra a sessão administrativa.

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
- **Carteira de origem**: sempre a `PERSONAL`. O campo opcional `walletKind`
  (alias `kind`) só aceita `"PERSONAL"`; `"MERCHANT"` devolve
  `403 { "error", "code": "WITHDRAWAL_MERCHANT_BLOCKED", "coin" }` sem debitar
  nada, e qualquer outro valor devolve `400`. O caixa do comerciante precisa ser
  movido com `POST /v1/wallet/transfer` (`toDeveloper: false`) antes de sair
  on-chain — envio em blockchain é irreversível.

### `GET /v1/deposits/history`
- **Autenticação**: Bearer JWT
- **Descrição**: Depósitos on-chain detectados para o usuário, com moeda, quantia,
  confirmações e hash. Só os da própria conta.

### `GET /v1/withdrawals`
- **Autenticação**: Bearer JWT
- **Descrição**: Saques do usuário, incluindo os que aguardam aprovação manual.

### `GET /v1/withdrawals/history`
- **Autenticação**: Bearer JWT
- **Descrição**: Histórico de saques do usuário autenticado.

### `GET /v1/withdrawals/addresses`
- **Autenticação**: Bearer JWT
- **Descrição**: Agenda de endereços de saque (servidor; sem localStorage).

### `POST /v1/withdrawals/addresses`
- **Autenticação**: Bearer JWT
- **Body**: `{ "coin": "BTC", "label": "cold", "address": "bc1…" }`
- **Descrição**: Salva/atualiza um endereço na agenda.

### `DELETE /v1/withdrawals/addresses/:id`
- **Autenticação**: Bearer JWT
- **Descrição**: Remove um endereço da agenda.

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

### `POST /v1/swap/quote`
- **Autenticação**: Bearer JWT
- **Descrição**: Mesma cotação do `GET`, aceitando o par no corpo. Uma cotação tem
  validade curta; executar fora dela recalcula.

### `POST /v1/swap/execute`
- **Autenticação**: Bearer JWT
- **Descrição**: Alias de `POST /v1/swap` — executa a conversão na cotação vigente.

### `GET /v1/swap/history`
- **Autenticação**: Bearer JWT
- **Descrição**: Conversões do usuário, com par, quantias de entrada e saída e taxa aplicada.
- **Campo `error`**: presente quando `status` é `FAILED` ou `REFUNDED`. Vem direto de
  `dex_swaps.error` — texto interno (status do provedor, erro de RPC), não uma mensagem
  pronta para usuário final; o client mapeia para um motivo curto antes de exibir.
  Reembolso sempre devolve o valor ao ledger do usuário automaticamente; `error` é só
  contexto de diagnóstico.

### `GET /v1/swap/orders/:id`
- **Autenticação**: Bearer JWT. Ordem de outro usuário → `403`.
- **Descrição**: Estado de uma conversão específica.

### `GET /v1/swap/telemetry`
- **Autenticação**: Bearer JWT
- **Descrição**: Saúde do motor de câmbio — idade das cotações em cache e disponibilidade
  das fontes de preço. Nenhuma cotação obsoleta é usada para converter dinheiro: preço
  velho falha fechado.

---

## 6. Faucet e Diretório Faucetlist (`/v1/faucet` e `/v1/faucetlist`)

### `GET /v1/faucet/status`
- **Autenticação**: Bearer JWT
- **Descrição**: Relógio de 11h por moeda, lido de `faucet_claims`. Alias `/faucet/status`.
  `{ "cooldownMinutes": 660, "coins": [{ "coin": "BTC", "nextClaimAt": null }] }`. `nextClaimAt` null = livre.

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

### `POST /v1/faucet/claim`
- **Autenticação**: Bearer JWT
- **Body**: `{"coin": "BTC", "captchaToken": "…"}`
- **Descrição**: Mesma operação de `/claim/:coin`, com a moeda no corpo.

### `POST /v1/faucetlist`
- **Autenticação**: Bearer JWT
- **Descrição**: Submete um site ao diretório. Entra como pendente — quem aprova é a
  curadoria, em `/v1/admin/faucetlist`.

### `GET /v1/faucetlist/mine`
- **Autenticação**: Bearer JWT
- **Descrição**: Sites submetidos pelo próprio usuário, incluindo os ainda não aprovados.

### `DELETE /v1/faucetlist/:id`
- **Autenticação**: Bearer JWT (dono do site). De outro dono → `403`.
- **Descrição**: Remove o próprio site do diretório.

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

### `GET /v1/stake/strategies`
- **Autenticação**: Bearer JWT
- **Descrição**: Prazos e taxas disponíveis para novos contratos (catálogo, não posições).

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

### `POST /v1/merchant/deposits` — criar fatura do gateway
- **Autenticação**: `x-api-key` (escopo `deposits`), requisição assinada HMAC, ou Bearer JWT (painel).
- **Aliases**: `POST /v1/merchant/deposits/create`, `POST /v1/merchant/invoices`.
- **Body**: `coin`, `amount`, `orderId`, `callbackUrl` obrigatórios; `siteUserId`, `siteName`,
  `successUrl`, `cancelUrl`, `customerEmail`, `customerName`, `description`, `expiryMinutes` opcionais.
- **`amount`**: inteiro em unidades de ledger (1e-8), como `/v1/public/send`. Valor com ponto/vírgula
  → `400 AMOUNT_NOT_INTEGER`; valor que zera on-chain → `400 AMOUNT_BELOW_MINIMUM`.
- **Idempotência**: `orderId` é único por comerciante. Repetir a mesma cobrança devolve `200` com a
  fatura original; mesmo `orderId` com outro valor → `409 DUPLICATE_ORDER_ID`.
- **Duas formas de precificar, nunca as duas juntas:**
  - `coin` + `amount` → fatura travada naquela moeda (contrato original, inalterado).
  - `amountUsd` (decimal, ex. `"25.00"`) + `acceptedCoins` opcional → o cliente escolhe a moeda
    no checkout. Sem `acceptedCoins`, usa a configuração do comerciante.
  - As duas juntas → `400 AMBIGUOUS_AMOUNT`; nenhuma → `400 INVALID_AMOUNT`.
  - `amount` é **inteiro** em unidades de 1e-8; `amountUsd` é **decimal**, porque é fiat.
- **Taxa**: `GATEWAY_FEE_BPS` = 25 (**0,25%**), única para todos os comerciantes, truncada
  para unidades inteiras a favor do comerciante — `feeAmount + netAmount == amount` exato.
- **Resposta `201`**: `id`, `status`, `coin`, `amount`, `feeAmount` (0,25%), `netAmount`,
  `depositAddress`, `payUrl` (relativo), `checkoutUrl` (absoluto, via `PUBLIC_BASE_URL`),
  `qrCode` (endereço `0x` cru para POL/USDT/USDC/PEPE; BIP21 para UTXO — não use `pol:` nem `ethereum:`), `orderId`, `expiresAt`, `createdAt`.
- **Pausa**: BTC/LTC/DOGE/BCH/DGB → `503 DEPOSIT_PAUSED` (`shared::DEPOSIT_WITHDRAW_PAUSED_COINS`).

### `GET /v1/merchant/deposits` / `GET /v1/merchant/deposits/:id`
- **Autenticação**: igual à criação. `:id` de outro comerciante → `403 INVOICE_FORBIDDEN`.
- Status: `PENDING → DETECTED → CONFIRMED | EXPIRED | CANCELLED` (não existe `PAID`).
- **Pagamento a mais** (comum: cliente arredonda, exchange manda além): confirma normalmente
  e envia o webhook. O comerciante recebe o `netAmount` **da fatura**, não o que chegou;
  `receivedAmount` mostra o total. O excedente não é creditado automaticamente — é
  devolvido pelo suporte, com o `id` da fatura.
- **Pagamento a menos**: **não confirma** e não envia webhook. Fica `DETECTED` com o parcial em
  `receivedAmount`. Entradas no mesmo endereço **somam**: completar antes de `expiresAt`
  confirma. Expirou incompleta → `EXPIRED`, parcial não creditado, devolução pelo suporte.
- A regra é por soma de entradas com o mínimo de confirmações da moeda
  (`coin_config(coin).min_confirmations`), comparada ao valor travado **da moeda paga**.

### `GET` / `PUT /v1/merchant/settings`
- **Autenticação**: igual à criação de fatura (escopo `deposits`).
- `GET` devolve `acceptedCoins` (resolvido) e `availableCoins` (todas as ativas).
- `PUT` recebe `{ "acceptedCoins": ["USDT","POL"] }`. Lista vazia guardada significa
  **todas as ativas** — assim uma moeda que sai da pausa passa a ser oferecida sozinha,
  sem ninguém editar nada. Moeda pausada é filtrada na leitura **e** na escrita.
- Seleção sem nenhuma moeda ativa → `400 NO_USABLE_COIN`.

### `POST /v1/public/pay/:id/select-coin`
- **Sem autenticação** — quem paga é um desconhecido com um link, não uma conta.
  Tudo que poderia ser abusado é limitado pela própria fatura:
  - a moeda tem de estar no `accepted_coins` **da fatura** (o cliente não amplia a lista)
    e não pode estar pausada → `400 COIN_NOT_ACCEPTED`;
  - endereços são únicos por `(fatura, moeda)`, então trocar de moeda ida e volta reusa
    linhas em vez de queimar índices HD — o pior caso por fatura é um endereço por moeda aceita;
  - com pagamento em andamento → `409 COIN_LOCKED`;
  - sem cotação fresca → `503 PRICE_UNAVAILABLE` (nunca cotamos com preço velho);
  - o payload não expõe nada de outro comerciante nem de outra fatura.
- **Cotação**: trava no instante da escolha e vale até `expiresAt`. Da trava até o pagamento
  chegar, a variação de preço é do **comerciante** — a plataforma não absorve. A conversão
  USD → unidades arredonda **para cima**, a favor do comerciante.
- **Endereço abandonado continua valendo**: se o cliente viu BTC, mandou, e depois trocou para
  POL, o watcher honra o pagamento em BTC e a fatura passa a apontar para BTC.

### `GET /v1/merchant/webhook-signing-secret`
- **Autenticação**: Bearer JWT.
- **Descrição**: devolve a chave HMAC do comerciante (derivada de `ENCRYPTION_KEY`), o header
  (`X-SatsPay-Signature`), o formato (`sha256=<hex>`), o evento (`deposit.confirmed`), a janela
  anti-replay e o número máximo de tentativas.

### `POST /v1/merchant/deposits/:id/test-webhook`
- **Autenticação**: Bearer JWT (dono da fatura). Dispara uma entrega de teste.

### `GET /v1/me/export` e `POST /v1/me/erase`
- Sessão JWT. Export devolve perfil, saldos (SUM do ledger), faturas sem `callbackUrl` e prefixos de key.
- Erase exige `confirmEmail` + `confirm: "APAGAR"`. Anonimiza e-mail/username, revoga sessões e keys. **Não** apaga `ledger_entries` (LGPD art. 16).

### `GET /v1/public/pay/:id` e `POST /v1/public/pay/:id/balance`
- Checkout público (sem `callbackUrl`/`siteUserId` na resposta) e pagamento com saldo SatsPay.
- Rate class `public-pay` por IP: GET 60/min, `POST .../select-coin` 20/min, `POST .../balance` 10/min
  (aliases sem `/v1` iguais). Excesso → `429` `RATE_LIMITED`.
- `amount` é o inteiro de ledger; `amountDisplay` é a mesma quantia em unidades da moeda,
  `qrCode` de EVM é só o endereço. UTXO usa BIP21 com a quantia em moedas, não o inteiro de ledger.
- `POST .../balance` debita a carteira **pessoal** de quem está logado e credita a carteira
  **comerciante** do dono da fatura. Se forem a mesma conta, `400 CANNOT_PAY_OWN_INVOICE` e
  nada é debitado. O checkout é para outro cliente. Mover o próprio dinheiro é a transferência
  entre carteiras da conta logada (`POST /v1/wallet/transfer`), não esta rota.

### `GET /v1/public/pay/demo`
- Fatura sintética para demonstração: sem linha no banco, sem dinheiro, sem webhook.
  Oferece as moedas que **aquele** comerciante aceita quando há sessão, e todas as ativas
  quando não há — ela existe para demonstrar o seletor, então esconder o seletor a
  descaracteriza.
  Responde `demo: true`, e o checkout usa isso para rotular a página e esconder o
  pagamento por saldo. As rotas `/demo` e `/merchant/demo` redirecionam para `/pay/demo`
  (antes redirecionavam para a página de depósitos do próprio usuário).

### `POST /v1/public/pay/demo/select-coin`
- **Sem autenticação.** Corpo `{ "coin": "POL" }`, mesma resposta do checkout.
- **Não escreve nada**: não grava linha, não consome índice HD e não cota dinheiro real.
  Os endereços demonstrativos são propositalmente **inválidos** em suas redes (todos contêm
  `-DEMO-`), para que ninguém consiga enviar moeda de verdade para a página de exemplo.

### `GET /v1/public/coins`
- **Sem autenticação** — pensado para rodar no navegador do cliente.
- Devolve `priceDecimals`, `amountDecimals` e, por moeda: `symbol`, `name`, `decimals`
  (escala de ledger), `onchainDecimals`, `minConfirmations`, `depositsEnabled`,
  `logoUrl` e `priceUsd` (escalado por `priceDecimals`, cotação de referência).
- Os ícones são servidos por este domínio em `/sdk/coins/<símbolo>.svg`
  (`client/public/sdk/coins/`, vendorizados de cryptocurrency-icons/MIT) com
  `Access-Control-Allow-Origin: *`. Antes vinham de CDN de terceiro e não havia
  endpoint nenhum de preço para comerciante — só `/v1/swap/prices`.

### Botão de pagamento — `/sdk/satspay-pay.js`
- Script embutível que renderiza o botão oficial a partir de
  `<div class="satspay-pay" data-checkout_url="…">`. O backend do comerciante cria a
  fatura; o botão só navega até o `checkoutUrl`. **Nenhuma chave vai para o navegador.**
- `data-theme`, `data-size`, `data-shape`, `data-label`, `data-amount`, `data-target`,
  `data-onclick`. Rótulo é inserido como texto (nunca HTML) e URL não-http(s) é recusada.
- SPAs chamam `window.SatsPay.renderButtons()` após injetar a marcação.

### Webhook `deposit.confirmed`
- `POST` na `callbackUrl` com `X-SatsPay-Signature: sha256=<hex HMAC-SHA256 do corpo cru>`,
  `X-SatsPay-Event`, `X-SatsPay-Timestamp`, `X-SatsPay-Delivery`.
- Corpo: `event`, `invoiceId`, `orderId`, `siteUserId`, `coin`, `amount`, `fee`, `netAmount`,
  `txHash`, `status`, `paidAt`, `customerEmail`, `timestamp`, `attempt`.
- Até `webhooks::MAX_WEBHOOK_ATTEMPTS` tentativas com backoff exponencial; o `timestamp` está
  dentro do corpo assinado, para o receptor recusar replay fora de 300s.

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

### `GET /v1/admin/stats`
- **Descrição**: Números agregados da plataforma — usuários, saldos, volume, operações pendentes.

### `GET /v1/admin/economics`
- **Descrição**: Visão econômica: emissão, taxas arrecadadas, passivo do ledger e liquidez.

### `GET /v1/admin/treasury-wallets`
- **Descrição**: Carteiras de tesouraria (`HOUSE`, `LEND_POOL`, hot wallets) com saldo on-chain
  e saldo contábil. Nunca devolve chave privada nem seed — nem em ambiente de desenvolvimento.

### `GET /v1/admin/treasury-health`
- **Descrição**: Reconciliação entre o que o ledger diz que devemos e o que existe on-chain.
  É o alarme de solvência: divergência aqui é `CRITICAL`, não um relatório.

### `GET /v1/admin/withdrawals`
- **Descrição**: Todos os saques, em qualquer estado (o `pending-withdrawals` é o recorte
  que aguarda decisão).

### `GET /v1/admin/audit-logs`
- **Descrição**: Trilha de auditoria das ações administrativas — quem, o quê, quando, de
  qual IP, sobre qual recurso, com qual resultado.

### `POST /v1/admin/rewards/programs/:id/active`
- **Body**: `{"active": false}`
- **Descrição**: Liga ou desliga a emissão de um programa sem apagá-lo.

### Credenciamento de comerciantes

### `GET /v1/admin/users`
- **Query**: `role` (`USER`|`ADMIN`), `q` (e-mail/username/UUID), `include_erased` (bool), `limit` (1–500, default 200)
- **Descrição**: Lista contas da plataforma (sem HOUSE). E-mail revelado para admin via `email_enc` quando PII está selado.

### `GET /v1/admin/merchant/applications`
- **Descrição**: Fila de pedidos de credenciamento aguardando decisão.

### `POST /v1/admin/merchant/:id/approve`
- **Descrição**: Aprova o pedido. A partir daí o usuário pode emitir chave com escopo
  `deposits` e criar cobranças.

### `POST /v1/admin/merchant/:id/reject`
- **Body**: `{"reason": "documentação insuficiente"}`
- **Descrição**: Recusa o pedido, com motivo registrado.

### `GET /v1/admin/merchants` / `GET /v1/admin/merchants/stats`
- **Descrição**: Comerciantes já credenciados e seus números agregados (faturas, volume, taxa).

### `POST /v1/admin/merchants/:id/approve` / `POST /v1/admin/merchants/:id/suspend`
- **Descrição**: Reativa ou suspende um comerciante credenciado. Suspenso não cria faturas novas.

### Curadoria do Faucetlist

### `GET /v1/admin/faucetlist`
- **Descrição**: Sites submetidos, incluindo os pendentes de curadoria.

### `POST /v1/admin/faucetlist/:id/approve`, `/v1/admin/faucetlist/:id/reject`, `/v1/admin/faucetlist/:id/suspend`
- **Descrição**: Publica, recusa ou tira do ar um site do diretório.

### Suporte (visão do operador)

### `GET /v1/admin/support/tickets` / `GET /v1/admin/support/tickets/:id`
- **Descrição**: Todos os tíquetes, de qualquer usuário — a versão administrativa de §14.

### `POST /v1/admin/support/tickets/:id/messages`
- **Body**: `{"body": "texto da resposta"}`
- **Descrição**: Responde ao usuário no tíquete.

### `POST /v1/admin/support/tickets/:id/status`
- **Body**: `{"status": "RESOLVED"}`
- **Descrição**: Move o tíquete de estado (`OPEN`, `PENDING`, `RESOLVED`, `CLOSED`).

### Observabilidade de erros

### `GET /v1/admin/telemetry/overview`
- **Descrição**: Painel de erros: contagem por severidade, taxa de erro, grupos mais
  frequentes, primeira e última ocorrência.

### `GET /v1/admin/telemetry/errors`
- **Descrição**: Ocorrências individuais, filtráveis. Os payloads passam por redação antes
  de serem gravados (`db::telemetry::redact_secrets`): senha, token, chave e seed nunca
  chegam ao banco.

### `POST /v1/admin/telemetry/errors/:id/resolve` e `POST /v1/admin/telemetry/errors/:id/ignore`
- **Descrição**: Move uma ocorrência no ciclo de vida (`NEW → RESOLVED | IGNORED`).

### `POST /v1/admin/telemetry/errors/batch-resolve`
- **Body**: `{"ids": ["…", "…"]}`
- **Descrição**: Resolve um conjunto específico.

### `POST /v1/admin/telemetry/errors/resolve-all` e `POST /v1/admin/telemetry/errors/clear`
- **Descrição**: Resolve tudo que está aberto; `clear` apaga o histórico. Operação
  destrutiva e auditada — `clear` descarta evidência, não apenas ruído de tela.

### `GET /v1/admin/telemetry/metrics-history`
- **Descrição**: Série temporal das métricas operacionais, para correlacionar pico de erro
  com deploy.

### `POST /v1/admin/telemetry/test-error`
- **Descrição**: Injeta um erro sintético para verificar ponta a ponta coleta, agrupamento
  e alerta. Existe para que o caminho de erro não seja testado pela primeira vez durante
  um incidente.

---

## 12. Chaves de API (`/v1/api-keys`)

*Aliases: `/v1/public/keys`, `/public/keys`, `/api-keys` — mesmo handler.*

A chave é a credencial de **servidor**. Nunca colocá-la no navegador: para pagamentos, o
botão oficial recebe apenas o `checkoutUrl` que o seu backend já criou.

### `POST /v1/api-keys`
- **Autenticação**: Bearer JWT (a chave é emitida por uma sessão de usuário, não por outra chave).
- **Body**:
  ```json
  {
    "label": "loja-producao",
    "scopes": ["deposits"],
    "allowedIps": ["203.0.113.10"],
    "expiresInDays": 365,
    "requireSignature": true
  }
  ```
- **Escopos efetivamente verificados**: `deposits` (gateway de cobranças), `send`
  (`/v1/public/send`) e `*` (curinga, concede tudo). Um escopo fora dessa lista é
  aceito e guardado, mas não habilita nada.
- **`allowedIps`**: vazio = qualquer origem. Preenchido, a chave só vale a partir daqueles IPs.
- **`requireSignature`**: exige HMAC em cada requisição — ver [`public-api-hmac.md`](public-api-hmac.md).
- **Resposta**: a chave em claro aparece **uma única vez**, nesta resposta. Não há como
  recuperá-la depois; perdida, rotacione.

### `GET /v1/api-keys`
- **Autenticação**: Bearer JWT
- **Descrição**: Chaves do usuário com `keyPrefix`, escopos, allowlist, expiração, último uso
  e data de desativação. **O segredo não é devolvido.**

### `POST /v1/api-keys/:id/rotate`
- **Autenticação**: Bearer JWT
- **Descrição**: Gera um segredo novo mantendo id, escopos, allowlist, expiração e política
  de assinatura. O segredo anterior para de valer imediatamente.

### `DELETE /v1/api-keys/:id`
- **Autenticação**: Bearer JWT
- **Descrição**: Desativa a chave. A linha continua existindo para a auditoria não perder o
  rastro de quem usou o quê.

### `POST /v1/public/send`
- **Autenticação**: `x-api-key` com escopo `send`; HMAC se a chave exigir.
- **Body**: `{"coin": "USDT", "amount": "2500000000", "toEmail": "destinatario@example.com", "idempotencyKey": "…"}`
- **`toEmail`**: e-mail da conta SatsPay que **recebe**. Dois jeitos, o mesmo campo: o usuário digita o e-mail da conta dele, ou entra com SatsPay e você manda o `email` verificado de `GET /v1/oauth/userinfo`. Não é endereço on-chain.
- **Resposta de erro**: sempre `{ "error": "…", "code": "…" }`. Trate pelo `code`.
  - `400 TARGET_INELIGIBLE` — não existe conta com esse e-mail. Nada debitado.
  - `400 SEND_TO_SELF` — `toEmail` é a conta que emitiu a chave. Não é falta de saldo. Nada debitado. Use outra conta.
  - `400 DAILY_LIMIT_REACHED` — limite diário da chave. Nada debitado.
  - `400 WALLET_NOT_FOUND` — sem carteira dessa moeda no remetente ou no destinatário. Nada debitado.
- **Descrição**: Transfere da carteira do dono da chave para outro usuário **pela plataforma**
  (não é saque on-chain). `idempotencyKey` é obrigatório: repetir a mesma chave devolve a
  operação original em vez de enviar de novo. Há limite diário por conta
  (`PUBLIC_API_DAILY_SEND_LIMIT`).

### `GET /v1/public/balance`
- **Autenticação**: `x-api-key` (qualquer chave válida).
- **Descrição**: Saldo do dono da chave, por moeda, em unidades de ledger.
- **Atenção**: este endpoint **não verifica escopo**. Uma chave emitida só com `deposits`
  lê o saldo completo da conta. Enquanto isso não mudar, trate toda chave como capaz de
  ler saldo e prefira contas separadas para integrações de terceiros.

---

## 13. OAuth 2.0 e OpenID Connect (`/v1/oauth`)

Fluxo *authorization code* com **PKCE obrigatório**. Serve para outro produto ("Entrar com
BitcoSats") — não é o caminho do gateway de cobranças, que usa chave de API.

### `GET /.well-known/openid-configuration`
- **Autenticação**: Pública
- **Descrição**: Documento de discovery OIDC — emissor, endpoints, algoritmos e escopos
  suportados. É daqui que uma biblioteca OIDC se configura sozinha.

### `GET /v1/oauth/authorize/info`
- **Autenticação**: Bearer JWT opcional.
- **Query**: `client_id`, `redirect_uri`, `scope`, `state`, `code_challenge`,
  `code_challenge_method=S256`.
- **Descrição**: Dados para desenhar a tela de consentimento (nome do app, escopos pedidos)
  e validação antecipada dos parâmetros. Sem sessão, indica que é preciso autenticar antes.

### `POST /v1/oauth/authorize`
- **Autenticação**: Bearer JWT (o usuário que está consentindo).
- **Descrição**: Registra o consentimento e devolve o `code` para o `redirect_uri`.
  `redirect_uri` precisa bater **exatamente** com um dos cadastrados no app — prefixo não
  basta, e é essa checagem que impede o código de vazar para um domínio do atacante.
  O `state` volta intacto, para o cliente detectar CSRF.

### `POST /v1/oauth/token`
- **Autenticação**: Pública (autentica pelo `client_id`/`client_secret` ou PKCE).
- **Body**: `grant_type=authorization_code` com `code`, `redirect_uri`, `client_id`,
  `code_verifier`; ou `grant_type=refresh_token` com `refresh_token`.
- **Descrição**: Troca o código por `access_token` (+ `refresh_token`, + `id_token` quando
  o escopo `openid` foi pedido). O código é de uso único e expira em minutos.

### `GET /v1/oauth/userinfo`
- **Autenticação**: `Authorization: Bearer <access_token>` do OAuth.
- **Descrição**: Reivindicações do usuário conforme os escopos concedidos. Devolve apenas
  o que foi consentido.

### `GET` / `POST /v1/oauth/apps`
- **Autenticação**: Bearer JWT
- **Descrição**: Aplicações OAuth do próprio desenvolvedor. Na criação, o
  `client_secret` aparece **uma única vez**.

### `PUT` / `DELETE /v1/oauth/apps/:id`
- **Autenticação**: Bearer JWT (dono do app). De outro dono → `403`.
- **Descrição**: Atualiza nome, logo e `redirect_uris`; ou remove o app.

### `POST /v1/oauth/apps/:id/rotate-secret`
- **Autenticação**: Bearer JWT (dono do app).
- **Descrição**: Novo `client_secret`; o anterior deixa de valer na hora.

### `GET /v1/oauth/authorized-apps`
- **Autenticação**: Bearer JWT
- **Descrição**: Aplicações às quais **este usuário** concedeu acesso — o outro lado da mesa.

### `DELETE /v1/oauth/authorized-apps/:id`
- **Autenticação**: Bearer JWT
- **Descrição**: Revoga o consentimento e invalida os tokens daquele app.

---

## 14. Suporte (`/v1/support`)

### `POST /v1/support/tickets`
- **Autenticação**: Bearer JWT
- **Body**: `{"subject": "…", "body": "…", "category": "…"}`
- **Descrição**: Abre um tíquete.

### `GET /v1/support/tickets` / `GET /v1/support/tickets/:id`
- **Autenticação**: Bearer JWT. Tíquete de outro usuário → `403`.
- **Descrição**: Tíquetes do próprio usuário e o histórico de mensagens de um deles.

### `POST /v1/support/tickets/:id/messages`
- **Autenticação**: Bearer JWT (autor do tíquete).
- **Body**: `{"body": "texto"}`
- **Descrição**: Acrescenta uma mensagem à conversa.

---

## 15. Indicação e Airdrop

### `GET /v1/referral/stats`
- **Autenticação**: Bearer JWT
- **Descrição**: Código de indicação do usuário, total de indicados e comissão acumulada.

### `GET /v1/referral/users` (alias `GET /v1/referral/list`)
- **Autenticação**: Bearer JWT
- **Descrição**: Quem este usuário indicou. Devolve identificação mínima do indicado — a
  conta de terceiro não é exposta a quem indicou.

### `GET /v1/referral/commissions`
- **Autenticação**: Bearer JWT
- **Descrição**: Comissões creditadas, com origem e data.

### `GET /v1/airdrop/overview`, `GET /v1/airdrop/profile`, `GET /v1/airdrop/history`, `GET /v1/airdrop/logs`
- **Autenticação**: Bearer JWT
- **Descrição**: Pontuação do usuário na campanha, perfil de participação, créditos já
  recebidos e trilha das ações pontuadas — todos restritos à própria conta.

### `GET /v1/airdrop/leaderboard`
- **Autenticação**: Bearer JWT (exige sessão, mas o ranking é o mesmo para todos).
- **Descrição**: Classificação por pontos, identificando participantes por apelido.

---

## 16. Status e Telemetria de Cliente

### `GET /v1/status/nodes`
- **Autenticação**: Pública
- **Descrição**: Latência e disponibilidade dos nós de cada rede que o gateway depende.
  Alimenta a página de status: se o nó da rede está fora, é aqui que aparece antes de
  virar fatura não confirmada.

### `POST /v1/telemetry/client-error`
- **Autenticação**: Pública (com limite de taxa).
- **Body**: `{"message": "…", "stack": "…", "url": "…", "userAgent": "…"}`
- **Descrição**: Erro de JavaScript do navegador, para o mesmo painel dos erros de backend.
  O conteúdo passa por redação antes de ser gravado, e estas rotas são excluídas da
  própria coleta de erros HTTP — senão uma falha na coleta se realimentaria.

### `POST /v1/telemetry/client-errors`
- **Autenticação**: Pública (com limite de taxa).
- **Descrição**: Mesma coisa em lote, para o buffer da SPA descarregar de uma vez.
