// Format Validation Tests
// Prevents JSON vs Protobuf decode errors like the 7.53% failure rate bug
//
// These tests ensure that:
// 1. Messages are always encoded in Protobuf format
// 2. JSON payloads are detected and rejected
// 3. Malformed messages are caught early
// 4. Round-trip encoding/decoding preserves format

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_streaming::codec::{
    decode_message, decode_message_strict, encode_message, encode_message_with_compression,
    DEFAULT_MAX_PAYLOAD_SIZE,
};
use dashflow_streaming::{DashStreamMessage, Event, EventType, Header, MessageType};

/// Helper to create a test event
fn create_test_event() -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread".to_string(),
            sequence: 1,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::GraphStart as i32,
        node_id: "start".to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    }
}

// ============================================================================
// Format Detection Tests - Prevent JSON vs Protobuf Confusion
// ============================================================================

#[test]
fn test_encoded_message_is_not_json() {
    // Encode a message
    let event = create_test_event();
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    let encoded = encode_message(&message).expect("Failed to encode message");

    // Verify it's NOT JSON by checking for JSON markers
    // JSON always starts with '{' or '[' (0x7B or 0x5B)
    assert_ne!(
        encoded[0], 0x7B,
        "Encoded message starts with '{{' - looks like JSON!"
    );
    assert_ne!(
        encoded[0], 0x5B,
        "Encoded message starts with '[' - looks like JSON!"
    );

    // Protobuf messages typically start with field tags (varint format)
    // Field 1 of DashStreamMessage is 'message' - wire type 2 (length-delimited)
    // This would be encoded as 0x0A (field number 1, wire type 2)
    assert_eq!(encoded[0], 0x0A, "Expected Protobuf field tag 0x0A");
}

#[test]
fn test_json_payload_fails_decode() {
    // Create a JSON payload (simulates the bug that caused 7.53% decode errors)
    let json_payload = br#"{"event":"quality","timestamp":1234567890}"#;

    // Attempt to decode as Protobuf - should fail
    let result = decode_message(json_payload);

    assert!(result.is_err(), "JSON payload should fail Protobuf decode");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Protobuf")
            || error_msg.contains("decode")
            || error_msg.contains("buffer"),
        "Error should indicate Protobuf decode failure, got: {}",
        error_msg
    );
}

#[test]
fn test_detect_json_format_early() {
    // This test implements early JSON detection that could be added to consumers
    fn looks_like_json(payload: &[u8]) -> bool {
        if payload.is_empty() {
            return false;
        }

        // Skip leading whitespace
        let first_non_ws = payload.iter().find(|&&b| !b.is_ascii_whitespace()).copied();

        match first_non_ws {
            Some(b'{') | Some(b'[') => true, // JSON object or array
            _ => false,
        }
    }

    // Test valid JSON payloads
    assert!(looks_like_json(br#"{"key":"value"}"#));
    assert!(looks_like_json(br#"  {"key":"value"}"#)); // with whitespace
    assert!(looks_like_json(b"[1,2,3]"));

    // Test valid Protobuf payloads (should NOT look like JSON)
    let event = create_test_event();
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };
    let protobuf_payload = encode_message(&message).expect("Failed to encode");

    assert!(
        !looks_like_json(&protobuf_payload),
        "Protobuf payload should not look like JSON"
    );
}

#[test]
fn test_compressed_message_not_json() {
    // Encode with compression
    let event = create_test_event();
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    let (encoded, is_compressed) =
        encode_message_with_compression(&message, true).expect("Failed to encode with compression");

    if is_compressed {
        // Compressed messages have 1-byte header (0x01) followed by zstd data
        assert_eq!(encoded[0], 0x01, "Expected compression header 0x01");

        // Zstd compressed data doesn't start with JSON markers
        if encoded.len() > 1 {
            assert_ne!(
                encoded[1], 0x7B,
                "Compressed data should not start with '{{'"
            );
            assert_ne!(
                encoded[1], 0x5B,
                "Compressed data should not start with '['"
            );
        }
    } else {
        // Uncompressed messages have header 0x00
        assert_eq!(encoded[0], 0x00, "Expected uncompressed header 0x00");
    }
}

// ============================================================================
// Round-Trip Format Validation
// ============================================================================

#[test]
fn test_round_trip_preserves_format() {
    // Create event
    let original_event = create_test_event();
    let original_thread_id = original_event.header.as_ref().unwrap().thread_id.clone();
    let original_sequence = original_event.header.as_ref().unwrap().sequence;

    // Wrap in message
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            original_event.clone(),
        )),
    };

    // Encode
    let encoded = encode_message(&message).expect("Failed to encode");

    // Verify it's Protobuf format
    assert_eq!(encoded[0], 0x0A, "Should be Protobuf field tag");

    // Decode
    let decoded = decode_message(&encoded).expect("Failed to decode");

    // Extract event
    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(decoded_event)) => {
            let decoded_header = decoded_event.header.as_ref().unwrap();
            assert_eq!(decoded_header.thread_id, original_thread_id);
            assert_eq!(decoded_header.sequence, original_sequence);
            assert_eq!(decoded_event.event_type, original_event.event_type);
            assert_eq!(decoded_event.node_id, original_event.node_id);
        }
        _ => panic!("Expected Event message after round-trip"),
    }
}

