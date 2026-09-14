//! Port 1:1 of legacy `apps/api/src/modules/wallet/services/house.service.ts`
//! (minus admin fund/seed endpoints, deferred to Fase 7).

use crate::ledger::{apply_ledger_entry, LedgerCreditInput, LedgerError};
use bigdecimal::BigDecimal;
use shared::{Coin, COINS};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

/// Internal system account that owns HOUSE/LEND_POOL wallets (platform liquidity).
pub const HOUSE_EMAIL: &str = "system@bitcosats.internal";

#[derive(Debug, thiserror::Error)]
pub enum HouseError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("house inventory not initialized")]
    NotInitialized,
    #[error("house wallet missing for {0:?}")]
    WalletMissing(Coin),
    #[error("platform inventory for {0:?} is insufficient for this operation")]
    InsufficientLiquidity(Coin),
}

/// Ensures the house system user + one HOUSE and one LEND_POOL wallet per
/// coin exist. Idempotent; safe to call on API/worker boot.
///
/// Does **not** mint fake ledger inventory. Liquidity for faucet/lend/HOUSE
/// swap must be funded explicitly (admin fund) — swaps use external DEX.
pub async fn ensure_house_inventory(pool: &PgPool) -> Result<Uuid, HouseError> {
    let password_hash = crypto::hash_password(&crypto::random_token(48)).expect("argon2 hashing is infallible for random input");

    let house_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, username, password_hash) VALUES ($1, $2, $3) \
         ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email RETURNING id",
    )
    .bind(HOUSE_EMAIL)
    .bind("system")
    .bind(&password_hash)
    .fetch_one(pool)
    .await?;

    for kind in ["HOUSE", "LEND_POOL"] {
        for coin in COINS {
            let _: Option<Uuid> = sqlx::query_scalar(
                "INSERT INTO wallets (user_id, coin, kind) VALUES ($1, $2::coin, $3::wallet_kind) \
                 ON CONFLICT (user_id, coin, kind) DO UPDATE SET coin = EXCLUDED.coin RETURNING id",
            )
            .bind(house_id)
            .bind(coin.as_str())
            .bind(kind)
            .fetch_optional(pool)
            .await?;
        }
    }

    reverse_legacy_liquidity_seed(pool).await?;

    Ok(house_id)
}

/// Undo the old auto-mint (`Platform Liquidity Reserve Seed` ≈ 1M units/coin).
/// Idempotent via memo guard.
async fn reverse_legacy_liquidity_seed(pool: &PgPool) -> Result<(), HouseError> {
    sqlx::query(
        "INSERT INTO ledger_entries (wallet_id, amount, type, memo, created_at) \
         SELECT le.wallet_id, -le.amount, 'ADJUSTMENT'::ledger_type, \
                'Reverse Platform Liquidity Reserve Seed', NOW() \
         FROM ledger_entries le \
         WHERE le.memo = 'Platform Liquidity Reserve Seed' \
           AND NOT EXISTS ( \
             SELECT 1 FROM ledger_entries r \
             WHERE r.wallet_id = le.wallet_id \
               AND r.memo = 'Reverse Platform Liquidity Reserve Seed' \
           )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn find_house_id<'e, E: sqlx::Executor<'e, Database = Postgres>>(executor: E) -> Result<Uuid, HouseError> {
    sqlx::query_scalar("SELECT id FROM users WHERE email = $1").bind(HOUSE_EMAIL).fetch_optional(executor).await?.ok_or(HouseError::NotInitialized)
}

pub async fn get_house_wallet_id(tx: &mut Transaction<'_, Postgres>, coin: Coin) -> Result<Uuid, HouseError> {
    let house_id = find_house_id(&mut **tx).await?;
    sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'HOUSE'")
        .bind(house_id)
        .bind(coin.as_str())
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(HouseError::WalletMissing(coin))
}

pub async fn get_lend_pool_wallet_id(tx: &mut Transaction<'_, Postgres>, coin: Coin) -> Result<Uuid, HouseError> {
    let house_id = find_house_id(&mut **tx).await?;
    sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'LEND_POOL'")
        .bind(house_id)
        .bind(coin.as_str())
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(HouseError::WalletMissing(coin))
}

/// Debits house inventory (swap payout, faucet, stake reward). Fails closed
/// (`InsufficientLiquidity`) if the pool cannot cover it — never overdraws
/// platform reserves to satisfy a user operation.
pub async fn debit_house(
    tx: &mut Transaction<'_, Postgres>,
    coin: Coin,
    amount: BigDecimal,
    ledger_type: &str,
    reference_id: Uuid,
    reference_type: &str,
    memo: &str,
) -> Result<(), HouseError> {
    use bigdecimal::Zero;
    if amount.is_zero() {
        return Ok(());
    }
    let wallet_id = get_house_wallet_id(tx, coin).await?;
    apply_ledger_entry(
        tx,
        LedgerCreditInput { reference_key: None, wallet_id, amount: -amount, ledger_type, reference_id: Some(reference_id), reference_type: Some(reference_type), memo: Some(memo) },
    )
    .await
    .map_err(|e| match e {
        LedgerError::InsufficientBalance { .. } => HouseError::InsufficientLiquidity(coin),
        LedgerError::Db(e) => HouseError::Db(e),
        LedgerError::ReorgHold { .. } => HouseError::InsufficientLiquidity(coin),
    })?;
    Ok(())
}

/// Credits house inventory (swap intake / fee retention).
pub async fn credit_house(
    tx: &mut Transaction<'_, Postgres>,
    coin: Coin,
    amount: BigDecimal,
    ledger_type: &str,
    reference_id: Uuid,
    reference_type: &str,
    memo: &str,
) -> Result<(), HouseError> {
    use bigdecimal::Zero;
    if amount.is_zero() {
        return Ok(());
    }
    let wallet_id = get_house_wallet_id(tx, coin).await?;
    apply_ledger_entry(
        tx,
        LedgerCreditInput { reference_key: None, wallet_id, amount, ledger_type, reference_id: Some(reference_id), reference_type: Some(reference_type), memo: Some(memo) },
    )
    .await
    .map_err(|e| match e {
        LedgerError::Db(e) => HouseError::Db(e),
        other => HouseError::Db(sqlx::Error::Protocol(other.to_string())),
    })?;
    Ok(())
}

pub async fn list_house_balances(pool: &PgPool) -> Result<Vec<(Coin, BigDecimal)>, HouseError> {
    list_balances_by_kind(pool, "HOUSE").await
}

pub async fn list_lend_pool_balances(pool: &PgPool) -> Result<Vec<(Coin, BigDecimal)>, HouseError> {
    list_balances_by_kind(pool, "LEND_POOL").await
}

async fn list_balances_by_kind(pool: &PgPool, kind: &str) -> Result<Vec<(Coin, BigDecimal)>, HouseError> {
    let Some(house_id): Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1").bind(HOUSE_EMAIL).fetch_optional(pool).await? else {
        return Ok(vec![]);
    };
    let rows = sqlx::query(
        "SELECT w.coin::text as coin, COALESCE(SUM(l.amount), 0) as balance
         FROM wallets w LEFT JOIN ledger_entries l ON l.wallet_id = w.id
         WHERE w.user_id = $1 AND w.kind = $2::wallet_kind
         GROUP BY w.coin ORDER BY w.coin",
    )
    .bind(house_id)
    .bind(kind)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(|r| { let c: String = r.get("coin"); c.parse::<Coin>().ok().map(|coin| (coin, r.get("balance"))) }).collect())
}
