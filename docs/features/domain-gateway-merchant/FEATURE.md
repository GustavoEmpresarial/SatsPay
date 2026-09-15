# FEATURE — Gateway merchant (domínio)

## Keywords

`merchant_deposit_invoices HMAC api_keys webhook checkout gateway fee 0.25% order_id uma-moeda-por-fatura DEVELOPER wallet`

## Fluxo

0. Onboarding: `POST /v1/merchant/apply` → `GET /v1/merchant/status`. Sem `APPROVED`, criar fatura → 403 mesmo com chave válida
1. Merchant APPROVED cria API key com escopo `deposits` (`x-api-key` ou requisição assinada; `requireSignature=true` exige a assinada)  
2. `POST /v1/merchant/deposits` cria invoice → endereço HD dedicado. Duas formas: `coin`+`amount` (inteiro 1e-8, trava a moeda) **ou** `amountUsd` (decimal) + `acceptedCoins` (cliente escolhe no checkout). As duas juntas → 400 AMBIGUOUS_AMOUNT. `orderId` único por merchant (idempotente; em USD a idempotência casa pelo valor em dólar, já que a moeda pode ter mudado)  
2b. Multi-moeda: `POST /v1/public/pay/:id/select-coin` (público) cota, gera endereço daquela rede e trava. Idempotente por `(invoice, coin)`; endereços ficam em `merchant_invoice_addresses` e o **abandonado continua vigiado**. Trava definitiva quando dinheiro aparece (`COIN_LOCKED`). Cotação obsoleta → 503, nunca preço inventado. Conversão USD→unidades arredonda **para cima** (a favor do merchant); a variação entre trava e pagamento é do merchant  
3. User paga on-chain → `worker::invoice_watcher` detecta, grava `received_amount`, confirma em `min_confirmations` e faz sweep; ou paga com saldo → `pay_invoice_with_balance`. Ambos creditam ledger MERCHANT (net) + fee 0,25% da plataforma (`GATEWAY_FEE_BPS=25`, truncada a favor do merchant)  
4. Webhook `deposit.confirmed` no `callback_url`, assinado com HMAC-SHA256 por merchant (`X-SatsPay-Signature: sha256=…`), com `timestamp`/`attempt` no corpo assinado e retry com backoff (`crates/webhooks`). Segredo derivado de `ENCRYPTION_KEY` (HKDF `bitcosats:webhook:v1`) — `GET /v1/merchant/webhook-signing-secret`. **Não** usar `satspay_secret_default`. Sem sandbox público `simulate-payment`.  
5. `callback_url` passa por gate anti-SSRF (https público; localhost/IP interno recusados)

## Admin

[`../admin-merchants/FEATURE.md`](../admin-merchants/FEATURE.md) — stats + moderação  

## Superfícies públicas (sem chave)

- `GET /v1/public/coins` — catálogo com preço e `logoUrl` servido do nosso domínio
- `GET /v1/public/pay/demo` + `POST /v1/public/pay/demo/select-coin` — fatura sintética que demonstra o seletor; `/demo` e `/merchant/demo` apontam para `/pay/demo`. Não grava linha nem consome índice HD, e os endereços são inválidos de propósito (`-DEMO-`)
- `/sdk/satspay-pay.js` — botão de pagamento com a marca (backend cria a fatura, botão só redireciona)
- `/sdk/coins/<símbolo>.svg` — ícones vendorizados (cryptocurrency-icons, MIT)

## Configuração e teste (merchant autenticado)

- `GET`/`PUT /v1/merchant/settings` — moedas aceitas (escopo `deposits`); vazio = todas as ativas, pausada filtrada na leitura **e** na escrita; sem nenhuma ativa → 400 `NO_USABLE_COIN`
- `POST /v1/merchant/deposits/:id/test-webhook` (Bearer JWT) — entrega assinada igual à real, para validar o HMAC sem esperar pagamento
- `POST /v1/public/pay/:id/balance` — cliente quita com saldo SatsPay; vai direto a CONFIRMED e dispara o mesmo webhook

## User / merchant app

- Checkout público `/pay/:id`  
- Merchant dashboard / deposits / sites  
- Docs HMAC: `docs/api/public-api-hmac.md` (reescrito contra o verificador: headers `x-key-id`/`x-timestamp`/`x-signature`, string canônica `ts\nMÉTODO\ncaminho\nsha256(corpo)`, sem nonce — o anti-replay reserva a própria assinatura)
- Referência completa das rotas: `docs/api/http-api-reference.md` (cobertura travada por `client/tests/unit/contract/routeCoverage.contract.test.ts`)  

## Schema

`crates/db/migrations/0010_merchant_deposit_invoices.sql`  
+ `0026_merchant_invoice_order_unique.sql` (único `(merchant_id, order_id)`)  
+ `0027_merchant_invoice_onchain.sql` (`hd_index`, `received_amount`, `webhook_next_retry_at`)  
+ `0028_merchant_gateway_settings.sql` (moedas aceitas por merchant; vazio = todas as ativas)  
+ `0029_invoice_multi_coin.sql` (`price_usd_scaled`, `accepted_coins`, `coin_locked_at`, `quote_price_scaled` + tabela `merchant_invoice_addresses`)  
Statuses: PENDING → DETECTED → CONFIRMED | EXPIRED | CANCELLED (não existe `PAID`)  
