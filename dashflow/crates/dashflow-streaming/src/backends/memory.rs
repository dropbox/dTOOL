// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Streaming - In-Memory Backend

//! # In-Memory Streaming Backend
//!
//! A simple in-memory backend for testing and development.
//! Messages are stored in memory and lost when the backend is dropped.
//!
//! ## Features
//!
//! - Zero external dependencies (no Kafka, Redis, etc.)
//! - Fast message delivery (direct memory access)
//! - Multiple topics support
//! - Consumer group support with offset tracking
//! - Ideal for unit tests and local development
//!
//! ## Example
//!
//! ```rust
//! use dashflow_streaming::backends::{StreamBackend, InMemoryBackend};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = InMemoryBackend::new();
//!
//!     // Create producer and consumer
//!     let producer = backend.create_producer("events").await?;
//!     let mut consumer = backend.create_consumer("events", "test-group").await?;
//!
//!     // ... send and receive messages
//!     Ok(())
//! }
//! ```

use super::traits::{BackendError, BackendResult, StreamBackend, StreamConsumer, StreamProducer};
use crate::DashStreamMessage;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Stored message with metadata
#[derive(Clone)]
#[allow(dead_code)] // Architectural: Struct fields accessed via Vec<StoredMessage> in TopicData
struct StoredMessage {
    message: DashStreamMessage,
    offset: i64,
    timestamp_us: i64,
}

/// Topic storage
struct TopicData {
    messages: tokio::sync::RwLock<Vec<StoredMessage>>,
    next_offset: AtomicI64,
    sender: broadcast::Sender<()>,
}

impl TopicData {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(DEFAULT_TOPIC_NOTIFICATION_CHANNEL_CAPACITY);
        Self {
            messages: tokio::sync::RwLock::new(Vec::new()),
            next_offset: AtomicI64::new(0),
            sender,
        }
    }
}

/// Consumer group offset tracking
struct ConsumerGroupOffsets {
    offsets: DashMap<String, i64>, // topic -> committed offset
}

impl ConsumerGroupOffsets {
    fn new() -> Self {
        Self {
            offsets: DashMap::new(),
        }
    }
}

/// In-memory streaming backend
///
/// Provides a fully in-memory message queue for testing and development.
/// All data is lost when the backend is dropped.
///
/// To avoid unbounded growth in long-running dev sessions, topics enforce a
/// soft cap on stored messages. The cap is high by default and can be adjusted.
pub const DEFAULT_MAX_MESSAGES_PER_TOPIC: usize = 100_000;

/// Default capacity for the per-topic broadcast notification channel.
///
/// This is used to wake consumers when new messages arrive; the payload is `()`.
pub const DEFAULT_TOPIC_NOTIFICATION_CHANNEL_CAPACITY: usize = 1024;

/// In-memory implementation of the streaming backend.
///
/// Intended for tests and local development where Kafka is not required.
pub struct InMemoryBackend {
    topics: Arc<DashMap<String, Arc<TopicData>>>,
    consumer_groups: Arc<DashMap<String, Arc<ConsumerGroupOffsets>>>,
    closed: AtomicBool,
    message_count: Arc<AtomicU64>,
    max_messages_per_topic: usize,
}

impl InMemoryBackend {
    /// Create a new in-memory backend
    pub fn new() -> Self {
        Self::with_max_messages_per_topic(DEFAULT_MAX_MESSAGES_PER_TOPIC)
    }

    /// Create a new in-memory backend with a custom per-topic cap.
    pub fn with_max_messages_per_topic(max_messages_per_topic: usize) -> Self {
        Self {
            topics: Arc::new(DashMap::new()),
            consumer_groups: Arc::new(DashMap::new()),
            closed: AtomicBool::new(false),
            message_count: Arc::new(AtomicU64::new(0)),
            max_messages_per_topic,
        }
    }

    /// Get or create topic data
    fn get_or_create_topic(&self, topic: &str) -> Arc<TopicData> {
        self.topics
            .entry(topic.to_string())
            .or_insert_with(|| Arc::new(TopicData::new()))
            .clone()
    }

    /// Get or create consumer group
    fn get_or_create_group(&self, group_id: &str) -> Arc<ConsumerGroupOffsets> {
        self.consumer_groups
            .entry(group_id.to_string())
            .or_insert_with(|| Arc::new(ConsumerGroupOffsets::new()))
            .clone()
    }

    /// Get total message count across all topics
    pub fn message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    /// Get message count for a specific topic
    pub async fn topic_message_count(&self, topic: &str) -> usize {
        if let Some(topic_data) = self.topics.get(topic) {
            topic_data.messages.read().await.len()
        } else {
            0
        }
    }

