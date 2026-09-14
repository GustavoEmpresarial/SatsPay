# FEATURE — Gateway merchant (domínio)

## Keywords

`merchant_deposit_invoices HMAC api_keys webhook checkout gateway fee order_id DEVELOPER wallet`

## Fluxo

1. Merchant APPROVED cria API key  
2. `POST /v1/public/…` (HMAC) cria invoice → endereço depósito  
3. User paga on-chain (ou saldo autenticado) → worker / `pay_invoice_with_balance` confirma → ledger DEVELOPER + fee plataforma  
4. Webhook `callback_url` assinado com HMAC-SHA256 por merchant (`X-SatsPay-Signature: sha256=…`). Segredo derivado de `ENCRYPTION_KEY` (HKDF `bitcosats:webhook:v1`) — `GET /v1/merchant/webhook-signing-secret`. **Não** usar `satspay_secret_default`. Sem sandbox público `simulate-payment`.

## Admin

[`../admin-merchants/FEATURE.md`](../admin-merchants/FEATURE.md) — stats + moderação  

## User / merchant app

- Checkout público `/pay/:id`  
- Merchant dashboard / deposits / sites  
- Docs HMAC: `docs/api/public-api-hmac.md`  

## Schema

`crates/db/migrations/0010_merchant_deposit_invoices.sql`  
Statuses: PENDING → DETECTED → CONFIRMED | EXPIRED | CANCELLED  
