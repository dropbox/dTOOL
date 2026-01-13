// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// Tests for dashflow-cli costs analysis

use dashflow_streaming::{
    attribute_value::Value, AttributeValue, Event, EventType, Header, MessageType,
};

#[derive(Debug, Default)]
struct TokenUsage {
    input_tokens: i64,
    output_tokens: i64,
    llm_calls: usize,
}

impl TokenUsage {
    fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }

    fn cost(&self, input_cost: f64, output_cost: f64) -> f64 {
        (self.input_tokens as f64 / 1_000_000.0 * input_cost)
            + (self.output_tokens as f64 / 1_000_000.0 * output_cost)
    }
}

#[test]
fn test_token_usage_total() {
    let usage = TokenUsage {
        input_tokens: 1000,
        output_tokens: 500,
        llm_calls: 1,
    };

    assert_eq!(usage.total_tokens(), 1500);
}

#[test]
fn test_token_usage_cost_calculation() {
    let usage = TokenUsage {
        input_tokens: 1_000_000,
        output_tokens: 1_000_000,
        llm_calls: 1,
    };

    // Default costs: $0.25 per 1M input, $1.25 per 1M output
    let cost = usage.cost(0.25, 1.25);
    assert!((cost - 1.50).abs() < 0.001); // $0.25 + $1.25 = $1.50
}

#[test]
fn test_token_usage_cost_partial() {
    let usage = TokenUsage {
        input_tokens: 500_000,  // Half a million
        output_tokens: 250_000, // Quarter million
        llm_calls: 1,
    };

    let cost = usage.cost(0.25, 1.25);
    // 0.5M * $0.25 = $0.125
    // 0.25M * $1.25 = $0.3125
    // Total: $0.4375
    assert!((cost - 0.4375).abs() < 0.001);
}

