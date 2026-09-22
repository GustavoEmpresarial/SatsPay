# Faucet — Rotas e API

## Client

- `/faucet`

## Backend

| Método | Path | Notas |
|--------|------|-------|
| GET | `/v1/faucet/status` | Relógio de 11h por moeda (`nextClaimAt`). Alias `/faucet/status`. |
| POST | `/v1/faucet/claim` | Body `{ coin, captchaToken }` |
| POST | `/v1/faucet/claim/:coin` | Path coin + captchaToken |
| POST | `/faucet/claim` · `/faucet/claim/:coin` | Aliases legados (compat) |

## Resposta claim (200)

```json
{
  "amount": "1",
  "coin": "BTC",
  "nextClaimAt": "…",
  "pointsAwarded": true,
  "seasonActive": true
}
```

- `amount` = reward em unidades ledger (1 sat).
- `pointsAwarded` / `seasonActive` refletem airdrop no mesmo request (await).

## Regras

- Cooldown **11h** (660 min) por user/IP → HTTP 429 `FAUCET_COOLDOWN`
- Captcha action `faucet_claim`
- HOUSE debit + ledger `FAUCET`; saldo só via SUM
- Ruído esperado de cooldown **não** deve poluir telemetria de erros
