# FEATURE — Airdrop

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Truth = código (não o gerador sozinho).

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `airdrop` |
| Título | Airdrop |
| Componente | `AirdropPage` |
| Auth | **user** — RequireAuth — usuário logado |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`airdrop AirdropPage SatsPoints FAUCET_CLAIM SWAP_EXECUTE season_active /airdrop user`

## Rotas

- `/airdrop`

## APIs usadas (client → `/v1…`)

- `GET /v1/airdrop/overview` (alias `/profile`) — inclui `season_active`
- `GET /v1/airdrop/leaderboard`
- `GET /v1/airdrop/logs` (alias `/history`) — filtrado pela season ACTIVE

## Arquivos-chave

- `client/src/pages/AirdropPage.tsx`
- `crates/db/src/airdrop.rs`
- `crates/api-http/src/airdrop.rs`
- `docs/pages/airdrop/`

## Comportamento (bruto)

1. Pontos só em season com `status='ACTIVE'`. Sem season: `award_airdrop_points` → `AwardResult::NoActiveSeason` (log info, **não** silent Ok vazio); profile `season_active: false`; UI banner “Temporada inativa”.
2. Activities: `FAUCET_CLAIM` (+50), `SWAP_EXECUTE` (+100), `REFERRAL_*` (+50), `DEPOSIT_CONFIRMED` (+100), `COMMISSION_EARNED` (+10).
3. Stake / LM rewards ≠ SatsPoints (sistemas distintos).
4. Faucet/swap await award no request (feedback `pointsAwarded`); não fire-and-forget.
5. Ops: verificar em prod row `airdrop_seasons` com `status='ACTIVE'`.

## Security notes

- Profile/logs: só `AuthUser` self — sem `userId` query (IDOR N/A); 401 sem auth.
- Award sob retry HTTP: claim cooldown impede double ledger; pontos inseridos uma vez por claim bem-sucedido (await). Gap: sem unique parcial activity+ref — não reprocessar awards em loop sem chave.
- Sem secrets em descriptions/logs de pontos.
- Referral commission `amount_usd` é ranking/log — **não** auto-credita wallet do referrer.

## Bugs / armadilhas conhecidas

- Awards de indicação/depósito anteriores a 2026-09-13 podem estar sem log — worker roda `backfill_missed_airdrop_points` no boot.
- Missão “saldo diário” **não** existe; UI lista faucet/swap/referral/depósito.
- ~~`award_airdrop_points` sem season ACTIVE é no-op silent~~ — corrigido: `NoActiveSeason` + banner.
- Não short-circuit hooks (`useA() || useB()`) — React #311.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
- Faucet: [`../faucet/FEATURE.md`](../faucet/FEATURE.md)
