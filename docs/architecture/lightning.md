# Arquitetura — BTC na Lightning

Companion do **ADR 0011** (`docs/decisions/0011-lightning-btc-custodia-e-isolamento.md`),
que fixa as decisões. Aqui vai o detalhe de implementação: componentes,
schema, contrato do `ln-bridge`, fluxos passo a passo, jobs do worker,
frontend, configuração e threat model.

Nada disto está implementado. Ordem de leitura: ADR 0011 → este doc.

---

## 1. Componentes

```
┌─────────────────────── VM do app (já existe) ────────────────────────┐
│                                                                     │
│  api-server ──┐                          worker ──┐                  │
│  (axum)       │                          (jobs)   │                  │
│              gRPC mTLS (cert: invoicer)  gRPC mTLS (cert: payer)     │
│               │                                   │                  │
│         Postgres (ln_invoices, ln_payments, ln_stream_checkpoints)   │
└───────────────┼───────────────────────────────────┼─────────────────┘
                │        WireGuard wg0 (só)          │
┌───────────────┼───────────────────────────────────┼─────────────────┐
│               ▼                                   ▼                  │
│        ln-bridge  (Rust, stateless)  ── localhost gRPC ──▶  lnd      │
│         · mTLS server, CA interna                          · seed    │
│         · mapeia SAN do cert → conjunto de RPCs            · macaroons│
│         · aplica fee cap / rate limit / thresholds          escopadas│
│                                                          · SCB → S3  │
│  Firewall default-deny · inbound público só 9735/tcp (ou só Tor)     │
│  RPC do lnd, ln-bridge e SSH só em wg0 · sem Postgres na VM          │
└──────────────────────── VM(s) do nó Lightning ──────────────────────┘
```

- **`ln-bridge`** — crate/binário novo neste workspace (`crates/ln-bridge`).
  Stateless. Fala LND por `localhost` (macaroon + tls.cert lidos de disco da
  VM). Expõe o gRPC da §3 pro app. Roda como container/systemd unit **na VM do
  nó**, não no k8s do app.
- **api-server** — ganha rotas `/v1/deposits/ln/*` e `/v1/withdrawals/ln/*`.
  Usa o `ln-bridge` só com o cert `invoicer` (nunca paga).
- **worker** — ganha 3 jobs (§5). O `ln_payment_worker` usa o cert `payer`.
- **Postgres principal** — dono de todo o estado. `ln-bridge` não tem banco.

### Por que o `ln-bridge` e não falar LND direto?

1. O segredo que move **todo** o saldo dos canais (macaroon admin / `offchain:write`)
   nunca precisa existir na VM do app. Se o `api-server` for comprometido, o
   atacante tem, no máximo, o cert `invoicer` — que só cria invoice.
2. Um ponto único onde impor fee cap, rate limit e threshold de aprovação,
   auditável, sem confiar que todo call-site do app lembrou de passar o limite.
3. Superfície de API pequena e estável: trocar LND por CLN no futuro é
   reescrever só o `ln-bridge`.

---

## 2. Modelo de dados (migration nova)

`crates/db/migrations/0010_lightning.sql` (número conforme a próxima folga):

