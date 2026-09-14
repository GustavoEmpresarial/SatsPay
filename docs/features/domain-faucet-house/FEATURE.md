# FEATURE — Faucet + inventário HOUSE

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `domain-faucet-house` |
| Título | Faucet + inventário HOUSE |
| Componente | `—` |
| Auth | **mixed** — pública ou autenticada conforme contexto |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`faucet claim HOUSE inventory cooldown Turnstile faucet_claim Sybil`

## Rotas

- (sem rota SPA — domínio backend)

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- `POST /v1/faucet/claim/:coin` (prefixo `/v1` no servidor)

## Arquivos-chave

- `crates/api-http/src/faucet.rs`
- `client/src/pages/FaucetPage.tsx`
- `crates/db/src/house.rs`

## Comportamento (bruto)

Debita HOUSE. Erro 'platform inventory insufficient' é esperado quando caixa vazia.

## Notas de overview legado

_sem overview em docs/pages_

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
