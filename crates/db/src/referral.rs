//! Módulo de banco de dados para gestão de Referrals e Comissões de Afiliados.

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReferralStats {
    pub referral_code: String,
    pub referral_link: String,
    pub total_referred: i64,
    pub active_referred_24h: i64,
    pub total_earned_usd: String,
    pub faucet_commission_pct: i32,
    pub swap_commission_pct: i32,
    pub merchant_commission_pct: f64,
    pub earnings_by_coin: Vec<CoinEarning>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinEarning {
    pub coin: String,
    pub total_amount: String,
    pub total_usd: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReferralCommissionItem {
    pub id: Uuid,
    pub referred_id: Uuid,
    pub activity_type: String,
    pub coin: String,
    pub amount: String,
    pub amount_usd: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReferredUserItem {
    pub id: Uuid,
    pub username: String,
    pub referral_code: String,
    pub is_active_24h: bool,
    pub total_commissions_usd: String,
    pub joined_at: DateTime<Utc>,
}

/// Links a newly registered user to a referrer via referral_code or username.
pub async fn link_referred_user(pool: &PgPool, referrer_code_or_username: &str, new_user_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    let clean = referrer_code_or_username.trim();
    if clean.is_empty() {
        return Ok(None);
    }

    // Find referrer user by username (case-insensitive) or by user ID / prefix
    let prefix = format!("{clean}%");
    let referrer = sqlx::query(
        "SELECT id FROM users WHERE LOWER(username) = LOWER($1) OR id::text = $1 OR id::text LIKE $2"
    )
    .bind(clean)
    .bind(prefix)
    .fetch_optional(pool)
    .await?;

    let Some(row) = referrer else {
        return Ok(None);
    };

    let referrer_id: Uuid = row.get("id");
    if referrer_id == new_user_id {
        return Ok(None); // Can't refer oneself
    }

    let res = sqlx::query(
        "INSERT INTO referral_links (referrer_id, referred_id, referral_code)
         VALUES ($1, $2, $3)
         ON CONFLICT (referred_id) DO NOTHING"
    )
    .bind(referrer_id)
    .bind(new_user_id)
    .bind(clean)
    .execute(pool)
    .await?;

    if res.rows_affected() > 0 {
        // Grant bonus Airdrop points to referrer (+50 pts) and referred user (+50 pts welcome) only on first link
        let _ = crate::airdrop::award_airdrop_points(pool, referrer_id, 50, 0, "REFERRAL_SIGNUP").await;
        let _ = crate::airdrop::award_airdrop_points(pool, new_user_id, 50, 0, "REFERRAL_WELCOME").await;
    }

    Ok(Some(referrer_id))
}

/// Fetches comprehensive referral stats for a user.
pub async fn get_referral_stats(pool: &PgPool, user_id: Uuid) -> Result<ReferralStats, sqlx::Error> {
    let username: Option<String> = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .flatten();

    let code = username.filter(|u| !u.is_empty()).unwrap_or_else(|| {
        let id_str = user_id.to_string();
        id_str[..8].to_string()
    });

    let referral_link = format!("https://www.satspay.pro/r/{code}");

    let total_referred: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM referral_links WHERE referrer_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let active_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT r.referred_id)
         FROM referral_links r
         JOIN users u ON u.id = r.referred_id
         WHERE r.referrer_id = $1 AND u.last_login_at >= NOW() - INTERVAL '24 hours'"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let total_usd: BigDecimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_usd), 0) FROM referral_commissions WHERE referrer_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| BigDecimal::from(0));

    // Earnings grouped by coin
    let coin_rows = sqlx::query(
        "SELECT coin, COALESCE(SUM(amount), 0) as total_amount, COALESCE(SUM(amount_usd), 0) as total_usd
         FROM referral_commissions
         WHERE referrer_id = $1
         GROUP BY coin"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let earnings_by_coin = coin_rows.into_iter().map(|r| {
        CoinEarning {
            coin: r.get("coin"),
            total_amount: r.get::<BigDecimal, _>("total_amount").to_string(),
            total_usd: r.get::<BigDecimal, _>("total_usd").to_string(),
        }
    }).collect();

    Ok(ReferralStats {
        referral_code: code,
        referral_link,
        total_referred,
        active_referred_24h: active_24h,
        total_earned_usd: total_usd.to_string(),
        faucet_commission_pct: 10,
        swap_commission_pct: 10,
        merchant_commission_pct: 0.10,
        earnings_by_coin,
    })
}

/// Records a commission payout in the ledger and commission log.
pub async fn record_referral_commission(
    pool: &PgPool,
    referred_id: Uuid,
    activity_type: &str,
    coin: &str,
    amount: BigDecimal,
    amount_usd: BigDecimal,
) -> Result<Option<Uuid>, sqlx::Error> {
    // Check if user has a referrer
    let referrer_row = sqlx::query(
        "SELECT referrer_id FROM referral_links WHERE referred_id = $1"
    )
    .bind(referred_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = referrer_row else {
        return Ok(None);
    };

    let referrer_id: Uuid = row.get("referrer_id");

    let comm_id: Uuid = sqlx::query_scalar(
        "INSERT INTO referral_commissions (referrer_id, referred_id, activity_type, coin, amount, amount_usd)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id"
    )
    .bind(referrer_id)
    .bind(referred_id)
    .bind(activity_type)
    .bind(coin)
    .bind(&amount)
    .bind(&amount_usd)
    .fetch_one(pool)
    .await?;

    // Also credit airdrop points for commission activity
    let _ = crate::airdrop::award_airdrop_points(pool, referrer_id, 10, 0, "COMMISSION_EARNED").await;

    Ok(Some(comm_id))
}

/// Lists recent commission payouts for a user.
pub async fn list_user_commissions(pool: &PgPool, user_id: Uuid, limit: i64) -> Result<Vec<ReferralCommissionItem>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, referred_id, activity_type, coin, amount, amount_usd, created_at
         FROM referral_commissions
         WHERE referrer_id = $1
         ORDER BY created_at DESC LIMIT $2"
    )
    .bind(user_id)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| ReferralCommissionItem {
        id: r.get("id"),
        referred_id: r.get("referred_id"),
        activity_type: r.get("activity_type"),
        coin: r.get("coin"),
        amount: r.get::<BigDecimal, _>("amount").to_string(),
        amount_usd: r.get::<BigDecimal, _>("amount_usd").to_string(),
        created_at: r.get("created_at"),
    }).collect())
}

