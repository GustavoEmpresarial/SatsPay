//! Transactional outbox writer — see `docs/decisions/transactional-outbox-pattern.md`.
//!
//! Every domain module that emits an event calls `OutboxWriter::stage`
//! *inside the same `sqlx::Transaction`* used to write `ledger_entries` /
//! update withdrawal state, so the event insert commits atomically with the
//! financial state change it describes. Publishing to Kafka happens later,
//! out-of-band, via a separate relay process (`worker::outbox_relay`) that
//! polls `published_at IS NULL` rows.

use crate::domain::DomainEvent;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error("failed to stage outbox event: {0}")]
    Db(#[from] sqlx::Error),
    #[error("failed to serialize event payload: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub struct OutboxWriter;

impl OutboxWriter {
    /// Inserts one pending `outbox_events` row within `tx`. Caller is
    /// responsible for committing `tx` — the insert only becomes durable (and
    /// eligible for the relay to pick up) once the surrounding transaction
    /// commits, which is exactly what gives us the atomicity guarantee.
    pub async fn stage(tx: &mut Transaction<'_, Postgres>, event: &DomainEvent) -> Result<Uuid, OutboxError> {
        let id = Uuid::new_v4();
        let event_type = event_type_name(event);
        let payload = serde_json::to_value(event)?;

        sqlx::query(
            r#"
            INSERT INTO outbox_events (id, aggregate_type, aggregate_id, event_type, payload, created_at)
            VALUES ($1, $2, $3, $4, $5, now())
            "#,
        )
        .bind(id)
        .bind(event.aggregate_type())
        .bind(event.aggregate_id())
        .bind(event_type)
        .bind(payload)
        .execute(&mut **tx)
        .await?;

        Ok(id)
    }
}

fn event_type_name(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::DepositConfirmed(_) => "DepositConfirmed",
        DomainEvent::WithdrawalBroadcasted(_) => "WithdrawalBroadcasted",
        DomainEvent::WithdrawalConfirmed(_) => "WithdrawalConfirmed",
        DomainEvent::WithdrawalFailed(_) => "WithdrawalFailed",
        DomainEvent::SwapExecuted(_) => "SwapExecuted",
        DomainEvent::LedgerEntryCreated(_) => "LedgerEntryCreated",
        DomainEvent::FaucetClaimed(_) => "FaucetClaimed",
        DomainEvent::StakeCreated(_) => "StakeCreated",
        DomainEvent::StakeClosed(_) => "StakeClosed",
        DomainEvent::LendPositionOpened(_) => "LendPositionOpened",
        DomainEvent::LendPositionClosed(_) => "LendPositionClosed",
        DomainEvent::RewardGranted(_) => "RewardGranted",
        DomainEvent::AdminActionPerformed(_) => "AdminActionPerformed",
    }
}
