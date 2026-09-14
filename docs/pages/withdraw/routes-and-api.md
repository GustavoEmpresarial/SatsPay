# Withdraw — Rotas e API

## Client

- `/withdraw`

## Backend

| Método | Path |
|--------|------|
| GET | `/v1/wallet?kind=PERSONAL` |
| GET | `/v1/withdrawals/history` |
| POST | `/v1/withdrawals` (+ idempotencyKey) |

## Segurança / financeiro

- Idempotência obrigatória
- Validar endereço client+server
- Threshold de aprovação admin por coin
