# Invariantes e Regras de Integridade do Ledger

O ledger do **BitcoSats** é regido por invariantes matemáticos e relacionais estritos que garantem a impossibilidade de criação indevida de moedas, saldo negativo acidental ou perda de rastreabilidade.

---

## 1. Invariantes Matemáticos

### 1.1 Invariante de Não-Negatividade de Carteiras Pessoais
Para qualquer carteira $W$ onde $W.\text{kind} \in \{\text{'PERSONAL'}, \text{'DEVELOPER'}\}$:
$$\text{Saldo}(W) = \sum_{e \in \text{ledger\_entries}, e.\text{wallet\_id} = W.\text{id}} e.\text{amount} \ge 0$$

> Qualquer transação que resulte em $\text{Saldo}(W) < 0$ é imediatamente abortada com `ROLLBACK` e erro `InsufficientBalance`.

### 1.2 Invariante de Conservação em Transferências Internas
Em qualquer transferência interna entre duas carteiras $W_A$ e $W_B$ da mesma moeda:
$$\Delta \text{Saldo}(W_A) + \Delta \text{Saldo}(W_B) = 0$$
Onde o débito em $W_A$ (`TRANSFER_OUT`, $-X$) é estritamente igual ao crédito em $W_B$ (`TRANSFER_IN`, $+X$).

### 1.3 Invariante de Conservação de Câmbio (Swap)
Para uma troca de quantidade $A$ da moeda $C_1$ por quantidade $B$ da moeda $C_2$ com taxa $F$:
1. Na moeda $C_1$: $\Delta \text{Saldo}_{\text{user}}(C_1) = -A$ e $\Delta \text{Saldo}_{\text{house}}(C_1) = +A$.
2. Na moeda $C_2$: $\Delta \text{Saldo}_{\text{user}}(C_2) = +B$ e $\Delta \text{Saldo}_{\text{house}}(C_2) = -(B + F)$.

---

## 2. Invariantes Relacionais e Restrições de Integridade

### 2.1 Deduplicação e Idempotência de Lançamentos
Para evitar que retentativas de jobs ou de requisições de rede dupliquem lançamentos contábeis, o banco impõe os seguintes índices únicos parciais:

```sql
-- Deduplicação por referência UUID (Saques, Swaps, Stakes)
create unique index uq_ledger_reference_type_dedup 
on ledger_entries (wallet_id, reference_id, reference_type, type)
where reference_id is not null;

-- Deduplicação por chave composta textual (API Pública, Posições de Lend)
create unique index uq_ledger_reference_key_dedup 
on ledger_entries (wallet_id, reference_key, reference_type, type)
where reference_key is not null;
```

### 2.2 Protocolo de Lock de Linha (Pessimistic Concurrency)
Toda função de escrita no ledger deve executar:
```sql
SELECT id FROM wallets WHERE id = $1 FOR UPDATE;
```
como a primeira instrução da transação SQL, antes de executar qualquer leitura de saldo ou inserção em tabelas secundárias.
