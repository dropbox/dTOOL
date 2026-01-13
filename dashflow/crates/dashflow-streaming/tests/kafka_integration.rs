// Kafka Integration Tests
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Integration tests for Kafka producer and consumer
//!
//! Run these tests with:
//! ```bash
//! docker-compose -f docker-compose-kafka.yml up -d
//! cargo test -p dashflow-streaming --test kafka_integration -- --ignored
//! ```

use dashflow_streaming::{
    consumer::{ConsumerConfig, DashStreamConsumer},
    kafka::{create_topic, delete_topic, list_topics, topic_exists},
    producer::{DashStreamProducer, ProducerConfig},
    Event, EventType, Header, MessageType, TokenChunk,
};
use std::time::Duration;

const BOOTSTRAP_SERVERS: &str = "localhost:9092";

/// Generate a unique topic name for each test to avoid interference
fn unique_topic(test_name: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("test-{}-{}", test_name, ts)
}

/// Topic config for tests - single partition so consumer reads all messages
fn test_topic_config() -> dashflow_streaming::kafka::TopicConfig {
    dashflow_streaming::kafka::TopicConfig {
        num_partitions: 1,                     // Single partition for tests
        replication_factor: 1,                 // Single broker
        retention_ms: 60 * 60 * 1000,          // 1 hour
        segment_bytes: 64 * 1024 * 1024,       // 64 MB
        cleanup_policy: "delete".to_string(),
        compression_type: "producer".to_string(),
        min_insync_replicas: None,             // Not needed for single broker tests
    }
}

/// K-8: RAII guard for topic cleanup - ensures topics are deleted even if test panics
struct TopicCleanupGuard {
    bootstrap_servers: String,
    topic: String,
}

impl TopicCleanupGuard {
    fn new(bootstrap_servers: &str, topic: &str) -> Self {
        Self {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
        }
    }
}

impl Drop for TopicCleanupGuard {
    fn drop(&mut self) {
        // Use a blocking runtime to clean up the topic
        // This handles panics and normal exits
        let bootstrap_servers = self.bootstrap_servers.clone();
        let topic = self.topic.clone();

        // Spawn blocking cleanup to ensure it runs even during panic unwind
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let _ = delete_topic(&bootstrap_servers, &topic).await;
            });
        })
        .join()
        .ok();
    }
}

fn create_test_event(sequence: u64) -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread-123".to_string(),
            sequence,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::GraphStart as i32,
        node_id: "start_node".to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    }
}

fn create_test_token_chunk(sequence: u64, text: &str) -> TokenChunk {
    TokenChunk {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread-123".to_string(),
            sequence,
            r#type: MessageType::TokenChunk as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        request_id: "".to_string(),
        text: text.to_string(),
        token_ids: vec![],
        logprobs: vec![],
        chunk_index: 0,
        is_final: false,
        finish_reason: 0,
        model: "".to_string(),
        stats: None,
    }
}

#[tokio::test]
#[ignore = "Requires Kafka running"]
async fn test_topic_management() {
    let topic = unique_topic("topic-mgmt");
    let _cleanup = TopicCleanupGuard::new(BOOTSTRAP_SERVERS, &topic);

    // Create topic
    let result = create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config()).await;
    assert!(result.is_ok(), "Failed to create topic: {:?}", result.err());

    // Wait for topic creation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check topic exists
    let exists = topic_exists(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Failed to check topic existence");
    assert!(exists, "Topic should exist after creation");

    // List topics
    let topics = list_topics(BOOTSTRAP_SERVERS)
        .await
        .expect("Failed to list topics");
    assert!(topics.contains(&topic), "Topic should be in list");

    // Cleanup handled by TopicCleanupGuard drop
}

#[tokio::test]
#[ignore = "Requires Kafka running"]
async fn test_producer_send_event() {
    let topic = unique_topic("producer-send");
    let _cleanup = TopicCleanupGuard::new(BOOTSTRAP_SERVERS, &topic);

    // Create topic
    let _ = create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config()).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Failed to create producer");

    // Send event
    let event = create_test_event(1);
    let result = producer.send_event(event).await;
    assert!(result.is_ok(), "Failed to send event: {:?}", result.err());

    // Flush
    let flush_result = producer.flush(Duration::from_secs(5)).await;
    assert!(
        flush_result.is_ok(),
        "Failed to flush: {:?}",
        flush_result.err()
    );

    // Cleanup handled by TopicCleanupGuard drop
}

