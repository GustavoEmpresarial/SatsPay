# ADR 0004 — Fila Interna de Jobs em PostgreSQL via SKIP LOCKED

## Status
Aceito.

## Contexto
O sistema legado utilizava Redis e BullMQ para processamento de filas de background. Ter duas fontes de verdade para o estado de saques e tarefas (Postgres e Redis) gerava problemas frequentes de sincronização e exigia manter uma infraestrutura adicional de Redis em alta disponibilidade.

## Decisão
Substituir o Redis por uma tabela interna no PostgreSQL (`internal_jobs`) consumida por workers através do comando:
```sql
SELECT * FROM internal_jobs 
WHERE status = 'PENDING' AND run_after <= now() 
ORDER BY run_after 
FOR UPDATE SKIP LOCKED 
LIMIT 1;
```

## Consequências
- A criação de um job ocorre atomicamente na mesma transação que altera os dados de negócio (ex: solicitação de saque).
- Eliminação do componente Redis da stack de infraestrutura.
- Múltiplas réplicas do `worker` podem drenar a fila concorrentemente sem risco de colisão.
