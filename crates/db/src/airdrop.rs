//! Módulo de banco de dados para gestão do Ecossistema de Airdrop ($SATS Points & Seasons).

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AirdropSeasonInfo {
    pub id: Uuid,
    pub season_number: i32,
    pub title: String,
    pub description: String,
    pub reward_pool_usd: String,
    pub status: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserAirdropProfile {
    pub season_id: Uuid,
    pub season_number: i32,
    pub season_title: String,
    pub base_points: i64,
    pub bonus_points: i64,
    pub total_points: i64,
    pub tier: String,
    pub multiplier: f64,
    pub global_rank: i64,
    pub total_participants: i64,
    pub claimed: bool,
    pub projected_reward_usd: String,
    pub days_remaining: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirdropPointLog {
    pub id: Uuid,
    pub user_id: Uuid,
    pub season_id: Uuid,
    pub activity_type: String,
    pub description: String,
    pub points: i64,
    pub bonus_points: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i64,
    pub user_id: Uuid,
    pub username: String,
    pub tier: String,
    pub total_points: i64,
    pub multiplier: f64,
}

/// Retrieves the active airdrop season.
pub async fn get_active_season(pool: &PgPool) -> Result<Option<AirdropSeasonInfo>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, season_number, title, description, reward_pool_usd, status, start_at, end_at
         FROM airdrop_seasons
         WHERE status = 'ACTIVE'
         ORDER BY season_number DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| AirdropSeasonInfo {
        id: r.get("id"),
        season_number: r.get("season_number"),
        title: r.get("title"),
        description: r.get("description"),
        reward_pool_usd: r.get::<BigDecimal, _>("reward_pool_usd").to_string(),
        status: r.get("status"),
        start_at: r.get("start_at"),
        end_at: r.get("end_at"),
    }))
}

/// Awards points to a user in the active season and recalibrates their tier.
pub async fn award_airdrop_points(
    pool: &PgPool,
    user_id: Uuid,
    base_add: i64,
    bonus_add: i64,
    activity: &str,
) -> Result<(), sqlx::Error> {
    let Some(season) = get_active_season(pool).await? else {
        return Ok(());
    };

    let description = match activity {
        "REFERRAL_SIGNUP" => "Bônus de Indicação: amigo cadastrado pelo seu link exclusivo (+50 pts)",
        "REFERRAL_WELCOME" => "Bônus de Boas-Vindas: cadastro via link de convite (+50 pts)",
        "FAUCET_CLAIM" => "Reivindicação de Torneira / Faucet (+50 pts)",
        "SWAP_EXECUTE" => "Operação de Conversão Instantânea / Swap (+100 pts)",
        "COMMISSION_EARNED" => "Comissão de Afiliado: atividade gerada por indicado (+10 pts)",
        "DEPOSIT_CONFIRMED" => "Depósito Confirmado On-Chain (+100 pts)",
        other => other,
    };

    // Insert tracking audit log
    let _ = sqlx::query(
        "INSERT INTO airdrop_point_logs (season_id, user_id, activity_type, description, points, bonus_points)
         VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(season.id)
    .bind(user_id)
    .bind(activity)
    .bind(description)
    .bind(base_add)
    .bind(bonus_add)
    .execute(pool)
    .await;

    // Calculate tier and multiplier based on projected new total
    let current_row = sqlx::query(
        "SELECT base_points, bonus_points FROM airdrop_user_points WHERE season_id = $1 AND user_id = $2"
    )
    .bind(season.id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let (cur_base, cur_bonus): (i64, i64) = current_row
        .map(|r| (r.get("base_points"), r.get("bonus_points")))
        .unwrap_or((0, 0));

    let new_base = cur_base + base_add;
    let new_bonus = cur_bonus + bonus_add;
    let raw_points = new_base + new_bonus;

    let (tier, mult) = calculate_tier(raw_points);
    let total_with_mult = (raw_points as f64 * mult).round() as i64;

    sqlx::query(
        "INSERT INTO airdrop_user_points (season_id, user_id, base_points, bonus_points, total_points, tier, multiplier, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (season_id, user_id) DO UPDATE SET
            base_points = $3,
            bonus_points = $4,
            total_points = $5,
            tier = $6,
            multiplier = $7,
            updated_at = NOW()"
    )
    .bind(season.id)
    .bind(user_id)
    .bind(new_base)
    .bind(new_bonus)
    .bind(total_with_mult)
    .bind(tier)
    .bind(BigDecimal::try_from(mult).unwrap_or_else(|_| BigDecimal::from(1)))
    .execute(pool)
    .await?;

    Ok(())
}

fn calculate_tier(points: i64) -> (&'static str, f64) {
    match points {
        p if p >= 5_000_000 => ("DIAMOND", 2.50),
        p if p >= 1_000_000 => ("PLATINUM", 2.00),
        p if p >= 250_000 => ("GOLD", 1.50),
        p if p >= 50_000 => ("SILVER", 1.25),
        _ => ("BRONZE", 1.00),
    }
}

/// Fetches user's airdrop profile in the active season.
pub async fn get_user_airdrop_profile(pool: &PgPool, user_id: Uuid) -> Result<UserAirdropProfile, sqlx::Error> {
    let season = get_active_season(pool).await?.unwrap_or(AirdropSeasonInfo {
        id: Uuid::nil(),
        season_number: 1,
        title: "Season 1: Genesis Growth Campaign".into(),
        description: "Campanha oficial de airdrop.".into(),
        reward_pool_usd: "100000.00".into(),
        status: "ACTIVE".into(),
        start_at: Utc::now(),
        end_at: Utc::now() + chrono::Duration::days(90),
    });

    let points_row = sqlx::query(
        "SELECT base_points, bonus_points, total_points, tier, multiplier, claimed, reward_amount_usd
         FROM airdrop_user_points
         WHERE season_id = $1 AND user_id = $2"
    )
    .bind(season.id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let (base_pts, bonus_pts, total_pts, tier, mult, claimed, reward_usd) = match points_row {
        Some(r) => (
            r.get::<i64, _>("base_points"),
            r.get::<i64, _>("bonus_points"),
            r.get::<i64, _>("total_points"),
            r.get::<String, _>("tier"),
            r.get::<BigDecimal, _>("multiplier").to_string().parse::<f64>().unwrap_or(1.0),
            r.get::<bool, _>("claimed"),
            r.get::<BigDecimal, _>("reward_amount_usd").to_string(),
        ),
        None => (0, 0, 0, "BRONZE".to_string(), 1.0, false, "0.0000".to_string()),
    };

    let global_rank: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM airdrop_user_points WHERE season_id = $1 AND total_points > $2"
    )
    .bind(season.id)
    .bind(total_pts)
    .fetch_one(pool)
    .await
    .unwrap_or(1);

    let total_participants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM airdrop_user_points WHERE season_id = $1"
    )
    .bind(season.id)
    .fetch_one(pool)
    .await
    .unwrap_or(1);

    let pool_tokens: f64 = 100_000_000.0;
    let total_season_points: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_points), 1) FROM airdrop_user_points WHERE season_id = $1"
    )
    .bind(season.id)
    .fetch_one(pool)
    .await
    .unwrap_or(1);

    let projected_reward = if total_season_points > 0 && total_pts > 0 {
        format!("{:.0}", ((total_pts as f64 / total_season_points as f64) * pool_tokens).round())
    } else {
        "0".into()
    };

    let days_rem = (season.end_at - Utc::now()).num_days().max(0);

    Ok(UserAirdropProfile {
        season_id: season.id,
        season_number: season.season_number,
        season_title: season.title,
        base_points: base_pts,
        bonus_points: bonus_pts,
        total_points: total_pts,
        tier,
        multiplier: mult,
        global_rank,
        total_participants: total_participants.max(1),
        claimed,
        projected_reward_usd: if claimed { reward_usd } else { projected_reward },
        days_remaining: days_rem,
    })
}

