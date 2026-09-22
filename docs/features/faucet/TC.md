# TC — Faucet

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
| TC-faucet-01 | smoke | Abrir rota(s) e renderizar sem crash | [ ] |
| TC-faucet-02 | auth | Gate user: anônimo / usuário / admin conforme esperado | [x] |
| TC-faucet-03 | api | Claim → amount=1, ledger FAUCET, wallet SUM | [x] |
| TC-faucet-04 | api-neg | 401 unauth; captchaToken obrigatório; cooldown 429 | [x] |
| TC-faucet-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR) | [ ] |
| TC-faucet-06 | obs | Cooldown esperado não flooda telemetria | [ ] |
| TC-faucet-07 | security | Race double-claim → 1 FAUCET; HOUSE debit; captcha field | [x] |
| TC-faucet-08 | structure | `client/tests/unit/pages/faucet/` | [x] |
| TC-faucet-09 | system | claim → GET /wallet → GET /airdrop/overview | [x] |
| TC-faucet-10 | ui | Dust credit: não mostrar `$0.00` falso (Analytics/Dashboard) | [x] |

## Security notes (checklist)

- [x] Double-claim / race — advisory lock + HTTP concurrency test
- [x] Captcha token field required (prod Turnstile)
- [x] HOUSE debit before user credit
- [x] Sem secrets em audit payload de claim
- [ ] Rate-limit edge em staging (manual)

## Automatizado

| Suite | Path |
|-------|------|
| Structure | `client/tests/unit/pages/faucet/` |
| Dust / wallets parse | `client/tests/unit/shared/coins.logic.test.ts`, `analytics.structure.test.ts` |
| API HTTP | `crates/api-http/tests/faucet_claim_http.rs` |
| SQLx | `crates/db/tests/swap_faucet_sqlx.rs`, `oauth_referral_airdrop_sqlx.rs` |

## Critérios de aceite

- [x] Rotas `:coin` (não `:id`) batem com Axum
- [x] Reward = 1 sat; UI perceptível via coin units / dust formatters
- [x] Docs FEATURE.md + TC.md atualizados nesta pasta
