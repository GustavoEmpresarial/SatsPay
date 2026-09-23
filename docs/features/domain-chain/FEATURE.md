# FEATURE — Integração on-chain

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `domain-chain` |
| Título | Integração on-chain |
| Componente | `—` |
| Auth | **mixed** — pública ou autenticada conforme contexto |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`BTC LTC DOGE BCH POL DGB SOL USDT USDC ZER RPC HD xpub sweep hot wallet t1 zerod`

## Rotas

- (sem rota SPA — domínio backend)

## Abas / seções internas

- (página sem abas internas)

## APIs usadas (client → `/v1…`)

- (sem chamadas `api()` na página / domínio)

## Arquivos-chave

- `crates/chain/`
- `docs/architecture/chain-integration.md`

## Comportamento (bruto)

Clientes RPC / explorers; hot + deposit addresses; ZER só t1 via zerod (sem z-addr, sem SwapKit). DGB: node RPC → Insight (digiexplorer, digibyte.host) → Blockbook (digibyte.atomicwallet.io) para UTXO, taxa, saldo e broadcast; Blockbook não traz scriptPubKey — `fill_missing_scripts` deriva do endereço. Mensagens de erro EVM usam `rpc_host()` (sem path/query: chaves de provedor ficam fora de logs).

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
