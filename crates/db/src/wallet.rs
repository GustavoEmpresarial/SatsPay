//! Port 1:1 of legacy `apps/api/src/modules/wallet/{services,repositories}/wallet.*.ts`.

use crate::ledger::{apply_ledger_entry, LedgerCreditInput, LedgerError};
use bigdecimal::BigDecimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error("amount must be > 0")]
    InvalidAmount,
    #[error("wallet not found")]
    NotFound,
}

#[derive(Debug, Clone)]
pub struct WalletView {
    pub coin: String,
    pub address: Option<String>,
    pub balance: BigDecimal,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct LedgerEntryView {
    pub id: Uuid,
    pub coin: String,
    pub amount: BigDecimal,
    pub entry_type: String,
    pub memo: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Recent ledger rows for a user's PERSONAL (or other) wallets — newest first.
pub async fn list_ledger_entries(
    pool: &PgPool,
    user_id: Uuid,
    kind: &str,
    coin: Option<&str>,
    take: i64,
) -> Result<Vec<LedgerEntryView>, WalletError> {
    let take = take.clamp(1, 500);
    let rows = sqlx::query(
        r#"
        SELECT l.id, w.coin::text AS coin, l.amount, l.type::text AS entry_type, l.memo, l.created_at
        FROM ledger_entries l
        JOIN wallets w ON w.id = l.wallet_id
        WHERE w.user_id = $1
          AND w.kind = $2::wallet_kind
          AND ($3::text IS NULL OR w.coin::text = $3)
        ORDER BY l.created_at DESC
        LIMIT $4
        "#,
    )
    .bind(user_id)
    .bind(kind)
    .bind(coin)
    .bind(take)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LedgerEntryView {
            id: r.get("id"),
            coin: r.get("coin"),
            amount: r.get("amount"),
            entry_type: r.get("entry_type"),
            memo: r.get("memo"),
            created_at: r.get("created_at"),
        })
        .collect())
}

pub async fn list_wallets(pool: &PgPool, user_id: Uuid, kind: &str) -> Result<Vec<WalletView>, WalletError> {
    let rows = sqlx::query(
        r#"
        SELECT w.coin::text AS coin, w.address, w.kind::text AS kind,
               COALESCE(SUM(l.amount), 0) AS balance
        FROM wallets w
        LEFT JOIN ledger_entries l ON l.wallet_id = w.id
        WHERE w.user_id = $1 AND w.kind = $2::wallet_kind
        GROUP BY w.id
        ORDER BY w.coin
        "#,
    )
    .bind(user_id)
    .bind(kind)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| WalletView { coin: r.get("coin"), address: r.get("address"), kind: r.get("kind"), balance: r.get("balance") })
        .collect())
}

/// Moves funds between the same user's PERSONAL and MERCHANT wallets for one
/// coin. Gateway invoices credit MERCHANT. `to_developer = true` debits
/// PERSONAL and credits MERCHANT (the merchant caixa), and vice-versa.
pub async fn transfer_between_kinds(pool: &PgPool, user_id: Uuid, coin: &str, amount: BigDecimal, to_developer: bool) -> Result<(), WalletError> {
    use bigdecimal::Zero;
    if amount.is_zero() || amount.sign() == bigdecimal::num_bigint::Sign::Minus {
        return Err(WalletError::InvalidAmount);
    }
    let (from_kind, to_kind) = if to_developer { ("PERSONAL", "MERCHANT") } else { ("MERCHANT", "PERSONAL") };
    sqlx::query(
        "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, 'MERCHANT') \
         ON CONFLICT (user_id, coin, kind) DO NOTHING",
    )
    .bind(user_id)
    .bind(coin)
    .execute(pool)
    .await?;

    let mut tx = pool.begin().await?;

    let from_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = $3::wallet_kind")
        .bind(user_id)
        .bind(coin)
        .bind(from_kind)
        .fetch_optional(&mut *tx)
        .await?;
    let to_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = $3::wallet_kind")
        .bind(user_id)
        .bind(coin)
        .bind(to_kind)
        .fetch_optional(&mut *tx)
        .await?;
    let (Some(from_id), Some(to_id)) = (from_id, to_id) else { return Err(WalletError::NotFound) };

    crate::ledger::lock_wallets(&mut tx, &[from_id, to_id]).await?;

    let ref_id = Uuid::new_v4();
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id: from_id,
            amount: -amount.clone(),
            ledger_type: "TRANSFER_OUT",
            reference_id: Some(ref_id),
            reference_type: Some("InternalTransfer"),
            memo: Some(&format!("To {} wallet", to_kind.to_lowercase())),
        },
    )
    .await?;
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id: to_id,
            amount,
            ledger_type: "TRANSFER_IN",
            reference_id: Some(ref_id),
            reference_type: Some("InternalTransfer"),
            memo: Some(&format!("From {} wallet", from_kind.to_lowercase())),
        },
    )
    .await?;

    tx.commit().await?;
    Ok(())
}
