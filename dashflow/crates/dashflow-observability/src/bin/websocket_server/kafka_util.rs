//! Kafka utility types for the WebSocket server.
//!
//! This module provides helpers for working with Kafka messages, particularly
//! for extracting trace context from message headers.

use opentelemetry::propagation::Extractor;
use rdkafka::message::Headers;
use rdkafka::Message; // Trait for .headers() method

/// Issue #14 / M-490: Helper struct for extracting trace context from Kafka headers.
/// Uses lazy extraction (no upfront HashMap allocation) to reduce allocation pressure
/// at high throughput. Headers are looked up on-demand from the Kafka message.
pub struct KafkaHeaderExtractor<'a> {
    msg: &'a rdkafka::message::BorrowedMessage<'a>,
}

impl<'a> KafkaHeaderExtractor<'a> {
    pub fn from_kafka_message(msg: &'a rdkafka::message::BorrowedMessage<'a>) -> Self {
        Self { msg }
    }
}

impl<'a> Extractor for KafkaHeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        let headers = self.msg.headers()?;
        for i in 0..headers.count() {
            let header = headers.get(i);
            if header.key == key {
                return header.value.and_then(|v| std::str::from_utf8(v).ok());
            }
        }
        None
    }

    fn keys(&self) -> Vec<&str> {
        let Some(headers) = self.msg.headers() else {
            return Vec::new();
        };
        (0..headers.count()).map(|i| headers.get(i).key).collect()
    }
}
