//! Port of legacy `apps/api/src/modules/admin/admin.routes.ts` — the
//! transactional pieces (approve/reject withdrawal, fund HOUSE/LEND_POOL,
//! faucetlist moderation) and rich dashboard aggregates.

use crate::house::HouseError;
use crate::ledger::{apply_ledger_entry, LedgerCreditInput};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::Serialize;
use shared::Coin;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    House(#[from] HouseError),
    #[error("withdrawal not found")]
    WithdrawalNotFound,
    #[error("merchant not found")]
    MerchantNotFound,
    #[error("withdrawal is not pending")]
    NotPending,
    #[error("amount must be > 0")]
    InvalidAmount,
}

#[derive(Debug, Serialize)]
pub struct PendingWithdrawal {
    pub id: Uuid,
    pub email: String,
    pub coin: String,
    pub to_address: String,
    pub amount: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AdminWithdrawalItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub coin: String,
    pub to_address: String,
    pub amount: String,
    pub fee: String,
    pub status: String,
    pub tx_hash: Option<String>,
    pub requires_approval: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AdminMerchantItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub name: String,
    pub website_url: Option<String>,
    pub webhook_url: Option<String>,
    pub description: Option<String>,
    pub status: String,
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
    pub invoices_total: i64,
    pub invoices_paid: i64,
    pub volume_paid: String,
    pub fees_paid: String,
    pub api_keys_count: i64,
    pub last_invoice_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerchantInvoiceWindowStats {
    pub created: i64,
    pub paid: i64,
    pub pending: i64,
    pub expired: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerchantCoinVolume {
    pub coin: String,
    pub paid_count: i64,
    pub amount: String,
    pub fee_amount: String,
    pub net_amount: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerchantTopItem {
    pub merchant_id: Uuid,
    pub email: String,
    pub name: String,
    pub paid_count: i64,
    pub volume_paid: String,
    pub fees_paid: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerchantDayBucket {
    pub day: String,
    pub created: i64,
    pub paid: i64,
    pub expired: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerchantRecentInvoice {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub merchant_email: String,
    pub merchant_name: String,
    pub coin: String,
    pub amount: String,
    pub fee_amount: String,
    pub status: String,
    pub order_id: String,
    pub site_name: Option<String>,
    pub webhook_delivered: bool,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerchantPlatformStats {
    pub accounts_total: i64,
    pub accounts_approved: i64,
    pub accounts_pending: i64,
    pub accounts_rejected: i64,
    pub invoices_all: MerchantInvoiceWindowStats,
    pub invoices_24h: MerchantInvoiceWindowStats,
    pub invoices_7d: MerchantInvoiceWindowStats,
    pub invoices_30d: MerchantInvoiceWindowStats,
    pub conversion_pct: f64,
    pub conversion_24h_pct: f64,
    pub conversion_7d_pct: f64,
    pub merchants_active_30d: i64,
    pub merchants_new_7d: i64,
    pub merchants_new_30d: i64,
    pub webhook_success_pct: f64,
    pub avg_confirm_minutes: Option<f64>,
    pub volume_by_coin: Vec<MerchantCoinVolume>,
    pub volume_by_coin_30d: Vec<MerchantCoinVolume>,
    pub api_keys_active: i64,
    pub api_keys_used_7d: i64,
    pub webhooks_delivered: i64,
    pub webhooks_failed: i64,
    pub top_merchants: Vec<MerchantTopItem>,
    pub series_14d: Vec<MerchantDayBucket>,
    pub recent_invoices: Vec<MerchantRecentInvoice>,
}

#[derive(Debug, Serialize)]
pub struct AdminFaucetItem {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_email: Option<String>,
    pub name: String,
    pub url: String,
    pub description: String,
    pub coins: Vec<String>,
    pub reward_info: Option<String>,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub clicks: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CoinVolumeItem {
    pub coin: String,
    pub count: i64,
    pub total_amount: String,
    pub total_fee: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct RecentDepositItem {
    pub id: Uuid,
    pub email: String,
    pub coin: String,
    pub amount: String,
    pub tx_hash: String,
    pub status: String,
    pub confirmations: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ServerResourceStats {
    pub uptime_seconds: u64,
    pub cpu_load_1m: f64,
    pub cpu_load_5m: f64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_pct: f64,
    pub db_connections_active: u32,
    pub db_connections_idle: u32,
    pub db_connections_max: u32,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct AdminDashboardStats {
    pub total_users: i64,
    pub new_users_24h: i64,
    pub active_users_24h: i64,
    pub total_wallets: i64,
    pub total_merchants: i64,
    pub total_faucet_sites: i64,
    pub pending_withdrawals: i64,
    pub total_deposits_count: i64,
    pub total_withdrawals_count: i64,
    pub deposits_by_coin: Vec<CoinVolumeItem>,
    pub withdrawals_by_coin: Vec<CoinVolumeItem>,
    pub user_balances: Vec<HouseBalanceItem>,
    pub house_balances: Vec<HouseBalanceItem>,
    pub recent_deposits: Vec<RecentDepositItem>,
    pub recent_withdrawals: Vec<AdminWithdrawalItem>,
    pub server: ServerResourceStats,
}

#[derive(Debug, Serialize, Clone)]
pub struct HouseBalanceItem {
    pub coin: String,
    pub balance: String,
}

pub fn get_server_resources(pool: &PgPool) -> ServerResourceStats {
    let uptime_seconds = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
        .map(|v| v as u64)
        .unwrap_or(0);

    let (cpu_load_1m, cpu_load_5m) = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| {
            let mut parts = s.split_whitespace();
            let p1 = parts.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let p2 = parts.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            (p1, p2)
        })
        .unwrap_or((0.0, 0.0));

    let (memory_used_mb, memory_total_mb, memory_pct) = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .map(|s| {
            let mut total_kb = 0u64;
            let mut avail_kb = 0u64;
            for line in s.lines() {
                if line.starts_with("MemTotal:") {
                    total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    avail_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
            let used_kb = total_kb.saturating_sub(avail_kb);
            let total_mb = total_kb / 1024;
            let used_mb = used_kb / 1024;
            let pct = if total_kb > 0 { (used_kb as f64 / total_kb as f64) * 100.0 } else { 0.0 };
            (used_mb, total_mb, (pct * 10.0).round() / 10.0)
        })
        .unwrap_or((0, 0, 0.0));

    let db_connections_max = 50;
    let db_connections_idle = pool.num_idle() as u32;
    let db_connections_active = (pool.size() as u32).saturating_sub(db_connections_idle);

    ServerResourceStats {
        uptime_seconds,
        cpu_load_1m,
        cpu_load_5m,
        memory_used_mb,
        memory_total_mb,
        memory_pct,
        db_connections_active,
        db_connections_idle,
        db_connections_max,
        status: "HEALTHY".to_string(),
    }
}

pub async fn get_dashboard_stats(pool: &PgPool) -> Result<AdminDashboardStats, AdminError> {
    let total_users: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE email <> $1",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let new_users_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE email <> $1 AND created_at >= NOW() - INTERVAL '24 hours'",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let total_wallets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallets w JOIN users u ON u.id = w.user_id \
         WHERE u.email <> $1 AND w.kind = 'PERSONAL'",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let total_merchants: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE merchant_status != 'NONE' AND email <> $1")
        .bind(crate::house::HOUSE_EMAIL)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let total_faucet_sites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM faucet_sites").fetch_one(pool).await.unwrap_or(0);
    let pending_withdrawals: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM withdrawals WHERE status = 'PENDING'").fetch_one(pool).await.unwrap_or(0);
    let total_deposits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM deposits WHERE amount > 0",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let total_withdrawals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM withdrawals WHERE tx_hash IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let active_users_24h: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE last_login_at >= NOW() - INTERVAL '24 hours'").fetch_one(pool).await.unwrap_or(0);

    // Deposit volumes by coin (ignore amount=0 stubs used to block re-credit after ledger wipe)
    let deposit_rows = sqlx::query(
        "SELECT wa.coin::text as coin, COUNT(*) as count, COALESCE(SUM(d.amount), 0) as total_amount \
         FROM deposits d JOIN wallets wa ON wa.id = d.wallet_id \
         WHERE d.status = 'CREDITED' AND d.amount > 0 GROUP BY wa.coin"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let deposits_by_coin: Vec<CoinVolumeItem> = deposit_rows
        .into_iter()
        .map(|r| CoinVolumeItem {
            coin: r.get("coin"),
            count: r.get("count"),
            total_amount: r.get::<BigDecimal, _>("total_amount").to_string(),
            total_fee: None,
        })
        .collect();

    // Withdrawal volumes by coin
    let withdrawal_rows = sqlx::query(
        "SELECT wa.coin::text as coin, COUNT(*) as count, COALESCE(SUM(w.amount), 0) as total_amount, COALESCE(SUM(w.fee_amount), 0) as total_fee \
         FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id \
         WHERE w.status IN ('BROADCASTED', 'CONFIRMED') GROUP BY wa.coin"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let withdrawals_by_coin: Vec<CoinVolumeItem> = withdrawal_rows
        .into_iter()
        .map(|r| CoinVolumeItem {
            coin: r.get("coin"),
            count: r.get("count"),
            total_amount: r.get::<BigDecimal, _>("total_amount").to_string(),
            total_fee: Some(r.get::<BigDecimal, _>("total_fee").to_string()),
        })
        .collect();

    // User total custody balances
    let user_balance_rows = sqlx::query(
        "SELECT wa.coin::text as coin, COALESCE(SUM(le.amount), 0) as total_balance \
         FROM ledger_entries le JOIN wallets wa ON wa.id = le.wallet_id JOIN users u ON u.id = wa.user_id \
         WHERE u.email <> $1 GROUP BY wa.coin"
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let user_balances: Vec<HouseBalanceItem> = user_balance_rows
        .into_iter()
        .map(|r| HouseBalanceItem {
            coin: r.get("coin"),
            balance: r.get::<BigDecimal, _>("total_balance").to_string(),
        })
        .collect();

    // House treasury balances
    let house_balances = match crate::house::list_house_balances(pool).await {
        Ok(rows) => rows
            .into_iter()
            .map(|(coin, bal)| HouseBalanceItem { coin: coin.as_str().to_string(), balance: bal.to_string() })
            .collect(),
        Err(_) => Vec::new(),
    };

    // Recent 6 deposits (skip zero-amount wipe stubs)
    let recent_deposits_rows = sqlx::query(
        "SELECT d.id, u.email, wa.coin::text as coin, d.amount, d.tx_hash, d.status::text as status, d.confirmations, d.detected_at \
         FROM deposits d JOIN wallets wa ON wa.id = d.wallet_id JOIN users u ON u.id = wa.user_id \
         WHERE d.amount > 0 \
         ORDER BY d.detected_at DESC LIMIT 6"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let recent_deposits: Vec<RecentDepositItem> = recent_deposits_rows
        .into_iter()
        .map(|r| RecentDepositItem {
            id: r.get("id"),
            email: r.get("email"),
            coin: r.get("coin"),
            amount: r.get::<BigDecimal, _>("amount").to_string(),
            tx_hash: r.get("tx_hash"),
            status: r.get("status"),
            confirmations: r.get("confirmations"),
            created_at: r.get("detected_at"),
        })
        .collect();

    // Recent 6 withdrawals
    let recent_withdrawals = list_all_withdrawals(pool, None, 6).await.unwrap_or_default();

    let server = get_server_resources(pool);

    Ok(AdminDashboardStats {
        total_users,
        new_users_24h,
        active_users_24h,
        total_wallets,
        total_merchants,
        total_faucet_sites,
        pending_withdrawals,
        total_deposits_count: total_deposits,
        total_withdrawals_count: total_withdrawals,
        deposits_by_coin,
        withdrawals_by_coin,
        user_balances,
        house_balances,
        recent_deposits,
        recent_withdrawals,
        server,
    })
}

pub async fn list_pending_withdrawals(pool: &PgPool) -> Result<Vec<PendingWithdrawal>, AdminError> {
    let rows = sqlx::query(
        "SELECT w.id, u.email, wa.coin::text as coin, w.to_address, w.amount, w.created_at \
         FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id JOIN users u ON u.id = wa.user_id \
         WHERE w.status = 'PENDING' AND w.requires_approval = true ORDER BY w.created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| PendingWithdrawal { id: r.get("id"), email: r.get("email"), coin: r.get("coin"), to_address: r.get("to_address"), amount: r.get::<BigDecimal, _>("amount").to_string(), created_at: r.get("created_at") }).collect())
}

const WITHDRAWAL_STATUSES: &[&str] = &[
    "PENDING", "APPROVED", "QUEUED", "BROADCASTING", "BROADCASTED", "CONFIRMED", "FAILED", "CANCELED",
];

pub async fn list_all_withdrawals(pool: &PgPool, status: Option<&str>, limit: i64) -> Result<Vec<AdminWithdrawalItem>, AdminError> {
    let limit = limit.clamp(1, 200);
    let filter = status.filter(|s| WITHDRAWAL_STATUSES.contains(s));
    let sql = if filter.is_some() {
        "SELECT w.id, u.id as user_id, u.email, wa.coin::text as coin, w.to_address, w.amount, w.fee_amount, \
                w.status::text as status, w.tx_hash, w.requires_approval, w.created_at \
         FROM withdrawals w \
         JOIN wallets wa ON wa.id = w.wallet_id \
         JOIN users u ON u.id = wa.user_id \
         WHERE w.status = $1::withdrawal_status \
         ORDER BY w.created_at DESC LIMIT $2"
    } else {
        "SELECT w.id, u.id as user_id, u.email, wa.coin::text as coin, w.to_address, w.amount, w.fee_amount, \
                w.status::text as status, w.tx_hash, w.requires_approval, w.created_at \
         FROM withdrawals w \
         JOIN wallets wa ON wa.id = w.wallet_id \
         JOIN users u ON u.id = wa.user_id \
         ORDER BY w.created_at DESC LIMIT $1"
    };

    let rows = if let Some(st) = filter {
        sqlx::query(sql).bind(st).bind(limit).fetch_all(pool).await?
    } else {
        sqlx::query(sql).bind(limit).fetch_all(pool).await?
    };
    Ok(rows.into_iter().map(|r| AdminWithdrawalItem {
        id: r.get("id"),
        user_id: r.get("user_id"),
        email: r.get("email"),
        coin: r.get("coin"),
        to_address: r.get("to_address"),
        amount: r.get::<BigDecimal, _>("amount").to_string(),
        fee: r.get::<BigDecimal, _>("fee_amount").to_string(),
        status: r.get("status"),
        tx_hash: r.get("tx_hash"),
        requires_approval: r.get("requires_approval"),
        created_at: r.get("created_at"),
    }).collect())
}

pub async fn list_all_merchants(pool: &PgPool) -> Result<Vec<AdminMerchantItem>, AdminError> {
    let rows = sqlx::query(
        "SELECT u.id, u.id as user_id, u.email, \
                COALESCE(NULLIF(u.merchant_business_name, ''), u.email) as name, \
                u.merchant_website as website_url, \
                NULL::text as webhook_url, \
                u.merchant_description as description, \
                u.merchant_status::text as status, \
                (u.merchant_status = 'APPROVED') as is_verified, \
                COALESCE(u.merchant_applied_at, u.created_at) as created_at, \
                COALESCE(inv.invoices_total, 0)::bigint as invoices_total, \
                COALESCE(inv.invoices_paid, 0)::bigint as invoices_paid, \
                COALESCE(inv.volume_paid, 0)::text as volume_paid, \
                COALESCE(inv.fees_paid, 0)::text as fees_paid, \
                COALESCE(keys.api_keys_count, 0)::bigint as api_keys_count, \
                inv.last_invoice_at \
         FROM users u \
         LEFT JOIN ( \
             SELECT merchant_id, \
                    COUNT(*)::bigint as invoices_total, \
                    COUNT(*) FILTER (WHERE status = 'CONFIRMED')::bigint as invoices_paid, \
                    COALESCE(SUM(amount) FILTER (WHERE status = 'CONFIRMED'), 0) as volume_paid, \
                    COALESCE(SUM(fee_amount) FILTER (WHERE status = 'CONFIRMED'), 0) as fees_paid, \
                    MAX(created_at) as last_invoice_at \
             FROM merchant_deposit_invoices \
             GROUP BY merchant_id \
         ) inv ON inv.merchant_id = u.id \
         LEFT JOIN ( \
             SELECT user_id, COUNT(*)::bigint as api_keys_count \
             FROM api_keys \
             WHERE disabled_at IS NULL \
             GROUP BY user_id \
         ) keys ON keys.user_id = u.id \
         WHERE u.merchant_status != 'NONE' AND u.email <> $1 \
         ORDER BY u.created_at DESC LIMIT 100"
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| AdminMerchantItem {
            id: r.get("id"),
            user_id: r.get("user_id"),
            email: r.get("email"),
            name: r.get("name"),
            website_url: r.get("website_url"),
            webhook_url: r.get("webhook_url"),
            description: r.get("description"),
            status: r.get("status"),
            is_verified: r.get("is_verified"),
            created_at: r.get("created_at"),
            invoices_total: r.get("invoices_total"),
            invoices_paid: r.get("invoices_paid"),
            volume_paid: r.get("volume_paid"),
            fees_paid: r.get("fees_paid"),
            api_keys_count: r.get("api_keys_count"),
            last_invoice_at: r.get("last_invoice_at"),
        })
        .collect())
}

fn invoice_window_from_row(r: &sqlx::postgres::PgRow) -> MerchantInvoiceWindowStats {
    MerchantInvoiceWindowStats {
        created: r.get("created"),
        paid: r.get("paid"),
        pending: r.get("pending"),
        expired: r.get("expired"),
    }
}

fn conversion_pct(w: &MerchantInvoiceWindowStats) -> f64 {
    if w.created > 0 {
        (w.paid as f64) * 100.0 / (w.created as f64)
    } else {
        0.0
    }
}

fn map_coin_volume_rows(rows: Vec<sqlx::postgres::PgRow>) -> Vec<MerchantCoinVolume> {
    rows.into_iter()
        .map(|r| MerchantCoinVolume {
            coin: r.get("coin"),
            paid_count: r.get("paid_count"),
            amount: r.get("amount"),
            fee_amount: r.get("fee_amount"),
            net_amount: r.get("net_amount"),
        })
        .collect()
}

async fn invoice_window_since(
    pool: &PgPool,
    interval: &str,
) -> Result<MerchantInvoiceWindowStats, AdminError> {
    let q = format!(
        "SELECT \
            COUNT(*)::bigint as created, \
            COUNT(*) FILTER (WHERE status = 'CONFIRMED')::bigint as paid, \
            COUNT(*) FILTER (WHERE status IN ('PENDING','DETECTED'))::bigint as pending, \
            COUNT(*) FILTER (WHERE status IN ('EXPIRED','CANCELLED'))::bigint as expired \
         FROM merchant_deposit_invoices \
         WHERE created_at >= NOW() - INTERVAL '{interval}'"
    );
    let row = sqlx::query(&q).fetch_one(pool).await?;
    Ok(invoice_window_from_row(&row))
}

async fn volume_by_coin_since(
    pool: &PgPool,
    interval: Option<&str>,
) -> Result<Vec<MerchantCoinVolume>, AdminError> {
    let sql = if let Some(interval) = interval {
        format!(
            "SELECT coin::text as coin, \
                    COUNT(*)::bigint as paid_count, \
                    COALESCE(SUM(amount), 0)::text as amount, \
                    COALESCE(SUM(fee_amount), 0)::text as fee_amount, \
                    COALESCE(SUM(net_amount), 0)::text as net_amount \
             FROM merchant_deposit_invoices \
             WHERE status = 'CONFIRMED' \
               AND COALESCE(paid_at, created_at) >= NOW() - INTERVAL '{interval}' \
             GROUP BY coin \
             ORDER BY coin"
        )
    } else {
        "SELECT coin::text as coin, \
                COUNT(*)::bigint as paid_count, \
                COALESCE(SUM(amount), 0)::text as amount, \
                COALESCE(SUM(fee_amount), 0)::text as fee_amount, \
                COALESCE(SUM(net_amount), 0)::text as net_amount \
         FROM merchant_deposit_invoices \
         WHERE status = 'CONFIRMED' \
         GROUP BY coin \
         ORDER BY coin"
            .to_string()
    };
    let rows = sqlx::query(&sql).fetch_all(pool).await.unwrap_or_default();
    Ok(map_coin_volume_rows(rows))
}

pub async fn get_merchant_platform_stats(pool: &PgPool) -> Result<MerchantPlatformStats, AdminError> {
    let accounts = sqlx::query(
        "SELECT \
            COUNT(*) FILTER (WHERE merchant_status != 'NONE')::bigint as total, \
            COUNT(*) FILTER (WHERE merchant_status = 'APPROVED')::bigint as approved, \
            COUNT(*) FILTER (WHERE merchant_status = 'PENDING')::bigint as pending, \
            COUNT(*) FILTER (WHERE merchant_status = 'REJECTED')::bigint as rejected \
         FROM users WHERE email <> $1",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_one(pool)
    .await?;

    let invoices_all = sqlx::query(
        "SELECT \
            COUNT(*)::bigint as created, \
            COUNT(*) FILTER (WHERE status = 'CONFIRMED')::bigint as paid, \
            COUNT(*) FILTER (WHERE status IN ('PENDING','DETECTED'))::bigint as pending, \
            COUNT(*) FILTER (WHERE status IN ('EXPIRED','CANCELLED'))::bigint as expired \
         FROM merchant_deposit_invoices",
    )
    .fetch_one(pool)
    .await?;

    let invoices_24h = invoice_window_since(pool, "24 hours").await?;
    let invoices_7d = invoice_window_since(pool, "7 days").await?;
    let invoices_30d = invoice_window_since(pool, "30 days").await?;
    let all = invoice_window_from_row(&invoices_all);
    let conversion_all = conversion_pct(&all);
    let conversion_24h = conversion_pct(&invoices_24h);
    let conversion_7d = conversion_pct(&invoices_7d);

    let volume_by_coin = volume_by_coin_since(pool, None).await?;
    let volume_by_coin_30d = volume_by_coin_since(pool, Some("30 days")).await?;

    let keys = sqlx::query(
        "SELECT \
            COUNT(*) FILTER (WHERE disabled_at IS NULL)::bigint as active, \
            COUNT(*) FILTER (WHERE disabled_at IS NULL AND last_used_at >= NOW() - INTERVAL '7 days')::bigint as used_7d \
         FROM api_keys ak \
         JOIN users u ON u.id = ak.user_id \
         WHERE u.merchant_status != 'NONE' AND u.email <> $1",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_one(pool)
    .await?;

    let hooks = sqlx::query(
        "SELECT \
            COUNT(*) FILTER (WHERE webhook_delivered)::bigint as delivered, \
            COUNT(*) FILTER (WHERE status = 'CONFIRMED' AND NOT webhook_delivered)::bigint as failed \
         FROM merchant_deposit_invoices \
         WHERE status = 'CONFIRMED'",
    )
    .fetch_one(pool)
    .await?;

    let delivered: i64 = hooks.get("delivered");
    let failed: i64 = hooks.get("failed");
    let webhook_success_pct = if delivered + failed > 0 {
        (delivered as f64) * 100.0 / ((delivered + failed) as f64)
    } else {
        100.0
    };

    let merchants_active_30d: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT merchant_id)::bigint \
         FROM merchant_deposit_invoices \
         WHERE status = 'CONFIRMED' \
           AND COALESCE(paid_at, created_at) >= NOW() - INTERVAL '30 days'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let merchants_new_7d: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM users \
         WHERE email <> $1 AND merchant_status != 'NONE' \
           AND COALESCE(merchant_applied_at, created_at) >= NOW() - INTERVAL '7 days'",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let merchants_new_30d: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM users \
         WHERE email <> $1 AND merchant_status != 'NONE' \
           AND COALESCE(merchant_applied_at, created_at) >= NOW() - INTERVAL '30 days'",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let avg_confirm_minutes: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(EXTRACT(EPOCH FROM (paid_at - created_at)) / 60.0) \
         FROM merchant_deposit_invoices \
         WHERE status = 'CONFIRMED' AND paid_at IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten();

    let series_rows = sqlx::query(
        "SELECT to_char(date_trunc('day', created_at), 'YYYY-MM-DD') as day, \
                COUNT(*)::bigint as created, \
                COUNT(*) FILTER (WHERE status = 'CONFIRMED')::bigint as paid, \
                COUNT(*) FILTER (WHERE status IN ('EXPIRED','CANCELLED'))::bigint as expired \
         FROM merchant_deposit_invoices \
         WHERE created_at >= NOW() - INTERVAL '14 days' \
         GROUP BY 1 \
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let top_rows = sqlx::query(
        "SELECT u.id as merchant_id, u.email, \
                COALESCE(NULLIF(u.merchant_business_name, ''), u.email) as name, \
                COUNT(*)::bigint as paid_count, \
                COALESCE(SUM(i.amount), 0)::text as volume_paid, \
                COALESCE(SUM(i.fee_amount), 0)::text as fees_paid \
         FROM merchant_deposit_invoices i \
         JOIN users u ON u.id = i.merchant_id \
         WHERE i.status = 'CONFIRMED' AND u.email <> $1 \
         GROUP BY u.id, u.email, u.merchant_business_name \
         ORDER BY SUM(i.amount) DESC \
         LIMIT 10",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let recent_rows = sqlx::query(
        "SELECT i.id, i.merchant_id, u.email as merchant_email, \
                COALESCE(NULLIF(u.merchant_business_name, ''), u.email) as merchant_name, \
                i.coin::text as coin, \
                i.amount::text as amount, \
                i.fee_amount::text as fee_amount, \
                i.status::text as status, \
                i.order_id, \
                i.site_name, \
                i.webhook_delivered, \
                i.created_at, \
                i.paid_at \
         FROM merchant_deposit_invoices i \
         JOIN users u ON u.id = i.merchant_id \
         WHERE u.email <> $1 \
         ORDER BY i.created_at DESC \
         LIMIT 20",
    )
    .bind(crate::house::HOUSE_EMAIL)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(MerchantPlatformStats {
        accounts_total: accounts.get("total"),
        accounts_approved: accounts.get("approved"),
        accounts_pending: accounts.get("pending"),
        accounts_rejected: accounts.get("rejected"),
        invoices_all: all,
        invoices_24h,
        invoices_7d,
        invoices_30d,
        conversion_pct: conversion_all,
        conversion_24h_pct: conversion_24h,
        conversion_7d_pct: conversion_7d,
        merchants_active_30d,
        merchants_new_7d,
        merchants_new_30d,
        webhook_success_pct,
        avg_confirm_minutes,
        volume_by_coin,
        volume_by_coin_30d,
        api_keys_active: keys.get("active"),
        api_keys_used_7d: keys.get("used_7d"),
        webhooks_delivered: delivered,
        webhooks_failed: failed,
        top_merchants: top_rows
            .into_iter()
            .map(|r| MerchantTopItem {
                merchant_id: r.get("merchant_id"),
                email: r.get("email"),
                name: r.get("name"),
                paid_count: r.get("paid_count"),
                volume_paid: r.get("volume_paid"),
                fees_paid: r.get("fees_paid"),
            })
            .collect(),
        series_14d: series_rows
            .into_iter()
            .map(|r| MerchantDayBucket {
                day: r.get("day"),
                created: r.get("created"),
                paid: r.get("paid"),
                expired: r.get("expired"),
            })
            .collect(),
        recent_invoices: recent_rows
            .into_iter()
            .map(|r| MerchantRecentInvoice {
                id: r.get("id"),
                merchant_id: r.get("merchant_id"),
                merchant_email: r.get("merchant_email"),
                merchant_name: r.get("merchant_name"),
                coin: r.get("coin"),
                amount: r.get("amount"),
                fee_amount: r.get("fee_amount"),
                status: r.get("status"),
                order_id: r.get("order_id"),
                site_name: r.get("site_name"),
                webhook_delivered: r.get("webhook_delivered"),
                created_at: r.get("created_at"),
                paid_at: r.get("paid_at"),
            })
            .collect(),
    })
}

#[derive(Debug, Serialize, Clone)]
pub struct EconCoinFlow {
    pub coin: String,
    pub count: i64,
    pub volume: String,
    pub fees: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct NetworkFeeByKind {
    pub coin: String,
    pub kind: String,
    pub count: i64,
    pub amount: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FeeMarginByCoin {
    pub coin: String,
    pub fees_earned: String,
    pub network_paid: String,
    pub faucet_cost: String,
    pub fee_margin: String,
    pub operating_margin: String,
    pub healthy: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct EconWindow {
    pub faucet_claims: i64,
    pub faucet_unique_users: i64,
    pub faucet_by_coin: Vec<EconCoinFlow>,
    pub gateway_created: i64,
    pub gateway_paid: i64,
    pub gateway_by_coin: Vec<EconCoinFlow>,
    pub deposits_count: i64,
    pub deposits_by_coin: Vec<EconCoinFlow>,
    pub withdrawals_count: i64,
    pub withdrawals_by_coin: Vec<EconCoinFlow>,
    pub swap_count: i64,
    pub swap_fees_by_coin: Vec<EconCoinFlow>,
    pub network_by_kind: Vec<NetworkFeeByKind>,
    pub fee_margin_by_coin: Vec<FeeMarginByCoin>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PlatformEconomics {
    pub all_time: EconWindow,
    pub last_24h: EconWindow,
}

fn map_econ_coin_rows(rows: Vec<sqlx::postgres::PgRow>) -> Vec<EconCoinFlow> {
    rows.into_iter()
        .map(|r| EconCoinFlow {
            coin: r.get("coin"),
            count: r.get("count"),
            volume: r.get::<BigDecimal, _>("volume").to_string(),
            fees: r.get::<BigDecimal, _>("fees").to_string(),
        })
        .collect()
}

fn parse_i128(s: &str) -> i128 {
    s.parse().unwrap_or(0)
}

fn sum_fees_by_coin(rows: &[EconCoinFlow]) -> std::collections::BTreeMap<String, i128> {
    let mut map = std::collections::BTreeMap::new();
    for r in rows {
        *map.entry(r.coin.clone()).or_default() += parse_i128(&r.fees);
    }
    map
}

fn sum_volume_by_coin(rows: &[EconCoinFlow]) -> std::collections::BTreeMap<String, i128> {
    let mut map = std::collections::BTreeMap::new();
    for r in rows {
        *map.entry(r.coin.clone()).or_default() += parse_i128(&r.volume);
    }
    map
}

fn build_fee_margins(
    gateway_by_coin: &[EconCoinFlow],
    withdrawals_by_coin: &[EconCoinFlow],
    swap_fees_by_coin: &[EconCoinFlow],
    faucet_by_coin: &[EconCoinFlow],
    network_by_kind: &[NetworkFeeByKind],
) -> Vec<FeeMarginByCoin> {
    let mut earned = sum_fees_by_coin(gateway_by_coin);
    for (c, v) in sum_fees_by_coin(withdrawals_by_coin) {
        *earned.entry(c).or_default() += v;
    }
    for (c, v) in sum_fees_by_coin(swap_fees_by_coin) {
        *earned.entry(c).or_default() += v;
    }
    let faucet = sum_volume_by_coin(faucet_by_coin);
    let mut network: std::collections::BTreeMap<String, i128> = std::collections::BTreeMap::new();
    for r in network_by_kind {
        *network.entry(r.coin.clone()).or_default() += parse_i128(&r.amount);
    }

    let mut coins: std::collections::BTreeSet<String> = earned.keys().cloned().collect();
    coins.extend(faucet.keys().cloned());
    coins.extend(network.keys().cloned());

    coins
        .into_iter()
        .map(|coin| {
            let fees_earned = *earned.get(&coin).unwrap_or(&0);
            let network_paid = *network.get(&coin).unwrap_or(&0);
            let faucet_cost = *faucet.get(&coin).unwrap_or(&0);
            let fee_margin = fees_earned - network_paid;
            let operating_margin = fee_margin - faucet_cost;
            FeeMarginByCoin {
                coin,
                fees_earned: fees_earned.to_string(),
                network_paid: network_paid.to_string(),
                faucet_cost: faucet_cost.to_string(),
                fee_margin: fee_margin.to_string(),
                operating_margin: operating_margin.to_string(),
                healthy: fee_margin >= 0,
            }
        })
        .filter(|m| m.fees_earned != "0" || m.network_paid != "0" || m.faucet_cost != "0")
        .collect()
}

async fn load_econ_window(pool: &PgPool, since_24h: bool) -> Result<EconWindow, AdminError> {
    let since_sql = if since_24h {
        " AND created_at >= NOW() - INTERVAL '24 hours'"
    } else {
        ""
    };
    let dep_since = if since_24h {
        " AND d.detected_at >= NOW() - INTERVAL '24 hours'"
    } else {
        ""
    };
    let wd_since = if since_24h {
        " AND w.created_at >= NOW() - INTERVAL '24 hours'"
    } else {
        ""
    };
    let swap_since = if since_24h {
        " AND created_at >= NOW() - INTERVAL '24 hours'"
    } else {
        ""
    };
    let inv_since = if since_24h {
        " AND created_at >= NOW() - INTERVAL '24 hours'"
    } else {
        ""
    };
    let inv_paid_since = if since_24h {
        " AND paid_at >= NOW() - INTERVAL '24 hours'"
    } else {
        ""
    };
    let net_since = if since_24h {
        " AND created_at >= NOW() - INTERVAL '24 hours'"
    } else {
        ""
    };

    let faucet_claims: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*)::bigint FROM faucet_claims WHERE TRUE{since_sql}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let faucet_unique_users: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(DISTINCT user_id)::bigint FROM faucet_claims WHERE TRUE{since_sql}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let faucet_by_coin = map_econ_coin_rows(
        sqlx::query(&format!(
            "SELECT coin::text as coin, COUNT(*)::bigint as count, \
                    COALESCE(SUM(amount), 0) as volume, 0::numeric as fees \
             FROM faucet_claims WHERE TRUE{since_sql} GROUP BY coin ORDER BY coin"
        ))
        .fetch_all(pool)
        .await
        .unwrap_or_default(),
    );

    let gateway_created: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*)::bigint FROM merchant_deposit_invoices WHERE TRUE{inv_since}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let gateway_paid: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*)::bigint FROM merchant_deposit_invoices \
         WHERE status = 'CONFIRMED'{inv_paid_since}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let gateway_by_coin = map_econ_coin_rows(
        sqlx::query(&format!(
            "SELECT coin::text as coin, COUNT(*)::bigint as count, \
                    COALESCE(SUM(amount), 0) as volume, \
                    COALESCE(SUM(fee_amount), 0) as fees \
             FROM merchant_deposit_invoices \
             WHERE status = 'CONFIRMED'{inv_paid_since} \
             GROUP BY coin ORDER BY coin"
        ))
        .fetch_all(pool)
        .await
        .unwrap_or_default(),
    );

    let deposits_count: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*)::bigint FROM deposits d \
         WHERE d.status = 'CREDITED' AND d.amount > 0{dep_since}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let deposits_by_coin = map_econ_coin_rows(
        sqlx::query(&format!(
            "SELECT wa.coin::text as coin, COUNT(*)::bigint as count, \
                    COALESCE(SUM(d.amount), 0) as volume, 0::numeric as fees \
             FROM deposits d JOIN wallets wa ON wa.id = d.wallet_id \
             WHERE d.status = 'CREDITED' AND d.amount > 0{dep_since} \
             GROUP BY wa.coin ORDER BY wa.coin"
        ))
        .fetch_all(pool)
        .await
        .unwrap_or_default(),
    );

    let withdrawals_count: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*)::bigint FROM withdrawals w \
         WHERE w.status IN ('BROADCASTED', 'CONFIRMED'){wd_since}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let withdrawals_by_coin = map_econ_coin_rows(
        sqlx::query(&format!(
            "SELECT wa.coin::text as coin, COUNT(*)::bigint as count, \
                    COALESCE(SUM(w.amount), 0) as volume, \
                    COALESCE(SUM(w.fee_amount), 0) as fees \
             FROM withdrawals w JOIN wallets wa ON wa.id = w.wallet_id \
             WHERE w.status IN ('BROADCASTED', 'CONFIRMED'){wd_since} \
             GROUP BY wa.coin ORDER BY wa.coin"
        ))
        .fetch_all(pool)
        .await
        .unwrap_or_default(),
    );

    let swap_count: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*)::bigint FROM dex_swaps \
         WHERE status = 'COMPLETED'{swap_since}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let swap_fees_by_coin = map_econ_coin_rows(
        sqlx::query(&format!(
            "SELECT from_coin::text as coin, COUNT(*)::bigint as count, \
                    COALESCE(SUM(from_amount), 0) as volume, \
                    COALESCE(SUM(platform_fee_amount), 0) as fees \
             FROM dex_swaps \
             WHERE status = 'COMPLETED'{swap_since} \
             GROUP BY from_coin ORDER BY from_coin"
        ))
        .fetch_all(pool)
        .await
        .unwrap_or_default(),
    );

    let network_rows = sqlx::query(&format!(
        "SELECT coin::text as coin, kind, COUNT(*)::bigint as count, \
                COALESCE(SUM(amount), 0) as amount \
         FROM network_fee_events WHERE TRUE{net_since} \
         GROUP BY coin, kind ORDER BY coin, kind"
    ))
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let network_by_kind: Vec<NetworkFeeByKind> = network_rows
        .into_iter()
        .map(|r| NetworkFeeByKind {
            coin: r.get("coin"),
            kind: r.get("kind"),
            count: r.get("count"),
            amount: r.get::<BigDecimal, _>("amount").to_string(),
        })
        .collect();

    let fee_margin_by_coin = build_fee_margins(
        &gateway_by_coin,
        &withdrawals_by_coin,
        &swap_fees_by_coin,
        &faucet_by_coin,
        &network_by_kind,
    );

    Ok(EconWindow {
        faucet_claims,
        faucet_unique_users,
        faucet_by_coin,
        gateway_created,
        gateway_paid,
        gateway_by_coin,
        deposits_count,
        deposits_by_coin,
        withdrawals_count,
        withdrawals_by_coin,
        swap_count,
        swap_fees_by_coin,
        network_by_kind,
        fee_margin_by_coin,
    })
}

pub async fn get_platform_economics(pool: &PgPool) -> Result<PlatformEconomics, AdminError> {
    Ok(PlatformEconomics {
        all_time: load_econ_window(pool, false).await?,
        last_24h: load_econ_window(pool, true).await?,
    })
}

pub async fn approve_merchant(pool: &PgPool, merchant_id: Uuid, admin_id: Uuid) -> Result<(), AdminError> {
    let result = sqlx::query("UPDATE users SET merchant_status = 'APPROVED'::merchant_status, merchant_reviewed_at = now(), merchant_reviewed_by_id = $2, updated_at = now() WHERE id = $1")
        .bind(merchant_id)
        .bind(admin_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AdminError::MerchantNotFound);
    }
    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id) VALUES ($1, 'MERCHANT_APPROVE', 'User', $2)")
        .bind(admin_id)
        .bind(merchant_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn suspend_merchant(pool: &PgPool, merchant_id: Uuid, admin_id: Uuid) -> Result<(), AdminError> {
    let result = sqlx::query("UPDATE users SET merchant_status = 'REJECTED'::merchant_status, merchant_reviewed_at = now(), merchant_reviewed_by_id = $2, updated_at = now() WHERE id = $1")
        .bind(merchant_id)
        .bind(admin_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AdminError::MerchantNotFound);
    }
    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id) VALUES ($1, 'MERCHANT_SUSPEND', 'User', $2)")
        .bind(admin_id)
        .bind(merchant_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_all_faucet_sites(pool: &PgPool) -> Result<Vec<AdminFaucetItem>, AdminError> {
    let rows = sqlx::query(
        "SELECT fs.id, fs.owner_id, u.email as owner_email, fs.name, fs.url, \
                fs.description, fs.coins::text[] as coins, fs.reward_info, \
                fs.status::text as status, fs.rejection_reason, fs.clicks, fs.created_at \
         FROM faucet_sites fs \
         LEFT JOIN users u ON u.id = fs.owner_id \
         ORDER BY fs.created_at DESC LIMIT 100"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| AdminFaucetItem {
        id: r.get("id"),
        owner_id: r.get("owner_id"),
        owner_email: r.get("owner_email"),
        name: r.get("name"),
        url: r.get("url"),
        description: r.get("description"),
        coins: r.get("coins"),
        reward_info: r.get("reward_info"),
        status: r.get("status"),
        rejection_reason: r.get("rejection_reason"),
        clicks: r.get("clicks"),
        created_at: r.get("created_at"),
    }).collect())
}

/// Atomic PENDING → APPROVED.
pub async fn approve_withdrawal(pool: &PgPool, withdrawal_id: Uuid, admin_id: Uuid, ip: Option<&str>) -> Result<(), AdminError> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query("UPDATE withdrawals SET status = 'APPROVED'::withdrawal_status, approved_by_id = $2, approved_at = now() WHERE id = $1 AND status = 'PENDING'::withdrawal_status")
        .bind(withdrawal_id)
        .bind(admin_id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AdminError::NotPending);
    }
    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id, ip) VALUES ($1, 'WITHDRAWAL_APPROVE', 'Withdrawal', $2, $3)")
        .bind(admin_id)
        .bind(withdrawal_id)
        .bind(ip)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Atomic PENDING → CANCELED + reversal credit.
pub async fn reject_withdrawal(pool: &PgPool, withdrawal_id: Uuid, admin_id: Uuid, ip: Option<&str>) -> Result<(), AdminError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query("SELECT wallet_id, amount, fee_amount FROM withdrawals WHERE id = $1").bind(withdrawal_id).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Err(AdminError::WithdrawalNotFound) };
    let wallet_id: Uuid = row.get("wallet_id");
    let amount: BigDecimal = row.get("amount");
    let fee_amount: BigDecimal = row.get("fee_amount");
    let reversal = amount + fee_amount;

    let result = sqlx::query("UPDATE withdrawals SET status = 'CANCELED'::withdrawal_status, updated_at = now() WHERE id = $1 AND status = 'PENDING'::withdrawal_status")
        .bind(withdrawal_id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AdminError::NotPending);
    }

    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None, wallet_id, amount: reversal, ledger_type: "WITHDRAWAL_REVERSAL", reference_id: Some(withdrawal_id), reference_type: Some("Withdrawal"), memo: Some("Rejected by admin") },
    )
    .await
    .map_err(|e| AdminError::Db(sqlx::Error::Protocol(e.to_string())))?;

    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id, ip) VALUES ($1, 'WITHDRAWAL_REJECT', 'Withdrawal', $2, $3)")
        .bind(admin_id)
        .bind(withdrawal_id)
        .bind(ip)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Admin-only: mint inventory into HOUSE (ops funding).
pub async fn fund_house(pool: &PgPool, coin: Coin, amount: u128, admin_id: Uuid) -> Result<(), AdminError> {
    if amount == 0 {
        return Err(AdminError::InvalidAmount);
    }
    let mut tx = pool.begin().await?;
    let wallet_id = crate::house::get_house_wallet_id(&mut tx, coin).await?;
    let reference_id = Uuid::new_v4();
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None, wallet_id, amount: BigDecimal::from(amount), ledger_type: "ADJUSTMENT", reference_id: Some(reference_id), reference_type: Some("HouseFund"), memo: Some(&format!("Admin fund HOUSE {}", coin.as_str())) },
    )
    .await
    .map_err(|e| AdminError::Db(sqlx::Error::Protocol(e.to_string())))?;
    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id, metadata) VALUES ($1, 'HOUSE_FUND', 'Wallet', $2, $3)")
        .bind(admin_id)
        .bind(wallet_id)
        .bind(serde_json::json!({ "coin": coin.as_str(), "amount": amount.to_string() }))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Admin-only: seed liquidity into a lending pool (LEND_POOL) for a coin.
pub async fn fund_lend_pool(pool: &PgPool, coin: Coin, amount: u128, admin_id: Uuid) -> Result<(), AdminError> {
    if amount == 0 {
        return Err(AdminError::InvalidAmount);
    }
    let mut tx = pool.begin().await?;
    let wallet_id = crate::house::get_lend_pool_wallet_id(&mut tx, coin).await?;
    let reference_id = Uuid::new_v4();
    apply_ledger_entry(
        &mut tx,
        LedgerCreditInput { reference_key: None, wallet_id, amount: BigDecimal::from(amount), ledger_type: "ADJUSTMENT", reference_id: Some(reference_id), reference_type: Some("LendPoolFund"), memo: Some(&format!("Admin seed LEND_POOL {}", coin.as_str())) },
    )
    .await
    .map_err(|e| AdminError::Db(sqlx::Error::Protocol(e.to_string())))?;
    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id, metadata) VALUES ($1, 'LEND_POOL_FUND', 'Wallet', $2, $3)")
        .bind(admin_id)
        .bind(wallet_id)
        .bind(serde_json::json!({ "coin": coin.as_str(), "amount": amount.to_string() }))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// --- Faucetlist moderation (approve/reject/suspend) ---

async fn transition_faucet_site(pool: &PgPool, id: Uuid, admin_id: Uuid, from: &[&str], to: &str, action: &str, rejection_reason: Option<&str>) -> Result<(), AdminError> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query("UPDATE faucet_sites SET status = $2::faucet_site_status, rejection_reason = $3, reviewed_by_id = $4, reviewed_at = now(), updated_at = now() WHERE id = $1 AND status::text = ANY($5)")
        .bind(id)
        .bind(to)
        .bind(rejection_reason)
        .bind(admin_id)
        .bind(from)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AdminError::NotPending);
    }
    sqlx::query("INSERT INTO audit_logs (user_id, action, entity, entity_id) VALUES ($1, $2, 'FaucetSite', $3)").bind(admin_id).bind(action).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn approve_faucet_site(pool: &PgPool, id: Uuid, admin_id: Uuid) -> Result<(), AdminError> {
    transition_faucet_site(pool, id, admin_id, &["PENDING", "SUSPENDED"], "APPROVED", "FAUCETSITE_APPROVE", None).await
}

pub async fn reject_faucet_site(pool: &PgPool, id: Uuid, admin_id: Uuid, reason: &str) -> Result<(), AdminError> {
    transition_faucet_site(pool, id, admin_id, &["PENDING", "APPROVED", "SUSPENDED"], "REJECTED", "FAUCETSITE_REJECT", Some(reason)).await
}

pub async fn suspend_faucet_site(pool: &PgPool, id: Uuid, admin_id: Uuid) -> Result<(), AdminError> {
    transition_faucet_site(pool, id, admin_id, &["APPROVED"], "SUSPENDED", "FAUCETSITE_SUSPEND", None).await
}
