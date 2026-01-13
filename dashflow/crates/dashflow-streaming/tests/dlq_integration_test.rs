//! Integration tests for Dead Letter Queue (DLQ) handling
//!
//! Tests the full DLQ workflow:
//! 1. Creating DLQ messages with error context
//! 2. Sending failed messages to Kafka DLQ topic
//! 3. Consuming and verifying DLQ message format
//! 4. Validating forensic data is preserved

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_streaming::dlq::{DlqHandler, DlqMessage};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message as KafkaMessage;
use rdkafka::producer::FutureProducer;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::kafka::apache;

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_dlq_message_serialization() {
    // Test that DlqMessage can be created and serialized correctly
    let original_payload = b"corrupted protobuf data";
    let msg = DlqMessage::new(
        original_payload,
        "Protobuf decode failed: invalid wire type",
        "dashstream-events",
        0,
        12345,
        "websocket-server-1",
        "decode_error",
    )
    .with_thread_id("session-abc-123")
    .with_tenant_id("tenant-xyz-789");

    let json = msg.to_json().expect("Should serialize to JSON");

    // Verify JSON contains all required fields
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse JSON");

    assert_eq!(parsed["error"], "Protobuf decode failed: invalid wire type");
    assert_eq!(parsed["source_topic"], "dashstream-events");
    assert_eq!(parsed["source_partition"], 0);
    assert_eq!(parsed["source_offset"], 12345);
    assert_eq!(parsed["consumer_id"], "websocket-server-1");
    assert_eq!(parsed["error_type"], "decode_error");
    assert_eq!(parsed["thread_id"], "session-abc-123");
    assert_eq!(parsed["tenant_id"], "tenant-xyz-789");

    // Verify base64 payload can be decoded
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::engine::Engine;
    let decoded = BASE64
        .decode(parsed["original_payload_base64"].as_str().unwrap())
        .expect("Should decode base64");
    assert_eq!(decoded, original_payload);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_dlq_handler_kafka_integration() {
    // Start Kafka testcontainer
    let kafka_container = apache::Kafka::default().start().await.unwrap();
    let bootstrap_servers = format!(
        "127.0.0.1:{}",
        kafka_container.get_host_port_ipv4(9093).await.unwrap()
    );

    // Wait for Kafka to be ready
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Create producer for DLQ
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create producer");

    // Create DlqHandler
    let dlq_handler = DlqHandler::new(producer, "test-dlq-topic", Duration::from_secs(5));

    // Create a DLQ message
    let original_payload = b"invalid protobuf bytes";
    let dlq_msg = DlqMessage::new(
        original_payload,
        "Failed to decode protobuf message",
        "dashstream-events",
        0,
        42,
        "test-consumer",
        "decode_error",
    )
    .with_trace_id("test-trace-123");

    // Send to DLQ
    dlq_handler
        .send(&dlq_msg)
        .await
        .expect("Should send to DLQ successfully");

    // Create consumer to verify message was sent
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("group.id", "dlq-test-consumer")
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Failed to create consumer");

    consumer
        .subscribe(&["test-dlq-topic"])
        .expect("Failed to subscribe to DLQ topic");

    // Consume the DLQ message
    let msg = tokio::time::timeout(Duration::from_secs(10), consumer.recv())
        .await
        .expect("Timeout waiting for DLQ message")
        .expect("Failed to receive DLQ message");

    // Verify message key is the trace ID
    let key = msg.key().expect("Message should have key");
    assert_eq!(std::str::from_utf8(key).unwrap(), "test-trace-123");

    // Verify message payload
    let payload = msg.payload().expect("Message should have payload");
    let json_str = std::str::from_utf8(payload).expect("Payload should be UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).expect("Payload should be valid JSON");

    assert_eq!(parsed["trace_id"], "test-trace-123");
    assert_eq!(parsed["error"], "Failed to decode protobuf message");
    assert_eq!(parsed["source_topic"], "dashstream-events");
    assert_eq!(parsed["source_partition"], 0);
    assert_eq!(parsed["source_offset"], 42);
    assert_eq!(parsed["consumer_id"], "test-consumer");
    assert_eq!(parsed["error_type"], "decode_error");

    // Verify original payload is preserved
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::engine::Engine;
    let decoded_payload = BASE64
        .decode(parsed["original_payload_base64"].as_str().unwrap())
        .expect("Should decode base64");
    assert_eq!(decoded_payload, original_payload);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_dlq_handler_fire_and_forget() {
    // Start Kafka testcontainer
    let kafka_container = apache::Kafka::default().start().await.unwrap();
    let bootstrap_servers = format!(
        "127.0.0.1:{}",
        kafka_container.get_host_port_ipv4(9093).await.unwrap()
    );

    tokio::time::sleep(Duration::from_secs(5)).await;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create producer");

    let dlq_handler = DlqHandler::new(producer, "test-dlq-fire-forget", Duration::from_secs(5));

    // Send using fire-and-forget
    let dlq_msg = DlqMessage::new(
        b"test payload",
        "test error",
        "test-topic",
        0,
        100,
        "test-consumer",
        "test_error",
    )
    .with_trace_id("fire-forget-trace");

    dlq_handler.send_fire_and_forget(dlq_msg);

    // Give time for async send to complete
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify message was sent
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("group.id", "dlq-fire-forget-consumer")
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Failed to create consumer");

    consumer
        .subscribe(&["test-dlq-fire-forget"])
        .expect("Failed to subscribe");

    let msg = tokio::time::timeout(Duration::from_secs(10), consumer.recv())
        .await
        .expect("Timeout waiting for message")
        .expect("Failed to receive message");

    let key = msg.key().expect("Should have key");
    assert_eq!(std::str::from_utf8(key).unwrap(), "fire-forget-trace");
}

#[test]
fn test_dlq_message_builder_pattern() {
    // Test the builder pattern for DlqMessage
    let msg = DlqMessage::new(
        b"payload",
        "error message",
        "topic",
        1,
        1000,
        "consumer-id",
        "error_type",
    )
    .with_thread_id("thread-1")
    .with_tenant_id("tenant-1")
    .with_trace_id("custom-trace");

    assert_eq!(msg.thread_id, Some("thread-1".to_string()));
    assert_eq!(msg.tenant_id, Some("tenant-1".to_string()));
    assert_eq!(msg.trace_id, "custom-trace");
    assert_eq!(msg.error, "error message");
    assert_eq!(msg.source_topic, "topic");
}

#[test]
fn test_dlq_message_forensic_data() {
    // Verify all forensic data is captured correctly
    let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let msg = DlqMessage::new(
        &payload,
        "Invalid protobuf wire type",
        "dashstream-events",
        2,
        9999,
        "websocket-server-prod-01",
        "wire_type_error",
    )
    .with_thread_id("session-critical-123")
    .with_tenant_id("enterprise-customer-456");

    // Verify all fields are set
    assert!(!msg.original_payload_base64.is_empty());
    assert_eq!(msg.error, "Invalid protobuf wire type");
    assert_eq!(msg.source_topic, "dashstream-events");
    assert_eq!(msg.source_partition, 2);
    assert_eq!(msg.source_offset, 9999);
    assert_eq!(msg.consumer_id, "websocket-server-prod-01");
    assert_eq!(msg.error_type, "wire_type_error");
    assert_eq!(msg.thread_id, Some("session-critical-123".to_string()));
    assert_eq!(msg.tenant_id, Some("enterprise-customer-456".to_string()));

    // Verify timestamp is valid
    assert!(chrono::DateTime::parse_from_rfc3339(&msg.timestamp).is_ok());

    // Verify trace_id is a valid UUID
    assert!(uuid::Uuid::parse_str(&msg.trace_id).is_ok());

    // Verify payload can be recovered
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::engine::Engine;
    let decoded = BASE64.decode(&msg.original_payload_base64).unwrap();
    assert_eq!(decoded, payload);
}

#[test]
fn test_dlq_message_json_roundtrip() {
    // Test that DlqMessage can survive JSON serialization/deserialization
    let original = DlqMessage::new(
        b"test payload",
        "test error",
        "test-topic",
        0,
        100,
        "test-consumer",
        "test_error",
    )
    .with_thread_id("thread-1");

    let json = original.to_json().expect("Should serialize");
    let deserialized: DlqMessage = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(
        deserialized.original_payload_base64,
        original.original_payload_base64
    );
    assert_eq!(deserialized.error, original.error);
    assert_eq!(deserialized.source_topic, original.source_topic);
    assert_eq!(deserialized.source_partition, original.source_partition);
    assert_eq!(deserialized.source_offset, original.source_offset);
    assert_eq!(deserialized.consumer_id, original.consumer_id);
    assert_eq!(deserialized.error_type, original.error_type);
    assert_eq!(deserialized.thread_id, original.thread_id);
    assert_eq!(deserialized.trace_id, original.trace_id);
}

#[test]
fn test_dlq_message_handles_large_payloads() {
    // Test that DlqMessage can handle large payloads without exceeding broker limits.
    let large_payload = vec![0xFF; 1024 * 1024]; // 1MB
    let msg = DlqMessage::new(
        &large_payload,
        "Large message decode failed",
        "dashstream-events",
        0,
        1000,
        "consumer",
        "decode_error",
    );

    // Large payloads are truncated to keep DLQ messages bounded.
    assert_eq!(msg.original_payload_truncated, Some(true));
    assert_eq!(msg.original_payload_size_bytes, Some(1024 * 1024));
    assert_eq!(msg.original_payload_included_bytes, Some(512 * 1024));
    assert!(msg.original_payload_sha256.is_some());

    // Verify JSON serialization works.
    let json = msg.to_json().expect("Should serialize large payload");

    // Verify it can be deserialized
    let deserialized: DlqMessage =
        serde_json::from_str(&json).expect("Should deserialize large payload");

    assert_eq!(deserialized.original_payload_truncated, Some(true));
    assert_eq!(deserialized.original_payload_size_bytes, Some(1024 * 1024));
    assert_eq!(deserialized.original_payload_included_bytes, Some(512 * 1024));
    assert!(deserialized.original_payload_sha256.is_some());

    // Verify payload prefix is preserved.
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::engine::Engine;
    let decoded = BASE64
        .decode(&deserialized.original_payload_base64)
        .unwrap();
    assert_eq!(decoded.len(), 512 * 1024);
    assert_eq!(decoded, large_payload[..512 * 1024]);
}

#[test]
fn test_dlq_message_optional_fields_omitted_in_json() {
    // Test that optional fields (thread_id, tenant_id) are omitted when None
    let msg = DlqMessage::new(
        b"payload",
        "error",
        "topic",
        0,
        100,
        "consumer",
        "error_type",
    );
    // Don't set thread_id or tenant_id

    let json = msg.to_json().expect("Should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Verify optional fields are not present in JSON
    assert!(parsed.get("thread_id").is_none());
    assert!(parsed.get("tenant_id").is_none());
}
