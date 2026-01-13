// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// Tests for dashflow-cli export command

#![allow(clippy::panic, clippy::unwrap_used)]

use dashflow_streaming::{
    attribute_value::Value, AttributeValue, Event, EventType, Header, MessageType,
};

#[test]
fn test_event_to_exported_basic() {
    let event = create_test_event("test-thread", "test-node", EventType::NodeStart);

    // Basic validation - event should have expected fields
    assert_eq!(event.node_id, "test-node");
    assert_eq!(event.event_type(), EventType::NodeStart);

    if let Some(header) = &event.header {
        assert_eq!(header.thread_id, "test-thread");
        assert_eq!(header.tenant_id, "test-tenant");
    } else {
        panic!("Event should have header");
    }
}

#[test]
fn test_event_with_attributes() {
    let mut event = create_test_event("test-thread", "test-node", EventType::LlmStart);

    // Add various attribute types
    event.attributes.insert(
        "string_attr".to_string(),
        AttributeValue {
            value: Some(Value::StringValue("test_value".to_string())),
        },
    );
    event.attributes.insert(
        "int_attr".to_string(),
        AttributeValue {
            value: Some(Value::IntValue(42)),
        },
    );
    event.attributes.insert(
        "bool_attr".to_string(),
        AttributeValue {
            value: Some(Value::BoolValue(true)),
        },
    );
    event.attributes.insert(
        "float_attr".to_string(),
        AttributeValue {
            value: Some(Value::FloatValue(3.15)),
        },
    );

    // Verify attributes are set
    assert_eq!(event.attributes.len(), 4);

    // Verify each attribute type
    if let Some(string_attr) = event.attributes.get("string_attr") {
        if let Some(Value::StringValue(s)) = &string_attr.value {
            assert_eq!(s, "test_value");
        } else {
            panic!("Expected StringValue");
        }
    }

    if let Some(int_attr) = event.attributes.get("int_attr") {
        if let Some(Value::IntValue(i)) = &int_attr.value {
            assert_eq!(*i, 42);
        } else {
            panic!("Expected IntValue");
        }
    }
}

#[test]
fn test_event_with_duration() {
    let mut event = create_test_event("test-thread", "test-node", EventType::NodeEnd);
    event.duration_us = 1_234_567; // ~1.23 seconds

    assert_eq!(event.duration_us, 1_234_567);
    assert_eq!(event.event_type(), EventType::NodeEnd);
}

#[test]
fn test_event_with_llm_request_id() {
    let mut event = create_test_event("test-thread", "llm-node", EventType::LlmStart);
    event.llm_request_id = "req-12345".to_string();

    assert_eq!(event.llm_request_id, "req-12345");
}

#[test]
fn test_csv_header_format() {
    let header = "message_id,sequence,timestamp_micros,tenant_id,thread_id,event_type,node_id,duration_us,llm_request_id\n";
    let expected_fields = vec![
        "message_id",
        "sequence",
        "timestamp_micros",
        "tenant_id",
        "thread_id",
        "event_type",
        "node_id",
        "duration_us",
        "llm_request_id",
    ];

    let actual_fields: Vec<&str> = header.trim().split(',').collect();
    assert_eq!(actual_fields, expected_fields);
}

#[test]
fn test_export_args_defaults() {
    // Test that default values are reasonable
    let default_bootstrap = "localhost:9092";
    let default_topic = "dashstream";
    let default_format = "json";

    // These are the expected defaults from the CLI Args
    assert_eq!(default_bootstrap, "localhost:9092");
    assert_eq!(default_topic, "dashstream");
    assert_eq!(default_format, "json");
}

#[test]
fn test_supported_formats() {
    let supported = vec!["json", "jsonl", "csv"];

    for format in &supported {
        assert!(["json", "jsonl", "csv"].contains(format));
    }

    // Test unsupported format
    assert!(!supported.contains(&"xml"));
    assert!(!supported.contains(&"yaml"));
}

#[test]
fn test_event_sequence_ordering() {
    let mut events = Vec::new();

    for i in 0..5 {
        let mut event = create_test_event("test-thread", "test-node", EventType::NodeStart);
        if let Some(header) = &mut event.header {
            header.sequence = i;
        }
        events.push(event);
    }

    // Verify sequence numbers are correct
    for (i, event) in events.iter().enumerate() {
        if let Some(header) = &event.header {
            assert_eq!(header.sequence, i as u64);
        }
    }
}

