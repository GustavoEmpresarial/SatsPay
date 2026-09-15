//! Multi-coin invoices: pricing in USD, and letting the paying customer pick
//! which coin to settle in.
//!
//! The invoice always holds a *current selection* (`coin`/`amount`/
//! `deposit_address`); this module changes which coin that is, and records
//! every address the invoice has ever shown so an abandoned one is still
//! honoured if money lands on it.

use bigdecimal::{BigDecimal, One, ToPrimitive};
use chrono::{DateTime, Utc};
use shared::Coin;
use sqlx::{PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

use crate::merchant_deposits::{MerchantDepositError, MerchantDepositInvoice};

#[derive(Debug, thiserror::Error)]
pub enum MultiCoinError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Invoice(#[from] MerchantDepositError),
    #[error(transparent)]
    Pricing(#[from] crate::pricing::PricingDbError),
    #[error("invoice not found")]
    NotFound,
    #[error("coin is not accepted for this invoice")]
    CoinNotAccepted,
    #[error("the coin is already locked for this invoice")]
    CoinLocked,
    #[error("invoice is no longer payable")]
    NotPayable,
    #[error("amount is too small to be paid in {0}")]
    AmountTooSmall(&'static str),
}

/// One coin a customer may choose, with the amount that coin would cost.
#[derive(Debug, Clone)]
pub struct CoinOption {
    pub coin: Coin,
    /// Ledger units (1e-8) of `coin`.
    pub amount: BigDecimal,
    /// Coin price used, scaled by `price_decimals`.
    pub price_scaled: BigDecimal,
}

/// Converts a USD amount into ledger units of `coin`, **rounding up**.
///
/// Truncating would hand the merchant less than they asked for on almost
/// every conversion, so the last unit always goes to the merchant. Both
/// values are scaled by the same `price_decimals`, so the scale cancels and
/// the result is in ledger units (1e-8), matching `amount` everywhere else.
pub fn usd_to_ledger_units(price_usd_scaled: &BigDecimal, coin_price_scaled: &BigDecimal) -> Option<BigDecimal> {
    if coin_price_scaled <= &BigDecimal::from(0) {
        return None;
    }
    let units_per_coin = BigDecimal::from(10u64.pow(shared::coin_config(Coin::Btc).decimals));
    let exact = (price_usd_scaled * units_per_coin) / coin_price_scaled;
    Some(ceil_to_integer(&exact))
}

/// Smallest integer >= `value`. The ledger holds integers only, and rounding
/// down here is the merchant silently eating the remainder.
fn ceil_to_integer(value: &BigDecimal) -> BigDecimal {
    let truncated = value.with_scale(0);
    if &truncated < value {
        truncated + BigDecimal::one()
    } else {
        truncated
    }
}

/// Prices `price_usd_scaled` in each coin the invoice accepts, skipping any
/// coin without a fresh price rather than quoting a stale one.
pub async fn price_options(
    pool: &PgPool,
    coins: &[Coin],
    price_usd_scaled: &BigDecimal,
    max_stale: Duration,
) -> Vec<CoinOption> {
    let mut out = Vec::with_capacity(coins.len());
    for coin in coins {
        // Fail-closed per coin: a coin whose price is missing or stale is not
        // offered, instead of being offered at an invented rate.
        let Ok((price, _decimals)) = crate::pricing::get_price(pool, *coin, max_stale).await else {
            tracing::warn!(coin = coin.as_str(), "multi-coin: no fresh price, coin not offered");
            continue;
        };
        let price_scaled = BigDecimal::from(
            price.to_u128().and_then(|p| u64::try_from(p).ok()).unwrap_or(0),
        );
        let Some(amount) = usd_to_ledger_units(price_usd_scaled, &price_scaled) else { continue };
        if amount <= BigDecimal::from(0) {
            continue;
        }
        // An amount that rounds to zero on-chain can never actually be paid.
        if amount.to_u128().map(|u| shared::to_onchain_amount(*coin, u) == 0).unwrap_or(true) {
            continue;
        }
        out.push(CoinOption { coin: *coin, amount, price_scaled });
    }
    out
}

/// The address (and locked amount) an invoice shows for `coin`, creating it
/// on first use.
///
/// Idempotent per `(invoice_id, coin)`: a refresh, a double click, or a
/// customer switching back and forth reuses the stored row instead of burning
/// another HD index and re-quoting the price.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_invoice_address(
    pool: &PgPool,
    invoice_id: Uuid,
    coin: Coin,
    address: &str,
    hd_index: Option<i64>,
    amount: &BigDecimal,
    quote_price_scaled: Option<&BigDecimal>,
) -> Result<InvoiceAddress, MultiCoinError> {
    let row = sqlx::query(
        "INSERT INTO merchant_invoice_addresses (invoice_id, coin, address, hd_index, amount, quote_price_scaled) \
         VALUES ($1, $2::coin, $3, $4, $5, $6) \
         ON CONFLICT (invoice_id, coin) DO UPDATE SET invoice_id = EXCLUDED.invoice_id \
         RETURNING id, invoice_id, coin::text AS coin, address, hd_index, amount, quote_price_scaled, created_at",
    )
    .bind(invoice_id)
    .bind(coin.as_str())
    .bind(address)
    .bind(hd_index)
    .bind(amount)
    .bind(quote_price_scaled)
    .fetch_one(pool)
    .await?;

    Ok(row_to_address(&row))
}

/// Existing address row for `(invoice, coin)`, if the invoice ever showed it.
pub async fn find_invoice_address(
    pool: &PgPool,
    invoice_id: Uuid,
    coin: Coin,
) -> Result<Option<InvoiceAddress>, MultiCoinError> {
    let row = sqlx::query(
        "SELECT id, invoice_id, coin::text AS coin, address, hd_index, amount, quote_price_scaled, created_at \
         FROM merchant_invoice_addresses WHERE invoice_id = $1 AND coin = $2::coin",
    )
    .bind(invoice_id)
    .bind(coin.as_str())
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| row_to_address(&r)))
}

