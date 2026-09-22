# API Docs — Rotas e API

## Client

- `/api`
- `/docs`
- `/api-docs`

## Backend

- (nenhum endpoint específico no mount — a página é estática)

## Contrato documentado (tem de bater com o backend)

A página descreve, e os testes de contrato verificam contra o código Rust:

- `POST /v1/merchant/deposits` → `201` com `payUrl` + `checkoutUrl` (`crates/api-http/src/merchant_deposits.rs`)
- `amount` inteiro em unidades de ledger (1e-8)
- Webhook `deposit.confirmed` com `X-SatsPay-Signature: sha256=<hex>` (`crates/webhooks/src/lib.rs`)
- Tabela de moedas renderizada de `client/src/shared/coins.ts` (pausadas marcadas)
- Catálogo de erros = códigos realmente emitidos
- `POST /v1/public/send`: `toEmail` digitado ou `email` do userinfo. `400` com `{ "error", "code" }`: `TARGET_INELIGIBLE`, `SEND_TO_SELF` (conta dona da chave, não é saldo), `DAILY_LIMIT_REACHED`, `WALLET_NOT_FOUND`. Nada debitado.
- `POST /v1/public/pay/:id/balance`: dono da fatura recebe `400 CANNOT_PAY_OWN_INVOICE`. Nada debitado.

## Critérios mínimos

- Rota montada em `App.tsx`
- Componente exporta a page function
- Sem crash no mount sem dados (estado vazio / loading)
- `client/tests/unit/contract/merchantGateway.contract.test.ts` verde
