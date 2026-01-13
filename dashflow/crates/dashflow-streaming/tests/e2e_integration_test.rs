// End-to-End Integration Test
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox
//
//! This test validates the COMPLETE streaming pipeline:
//! Producer → Kafka → Consumer → Decode → Validate
//!
//! Run with LIVE Kafka: `cargo test --test e2e_integration_test -- --ignored --test-threads=1`
//!
//! **Prerequisites**:
//! ```bash
//! docker-compose -f docker-compose.dashstream.yml up -d
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_streaming::consumer::DashStreamConsumer;
use dashflow_streaming::kafka::{create_topic, delete_topic};
use dashflow_streaming::producer::{DashStreamProducer, ProducerConfig};
use dashflow_streaming::{Event, EventType, Header, MessageType, StateDiff, TokenChunk};
use std::collections::HashSet;
use std::time::Duration;

const BOOTSTRAP_SERVERS: &str = "localhost:9092";

/// Helper to generate unique topic name for this test
fn test_topic() -> String {
    format!(
        "e2e-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    )
}

/// Helper to generate unique consumer group name for a topic
fn test_consumer_group(topic: &str) -> String {
    format!("cg-{}", topic)
}

/// Get E2E test topic configuration (single partition for simple consumer testing)
fn test_topic_config() -> dashflow_streaming::kafka::TopicConfig {
    use dashflow_streaming::kafka::TopicConfig;
    TopicConfig {
        num_partitions: 1, // Single partition to avoid multi-partition consumer complexity
        replication_factor: 1,
        retention_ms: 24 * 60 * 60 * 1000, // 1 day
        segment_bytes: 256 * 1024 * 1024,  // 256 MB
        cleanup_policy: "delete".to_string(),
        compression_type: "producer".to_string(), // Use producer's compression
        min_insync_replicas: None, // Not needed for single-replica test setup
    }
}

/// Create test event with specific parameters
fn create_test_event(
    tenant_id: &str,
    thread_id: &str,
    sequence: u64,
    schema_version: u32,
) -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: tenant_id.to_string(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version,
        }),
        event_type: EventType::NodeStart as i32,
        node_id: format!("node_{}", sequence),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    }
}