/// Every address an invoice has ever shown. The watcher scans all of them:
/// a customer who was shown one coin and switched may already have paid the
/// first address.
pub async fn list_invoice_addresses(
    pool: &PgPool,
    invoice_id: Uuid,
) -> Result<Vec<InvoiceAddress>, MultiCoinError> {
    let rows = sqlx::query(
        "SELECT id, invoice_id, coin::text AS coin, address, hd_index, amount, quote_price_scaled, created_at \
         FROM merchant_invoice_addresses WHERE invoice_id = $1 ORDER BY created_at ASC",
    )
    .bind(invoice_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(row_to_address).collect())
}

/// Points the invoice's current selection at `coin`. Refuses once the coin is
/// locked, so a customer cannot switch out from under a payment in flight.
pub async fn select_coin(
    pool: &PgPool,
    invoice_id: Uuid,
    coin: Coin,
    addr: &InvoiceAddress,
    lock: bool,
) -> Result<MerchantDepositInvoice, MultiCoinError> {
    let (fee, net) = crate::merchant_deposits::split_fee(&addr.amount);

    let updated = sqlx::query(
        "UPDATE merchant_deposit_invoices \
         SET coin = $2::coin, amount = $3, fee_amount = $4, net_amount = $5, \
             deposit_address = $6, hd_index = $7, quote_price_scaled = $8, \
             coin_locked_at = CASE WHEN $9 THEN now() ELSE coin_locked_at END, \
             updated_at = now() \
         WHERE id = $1 \
           AND coin_locked_at IS NULL \
           AND status = 'PENDING'::deposit_invoice_status \
           AND expires_at > now()",
    )
    .bind(invoice_id)
    .bind(coin.as_str())
    .bind(&addr.amount)
    .bind(&fee)
    .bind(&net)
    .bind(&addr.address)
    .bind(addr.hd_index)
    .bind(addr.quote_price_scaled.as_ref())
    .bind(lock)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 0 {
        // Either the coin is already locked or the invoice is not payable —
        // distinguish so the caller can answer 409 vs 400.
        let inv = crate::merchant_deposits::get_invoice_by_id(pool, invoice_id).await?;
        return Err(if inv.coin_locked_at.is_some() { MultiCoinError::CoinLocked } else { MultiCoinError::NotPayable });
    }

    Ok(crate::merchant_deposits::get_invoice_by_id(pool, invoice_id).await?)
}