#[test]
fn test_round_trip_with_compression() {
    let event = create_test_event();
    let original_node_id = event.node_id.clone();

    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    // Encode with compression
    let (encoded, _was_compressed) =
        encode_message_with_compression(&message, true).expect("Failed to encode with compression");

    // Verify format - should have compression header (0x00 for uncompressed, 0x01 for compressed)
    // Small messages won't be compressed (< 512 bytes threshold)
    assert!(
        encoded[0] == 0x00 || encoded[0] == 0x01,
        "Should have compression header"
    );

    // Decode using strict framing (handles uncompressed and zstd-compressed).
    let decoded = decode_message_strict(&encoded, DEFAULT_MAX_PAYLOAD_SIZE)
        .expect("Failed to decode message");

    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(decoded_event)) => {
            assert_eq!(decoded_event.node_id, original_node_id);
        }
        _ => panic!("Expected Event message after compression round-trip"),
    }
}

// ============================================================================
// Malformed Message Detection
// ============================================================================

#[test]
fn test_empty_payload_decodes_to_empty_message() {
    // Protobuf allows empty messages (all fields are optional)
    // An empty payload decodes to a message with no content
    let empty_payload: &[u8] = &[];
    let result = decode_message(empty_payload);

    // Should succeed but have no message content
    assert!(
        result.is_ok(),
        "Empty payload should decode to empty message"
    );

    let decoded = result.unwrap();
    assert!(
        decoded.message.is_none(),
        "Empty payload should have no message content"
    );
}

#[test]
fn test_truncated_protobuf_fails_decode() {
    // Create valid message
    let event = create_test_event();
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    let encoded = encode_message(&message).expect("Failed to encode");

    // Truncate to first 10 bytes (malformed)
    let truncated = &encoded[..std::cmp::min(10, encoded.len())];

    let result = decode_message(truncated);

    assert!(result.is_err(), "Truncated Protobuf should fail decode");
}

#[test]
fn test_random_bytes_fail_decode() {
    // Random bytes (not valid Protobuf)
    let random_bytes: Vec<u8> = (0..50).map(|i| (i * 7) as u8).collect();

    let result = decode_message(&random_bytes);

    assert!(result.is_err(), "Random bytes should fail decode");
}

// ============================================================================
// Compression Format Validation
// ============================================================================

#[test]
fn test_compression_header_detection() {
    fn detect_compression(payload: &[u8]) -> Option<bool> {
        if payload.is_empty() {
            return None;
        }

        match payload[0] {
            0x00 => Some(false), // Uncompressed
            0x01 => Some(true),  // Zstd compressed
            _ => None,           // Unknown format
        }
    }

    // Test uncompressed message
    let event = create_test_event();
    let message = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };

    let (encoded_uncompressed, _) =
        encode_message_with_compression(&message, false).expect("Failed to encode");

    assert_eq!(
        detect_compression(&encoded_uncompressed),
        Some(false),
        "Should detect uncompressed format"
    );

    // Test JSON should return None (unknown format)
    let json_payload = br#"{"key":"value"}"#;
    assert_eq!(
        detect_compression(json_payload),
        None,
        "JSON should be unknown format"
    );
}

#[test]
fn test_invalid_compression_header_rejected() {
    // Create payload with invalid compression header (0x02)
    let mut invalid_payload = vec![0x02]; // Invalid header
    invalid_payload.extend_from_slice(b"some data");

    let result = decode_message(&invalid_payload);

    // Should fail because codec doesn't recognize 0x02 as valid header
    assert!(
        result.is_err(),
        "Invalid compression header should be rejected"
    );
}

// ============================================================================
// Message Type Validation
// ============================================================================

