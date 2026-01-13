// Mock Integration Tests
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

//! Integration tests that don't require Kafka
//!
//! These tests verify DashStream functionality using mocks and in-memory data structures,
//! enabling fast CI/CD testing without external dependencies.
//!
//! Run these tests with:
//! ```bash
//! cargo test --package dashflow-streaming --test mock_integration
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_streaming::{
    codec::{
        decode_message, decode_message_strict, encode_message, encode_message_with_compression,
        DEFAULT_MAX_PAYLOAD_SIZE,
    },
    compression::decompress_zstd,
    diff::{apply_patch, diff_states},
    producer::ProducerConfig,
    DashStreamMessage, Event, EventType, Header, MessageType, TokenChunk,
};
use serde_json::json;
use std::time::Duration;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_header(sequence: u64) -> Header {
    Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: chrono::Utc::now().timestamp_micros(),
        tenant_id: "test-tenant".to_string(),
        thread_id: "test-thread-123".to_string(),
        sequence,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    }
}

fn create_test_event(sequence: u64) -> Event {
    Event {
        header: Some(create_test_header(sequence)),
        event_type: EventType::GraphStart as i32,
        node_id: "start_node".to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    }
}

fn create_test_token_chunk(sequence: u64, text: &str, is_final: bool) -> TokenChunk {
    TokenChunk {
        header: Some(create_test_header(sequence)),
        request_id: "req-123".to_string(),
        text: text.to_string(),
        token_ids: vec![],
        logprobs: vec![],
        chunk_index: sequence as u32,
        is_final,
        finish_reason: if is_final { 1 } else { 0 }, // STOP if final
        model: "gpt-4".to_string(),
        stats: None,
    }
}

// ============================================================================
// Message Serialization Tests
// ============================================================================

#[test]
fn test_event_message_serialization() {
    let event = create_test_event(1);
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event.clone(),
        )),
    };

    // Encode
    let bytes = encode_message(&message).expect("Failed to encode event message");
    assert!(!bytes.is_empty());
    assert!(bytes.len() > 10); // Should have meaningful data

    // Decode
    let decoded = decode_message(&bytes).expect("Failed to decode event message");
    assert!(decoded.message.is_some());

    // Verify content
    if let Some(dashflow_streaming::dash_stream_message::Message::Event(decoded_event)) =
        decoded.message
    {
        assert_eq!(decoded_event.event_type, event.event_type);
        assert_eq!(decoded_event.node_id, event.node_id);
        assert!(decoded_event.header.is_some());
        let header = decoded_event.header.unwrap();
        assert_eq!(header.tenant_id, "test-tenant");
        assert_eq!(header.thread_id, "test-thread-123");
    } else {
        panic!("Decoded message is not an Event");
    }
}

#[test]
fn test_token_chunk_serialization() {
    let chunk = create_test_token_chunk(1, "Hello, world!", false);
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(chunk)),
    };

    // Encode
    let bytes = encode_message(&message).expect("Failed to encode token chunk");
    assert!(!bytes.is_empty());

    // Decode
    let decoded = decode_message(&bytes).expect("Failed to decode token chunk");

    // Verify
    if let Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(decoded_chunk)) =
        decoded.message
    {
        assert_eq!(decoded_chunk.text, "Hello, world!");
        assert!(!decoded_chunk.is_final);
        assert_eq!(decoded_chunk.model, "gpt-4");
    } else {
        panic!("Decoded message is not a TokenChunk");
    }
}

#[test]
fn test_multiple_message_types() {
    // Test all message types can be serialized/deserialized
    let messages = [
        DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::Event(
                create_test_event(1),
            )),
        },
        DashStreamMessage {
            message: Some(
                dashflow_streaming::dash_stream_message::Message::TokenChunk(
                    create_test_token_chunk(2, "test", true),
                ),
            ),
        },
    ];

    for (i, msg) in messages.iter().enumerate() {
        let bytes =
            encode_message(msg).unwrap_or_else(|_| panic!("Failed to encode message {}", i));
        let decoded =
            decode_message(&bytes).unwrap_or_else(|_| panic!("Failed to decode message {}", i));
        assert!(decoded.message.is_some());
    }
}

