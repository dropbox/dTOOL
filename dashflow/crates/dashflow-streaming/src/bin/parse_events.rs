#!/usr/bin/env cargo
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// parse_events - Decode and display DashFlow Streaming protobuf events from Kafka

//! # `DashFlow Streaming` Event Parser
//!
//! Command-line tool to consume and decode DashFlow Streaming protobuf events from Kafka,
//! outputting human-readable JSON for analysis and debugging.
//!
//! ## Usage
//!
//! ```bash
//! # View all events from the beginning (default topic: dashstream_events)
//! cargo run --bin parse_events
//!
//! # Specify custom topic
//! cargo run --bin parse_events -- --topic my_topic
//!
//! # Tail mode: only show new events
//! cargo run --bin parse_events -- --tail
//!
//! # With custom broker
//! cargo run --bin parse_events -- --broker localhost:9092
//!
//! # Limit number of messages
//! cargo run --bin parse_events -- --limit 10
//! ```

use dashflow_streaming::{
    consumer::DashStreamConsumer, dash_stream_message::Message, EventType, MessageType,
};
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

const DEFAULT_BROKER: &str = "localhost:9092";
const DEFAULT_TOPIC: &str = "dashstream_events";
const DEFAULT_GROUP: &str = "parse-events-cli";

/// Convert `EventType` enum to string
fn event_type_to_string(event_type: i32) -> String {
    match EventType::try_from(event_type) {
        Ok(EventType::GraphStart) => "GRAPH_START".to_string(),
        Ok(EventType::GraphEnd) => "GRAPH_END".to_string(),
        Ok(EventType::GraphError) => "GRAPH_ERROR".to_string(),
        Ok(EventType::NodeStart) => "NODE_START".to_string(),
        Ok(EventType::NodeEnd) => "NODE_END".to_string(),
        Ok(EventType::NodeError) => "NODE_ERROR".to_string(),
        Ok(EventType::EdgeTraversal) => "EDGE_TRAVERSAL".to_string(),
        Ok(EventType::ConditionalBranch) => "CONDITIONAL_BRANCH".to_string(),
        Ok(EventType::ParallelStart) => "PARALLEL_START".to_string(),
        Ok(EventType::ParallelEnd) => "PARALLEL_END".to_string(),
        Ok(EventType::LlmStart) => "LLM_START".to_string(),
        Ok(EventType::LlmEnd) => "LLM_END".to_string(),
        Ok(EventType::LlmError) => "LLM_ERROR".to_string(),
        Ok(EventType::LlmRetry) => "LLM_RETRY".to_string(),
        Ok(EventType::ToolStart) => "TOOL_START".to_string(),
        Ok(EventType::ToolEnd) => "TOOL_END".to_string(),
        Ok(EventType::ToolError) => "TOOL_ERROR".to_string(),
        Ok(EventType::CheckpointSave) => "CHECKPOINT_SAVE".to_string(),
        Ok(EventType::CheckpointLoad) => "CHECKPOINT_LOAD".to_string(),
        Ok(EventType::CheckpointDelete) => "CHECKPOINT_DELETE".to_string(),
        Ok(EventType::MemorySave) => "MEMORY_SAVE".to_string(),
        Ok(EventType::MemoryLoad) => "MEMORY_LOAD".to_string(),
        Ok(EventType::HumanInterrupt) => "HUMAN_INTERRUPT".to_string(),
        Ok(EventType::HumanResume) => "HUMAN_RESUME".to_string(),
        _ => format!("UNKNOWN({event_type})"),
    }
}

/// Convert `MessageType` enum to string
fn message_type_to_string(message_type: i32) -> String {
    match MessageType::try_from(message_type) {
        Ok(MessageType::Event) => "EVENT".to_string(),
        Ok(MessageType::StateDiff) => "STATE_DIFF".to_string(),
        Ok(MessageType::TokenChunk) => "TOKEN_CHUNK".to_string(),
        Ok(MessageType::ToolExecution) => "TOOL_EXECUTION".to_string(),
        Ok(MessageType::Checkpoint) => "CHECKPOINT".to_string(),
        Ok(MessageType::Metrics) => "METRICS".to_string(),
        Ok(MessageType::Error) => "ERROR".to_string(),
        Ok(MessageType::EventBatch) => "EVENT_BATCH".to_string(),
        _ => format!("UNKNOWN({message_type})"),
    }
}

