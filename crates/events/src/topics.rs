//! Centralized Kafka topic names — one per bounded context, not per event
//! type (see `docs/decisions/kafka-topic-naming-and-partitioning.md`).

pub const WALLET_EVENTS: &str = "bitcosats.wallet.events";
pub const SWAP_EVENTS: &str = "bitcosats.swap.events";
pub const LEDGER_EVENTS: &str = "bitcosats.ledger.events";
pub const FAUCET_EVENTS: &str = "bitcosats.faucet.events";
pub const STAKE_EVENTS: &str = "bitcosats.stake.events";
pub const LEND_EVENTS: &str = "bitcosats.lend.events";
pub const REWARDS_EVENTS: &str = "bitcosats.rewards.events";
/// Audit trail — longer retention than the operational topics above.
pub const ADMIN_EVENTS: &str = "bitcosats.admin.events";

use crate::domain::DomainEvent;

/// Maps an event to the topic it belongs to, mirroring `DomainEvent::aggregate_type`.
pub fn topic_for(event: &DomainEvent) -> &'static str {
    match event.aggregate_type() {
        "wallet" => WALLET_EVENTS,
        "swap" => SWAP_EVENTS,
        "ledger" => LEDGER_EVENTS,
        "faucet" => FAUCET_EVENTS,
        "stake" => STAKE_EVENTS,
        "lend" => LEND_EVENTS,
        "rewards" => REWARDS_EVENTS,
        "admin" => ADMIN_EVENTS,
        other => unreachable!("unmapped aggregate_type: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AdminActionPerformed, DepositConfirmed, DomainEvent, FaucetClaimed, LedgerEntryCreated,
        LendPositionOpened, RewardGranted, StakeCreated, SwapExecuted,
    };
    use uuid::Uuid;

    fn id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn topic_for_covers_all_aggregates() {
        let cases: Vec<(DomainEvent, &str)> = vec![
            (
                DomainEvent::DepositConfirmed(DepositConfirmed {
                    wallet_id: id(),
                    deposit_id: id(),
                    coin: "BTC".into(),
                    amount: "1".into(),
                    tx_hash: "x".into(),
                    confirmed_at: chrono::Utc::now(),
                }),
                WALLET_EVENTS,
            ),
            (
                DomainEvent::SwapExecuted(SwapExecuted {
                    swap_id: id(),
                    wallet_id: id(),
                    from_coin: "BTC".into(),
                    to_coin: "LTC".into(),
                    from_amount: "1".into(),
                    to_amount: "1".into(),
                    fee_amount: "0".into(),
                }),
                SWAP_EVENTS,
            ),
            (
                DomainEvent::LedgerEntryCreated(LedgerEntryCreated {
                    wallet_id: id(),
                    entry_id: id(),
                    ledger_type: "CREDIT".into(),
                    amount: "1".into(),
                    reference_id: id(),
                    reference_type: "t".into(),
                }),
                LEDGER_EVENTS,
            ),
            (
                DomainEvent::FaucetClaimed(FaucetClaimed {
                    wallet_id: id(),
                    coin: "BTC".into(),
                    amount: "1".into(),
                }),
                FAUCET_EVENTS,
            ),
            (
                DomainEvent::StakeCreated(StakeCreated {
                    stake_id: id(),
                    wallet_id: id(),
                    coin: "BTC".into(),
                    principal: "1".into(),
                    lock_days: 30,
                }),
                STAKE_EVENTS,
            ),
            (
                DomainEvent::LendPositionOpened(LendPositionOpened {
                    position_id: id(),
                    wallet_id: id(),
                    coin: "USDT".into(),
                    kind: "supply".into(),
                    amount: "1".into(),
                }),
                LEND_EVENTS,
            ),
            (
                DomainEvent::RewardGranted(RewardGranted {
                    wallet_id: id(),
                    coin: "BTC".into(),
                    amount: "1".into(),
                    reason: "x".into(),
                }),
                REWARDS_EVENTS,
            ),
            (
                DomainEvent::AdminActionPerformed(AdminActionPerformed {
                    admin_id: id(),
                    action: "approve".into(),
                    target_id: None,
                    details: serde_json::json!({}),
                }),
                ADMIN_EVENTS,
            ),
        ];
        for (event, topic) in cases {
            assert_eq!(topic_for(&event), topic);
            assert!(!event.aggregate_id().is_nil());
        }
    }
}
