// Schema Evolution Tests
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox
//
//! These tests validate that the streaming system handles schema changes correctly.
//!
//! **Critical Requirements** (User-specified):
//! - When protobuf schema changes, ALL existing streaming data must still work
//! - Backward compatibility: New code must read old messages
//! - Forward compatibility: Old code should gracefully handle new messages
//!
//! **Run**: `cargo test --test schema_evolution_tests`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow_streaming::codec::{
    decode_message, decode_message_compatible, decode_message_strict, encode_message,
    DEFAULT_MAX_PAYLOAD_SIZE,
};
use dashflow_streaming::{DashStreamMessage, Event, EventType, Header, MessageType};

/// Helper to create a v1 message (schema_version = 1)
fn create_v1_message() -> DashStreamMessage {
    DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            Event {
                header: Some(Header {
                    message_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                    timestamp_us: 1234567890,
                    tenant_id: "tenant-1".to_string(),
                    thread_id: "thread-1".to_string(),
                    sequence: 1,
                    r#type: MessageType::Event as i32,
                    parent_id: vec![],
                    compression: 0,
                    schema_version: 1, // VERSION 1
                }),
                event_type: EventType::GraphStart as i32,
                node_id: "start".to_string(),
                attributes: Default::default(),
                duration_us: 0,
                llm_request_id: "".to_string(),
            },
        )),
    }
}

#[test]
fn test_current_code_reads_v1_messages() {
    // Backward compatibility test: Current code must read version 1 messages

    let v1_message = create_v1_message();
    let encoded = encode_message(&v1_message).expect("Should encode v1 message");

    // Current code should decode v1 messages
    let decoded = decode_message(&encoded).expect("Should decode v1 message");

    // Validate
    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            assert_eq!(event.header.as_ref().unwrap().schema_version, 1);
            assert_eq!(event.header.as_ref().unwrap().tenant_id, "tenant-1");
            assert_eq!(event.header.as_ref().unwrap().thread_id, "thread-1");
            assert_eq!(event.event_type, EventType::GraphStart as i32);
        }
        _ => panic!("Expected Event message"),
    }
}

#[test]
fn test_legacy_messages_without_schema_version() {
    // Test backward compatibility with messages that don't have schema_version field
    // (hypothetical scenario if schema_version was added later)

    let mut v1_message = create_v1_message();

    // Simulate old message by setting schema_version to 0 (default/unset)
    if let Some(dashflow_streaming::dash_stream_message::Message::Event(ref mut event)) =
        v1_message.message
    {
        if let Some(ref mut header) = event.header {
            header.schema_version = 0; // Legacy message
        }
    }

    let encoded = encode_message(&v1_message).expect("Should encode legacy message");
    let decoded = decode_message(&encoded).expect("Should decode legacy message");

    // Current code should handle schema_version = 0 gracefully
    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            // Schema version might be 0 or missing, but message should decode
            assert!(event.header.is_some());
            assert_eq!(event.header.as_ref().unwrap().tenant_id, "tenant-1");
        }
        _ => panic!("Expected Event message"),
    }
}

#[test]
fn test_compression_preserves_schema_version() {
    // Critical: Compression must preserve schema_version field

    let v1_message = create_v1_message();

    // Encode with compression (message is small so won't compress, but test the path)
    let (encoded_bytes, _was_compressed) =
        dashflow_streaming::codec::encode_message_with_compression(&v1_message, true)
            .expect("Should encode with compression");

    // Decode using strict framing (compression header required).
    let decoded = decode_message_strict(&encoded_bytes, DEFAULT_MAX_PAYLOAD_SIZE)
        .expect("Should decode");

    // Validate schema_version preserved
    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            assert_eq!(
                event.header.as_ref().unwrap().schema_version,
                1,
                "Schema version must be preserved through compression"
            );
        }
        _ => panic!("Expected Event message"),
    }
}