    /// Clear all messages (useful for test reset)
    pub async fn clear(&self) {
        for entry in self.topics.iter() {
            let mut messages = entry.messages.write().await;
            messages.clear();
            entry.next_offset.store(0, Ordering::SeqCst);
        }
        self.message_count.store(0, Ordering::Relaxed);
    }

    /// List all topics
    pub fn list_topics(&self) -> Vec<String> {
        self.topics.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StreamBackend for InMemoryBackend {
    type Producer = InMemoryProducer;
    type Consumer = InMemoryConsumer;

    async fn create_producer(&self, topic: &str) -> BackendResult<Self::Producer> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }

        let topic_data = self.get_or_create_topic(topic);

        Ok(InMemoryProducer {
            topic: topic.to_string(),
            topic_data,
            message_count: Arc::new(AtomicU64::new(0)),
            global_count: Arc::clone(&self.message_count),
            max_messages_per_topic: self.max_messages_per_topic,
        })
    }

    async fn create_consumer(&self, topic: &str, group_id: &str) -> BackendResult<Self::Consumer> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }

        let topic_data = self.get_or_create_topic(topic);
        let group = self.get_or_create_group(group_id);

        // Get starting offset (committed offset or 0)
        // DashMap::get returns Ref wrapper, dereference to get value
        let start_offset = group.offsets.get(topic).map_or(0, |r| *r);

        Ok(InMemoryConsumer {
            topic: topic.to_string(),
            group_id: group_id.to_string(),
            topic_data,
            group_offsets: group,
            current_offset: start_offset,
            receiver: None,
        })
    }

    async fn health_check(&self) -> BackendResult<()> {
        if self.closed.load(Ordering::Relaxed) {
            Err(BackendError::Closed)
        } else {
            Ok(())
        }
    }

    async fn close(&self) -> BackendResult<()> {
        self.closed.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// In-memory producer
pub struct InMemoryProducer {
    topic: String,
    topic_data: Arc<TopicData>,
    message_count: Arc<AtomicU64>,
    global_count: Arc<AtomicU64>,
    max_messages_per_topic: usize,
}

#[async_trait]
impl StreamProducer for InMemoryProducer {
    async fn send(&self, message: DashStreamMessage) -> BackendResult<()> {
        let timestamp_us = chrono::Utc::now().timestamp_micros();

        let mut messages = self.topic_data.messages.write().await;
        if messages.len() >= self.max_messages_per_topic {
            return Err(BackendError::Other(format!(
                "InMemoryBackend topic '{}' exceeded max_messages_per_topic={}",
                self.topic, self.max_messages_per_topic
            )));
        }

        let offset = self.topic_data.next_offset.fetch_add(1, Ordering::SeqCst);
        messages.push(StoredMessage {
            message,
            offset,
            timestamp_us,
        });

        self.message_count.fetch_add(1, Ordering::Relaxed);
        self.global_count.fetch_add(1, Ordering::Relaxed);

        // Notify waiting consumers
        // SAFETY: Broadcast send failure means no consumers are waiting - that's OK.
        let _ = self.topic_data.sender.send(());

        Ok(())
    }

    async fn flush(&self) -> BackendResult<()> {
        // In-memory backend doesn't need flushing
        Ok(())
    }

    fn topic(&self) -> &str {
        &self.topic
    }
}

/// In-memory consumer
pub struct InMemoryConsumer {
    topic: String,
    group_id: String,
    topic_data: Arc<TopicData>,
    group_offsets: Arc<ConsumerGroupOffsets>,
    current_offset: i64,
    receiver: Option<broadcast::Receiver<()>>,
}

#[async_trait]
impl StreamConsumer for InMemoryConsumer {
    async fn next(&mut self) -> Option<BackendResult<DashStreamMessage>> {
        // Check for available message
        loop {
            {
                let messages = self.topic_data.messages.read().await;
                if let Some(stored) = messages.get(self.current_offset as usize) {
                    self.current_offset += 1;
                    return Some(Ok(stored.message.clone()));
                }
            }

            // No message available, subscribe and wait
            if self.receiver.is_none() {
                self.receiver = Some(self.topic_data.sender.subscribe());
            }

            // Wait for notification
            if let Some(ref mut rx) = self.receiver {
                match rx.recv().await {
                    Ok(_) => continue, // Check again
                    Err(broadcast::error::RecvError::Closed) => return None,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        }
    }

    async fn next_timeout(
        &mut self,
        timeout: Duration,
    ) -> Option<BackendResult<DashStreamMessage>> {
        tokio::time::timeout(timeout, self.next())
            .await
            .unwrap_or(None)
    }

    async fn commit(&mut self) -> BackendResult<()> {
        self.group_offsets
            .offsets
            .insert(self.topic.clone(), self.current_offset);
        Ok(())
    }

    fn topic(&self) -> &str {
        &self.topic
    }

    fn group_id(&self) -> &str {
        &self.group_id
    }

    fn current_offset(&self) -> i64 {
        self.current_offset
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, EventType, Header, MessageType};

    fn create_test_event(thread_id: &str, sequence: u64) -> Event {
        Event {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: thread_id.to_string(),
                sequence,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: format!("node-{}", sequence),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        }
    }

    #[tokio::test]
    async fn test_backend_new() {
        let backend = InMemoryBackend::new();
        assert_eq!(backend.message_count(), 0);
        assert!(backend.list_topics().is_empty());
    }

    #[tokio::test]
    async fn test_backend_default() {
        let backend = InMemoryBackend::default();
        assert_eq!(backend.message_count(), 0);
    }

    #[test]
    fn test_default_notification_channel_capacity() {
        assert_eq!(DEFAULT_TOPIC_NOTIFICATION_CHANNEL_CAPACITY, 1024);
    }

    #[tokio::test]
    async fn test_create_producer() {
        let backend = InMemoryBackend::new();
        let producer = backend.create_producer("test-topic").await.unwrap();
        assert_eq!(producer.topic(), "test-topic");
    }

    #[tokio::test]
    async fn test_create_consumer() {
        let backend = InMemoryBackend::new();
        let consumer = backend
            .create_consumer("test-topic", "test-group")
            .await
            .unwrap();
        assert_eq!(consumer.topic(), "test-topic");
        assert_eq!(consumer.group_id(), "test-group");
        assert_eq!(consumer.current_offset(), 0);
    }

    #[tokio::test]
    async fn test_send_and_receive() {
        let backend = InMemoryBackend::new();
        let producer = backend.create_producer("events").await.unwrap();
        let mut consumer = backend
            .create_consumer("events", "test-group")
            .await
            .unwrap();

        // Send event
        let event = create_test_event("thread-1", 1);
        producer.send_event(event.clone()).await.unwrap();

        // Receive event
        let received = consumer
            .next_timeout(Duration::from_millis(100))
            .await
            .unwrap()
            .unwrap();

        match received.message {
            Some(crate::dash_stream_message::Message::Event(e)) => {
                assert_eq!(e.node_id, "node-1");
            }
            _ => panic!("Expected Event message"),
        }

        assert_eq!(consumer.current_offset(), 1);
    }

    #[tokio::test]
    async fn test_multiple_messages() {
        let backend = InMemoryBackend::new();
        let producer = backend.create_producer("events").await.unwrap();
        let mut consumer = backend
            .create_consumer("events", "test-group")
            .await
            .unwrap();

        // Send multiple events
        for i in 1..=5 {
            let event = create_test_event("thread-1", i);
            producer.send_event(event).await.unwrap();
        }

        // Receive all events
        for i in 1..=5 {
            let received = consumer
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap()
                .unwrap();

            match received.message {
                Some(crate::dash_stream_message::Message::Event(e)) => {
                    assert_eq!(e.node_id, format!("node-{}", i));
                }
                _ => panic!("Expected Event message"),
            }
        }

        assert_eq!(consumer.current_offset(), 5);
        assert_eq!(backend.message_count(), 5);
    }

    #[tokio::test]
    async fn test_multiple_consumers_same_group() {
        let backend = InMemoryBackend::new();
        let producer = backend.create_producer("events").await.unwrap();

        // Create first consumer and read some messages
        let mut consumer1 = backend
            .create_consumer("events", "shared-group")
            .await
            .unwrap();

        // Send events
        for i in 1..=3 {
            let event = create_test_event("thread-1", i);
            producer.send_event(event).await.unwrap();
        }

        // Consumer 1 reads and commits
        let _ = consumer1
            .next_timeout(Duration::from_millis(100))
            .await
            .unwrap();
        let _ = consumer1
            .next_timeout(Duration::from_millis(100))
            .await
            .unwrap();
        consumer1.commit().await.unwrap();

        // Create second consumer in same group - should start from committed offset
        let consumer2 = backend
            .create_consumer("events", "shared-group")
            .await
            .unwrap();
        assert_eq!(consumer2.current_offset(), 2);
    }

    #[tokio::test]
    async fn test_multiple_topics() {
        let backend = InMemoryBackend::new();

        let producer1 = backend.create_producer("topic-1").await.unwrap();
        let producer2 = backend.create_producer("topic-2").await.unwrap();

        let mut consumer1 = backend.create_consumer("topic-1", "group").await.unwrap();
        let mut consumer2 = backend.create_consumer("topic-2", "group").await.unwrap();

        // Send to different topics
        producer1
            .send_event(create_test_event("t1", 1))
            .await
            .unwrap();
        producer2
            .send_event(create_test_event("t2", 1))
            .await
            .unwrap();

        // Verify isolation
        let msg1 = consumer1
            .next_timeout(Duration::from_millis(100))
            .await
            .unwrap()
            .unwrap();
        let msg2 = consumer2
            .next_timeout(Duration::from_millis(100))
            .await
            .unwrap()
            .unwrap();

        match msg1.message {
            Some(crate::dash_stream_message::Message::Event(e)) => {
                assert_eq!(e.header.unwrap().thread_id, "t1");
            }
            _ => panic!("Expected Event"),
        }

        match msg2.message {
            Some(crate::dash_stream_message::Message::Event(e)) => {
                assert_eq!(e.header.unwrap().thread_id, "t2");
            }
            _ => panic!("Expected Event"),
        }

        assert!(backend.list_topics().contains(&"topic-1".to_string()));
        assert!(backend.list_topics().contains(&"topic-2".to_string()));
    }

    #[tokio::test]
    async fn test_timeout_no_messages() {
        let backend = InMemoryBackend::new();
        let _ = backend.create_producer("empty-topic").await.unwrap();
        let mut consumer = backend
            .create_consumer("empty-topic", "group")
            .await
            .unwrap();

        let result = consumer.next_timeout(Duration::from_millis(50)).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_health_check() {
        let backend = InMemoryBackend::new();
        assert!(backend.health_check().await.is_ok());

        backend.close().await.unwrap();
        assert!(matches!(
            backend.health_check().await,
            Err(BackendError::Closed)
        ));
    }

    #[tokio::test]
    async fn test_close_backend() {
        let backend = InMemoryBackend::new();
        backend.close().await.unwrap();

        // Creating producer/consumer after close should fail
        assert!(matches!(
            backend.create_producer("test").await,
            Err(BackendError::Closed)
        ));
        assert!(matches!(
            backend.create_consumer("test", "group").await,
            Err(BackendError::Closed)
        ));
    }

    #[tokio::test]
    async fn test_clear() {
        let backend = InMemoryBackend::new();
        let producer = backend.create_producer("events").await.unwrap();

        // Send events
        for i in 1..=5 {
            producer
                .send_event(create_test_event("thread-1", i))
                .await
                .unwrap();
        }

        assert_eq!(backend.message_count(), 5);
        assert_eq!(backend.topic_message_count("events").await, 5);

        // Clear
        backend.clear().await;

        assert_eq!(backend.message_count(), 0);
        assert_eq!(backend.topic_message_count("events").await, 0);
    }

    #[tokio::test]
    async fn test_flush_is_noop() {
        let backend = InMemoryBackend::new();
        let producer = backend.create_producer("events").await.unwrap();

        // Flush should succeed (no-op for in-memory)
        assert!(producer.flush().await.is_ok());
    }

    #[tokio::test]
    async fn test_commit_offset() {
        let backend = InMemoryBackend::new();
        let producer = backend.create_producer("events").await.unwrap();

        // Send events
        for i in 1..=3 {
            producer
                .send_event(create_test_event("thread-1", i))
                .await
                .unwrap();
        }

        // Consumer reads 2 messages and commits
        let mut consumer = backend.create_consumer("events", "group").await.unwrap();
        let _ = consumer.next_timeout(Duration::from_millis(100)).await;
        let _ = consumer.next_timeout(Duration::from_millis(100)).await;
        consumer.commit().await.unwrap();

        // New consumer should start from committed offset
        let new_consumer = backend.create_consumer("events", "group").await.unwrap();
        assert_eq!(new_consumer.current_offset(), 2);
    }

    #[tokio::test]
    async fn test_concurrent_producers() {
        let backend = Arc::new(InMemoryBackend::new());

        let handles: Vec<_> = (0..3)
            .map(|i| {
                let backend = backend.clone();
                tokio::spawn(async move {
                    let producer = backend.create_producer("events").await.unwrap();
                    for j in 0..10 {
                        producer
                            .send_event(create_test_event(&format!("thread-{}", i), j))
                            .await
                            .unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(backend.message_count(), 30);
    }
}