// ============================================================================
// Compression Tests
// ============================================================================

#[test]
fn test_message_compression() {
    // Create a large event with lots of data
    let mut event = create_test_event(1);
    event.node_id = "a".repeat(1000); // Make it large enough to benefit from compression

    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    // Encode with compression
    let (compressed_bytes, was_compressed) =
        encode_message_with_compression(&message, true).expect("Failed to compress message");

    // Should be compressed since message > 512 bytes
    assert!(was_compressed, "Message should be compressed");
    assert!(!compressed_bytes.is_empty());

    // Decode
    let decoded = decode_message_strict(&compressed_bytes, DEFAULT_MAX_PAYLOAD_SIZE)
        .expect("Failed to decode message");
    assert!(decoded.message.is_some());

    // Verify content survived compression
    if let Some(dashflow_streaming::dash_stream_message::Message::Event(decoded_event)) =
        decoded.message
    {
        assert_eq!(decoded_event.node_id.len(), 1000);
        assert!(decoded_event.node_id.starts_with("aaa"));
    }
}

#[test]
fn test_small_message_not_compressed() {
    // Small message (< 512 bytes) should not be compressed
    let event = create_test_event(1);
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    let (bytes, was_compressed) =
        encode_message_with_compression(&message, true).expect("Failed to encode message");

    // Small messages should not be compressed
    assert!(!was_compressed, "Small message should not be compressed");
    assert!(!bytes.is_empty());

    // Should still decode correctly
    let decoded =
        decode_message_strict(&bytes, DEFAULT_MAX_PAYLOAD_SIZE).expect("Failed to decode");
    assert!(decoded.message.is_some());
}

#[test]
fn test_compression_ratio() {
    // Test that compression actually reduces size for repetitive data
    let mut event = create_test_event(1);
    event.node_id = "a".repeat(10000); // Very repetitive data

    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    // Uncompressed
    let uncompressed = encode_message(&message).expect("Failed to encode");

    // Compressed
    let (compressed, was_compressed) =
        encode_message_with_compression(&message, true).expect("Failed to compress");

    assert!(was_compressed);
    assert!(compressed.len() < uncompressed.len());

    // Compression ratio should be significant for repetitive data
    let ratio = compressed.len() as f64 / uncompressed.len() as f64;
    assert!(
        ratio < 0.1,
        "Compression ratio should be < 10% for repetitive data, got {}",
        ratio
    );
}

// ============================================================================
// State Diffing Tests
// ============================================================================

#[test]
fn test_state_diff_generation() {
    let state1 = json!({
        "messages": ["Hello"],
        "counter": 1
    });

    let state2 = json!({
        "messages": ["Hello", "World"],
        "counter": 2
    });

    // Generate diff
    let diff = diff_states(&state1, &state2).expect("Failed to generate diff");

    // Should have metadata
    assert!(diff.patch_size > 0); // Patch should have size
    assert!(diff.full_state_size > 0); // State should have size
    assert!(!diff.state_hash.is_empty()); // Should have state hash
    assert!(!diff.patch_hash.is_empty()); // Should have patch hash
}

#[test]
fn test_state_diff_roundtrip() {
    let state1 = json!({
        "counter": 5,
        "messages": ["a", "b"]
    });

    let state2 = json!({
        "counter": 10,
        "messages": ["a", "b", "c"]
    });

    // Generate diff
    let diff = diff_states(&state1, &state2).expect("Failed to generate diff");

    // Apply patch to original state
    let result = apply_patch(&state1, &diff.patch).expect("Failed to apply patch");

    // Result should match state2
    assert_eq!(result, state2);
}

