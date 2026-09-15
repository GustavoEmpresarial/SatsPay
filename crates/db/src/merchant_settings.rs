//! Per-merchant gateway configuration: which coins the hosted checkout may
//! offer a paying customer.

use shared::Coin;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum MerchantSettingsError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("no usable coin: every coin given is paused or unknown")]
    NoUsableCoin,
}

/// Coins the checkout may offer for this merchant.
///
/// Stored empty means "whatever is live right now", so a merchant who never
/// opened the setting keeps accepting everything, and a coin leaving the
/// pause list becomes available without anyone editing a row. A paused coin
/// is filtered on read as well as on write: pausing must take effect for
/// merchants who configured the coin before the pause.
pub async fn accepted_coins(pool: &PgPool, merchant_id: Uuid) -> Result<Vec<Coin>, MerchantSettingsError> {
    let row = sqlx::query("SELECT accepted_coins::text[] AS coins FROM merchant_gateway_settings WHERE merchant_id = $1")
        .bind(merchant_id)
        .fetch_optional(pool)
        .await?;

    let configured: Vec<Coin> = row
        .map(|r| {
            let raw: Vec<String> = r.get("coins");
            raw.iter().filter_map(|c| c.parse::<Coin>().ok()).collect()
        })
        .unwrap_or_default();

    Ok(live_coins(&configured))
}

/// Resolves a stored configuration against what the platform currently
/// accepts. Pure, so the "empty means all" and "paused is never offered"
/// rules are testable without a database.
pub fn live_coins(configured: &[Coin]) -> Vec<Coin> {
    let enabled: Vec<Coin> = shared::COINS
        .into_iter()
        .filter(|c| !shared::is_deposit_withdraw_paused(*c))
        .collect();

    if configured.is_empty() {
        return enabled;
    }
    enabled.into_iter().filter(|c| configured.contains(c)).collect()
}

/// Replaces the merchant's list. Rejects a selection that would leave the
/// checkout with nothing to offer — an invoice nobody can pay is worse than
/// a rejected setting.
pub async fn set_accepted_coins(
    pool: &PgPool,
    merchant_id: Uuid,
    coins: &[Coin],
) -> Result<Vec<Coin>, MerchantSettingsError> {
    if !coins.is_empty() && live_coins(coins).is_empty() {
        return Err(MerchantSettingsError::NoUsableCoin);
    }
    let as_text: Vec<String> = coins.iter().map(|c| c.as_str().to_string()).collect();

    sqlx::query(
        "INSERT INTO merchant_gateway_settings (merchant_id, accepted_coins) \
         VALUES ($1, $2::text[]::coin[]) \
         ON CONFLICT (merchant_id) DO UPDATE \
         SET accepted_coins = EXCLUDED.accepted_coins, updated_at = now()",
    )
    .bind(merchant_id)
    .bind(&as_text)
    .execute(pool)
    .await?;

    accepted_coins(pool, merchant_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_configuration_means_every_live_coin() {
        let live = live_coins(&[]);
        assert!(!live.is_empty());
        assert!(live.iter().all(|c| !shared::is_deposit_withdraw_paused(*c)));
        assert!(live.contains(&Coin::Usdt));
        // A merchant who never configured anything must not be offering a
        // coin the gateway refuses to create an invoice for.
        assert!(!live.contains(&Coin::Btc), "BTC deposits are paused");
    }

    #[test]
    fn a_paused_coin_is_dropped_even_if_configured() {
        // Configured before the pause; the pause has to win on read.
        let live = live_coins(&[Coin::Btc, Coin::Usdt]);
        assert_eq!(live, vec![Coin::Usdt]);
    }

    #[test]
    fn configuration_narrows_to_what_was_chosen() {
        let live = live_coins(&[Coin::Usdt, Coin::Pol]);
        assert_eq!(live.len(), 2);
        assert!(live.contains(&Coin::Usdt) && live.contains(&Coin::Pol));
        assert!(!live.contains(&Coin::Usdc));
    }

    #[test]
    fn an_all_paused_selection_leaves_nothing_payable() {
        assert!(live_coins(&[Coin::Btc, Coin::Ltc, Coin::Doge, Coin::Dgb]).is_empty());
    }
}
