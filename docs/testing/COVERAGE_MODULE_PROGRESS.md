# Progresso cobertura api-http (módulo a módulo)

Meta: subir `api-http` de ~87% → alto (90%+; 100% só onde fechável).
CI do monorepo continua fail-under **90**, não 100% global.

## Fechado nesta sessão (testes **passando**)

| Módulo | Suíte | Status |
|--------|-------|--------|
| `support` | `support_http` + unit `map_err` | ✅ 100% lines (medido antes) |
| `faucet` / withdrawals / merchant_deposits / treasury | `coverage_to_100_http` | ✅ 10/10 |
| `oauth` edges + PKCE/redirect units | `oauth_coverage_edges_http` + `--lib` | ✅ |
| `notify_email` | units | ✅ 100% lines (foco) |
| `admin` faucetlist notify + CONFLICT | `admin_faucet_notify_http` | ✅ 1/1 |
| `oauth` token + userinfo negatives | `oauth_token_negatives_http` | ✅ 1/1 |
| `stake` claim/invalid + `lend` supply/Err | `stake_lend_edges_http` | ✅ 2/2 |
| `csrf` | units (cross-site + reject) | ✅ 100% lines (foco) |
| `rate_limit` | units (+ window reset) | ✅ ~86% lines (foco) |
| `admin` rewards programs | `admin_rewards_http` | ✅ 1/1 |
| `swap` quote validation | `swap_quote_validation_http` | ✅ 1/1 |
| deposit/withdraw/merchant pause | `deposit_withdraw_pause_http` + client unit | ✅ BTC/LTC/DOGE/BCH/DGB 503; POL/SOL/ZER ativos; `/public/send` aberto |

## Próximos

1. Remedir pacote: `./scripts/cov_lowmem.sh api-http` (**um** job)
2. Atualizar tabela em `COVERAGE_90.md` com percentual real do pacote

## Nota de medição

Remedição **focada** (lib + 4 suítes) ≠ cobertura do pacote. Número focado ~42% é artefato de poucos testes.
Não reclamar 100% do monorepo.
