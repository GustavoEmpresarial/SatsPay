# ADR 0002 — Razão Contábil Imutável sem Coluna de Saldo

## Status
Aceito.

## Contexto
Tabelas com coluna `balance` atualizadas via `UPDATE wallets SET balance = balance + X` são a principal causa de falhas contábeis, double-spending e perda de rastreabilidade em sistemas de custódia cripto.

## Decisão
1. Eliminar qualquer coluna de saldo mutável no banco de dados.
2. Saldo é estritamente derivado via `SUM(amount)` da tabela imutável `ledger_entries`.
3. Todo lançamento é auditável com tipo, timestamp, referência única (`reference_id` ou `reference_key`) e carteiras de contraparte (`HOUSE`, `LEND_POOL`, etc.).

## Consequências
- Impossibilidade matemática de "criar dinheiro do nada" por inconsistência em `UPDATE`.
- Auditoria contábil completa disponível a qualquer momento.
- Toda escrita exige a trava pessimista na linha da carteira para garantir a serialização dos lançamentos.