#[test]
fn test_token_usage_cost_zero() {
    let usage = TokenUsage {
        input_tokens: 0,
        output_tokens: 0,
        llm_calls: 0,
    };

    let cost = usage.cost(0.25, 1.25);
    assert!((cost - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_token_usage_cost_custom_rates() {
    let usage = TokenUsage {
        input_tokens: 1_000_000,
        output_tokens: 1_000_000,
        llm_calls: 1,
    };

    // Custom rates: $5.00 input, $15.00 output
    let cost = usage.cost(5.0, 15.0);
    assert!((cost - 20.0).abs() < 0.001); // $5.00 + $15.00 = $20.00
}

#[test]
fn test_token_usage_large_numbers() {
    let usage = TokenUsage {
        input_tokens: 10_000_000, // 10 million
        output_tokens: 5_000_000, // 5 million
        llm_calls: 100,
    };

    let cost = usage.cost(0.25, 1.25);
    // 10M * $0.25 = $2.50
    // 5M * $1.25 = $6.25
    // Total: $8.75
    assert!((cost - 8.75).abs() < 0.001);
}

#[test]
fn test_default_costs_args() {
    // Verify default cost values are reasonable
    let input_cost = 0.25;
    let output_cost = 1.25;

    assert!(input_cost > 0.0);
    assert!(output_cost > 0.0);
    assert!(output_cost > input_cost); // Output typically costs more
}

#[test]
fn test_llm_event_detection() {
    let llm_start = create_llm_event("thread-1", EventType::LlmStart, 1000, 500);
    let llm_end = create_llm_event("thread-1", EventType::LlmEnd, 1000, 500);
    let node_start = create_llm_event("thread-1", EventType::NodeStart, 0, 0);

    assert_eq!(llm_start.event_type(), EventType::LlmStart);
    assert_eq!(llm_end.event_type(), EventType::LlmEnd);
    assert_eq!(node_start.event_type(), EventType::NodeStart);

    // Only LLM events should match
    assert!(matches!(
        llm_start.event_type(),
        EventType::LlmStart | EventType::LlmEnd
    ));
    assert!(matches!(
        llm_end.event_type(),
        EventType::LlmStart | EventType::LlmEnd
    ));
    assert!(!matches!(
        node_start.event_type(),
        EventType::LlmStart | EventType::LlmEnd
    ));
}

#[test]
fn test_token_extraction_from_event() {
    let event = create_llm_event("thread-1", EventType::LlmEnd, 2000, 1500);

    // Extract input_tokens attribute
    let input_tokens = event
        .attributes
        .get("input_tokens")
        .and_then(|v| v.value.as_ref())
        .and_then(|val| match val {
            Value::IntValue(i) => Some(*i),
            _ => None,
        });

    // Extract output_tokens attribute
    let output_tokens = event
        .attributes
        .get("output_tokens")
        .and_then(|v| v.value.as_ref())
        .and_then(|val| match val {
            Value::IntValue(i) => Some(*i),
            _ => None,
        });

    assert_eq!(input_tokens, Some(2000));
    assert_eq!(output_tokens, Some(1500));
}

#[test]
fn test_missing_token_attributes() {
    let mut event = create_llm_event("thread-1", EventType::LlmEnd, 0, 0);
    event.attributes.clear(); // Remove all attributes

    let input_tokens = event
        .attributes
        .get("input_tokens")
        .and_then(|v| v.value.as_ref())
        .and_then(|val| match val {
            Value::IntValue(i) => Some(*i),
            _ => None,
        });

    assert!(input_tokens.is_none());
}

#[test]
fn test_cost_per_call_calculation() {
    let usage = TokenUsage {
        input_tokens: 10_000,
        output_tokens: 5_000,
        llm_calls: 5,
    };

    let total_cost = usage.cost(0.25, 1.25);
    let cost_per_call = total_cost / usage.llm_calls as f64;

    // Total tokens: 15,000
    // Total cost: (0.01 * 0.25) + (0.005 * 1.25) = 0.0025 + 0.00625 = 0.00875
    // Cost per call: 0.00875 / 5 = 0.00175
    assert!(cost_per_call > 0.0);
    assert!((total_cost - 0.00875).abs() < 0.001);
}

#[test]
fn test_aggregation_by_thread() {
    let events = [
        create_llm_event("thread-1", EventType::LlmEnd, 1000, 500),
        create_llm_event("thread-1", EventType::LlmEnd, 2000, 1000),
        create_llm_event("thread-2", EventType::LlmEnd, 3000, 1500),
    ];

    // Count events per thread
    let thread1_count = events
        .iter()
        .filter(|e| e.header.as_ref().map(|h| h.thread_id.as_str()) == Some("thread-1"))
        .count();

    let thread2_count = events
        .iter()
        .filter(|e| e.header.as_ref().map(|h| h.thread_id.as_str()) == Some("thread-2"))
        .count();

    assert_eq!(thread1_count, 2);
    assert_eq!(thread2_count, 1);
}

#[test]
fn test_aggregation_by_node() {
    let events = [
        create_llm_event_with_node("thread-1", "researcher", EventType::LlmEnd, 1000, 500),
        create_llm_event_with_node("thread-1", "writer", EventType::LlmEnd, 2000, 1000),
        create_llm_event_with_node("thread-1", "researcher", EventType::LlmEnd, 1500, 750),
    ];

    // Count events per node
    let researcher_count = events.iter().filter(|e| e.node_id == "researcher").count();

    let writer_count = events.iter().filter(|e| e.node_id == "writer").count();

    assert_eq!(researcher_count, 2);
    assert_eq!(writer_count, 1);
}

#[test]
fn test_token_accumulation() {
    let events = [
        create_llm_event("thread-1", EventType::LlmEnd, 1000, 500),
        create_llm_event("thread-1", EventType::LlmEnd, 2000, 1500),
        create_llm_event("thread-1", EventType::LlmEnd, 1500, 1000),
    ];

    // Accumulate tokens
    let total_input: i64 = events
        .iter()
        .filter_map(|e| {
            e.attributes
                .get("input_tokens")
                .and_then(|v| v.value.as_ref())
                .and_then(|val| match val {
                    Value::IntValue(i) => Some(*i),
                    _ => None,
                })
        })
        .sum();

    let total_output: i64 = events
        .iter()
        .filter_map(|e| {
            e.attributes
                .get("output_tokens")
                .and_then(|v| v.value.as_ref())
                .and_then(|val| match val {
                    Value::IntValue(i) => Some(*i),
                    _ => None,
                })
        })
        .sum();

    assert_eq!(total_input, 4500); // 1000 + 2000 + 1500
    assert_eq!(total_output, 3000); // 500 + 1500 + 1000
}

#[test]
fn test_format_currency() {
    let cost = 12.345678;
    let formatted = format!("${:.2}", cost);
    assert_eq!(formatted, "$12.35");

    let small_cost = 0.001234;
    let formatted_small = format!("${:.4}", small_cost);
    assert_eq!(formatted_small, "$0.0012");
}

#[test]
fn test_cost_breakdown() {
    let usage = TokenUsage {
        input_tokens: 1_000_000,
        output_tokens: 500_000,
        llm_calls: 10,
    };

    let input_cost_rate = 0.25;
    let output_cost_rate = 1.25;

    let input_cost = usage.input_tokens as f64 / 1_000_000.0 * input_cost_rate;
    let output_cost = usage.output_tokens as f64 / 1_000_000.0 * output_cost_rate;
    let total_cost = input_cost + output_cost;

    assert!((input_cost - 0.25).abs() < 0.001);
    assert!((output_cost - 0.625).abs() < 0.001);
    assert!((total_cost - 0.875).abs() < 0.001);
}

// Helper functions

fn create_llm_event(
    thread_id: &str,
    event_type: EventType,
    input_tokens: i64,
    output_tokens: i64,
) -> Event {
    create_llm_event_with_node(
        thread_id,
        "default-node",
        event_type,
        input_tokens,
        output_tokens,
    )
}

fn create_llm_event_with_node(
    thread_id: &str,
    node_id: &str,
    event_type: EventType,
    input_tokens: i64,
    output_tokens: i64,
) -> Event {
    let mut event = Event {
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
    };

    // Add token attributes
    event.attributes.insert(
        "input_tokens".to_string(),
        AttributeValue {
            value: Some(Value::IntValue(input_tokens)),
        },
    );
    event.attributes.insert(
        "output_tokens".to_string(),
        AttributeValue {
            value: Some(Value::IntValue(output_tokens)),
        },
    );

    event
}