/// Create test token chunk
fn create_test_token_chunk(
    tenant_id: &str,
    thread_id: &str,
    sequence: u64,
    text: &str,
) -> TokenChunk {
    TokenChunk {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: tenant_id.to_string(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: MessageType::TokenChunk as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        request_id: "req-123".to_string(),
        text: text.to_string(),
        token_ids: vec![],
        logprobs: vec![],
        chunk_index: sequence as u32,
        is_final: false,
        finish_reason: 0,
        model: "test-model".to_string(),
        stats: None,
    }
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_end_to_end_single_message() {
    println!("\n=== E2E Test: Single Message ===");

    let topic = test_topic();
    println!("Topic: {}", topic);

    // 1. Create topic
    create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config())
        .await
        .expect("Should create topic");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 2. Create producer
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Should create producer");

    // 3. Send one event
    let event = create_test_event("tenant-1", "thread-1", 0, 1);
    producer
        .send_event(event.clone())
        .await
        .expect("Should send event");
    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Should flush");

    println!("✅ Sent 1 message");

    tokio::time::sleep(Duration::from_secs(2)).await;

    // 4. Create consumer with unique group
    let mut consumer =
        DashStreamConsumer::new(BOOTSTRAP_SERVERS, &topic, &test_consumer_group(&topic))
            .await
            .expect("Should create consumer");

    // 5. Consume message
    let msg = consumer
        .next_timeout(Duration::from_secs(10))
        .await
        .expect("Should receive message")
        .expect("Should decode message");

    // 6. Validate
    match msg.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(received_event)) => {
            let header = received_event.header.as_ref().unwrap();
            assert_eq!(header.tenant_id, "tenant-1");
            assert_eq!(header.thread_id, "thread-1");
            assert_eq!(header.sequence, 0);
            assert_eq!(header.schema_version, 1);
            assert_eq!(received_event.event_type, EventType::NodeStart as i32);
            assert_eq!(received_event.node_id, "node_0");
            println!("✅ Message validated successfully");
        }
        _ => panic!("Expected Event message"),
    }

    // Cleanup
    let _ = delete_topic(BOOTSTRAP_SERVERS, &topic).await;
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_end_to_end_message_ordering() {
    println!("\n=== E2E Test: Message Ordering ===");

    let topic = test_topic();
    println!("Topic: {}", topic);

    // Create topic
    create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config())
        .await
        .expect("Should create topic");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Should create producer");

    // Send 10 messages with increasing sequence numbers
    println!("Sending 10 messages...");
    for i in 0..10 {
        let event = create_test_event("tenant-1", "thread-1", i, 1);
        producer.send_event(event).await.expect("Should send");
    }
    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Should flush");
    println!("✅ Sent 10 messages");

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create consumer
    let mut consumer =
        DashStreamConsumer::new(BOOTSTRAP_SERVERS, &topic, &test_consumer_group(&topic))
            .await
            .expect("Should create consumer");

    // Consume all messages and check ordering
    let mut sequences = Vec::new();
    for _ in 0..10 {
        if let Some(Ok(msg)) = consumer.next_timeout(Duration::from_secs(10)).await {
            match msg.message {
                Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
                    sequences.push(event.header.as_ref().unwrap().sequence);
                }
                _ => panic!("Expected Event"),
            }
        }
    }

    // Validate ordering
    assert_eq!(sequences.len(), 10, "Should receive all 10 messages");
    for (i, &seq) in sequences.iter().enumerate().take(10) {
        assert_eq!(
            seq, i as u64,
            "Message sequence should be in order: expected {}, got {}",
            i, seq
        );
    }
    println!("✅ All messages received in correct order: {:?}", sequences);

    // Cleanup
    let _ = delete_topic(BOOTSTRAP_SERVERS, &topic).await;
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_end_to_end_no_message_loss() {
    println!("\n=== E2E Test: No Message Loss ===");

    let topic = test_topic();
    println!("Topic: {}", topic);

    // Create topic
    create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config())
        .await
        .expect("Should create topic");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Should create producer");

    // Send 100 messages and track message IDs
    println!("Sending 100 messages...");
    let mut sent_message_ids = HashSet::new();
    for i in 0..100 {
        let event = create_test_event("tenant-1", "thread-1", i, 1);
        let message_id = event.header.as_ref().unwrap().message_id.clone();
        sent_message_ids.insert(message_id);
        producer.send_event(event).await.expect("Should send");
    }
    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Should flush");
    println!("✅ Sent 100 messages");

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Create consumer
    let mut consumer =
        DashStreamConsumer::new(BOOTSTRAP_SERVERS, &topic, &test_consumer_group(&topic))
            .await
            .expect("Should create consumer");

    // Consume all messages
    let mut received_message_ids = HashSet::new();
    for _ in 0..100 {
        if let Some(Ok(msg)) = consumer.next_timeout(Duration::from_secs(10)).await {
            match msg.message {
                Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
                    let message_id = event.header.as_ref().unwrap().message_id.clone();
                    received_message_ids.insert(message_id);
                }
                _ => panic!("Expected Event"),
            }
        }
    }

    // Validate NO message loss
    assert_eq!(
        sent_message_ids.len(),
        100,
        "Should have sent 100 unique messages"
    );
    assert_eq!(
        received_message_ids.len(),
        100,
        "Should have received 100 unique messages"
    );
    assert_eq!(
        sent_message_ids, received_message_ids,
        "All sent messages should be received"
    );

    println!("✅ Zero message loss: 100 sent = 100 received");

    // Cleanup
    let _ = delete_topic(BOOTSTRAP_SERVERS, &topic).await;
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_end_to_end_mixed_message_types() {
    println!("\n=== E2E Test: Mixed Message Types ===");

    let topic = test_topic();
    println!("Topic: {}", topic);

    // Create topic
    create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config())
        .await
        .expect("Should create topic");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, &topic)
        .await
        .expect("Should create producer");

    // Send mixed message types
    println!("Sending mixed message types...");

    // 5 events
    for i in 0..5 {
        let event = create_test_event("tenant-1", "thread-1", i, 1);
        producer.send_event(event).await.expect("Should send event");
    }

    // 5 token chunks
    for i in 5..10 {
        let chunk = create_test_token_chunk("tenant-1", "thread-1", i, "Hello");
        producer
            .send_token_chunk(chunk)
            .await
            .expect("Should send token chunk");
    }

    // 5 state diffs
    for i in 10..15 {
        let state_diff = StateDiff {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "tenant-1".to_string(),
                thread_id: "thread-1".to_string(),
                sequence: i,
                r#type: MessageType::StateDiff as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            base_checkpoint_id: vec![],
            operations: vec![],
            state_hash: vec![1, 2, 3],
            full_state: vec![],
        };
        producer
            .send_state_diff(state_diff)
            .await
            .expect("Should send state diff");
    }

    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Should flush");
    println!("✅ Sent 15 mixed messages (5 events, 5 token chunks, 5 state diffs)");

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Create consumer
    let mut consumer =
        DashStreamConsumer::new(BOOTSTRAP_SERVERS, &topic, &test_consumer_group(&topic))
            .await
            .expect("Should create consumer");

    // Consume and count by type
    let mut event_count = 0;
    let mut token_count = 0;
    let mut state_diff_count = 0;

    for _ in 0..15 {
        if let Some(Ok(msg)) = consumer.next_timeout(Duration::from_secs(10)).await {
            match msg.message {
                Some(dashflow_streaming::dash_stream_message::Message::Event(_)) => {
                    event_count += 1
                }
                Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(_)) => {
                    token_count += 1
                }
                Some(dashflow_streaming::dash_stream_message::Message::StateDiff(_)) => {
                    state_diff_count += 1
                }
                _ => panic!("Unexpected message type"),
            }
        }
    }

    // Validate counts
    assert_eq!(event_count, 5, "Should receive 5 events");
    assert_eq!(token_count, 5, "Should receive 5 token chunks");
    assert_eq!(state_diff_count, 5, "Should receive 5 state diffs");

    println!(
        "✅ All message types received: {} events, {} token chunks, {} state diffs",
        event_count, token_count, state_diff_count
    );

    // Cleanup
    let _ = delete_topic(BOOTSTRAP_SERVERS, &topic).await;
}