#[test]
fn test_empty_state_diff() {
    let state = json!({"counter": 1});

    // Diff with itself should produce minimal patch
    let diff = diff_states(&state, &state).expect("Failed to generate diff");

    // Apply patch (should be a no-op)
    let result = apply_patch(&state, &diff.patch).expect("Failed to apply patch");
    assert_eq!(result, state);
}

#[test]
fn test_complex_state_diff() {
    let state1 = json!({
        "nested": {
            "field1": "value1",
            "field2": [1, 2, 3]
        },
        "top_level": "test"
    });

    let state2 = json!({
        "nested": {
            "field1": "modified",
            "field2": [1, 2, 3, 4],
            "field3": "new"
        },
        "top_level": "test",
        "another_field": true
    });

    // Generate and apply patch
    let diff = diff_states(&state1, &state2).expect("Failed to generate diff");
    let result = apply_patch(&state1, &diff.patch).expect("Failed to apply patch");

    // Result should match state2
    assert_eq!(result, state2);
}

// ============================================================================
// Producer Configuration Tests
// ============================================================================

#[test]
fn test_producer_config_default() {
    let config = ProducerConfig::default();

    assert!(!config.bootstrap_servers.is_empty());
    assert!(!config.topic.is_empty());
    assert!(config.enable_compression);
    assert!(config.timeout.as_secs() > 0);
    assert!(config.max_in_flight > 0);
}

#[test]
fn test_producer_config_custom() {
    let config = ProducerConfig {
        bootstrap_servers: "kafka:9092".to_string(),
        topic: "custom-topic".to_string(),
        tenant_id: "test-tenant".to_string(),
        enable_compression: false,
        timeout: Duration::from_secs(60),
        enable_idempotence: true,
        max_in_flight: 1,
        kafka_compression: "gzip".to_string(),
        max_message_size: 1_048_576,
        ..Default::default()
    };

    assert_eq!(config.bootstrap_servers, "kafka:9092");
    assert_eq!(config.topic, "custom-topic");
    assert_eq!(config.tenant_id, "test-tenant");
    assert!(!config.enable_compression);
    assert_eq!(config.timeout, Duration::from_secs(60));
    assert_eq!(config.kafka_compression, "gzip");
}

#[test]
fn test_producer_config_validation() {
    // Test various valid configurations
    let configs = vec![
        ProducerConfig {
            enable_compression: false,
            ..Default::default()
        },
        ProducerConfig {
            enable_compression: true,
            ..Default::default()
        },
        ProducerConfig {
            max_in_flight: 1,
            ..Default::default()
        },
        ProducerConfig {
            max_in_flight: 1000,
            ..Default::default()
        },
    ];

    // All should be valid (no panics)
    for config in configs {
        assert!(!config.bootstrap_servers.is_empty());
    }
}

// ============================================================================
// Message Sequence Tests
// ============================================================================

#[test]
fn test_message_sequence_ordering() {
    let mut messages = vec![];

    // Create sequence of events
    for i in 0..100 {
        let event = create_test_event(i);
        let message = DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::Event(
                event,
            )),
        };
        messages.push(message);
    }

    // Serialize all messages
    for (i, msg) in messages.iter().enumerate() {
        let bytes =
            encode_message(msg).unwrap_or_else(|_| panic!("Failed to encode message {}", i));
        let decoded =
            decode_message(&bytes).unwrap_or_else(|_| panic!("Failed to decode message {}", i));

        // Verify sequence is preserved
        if let Some(dashflow_streaming::dash_stream_message::Message::Event(event)) =
            decoded.message
        {
            let header = event.header.expect("Missing header");
            assert_eq!(header.sequence, i as u64);
        }
    }
}

