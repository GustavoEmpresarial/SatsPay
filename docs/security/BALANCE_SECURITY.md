# Segurança e Integridade de Saldos (Balance Security)

Este documento estabelece o modelo formal de segurança que rege todos os cálculos e manipulações de saldos no ecossistema **BitcoSats**.

---

## 1. O Problema das Tabelas com Coluna de Saldo

Em sistemas tradicionais com colunas mutáveis de saldo (ex: `UPDATE users SET balance = balance + 100`):
1. **Condições de Corrida (*Race Conditions*)**: Duas operações simultâneas podem ler o mesmo saldo anterior e sobrescrever uma à outra, causando perda de fundos ou saques duplicados (*double spend*).
2. **Falta de Rastreabilidade Contábil**: Uma coluna de saldo não explica a procedência de cada centavo.
3. **Vulnerabilidade a Injeção/Manipulação**: Qualquer bug que execute um `UPDATE` indevido corrompe irreversivelmente o estado financeiro.

---

## 2. A Solução BitcoSats: Razão Contábil Puro (*Append-Only*)

No BitcoSats, o saldo é **sempre** uma função matemática pura sobre os lançamentos imutáveis do ledger:

$$\text{Saldo}(\text{wallet\_id}) = \sum_{e \in \text{ledger\_entries}} e.\text{amount}$$

### 2.1 Por que este modelo é matematicamente seguro?

1. **Imutabilidade**: A tabela `ledger_entries` não recebe `UPDATE` nem `DELETE`. Novos eventos apenas adicionam linhas.
2. **Serialização Pessimista Estrita**: Antes de qualquer operação de débito ou crédito, o sistema trava a linha da carteira no PostgreSQL:
   ```sql
   SELECT id FROM wallets WHERE id = $1 FOR UPDATE;
   ```
   Isso garante que duas transações simultâneas na mesma carteira sejam executadas estritamente em série.
3. **Idempotência por Chave Única**: Retentativas de requisições de rede ou reprocessamentos de mensagens nunca causam lançamentos duplicados devido aos índices únicos parciais (`uq_ledger_reference_type_dedup` e `uq_ledger_reference_key_dedup`).

---

## 3. Prevenção de Saldo Negativo e Ataques de Concorrência

Todo fluxo de débito segue o algoritmo invariante abaixo implementado em `crates/db/src/ledger/`:

```rust
// 1. Iniciar transação PostgreSQL
let mut tx = pool.begin().await?;

// 2. Travar a carteira (Serialização)
sqlx::query("SELECT id FROM wallets WHERE id = $1 FOR UPDATE")
    .bind(wallet_id)
    .execute(&mut *tx)
    .await?;

// 3. Calcular saldo atual real
let current_balance: BigDecimal = sqlx::query_scalar(
    "SELECT COALESCE(SUM(amount), 0) FROM ledger_entries WHERE wallet_id = $1"
)
.bind(wallet_id)
.fetch_one(&mut *tx)
.await?;

// 4. Validar se saldo >= débito necessário
if current_balance < debit_amount {
    return Err(LedgerError::InsufficientBalance); // Aborta e faz ROLLBACK automático
}

// 5. Inserir lançamento de débito (amount negativo)
sqlx::query(
    "INSERT INTO ledger_entries (wallet_id, amount, type, reference_id, reference_type)
     VALUES ($1, $2, $3, $4, $5)"
)
.bind(wallet_id)
.bind(-debit_amount)
.bind(ledger_type)
.bind(reference_id)
.bind(reference_type)
.execute(&mut *tx)
.await?;

// 6. Confirmar transação atomicamente
tx.commit().await?;
```

---

## 4. Auditoria Contábil Automática

Como todo lançamento possui data, carteira de origem/destino, tipo e referência ao evento causador, a plataforma pode ser auditada a qualquer momento comparando os depósitos on-chain reais com as somas das carteiras dos usuários e a carteira de tesouraria `HOUSE`.
