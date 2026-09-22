//! Módulo de banco de dados para gestão do Ecossistema de Airdrop ($SATS Points & Seasons).

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Row, Transaction};
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
    /// False when no `airdrop_seasons` row has `status='ACTIVE'`.
    pub season_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwardResult {
    Awarded,
    NoActiveSeason,
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
/// Log + balance update run in one transaction; concurrent awards serialize via row lock.
/// Returns `NoActiveSeason` (observable, not silent) when no ACTIVE season exists.
pub async fn award_airdrop_points(
    pool: &PgPool,
    user_id: Uuid,
    base_add: i64,
    bonus_add: i64,
    activity: &str,
) -> Result<AwardResult, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let result = award_airdrop_points_tx(&mut tx, user_id, base_add, bonus_add, activity).await?;
    match result {
        AwardResult::Awarded => tx.commit().await?,
        AwardResult::NoActiveSeason => tx.rollback().await?,
    }
    Ok(result)
}

/// Same award as [`award_airdrop_points`], on a caller-owned transaction.
/// Faucet uses this so the ledger credit and the +50 points commit together.
pub async fn award_airdrop_points_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    base_add: i64,
    bonus_add: i64,
    activity: &str,
) -> Result<AwardResult, sqlx::Error> {
    let season = sqlx::query(
        "SELECT id FROM airdrop_seasons WHERE status = 'ACTIVE' ORDER BY season_number DESC LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(season) = season else {
        tracing::info!(%user_id, activity, "airdrop award skipped: no ACTIVE season");
        return Ok(AwardResult::NoActiveSeason);
    };
    let season_id: Uuid = season.get("id");

    let description = match activity {
        "REFERRAL_SIGNUP" => "Bônus de Indicação: amigo cadastrado pelo seu link exclusivo (+50 pts)",
        "REFERRAL_WELCOME" => "Bônus de Boas-Vindas: cadastro via link de convite (+50 pts)",
        "FAUCET_CLAIM" => "Reivindicação de Torneira / Faucet (+50 pts)",
        "SWAP_EXECUTE" => "Operação de Conversão Instantânea / Swap (+100 pts)",
        "COMMISSION_EARNED" => "Comissão de Afiliado: atividade gerada por indicado (+10 pts)",
        "DEPOSIT_CONFIRMED" => "Depósito Confirmado On-Chain (+100 pts)",
        other => other,
    };

    sqlx::query(
        "INSERT INTO airdrop_point_logs (season_id, user_id, activity_type, description, points, bonus_points)
         VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(season_id)
    .bind(user_id)
    .bind(activity)
    .bind(description)
    .bind(base_add)
    .bind(bonus_add)
    .execute(&mut **tx)
    .await?;

    // Atomic increment so concurrent faucet/swap awards cannot clobber each other.
    let points_row = sqlx::query(
        "INSERT INTO airdrop_user_points
            (season_id, user_id, base_points, bonus_points, total_points, tier, multiplier, updated_at)
         VALUES ($1, $2, $3, $4, $3 + $4, 'BRONZE', 1, NOW())
         ON CONFLICT (season_id, user_id) DO UPDATE SET
            base_points = airdrop_user_points.base_points + EXCLUDED.base_points,
            bonus_points = airdrop_user_points.bonus_points + EXCLUDED.bonus_points,
            updated_at = NOW()
         RETURNING base_points, bonus_points"
    )
    .bind(season_id)
    .bind(user_id)
    .bind(base_add)
    .bind(bonus_add)
    .fetch_one(&mut **tx)
    .await?;

    let new_base: i64 = points_row.get("base_points");
    let new_bonus: i64 = points_row.get("bonus_points");
    let raw_points = new_base + new_bonus;
    let (tier, mult) = calculate_tier(raw_points);
    let total_with_mult = (raw_points as f64 * mult).round() as i64;

    sqlx::query(
        "UPDATE airdrop_user_points
         SET total_points = $3, tier = $4, multiplier = $5, updated_at = NOW()
         WHERE season_id = $1 AND user_id = $2"
    )
    .bind(season_id)
    .bind(user_id)
    .bind(total_with_mult)
    .bind(tier)
    .bind(BigDecimal::try_from(mult).unwrap_or_else(|_| BigDecimal::from(1)))
    .execute(&mut **tx)
    .await?;

    Ok(AwardResult::Awarded)
}

/// Idempotent repair for activity that created side-effects (referrals / credited
/// deposits) before airdrop awards were reliably persisted.
pub async fn backfill_missed_airdrop_points(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let mut awarded = 0u64;

    let referral_gaps = sqlx::query(
        "SELECT rl.referrer_id AS user_id,
                COUNT(*)::bigint AS expected,
                COALESCE((
                    SELECT COUNT(*)::bigint FROM airdrop_point_logs l
                    WHERE l.user_id = rl.referrer_id AND l.activity_type = 'REFERRAL_SIGNUP'
                ), 0) AS already
         FROM referral_links rl
         GROUP BY rl.referrer_id
         HAVING COUNT(*) > COALESCE((
                    SELECT COUNT(*)::bigint FROM airdrop_point_logs l
                    WHERE l.user_id = rl.referrer_id AND l.activity_type = 'REFERRAL_SIGNUP'
                ), 0)"
    )
    .fetch_all(pool)
    .await?;

    for row in referral_gaps {
        let user_id: Uuid = row.get("user_id");
        let missing = row.get::<i64, _>("expected") - row.get::<i64, _>("already");
        for _ in 0..missing.max(0) {
            award_airdrop_points(pool, user_id, 50, 0, "REFERRAL_SIGNUP").await?;
            awarded += 1;
        }
    }

    let welcome_gaps = sqlx::query(
        "SELECT rl.referred_id AS user_id
         FROM referral_links rl
         WHERE NOT EXISTS (
             SELECT 1 FROM airdrop_point_logs l
             WHERE l.user_id = rl.referred_id AND l.activity_type = 'REFERRAL_WELCOME'
         )"
    )
    .fetch_all(pool)
    .await?;

    for row in welcome_gaps {
        let user_id: Uuid = row.get("user_id");
        award_airdrop_points(pool, user_id, 50, 0, "REFERRAL_WELCOME").await?;
        awarded += 1;
    }

    let deposit_gaps = sqlx::query(
        "SELECT w.user_id AS user_id,
                COUNT(*)::bigint AS expected,
                COALESCE((
                    SELECT COUNT(*)::bigint FROM airdrop_point_logs l
                    WHERE l.user_id = w.user_id AND l.activity_type = 'DEPOSIT_CONFIRMED'
                ), 0) AS already
         FROM deposits d
         JOIN wallets w ON w.id = d.wallet_id
         WHERE d.status = 'CREDITED'
         GROUP BY w.user_id
         HAVING COUNT(*) > COALESCE((
                    SELECT COUNT(*)::bigint FROM airdrop_point_logs l
                    WHERE l.user_id = w.user_id AND l.activity_type = 'DEPOSIT_CONFIRMED'
                ), 0)"
    )
    .fetch_all(pool)
    .await?;

    for row in deposit_gaps {
        let user_id: Uuid = row.get("user_id");
        let missing = row.get::<i64, _>("expected") - row.get::<i64, _>("already");
        for _ in 0..missing.max(0) {
            award_airdrop_points(pool, user_id, 100, 0, "DEPOSIT_CONFIRMED").await?;
            awarded += 1;
        }
    }

    if let Some(season) = get_active_season(pool).await? {
        let faucet_gaps = sqlx::query(
            "SELECT fc.user_id AS user_id,
                    COUNT(*)::bigint AS expected,
                    COALESCE((
                        SELECT COUNT(*)::bigint FROM airdrop_point_logs l
                        WHERE l.user_id = fc.user_id
                          AND l.season_id = $1
                          AND l.activity_type = 'FAUCET_CLAIM'
                    ), 0) AS already
             FROM faucet_claims fc
             WHERE fc.created_at >= $2
             GROUP BY fc.user_id
             HAVING COUNT(*) > COALESCE((
                        SELECT COUNT(*)::bigint FROM airdrop_point_logs l
                        WHERE l.user_id = fc.user_id
                          AND l.season_id = $1
                          AND l.activity_type = 'FAUCET_CLAIM'
                    ), 0)",
        )
        .bind(season.id)
        .bind(season.start_at)
        .fetch_all(pool)
        .await?;

        for row in faucet_gaps {
            let user_id: Uuid = row.get("user_id");
            let missing = row.get::<i64, _>("expected") - row.get::<i64, _>("already");
            for _ in 0..missing.max(0) {
                award_airdrop_points(pool, user_id, 50, 0, "FAUCET_CLAIM").await?;
                awarded += 1;
            }
        }
    }

    // Merchant invoices, merchant wallets and gateway payouts never award points.
    if awarded > 0 {
        tracing::info!(awarded, "airdrop missed-points backfill applied");
    }
    Ok(awarded)
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
    let Some(season) = get_active_season(pool).await? else {
        return Ok(UserAirdropProfile {
            season_id: Uuid::nil(),
            season_number: 0,
            season_title: "Nenhuma temporada ativa".into(),
            base_points: 0,
            bonus_points: 0,
            total_points: 0,
            tier: "BRONZE".into(),
            multiplier: 1.0,
            global_rank: 0,
            total_participants: 0,
            claimed: false,
            projected_reward_usd: "0".into(),
            days_remaining: 0,
            season_active: false,
        });
    };

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

    // 0 = unranked (no points yet). Avoids "#5 entre 4 participantes".
    let global_rank: i64 = if total_pts <= 0 {
        0
    } else {
        sqlx::query_scalar(
            "SELECT COUNT(*) + 1 FROM airdrop_user_points WHERE season_id = $1 AND total_points > $2"
        )
        .bind(season.id)
        .bind(total_pts)
        .fetch_one(pool)
        .await
        .unwrap_or(1)
    };

    let total_participants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM airdrop_user_points WHERE season_id = $1 AND total_points > 0"
    )
    .bind(season.id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

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
        total_participants: total_participants.max(0),
        claimed,
        projected_reward_usd: if claimed { reward_usd } else { projected_reward },
        days_remaining: days_rem,
        season_active: true,
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

/// Lists chronological point audit log entries for a user in the ACTIVE season.
pub async fn list_user_point_logs(pool: &PgPool, user_id: Uuid, limit: i64) -> Result<Vec<AirdropPointLog>, sqlx::Error> {
    let Some(season) = get_active_season(pool).await? else {
        return Ok(vec![]);
    };

    let rows = sqlx::query(
        "SELECT id, user_id, season_id, activity_type, description, points, bonus_points, created_at
         FROM airdrop_point_logs
         WHERE user_id = $1 AND season_id = $2
         ORDER BY created_at DESC LIMIT $3"
    )
    .bind(user_id)
    .bind(season.id)
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

