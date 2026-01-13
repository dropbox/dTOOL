//! Test to catch 100% decode error scenarios
//!
//! This test ensures that:
//! 1. The producer encodes messages correctly
//! 2. The consumer can decode what the producer encodes
//! 3. Decode errors are reported properly (not silently dropped)
//!
//! If this test fails, it means there's a codec mismatch between producer and consumer.

use anyhow::{bail, Context, Result};
use dashflow_streaming::codec::{
    decode_message_strict, encode_message_with_compression, encode_message_with_compression_config,
    DEFAULT_COMPRESSION_LEVEL, DEFAULT_MAX_PAYLOAD_SIZE, HEADER_UNCOMPRESSED,
};
use dashflow_streaming::dash_stream_message::Message;
use dashflow_streaming::{DashStreamMessage, Event, EventType, Header, MessageType};
use std::collections::HashMap;

/// Create a test message with typical DashStream content
fn create_test_message(node_id: &str, event_type: EventType) -> DashStreamMessage {
    let event = Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread-123".to_string(),
            sequence: 1,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: event_type as i32,
        node_id: node_id.to_string(),
        attributes: HashMap::new(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    };

    DashStreamMessage {
        message: Some(Message::Event(event)),
    }
}

#[test]
fn test_encode_decode_roundtrip_uncompressed() -> Result<()> {
    let msg = create_test_message("analyze", EventType::NodeStart);

    // Encode without compression (but still in the framed format expected by strict decoding)
    let (encoded, _was_compressed) =
        encode_message_with_compression(&msg, false)?;

    // Decode
    let decoded = decode_message_strict(&encoded, DEFAULT_MAX_PAYLOAD_SIZE)?;

    // Verify fields
    let Some(Message::Event(event)) = decoded.message else {
        bail!("Decoded message should contain an Event");
    };
    let header = event.header.context("Header should be present")?;
    assert_eq!(header.tenant_id, "test-tenant");
    assert_eq!(header.thread_id, "test-thread-123");
    assert_eq!(event.event_type, EventType::NodeStart as i32);
    assert_eq!(event.node_id, "analyze");
    Ok(())
}

#[test]
fn test_encode_decode_roundtrip_compressed() -> Result<()> {
    // Create a larger message that will be compressed
    let mut event = Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread-123".to_string(),
            sequence: 1,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::NodeEnd as i32,
        node_id: "search".to_string(),
        attributes: HashMap::new(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    };

    // Add enough data to trigger compression (> 512 bytes threshold)
    // Use a long node_id instead of attributes to avoid AttributeValue conversion
    event.node_id = "x".repeat(1000);

    let msg = DashStreamMessage {
        message: Some(Message::Event(event)),
    };

    // Encode with compression
    let (encoded, was_compressed) =
        encode_message_with_compression(&msg, true)?;

    // Verify compression occurred for large message
    println!(
        "Message was compressed: {}, encoded size: {}",
        was_compressed,
        encoded.len()
    );

    // Decode (should auto-detect compression)
    let decoded = decode_message_strict(&encoded, DEFAULT_MAX_PAYLOAD_SIZE)?;

    // Verify fields survive roundtrip
    let Some(Message::Event(event)) = decoded.message else {
        bail!("Decoded message should contain an Event");
    };
    let header = event.header.context("Header should be present")?;
    assert_eq!(header.tenant_id, "test-tenant");
    assert_eq!(event.node_id.len(), 1000); // Verify long node_id survived
    Ok(())
}

