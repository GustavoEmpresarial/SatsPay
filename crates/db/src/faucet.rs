//! Port 1:1 of legacy `apps/api/src/modules/faucet/services/faucet.service.ts`.
//! Captcha is enforced in `api-http` (Turnstile + action pin). This module owns
//! cooldown / anti-Sybil / HOUSE debit / ledger credit.

use crate::house::{debit_house, HouseError};
use crate::ledger::{apply_ledger_entry, lock_wallet, LedgerCreditInput};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use shared::{coin_config, Coin};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum FaucetError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    House(#[from] HouseError),
    #[error("wallet not found")]
    NotFound,
    #[error("next claim available at {next_claim_at}")]
    Cooldown { next_claim_at: DateTime<Utc> },
}

pub struct ClaimResult {
    pub amount: u128,
    pub coin: Coin,
    pub next_claim_at: DateTime<Utc>,
}

/// Stable 64-bit advisory lock key for (ip, coin) — serializes cross-user
/// faucet claims sharing an IP, closing the Sybil race where two accounts on
/// the same network both pass the cooldown check simultaneously.
pub fn faucet_ip_lock_key(ip: &str, coin: Coin) -> i64 {
    let mut hasher = Sha256::new();
    hasher.update(format!("faucet:{}:{}", coin.as_str(), ip));
    let digest = hasher.finalize();
    i64::from_be_bytes(digest[0..8].try_into().unwrap())
}

pub async fn claim(pool: &PgPool, user_id: Uuid, coin: Coin, ip: &str, cooldown_minutes: i64) -> Result<ClaimResult, FaucetError> {
    let cooldown = Duration::minutes(cooldown_minutes);
    let now = Utc::now();
    let ip_cooldown_since = now - cooldown;

    let mut tx = pool.begin().await?;

    // Cross-user IP serialization BEFORE cooldown checks.
    sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(faucet_ip_lock_key(ip, coin)).execute(&mut *tx).await?;

    let wallet_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wallets WHERE user_id = $1 AND coin = $2::coin AND kind = 'PERSONAL'")
        .bind(user_id)
        .bind(coin.as_str())
        .fetch_optional(&mut *tx)
        .await?;
    let wallet_id = wallet_id.ok_or(FaucetError::NotFound)?;

    // Serialize claims for this wallet BEFORE reading the cooldown, so two
    // requests racing at the end of the cooldown can't both pass the check.
    lock_wallet(&mut tx, wallet_id).await.map_err(|e| FaucetError::Db(sqlx::Error::Protocol(e.to_string())))?;

    // Per-user cooldown.
    let last_by_user: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT created_at FROM faucet_claims WHERE user_id = $1 AND coin = $2::coin ORDER BY created_at DESC LIMIT 1")
            .bind(user_id)
            .bind(coin.as_str())
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(last) = last_by_user {
        let next_allowed = last + cooldown;
        if next_allowed > now {
            return Err(FaucetError::Cooldown { next_claim_at: next_allowed });
        }
    }

    // Per-IP cooldown (Sybil defense across accounts sharing an IP).
    let ip_recent: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT created_at FROM faucet_claims WHERE ip = $1 AND coin = $2::coin AND created_at > $3 ORDER BY created_at DESC LIMIT 1")
            .bind(ip)
            .bind(coin.as_str())
            .bind(ip_cooldown_since)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(recent) = ip_recent {
        let next_allowed = recent + cooldown;
        return Err(FaucetError::Cooldown { next_claim_at: next_allowed });
    }

    let amount = coin_config(coin).faucet_reward;
    let claim_id: Uuid = sqlx::query_scalar("INSERT INTO faucet_claims (user_id, coin, amount, ip) VALUES ($1, $2::coin, $3, $4) RETURNING id")
        .bind(user_id)
        .bind(coin.as_str())
        .bind(BigDecimal::from(amount))
        .bind(ip)
        .fetch_one(&mut *tx)
        .await?;

    // Pay from HOUSE inventory — fails closed if pool empty.
    debit_house(&mut tx, coin, BigDecimal::from(amount), "FAUCET", claim_id, "FaucetClaim", &format!("Faucet payout {}", coin.as_str())).await?;

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None,
            wallet_id,
            amount: BigDecimal::from(amount),
            ledger_type: "FAUCET",
            reference_id: Some(claim_id),
            reference_type: Some("FaucetClaim"),
            memo: Some(&format!("Faucet reward {}", coin.as_str())),
        },
    )
    .await
    .map_err(|e| FaucetError::Db(sqlx::Error::Protocol(e.to_string())))?;

    tx.commit().await?;

    Ok(ClaimResult { amount, coin, next_claim_at: now + cooldown })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faucet_ip_lock_key_stable_and_distinct() {
        let a = faucet_ip_lock_key("203.0.113.5", Coin::Btc);
        let b = faucet_ip_lock_key("203.0.113.5", Coin::Btc);
        let c = faucet_ip_lock_key("203.0.113.6", Coin::Btc);
        let d = faucet_ip_lock_key("203.0.113.5", Coin::Ltc);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }
}
