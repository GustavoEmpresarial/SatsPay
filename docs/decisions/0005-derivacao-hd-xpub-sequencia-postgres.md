# ADR 0005 — Derivação de Endereços HD via xpub e Sequências PostgreSQL

## Status
Aceito.

## Contexto
O legado utilizava hashes do UUID do usuário para gerar caminhos de derivação de carteira HD. Essa abordagem não garantia ausência de colisão e desviava dos padrões da indústria (BIP32 / BIP44). Além disso, ter a chave privada (`xprv`) no servidor HTTP gerava alto risco em caso de invasão.

## Decisão
1. O servidor web (`api-server`) armazena e utiliza **apenas a chave pública estendida (`xpub`)** mestra.
2. Cada moeda possui uma sequência atômica no PostgreSQL (`btc_hd_index_seq`, `ltc_hd_index_seq`, etc.).
3. O endereço da carteira é gerado pelo índice sequencial `m/0/index`, garantindo ordem estrita e ausência de colisão.

## Consequências
- Impossibilidade de movimentação de fundos mesmo em caso de comprometimento do `api-server`.
- Conformidade total com a derivação BIP32/BIP44.
