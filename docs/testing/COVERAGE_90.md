# Cobertura do monorepo (piso CI ≥90% — **não** é 100% global)

> **Honestidade:** 100% **não** é a cobertura do monorepo.  
> O que chegou a **100% de linhas** (llvm-cov) foi só o módulo de suporte:
> - `crates/db/src/support.rs` → 100% lines / functions  
> - `crates/api-http/src/support.rs` → 100% lines / functions / regions  
> Em remediação **focada** (não pacote): também `csrf`, `notify_email`, `state`.  
> Todo o resto abaixo é o estado real medido (≈90% gate, `api-http` ainda <100).

## Realidade (medido)

| Escopo | Lines | Gate |
|--------|------:|------|
| Client `src/**` | **~90%** | CI ≥90 |
| `shared`+`crypto`+`domain` | **~97.5%** | CI ≥90 |
| `pricing` | **~95%** | CI ≥90 (`--tests`) |
| `swapkit` | **~90.6%** | CI ≥90 |
| `captcha` | **~90%** | CI ≥90 |
| `queue` | **100%** | CI ≥90 |
| `events` (sem producer/relay Kafka) | **~95%** | CI ≥90 |
| `chain` (sem RPC/real/sol clients) | **~94%** | CI ≥90 (ignore regex) |
| `db` (pacote inteiro) | **~90%** | CI ≥90 (`--lib --tests`) |
| `db::support` only | **100%** lines | meta módulo |
| `api-http` (pacote inteiro) | **~87–90%** | remedir: `cov_lowmem.sh api-http` em andamento |
| `api-http::support` only | **100%** lines | meta módulo |

## CI

- `rust-llvm-cov`: fail-under **90** (não 100) em shared/crypto/domain + pricing/swapkit/captcha/queue + events (ignore Kafka) + chain (ignore RPC) + **db**
- `api-http`: summary-only no CI — **sem** fail-under 100
- `rust-db-sqlx`: testes sqlx (Postgres service)

## Máquinas com pouca RAM (~14 GiB)

`llvm-cov` + vários `rustc` em paralelo estoura RAM e cai em swap → UI congela.

Mitigações locais:

1. **mold** + `[build] jobs = 2` em `~/.cargo/config.toml`
2. `scripts/cov_lowmem.sh` (`-j 2`, `RUST_TEST_THREADS=2`)
3. **zram** ativo

```bash
# só suporte (este sim tem fail-under 100 nos dois arquivos)
./scripts/cov_lowmem.sh support

# pacote api-http inteiro (ainda NÃO é 100%)
./scripts/cov_lowmem.sh api-http
```

## Novos testes (empurram api-http)

1. `coverage_to_100_http.rs`
2. `admin_faucet_notify_http.rs` / `oauth_token_negatives_http.rs` / `stake_lend_edges_http.rs`
3. `admin_rewards_http.rs` / `swap_quote_validation_http.rs`
4. Units: `csrf` (cross-site), `rate_limit` (window reset)
5. Client: `coverageTo100.nav.support.test.tsx` + `db/tests/support_extra_sqlx.rs`

## Ainda faltando

1. Gaps em merchant_deposits / status / wallet / swap execute
2. Remedir pacote inteiro e atualizar a tabela com o TOTAL real
