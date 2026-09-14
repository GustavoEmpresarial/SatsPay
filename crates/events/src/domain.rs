//! Domain event payloads published to Kafka via the transactional outbox.
//! One variant per fact defined in `docs/decisions/domain-events-vs-internal-jobs.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DomainEvent {
    DepositConfirmed(DepositConfirmed),
    WithdrawalBroadcasted(WithdrawalBroadcasted),
    WithdrawalConfirmed(WithdrawalConfirmed),
    WithdrawalFailed(WithdrawalFailed),
    SwapExecuted(SwapExecuted),
    LedgerEntryCreated(LedgerEntryCreated),
    FaucetClaimed(FaucetClaimed),
    StakeCreated(StakeCreated),
    StakeClosed(StakeClosed),
    LendPositionOpened(LendPositionOpened),
    LendPositionClosed(LendPositionClosed),
    RewardGranted(RewardGranted),
    AdminActionPerformed(AdminActionPerformed),
}

impl DomainEvent {
    /// The outbox `aggregate_type`/topic-selection key for this event.
    pub fn aggregate_type(&self) -> &'static str {
        match self {
            DomainEvent::DepositConfirmed(_)
            | DomainEvent::WithdrawalBroadcasted(_)
            | DomainEvent::WithdrawalConfirmed(_)
            | DomainEvent::WithdrawalFailed(_) => "wallet",
            DomainEvent::SwapExecuted(_) => "swap",
            DomainEvent::LedgerEntryCreated(_) => "ledger",
            DomainEvent::FaucetClaimed(_) => "faucet",
            DomainEvent::StakeCreated(_) | DomainEvent::StakeClosed(_) => "stake",
            DomainEvent::LendPositionOpened(_) | DomainEvent::LendPositionClosed(_) => "lend",
            DomainEvent::RewardGranted(_) => "rewards",
            DomainEvent::AdminActionPerformed(_) => "admin",
        }
    }

    /// The wallet/swap/etc id used as the Kafka partition key, so events for
    /// the same entity are strictly ordered.
    pub fn aggregate_id(&self) -> Uuid {
        match self {
            DomainEvent::DepositConfirmed(e) => e.wallet_id,
            DomainEvent::WithdrawalBroadcasted(e) => e.wallet_id,
            DomainEvent::WithdrawalConfirmed(e) => e.wallet_id,
            DomainEvent::WithdrawalFailed(e) => e.wallet_id,
            DomainEvent::SwapExecuted(e) => e.swap_id,
            DomainEvent::LedgerEntryCreated(e) => e.wallet_id,
            DomainEvent::FaucetClaimed(e) => e.wallet_id,
            DomainEvent::StakeCreated(e) => e.stake_id,
            DomainEvent::StakeClosed(e) => e.stake_id,
            DomainEvent::LendPositionOpened(e) => e.position_id,
            DomainEvent::LendPositionClosed(e) => e.position_id,
            DomainEvent::RewardGranted(e) => e.wallet_id,
            DomainEvent::AdminActionPerformed(e) => e.admin_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositConfirmed {
    pub wallet_id: Uuid,
    pub deposit_id: Uuid,
    pub coin: String,
    pub amount: String, // u128 as decimal string (JSON has no 128-bit int)
    pub tx_hash: String,
    pub confirmed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalBroadcasted {
    pub wallet_id: Uuid,
    pub withdrawal_id: Uuid,
    pub coin: String,
    pub amount: String,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalConfirmed {
    pub wallet_id: Uuid,
    pub withdrawal_id: Uuid,
    pub confirmations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalFailed {
    pub wallet_id: Uuid,
    pub withdrawal_id: Uuid,
    pub reason: String,
    pub safe_to_reverse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapExecuted {
    pub swap_id: Uuid,
    pub wallet_id: Uuid,
    pub from_coin: String,
    pub to_coin: String,
    pub from_amount: String,
    pub to_amount: String,
    pub fee_amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntryCreated {
    pub wallet_id: Uuid,
    pub entry_id: Uuid,
    pub ledger_type: String,
    pub amount: String, // signed decimal string (can be negative)
    pub reference_id: Uuid,
    pub reference_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaucetClaimed {
    pub wallet_id: Uuid,
    pub coin: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeCreated {
    pub stake_id: Uuid,
    pub wallet_id: Uuid,
    pub coin: String,
    pub principal: String,
    pub lock_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeClosed {
    pub stake_id: Uuid,
    pub wallet_id: Uuid,
    pub reward_paid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LendPositionOpened {
    pub position_id: Uuid,
    pub wallet_id: Uuid,
    pub coin: String,
    pub kind: String, // "supply" | "borrow"
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LendPositionClosed {
    pub position_id: Uuid,
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardGranted {
    pub wallet_id: Uuid,
    pub coin: String,
    pub amount: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminActionPerformed {
    pub admin_id: Uuid,
    pub action: String,
    pub target_id: Option<Uuid>,
    pub details: serde_json::Value,
}