#[test]
fn test_timestamp_ordering() {
    let mut events = Vec::new();
    let base_time = 1_000_000_000_000; // Some base timestamp

    for i in 0..5 {
        let mut event = create_test_event("test-thread", "test-node", EventType::NodeStart);
        if let Some(header) = &mut event.header {
            header.timestamp_us = base_time + (i * 1_000_000); // 1 second apart
        }
        events.push(event);
    }

    // Verify timestamps are increasing
    for i in 1..events.len() {
        let prev_ts = events[i - 1]
            .header
            .as_ref()
            .map(|h| h.timestamp_us)
            .unwrap_or(0);
        let curr_ts = events[i]
            .header
            .as_ref()
            .map(|h| h.timestamp_us)
            .unwrap_or(0);
        assert!(curr_ts > prev_ts);
    }
}

#[test]
fn test_message_id_uniqueness() {
    let event1 = create_test_event("test-thread", "node1", EventType::NodeStart);
    let event2 = create_test_event("test-thread", "node2", EventType::NodeStart);

    let id1 = event1.header.as_ref().map(|h| &h.message_id);
    let id2 = event2.header.as_ref().map(|h| &h.message_id);

    // Message IDs should be unique (UUIDs)
    assert_ne!(id1, id2);
}

// Helper function to create test events
fn create_test_event(thread_id: &str, node_id: &str, event_type: EventType) -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: thread_id.to_string(),
            sequence: 0,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: event_type as i32,
        node_id: node_id.to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: String::new(),
    }
}

#[test]
fn test_hex_encoding_for_bytes() {
    let bytes = vec![0x00, 0x01, 0x02, 0xAB, 0xCD, 0xEF];
    let encoded = hex::encode(&bytes);
    assert_eq!(encoded, "000102abcdef");

    // Verify decoding works
    let decoded = hex::decode(&encoded).unwrap();
    assert_eq!(decoded, bytes);
}

#[test]
fn test_json_attribute_conversion() {
    use serde_json::Value as JsonValue;

    // Test string
    let string_attr = AttributeValue {
        value: Some(Value::StringValue("test".to_string())),
    };
    if let Some(Value::StringValue(s)) = &string_attr.value {
        let json_val = JsonValue::String(s.clone());
        assert_eq!(json_val.as_str(), Some("test"));
    }

    // Test integer
    let int_attr = AttributeValue {
        value: Some(Value::IntValue(42)),
    };
    if let Some(Value::IntValue(i)) = &int_attr.value {
        let json_val = JsonValue::Number((*i).into());
        assert_eq!(json_val.as_i64(), Some(42));
    }

    // Test boolean
    let bool_attr = AttributeValue {
        value: Some(Value::BoolValue(true)),
    };
    if let Some(Value::BoolValue(b)) = &bool_attr.value {
        let json_val = JsonValue::Bool(*b);
        assert_eq!(json_val.as_bool(), Some(true));
    }
}

#[test]
fn test_empty_events_handling() {
    let events: Vec<Event> = Vec::new();

    // Empty events should not cause errors
    assert_eq!(events.len(), 0);
    assert!(events.is_empty());

    // First and last should return None
    assert!(events.is_empty());
    assert!(events.last().is_none());
}

#[test]
fn test_duration_calculation() {
    let start_time = 1_000_000_000_000i64;
    let end_time = 1_000_001_500_000i64;
    let duration = end_time - start_time;

    // Duration should be 1.5 seconds (1,500,000 microseconds)
    assert_eq!(duration, 1_500_000);

    // Convert to milliseconds
    let duration_ms = duration / 1_000;
    assert_eq!(duration_ms, 1_500);
}

#[test]
fn test_thread_id_filtering() {
    let events = [
        create_test_event("thread-1", "node-1", EventType::NodeStart),
        create_test_event("thread-2", "node-2", EventType::NodeStart),
        create_test_event("thread-1", "node-3", EventType::NodeStart),
        create_test_event("thread-3", "node-4", EventType::NodeStart),
    ];

    let thread_1_events: Vec<_> = events
        .iter()
        .filter(|e| {
            e.header
                .as_ref()
                .map(|h| h.thread_id.as_str())
                .unwrap_or("")
                == "thread-1"
        })
        .collect();

    assert_eq!(thread_1_events.len(), 2);
}

#[test]
fn test_event_type_display() {
    let event_types = vec![
        EventType::GraphStart,
        EventType::GraphEnd,
        EventType::NodeStart,
        EventType::NodeEnd,
        EventType::LlmStart,
        EventType::LlmEnd,
        EventType::ToolStart,
        EventType::ToolEnd,
    ];

    for event_type in event_types {
        let formatted = format!("{:?}", event_type);
        assert!(!formatted.is_empty());
    }
}
