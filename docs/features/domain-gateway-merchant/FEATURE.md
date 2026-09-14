# FEATURE — Gateway merchant (domínio)

## Keywords

`merchant_deposit_invoices HMAC api_keys webhook checkout gateway fee order_id DEVELOPER wallet`

## Fluxo

1. Merchant APPROVED cria API key  
2. `POST /v1/public/…` (HMAC) cria invoice → endereço depósito  
3. User paga on-chain → worker confirma → ledger DEVELOPER + fee plataforma  
4. Webhook `callback_url` (retry/attempts)  

## Admin

[`../admin-merchants/FEATURE.md`](../admin-merchants/FEATURE.md) — stats + moderação  

## User / merchant app

- Checkout público `/pay/:id`  
- Merchant dashboard / deposits / sites  
- Docs HMAC: `docs/api/public-api-hmac.md`  

## Schema

`crates/db/migrations/0010_merchant_deposit_invoices.sql`  
Statuses: PENDING → DETECTED → CONFIRMED | EXPIRED | CANCELLED  
