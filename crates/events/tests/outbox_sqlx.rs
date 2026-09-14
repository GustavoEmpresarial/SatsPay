use events::domain::*;
use events::{DomainEvent, OutboxWriter};
use sqlx::PgPool;
use uuid::Uuid;

fn id() -> Uuid {
    Uuid::new_v4()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn stage_every_domain_event_variant(pool: PgPool) {
    let events = vec![
        DomainEvent::DepositConfirmed(DepositConfirmed {
            wallet_id: id(),
            deposit_id: id(),
            coin: "BTC".into(),
            amount: "1".into(),
            tx_hash: "t".into(),
            confirmed_at: chrono::Utc::now(),
        }),
        DomainEvent::WithdrawalBroadcasted(WithdrawalBroadcasted {
            wallet_id: id(),
            withdrawal_id: id(),
            coin: "BTC".into(),
            amount: "1".into(),
            tx_hash: "t".into(),
        }),
        DomainEvent::WithdrawalConfirmed(WithdrawalConfirmed {
            wallet_id: id(),
            withdrawal_id: id(),
            confirmations: 1,
        }),
        DomainEvent::WithdrawalFailed(WithdrawalFailed {
            wallet_id: id(),
            withdrawal_id: id(),
            reason: "x".into(),
            safe_to_reverse: true,
        }),
        DomainEvent::SwapExecuted(SwapExecuted {
            swap_id: id(),
            wallet_id: id(),
            from_coin: "BTC".into(),
            to_coin: "LTC".into(),
            from_amount: "1".into(),
            to_amount: "1".into(),
            fee_amount: "0".into(),
        }),
        DomainEvent::LedgerEntryCreated(LedgerEntryCreated {
            wallet_id: id(),
            entry_id: id(),
            ledger_type: "CREDIT".into(),
            amount: "1".into(),
            reference_id: id(),
            reference_type: "t".into(),
        }),
        DomainEvent::FaucetClaimed(FaucetClaimed {
            wallet_id: id(),
            coin: "LTC".into(),
            amount: "1".into(),
        }),
        DomainEvent::StakeCreated(StakeCreated {
            stake_id: id(),
            wallet_id: id(),
            coin: "BTC".into(),
            principal: "1".into(),
            lock_days: 0,
        }),
        DomainEvent::StakeClosed(StakeClosed {
            stake_id: id(),
            wallet_id: id(),
            reward_paid: "1".into(),
        }),
        DomainEvent::LendPositionOpened(LendPositionOpened {
            position_id: id(),
            wallet_id: id(),
            coin: "BTC".into(),
            kind: "supply".into(),
            amount: "1".into(),
        }),
        DomainEvent::LendPositionClosed(LendPositionClosed {
            position_id: id(),
            wallet_id: id(),
        }),
        DomainEvent::RewardGranted(RewardGranted {
            wallet_id: id(),
            coin: "LTC".into(),
            amount: "1".into(),
            reason: "r".into(),
        }),
        DomainEvent::AdminActionPerformed(AdminActionPerformed {
            admin_id: id(),
            action: "X".into(),
            target_id: Some(id()),
            details: serde_json::json!({}),
        }),
    ];

    let mut tx = pool.begin().await.unwrap();
    for event in &events {
        let _ = event.aggregate_type();
        let _ = event.aggregate_id();
        OutboxWriter::stage(&mut tx, event).await.unwrap();
    }
    tx.commit().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, events.len() as i64);
}