```sql
-- BTC na Lightning. Não há coin nova: estas tabelas creditam/debitam a
-- MESMA wallet (user, 'BTC', 'PERSONAL') via ledger_entries. Ver ADR 0011.

create type ln_invoice_status as enum (
    'PENDING',   -- emitido, aguardando pagamento
    'PAID',      -- nó reportou HTLC settled; ainda não creditado no ledger
    'SETTLED',   -- creditado no ledger (estado terminal feliz)
    'EXPIRED',   -- passou de expires_at sem pagamento
    'CANCELED'   -- cancelado por nós (ex.: hold invoice não resgatado)
);

create type ln_payment_status as enum (
    'PENDING',    -- aceito, valor travado no ledger, job enfileirado
    'PAYING',     -- ln-bridge.PayInvoice em andamento
    'SUCCEEDED',  -- preimage recebido, fee finalizada (terminal)
    'FAILED',     -- falha definitiva, hold revertido (terminal)
    'CANCELED'    -- cancelado antes de PAYING (terminal)
);

create table ln_invoices (
    id                       uuid primary key default gen_random_uuid(),
    wallet_id                uuid not null references wallets (id) on delete cascade,
    payment_hash             bytea not null unique,          -- 32 bytes
    payment_request          text  not null,                 -- BOLT11
    amount_msat              numeric(39, 0),                 -- null = amountless
    amount_paid_msat         numeric(39, 0),                 -- verdade do nó
    status                   ln_invoice_status not null default 'PENDING',
    memo                     text,
    add_index                bigint,                         -- cursor LND (add)
    settle_index             bigint,                         -- cursor LND (settle)
    credited_ledger_entry_id uuid references ledger_entries (id),  -- marca idempotência
    created_at               timestamptz not null default now(),
    expires_at               timestamptz not null,
    settled_at               timestamptz
);
create index idx_ln_invoices_status  on ln_invoices (status);
create index idx_ln_invoices_wallet  on ln_invoices (wallet_id);
create index idx_ln_invoices_pending on ln_invoices (expires_at) where status = 'PENDING';

create table ln_payments (
    id                uuid primary key default gen_random_uuid(),
    wallet_id         uuid not null references wallets (id) on delete cascade,
    payment_hash      bytea not null unique,
    payment_request   text  not null,
    dest_pubkey       bytea,
    amount_msat       numeric(39, 0) not null,
    fee_msat_limit    numeric(39, 0) not null,               -- teto passado ao nó
    fee_msat_paid     numeric(39, 0),                        -- fee real
    preimage          bytea,
    status            ln_payment_status not null default 'PENDING',
    failure_reason    text,
    idempotency_key   text unique,
    requires_approval boolean not null default false,
    approved_by_id    uuid references users (id),
    approved_at       timestamptz,
    requested_ip      text,
    created_at        timestamptz not null default now(),
    updated_at        timestamptz not null default now()
);
create index idx_ln_payments_status on ln_payments (status);
create index idx_ln_payments_wallet on ln_payments (wallet_id);

-- Cursor do stream SubscribeInvoices, pra retomar sem reprocessar.
create table ln_stream_checkpoints (
    stream     text primary key,          -- 'invoices'
    last_index bigint not null default 0, -- último settle_index processado
    updated_at timestamptz not null default now()
);
insert into ln_stream_checkpoints (stream) values ('invoices');

-- Distinguir o trilho nos lançamentos e nos deposits on-chain existentes.
alter type ledger_type add value if not exists 'LN_DEPOSIT';        -- opcional; ver nota
-- (alternativa preferida: manter type = 'DEPOSIT'/'WITHDRAWAL' e distinguir
--  só por reference_type = 'ln_invoice' | 'ln_payment'. Decidir na F0.)
```

**Idempotência do crédito**: `credit_ln_deposit` só escreve se
`credited_ledger_entry_id IS NULL` (checado com a wallet travada). O
`(wallet_id, reference_id, reference_type, type)` do `ledger_entries` já barra
duplicata como segunda linha de defesa (ver `0001_init.sql`).

**Unidade**: `amount_paid_msat` → crédito no ledger de `floor(amount_paid_msat / 1000)`
sat. Se `< 1 sat`, não credita e marca `SETTLED` com `credited_ledger_entry_id`
nulo + log (não deveria acontecer: invoices sempre em sat inteiro).

---

## 3. Contrato do `ln-bridge` (gRPC)

`crates/ln-bridge/proto/ln_bridge.proto` (esboço):

