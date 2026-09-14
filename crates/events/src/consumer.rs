//! Thin wrapper over `rdkafka::consumer::StreamConsumer` plus an `event_id`
//! dedup helper, for future internal consumers (e.g. an audit/notifications
//! service also written in Rust). External consumers (analytics, fraud
//! detection) are expected to implement their own client against these
//! topics; this module is only for consumers living in this workspace.

use crate::domain::DomainEvent;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ConsumerError {
    #[error("kafka consumer error: {0}")]
    Kafka(#[from] rdkafka::error::KafkaError),
    #[error("malformed event envelope: {0}")]
    Deserialize(#[from] serde_json::Error),
}

pub struct DecodedEnvelope {
    pub event_id: Uuid,
    pub event: DomainEvent,
}

pub fn build_consumer(bootstrap_servers: &str, group_id: &str, topics: &[&str]) -> Result<StreamConsumer, ConsumerError> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap_servers)
        .set("group.id", group_id)
        .set("enable.auto.commit", "false")
        .create()?;
    consumer.subscribe(topics)?;
    Ok(consumer)
}

/// Parses the `{event_id, event}` envelope produced by `relay::relay_once`.
/// Callers should dedupe on `event_id` (e.g. `INSERT ... ON CONFLICT DO NOTHING`
/// into a `processed_event_ids` table) before applying the event's effect,
/// since delivery is at-least-once.
pub fn decode_envelope(payload: &[u8]) -> Result<DecodedEnvelope, ConsumerError> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        event_id: Uuid,
        event: serde_json::Value,
    }
    let envelope: Envelope = serde_json::from_slice(payload)?;
    let event: DomainEvent = serde_json::from_value(envelope.event)?;
    Ok(DecodedEnvelope { event_id: envelope.event_id, event })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DepositConfirmed, DomainEvent};
    use uuid::Uuid;

    #[test]
    fn decode_envelope_round_trip() {
        let event_id = Uuid::new_v4();
        let wallet_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "event_id": event_id,
            "event": {
                "event_type": "DepositConfirmed",
                "wallet_id": wallet_id,
                "deposit_id": Uuid::new_v4(),
                "coin": "BTC",
                "amount": "1",
                "tx_hash": "abc",
                "confirmed_at": "2024-01-01T00:00:00Z"
            }
        });
        let decoded = decode_envelope(payload.to_string().as_bytes()).unwrap();
        assert_eq!(decoded.event_id, event_id);
        assert_eq!(decoded.event.aggregate_type(), "wallet");
        assert_eq!(decoded.event.aggregate_id(), wallet_id);
        assert!(matches!(decoded.event, DomainEvent::DepositConfirmed(DepositConfirmed { .. })));
    }

    #[test]
    fn decode_envelope_rejects_garbage() {
        assert!(decode_envelope(b"{not-json").is_err());
        assert!(decode_envelope(br#"{"event_id":"x","event":{}}"#).is_err());
    }
}