#[test]
fn test_streaming_token_sequence() {
    // Simulate streaming response
    let tokens = [
        "Hello", ", ", "world", "!", " ", "How", " ", "are", " ", "you", "?",
    ];

    let mut messages = vec![];
    for (i, token) in tokens.iter().enumerate() {
        let is_final = i == tokens.len() - 1;
        let chunk = create_test_token_chunk(i as u64, token, is_final);
        let message = DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(chunk)),
        };
        messages.push(message);
    }

    // Verify all messages can be serialized
    for msg in &messages {
        let bytes = encode_message(msg).expect("Failed to encode");
        let decoded = decode_message(&bytes).expect("Failed to decode");
        assert!(decoded.message.is_some());
    }

    // Verify last message is marked as final
    let last_bytes = encode_message(messages.last().unwrap()).expect("Failed to encode");
    let decoded = decode_message(&last_bytes).expect("Failed to decode");
    if let Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(chunk)) =
        decoded.message
    {
        assert!(chunk.is_final, "Last chunk should be marked as final");
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_decode_invalid_data() {
    let invalid_bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
    let result = decode_message(&invalid_bytes);
    assert!(result.is_err(), "Should fail to decode invalid data");
}

#[test]
fn test_decode_empty_data() {
    let empty_bytes = vec![];
    let result = decode_message(&empty_bytes);
    // Empty message can be decoded as default/empty protobuf - this is ok
    // The important thing is that it doesn't panic
    if let Ok(msg) = result {
        assert!(
            msg.message.is_none(),
            "Empty message should have no content"
        );
    }
}

#[test]
fn test_decompress_invalid_data() {
    let invalid_compressed = vec![0xFF, 0xFF, 0xFF, 0xFF];
    let result = decompress_zstd(&invalid_compressed);
    assert!(result.is_err(), "Should fail to decompress invalid data");
}

// ============================================================================
// Integration: Full Message Lifecycle
// ============================================================================

#[test]
fn test_full_message_lifecycle() {
    // Simulate a complete workflow
    let workflow = vec![
        ("GraphStart", create_test_event(0)),
        ("NodeStart", create_test_event(1)),
        ("NodeEnd", create_test_event(2)),
        ("GraphEnd", create_test_event(3)),
    ];

    for (label, mut event) in workflow {
        // Set appropriate event type
        event.event_type = match label {
            "GraphStart" => EventType::GraphStart as i32,
            "NodeStart" => EventType::NodeStart as i32,
            "NodeEnd" => EventType::NodeEnd as i32,
            "GraphEnd" => EventType::GraphEnd as i32,
            _ => EventType::Unspecified as i32,
        };

        // Create message
        let message = DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::Event(
                event,
            )),
        };

        // Encode with compression
        let (bytes, _was_compressed) = encode_message_with_compression(&message, true)
            .unwrap_or_else(|_| panic!("Failed to encode {} event", label));

        // Decode
        let decoded = decode_message_strict(&bytes, DEFAULT_MAX_PAYLOAD_SIZE)
            .unwrap_or_else(|_| panic!("Failed to decode {} event", label));

        // Verify
        assert!(decoded.message.is_some(), "{} event lost data", label);
    }
}

// ============================================================================
// Performance: Batch Operations
// ============================================================================

#[test]
fn test_batch_message_processing() {
    let batch_size: usize = 1000;
    let mut messages = vec![];

    // Create batch
    for i in 0..batch_size {
        let event = create_test_event(i as u64);
        let message = DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::Event(
                event,
            )),
        };
        messages.push(message);
    }

    // Process batch
    let mut encoded_messages = vec![];
    for msg in &messages {
        let bytes = encode_message(msg).expect("Failed to encode");
        encoded_messages.push(bytes);
    }

    // Verify all encoded
    assert_eq!(encoded_messages.len(), batch_size);

    // Decode batch
    for bytes in &encoded_messages {
        let decoded = decode_message(bytes).expect("Failed to decode");
        assert!(decoded.message.is_some());
    }
}