```proto
syntax = "proto3";
package bitcosats.lnbridge.v1;

service LnBridge {
  // --- disponível pro cert "invoicer" e pro "payer" ---
  rpc CreateInvoice     (CreateInvoiceRequest)   returns (Invoice);
  rpc LookupInvoice     (LookupInvoiceRequest)   returns (Invoice);
  rpc SubscribeInvoices (SubscribeRequest)       returns (stream Invoice);
  rpc DecodeInvoice     (DecodeInvoiceRequest)   returns (DecodedInvoice);
  rpc NodeStatus        (NodeStatusRequest)      returns (NodeStatus);

  // --- SÓ pro cert "payer" ---
  rpc PayInvoice        (PayInvoiceRequest)      returns (Payment);
  rpc TrackPayment      (TrackPaymentRequest)    returns (stream Payment);
}

message CreateInvoiceRequest {
  optional uint64 amount_msat = 1;   // ausente = amountless
  uint32 expiry_secs          = 2;   // ex.: 900
  string memo                 = 3;
  string external_id          = 4;   // ln_invoices.id, pra correlação em log
}

message Invoice {
  bytes  payment_hash     = 1;
  string payment_request  = 2;       // BOLT11
  uint64 add_index        = 3;
  uint64 settle_index     = 4;       // 0 enquanto não settled
  InvoiceState state       = 5;      // OPEN | SETTLED | CANCELED | ACCEPTED
  optional uint64 amount_msat      = 6;
  optional uint64 amount_paid_msat = 7;
  google.protobuf.Timestamp created_at = 8;
  google.protobuf.Timestamp expires_at = 9;
  optional google.protobuf.Timestamp settled_at = 10;
}

message SubscribeRequest { uint64 start_settle_index = 1; } // retomada

message DecodedInvoice {
  bytes  payment_hash = 1;
  bytes  dest_pubkey  = 2;
  optional uint64 amount_msat = 3;   // ausente = amountless (app precisa exigir valor)
  string description  = 4;
  string network      = 5;           // "mainnet" | "signet" | "regtest" | "testnet"
  google.protobuf.Timestamp expires_at = 6;
  bool   is_expired   = 7;
}

message PayInvoiceRequest {
  string  payment_request = 1;
  optional uint64 amount_msat = 2;   // obrigatório se o invoice for amountless
  uint64  fee_limit_msat  = 3;       // OBRIGATÓRIO > 0; bridge rejeita se 0
  uint32  timeout_secs    = 4;       // ex.: 60
  string  idempotency_key = 5;       // ln_payments.idempotency_key
}

message Payment {
  bytes  payment_hash  = 1;
  PaymentState state    = 2;         // IN_FLIGHT | SUCCEEDED | FAILED
  optional bytes  preimage       = 3;
  optional uint64 fee_msat_paid  = 4;
  optional string failure_reason = 5; // "NO_ROUTE" | "TIMEOUT" | "INCORRECT_PAYMENT_DETAILS" | ...
}
```

### Regras que o `ln-bridge` aplica (não o app)

| Regra | Valor (config) |
|---|---|
| `PayInvoice` só com cert `payer` | SAN do cert = `payer` |
| `fee_limit_msat == 0` → rejeita | sempre |
| `fee_limit_msat` acima do teto absoluto → clampa e loga | `LN_MAX_FEE_MSAT_ABS` |
| valor do pagamento acima do teto sem `approved=true` no request | `LN_PAYMENT_APPROVAL_THRESHOLD_SAT` |
| nº de `PayInvoice` por janela | `LN_PAY_RATE_LIMIT` (ex.: 30/min) |
| destino em allowlist (opcional, F2) | `LN_DEST_ALLOWLIST` (lista de pubkeys) |
| `CreateInvoice` acima do cap de inbound | `LN_INBOUND_CAP_SAT` menos saldo local atual |

`mTLS`: server cert do bridge assinado por uma CA interna; `api-server` e
`worker` cada um com seu cert de cliente (`CN=invoicer` / `CN=payer`) assinado
pela mesma CA. Rotação: certs de 90 dias, emitidos pelo mesmo mecanismo de
secret do resto (External Secrets / Vault em prod).

---

## 4. Fluxos

### 4.1 Depósito Lightning

