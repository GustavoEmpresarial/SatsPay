# Dashboard — Rotas e API

## Client

- `/dashboard` (RequireAuth + AppLayout)

## Backend (via client `api`)

| Método | Path | Uso |
|--------|------|-----|
| GET | `/v1/wallet?kind=PERSONAL` | Saldos |
| GET | `/v1/swap/prices` | USD (skipAuth ok) |
| GET | `/v1/wallet/ledger?take=15` | Atividade recente |

## Aceite

- Loading sem crash com arrays vazios
- Preços stale não derrubam a página