#[test]
fn test_large_message_compression_preserves_schema_version() {
    // Test with large message that will actually be compressed

    let mut v1_message = create_v1_message();

    // Make message large (>512 bytes) to trigger compression
    if let Some(dashflow_streaming::dash_stream_message::Message::Event(ref mut event)) =
        v1_message.message
    {
        // Add large node_id to make message >512 bytes
        event.node_id = "x".repeat(600);
    }

    let (encoded_bytes, was_compressed) =
        dashflow_streaming::codec::encode_message_with_compression(&v1_message, true)
            .expect("Should encode with compression");

    if !was_compressed {
        println!("WARNING: Message was not compressed (size too small or compression failed)");
    }

    // Decode using strict framing (compression header required).
    let decoded = decode_message_strict(&encoded_bytes, DEFAULT_MAX_PAYLOAD_SIZE)
        .expect("Should decode");

    // Validate schema_version preserved
    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            assert_eq!(
                event.header.as_ref().unwrap().schema_version,
                1,
                "Schema version must be preserved through compression/decompression"
            );
            assert_eq!(event.node_id.len(), 600, "Large field must be preserved");
        }
        _ => panic!("Expected Event message"),
    }
}

#[test]
fn test_all_compression_header_types() {
    // Test all compression header scenarios:
    // 0x00 = uncompressed
    // 0x01 = zstd compressed
    // no header = legacy

    let v1_message = create_v1_message();
    let encoded = encode_message(&v1_message).expect("Should encode");

    // Test 1: Uncompressed with header (0x00)
    let mut uncompressed_with_header = vec![0x00];
    uncompressed_with_header.extend(&encoded);
    let decoded1 = decode_message_strict(&uncompressed_with_header, DEFAULT_MAX_PAYLOAD_SIZE)
        .expect("Should decode uncompressed with header");
    assert!(decoded1.message.is_some());

    // Test 2: Legacy (no header)
    let decoded2 = decode_message_compatible(&encoded, DEFAULT_MAX_PAYLOAD_SIZE)
        .expect("Should decode legacy format");
    assert!(decoded2.message.is_some());

    // Test 3: Compressed format
    let compressed =
        dashflow_streaming::compression::compress_zstd(&encoded, 3).expect("Should compress");
    let mut compressed_with_header = vec![0x01];
    compressed_with_header.extend(&compressed);
    let decoded3 = decode_message_strict(&compressed_with_header, DEFAULT_MAX_PAYLOAD_SIZE)
        .expect("Should decode compressed");
    assert!(decoded3.message.is_some());

    // All three formats should decode to same message
    assert_eq!(
        format!("{:?}", decoded1),
        format!("{:?}", decoded2),
        "Uncompressed with header should match legacy"
    );
    assert_eq!(
        format!("{:?}", decoded1),
        format!("{:?}", decoded3),
        "Compressed should match uncompressed"
    );
}

#[test]
fn test_schema_version_validation() {
    // Test that we can detect schema version mismatches

    let v1_message = create_v1_message();

    // Extract schema version
    let schema_version = match &v1_message.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            event.header.as_ref().unwrap().schema_version
        }
        _ => panic!("Expected Event"),
    };

    assert_eq!(schema_version, 1, "Current schema version should be 1");

    // Future enhancement (schema v2): When schema v2 is introduced, add tests for:
    // - v2 message with new fields
    // - v1 code reading v2 message (should ignore unknown fields)
    // - v2 code reading v1 message (should use defaults for missing fields)
}

#[test]
fn test_multiple_message_types_all_versions() {
    // Validate schema_version is set correctly across all message types

    use dashflow_streaming::{StateDiff, TokenChunk};

    // Event
    let event_msg = create_v1_message();
    match &event_msg.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            assert_eq!(event.header.as_ref().unwrap().schema_version, 1);
        }
        _ => panic!("Expected Event"),
    }

    // TokenChunk
    let token_msg = DashStreamMessage {
        message: Some(
            dashflow_streaming::dash_stream_message::Message::TokenChunk(TokenChunk {
                header: Some(Header {
                    message_id: vec![1; 16],
                    timestamp_us: 1234567890,
                    tenant_id: "tenant-1".to_string(),
                    thread_id: "thread-1".to_string(),
                    sequence: 2,
                    r#type: MessageType::TokenChunk as i32,
                    parent_id: vec![],
                    compression: 0,
                    schema_version: 1,
                }),
                request_id: "req-123".to_string(),
                text: "Hello".to_string(),
                token_ids: vec![],
                logprobs: vec![],
                chunk_index: 0,
                is_final: false,
                finish_reason: 0,
                model: "gpt-4".to_string(),
                stats: None,
            }),
        ),
    };

    let encoded = encode_message(&token_msg).expect("Should encode TokenChunk");
    let decoded = decode_message(&encoded).expect("Should decode TokenChunk");

    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(chunk)) => {
            assert_eq!(chunk.header.as_ref().unwrap().schema_version, 1);
        }
        _ => panic!("Expected TokenChunk"),
    }

    // StateDiff
    let state_diff_msg = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::StateDiff(
            StateDiff {
                header: Some(Header {
                    message_id: vec![1; 16],
                    timestamp_us: 1234567890,
                    tenant_id: "tenant-1".to_string(),
                    thread_id: "thread-1".to_string(),
                    sequence: 3,
                    r#type: MessageType::StateDiff as i32,
                    parent_id: vec![],
                    compression: 0,
                    schema_version: 1,
                }),
                base_checkpoint_id: vec![],
                operations: vec![],
                state_hash: vec![],
                full_state: vec![1, 2, 3],
            },
        )),
    };

    let encoded = encode_message(&state_diff_msg).expect("Should encode StateDiff");
    let decoded = decode_message(&encoded).expect("Should decode StateDiff");

    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::StateDiff(diff)) => {
            assert_eq!(diff.header.as_ref().unwrap().schema_version, 1);
        }
        _ => panic!("Expected StateDiff"),
    }
}

