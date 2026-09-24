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

## Resiliência (2026-09-23)

- DOGE tenta BlockCypher, Bitcore e o Blockbook público `dogecoin.atomicwallet.io`, sem credencial. `DOGE_BLOCKBOOK_API` permite trocar o host e `DOGE_BLOCKBOOK_API_KEY` envia uma chave opcional apenas no header `api-key`, para provedores como NOWNodes. O antigo fallback SoChain foi removido: devolvia lista vazia sem interpretar depósitos. DGB mantém RPC próprio, dois Insight e Blockbook. O nó DGB e os indexadores abrem o circuit breaker após três erros, esperam 60 s e aceitam uma sondagem para fechar.
- Em produção, o signer exige `DEPOSIT_MNEMONIC_ENC`/arquivo e publica a cada 30 s um marcador de coerência no PostgreSQL. O fingerprint inclui rede, xpubs e endereços hot com comparação exata. A API retorna 503 em `/healthz` e bloqueia endereços pessoais e merchant quando o marcador está ausente, divergente ou há mais de 90 s sem heartbeat.
- Falha de todos os provedores de depósitos por três ciclos e falha de sweep por três ciclos geram alerta por moeda. Um ciclo bem-sucedido zera a sequência. Resposta JSON inválida é falha, mesmo com HTTP 200.
- ZER consulta ZeroChain público e transmite transações já assinadas pelo Insight público, com `https://zerochain.info/api/rawtx/{raw}/{api-key}` como reserva quando `ZER_EXPLORER_API_KEY` está configurada. A ZeroChain é API REST pública, não JSON-RPC. `rawtxbuild` exige a chave privada na URL e não é usado. `ZER_RPC_URL` fica só no worker e só aceita destino local ou privado para `signrawtransaction` com WIF. Validar a assinatura no nó próprio antes de liberar saques/sweeps enquanto ele sincroniza.