#[tokio::test]
#[ignore = "requires Kafka"]
async fn test_end_to_end_multi_tenant() {
    println!("\n=== E2E Test: Multi-Tenant Isolation ===");

    let topic = test_topic();
    println!("Topic: {}", topic);

    // Create topic
    create_topic(BOOTSTRAP_SERVERS, &topic, test_topic_config())
        .await
        .expect("Should create topic");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producers for 2 tenants
    let config_a = ProducerConfig {
        bootstrap_servers: BOOTSTRAP_SERVERS.to_string(),
        topic: topic.clone(),
        tenant_id: "tenant-a".to_string(),
        ..Default::default()
    };
    let producer_a = DashStreamProducer::with_config(config_a)
        .await
        .expect("Should create producer A");

    let config_b = ProducerConfig {
        bootstrap_servers: BOOTSTRAP_SERVERS.to_string(),
        topic: topic.clone(),
        tenant_id: "tenant-b".to_string(),
        ..Default::default()
    };
    let producer_b = DashStreamProducer::with_config(config_b)
        .await
        .expect("Should create producer B");

    // Send messages from both tenants
    println!("Sending messages from 2 tenants...");
    for i in 0..5 {
        let event_a = create_test_event("tenant-a", "thread-a", i, 1);
        producer_a.send_event(event_a).await.expect("Should send A");

        let event_b = create_test_event("tenant-b", "thread-b", i, 1);
        producer_b.send_event(event_b).await.expect("Should send B");
    }

    producer_a
        .flush(Duration::from_secs(5))
        .await
        .expect("Should flush A");
    producer_b
        .flush(Duration::from_secs(5))
        .await
        .expect("Should flush B");
    println!("✅ Sent 10 messages (5 per tenant)");

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Create consumer
    let mut consumer =
        DashStreamConsumer::new(BOOTSTRAP_SERVERS, &topic, &test_consumer_group(&topic))
            .await
            .expect("Should create consumer");

    // Consume all messages and separate by tenant
    let mut tenant_a_messages = Vec::new();
    let mut tenant_b_messages = Vec::new();

    for _ in 0..10 {
        if let Some(Ok(msg)) = consumer.next_timeout(Duration::from_secs(10)).await {
            match msg.message {
                Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
                    let tenant_id = event.header.as_ref().unwrap().tenant_id.clone();
                    if tenant_id == "tenant-a" {
                        tenant_a_messages.push(event);
                    } else if tenant_id == "tenant-b" {
                        tenant_b_messages.push(event);
                    }
                }
                _ => panic!("Expected Event"),
            }
        }
    }

    // Validate isolation
    assert_eq!(
        tenant_a_messages.len(),
        5,
        "Should receive 5 messages for tenant-a"
    );
    assert_eq!(
        tenant_b_messages.len(),
        5,
        "Should receive 5 messages for tenant-b"
    );

    // Validate no cross-tenant leakage
    for event in &tenant_a_messages {
        assert_eq!(event.header.as_ref().unwrap().tenant_id, "tenant-a");
        assert_eq!(event.header.as_ref().unwrap().thread_id, "thread-a");
    }
    for event in &tenant_b_messages {
        assert_eq!(event.header.as_ref().unwrap().tenant_id, "tenant-b");
        assert_eq!(event.header.as_ref().unwrap().thread_id, "thread-b");
    }

    println!("✅ Tenant isolation validated: no cross-tenant data leakage");

    // Cleanup
    let _ = delete_topic(BOOTSTRAP_SERVERS, &topic).await;
}
