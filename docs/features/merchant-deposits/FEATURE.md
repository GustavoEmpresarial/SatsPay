# FEATURE — Merchant Deposits

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `merchant-deposits` |
| Título | Merchant Deposits |
| Componente | `MerchantDepositsPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`merchant deposits MerchantDepositsPage /merchant/deposits /merchant/deposits /merchant/deposits/:id/test-webhook /merchant/settings Endereços de Depósito Chaves de API Ver Checkout (Demo) Documentação user`

## Rotas

- `/merchant/deposits`

## Abas / seções internas

- Endereços de Depósito
- Chaves de API
- Ver Checkout (Demo)
- Documentação

## APIs usadas (client → `/v1…`)

- `/merchant/deposits` (prefixo `/v1` no servidor)
- `/merchant/deposits/:id/test-webhook` (prefixo `/v1` no servidor)
- `/merchant/settings` (prefixo `/v1` no servidor)

## Arquivos-chave

- `client/src/pages/MerchantDepositsPage.tsx`
- `docs/pages/merchant-deposits/`

## Comportamento (bruto)

Página React `MerchantDepositsPage`. Chama 3 endpoint(s) via `api()`. Abas/labels: Endereços de Depósito, Chaves de API, Ver Checkout (Demo), Documentação. Endereço HD por coin; watcher no worker credita ledger. Gateway: `POST /v1/merchant/deposits` (aliases `/deposits/create`, `/invoices`) → 201 com `checkoutUrl`/`payUrl`. `amount` em unidades de ledger (1e-8). `orderId` idempotente por merchant. Confirmação via `worker::invoice_watcher` (on-chain) ou `POST /v1/public/pay/:id/balance` (saldo). Webhook `deposit.confirmed` assinado `sha256=<hex>`; segredo em `GET /v1/merchant/webhook-signing-secret`. Pausa BTC/LTC/DOGE/BCH/DGB → 503 DEPOSIT_PAUSED; `/v1/public/send` não pausa.

## Notas de overview legado

# Merchant Deposits — Overview

## Papel

Página **Merchant Deposits** (`MerchantDepositsPage.tsx`).

- Auth gate: **user**
- Rotas: `/merchant/deposits`


## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (`MarketingLayout` / `AppLayout` / `AdminLayout` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por `formatApiError` / telemetria quando aplicável.

## i18n

Preferir chaves em `client/src/i18n/locales/{pt,en}.json` quando a página for traduzida.

## Segurança

- Respeitar gate `user` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.


## Pagamento a mais / a menos (regra do watcher)

`crates/worker/src/invoice_watcher.rs` soma **todas** as entradas no endereço da moeda paga e
confirma quando a soma com `min_confirmations` ≥ `amount` travado daquela moeda
(`merchant_invoice_addresses.amount`, não a seleção atual da fatura).

| Caso | Status | Webhook | Crédito do comerciante | Dinheiro on-chain |
|---|---|---|---|---|
| Exato | `CONFIRMED` | sim | `net_amount` da fatura | sweep → hot |
| **A mais** (comum: arredondamento, exchange) | `CONFIRMED` | sim | `net_amount` **da fatura**; excedente **não** creditado | sweep leva **tudo** → o excedente fica na hot, sem dono no ledger |
| **A menos** | `DETECTED`, `received_amount` parcial | **não** | nada | fica no endereço da fatura |
| A menos + completou antes de expirar | `CONFIRMED` | sim | `net_amount` | sweep → hot |
| A menos e expirou | `EXPIRED` | não | nada | **parado no endereço** (sem sweep: só roda após confirmar) |

Suporte:
- Excedente: conferir `received_amount − amount` na fatura e devolver/creditar manualmente
  com lançamento auditado. Não existe fluxo automático de reembolso.
- Parcial expirado: o valor segue no endereço derivado (`hd_index` preservado, porque o sweep não
  rodou). Devolução exige sweep manual + lançamento auditado.
- `received_amount` só é atualizado enquanto `PENDING/DETECTED`: entradas que chegam **depois**
  do `CONFIRMED` ou do `EXPIRED` não aparecem na fatura — conferir on-chain.

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
