use crate::helpers::{attr_keys, decode_payload, get_string_attr, get_thread_id, get_timestamp_us};
use crate::output::{print_info, print_success, print_warning};
use anyhow::{Context, Result};
use clap::Args;
use dashflow::constants::SHORT_TIMEOUT;
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::{dash_stream_message, Event, EventType};
use inferno::flamegraph::{self, Options};
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    Message,
};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};

/// Generate flamegraph for performance visualization
#[derive(Args)]
pub struct FlamegraphArgs {
    /// Kafka bootstrap servers (comma-separated)
    #[arg(short, long, default_value = "localhost:9092")]
    bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    #[arg(short, long, default_value = "dashstream-events")]
    topic: String,

    /// Thread ID to analyze
    #[arg(long = "thread", alias = "thread-id", required = true)]
    thread_id: String,

    /// Output file for flamegraph data
    #[arg(short, long, default_value = "flamegraph.txt")]
    output: String,

    /// Output format (folded, svg)
    #[arg(long, default_value = "folded")]
    format: String,
}

#[derive(Debug, Clone)]
struct StackFrame {
    name: String,
    start_time: i64,
    end_time: Option<i64>,
}

pub async fn run(args: FlamegraphArgs) -> Result<()> {
    print_info(&format!(
        "Generating flamegraph for thread '{}' from topic '{}'...",
        args.thread_id, args.topic
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-flamegraph-{}", uuid::Uuid::new_v4()),
        )
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false");
    let consumer: StreamConsumer = client_config
        .create()
        .context("Failed to create Kafka consumer")?;

    consumer
        .subscribe(&[&args.topic])
        .context("Failed to subscribe to topic")?;

    print_info("Reading events from Kafka...");

    // Collect all events for this thread
    let mut events = Vec::new();
    let timeout = SHORT_TIMEOUT;

    loop {
        match tokio::time::timeout(timeout, consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    if let Ok(msg) = decode_payload(payload) {
                        if let Some(dash_stream_message::Message::Event(event)) = msg.message {
                            if get_thread_id(&event) == Some(args.thread_id.as_str()) {
                                events.push(event);
                            }
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                print_warning(&format!("Error reading message: {e}"));
                break;
            }
            Err(_) => {
                // Timeout - we've reached the end
                break;
            }
        }
    }

    if events.is_empty() {
        anyhow::bail!("No events found for this thread");
    }

    print_success(&format!("Collected {} events", events.len()));

    // Build call stack from events
    let stack = build_call_stack(&events)?;

    // Generate flamegraph data
    let folded = generate_folded_stacks(&stack);

    // Write output
    match args.format.as_str() {
        "folded" => {
            tokio::fs::write(&args.output, folded.as_bytes())
                .await
                .context(format!("Failed to create output file: {}", args.output))?;

            print_success(&format!("Flamegraph data written to {}", args.output));
            println!("\nTo generate SVG flamegraph, run:");
            println!("  cat {} | flamegraph.pl > flamegraph.svg", args.output);
            println!("  or");
            println!("  inferno-flamegraph {} > flamegraph.svg", args.output);
        }
        "svg" => {
            // Generate SVG directly using inferno
            let mut svg_output = Vec::new();
            let mut options = Options::default();
            options.title = format!("DashFlow Flamegraph - Thread {}", args.thread_id);
            options.count_name = "microseconds".to_string();

            // Create reader from folded stack data
            let reader = BufReader::new(Cursor::new(folded.as_bytes()));

            flamegraph::from_reader(&mut options, reader, &mut svg_output)
                .context("Failed to generate SVG flamegraph")?;

            tokio::fs::write(&args.output, &svg_output)
                .await
                .context(format!("Failed to create output file: {}", args.output))?;

            print_success(&format!("SVG flamegraph written to {}", args.output));
            println!(
                "\nOpen {} in a browser to view the interactive flamegraph.",
                args.output
            );
        }
        _ => {
            anyhow::bail!("Unsupported format: {}", args.format);
        }
    }

    Ok(())
}

fn build_call_stack(events: &[Event]) -> Result<Vec<StackFrame>> {
    let mut stack: Vec<StackFrame> = Vec::new();
    let mut node_stack: Vec<(String, i64)> = Vec::new();

    for event in events {
        match event.event_type() {
            EventType::NodeStart => {
                if !event.node_id.is_empty() {
                    if let Some(ts) = get_timestamp_us(event) {
                        node_stack.push((event.node_id.clone(), ts));
                    }
                }
            }
            EventType::NodeEnd => {
                if !event.node_id.is_empty() {
                    // M-505: Log warnings for stack mismatches
                    if let Some((start_name, start_time)) = node_stack.pop() {
                        if start_name == event.node_id {
                            if let Some(end_ts) = get_timestamp_us(event) {
                                stack.push(StackFrame {
                                    name: event.node_id.clone(),
                                    start_time,
                                    end_time: Some(end_ts),
                                });
                            }
                        } else {
                            print_warning(&format!(
                                "Stack mismatch: NodeEnd for '{}' but popped '{}' - flamegraph may be inaccurate",
                                event.node_id, start_name
                            ));
                        }
                    } else {
                        print_warning(&format!(
                            "NodeEnd for '{}' without matching NodeStart - flamegraph may be inaccurate",
                            event.node_id
                        ));
                    }
                }
            }
            EventType::LlmStart => {
                if !event.node_id.is_empty() {
                    if let Some(ts) = get_timestamp_us(event) {
                        let name = format!("{}::LLM", event.node_id);
                        node_stack.push((name, ts));
                    }
                }
            }
            EventType::LlmEnd => {
                if !event.node_id.is_empty() {
                    let name = format!("{}::LLM", event.node_id);
                    // M-505: Log warnings for stack mismatches
                    if let Some((start_name, start_time)) = node_stack.pop() {
                        if start_name == name {
                            if let Some(end_ts) = get_timestamp_us(event) {
                                stack.push(StackFrame {
                                    name,
                                    start_time,
                                    end_time: Some(end_ts),
                                });
                            }
                        } else {
                            print_warning(&format!(
                                "Stack mismatch: LlmEnd for '{}' but popped '{}' - flamegraph may be inaccurate",
                                name, start_name
                            ));
                        }
                    } else {
                        print_warning(&format!(
                            "LlmEnd for '{}' without matching LlmStart - flamegraph may be inaccurate",
                            name
                        ));
                    }
                }
            }
            EventType::ToolStart => {
                if let Some(tool_name) = get_string_attr(event, attr_keys::TOOL_NAME) {
                    if let Some(ts) = get_timestamp_us(event) {
                        node_stack.push((format!("Tool::{tool_name}"), ts));
                    }
                }
            }
            EventType::ToolEnd => {
                if let Some(tool_name) = get_string_attr(event, attr_keys::TOOL_NAME) {
                    let name = format!("Tool::{tool_name}");
                    // M-505: Log warnings for stack mismatches
                    if let Some((start_name, start_time)) = node_stack.pop() {
                        if start_name == name {
                            if let Some(end_ts) = get_timestamp_us(event) {
                                stack.push(StackFrame {
                                    name,
                                    start_time,
                                    end_time: Some(end_ts),
                                });
                            }
                        } else {
                            print_warning(&format!(
                                "Stack mismatch: ToolEnd for '{}' but popped '{}' - flamegraph may be inaccurate",
                                name, start_name
                            ));
                        }
                    } else {
                        print_warning(&format!(
                            "ToolEnd for '{}' without matching ToolStart - flamegraph may be inaccurate",
                            name
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(stack)
}

fn generate_folded_stacks(frames: &[StackFrame]) -> String {
    let mut output = String::new();

    // Group frames by name and sum their durations
    let mut frame_durations: HashMap<String, i64> = HashMap::new();

    for frame in frames {
        if let Some(end_time) = frame.end_time {
            let duration = end_time - frame.start_time;
            *frame_durations.entry(frame.name.clone()).or_insert(0) += duration;
        }
    }

    // Generate folded stack format
    // Format: stack_trace sample_count
    // For DashFlow, we'll use: node_name duration_micros
    for (name, duration) in &frame_durations {
        output.push_str(&format!("{name} {duration}\n"));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow_streaming::{attribute_value, AttributeValue, Event, EventType, Header, MessageType};
    use std::collections::HashMap;

    fn event(event_type: EventType, node_id: &str, ts: i64) -> Event {
        Event {
            header: Some(Header {
                message_id: vec![0; 16],
                timestamp_us: ts,
                tenant_id: "t".to_string(),
                thread_id: "th".to_string(),
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
            llm_request_id: String::new(),
        }
    }

    fn tool_event(event_type: EventType, tool_name: &str, ts: i64) -> Event {
        let mut event = event(event_type, "", ts);
        event.attributes.insert(
            attr_keys::TOOL_NAME.to_string(),
            AttributeValue {
                value: Some(attribute_value::Value::StringValue(tool_name.to_string())),
            },
        );
        event
    }

    #[test]
    fn build_call_stack_emits_frames_for_nodes_llms_and_tools() {
        let events = vec![
            event(EventType::NodeStart, "A", 10),
            event(EventType::LlmStart, "A", 20),
            event(EventType::LlmEnd, "A", 30),
            tool_event(EventType::ToolStart, "search", 40),
            tool_event(EventType::ToolEnd, "search", 55),
            event(EventType::NodeEnd, "A", 100),
        ];

        let stack = build_call_stack(&events).expect("build_call_stack");
        let mut by_name: HashMap<String, (i64, i64)> = HashMap::new();
        for frame in stack {
            let end = frame.end_time.expect("frame should be ended");
            by_name.insert(frame.name, (frame.start_time, end));
        }

        assert_eq!(by_name.get("A"), Some(&(10, 100)));
        assert_eq!(by_name.get("A::LLM"), Some(&(20, 30)));
        assert_eq!(by_name.get("Tool::search"), Some(&(40, 55)));
    }

    #[test]
    fn generate_folded_stacks_sums_durations_by_name() {
        let frames = vec![
            StackFrame {
                name: "A".to_string(),
                start_time: 0,
                end_time: Some(10),
            },
            StackFrame {
                name: "A".to_string(),
                start_time: 20,
                end_time: Some(30),
            },
            StackFrame {
                name: "B".to_string(),
                start_time: 5,
                end_time: Some(8),
            },
        ];

        let folded = generate_folded_stacks(&frames);
        let mut totals: HashMap<&str, i64> = HashMap::new();
        for line in folded.lines() {
            let mut parts = line.split(' ');
            let name = parts.next().expect("name");
            let duration = parts
                .next()
                .expect("duration")
                .parse::<i64>()
                .expect("duration parse");
            totals.insert(name, duration);
        }

        assert_eq!(totals.get("A"), Some(&20));
        assert_eq!(totals.get("B"), Some(&3));
    }
}