/// Freezes the selection. Called the moment any address of the invoice shows
/// a payment: after that, switching coins would strand real money.
pub async fn lock_coin(pool: &PgPool, invoice_id: Uuid) -> Result<(), MultiCoinError> {
    sqlx::query("UPDATE merchant_deposit_invoices SET coin_locked_at = now(), updated_at = now() WHERE id = $1 AND coin_locked_at IS NULL")
        .bind(invoice_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct InvoiceAddress {
    pub id: Uuid,
    pub invoice_id: Uuid,
    pub coin: String,
    pub address: String,
    pub hd_index: Option<i64>,
    pub amount: BigDecimal,
    pub quote_price_scaled: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
}

fn row_to_address(row: &sqlx::postgres::PgRow) -> InvoiceAddress {
    InvoiceAddress {
        id: row.get("id"),
        invoice_id: row.get("invoice_id"),
        coin: row.get("coin"),
        address: row.get("address"),
        hd_index: row.get("hd_index"),
        amount: row.get("amount"),
        quote_price_scaled: row.get("quote_price_scaled"),
        created_at: row.get("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Price scale cancels out, so the result is ledger units of the coin.
    #[test]
    fn converts_usd_into_ledger_units() {
        let price_usd = BigDecimal::from(2_500_000_000u64); // US$ 25 at 8 decimals
        let usdt = BigDecimal::from(100_000_000u64); // US$ 1.00
        assert_eq!(usd_to_ledger_units(&price_usd, &usdt).unwrap(), BigDecimal::from(2_500_000_000u64));

        let pol = BigDecimal::from(45_000_000u64); // US$ 0.45
        // 25 / 0.45 = 55.555… POL → 5555555556 units, rounded up.
        assert_eq!(usd_to_ledger_units(&price_usd, &pol).unwrap(), BigDecimal::from(5_555_555_556u64));
    }

    /// Rounding down would make the merchant eat the remainder on nearly
    /// every conversion.
    #[test]
    fn rounding_always_favours_the_merchant() {
        let price_usd = BigDecimal::from(100_000_000u64); // US$ 1
        let odd = BigDecimal::from(30_000_000u64); // US$ 0.30 → 3.333… coins
        let units = usd_to_ledger_units(&price_usd, &odd).unwrap();
        assert_eq!(units, BigDecimal::from(333_333_334u64));
        assert!(units > BigDecimal::from_str("333333333.33").unwrap());
    }

    #[test]
    fn an_exact_conversion_is_not_bumped_up() {
        let price_usd = BigDecimal::from(200_000_000u64); // US$ 2
        let coin = BigDecimal::from(100_000_000u64); // US$ 1 → exactly 2 coins
        assert_eq!(usd_to_ledger_units(&price_usd, &coin).unwrap(), BigDecimal::from(200_000_000u64));
    }

    #[test]
    fn a_zero_or_negative_price_never_quotes() {
        let price_usd = BigDecimal::from(100_000_000u64);
        assert!(usd_to_ledger_units(&price_usd, &BigDecimal::from(0)).is_none());
        assert!(usd_to_ledger_units(&price_usd, &BigDecimal::from(-1)).is_none());
    }

    #[test]
    fn ceil_handles_integers_and_fractions() {
        assert_eq!(ceil_to_integer(&BigDecimal::from_str("5").unwrap()), BigDecimal::from(5));
        assert_eq!(ceil_to_integer(&BigDecimal::from_str("5.0001").unwrap()), BigDecimal::from(6));
        assert_eq!(ceil_to_integer(&BigDecimal::from_str("0.1").unwrap()), BigDecimal::from(1));
    }
}

/// Repoints a locked invoice at the coin that actually received money.
///
/// Used when a customer pays an address they were shown and then switched
/// away from: the ledger credit, the webhook and the merchant's statement
/// must all name the coin the money arrived in, not the last one clicked.
pub async fn point_selection_at(
    pool: &PgPool,
    invoice_id: Uuid,
    coin: Coin,
    addr: &InvoiceAddress,
) -> Result<(), MultiCoinError> {
    let (fee, net) = crate::merchant_deposits::split_fee(&addr.amount);

    sqlx::query(
        "UPDATE merchant_deposit_invoices \
         SET coin = $2::coin, amount = $3, fee_amount = $4, net_amount = $5, \
             deposit_address = $6, hd_index = $7, quote_price_scaled = $8, updated_at = now() \
         WHERE id = $1 AND status <> 'CONFIRMED'::deposit_invoice_status",
    )
    .bind(invoice_id)
    .bind(coin.as_str())
    .bind(&addr.amount)
    .bind(&fee)
    .bind(&net)
    .bind(&addr.address)
    .bind(addr.hd_index)
    .bind(addr.quote_price_scaled.as_ref())
    .execute(pool)
    .await?;

    Ok(())
}
