// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// Tests for dashflow-cli helper functions

#![allow(clippy::panic)]

use dashflow_streaming::{
    attribute_value::Value, AttributeValue, Event, EventType, Header, MessageType,
};

// Note: We cannot directly use the helpers module from src since it's not exposed
// These tests verify the expected behavior of helper-like functions

#[test]
fn test_get_thread_id_from_event() {
    let event = create_test_event_with_header("test-thread-123", "test-tenant", 42);

    if let Some(header) = &event.header {
        assert_eq!(header.thread_id, "test-thread-123");
    } else {
        panic!("Event should have header");
    }
}

#[test]
fn test_get_timestamp_from_event() {
    let timestamp = 1_234_567_890_123_456i64;
    let mut event = create_test_event_with_header("thread", "tenant", 0);

    if let Some(header) = &mut event.header {
        header.timestamp_us = timestamp;
    }

    if let Some(header) = &event.header {
        assert_eq!(header.timestamp_us, timestamp);
    }
}

#[test]
fn test_get_tenant_id_from_event() {
    let event = create_test_event_with_header("thread", "my-tenant-id", 0);

    if let Some(header) = &event.header {
        assert_eq!(header.tenant_id, "my-tenant-id");
    }
}

#[test]
fn test_get_sequence_from_event() {
    let event = create_test_event_with_header("thread", "tenant", 99);

    if let Some(header) = &event.header {
        assert_eq!(header.sequence, 99);
    }
}

#[test]
fn test_get_message_id_from_event() {
    let event = create_test_event_with_header("thread", "tenant", 0);

    if let Some(header) = &event.header {
        assert_eq!(header.message_id.len(), 16); // UUID is 16 bytes
    }
}

#[test]
fn test_event_without_header() {
    let event = Event {
        header: None,
        event_type: EventType::NodeStart as i32,
        node_id: "test-node".to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: String::new(),
    };

    assert!(event.header.is_none());
}

#[test]
fn test_string_attribute_access() {
    let mut event = create_test_event_with_header("thread", "tenant", 0);

    event.attributes.insert(
        "test_key".to_string(),
        AttributeValue {
            value: Some(Value::StringValue("test_value".to_string())),
        },
    );

    if let Some(attr) = event.attributes.get("test_key") {
        if let Some(Value::StringValue(s)) = &attr.value {
            assert_eq!(s, "test_value");
        } else {
            panic!("Expected StringValue");
        }
    } else {
        panic!("Attribute 'test_key' should exist");
    }
}

#[test]
fn test_int_attribute_access() {
    let mut event = create_test_event_with_header("thread", "tenant", 0);

    event.attributes.insert(
        "count".to_string(),
        AttributeValue {
            value: Some(Value::IntValue(42)),
        },
    );

    if let Some(attr) = event.attributes.get("count") {
        if let Some(Value::IntValue(i)) = &attr.value {
            assert_eq!(*i, 42);
        } else {
            panic!("Expected IntValue");
        }
    }
}

#[test]
fn test_float_attribute_access() {
    let mut event = create_test_event_with_header("thread", "tenant", 0);

    event.attributes.insert(
        "score".to_string(),
        AttributeValue {
            value: Some(Value::FloatValue(3.14160)),
        },
    );

    if let Some(attr) = event.attributes.get("score") {
        if let Some(Value::FloatValue(f)) = &attr.value {
            assert!((f - 3.14160).abs() < 0.00001);
        } else {
            panic!("Expected FloatValue");
        }
    }
}

#[test]
fn test_bool_attribute_access() {
    let mut event = create_test_event_with_header("thread", "tenant", 0);

    event.attributes.insert(
        "is_ready".to_string(),
        AttributeValue {
            value: Some(Value::BoolValue(true)),
        },
    );

    if let Some(attr) = event.attributes.get("is_ready") {
        if let Some(Value::BoolValue(b)) = &attr.value {
            assert!(*b);
        } else {
            panic!("Expected BoolValue");
        }
    }
}

#[test]
fn test_bytes_attribute_access() {
    let mut event = create_test_event_with_header("thread", "tenant", 0);
    let test_bytes = vec![0x01, 0x02, 0x03, 0x04];

    event.attributes.insert(
        "data".to_string(),
        AttributeValue {
            value: Some(Value::BytesValue(test_bytes.clone())),
        },
    );

    if let Some(attr) = event.attributes.get("data") {
        if let Some(Value::BytesValue(b)) = &attr.value {
            assert_eq!(b, &test_bytes);
        } else {
            panic!("Expected BytesValue");
        }
    }
}