#[tokio::test]
#[ignore = "Requires Kafka running"]
async fn test_producer_consumer_roundtrip() {
    let topic = unique_topic("roundtrip");
    let _cleanup = TopicCleanupGuard::new(BOOTSTRAP_SERVERS, &topic);

    // Create topic
    let _ = create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config()).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Failed to create producer");

    // Send events
    for i in 0..5 {
        let event = create_test_event(i);
        producer
            .send_event(event)
            .await
            .expect("Failed to send event");
    }
    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Failed to flush");

    // Give Kafka time to process
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create consumer
    let mut consumer = DashStreamConsumer::new(BOOTSTRAP_SERVERS, &topic, "test-consumer-group")
        .await
        .expect("Failed to create consumer");

    // Consume messages
    let mut count = 0;
    for _ in 0..5 {
        if let Some(result) = consumer.next_timeout(Duration::from_secs(10)).await {
            match result {
                Ok(msg) => {
                    assert!(msg.message.is_some(), "Message should have content");
                    count += 1;
                }
                Err(e) => {
                    panic!("Failed to decode message: {}", e);
                }
            }
        } else {
            break;
        }
    }

    assert_eq!(count, 5, "Should receive all 5 messages");

    // Cleanup handled by TopicCleanupGuard drop
}

#[tokio::test]
#[ignore = "Requires Kafka running"]
async fn test_compression_roundtrip() {
    let topic = unique_topic("compression");
    let _cleanup = TopicCleanupGuard::new(BOOTSTRAP_SERVERS, &topic);

    // Create topic
    let _ = create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config()).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer with compression
    let config = ProducerConfig {
        bootstrap_servers: BOOTSTRAP_SERVERS.to_string(),
        topic: topic.clone(),
        enable_compression: true,
        ..Default::default()
    };
    let producer = DashStreamProducer::with_config(config)
        .await
        .expect("Failed to create producer");

    // Send large token chunks (should be compressed)
    let large_text = "This is a test token chunk. ".repeat(100); // >512 bytes
    for i in 0..3 {
        let chunk = create_test_token_chunk(i, &large_text);
        producer
            .send_token_chunk(chunk)
            .await
            .expect("Failed to send chunk");
    }
    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Failed to flush");

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create consumer with decompression
    let consumer_config = ConsumerConfig {
        bootstrap_servers: BOOTSTRAP_SERVERS.to_string(),
        topic: topic.clone(),
        enable_decompression: true,
        ..Default::default()
    };
    let mut consumer = DashStreamConsumer::with_config(consumer_config)
        .await
        .expect("Failed to create consumer");

    // Consume and verify
    let mut count = 0;
    for _ in 0..3 {
        if let Some(result) = consumer.next_timeout(Duration::from_secs(10)).await {
            match result {
                Ok(msg) => {
                    // Verify decompressed message
                    if let Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(
                        chunk,
                    )) = msg.message
                    {
                        assert!(chunk.text.len() > 512, "Token chunk should be large");
                        count += 1;
                    }
                }
                Err(e) => {
                    panic!("Failed to decode message: {}", e);
                }
            }
        } else {
            break;
        }
    }

    assert_eq!(count, 3, "Should receive all 3 compressed messages");

    // Cleanup handled by TopicCleanupGuard drop
}

#[tokio::test]
#[ignore = "Requires Kafka running"]
async fn test_partition_ordering() {
    let topic = unique_topic("ordering");
    let _cleanup = TopicCleanupGuard::new(BOOTSTRAP_SERVERS, &topic);

    // Create topic
    let _ = create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config()).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Failed to create producer");

    // Send events with same thread_id (should go to same partition)
    for i in 0..10 {
        let event = create_test_event(i);
        producer
            .send_event(event)
            .await
            .expect("Failed to send event");
    }
    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Failed to flush");

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create consumer
    let mut consumer = DashStreamConsumer::new(BOOTSTRAP_SERVERS, &topic, "test-ordering-consumer")
        .await
        .expect("Failed to create consumer");

    // Verify ordering
    let mut last_sequence = None;
    for _ in 0..10 {
        if let Some(result) = consumer.next_timeout(Duration::from_secs(10)).await {
            match result {
                Ok(msg) => {
                    if let Some(dashflow_streaming::dash_stream_message::Message::Event(event)) =
                        msg.message
                    {
                        if let Some(header) = event.header {
                            if let Some(last_seq) = last_sequence {
                                assert!(
                                    header.sequence > last_seq,
                                    "Sequences should be in order: {} > {}",
                                    header.sequence,
                                    last_seq
                                );
                            }
                            last_sequence = Some(header.sequence);
                        }
                    }
                }
                Err(e) => {
                    panic!("Failed to decode message: {}", e);
                }
            }
        } else {
            break;
        }
    }

    assert!(
        last_sequence.is_some(),
        "Should receive at least one message"
    );
    assert_eq!(
        last_sequence.unwrap(),
        9,
        "Should receive message with sequence 9"
    );

    // Cleanup handled by TopicCleanupGuard drop
}