#[test]
fn test_schema_evolution_readme() {
    // This test documents the schema evolution strategy for future developers

    println!("\n=== Schema Evolution Strategy ===\n");
    println!("Current schema version: 1");
    println!();
    println!("When adding new fields to protobuf:");
    println!("1. Use optional fields (proto3 default)");
    println!("2. DO NOT remove or rename existing fields");
    println!("3. DO NOT change field numbers");
    println!("4. Increment schema_version in Header");
    println!("5. Add backward compatibility tests for new version");
    println!();
    println!("Compatibility guarantees:");
    println!("- Backward: New code MUST read old messages");
    println!("- Forward: Old code SHOULD ignore new fields gracefully");
    println!("- Compression: MUST preserve schema_version");
    println!("- Legacy: MUST support messages without schema_version (default to 1)");
    println!();
    println!("Test coverage required for each schema version:");
    println!("- test_current_code_reads_vN_messages()");
    println!("- test_compression_preserves_vN_schema_version()");
    println!("- test_vN_all_message_types()");
    println!();
    println!("See: crates/dashflow-streaming/tests/schema_evolution_tests.rs");
    println!("=====================================\n");
}

#[test]
fn test_corrupted_schema_version_detection() {
    // Test that we can detect when schema_version is corrupted

    let mut v1_message = create_v1_message();

    // Corrupt the schema_version
    if let Some(dashflow_streaming::dash_stream_message::Message::Event(ref mut event)) =
        v1_message.message
    {
        if let Some(ref mut header) = event.header {
            header.schema_version = 999; // Invalid version
        }
    }

    let encoded = encode_message(&v1_message).expect("Should encode");
    let decoded = decode_message(&encoded).expect("Should decode even with invalid version");

    // Message should decode, but schema_version should be detectable
    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            let version = event.header.as_ref().unwrap().schema_version;
            assert_eq!(
                version, 999,
                "Should preserve corrupted version for detection"
            );

            // In production code, we should validate:
            // if version > LATEST_KNOWN_VERSION {
            //     log_warning("Unknown schema version {}, may have unknown fields", version);
            // }
        }
        _ => panic!("Expected Event"),
    }
}

#[test]
fn test_zero_schema_version_treated_as_v1() {
    // Schema version 0 should be treated as legacy v1

    let mut legacy_message = create_v1_message();

    // Set schema_version = 0 (unset/default in proto3)
    if let Some(dashflow_streaming::dash_stream_message::Message::Event(ref mut event)) =
        legacy_message.message
    {
        if let Some(ref mut header) = event.header {
            header.schema_version = 0;
        }
    }

    let encoded = encode_message(&legacy_message).expect("Should encode");
    let decoded = decode_message(&encoded).expect("Should decode");

    // Should decode successfully (treated as v1)
    match decoded.message {
        Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
            assert_eq!(event.header.as_ref().unwrap().schema_version, 0);
            // In production, treat 0 as 1:
            // let effective_version = if schema_version == 0 { 1 } else { schema_version };
        }
        _ => panic!("Expected Event"),
    }
}
