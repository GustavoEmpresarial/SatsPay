# ADR 0011 — BTC na Lightning: custódia, isolamento e modelo de dados

## Status

Proposto. Nada implementado ainda — este ADR fixa as decisões estruturais
antes da primeira linha de código, porque o caminho Lightning move **fundos
quentes reais** e correções depois de mainnet são caras.

## Contexto

Hoje BTC só existe on-chain (`ChainClient` + Bitcore, ver ADR 0008). Queremos
aceitar e pagar BTC também pela Lightning Network. Restrições dadas:

- Os nós Lightning ficam em **VM(s) separada(s)**, dedicada(s) só a isso.
- Cada depósito gera um "endereço de pagamento" (na prática, um **invoice
  BOLT11**) para o usuário pagar.
- Segurança em primeiro lugar; o sistema tem que já nascer pronto pra escalar.

Diferenças de fundo entre on-chain e Lightning que moldam o desenho:

| | On-chain BTC | Lightning |
|---|---|---|
| Recebimento | endereço reutilizável, derivado de xpub watch-only | invoice BOLT11 por pagamento, com valor e expiração |
| Detecção | polling de UTXOs por endereço | nó reporta invoice *settled* (stream `SubscribeInvoices` ou `LookupInvoice`) |
| Chave quente | 1 WIF assina saques; xpub não gasta | a chave do nó move **todo o saldo dos canais** |
| Unidade | satoshi (8 casas) | milissatoshi (11 casas) |
| Saque | assina + faz broadcast; confirma em blocos | paga invoice de terceiro; roteamento probabilístico, HTLC em voo, falha parcial |
| Backup | só a seed | seed **+ estado de canais** (SCB); perder o estado = perder fundos |

## Decisões

### D1 — BTC é **um só ativo**; Lightning é um **trilho**, não uma moeda nova

Não entra valor novo no enum `coin`. O saldo BTC do usuário é único; on-chain e
Lightning são só formas de entrada/saída do mesmo saldo. A plataforma cuida
internamente do rebalanceamento on-chain ↔ canais.

Consequência no schema: **nada** de `coin = 'LN_BTC'`. Duas tabelas novas
(`ln_invoices`, `ln_payments`) que escrevem no **mesmo** ledger da wallet
`(user, BTC, PERSONAL)` via lançamentos `DEPOSIT` / `WITHDRAWAL` com
`reference_type = 'ln_invoice' | 'ln_payment'`. O invariante do ledger
(`SUM(ledger_entries.amount)`, lock na linha da wallet — ADR 0006) vale igual.

### D2 — Unidade do ledger continua **satoshi**

O ledger não passa a ser msat. As tabelas LN guardam msat; o crédito ao ledger
é `floor(amount_paid_msat / 1000)`. O resto sub-satoshi fica com a plataforma
(documentado; é poeira: < 1 sat por depósito). Invoices são emitidos sempre em
sat inteiro. **Subpagamento nunca credita**; sobrepagamento credita o
`amount_paid` reportado pelo nó (verdade do nó), não o valor do invoice.

### D3 — Implementação do nó: **LND**, com macaroons escopados

LND pela maturidade do gRPC, streams de subscription resumíveis
(`SubscribeInvoices` com `settle_index`), `routerrpc` (fee limit + timeout +
`TrackPaymentV2`) e a *macaroon bakery* pra emitir credenciais de permissão
mínima. Core Lightning + runes é alternativa aceitável; a decisão é LND. LDK
fica fora — é biblioteca, não nó; reavaliar só se formos pra custódia mais
profunda no futuro.

### D4 — Serviço `ln-bridge`, **co-locado na VM do nó**, é o único que fala com o nó

Ninguém do app (`api-server`, `worker`) fala gRPC com o LND. Fala com o
`ln-bridge` — um binário Rust novo neste workspace — que roda **na própria VM
do nó**, conversa com o LND por `localhost`, e expõe uma API gRPC **mínima e
tipada** pro app, sobre WireGuard, com **mTLS**.

- `admin.macaroon` e a seed do LND **nunca saem da VM**.
- Duas identidades de cliente no `ln-bridge`, cada uma com seu cert mTLS e seu
  conjunto de RPCs permitido (mapeado pelo SAN do cert do cliente):
  - **`invoicer`** — `CreateInvoice`, `LookupInvoice`, `SubscribeInvoices`,
    `DecodeInvoice`, `NodeStatus`. **Não paga.** É o que o caminho de depósito
    usa.
  - **`payer`** — tudo do `invoicer` mais `PayInvoice` / `TrackPayment`. É o que
    o worker de saque usa, e só ele. Fee cap, timeout, limites de velocidade e
    thresholds de aprovação são aplicados **no `ln-bridge`**, não só no app.
- No lado do LND, o `invoicer` recebe uma macaroon `invoices:read/write` +
  `offchain:read`; o `payer` recebe uma macaroon separada com `offchain:write`.
  mTLS + escopo de RPC no bridge é redundância proposital por cima disso.

`ln-bridge` é **stateless** — todo estado (invoices, pagamentos, checkpoints de
stream) vive no Postgres principal, escrito pelo `api-server`/`worker`. Assim a
VM do nó não tem banco nem segredo do app.

### D5 — Topologia de isolamento

VM do nó Lightning:

- Inbound público: **só** a porta P2P `9735/tcp` (ou só Tor, sem inbound
  público — decisão de operação por ambiente).
- gRPC/REST do LND, o `ln-bridge` e o SSH: **só** na interface WireGuard `wg0`.
- Firewall **default-deny**. Sem Postgres, sem Kafka, sem segredo do app na VM.
- App VM ↔ VM do nó: só por WireGuard, só a porta do `ln-bridge`, só mTLS.

