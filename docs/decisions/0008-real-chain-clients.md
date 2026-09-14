# ADR 0008 — Integração on-chain real (item 1 do threat-model)

## Status
Aceito — leitura e broadcast pras 5 moedas. Broadcast com fundos reais ainda não exercitado em mainnet; caminho de testnet disponível via `CHAIN_NETWORK=testnet`.

## Contexto
O `StubClient` (fase 1) nunca foi substituído por integração real durante as fases 0–8. Esse era o maior item do `threat-model-and-gaps.md`.

## Decisão

**APIs públicas, sem chave, usadas em produção real (verificadas ao vivo durante o desenvolvimento):**
- BTC/LTC/DOGE/BCH: `https://api.bitcore.io` (Bitpay/Bitcore, API Insight-style) — uma única API cobre as 4 moedas UTXO com o mesmo formato de resposta. Path `/api/{COIN}/mainnet|testnet/…` (BTC testnet4 sob `testnet` — verificado ao vivo).
- POL: qualquer RPC JSON-RPC público do Polygon — `eth_getBalance` / `eth_getTransactionCount` / `eth_gasPrice` / `eth_estimateGas` / `eth_sendRawTransaction`. Mainnet chain id **137**; testnet atual = **Amoy (80002)** — constantes EIP-155 em `params_for`.

**Rede (`CHAIN_NETWORK`)**: quando `USE_REAL_CHAIN_CLIENTS=true`, `CHAIN_NETWORK` é **obrigatória** (`mainnet` | `testnet`) — sem default implícito. Controla path Bitcore, HRPs/version bytes de endereço e chain id POL. Ver `docs/operations/env-vars-reference.md`.

**Derivação de endereço**: BIP32 HD real a partir de um xpub (nunca a chave privada). Índice via sequência Postgres por moeda (não hash do user_id). Hot wallet: WIF → endereço via `params_for(coin, network)` (testnet WIF/version byte tratado por `PrivateKey::from_wif`).

**Uma única xpub reaproveitada entre as 5 moedas**: derivação não-hardened de um xpub produz a mesma chave pública secp256k1; só a *codificação* do endereço muda. Manter xpub-only (nunca expor xprv) foi priorizado sobre BIP44 à risca.

**Codificação de endereço**: base58check, bech32 P2WPKH, CashAddr (BCH), EIP-55 (POL) — vetores oficiais cobertos em testes. Testnet: BTC `tb`/0x6f, LTC `tltc`/0x6f, DOGE 0x71, BCH `bchtest` (chainparams / CashAddr registry).

**Bug real encontrado**: a API do bitcore rejeita silenciosamente o endereço BCH com prefixo `bitcoincash:` — precisa da forma bare. Corrigido no client Bitcore.

**Broadcast de saque**: implementado pras **5 moedas**:
- BTC/LTC/DOGE — P2WPKH via `btc_sign`.
- BCH — P2PKH + `SIGHASH_ALL|SIGHASH_FORKID` (0x41) em `bch_sign` (digest estilo BIP143; o crate `bitcoin` não cobre FORKID).
- POL — tx legacy EIP-155 + RLP em `evm_sign`; nonce / gasPrice / gasLimit sempre do RPC ao vivo; `chain_id` de `params_for` (137 / 80002).

Fee UTXO vem de `FEE_CONFIRMATION_TARGET` (env) + endpoint Bitcore `/{network}/fee/{target}` — sem sat/vB inventado.

Caminho de assinatura: `cargo run -p chain --example broadcast_sign_smoke` (não envia fundos).
Broadcast testnet: `cargo run -p chain --example broadcast_testnet_smoke` (`CHAIN_NETWORK=testnet`, hot WIF fundado).

**Não testado com fundos mainnet**: leitura verificada ao vivo contra mainnet. Preferir exercitar broadcast em testnet (`broadcast_testnet_smoke`) antes de mainnet.

## Consequência

`ChainRegistry::from_env` decide stub vs real via `USE_REAL_CHAIN_CLIENTS` e exige `CHAIN_NETWORK` no path real. Antes de mover fundos de mainnet: (1) testar broadcast das 5 moedas com hot wallet fundada em testnet, (2) considerar migrar de API pública para nó próprio ou provedor com SLA.