```
Usuário            api-server                 Postgres            ln-bridge/lnd        worker
  │                    │                          │                     │                │
  │ POST /v1/deposits/ln/invoice {amount_sat?}    │                     │                │
  │───────────────────▶│                          │                     │                │
  │                    │ resolve wallet (user,BTC,PERSONAL)             │                │
  │                    │ CreateInvoice(amount_msat, expiry=900, ext_id) │                │
  │                    │─────────────────────────────────────────────▶ │                │
  │                    │ ◀── {payment_hash, payment_request, add_index, expires_at}      │
  │                    │ INSERT ln_invoices (PENDING)                   │                │
  │                    │─────────────────────────▶│                     │                │
  │ ◀── {payment_request, payment_hash, expires_at}                     │                │
  │                    │                          │                     │                │
  │ (mostra QR do BOLT11; faz polling GET /v1/deposits/ln/invoice/:hash)│                │
  │                    │                          │                     │                │
  │ ....... paga o invoice de outra carteira ......│                     │                │
  │                    │                          │  HTLC settled       │                │
  │                    │                          │                     │  stream Invoice{SETTLED, amount_paid_msat, settle_index}
  │                    │                          │                     │───────────────▶│
  │                    │                          │  ◀── credit_ln_deposit(payment_hash, amount_paid_msat):
  │                    │                          │        BEGIN; lock wallet;
  │                    │                          │        if ln_invoices.credited_ledger_entry_id is null:
  │                    │                          │          INSERT ledger_entries(DEPOSIT, ref=ln_invoice)
  │                    │                          │          UPDATE ln_invoices SET status='SETTLED', ...
  │                    │                          │          INSERT outbox (DepositConfirmed{rail=LIGHTNING})
  │                    │                          │        COMMIT
  │                    │                          │  UPDATE ln_stream_checkpoints SET last_index = settle_index
  │ ◀── GET .../invoice/:hash → {status:"SETTLED", credited:true}       │                │
```

Regras:
- `expiry` curto (default 900 s, `LN_INVOICE_EXPIRY_SECS`). Invoice expirado →
  poller marca `EXPIRED`; o front oferece "gerar novo".
- **Subpagamento** (invoice com valor, pago a menos): LND não settla — não
  chega evento. **Sobrepagamento**: credita `amount_paid_msat`.
- **Amountless**: o front **exige** um valor no formulário e emite invoice com
  valor; amountless só é aceito se algum dia expusermos LNURL-pay.
- Rate limit por user em `CreateInvoice` + cap de invoices `PENDING` por
  wallet (`LN_MAX_PENDING_INVOICES`, ex.: 5).

### 4.2 Saque Lightning

```
Usuário           api-server                Postgres           worker            ln-bridge/lnd
  │ POST /v1/withdrawals/ln {payment_request, amount_sat?}  (+ 2FA, igual on-chain)
  │─────────────▶│                             │               │                    │
  │              │ DecodeInvoice ──────────────────────────────────────────────────▶│
  │              │ ◀── {dest_pubkey, amount_msat, network, is_expired, description}  │
  │              │ valida: network == LN_NETWORK; !is_expired;                       │
  │              │         amount bate; dest != nosso nó (ou permitido);            │
  │              │         amount_sat <= saldo BTC disponível da wallet             │
  │              │ BEGIN; lock wallet;                                              │
  │              │   saldo suficiente? senão 422                                    │
  │              │   INSERT ledger_entries(WITHDRAWAL  -amount, ref=ln_payment)     │
  │              │   INSERT ledger_entries(WITHDRAWAL_FEE -fee_reserva, ref=...)    │
  │              │   INSERT ln_payments(PENDING, fee_msat_limit, idempotency_key,   │
  │              │           requires_approval = amount >= threshold)               │
  │              │   INSERT queue job 'ln_payment_send' {ln_payment_id}             │
  │              │ COMMIT                                                           │
  │ ◀── {id, status:"PENDING"}                   │               │                    │
  │              │                             │  claim job ────▶│                    │
  │              │                             │  se requires_approval e !approved: espera admin
  │              │                             │  UPDATE ln_payments SET status='PAYING'
  │              │                             │  PayInvoice(pr, fee_limit_msat, timeout=60, idem_key) ─▶│
  │              │                             │                 │  ◀── Payment{SUCCEEDED, preimage, fee_msat_paid}
  │              │                             │  BEGIN; lock wallet;
  │              │                             │    ajusta WITHDRAWAL_FEE pro fee real (estorna diferença)
  │              │                             │    UPDATE ln_payments SET status='SUCCEEDED', preimage, fee_msat_paid
  │              │                             │    INSERT outbox (LnPaymentSucceeded)
  │              │                             │  COMMIT; queue.complete(job)
```