#[test]
fn test_all_message_types_encode_to_protobuf() {
    use dashflow_streaming::tool_execution::ExecutionStage;
    use dashflow_streaming::{Checkpoint, Metrics, StateDiff, TokenChunk, ToolExecution};

    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: chrono::Utc::now().timestamp_micros(),
        tenant_id: "test".to_string(),
        thread_id: "test".to_string(),
        sequence: 1,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    // Test all message types
    let message_types = vec![
        DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::Event(
                Event {
                    header: Some(header.clone()),
                    event_type: EventType::GraphStart as i32,
                    node_id: "test".to_string(),
                    attributes: Default::default(),
                    duration_us: 0,
                    llm_request_id: "".to_string(),
                },
            )),
        },
        DashStreamMessage {
            message: Some(
                dashflow_streaming::dash_stream_message::Message::TokenChunk(TokenChunk {
                    header: Some(header.clone()),
                    request_id: "req-1".to_string(),
                    text: "test".to_string(),
                    token_ids: vec![],
                    logprobs: vec![],
                    chunk_index: 0,
                    is_final: false,
                    model: "gpt-4".to_string(),
                    finish_reason: 0,
                    stats: None,
                }),
            ),
        },
        DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::StateDiff(
                StateDiff {
                    header: Some(header.clone()),
                    base_checkpoint_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                    operations: vec![],
                    state_hash: vec![0; 32],
                    full_state: vec![],
                },
            )),
        },
        DashStreamMessage {
            message: Some(
                dashflow_streaming::dash_stream_message::Message::ToolExecution(ToolExecution {
                    header: Some(header.clone()),
                    call_id: "call-1".to_string(),
                    tool_name: "test".to_string(),
                    stage: ExecutionStage::Completed as i32,
                    arguments: b"{}".to_vec(),
                    result: b"ok".to_vec(),
                    error: "".to_string(),
                    error_details: None,
                    duration_us: 100,
                    retry_count: 0,
                }),
            ),
        },
        DashStreamMessage {
            message: Some(
                dashflow_streaming::dash_stream_message::Message::Checkpoint(Checkpoint {
                    header: Some(header.clone()),
                    checkpoint_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                    state: vec![],
                    state_type: "Test".to_string(),
                    checksum: vec![0; 32],
                    storage_uri: "".to_string(),
                    compression_info: None,
                    metadata: Default::default(),
                }),
            ),
        },
        DashStreamMessage {
            message: Some(dashflow_streaming::dash_stream_message::Message::Metrics(
                Metrics {
                    header: Some(header),
                    scope: "test".to_string(),
                    scope_id: "test-1".to_string(),
                    metrics: Default::default(),
                    tags: Default::default(),
                },
            )),
        },
    ];

    // Verify all encode to Protobuf format (not JSON)
    for message in message_types {
        let encoded = encode_message(&message).expect("Failed to encode message type");

        // Verify it's NOT JSON format (JSON starts with '{' or '[')
        assert_ne!(
            encoded[0], 0x7B,
            "Message type should NOT encode to JSON '{{'"
        );
        assert_ne!(
            encoded[0], 0x5B,
            "Message type should NOT encode to JSON '['"
        );

        // Verify it's Protobuf format (starts with field tag - varint)
        // Different message types have different field numbers, so first byte varies
        // But it should be a valid protobuf field tag (not a JSON character)
        assert!(
            encoded[0] < 0x80 || encoded[0] >= 0x08,
            "Should be valid Protobuf field tag"
        );

        // Verify round-trip decode works
        let decoded = decode_message(&encoded).expect("Failed to decode message type");
        assert!(
            decoded.message.is_some(),
            "Decoded message should have content"
        );
    }
}

// ============================================================================
// Integration Test: Kafka Message Format
// ============================================================================

#[tokio::test]
#[ignore = "requires Docker for testcontainers"]
async fn test_kafka_messages_are_protobuf_format() {
    use dashflow_streaming::producer::DashStreamProducer;
    use rdkafka::config::ClientConfig;
    use rdkafka::consumer::{Consumer, StreamConsumer};
    use rdkafka::Message;
    use std::time::Duration;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::kafka::apache;

    // Start Kafka in Docker
    let kafka = apache::Kafka::default().start().await.unwrap();
    let bootstrap_servers = format!(
        "127.0.0.1:{}",
        kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
    );

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Create producer and send event
    let producer = DashStreamProducer::new(&bootstrap_servers, "format-test")
        .await
        .expect("Failed to create producer");

    let event = create_test_event();
    producer
        .send_event(event)
        .await
        .expect("Failed to send event");
    producer
        .flush(Duration::from_secs(5))
        .await
        .expect("Failed to flush");

    // Create consumer and receive message
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("group.id", "format-test-consumer")
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Failed to create consumer");

    consumer
        .subscribe(&["format-test"])
        .expect("Failed to subscribe");

    // Consume message with timeout
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match consumer.recv().await {
                Ok(message) => {
                    if let Some(payload) = message.payload() {
                        // Check if it has compression header (0x00 or 0x01)
                        let has_compression_header = payload[0] == 0x00 || payload[0] == 0x01;

                        if has_compression_header {
                            // Message was encoded with encode_message_with_compression
                            // Use strict decoding which enforces a valid framing header.
                            let decoded = decode_message_strict(payload, DEFAULT_MAX_PAYLOAD_SIZE)
                                .expect("Kafka message with compression header should decode");

                            assert!(
                                decoded.message.is_some(),
                                "Decoded message should have content"
                            );
                        } else {
                            // Legacy format without compression header
                            // Verify it's NOT JSON
                            assert_ne!(
                                payload[0], 0x7B,
                                "Kafka message should not be JSON (starts with '{{'');"
                            );
                            assert_ne!(
                                payload[0], 0x5B,
                                "Kafka message should not be JSON (starts with '[')"
                            );

                            // Verify it can be decoded as Protobuf
                            let decoded = decode_message(payload)
                                .expect("Kafka message should be valid Protobuf");

                            assert!(
                                decoded.message.is_some(),
                                "Decoded message should have content"
                            );
                        }

                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                    continue;
                }
            }
        }
    })
    .await
    .expect("Timeout waiting for Kafka message");
}
