# FEATURE — Faucet

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Truth = código (não o gerador sozinho).

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `faucet` |
| Título | Faucet |
| Componente | `FaucetPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`faucet FaucetPage /faucet /faucet/claim/:coin FAUCET_CLAIM HOUSE Turnstile user`

## Rotas

- `/faucet`

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `POST /v1/faucet/claim` (body `{ coin, captchaToken }`)
- `POST /v1/faucet/claim/:coin` (path coin + captchaToken)
- Aliases legados intencionais: `/faucet/claim`, `/faucet/claim/:coin` (sem `/v1`)

## Arquivos-chave

- `client/src/pages/FaucetPage.tsx`
- `crates/api-http/src/faucet.rs`
- `crates/db/src/faucet.rs`
- `docs/pages/faucet/`

## Comportamento (bruto)

1. Turnstile action `faucet_claim`; cooldown 11h (660 min) **por user+coin** em `faucet_claims` (tabela no Postgres). O fingerprint de IP é gravado na linha só para auditoria — **não** bloqueia outra conta na mesma rede.
2. Claim debita HOUSE e credita ledger `type=FAUCET` com **`faucet_reward = 1` sat** (unidade mínima) — produto travado; não aumentar.
3. Na mesma transação do crédito: `award_airdrop_points_tx(…, FAUCET_CLAIM, +50)` se season `ACTIVE`. Sem temporada, o claim credita a carteira e devolve `seasonActive: false`. Resposta inclui `pointsAwarded` / `seasonActive`. Claims da temporada sem log são reparados no boot do worker (`backfill_missed_airdrop_points`).
4. Referral: `record_referral_commission` com `amount_usd` = price × commission amount (ranking/log). **Não** credita ledger do referrer nesta fase.
5. UI mostra amount na moeda (nunca fingir `$0.00` se houve crédito — ver Dashboard/Analytics dust).
6. Relógio de 11h: `GET /v1/faucet/status` lê `faucet_claims` **por user**. A página espera o status do servidor; sem `localStorage`.

## Security notes

- Double-claim / race: advisory lock + cooldown em `db::faucet::claim` (HTTP test `faucet_claim_double_race_one_wins`).
- Captcha obrigatório no body; produção verifica Turnstile (dev: `disabled_in_dev`).
- HOUSE: sem crédito sem debit; floor inventory.
- Sem secrets em audit logs de claim (só coin/amount).
- Rate limit / fee-margin pause podem bloquear claim (`FEE_MARGIN_NEGATIVE`).

## Bugs / armadilhas conhecidas

- Relógio da UI antigo (`bitcosats_faucet_v2_*` no `localStorage`) não era recarregado do servidor e podia ficar preso na chave `guest`. Fonte agora é `GET /faucet/status`.
- 1 sat → USD hero arredonda a `$0.00` se UI não usar `formatPortfolioUsd` / `formatUsdValue`.
- Docs antigos diziam `/claim/:id` — rota real é `:coin`.
- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger SUM.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
- Airdrop points: [`../airdrop/FEATURE.md`](../airdrop/FEATURE.md)