Falhas:
- **Definitiva** (`FAILED` do nó: `NO_ROUTE`, `INCORRECT_PAYMENT_DETAILS`,
  invoice expirado): `BEGIN; lock wallet; INSERT WITHDRAWAL_REVERSAL +amount;
  INSERT WITHDRAWAL_FEE_REVERSAL +fee_reserva; UPDATE ln_payments FAILED;
  outbox LnPaymentFailed; COMMIT; queue.complete`.
- **Ambígua** (timeout do `PayInvoice`, `ln-bridge` caiu, `IN_FLIGHT`):
  **não mexe no ledger**. `queue.fail(job, "ambiguo")` → re-tentado. Na
  re-tentativa o worker chama `TrackPayment(payment_hash)` primeiro; só
  finaliza (feliz ou revertendo) quando o nó der estado terminal. LND recusa
  segundo `PayInvoice` do mesmo hash em voo — não há risco de pagar 2×.
- **Idempotência**: `idempotency_key` único em `ln_payments` (barra duplo
  submit do user) + `payment_hash` único (barra duplo pagamento no nó).

### 4.3 Reconciliação / defesa em profundidade

`ln_invoice_poller` (ver §5) cobre o caso do stream ter perdido um evento
(bridge reiniciou, gap no `settle_index`): a cada N s varre `ln_invoices`
`PENDING` e chama `LookupInvoice`; se `SETTLED`, roda o mesmo
`credit_ln_deposit`. Idempotente, então rodar junto com o stream é seguro.

---

## 5. Jobs do worker

| Job | Gatilho | O que faz |
|---|---|---|
| `ln_settlement_watcher` | task contínua | mantém `SubscribeInvoices(start=checkpoint)`; em `SETTLED` → `credit_ln_deposit` + avança `ln_stream_checkpoints`. Reconecta com backoff; ao reconectar retoma do checkpoint. |
| `ln_invoice_poller` | `LN_INVOICE_POLL_SECS` (ex.: 30) | reconciliação (§4.3) + marca `EXPIRED` os `PENDING` com `expires_at < now()`. |
| `ln_payment_worker` | job `ln_payment_send` na `queue` (SKIP LOCKED) | fluxo §4.2. Usa cert `payer`. |
| `ln_health_reporter` | `LN_HEALTH_SECS` (ex.: 30) | `NodeStatus` → grava snapshot pra `/status` + dispara alerta em thresholds (saldo local/remoto, HTLC pendente, `synced_to_chain=false`, versão). |

Todos seguem o padrão dos jobs atuais (`tokio::spawn` + `interval` em
`worker/src/main.rs`; fila interna via crate `queue`).

---

## 6. Rotas da API

| Método | Rota | Auth | Nota |
|---|---|---|---|
| `POST` | `/v1/deposits/ln/invoice` | user | body `{ amount_sat?: number }`; cria invoice pra wallet BTC do caller |
| `GET` | `/v1/deposits/ln/invoice/:payment_hash` | user | status + `credited` |
| `GET` | `/v1/deposits/ln/invoices` | user | histórico paginado |
| `POST` | `/v1/withdrawals/ln` | user + 2FA | body `{ payment_request: string, amount_sat?: number }` |
| `GET` | `/v1/withdrawals/ln/:id` | user | status + `preimage` quando `SUCCEEDED` |
| `GET` | `/v1/admin/ln/pending-payments` | admin | fila de aprovação (reusa superfície do saque admin) |
| `POST` | `/v1/admin/ln/payments/:id/approve` | admin | libera `requires_approval` |
| `POST` | `/v1/admin/ln/payments/:id/reject` | admin | cancela + reverte hold |

`DepositConfirmed` ganha `rail: "ONCHAIN" | "LIGHTNING"`. Novos:
`LnPaymentSucceeded`, `LnPaymentFailed` (aggregate `wallet`).

---

## 7. Frontend

- **`DepositPage`** — seletor de trilho `On-chain | Lightning`.
  - Lightning: input de valor em sats (obrigatório) → `POST /v1/deposits/ln/invoice`
    → renderiza QR do `payment_request` (BOLT11 em maiúsculas pra densidade
    de QR menor), botão copiar, contador regressivo até `expires_at`, status
    ao vivo (polling do `GET :hash` a cada 3 s ou SSE). Ao expirar: botão
    "gerar novo invoice".
  - Generalizar `AddressQr` → `QrBox` (aceita qualquer string).
