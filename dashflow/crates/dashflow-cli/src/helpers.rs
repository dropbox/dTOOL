/// Helper functions for accessing Event fields from protobuf structure
///
/// The Event struct has a nested Header and attributes `HashMap`, so we need
/// helper functions for ergonomic field access.
use anyhow::{anyhow, Result};
use dashflow_streaming::codec::{decode_message_compatible, DEFAULT_MAX_PAYLOAD_SIZE};
use dashflow_streaming::{attribute_value, DashStreamMessage, Event, Header};

/// Decode a Kafka payload into a DashStreamMessage.
///
/// Producer frames include a 1-byte compression header (0x00/0x01). We decode
/// strictly by default, but fall back to legacy headerless decoding for
/// backwards compatibility with v1.0 topics.
pub fn decode_payload(payload: &[u8]) -> Result<DashStreamMessage> {
    decode_message_compatible(payload, DEFAULT_MAX_PAYLOAD_SIZE)
        .map_err(|e| anyhow!("Failed to decode DashStream payload: {}", e))
}

/// Get the header from an event
pub fn get_header(event: &Event) -> Option<&Header> {
    event.header.as_ref()
}

/// Get `thread_id` from event header
pub fn get_thread_id(event: &Event) -> Option<&str> {
    event.header.as_ref().map(|h| h.thread_id.as_str())
}

/// Get `timestamp_us` from event header
pub fn get_timestamp_us(event: &Event) -> Option<i64> {
    event.header.as_ref().map(|h| h.timestamp_us)
}

/// Get `tenant_id` from event header
pub fn get_tenant_id(event: &Event) -> Option<&str> {
    event.header.as_ref().map(|h| h.tenant_id.as_str())
}

/// Get sequence from event header
pub fn get_sequence(event: &Event) -> Option<u64> {
    event.header.as_ref().map(|h| h.sequence)
}

/// Get `message_id` from event header
pub fn get_message_id(event: &Event) -> Option<&[u8]> {
    event.header.as_ref().map(|h| h.message_id.as_slice())
}

/// Get a string attribute by key
pub fn get_string_attr(event: &Event, key: &str) -> Option<String> {
    event
        .attributes
        .get(key)
        .and_then(|v| v.value.as_ref())
        .and_then(|val| match val {
            attribute_value::Value::StringValue(s) => Some(s.clone()),
            _ => None,
        })
}

/// Get an int attribute by key
pub fn get_int_attr(event: &Event, key: &str) -> Option<i64> {
    event
        .attributes
        .get(key)
        .and_then(|v| v.value.as_ref())
        .and_then(|val| match val {
            attribute_value::Value::IntValue(i) => Some(*i),
            _ => None,
        })
}

/// Commonly used attribute keys
pub mod attr_keys {
    pub const CHECKPOINT_ID: &str = "checkpoint_id";
    pub const EDGE_FROM: &str = "edge_from";
    pub const EDGE_TO: &str = "edge_to";
    pub const TOOL_NAME: &str = "tool_name";
    pub const INPUT_TOKENS: &str = "input_tokens";
    pub const OUTPUT_TOKENS: &str = "output_tokens";
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow_streaming::codec::encode_message;
    use dashflow_streaming::{dash_stream_message, AttributeValue, EventType, MessageType};
    use std::collections::HashMap;

    fn make_header() -> Header {
        Header {
            message_id: vec![42; 16],
            timestamp_us: 123_456,
            tenant_id: "tenant".to_string(),
            thread_id: "thread".to_string(),
            sequence: 7,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }
    }

    fn make_event() -> Event {
        Event {
            header: Some(make_header()),
            event_type: EventType::NodeStart as i32,
            node_id: "node-1".to_string(),
            attributes: HashMap::new(),
            duration_us: 0,
            llm_request_id: String::new(),
        }
    }

    #[test]
    fn decode_payload_accepts_unframed_and_framed() {
        let msg = DashStreamMessage {
            message: Some(dash_stream_message::Message::Event(make_event())),
        };
        let bytes = encode_message(&msg).expect("encode_message");

        let decoded = decode_payload(&bytes).expect("decode_payload unframed");
        assert_eq!(decoded, msg);

        let mut framed = vec![0x00];
        framed.extend_from_slice(&bytes);
        let decoded = decode_payload(&framed).expect("decode_payload framed");
        assert_eq!(decoded, msg);
    }

    #[test]
    fn decode_payload_empty_is_error() {
        let err = decode_payload(&[]).expect_err("empty payload should fail");
        let msg = err.to_string();
        assert!(msg.contains("Failed to decode DashStream payload"));
    }

    #[test]
    fn header_getters_work() {
        let event = make_event();
        assert!(get_header(&event).is_some());
        assert_eq!(get_thread_id(&event), Some("thread"));
        assert_eq!(get_timestamp_us(&event), Some(123_456));
        assert_eq!(get_tenant_id(&event), Some("tenant"));
        assert_eq!(get_sequence(&event), Some(7));
        assert_eq!(get_message_id(&event), Some([42u8; 16].as_slice()));
    }

    #[test]
    fn attribute_getters_enforce_type() {
        let mut event = make_event();
        event.attributes.insert(
            "s".to_string(),
            AttributeValue {
                value: Some(attribute_value::Value::StringValue("v".to_string())),
            },
        );
        event.attributes.insert(
            "i".to_string(),
            AttributeValue {
                value: Some(attribute_value::Value::IntValue(123)),
            },
        );

        assert_eq!(get_string_attr(&event, "s"), Some("v".to_string()));
        assert_eq!(get_int_attr(&event, "i"), Some(123));
        assert_eq!(get_string_attr(&event, "i"), None);
        assert_eq!(get_int_attr(&event, "s"), None);
    }
}
