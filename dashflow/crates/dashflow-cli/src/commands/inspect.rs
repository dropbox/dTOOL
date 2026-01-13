use crate::helpers::{
    attr_keys, decode_payload, get_string_attr, get_tenant_id, get_thread_id, get_timestamp_us,
};
use crate::output::{
    create_table, format_duration, format_event_type, format_timestamp, print_info, print_success,
};
use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dashflow::constants::SHORT_TIMEOUT;
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::{dash_stream_message, Event, EventType};
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    Message,
};
use std::collections::HashMap;

/// Show thread details and execution history
#[derive(Args)]
pub struct InspectArgs {
    /// Kafka bootstrap servers (comma-separated)
    /// M-631: Support KAFKA_BROKERS env var for consistency
    #[arg(short, long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    /// M-631: Support KAFKA_TOPIC env var for consistency
    #[arg(short, long, env = "KAFKA_TOPIC", default_value = "dashstream-events")]
    topic: String,

    /// Thread ID to inspect
    #[arg(long = "thread", alias = "thread-id", required = true)]
    thread_id: String,

    /// Show detailed event log
    #[arg(long)]
    detailed: bool,

    /// Show node execution statistics
    #[arg(long)]
    stats: bool,
}

pub async fn run(args: InspectArgs) -> Result<()> {
    print_info(&format!(
        "Inspecting thread '{}' on topic '{}'...",
        args.thread_id, args.topic
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-inspect-{}", uuid::Uuid::new_v4()),
        )
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false");
    let consumer: StreamConsumer = client_config
        .create()
        .context("Failed to create Kafka consumer")?;

    // Subscribe and seek to beginning
    consumer
        .subscribe(&[&args.topic])
        .context("Failed to subscribe to topic")?;

    // Collect all events for this thread
    let mut events = Vec::new();
    let timeout = SHORT_TIMEOUT;

    print_info("Reading events from Kafka...");

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
                eprintln!("Error reading message: {e}");
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

    print_success(&format!("Found {} events", events.len()));
    println!();

    // Display thread summary
    display_thread_summary(&events, &args.thread_id);

    // Display detailed event log if requested
    if args.detailed {
        println!();
        display_event_log(&events);
    }

    // Display node statistics if requested
    if args.stats {
        println!();
        display_node_stats(&events);
    }

    Ok(())
}

fn display_thread_summary(events: &[Event], thread_id: &str) {
    println!("{}", "Thread Summary".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec!["Property", "Value"]);

    // Thread ID
    table.add_row(vec!["Thread ID", thread_id]);

    // Tenant ID (from first event)
    if let Some(first) = events.first() {
        if let Some(tenant_id) = get_tenant_id(first) {
            table.add_row(vec!["Tenant ID", tenant_id]);
        }
    }

    // Time range
    if let (Some(first), Some(last)) = (events.first(), events.last()) {
        if let (Some(first_ts), Some(last_ts)) = (get_timestamp_us(first), get_timestamp_us(last)) {
            table.add_row(vec!["Start Time", &format_timestamp(first_ts)]);
            table.add_row(vec!["End Time", &format_timestamp(last_ts)]);

            let duration = last_ts - first_ts;
            table.add_row(vec!["Duration", &format_duration(duration)]);
        }
    }

    // Event count
    table.add_row(vec!["Total Events", &events.len().to_string()]);

    // Event type breakdown
    let mut type_counts: HashMap<EventType, usize> = HashMap::new();
    for event in events {
        *type_counts.entry(event.event_type()).or_insert(0) += 1;
    }

    let mut type_summary = String::new();
    for (event_type, count) in &type_counts {
        if !type_summary.is_empty() {
            type_summary.push_str(", ");
        }
        type_summary.push_str(&format!("{event_type:?}: {count}"));
    }
    table.add_row(vec!["Event Types", &type_summary]);

    println!("{table}");
}

fn display_event_log(events: &[Event]) {
    println!("{}", "Event Log".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec!["#", "Timestamp", "Type", "Details"]);

    for (i, event) in events.iter().enumerate() {
        let timestamp = get_timestamp_us(event).map_or_else(|| "N/A".to_string(), format_timestamp);
        let event_type = format_event_type(event.event_type());
        let details = format_event_details(event);

        table.add_row(vec![(i + 1).to_string(), timestamp, event_type, details]);
    }

    println!("{table}");
}

fn display_node_stats(events: &[Event]) {
    println!("{}", "Node Execution Statistics".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    // Track node execution times
    let mut node_starts: HashMap<String, i64> = HashMap::new();
    let mut node_durations: HashMap<String, Vec<i64>> = HashMap::new();
    let mut node_counts: HashMap<String, usize> = HashMap::new();
    let mut node_errors: HashMap<String, usize> = HashMap::new();

    for event in events {
        let node_id = &event.node_id;
        if !node_id.is_empty() {
            if let Some(timestamp) = get_timestamp_us(event) {
                match event.event_type() {
                    EventType::NodeStart => {
                        node_starts.insert(node_id.clone(), timestamp);
                    }
                    EventType::NodeEnd => {
                        if let Some(start_time) = node_starts.remove(node_id) {
                            let duration = timestamp - start_time;
                            node_durations
                                .entry(node_id.clone())
                                .or_default()
                                .push(duration);
                        }
                        *node_counts.entry(node_id.clone()).or_insert(0) += 1;
                    }
                    EventType::NodeError => {
                        *node_errors.entry(node_id.clone()).or_insert(0) += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    let mut table = create_table();
    table.set_header(vec![
        "Node",
        "Executions",
        "Avg Duration",
        "Total Duration",
        "Errors",
    ]);

    for (node_name, count) in &node_counts {
        let empty_vec = Vec::new();
        let durations = node_durations.get(node_name).unwrap_or(&empty_vec);
        let total_duration: i64 = durations.iter().sum();
        let avg_duration = if durations.is_empty() {
            0
        } else {
            total_duration / durations.len() as i64
        };
        let errors = node_errors.get(node_name).unwrap_or(&0);

        table.add_row(vec![
            node_name.clone(),
            count.to_string(),
            format_duration(avg_duration),
            format_duration(total_duration),
            errors.to_string(),
        ]);
    }

    println!("{table}");
}

fn format_event_details(event: &Event) -> String {
    if !event.node_id.is_empty() {
        event.node_id.clone()
    } else if let Some(from) = get_string_attr(event, attr_keys::EDGE_FROM) {
        if let Some(to) = get_string_attr(event, attr_keys::EDGE_TO) {
            format!("{from} → {to}")
        } else {
            from
        }
    } else if let Some(tool) = get_string_attr(event, attr_keys::TOOL_NAME) {
        format!("tool: {tool}")
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        inspect: InspectArgs,
    }

    #[test]
    fn test_inspect_args_required_thread_id() {
        let cli = Cli::parse_from(["test", "--thread-id", "thread-abc"]);

        assert_eq!(cli.inspect.thread_id, "thread-abc");
        assert_eq!(cli.inspect.bootstrap_servers, "localhost:9092");
        assert_eq!(cli.inspect.topic, "dashstream-events");
        assert!(!cli.inspect.detailed);
        assert!(!cli.inspect.stats);
    }

    #[test]
    fn test_inspect_args_with_options() {
        let cli = Cli::parse_from([
            "test",
            "--thread-id", "t1",
            "--detailed",
            "--stats",
        ]);

        assert_eq!(cli.inspect.thread_id, "t1");
        assert!(cli.inspect.detailed);
        assert!(cli.inspect.stats);
    }

    #[test]
    fn test_inspect_args_custom_kafka() {
        let cli = Cli::parse_from([
            "test",
            "--thread-id", "t1",
            "--bootstrap-servers", "kafka:9093",
            "--topic", "my-topic",
        ]);

        assert_eq!(cli.inspect.bootstrap_servers, "kafka:9093");
        assert_eq!(cli.inspect.topic, "my-topic");
    }

    #[test]
    fn test_format_event_details_node_id() {
        let event = Event {
            node_id: "my-node".to_string(),
            ..Default::default()
        };
        assert_eq!(format_event_details(&event), "my-node");
    }

    #[test]
    fn test_format_event_details_empty() {
        let event = Event::default();
        assert_eq!(format_event_details(&event), "");
    }
}