O `/status` já existente ganha um check de nó (via `NodeStatus` do bridge).

### D6 — Crédito de depósito **exatamente-uma-vez**, chaveado por `payment_hash`

`payment_hash` é globalmente único. O `worker` mantém um consumidor do
`SubscribeInvoices` (via `ln-bridge`) com checkpoint `settle_index` em
`ln_stream_checkpoints`; ao receber *settled*, chama
`db::ln::credit_ln_deposit(payment_hash, amount_paid_msat)`, que num **único
`sqlx::Transaction`**: trava a wallet, checa se o invoice já foi creditado
(`credited_ledger_entry_id IS NULL`), escreve o `DEPOSIT` no ledger, marca o
invoice `SETTLED` e emite o evento de domínio no outbox — mesma disciplina do
`credit_deposit` on-chain. Um poller de reconciliação varre invoices `PENDING`
não expirados como defesa em profundidade e marca os vencidos `EXPIRED`.

### D7 — Máquina de estados de saque LN, espelhando a on-chain

`PENDING → APPROVED → QUEUED → PAYING → PAID | FAILED | CANCELED`.

- Na requisição: valida (rede certa, não expirado, valor bate, opcionalmente
  bloqueia pagar o próprio nó), **trava o valor no ledger** (débito
  `WITHDRAWAL` + `WITHDRAWAL_FEE` reservado) e enfileira job interno
  (`queue`, SKIP LOCKED — igual saque on-chain).
- `ln_payment_worker` drena o job → `payer.PayInvoice` com `fee_limit_msat`
  obrigatório (ex.: `0,5% + 1 sat`) e `timeout`.
- Sucesso → grava `preimage` + `fee_msat_paid`, ajusta o lançamento de fee pro
  valor real, estado `PAID`.
- Falha **definitiva** (sem rota, invoice expirado, rejeitado) → reverte o hold
  (`WITHDRAWAL_REVERSAL`), estado `FAILED`.
- Resultado **ambíguo** (timeout, bridge caiu, HTLC em voo) → **não reverte**;
  mantém o hold e faz `TrackPayment(payment_hash)` até resolver. Nunca
  re-paga: o LND recusa um `payment_hash` já em voo — idempotência natural,
  reforçada por `idempotency_key`.

### D8 — O nó é uma hot wallet: **limitar o raio de explosão**

Saldo em canais = fundo quente por natureza. Controles obrigatórios desde o
piloto:

- **Cap global de inbound** — não aceitar depósito que leve o saldo local
  total acima de X BTC.
- **Limites de velocidade** de depósito por usuário e global.
- **Razão quente alvo** + **sweep programado** do excesso de saldo local pra
  cold storage on-chain (loop-out ou manual no início).
- **Alertas** em thresholds de saldo local/remoto e em contagem de HTLC
  pendente / force-close.

### D9 — Backup e DR

- Seed do LND em *sealed secret* + cópia offline (papel/HSM).
- **SCB** (`multi_chan_backup`) enviado pra object storage **a cada evento de
  canal** (`SubscribeChannelEvents`), com alerta se o envio falhar. Sem SCB,
  perder o disco da VM = perder fundos.
- Runbook de restore documentado em `docs/operations/`.
- `wumbo` desligado no começo; peers escolhidos por confiabilidade;
  watchtower (`wtclient` ou torre externa) antes de subir os caps.

### D10 — Eventos de domínio

Reusar `DepositConfirmed` com um campo `rail: ONCHAIN | LIGHTNING` em vez de um
evento novo; adicionar `LnPaymentSucceeded` / `LnPaymentFailed` no
`wallet.events` / `ledger.events`. `aggregate_type = "wallet"`.

### D11 — Rollout em fases (sem pular)

| Fase | Ambiente | Porta de saída |
|---|---|---|
| F0 | regtest no docker-compose (`lnd` + `ln-bridge` + 2 nós de teste) | fluxo depósito+saque ponta a ponta, testes verdes |
| F1 | signet/testnet, VM real do nó, WireGuard, mTLS, pipeline de SCB, `/status` | caps mínimos; SCB restaurado com sucesso 1× |
| F2 | mainnet piloto | canais pequenos; cap global ~0,05 BTC; cap/usuário ~200k sat; **todo** saque LN com aprovação manual; sweep manual |
| F3 | produção | caps por política; sweep automatizado; watchtower; LNURL-pay por usuário (endereço Lightning reutilizável); avaliar BOLT12 |

## Consequências

- Um crate/binário novo: `ln-bridge`. Um novo alvo de deploy (VM + unit
  systemd ou container) fora do k8s do app, ou um nó dedicado no cluster com
  `NetworkPolicy` restrita — decisão de F1.
- Migrations novas: `ln_invoices`, `ln_payments`, `ln_stream_checkpoints`,
  enums `ln_invoice_status` / `ln_payment_status`, coluna `rail` onde fizer
  sentido. Detalhe em `docs/architecture/lightning.md`.
- Frontend: `DepositPage` e `WithdrawPage` ganham um seletor de trilho
  (On-chain | Lightning). Depósito LN mostra QR do BOLT11 e status ao vivo;
  saque LN cola um BOLT11 e mostra preview decodificado.
- `threat-model-and-gaps.md` ganha a seção Lightning (tabela em
  `docs/architecture/lightning.md`).
- Antes de F2: exercitar F0 e F1 por completo, inclusive um restore de SCB
  real e um teste de "bridge caiu no meio do pagamento".
