# ADR 0006 — Lock na linha da wallet, não no agregado de ledger_entries

## Status
Aceito (corrige um bug introduzido na Fase 1).

## Contexto
`SELECT COALESCE(SUM(amount),0) FROM ledger_entries WHERE wallet_id=$1 FOR UPDATE` foi minha primeira implementação do lock de saldo. Parecia certo — travar as linhas que compõem o saldo antes de ler. Mas `FOR UPDATE` só trava linhas retornadas pela query; se a wallet ainda não tem nenhum `ledger_entry`, zero linhas são retornadas, e não há nada pra travar. Duas operações concorrentes na primeira credit/debit de uma wallet passam ambas pelo check sem serializar.

O bug ficou visível na Fase 5, ao portar `swap.service.ts`: o legado sempre trava a **wallet**, não o agregado (`wallet/domain/ledger.ts`, `lockWallet`), e um smoke test de swap expôs o problema de outra forma (violação de unique constraint, não a race em si, mas revisando o código percebi que o lock estava errado).

## Decisão
`db::ledger::lock_wallet`/`lock_wallets` travam `SELECT id FROM wallets WHERE id=$1 FOR UPDATE` — a linha da wallet sempre existe. `apply_ledger_entry` chama isso primeiro, antes de qualquer leitura de saldo.

## Consequência
Toda operação balance-changing paga o custo de um lock extra na tabela `wallets`, mas ganha a garantia real de serialização mesmo no primeiro lançamento de uma wallet nova.
