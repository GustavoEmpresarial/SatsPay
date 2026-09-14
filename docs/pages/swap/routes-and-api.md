# Swap — Rotas e API

## Client

- `/swap`

## Backend

| Método | Path |
|--------|------|
| GET | `/v1/wallet?kind=PERSONAL` |
| GET | `/v1/swap/prices` |
| GET | `/v1/swap/quote` |
| GET | `/v1/swap/history` |
| POST | `/v1/swap` (e legado `/v1/swap/execute`) |

## Notas

- `SWAPKIT_ENABLED=false` → só **SatsPay Liquidity** (HOUSE)
- UI deve mostrar taxa plataforma + provedor
