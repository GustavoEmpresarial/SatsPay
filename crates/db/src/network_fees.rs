//! Persist on-chain network fees paid by the hot wallet (telemetry, not ledger).

use bigdecimal::BigDecimal;
use shared::Coin;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkFeeKind {
    Withdrawal,
    Sweep,
    DexDeposit,
    GasTopup,
}

impl NetworkFeeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Withdrawal => "WITHDRAWAL",
            Self::Sweep => "SWEEP",
            Self::DexDeposit => "DEX_DEPOSIT",
            Self::GasTopup => "GAS_TOPUP",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordNetworkFeeInput<'a> {
    pub coin: Coin,
    pub kind: NetworkFeeKind,
    pub amount: u128,
    pub tx_hash: Option<&'a str>,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<&'a str>,
}

/// Insert a network-fee event. Idempotent on `(coin, tx_hash)` when hash is set.
/// Zero amounts are skipped (nothing paid).
pub async fn record_network_fee(pool: &PgPool, input: RecordNetworkFeeInput<'_>) -> Result<bool, sqlx::Error> {
    if input.amount == 0 {
        return Ok(false);
    }
    let amount = BigDecimal::from(input.amount);
    let result = sqlx::query(
        "INSERT INTO network_fee_events (coin, kind, amount, tx_hash, reference_id, reference_type) \
         VALUES ($1::coin, $2, $3, $4, $5, $6) \
         ON CONFLICT (coin, tx_hash) WHERE tx_hash IS NOT NULL DO NOTHING",
    )
    .bind(input.coin.as_str())
    .bind(input.kind.as_str())
    .bind(amount)
    .bind(input.tx_hash)
    .bind(input.reference_id)
    .bind(input.reference_type)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
