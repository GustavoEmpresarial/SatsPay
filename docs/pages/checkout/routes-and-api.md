# Checkout — Rotas e API

## Client

- `/pay/:id` (público)

## Backend

| Método | Path |
|--------|------|
| GET | `/v1/public/pay/:id` |
| POST | `/v1/public/pay/:id/select-coin` |
| POST | `/v1/public/pay/:id/balance` |

Invoice merchant; prazo de depósito; pay-with-balance autenticado.

Rate class `public-pay` (por IP): GET 60/min, select-coin 20/min, balance 10/min. Aliases sem `/v1` iguais.
