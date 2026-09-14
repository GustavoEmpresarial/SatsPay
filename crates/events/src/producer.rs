//! Thin wrapper over `rdkafka::producer::FutureProducer`, configured for
//! idempotent delivery (see `docs/decisions/kafka-client-rdkafka.md`).

use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ProducerError {
    #[error("kafka producer error: {0}")]
    Kafka(#[from] rdkafka::error::KafkaError),
}

pub struct EventProducer {
    inner: FutureProducer,
}

impl EventProducer {
    pub fn new(bootstrap_servers: &str) -> Result<Self, ProducerError> {
        let inner = ClientConfig::new()
            .set("bootstrap.servers", bootstrap_servers)
            .set("enable.idempotence", "true")
            .set("acks", "all")
            .set("message.timeout.ms", "30000")
            .create()?;
        Ok(Self { inner })
    }

    /// Publishes `payload` to `topic`, keyed by `key` (the aggregate id) so
    /// events for the same wallet/swap/etc land in the same partition and
    /// stay strictly ordered.
    pub async fn publish(&self, topic: &str, key: &str, payload: &[u8]) -> Result<(), ProducerError> {
        let record = FutureRecord::to(topic).key(key).payload(payload);
        self.inner
            .send(record, Timeout::After(Duration::from_secs(10)))
            .await
            .map_err(|(err, _msg)| err)?;
        Ok(())
    }
}