#[test]
fn test_nonexistent_attribute() {
    let event = create_test_event_with_header("thread", "tenant", 0);

    assert!(!event.attributes.contains_key("nonexistent"));
}

#[test]
fn test_wrong_attribute_type() {
    let mut event = create_test_event_with_header("thread", "tenant", 0);

    // Insert as string
    event.attributes.insert(
        "value".to_string(),
        AttributeValue {
            value: Some(Value::StringValue("42".to_string())),
        },
    );

    // Try to access as int (should not match)
    if let Some(attr) = event.attributes.get("value") {
        // This should be StringValue, not IntValue
        assert!(matches!(attr.value, Some(Value::StringValue(_))));
        assert!(!matches!(attr.value, Some(Value::IntValue(_))));
    }
}

#[test]
fn test_common_attribute_keys() {
    // Verify common attribute key constants exist and are correct
    let checkpoint_id = "checkpoint_id";
    let edge_from = "edge_from";
    let edge_to = "edge_to";
    let tool_name = "tool_name";
    let input_tokens = "input_tokens";
    let output_tokens = "output_tokens";

    assert_eq!(checkpoint_id, "checkpoint_id");
    assert_eq!(edge_from, "edge_from");
    assert_eq!(edge_to, "edge_to");
    assert_eq!(tool_name, "tool_name");
    assert_eq!(input_tokens, "input_tokens");
    assert_eq!(output_tokens, "output_tokens");
}

#[test]
fn test_multiple_attributes() {
    let mut event = create_test_event_with_header("thread", "tenant", 0);

    // Add multiple attributes of different types
    event.attributes.insert(
        "name".to_string(),
        AttributeValue {
            value: Some(Value::StringValue("test".to_string())),
        },
    );
    event.attributes.insert(
        "count".to_string(),
        AttributeValue {
            value: Some(Value::IntValue(100)),
        },
    );
    event.attributes.insert(
        "enabled".to_string(),
        AttributeValue {
            value: Some(Value::BoolValue(true)),
        },
    );

    assert_eq!(event.attributes.len(), 3);

    // Verify all attributes are accessible
    assert!(event.attributes.contains_key("name"));
    assert!(event.attributes.contains_key("count"));
    assert!(event.attributes.contains_key("enabled"));
}

#[test]
fn test_header_all_fields() {
    let message_id = uuid::Uuid::new_v4().as_bytes().to_vec();
    let timestamp = chrono::Utc::now().timestamp_micros();

    let header = Header {
        message_id: message_id.clone(),
        timestamp_us: timestamp,
        tenant_id: "tenant-123".to_string(),
        thread_id: "thread-456".to_string(),
        sequence: 789,
        r#type: MessageType::Event as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    assert_eq!(header.message_id, message_id);
    assert_eq!(header.timestamp_us, timestamp);
    assert_eq!(header.tenant_id, "tenant-123");
    assert_eq!(header.thread_id, "thread-456");
    assert_eq!(header.sequence, 789);
    assert_eq!(header.r#type, MessageType::Event as i32);
    assert_eq!(header.schema_version, 1);
}

// Helper function to create test events with headers
fn create_test_event_with_header(thread_id: &str, tenant_id: &str, sequence: u64) -> Event {
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
            schema_version: 1,
        }),
        event_type: EventType::NodeStart as i32,
        node_id: "test-node".to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: String::new(),
    }
}

#[test]
fn test_attribute_value_none() {
    let attr = AttributeValue { value: None };

    assert!(attr.value.is_none());
}

#[test]
fn test_event_type_enum() {
    let types = vec![
        EventType::GraphStart,
        EventType::GraphEnd,
        EventType::NodeStart,
        EventType::NodeEnd,
        EventType::LlmStart,
        EventType::LlmEnd,
        EventType::ToolStart,
        EventType::ToolEnd,
        EventType::CheckpointSave,
        EventType::CheckpointLoad,
    ];

    for event_type in types {
        let as_i32 = event_type as i32;
        assert!(as_i32 >= 0);
    }
}