/// Lists all referred users linked to a referrer.
pub async fn list_referred_users(pool: &PgPool, referrer_id: Uuid, limit: i64) -> Result<Vec<ReferredUserItem>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT r.id, r.referred_id, r.referral_code, r.created_at,
                COALESCE(NULLIF(u.username, ''), SUBSTRING(u.email FROM 1 FOR 3) || '***@' || SPLIT_PART(u.email, '@', 2)) as display_name,
                (u.last_login_at IS NOT NULL AND u.last_login_at >= NOW() - INTERVAL '24 hours') as is_active_24h,
                COALESCE((SELECT SUM(amount_usd) FROM referral_commissions rc WHERE rc.referrer_id = $1 AND rc.referred_id = r.referred_id), 0) as total_commissions_usd
         FROM referral_links r
         JOIN users u ON u.id = r.referred_id
         WHERE r.referrer_id = $1
         ORDER BY r.created_at DESC LIMIT $2"
    )
    .bind(referrer_id)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| ReferredUserItem {
        id: r.get("referred_id"),
        username: r.get("display_name"),
        referral_code: r.get("referral_code"),
        is_active_24h: r.get("is_active_24h"),
        total_commissions_usd: r.get::<BigDecimal, _>("total_commissions_usd").to_string(),
        joined_at: r.get("created_at"),
    }).collect())
}