- **`WithdrawPage`** — seletor de trilho.
  - Lightning: textarea cola BOLT11 → chama um `POST /v1/withdrawals/ln/decode`
    (ou decodifica client-side com uma lib e confirma no submit) → preview
    `{ valor, destino curto, descrição, expira em }` → confirmar (2FA) →
    acompanha status; mostra `preimage` como recibo quando `SUCCEEDED`.
- **`/status`** — adicionar linha "Nó Lightning" alimentada pelo snapshot do
  `ln_health_reporter` (novo `GET /v1/status/ln` público e enxuto:
  `{ synced, channels, htlc_pending }`, sem expor saldos).

---

## 8. Configuração (env)

### `ln-bridge` (na VM do nó)

| Var | Exemplo | Nota |
|---|---|---|
| `LND_GRPC_ADDR` | `127.0.0.1:10009` | sempre localhost |
| `LND_TLS_CERT_PATH` | `/root/.lnd/tls.cert` | |
| `LND_MACAROON_INVOICER_PATH` | `/etc/ln-bridge/invoicer.macaroon` | `invoices:*` + `offchain:read` |
| `LND_MACAROON_PAYER_PATH` | `/etc/ln-bridge/payer.macaroon` | + `offchain:write` |
| `BRIDGE_LISTEN` | `10.9.0.2:9911` | IP do `wg0` |
| `BRIDGE_TLS_CERT_PATH` / `_KEY_PATH` | | server cert mTLS |
| `BRIDGE_CLIENT_CA_PATH` | | CA que assina os certs de `api-server`/`worker` |
| `LN_NETWORK` | `signet` | tem que casar com o que o app valida |
| `LN_INBOUND_CAP_SAT` | `5_000_000` | F2 |
| `LN_MAX_FEE_MSAT_ABS` | `50_000` | teto absoluto de fee por pagamento |
| `LN_PAYMENT_APPROVAL_THRESHOLD_SAT` | `200_000` | acima disso exige `approved` |
| `LN_PAY_RATE_LIMIT` | `30/min` | |
| `LN_DEST_ALLOWLIST` | (vazio) | opcional, F2 |
| `SCB_UPLOAD_URL` | `s3://…/scb/` | destino do `multi_chan_backup` |

### app (`api-server` / `worker`)

| Var | Exemplo |
|---|---|
| `LN_BRIDGE_ADDR` | `10.9.0.2:9911` |
| `LN_BRIDGE_CA_PATH` | CA do server cert do bridge |
| `LN_BRIDGE_CLIENT_CERT_PATH` / `_KEY_PATH` | cert `invoicer` (api-server) / `payer` (worker) |
| `LN_NETWORK` | `signet` |
| `LN_INVOICE_EXPIRY_SECS` | `900` |
| `LN_MAX_PENDING_INVOICES` | `5` |
| `LN_INVOICE_POLL_SECS` | `30` |
| `LN_WITHDRAWAL_FEE_RESERVE_BPS` | `50` (0,5%) — reserva do hold; sobra estorna |
| `LN_HEALTH_SECS` | `30` |

Feature flag: `LN_ENABLED=false` por padrão — desligado, as rotas `/v1/*/ln/*`
retornam 404 e o front esconde o seletor de trilho.

---

## 9. Threat model — adições Lightning

Complementa `security/threat-model-and-gaps.md`.