/// Retrieves global top leaderboard.
pub async fn get_airdrop_leaderboard(pool: &PgPool, limit: i64) -> Result<Vec<LeaderboardEntry>, sqlx::Error> {
    let Some(season) = get_active_season(pool).await? else {
        return Ok(vec![]);
    };

    let rows = sqlx::query(
        "SELECT p.user_id, COALESCE(u.username, SUBSTRING(u.email FROM 1 FOR 3) || '***') as username,
                p.tier, p.total_points, p.multiplier
         FROM airdrop_user_points p
         JOIN users u ON u.id = p.user_id
         WHERE p.season_id = $1
         ORDER BY p.total_points DESC LIMIT $2"
    )
    .bind(season.id)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;

    let mut entries = Vec::new();
    for (idx, r) in rows.into_iter().enumerate() {
        entries.push(LeaderboardEntry {
            rank: (idx + 1) as i64,
            user_id: r.get("user_id"),
            username: r.get("username"),
            tier: r.get("tier"),
            total_points: r.get("total_points"),
            multiplier: r.get::<BigDecimal, _>("multiplier").to_string().parse::<f64>().unwrap_or(1.0),
        });
    }

    Ok(entries)
}

/// Lists chronological point audit log entries for a user.
pub async fn list_user_point_logs(pool: &PgPool, user_id: Uuid, limit: i64) -> Result<Vec<AirdropPointLog>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, user_id, season_id, activity_type, description, points, bonus_points, created_at
         FROM airdrop_point_logs
         WHERE user_id = $1
         ORDER BY created_at DESC LIMIT $2"
    )
    .bind(user_id)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| AirdropPointLog {
        id: r.get("id"),
        user_id: r.get("user_id"),
        season_id: r.get("season_id"),
        activity_type: r.get("activity_type"),
        description: r.get("description"),
        points: r.get("points"),
        bonus_points: r.get("bonus_points"),
        created_at: r.get("created_at"),
    }).collect())
}

