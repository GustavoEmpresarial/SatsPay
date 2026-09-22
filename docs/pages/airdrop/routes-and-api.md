# Airdrop — Rotas e API

## Client

- `/airdrop`

## Backend

| Método | Path | Notas |
|--------|------|-------|
| GET | `/v1/airdrop/overview` | Profile + `season_active` |
| GET | `/v1/airdrop/profile` | Alias de overview |
| GET | `/v1/airdrop/leaderboard` | Season ACTIVE only |
| GET | `/v1/airdrop/logs` | Self-only; filtrado por season ACTIVE |
| GET | `/v1/airdrop/history` | Alias de logs |

## Profile fields

- `season_active: boolean` — false quando não há row ACTIVE (UI: banner “Temporada inativa”)
- `total_points`, tiers, rank — zerados se season inativa

## Critérios mínimos

- Rota montada em `App.tsx`
- Sem crash no mount sem dados
- Awards (`FAUCET_CLAIM`, etc.) exigem season ACTIVE — sem silent no-op