#[test]
fn test_encode_decode_with_custom_compression_config() -> Result<()> {
    let mut event = Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread-123".to_string(),
            sequence: 1,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::NodeStart as i32,
        node_id: "write".to_string(),
        attributes: HashMap::new(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    };

    // Add data that exceeds custom threshold
    // Use a longer node_id to avoid AttributeValue conversion issues
    event.node_id = "y".repeat(256);

    let msg = DashStreamMessage {
        message: Some(Message::Event(event)),
    };

    // Use custom compression config with lower threshold
    let (encoded, _was_compressed) = encode_message_with_compression_config(
        &msg,
        true,
        100, // Lower threshold than default
        DEFAULT_COMPRESSION_LEVEL,
    )
    ?;

    // Decode
    let decoded = decode_message_strict(&encoded, DEFAULT_MAX_PAYLOAD_SIZE)?;

    assert!(decoded.message.is_some());
    Ok(())
}

#[test]
fn test_invalid_data_returns_error_not_panic() -> Result<()> {
    // Test that invalid data produces an error, not a panic or silent failure
    let mut invalid_data = vec![HEADER_UNCOMPRESSED];
    invalid_data.extend_from_slice(b"not a valid protobuf message");

    let result = decode_message_strict(&invalid_data, DEFAULT_MAX_PAYLOAD_SIZE);
    assert!(
        result.is_err(),
        "Invalid data should return error, not succeed"
    );
    Ok(())
}

#[test]
fn test_empty_data_returns_error() -> Result<()> {
    let empty_data: &[u8] = &[];

    let result = decode_message_strict(empty_data, DEFAULT_MAX_PAYLOAD_SIZE);
    assert!(result.is_err(), "Empty data should return error");
    Ok(())
}

#[test]
fn test_truncated_data_returns_error() -> Result<()> {
    let msg = create_test_message("node", EventType::NodeStart);
    let (encoded, _was_compressed) =
        encode_message_with_compression(&msg, false)?;

    // Truncate the message
    let truncated = &encoded[..encoded.len() / 2];

    let result = decode_message_strict(truncated, DEFAULT_MAX_PAYLOAD_SIZE);
    assert!(result.is_err(), "Truncated data should return error");
    Ok(())
}

/// CRITICAL: This test simulates what happens when producer and consumer have mismatched codecs
/// If this test fails with 100% decode errors, the live graph visualization is broken!
#[test]
fn test_producer_consumer_compatibility_zero_decode_errors() -> Result<()> {
    // Simulate producer sending a batch of messages
    let messages = vec![
        create_test_message("analyze", EventType::GraphStart),
        create_test_message("analyze", EventType::NodeStart),
        create_test_message("search", EventType::NodeStart),
        create_test_message("search", EventType::NodeEnd),
        create_test_message("write", EventType::NodeStart),
        create_test_message("write", EventType::NodeEnd),
        create_test_message("", EventType::GraphEnd),
    ];

    let mut decode_errors = 0;
    let mut decode_successes = 0;

    for msg in &messages {
        // Producer encodes with current settings (compression enabled)
        let (encoded, _) = encode_message_with_compression(msg, true)?;

        // Consumer decodes
        match decode_message_strict(&encoded, DEFAULT_MAX_PAYLOAD_SIZE) {
            Ok(_decoded) => {
                decode_successes += 1;
            }
            Err(e) => {
                decode_errors += 1;
                eprintln!("Decode error: {}", e);
            }
        }
    }

    let total = decode_errors + decode_successes;
    let error_rate = if total > 0 {
        decode_errors as f64 / total as f64
    } else {
        1.0
    };

    println!(
        "=== DECODE ERROR RATE: {:.2}% ({} errors / {} total) ===",
        error_rate * 100.0,
        decode_errors,
        total
    );

    // CRITICAL: This test catches the 100% decode error scenario
    // If this assertion fails, the live graph visualization is broken!
    assert!(
        error_rate < 0.01,
        "CRITICAL: Decode error rate is {:.2}% ({} errors / {} total). \
         Producer and consumer codec MISMATCH! \
         The live graph visualization will show 100% decode errors.",
        error_rate * 100.0,
        decode_errors,
        total
    );

    assert!(
        decode_successes == messages.len(),
        "All {} messages should decode successfully, but only {} did",
        messages.len(),
        decode_successes
    );
    Ok(())
}
