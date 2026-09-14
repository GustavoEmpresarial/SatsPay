# Faucet — Rotas e API

## Client

- `/faucet`

## Backend

| Método | Path |
|--------|------|
| POST | `/v1/faucet/claim/:coin` (+ Turnstile) |

## Regras

- Cooldown **11h** (660 min) por user/IP
- Captcha action faucet
- Ruído esperado de cooldown **não** deve poluir telemetria de erros
