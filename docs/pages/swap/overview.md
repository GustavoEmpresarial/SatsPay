# Swap — Overview

## Papel

Página **Swap** (`SwapPage.tsx`).

- Auth gate: **user**
- Rotas: `/swap`
- Nota: HOUSE; SwapKit off

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

## Bridge cross-chain (Relay) — reembolso e hot wallet

Um bridge (ex.: SOL → PEPE) faz nossa própria hot wallet de origem depositar
fundos reais on-chain no Relay — não é só um lançamento de ledger interno. Se
a hot wallet daquela rede estiver com saldo baixo demais para cobrir o valor
mais o mínimo de rent-exemption (Solana) ou o gas (EVM), o broadcast falha
**antes de sair da rede de origem**, e o worker reembolsa automaticamente
(`crates/worker/src/dex_swap_runner.rs`, `mark_failed` + `refund`, ambos no
mesmo `wallet_id` que foi debitado — sem perda para o usuário).

`RelayIntentStatus::is_failed()` (`crates/relay/src/lib.rs`) reconhece os
valores reais da API do Relay: `failure` e `refund` — **não** `refunded`. Se
um valor de status novo aparecer e não for reconhecido, a operação fica presa
em `IN_FLIGHT` silenciosamente; ao investigar um caso assim, comparar contra
`docs.relay.link/references/api/get-intents-status-v3`.

`GET /v1/swap/history` expõe `error` (texto interno) quando `status` é
`FAILED`/`REFUNDED`. O client (`explainSwapError` em `SwapPage.tsx`) traduz
para um motivo curto — nunca mostrar o texto cru de RPC/provedor para o
usuário final, só em ferramenta de suporte/admin.

Consultar rapidamente o motivo de um reembolso em produção (somente leitura):
```sql
SELECT id, status, error, swap_payload->>'requestId' AS relay_request_id
FROM dex_swaps WHERE id = '<uuid>';
```