| # | Ameaça | Mitigação |
|---|---|---|
| L1 | `api-server`/`worker` comprometido | Cert `invoicer` não paga. Cert `payer` só no worker, com fee cap + rate limit + threshold **no bridge**. Macaroon do nó nunca sai da VM. |
| L2 | VM do nó comprometida | Sem Postgres nem segredo do app na VM. Inbound só 9735/Tor. RPC/SSH só em `wg0`. Firewall default-deny. Seed em secret selado + cópia offline. |
| L3 | Perda do disco / estado de canal | SCB (`multi_chan_backup`) enviado a `SCB_UPLOAD_URL` a cada `SubscribeChannelEvents`; alerta se falhar. Runbook de restore em `docs/operations/`. Restore exercitado 1× antes da F2. |
| L4 | Force-close / peer malicioso | Watchtower (`wtclient` ou torre externa) antes de subir caps. Escolha de peers. Monitor de HTLC pendente e de canais em `pending_force_close`. |
| L5 | Nó é hot wallet (todo saldo de canal é quente) | `LN_INBOUND_CAP_SAT` global. Sweep programado do excesso de saldo local pra cold on-chain (loop-out ou manual na F2). Alerta de razão quente. |
| L6 | Pagamento duplicado no saque | `payment_hash` único + LND recusa hash em voo. `idempotency_key` único. Estado ambíguo **nunca** reverte, só `TrackPayment`. |
| L7 | Probing de saldo via invoices | Expiração curta, sem reutilização de invoice, route hints controladas. (Hold invoice opcional no futuro.) |
| L8 | Sub/sobre-pagamento | Credita `amount_paid_msat` do nó (verdade), nunca o valor do invoice. Subpagamento não settla. |
| L9 | DoS por criação de invoices | Rate limit por user + `LN_MAX_PENDING_INVOICES` por wallet. |
| L10 | Fee de roteamento abusiva | `fee_limit_msat` obrigatório (`LN_WITHDRAWAL_FEE_RESERVE_BPS`); excesso = falha, não débito surpresa. Diferença entre reserva e fee real é estornada. |
| L11 | Pagar o próprio nó (lavagem de saldo interno) | `DecodeInvoice.dest_pubkey == nosso nó` → recusa (ou caminho interno explícito no futuro). |
| L12 | Rede errada (mainnet vs signet) | `DecodedInvoice.network` tem que casar `LN_NETWORK`; recusa caso contrário. |
| L13 | `ln-bridge` indisponível no meio de um pagamento | Job fica ambíguo, não reverte; `TrackPayment` na volta resolve. Alerta de bridge down no `ln_health_reporter`. |

---

## 10. Rollout (detalhe da tabela do ADR)

### F0 — regtest (docker-compose)

- `deploy/docker/docker-compose.ln.yml`: `lnd` (regtest) + `ln-bridge` + 2 nós
  LND de teste (contraparte) + `bitcoind` regtest.
- Abrir canais entre os 3, minerar blocos por script.
- Testes: `crates/db/tests/ln_deposit_smoke.rs`, `ln_payment_smoke.rs`
  (`#[sqlx::test]`); `crates/ln-bridge/tests/` contra o LND regtest.
- Critério de saída: depósito e saque ponta a ponta verdes; teste de "bridge
  reiniciado no meio" verde; teste de gap no `settle_index` (poller recupera).

### F1 — signet / testnet

- VM real do nó. WireGuard app↔nó. CA interna + certs mTLS emitidos.
- Pipeline de SCB pra bucket real; **restaurar** o nó de um SCB com sucesso.
- `/status` com a linha do nó. `LN_ENABLED=true` só em staging.
- Caps mínimos (`LN_INBOUND_CAP_SAT` ~100k). Watchtower configurado.
- Critério de saída: 1 semana sem incidente; restore de SCB documentado e
  executado; alertas disparando de verdade em teste de falha.

### F2 — mainnet piloto

- Canais pequenos com 2–3 peers confiáveis. `LN_INBOUND_CAP_SAT` ~5M (0,05 BTC).
- `LN_PAYMENT_APPROVAL_THRESHOLD_SAT` baixo → **todo** saque LN com aprovação
  manual no começo. Sweep manual do excesso.
- Monitorar por semanas antes de relaxar.

### F3 — produção

- Caps por política. Sweep automatizado (loop-out agendado). Watchtower
  redundante.
- **LNURL-pay por usuário** (endereço Lightning reutilizável
  `user@bitcosats.com` via `/.well-known/lnurlp/{user}`) — gera invoice fresco
  por request; some a fricção de "gerar invoice" no depósito.
- Avaliar BOLT12 offers quando o ecossistema de carteiras amadurecer.