/// Convert protobuf `AttributeValue` to JSON
fn attribute_value_to_json(attr: &dashflow_streaming::AttributeValue) -> Value {
    use dashflow_streaming::attribute_value::Value as AttrValue;

    match &attr.value {
        Some(AttrValue::StringValue(s)) => json!(s),
        Some(AttrValue::IntValue(i)) => json!(i),
        Some(AttrValue::FloatValue(f)) => json!(f),
        Some(AttrValue::BoolValue(b)) => json!(b),
        Some(AttrValue::BytesValue(b)) => {
            // Try to decode as UTF-8, otherwise hex
            match String::from_utf8(b.clone()) {
                Ok(s) => json!(s),
                Err(_) => json!(hex::encode(b)),
            }
        }
        Some(AttrValue::ArrayValue(arr)) => {
            let values: Vec<Value> = arr.values.iter().map(attribute_value_to_json).collect();
            json!(values)
        }
        Some(AttrValue::MapValue(map)) => {
            let obj: serde_json::Map<String, Value> = map
                .values
                .iter()
                .map(|(k, v)| (k.clone(), attribute_value_to_json(v)))
                .collect();
            json!(obj)
        }
        None => json!(null),
    }
}

/// Convert protobuf Header to JSON
fn header_to_json(header: &dashflow_streaming::Header) -> Value {
    json!({
        "message_id": uuid::Uuid::from_slice(&header.message_id).map_or_else(|_| hex::encode(&header.message_id), |u| u.to_string()),
        "timestamp_us": header.timestamp_us,
        "timestamp_iso": chrono::DateTime::from_timestamp_micros(header.timestamp_us).map_or_else(|| "invalid".to_string(), |dt| dt.to_rfc3339()),
        "tenant_id": header.tenant_id,
        "thread_id": header.thread_id,
        "sequence": header.sequence,
        "type": message_type_to_string(header.r#type),
        "parent_id": if header.parent_id.is_empty() {
            json!(null)
        } else {
            json!(hex::encode(&header.parent_id))
        },
        "schema_version": header.schema_version,
    })
}

/// Convert Event to JSON
fn event_to_json(event: &dashflow_streaming::Event) -> Value {
    let header_json = event
        .header
        .as_ref()
        .map(header_to_json)
        .unwrap_or(json!(null));

    let attributes_json: serde_json::Map<String, Value> = event
        .attributes
        .iter()
        .map(|(k, v)| (k.clone(), attribute_value_to_json(v)))
        .collect();

    json!({
        "type": "Event",
        "header": header_json,
        "event_type": event_type_to_string(event.event_type),
        "node_id": if event.node_id.is_empty() { json!(null) } else { json!(&event.node_id) },
        "attributes": if attributes_json.is_empty() { json!(null) } else { json!(attributes_json) },
        "duration_us": if event.duration_us > 0 { json!(event.duration_us) } else { json!(null) },
        "duration_ms": if event.duration_us > 0 { json!(event.duration_us as f64 / 1000.0) } else { json!(null) },
        "llm_request_id": if event.llm_request_id.is_empty() { json!(null) } else { json!(&event.llm_request_id) },
    })
}

/// Convert `TokenChunk` to JSON
fn token_chunk_to_json(chunk: &dashflow_streaming::TokenChunk) -> Value {
    let header_json = chunk
        .header
        .as_ref()
        .map(header_to_json)
        .unwrap_or(json!(null));

    json!({
        "type": "TokenChunk",
        "header": header_json,
        "request_id": &chunk.request_id,
        "text": &chunk.text,
        "chunk_index": chunk.chunk_index,
        "is_final": chunk.is_final,
        "model": if chunk.model.is_empty() { json!(null) } else { json!(&chunk.model) },
        "stats": chunk.stats.as_ref().map(|s| json!({
            "prompt_tokens": s.prompt_tokens,
            "completion_tokens": s.completion_tokens,
            "total_tokens": s.total_tokens,
        })),
    })
}

/// Convert `StateDiff` to JSON
fn state_diff_to_json(diff: &dashflow_streaming::StateDiff) -> Value {
    let header_json = diff
        .header
        .as_ref()
        .map(header_to_json)
        .unwrap_or(json!(null));

    json!({
        "type": "StateDiff",
        "header": header_json,
        "base_checkpoint_id": if diff.base_checkpoint_id.is_empty() {
            json!(null)
        } else {
            json!(hex::encode(&diff.base_checkpoint_id))
        },
        "operations_count": diff.operations.len(),
        "state_hash": if diff.state_hash.is_empty() {
            json!(null)
        } else {
            json!(hex::encode(&diff.state_hash))
        },
    })
}

/// Convert `ToolExecution` to JSON
fn tool_execution_to_json(tool_exec: &dashflow_streaming::ToolExecution) -> Value {
    use dashflow_streaming::tool_execution::ExecutionStage;

    let header_json = tool_exec
        .header
        .as_ref()
        .map(header_to_json)
        .unwrap_or(json!(null));

    let stage_str = match ExecutionStage::try_from(tool_exec.stage) {
        Ok(ExecutionStage::Requested) => "REQUESTED",
        Ok(ExecutionStage::Started) => "STARTED",
        Ok(ExecutionStage::Completed) => "COMPLETED",
        Ok(ExecutionStage::Failed) => "FAILED",
        Ok(ExecutionStage::Retrying) => "RETRYING",
        _ => "UNKNOWN",
    };

    // Try to parse arguments as JSON
    let arguments_json = if tool_exec.arguments.is_empty() {
        json!(null)
    } else {
        serde_json::from_slice::<Value>(&tool_exec.arguments)
            .unwrap_or_else(|_| json!(hex::encode(&tool_exec.arguments)))
    };

    // Try to parse result as JSON
    let result_json = if tool_exec.result.is_empty() {
        json!(null)
    } else {
        serde_json::from_slice::<Value>(&tool_exec.result)
            .unwrap_or_else(|_| json!(hex::encode(&tool_exec.result)))
    };

    json!({
        "type": "ToolExecution",
        "header": header_json,
        "call_id": &tool_exec.call_id,
        "tool_name": &tool_exec.tool_name,
        "stage": stage_str,
        "arguments": arguments_json,
        "result": result_json,
        "error": if tool_exec.error.is_empty() { json!(null) } else { json!(&tool_exec.error) },
        "duration_us": if tool_exec.duration_us > 0 { json!(tool_exec.duration_us) } else { json!(null) },
        "duration_ms": if tool_exec.duration_us > 0 { json!(tool_exec.duration_us as f64 / 1000.0) } else { json!(null) },
        "retry_count": tool_exec.retry_count,
    })
}

/// Convert Metrics to JSON
fn metrics_to_json(metrics: &dashflow_streaming::Metrics) -> Value {
    let header_json = metrics
        .header
        .as_ref()
        .map(header_to_json)
        .unwrap_or(json!(null));

    let metrics_map: serde_json::Map<String, Value> = metrics
        .metrics
        .iter()
        .map(|(k, v)| {
            use dashflow_streaming::metric_value::Value as MetricVal;
            let value = match &v.value {
                Some(MetricVal::IntValue(i)) => json!(i),
                Some(MetricVal::FloatValue(f)) => json!(f),
                Some(MetricVal::StringValue(s)) => json!(s),
                Some(MetricVal::BoolValue(b)) => json!(b),
                Some(MetricVal::BytesValue(b)) => json!(hex::encode(b)),
                None => json!(null),
            };
            (k.clone(), value)
        })
        .collect();

    json!({
        "type": "Metrics",
        "header": header_json,
        "scope": &metrics.scope,
        "scope_id": &metrics.scope_id,
        "metrics": metrics_map,
        "tags": if metrics.tags.is_empty() { json!(null) } else { json!(&metrics.tags) },
    })
}

/// Convert Error to JSON
fn error_to_json(error: &dashflow_streaming::Error) -> Value {
    use dashflow_streaming::error::Severity;

    let header_json = error
        .header
        .as_ref()
        .map(header_to_json)
        .unwrap_or(json!(null));

    let severity_str = match Severity::try_from(error.severity) {
        Ok(Severity::Debug) => "DEBUG",
        Ok(Severity::Info) => "INFO",
        Ok(Severity::Warning) => "WARNING",
        Ok(Severity::Error) => "ERROR",
        Ok(Severity::Fatal) => "FATAL",
        _ => "UNKNOWN",
    };

    json!({
        "type": "Error",
        "header": header_json,
        "error_code": &error.error_code,
        "message": &error.message,
        "severity": severity_str,
        "exception_type": if error.exception_type.is_empty() { json!(null) } else { json!(&error.exception_type) },
        "context": if error.context.is_empty() { json!(null) } else { json!(&error.context) },
        "suggestions": if error.suggestions.is_empty() { json!(null) } else { json!(&error.suggestions) },
    })
}

/// Convert `DashStreamMessage` to JSON
fn message_to_json(msg: &dashflow_streaming::DashStreamMessage) -> Value {
    match &msg.message {
        Some(Message::Event(event)) => event_to_json(event),
        Some(Message::TokenChunk(chunk)) => token_chunk_to_json(chunk),
        Some(Message::StateDiff(diff)) => state_diff_to_json(diff),
        Some(Message::ToolExecution(tool_exec)) => tool_execution_to_json(tool_exec),
        Some(Message::Metrics(metrics)) => metrics_to_json(metrics),
        Some(Message::Error(error)) => error_to_json(error),
        Some(Message::Checkpoint(checkpoint)) => {
            let header_json = checkpoint
                .header
                .as_ref()
                .map(header_to_json)
                .unwrap_or(json!(null));
            json!({
                "type": "Checkpoint",
                "header": header_json,
                "checkpoint_id": hex::encode(&checkpoint.checkpoint_id),
                "state_type": &checkpoint.state_type,
                "state_size_bytes": checkpoint.state.len(),
            })
        }
        Some(Message::EventBatch(batch)) => {
            let header_json = batch
                .header
                .as_ref()
                .map(header_to_json)
                .unwrap_or(json!(null));
            json!({
                "type": "EventBatch",
                "header": header_json,
                "event_count": batch.events.len(),
            })
        }
        Some(Message::ExecutionTrace(trace)) => {
            let header_json = trace
                .header
                .as_ref()
                .map(header_to_json)
                .unwrap_or(json!(null));
            json!({
                "type": "ExecutionTrace",
                "header": header_json,
                "execution_id": &trace.execution_id,
                "thread_id": &trace.thread_id,
                "total_duration_ms": trace.total_duration_ms,
                "total_tokens": trace.total_tokens,
                "completed": trace.completed,
                "nodes_count": trace.nodes_executed.len(),
                "errors_count": trace.errors.len(),
            })
        }
        None => json!({"type": "Empty"}),
    }
}

#[derive(Debug)]
struct Config {
    broker: String,
    topic: String,
    group: String,
    tail: bool,
    limit: Option<usize>,
}

fn parse_args() -> Config {
    let args: Vec<String> = env::args().collect();
    let mut config = Config {
        broker: DEFAULT_BROKER.to_string(),
        topic: DEFAULT_TOPIC.to_string(),
        group: DEFAULT_GROUP.to_string(),
        tail: false,
        limit: None,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--broker" | "-b" => {
                if i + 1 < args.len() {
                    config.broker = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --broker requires a value");
                    std::process::exit(1);
                }
            }
            "--topic" | "-t" => {
                if i + 1 < args.len() {
                    config.topic = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --topic requires a value");
                    std::process::exit(1);
                }
            }
            "--group" | "-g" => {
                if i + 1 < args.len() {
                    config.group = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --group requires a value");
                    std::process::exit(1);
                }
            }
            "--tail" => {
                config.tail = true;
                i += 1;
            }
            "--limit" | "-n" => {
                if i + 1 < args.len() {
                    if let Ok(n) = args[i + 1].parse::<usize>() {
                        config.limit = Some(n);
                        i += 2;
                    } else {
                        eprintln!("Error: --limit requires a number");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Error: --limit requires a value");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                print_help(&args[0]);
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {other}");
                print_help(&args[0]);
                std::process::exit(1);
            }
        }
    }

    config
}

fn print_help(program: &str) {
    eprintln!("DashFlow Streaming Event Parser - Decode protobuf events from Kafka");
    eprintln!();
    eprintln!("Usage: {program} [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -b, --broker <HOST:PORT>   Kafka broker (default: {DEFAULT_BROKER})");
    eprintln!("  -t, --topic <TOPIC>        Kafka topic (default: {DEFAULT_TOPIC})");
    eprintln!("  -g, --group <GROUP>        Consumer group (default: {DEFAULT_GROUP})");
    eprintln!("  --tail                     Start from latest (default: from beginning)");
    eprintln!("  -n, --limit <N>            Limit messages to N");
    eprintln!("  -h, --help                 Show this help");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {program} --topic dashstream_events");
    eprintln!("  {program} --tail --limit 10");
    eprintln!("  {program} --broker localhost:9092");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = parse_args();

    eprintln!("📡 DashFlow Streaming Event Parser");
    eprintln!("==========================");
    eprintln!("Broker: {}", config.broker);
    eprintln!("Topic: {}", config.topic);
    eprintln!("Group: {}", config.group);
    eprintln!(
        "Mode: {}",
        if config.tail {
            "tail"
        } else {
            "from beginning"
        }
    );
    if let Some(limit) = config.limit {
        eprintln!("Limit: {limit} messages");
    }
    eprintln!();

    // Create consumer
    let mut consumer =
        DashStreamConsumer::new(&config.broker, &config.topic, &config.group).await?;

    eprintln!("✅ Connected. Listening for messages...");
    eprintln!();

    let mut count = 0;
    loop {
        if let Some(limit) = config.limit {
            if count >= limit {
                eprintln!("\n📊 Reached limit of {limit} messages");
                break;
            }
        }

        match consumer.next_timeout(Duration::from_secs(30)).await {
            Some(Ok(msg)) => {
                count += 1;
                let json = message_to_json(&msg);
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            Some(Err(e)) => {
                eprintln!("❌ Error decoding message: {e}");
            }
            None => {
                eprintln!("\n⏱️  Timeout - no messages in 30s");
                break;
            }
        }
    }

    eprintln!("\n✅ Processed {count} messages");
    Ok(())
}
