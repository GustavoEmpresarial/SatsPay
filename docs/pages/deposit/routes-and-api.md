# Deposit — Rotas e API

## Client

- `/deposit`

## Backend

| Método | Path | Uso |
|--------|------|-----|
| GET | `/v1/wallet?kind=PERSONAL` | Carteiras / address cache |
| GET | `/v1/deposits/address/:coin` | Endereço HD |
| GET | `/v1/deposits/history` | Histórico |

## Segurança

- Endereço derivado server-side (xpub); client só exibe QR/copia
