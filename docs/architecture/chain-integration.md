# Integração com Blockchain e Clientes On-Chain

A crate `chain` (`crates/chain/`) é responsável por toda a comunicação com redes blockchain públicas, derivação determinística de endereços de depósito, consulta de saldos e UTXOs, estimativa de taxas e montagem/assinatura offline de transações de saque.

---

## 1. Moedas Suportadas e Redes

| Moeda | Símbolo | Tipo de Rede | Padrão de Endereço | Provedor / RPC |
|---|---|---|---|---|
| **Bitcoin** | `BTC` | UTXO | Bech32 (P2WPKH / SegWit Nativo) `bc1q...` / `tb1q...` | Bitcore Insight API / Nó Próprio |
| **Litecoin** | `LTC` | UTXO | Bech32 (P2WPKH) `ltc1q...` / `tltc1q...` | Bitcore Insight API / Nó Próprio |
| **Dogecoin** | `DOGE` | UTXO | Base58Check (P2PKH) `D...` / `n...` | Bitcore Insight API / Nó Próprio |
| **Bitcoin Cash** | `BCH` | UTXO (ForkId) | CashAddr `q...` / `bchtest:...` | Bitcore Insight API / Nó Próprio |
| **Polygon (POL)** | `POL` | EVM (Account) | Hexadecimal EIP-55 `0x...` | JSON-RPC Polygon (ChainId 137 / 80002) |
| **Zero** | `ZER` | UTXO (Zcash-family) | Transparent P2PKH `t1…` apenas (2-byte Base58Check). `t3` / `z*` rejeitados. | `zerod` JSON-RPC (`ZER_RPC_URL`) + fallback `zerochain.info` |
| **Pepe** | `PEPE` | BEP-20 na BNB Smart Chain (chain id 56) | Hexadecimal EIP-55 `0x…` | JSON-RPC BSC (`BSC_RPC_URL`). Contrato `0x25d887Ce7a35172C62FeBFD67a1856F20FaEbB00` (18 casas). Gas em **BNB**, não em PEPE. Testnet recusado. Não é o PEPE da Ethereum (`0x6982…`). |

---

## 2. Derivação Determinística de Endereços (HD Wallet)

O sistema utiliza derivação hierárquica determinística (**BIP32**) a partir de uma **chave pública estendida (`xpub`)** mestra por moeda (ou compartilhada entre moedas secp256k1):

```text
xpub Mestra (Somente Leitura no api-server)
      │
      ├── BTC  Seq: btc_hd_index_seq  ──> m/0/1 -> m/0/2 -> m/0/N
      ├── LTC  Seq: ltc_hd_index_seq  ──> m/0/1 -> m/0/2 -> m/0/N
      ├── DOGE Seq: doge_hd_index_seq ──> m/0/1 -> m/0/2 -> m/0/N
      ├── BCH  Seq: bch_hd_index_seq  ──> m/0/1 -> m/0/2 -> m/0/N
      ├── POL  Seq: pol_hd_index_seq  ──> m/0/1 -> m/0/2 -> m/0/N
      ├── PEPE Seq: pepe_hd_index_seq ──> m/44'/60'/0'/0/{i} (mesmo endereço 0x da hot EVM; gas em BNB)
      └── ZER  Seq: zer_hd_index_seq  ──> m/44'/323'/0'/0/{i} (`t1`)
```

### 2.1 Por que Sequência Postgres em vez de Hash de User ID?
Conforme o [ADR 0008](file:///home/gustavo/Documentos/BitcoSats/current/docs/decisions/0008-real-chain-clients.md), a derivação usa sequências atômicas do PostgreSQL (`*_hd_index_seq`). Isso garante:
1. Índices estritamente sequenciais e sem colisões.
2. Conformidade com os padrões de derivação de carteiras HD da indústria.
3. Segurança: o servidor web (`api-server`) possui apenas a chave pública (`xpub`), sendo incapaz de assinar ou mover fundos.

---

## 3. Máquina de Estados do Saque e Assinatura

```mermaid
stateDiagram-v2
    [*] --> PENDING: Usuário solicita saque (2FA)
    PENDING --> APPROVED: Valor < Threshold ou Aprovado pelo Admin
    PENDING --> CANCELED: Rejeitado pelo Admin (Reverte Ledger)
    APPROVED --> QUEUED: Enfileirado em internal_jobs
    QUEUED --> BROADCASTING: Worker assume a tarefa
    BROADCASTING --> BROADCASTED: Transação assinada e enviada para a rede (txHash registrado)
    BROADCASTING --> FAILED: Erro permanente na rede (Reverte Ledger)
    BROADCASTED --> CONFIRMED: N confirmações atingidas no bloco
```

### 3.1 Assinatura Segura por Moeda
- **BTC, LTC, DOGE**: Assinatura P2WPKH usando curvas secp256k1.
- **BCH**: Assinatura P2PKH com algoritmo de digest BIP143 e flag obrigatória `SIGHASH_FORKID` (0x41).
- **POL / USDT / USDC**: Transação EVM assinada no padrão EIP-155 com RLP encoding, consultando nonce e gas price em tempo real via JSON-RPC da Polygon. Gas em POL (piso 30 gwei).
- **PEPE**: Mesma assinatura EIP-155, chain id **56**, contrato BEP-20 na BNB Smart Chain. Gas price ao vivo com piso 0,05 gwei. Antes de assinar, a hot precisa ter BNB `>= gas_price × gas_limit`; se faltar, o saque reverte no ledger e nada vai para a rede. Sweep de depósito faz top-up de BNB (`gas_price × limite × 2`) a partir da hot.
- **ZER**: raw montado no `zerod` (`createrawtransaction`) e assinado no nó (`signrawtransactionwithkey` / `signrawtransaction`). **Não** reusar o signer Bitcoin (sighash Zcash/ZIP-243). Sem `ZER_RPC_URL` o saque falha fechado. SatsPay **não** custodia shielded (`z…`).

---

## 4. Modos de Operação: Stub vs Real

Controlado pelas variáveis de ambiente:
- `USE_REAL_CHAIN_CLIENTS=true`: Ativa comunicação real com nós de blockchain.
- `ALLOW_STUB_CHAIN=false`: Bloqueia o uso de geradores falsos de endereço em produção (fail-closed).
- `CHAIN_NETWORK`: Define se a rede alvo é `mainnet` ou `testnet`.
