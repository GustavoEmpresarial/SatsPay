# FEATURE — Gateway merchant (invoices + HMAC)

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `domain-gateway-merchant` |
| Título | Gateway merchant (invoices + HMAC) |
| Componente | `—` |
| Auth | **mixed** — pública ou autenticada conforme contexto |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`merchant_deposit_invoices gateway HMAC api_keys webhook checkout order_id fee 0.25% uma-moeda-por-fatura deposit.confirmed checkoutUrl payUrl invoice_watcher ledger units idempotency toEmail public/send PEPE payout email OAuth`

## Rotas

- (sem rota SPA — domínio backend)

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `POST /v1/merchant/deposits` (prefixo `/v1` no servidor)
- `GET /v1/merchant/deposits/:id` (prefixo `/v1` no servidor)
- `GET /v1/merchant/webhook-signing-secret` (prefixo `/v1` no servidor)
- `GET /v1/public/pay/:id` (prefixo `/v1` no servidor)
- `POST /v1/public/pay/:id/balance` (prefixo `/v1` no servidor)
- `GET /v1/admin/merchants/stats` (prefixo `/v1` no servidor)

## Arquivos-chave

- `crates/db/migrations/0010_merchant_deposit_invoices.sql`
- `crates/db/migrations/0026_merchant_invoice_order_unique.sql`
- `crates/db/migrations/0027_merchant_invoice_onchain.sql`
- `crates/api-http/src/merchant_deposits.rs`
- `crates/webhooks/src/lib.rs`
- `crates/worker/src/invoice_watcher.rs`
- `docs/api/public-api-hmac.md`
- `docs/api/http-api-reference.md`
- `client/src/pages/CheckoutPage.tsx`
- `client/src/pages/AdminMerchantsPage.tsx`

## Comportamento (bruto)

Criar fatura: `POST /v1/merchant/deposits` (201; aliases `/deposits/create`, `/invoices`) — `/v1/public/pay` NÃO cria nada. Resposta traz `checkoutUrl` (absoluto) + `payUrl` (relativo). `amount` é inteiro em unidades de ledger (1e-8), nunca decimal da moeda: 25 USDT = "2500000000" (decimal → 400 AMOUNT_NOT_INTEGER). `orderId` único por merchant: repetir devolve 200 com a mesma fatura, divergir devolve 409 DUPLICATE_ORDER_ID. Taxa plataforma 0,25% — GATEWAY_FEE_BPS=25 (`feeAmount`/`netAmount`; no webhook o campo chama `fee`). Auth: `x-api-key` ou requisição assinada, escopo `deposits`, whitelist de IP com IP real. Confirmação on-chain: `worker::invoice_watcher` → `confirm_invoice` → sweep. Webhook `deposit.confirmed`, `X-SatsPay-Signature: sha256=<hex>` sobre o corpo cru, com `timestamp`/`attempt` no corpo e retry com backoff (`crates/webhooks`). Statuses: PENDING → DETECTED → CONFIRMED | EXPIRED | CANCELLED (não existe PAID). Duas formas de precificar: `coin`+`amount` (trava a moeda) ou `amountUsd` (o cliente escolhe no checkout via POST /v1/public/pay/:id/select-coin). Cotação trava na escolha; variação até o pagamento é do merchant; conversão arredonda para cima. Endereço abandonado continua vigiado (`merchant_invoice_addresses`). Moedas aceitas por merchant em /v1/merchant/settings (vazio = todas as ativas; pausada nunca é oferecida). `POST /v1/public/send` `toEmail` é o e-mail de uma conta SatsPay que já existe — e-mail digitado e e-mail do OAuth são os dois válidos, o mesmo campo, não é endereço on-chain. Sem conta → recusa, sem débito. Não documentar "só OAuth" nem "não aceita e-mail arbitrário".

## Notas de overview legado

_sem overview em docs/pages_

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.
- `POST /v1/public/send` `toEmail`: e-mail de conta SatsPay que já existe. E-mail digitado pelo usuário e e-mail do OAuth são os dois válidos — o mesmo campo. Não é endereço on-chain. Sem conta → recusa, sem débito. Proibido documentar ou implementar "só o e-mail do login" / "não aceita e-mail arbitrário".
- `POST /v1/public/send` recusa `toEmail` da conta dona da chave: HTTP 400, `code` `SEND_TO_SELF`, corpo explica, nada debitado. Não é saldo insuficiente. Checkout `POST /v1/public/pay/:id/balance` recusa o dono da fatura: `CANNOT_PAY_OWN_INVOICE`, nada debitado.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
