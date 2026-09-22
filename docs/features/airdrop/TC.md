# TC — Airdrop

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-airdrop-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-airdrop-02 | auth | Gate user; logs 401 sem token | [x] |
| TC-airdrop-03 | api | Award com ACTIVE incrementa; profile `season_active` | [x] |
| TC-airdrop-04 | api-neg | Sem ACTIVE → `NoActiveSeason`; profile false; zero pts | [x] |
| TC-airdrop-05 | i18n | Banner temporada inativa em pt | [x] |
| TC-airdrop-06 | obs | Award skip loga `info` (não silent debug-only) | [x] |
| TC-airdrop-07 | security | Logs self-only; sem userId IDOR param | [x] |
| TC-airdrop-08 | structure | Banner season inactive + structure | [x] |
| TC-airdrop-09 | integration | Claim FAUCET_CLAIM +50 com season ACTIVE | [x] |
| TC-airdrop-10 | referral | `amount_usd` > 0 quando passado; sem crédito wallet referrer | [x] |

## Security notes (checklist)

- [x] IDOR profile/logs — auth self only
- [x] Sem season → zero pontos (não inventar season fake)
- [x] Claim await award (sem spawn duplicando sob retry cego)
- [ ] Unique parcial activity+ref (gap documentado; cooldown mitiga faucet)
- [x] Sem secrets em point log descriptions

## Ops

- [ ] Prod: existe `airdrop_seasons` com `status='ACTIVE'`

## Automatizado

| Suite | Path |
|-------|------|
| Structure | `client/tests/unit/pages/airdrop/` |
| SQLx | `crates/db/tests/oauth_referral_airdrop_sqlx.rs` |
| API HTTP | `crates/api-http/tests/faucet_claim_http.rs` (overview + logs auth) |

## Critérios de aceite

- [x] `season_active` no profile; UI banner quando false
- [x] Docs FEATURE.md + TC.md atualizados nesta pasta
