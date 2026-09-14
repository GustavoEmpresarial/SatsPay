# ADR 0003 — Transactional Outbox Pattern com Apache Kafka

## Status
Aceito.

## Contexto
Publicar mensagens diretamente no broker de mensageria durante o processamento de uma requisição HTTP introduz o problema de *dual-write*: se a transação do banco falha após o envio ao Kafka, terceiros consom dados inexistentes; se o banco confirma mas o envio ao Kafka falha, eventos são perdidos.

## Decisão
1. Toda alteração de estado no banco grava simultaneamente o evento de domínio na tabela `outbox_events` na mesma transação atômica.
2. Um daemon autônomo (`worker::outbox_relay`) consulta a tabela periodicamente e publica no Apache Kafka garantindo entrega *at-least-once*.
3. Mensagens recebem marcação `published_at` apenas após confirmação de recebimento (ACK) do cluster Kafka.

## Consequências
- Consistência eventual garantida entre o PostgreSQL e o barramento de eventos.
- Consumidores devem tratar potenciais mensagens duplicadas através do `id` único do evento.
