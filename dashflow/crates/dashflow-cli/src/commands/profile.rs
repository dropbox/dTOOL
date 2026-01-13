use crate::helpers::{attr_keys, decode_payload, get_string_attr, get_thread_id, get_timestamp_us};
use crate::output::{create_table, format_duration, print_info, print_success};
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

/// Profile execution performance
#[derive(Args)]
pub struct ProfileArgs {
    /// Kafka bootstrap servers (comma-separated)
    #[arg(short, long, default_value = "localhost:9092")]
    bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    #[arg(short, long, default_value = "dashstream-events")]
    topic: String,

    /// Thread ID to profile
    #[arg(long = "thread", alias = "thread-id", required = true)]
    thread_id: String,

    /// Show detailed breakdown
    #[arg(long)]
    detailed: bool,

    /// Show slowest operations
    #[arg(long, default_value = "10")]
    top: usize,
}

#[derive(Debug)]
struct NodeProfile {
    executions: usize,
    total_duration: i64,
    min_duration: i64,
    max_duration: i64,
    durations: Vec<i64>,
}

impl NodeProfile {
    fn new() -> Self {
        Self {
            executions: 0,
            total_duration: 0,
            min_duration: i64::MAX,
            max_duration: 0,
            durations: Vec::new(),
        }
    }

    fn add_execution(&mut self, duration: i64) {
        self.executions += 1;
        self.total_duration += duration;
        self.min_duration = self.min_duration.min(duration);
        self.max_duration = self.max_duration.max(duration);
        self.durations.push(duration);
    }

    fn avg_duration(&self) -> i64 {
        if self.executions > 0 {
            self.total_duration / self.executions as i64
        } else {
            0
        }
    }

    fn median_duration(&self) -> i64 {
        if self.durations.is_empty() {
            return 0;
        }

        let mut sorted = self.durations.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    fn p95_duration(&self) -> i64 {
        if self.durations.is_empty() {
            return 0;
        }

        let mut sorted = self.durations.clone();
        sorted.sort_unstable();
        let idx = (sorted.len() as f64 * 0.95) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

pub async fn run(args: ProfileArgs) -> Result<()> {
    print_info(&format!(
        "Profiling thread '{}' from topic '{}'...",
        args.thread_id, args.topic
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-profile-{}", uuid::Uuid::new_v4()),
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

    print_success(&format!("Collected {} events", events.len()));
    println!();

    // Build performance profile
    let profile = build_profile(&events);

    // Display overall statistics
    display_overall_stats(&events);

    // Display node performance
    println!();
    display_node_performance(&profile, args.top, args.detailed);

    // Display edge traversal times
    println!();
    display_edge_performance(&events);

    Ok(())
}

fn build_profile(events: &[Event]) -> HashMap<String, NodeProfile> {
    let mut profiles: HashMap<String, NodeProfile> = HashMap::new();
    let mut node_starts: HashMap<String, i64> = HashMap::new();

    for event in events {
        if !event.node_id.is_empty() {
            match event.event_type() {
                EventType::NodeStart => {
                    if let Some(ts) = get_timestamp_us(event) {
                        node_starts.insert(event.node_id.clone(), ts);
                    }
                }
                EventType::NodeEnd => {
                    if let Some(start_time) = node_starts.remove(&event.node_id) {
                        if let Some(end_ts) = get_timestamp_us(event) {
                            let duration = end_ts - start_time;
                            profiles
                                .entry(event.node_id.clone())
                                .or_insert_with(NodeProfile::new)
                                .add_execution(duration);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    profiles
}

fn display_overall_stats(events: &[Event]) {
    println!("{}", "Overall Performance".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let start_time = events.first().and_then(get_timestamp_us).unwrap_or(0);
    let end_time = events.last().and_then(get_timestamp_us).unwrap_or(0);
    let total_duration = end_time - start_time;

    let mut table = create_table();
    table.set_header(vec!["Metric", "Value"]);

    table.add_row(vec!["Total Events", &events.len().to_string()]);
    table.add_row(vec!["Total Duration", &format_duration(total_duration)]);

    // Count event types
    let _node_starts = events
        .iter()
        .filter(|e| e.event_type() == EventType::NodeStart)
        .count();
    let node_ends = events
        .iter()
        .filter(|e| e.event_type() == EventType::NodeEnd)
        .count();
    let edges = events
        .iter()
        .filter(|e| e.event_type() == EventType::EdgeTraversal)
        .count();
    let errors = events
        .iter()
        .filter(|e| e.event_type() == EventType::NodeError)
        .count();

    table.add_row(vec!["Nodes Executed", &node_ends.to_string()]);
    table.add_row(vec!["Edges Traversed", &edges.to_string()]);
    table.add_row(vec!["Errors", &errors.to_string()]);

    println!("{table}");
}

fn display_node_performance(profiles: &HashMap<String, NodeProfile>, top: usize, detailed: bool) {
    println!("{}", "Node Performance".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    // Sort by total duration (descending)
    let mut profile_vec: Vec<_> = profiles.iter().collect();
    profile_vec.sort_by(|a, b| b.1.total_duration.cmp(&a.1.total_duration));

    let mut table = create_table();

    if detailed {
        table.set_header(vec![
            "Node",
            "Executions",
            "Total",
            "Avg",
            "Median",
            "P95",
            "Min",
            "Max",
        ]);

        for (node_name, profile) in profile_vec.iter().take(top) {
            table.add_row(vec![
                (*node_name).clone(),
                profile.executions.to_string(),
                format_duration(profile.total_duration),
                format_duration(profile.avg_duration()),
                format_duration(profile.median_duration()),
                format_duration(profile.p95_duration()),
                format_duration(profile.min_duration),
                format_duration(profile.max_duration),
            ]);
        }
    } else {
        table.set_header(vec!["Node", "Executions", "Total", "Avg", "% of Total"]);

        let total_time: i64 = profile_vec.iter().map(|(_, p)| p.total_duration).sum();

        for (node_name, profile) in profile_vec.iter().take(top) {
            let percentage = if total_time > 0 {
                (profile.total_duration as f64 / total_time as f64) * 100.0
            } else {
                0.0
            };

            table.add_row(vec![
                (*node_name).clone(),
                profile.executions.to_string(),
                format_duration(profile.total_duration),
                format_duration(profile.avg_duration()),
                format!("{:.1}%", percentage),
            ]);
        }
    }

    println!("{table}");
}

fn display_edge_performance(events: &[Event]) {
    let mut edge_counts: HashMap<(String, String), usize> = HashMap::new();

    for event in events {
        if event.event_type() == EventType::EdgeTraversal {
            if let (Some(from), Some(to)) = (
                get_string_attr(event, attr_keys::EDGE_FROM),
                get_string_attr(event, attr_keys::EDGE_TO),
            ) {
                *edge_counts.entry((from, to)).or_insert(0) += 1;
            }
        }
    }

    if edge_counts.is_empty() {
        return;
    }

    println!("{}", "Edge Traversals".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec!["From", "To", "Traversals"]);

    // Sort by traversal count (descending)
    let mut edge_vec: Vec<_> = edge_counts.iter().collect();
    edge_vec.sort_by(|a, b| b.1.cmp(a.1));

    for ((from, to), count) in edge_vec.iter().take(10) {
        table.add_row(vec![from.clone(), to.clone(), count.to_string()]);
    }

    println!("{table}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow_streaming::{Event, EventType, Header, MessageType};
    use std::collections::HashMap;

    fn event(node_id: &str, event_type: EventType, ts: i64) -> Event {
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

    #[test]
    fn node_profile_stats_work() {
        let mut profile = NodeProfile::new();
        assert_eq!(profile.avg_duration(), 0);
        assert_eq!(profile.median_duration(), 0);
        assert_eq!(profile.p95_duration(), 0);

        for d in [10, 20, 30, 40, 50] {
            profile.add_execution(d);
        }
        assert_eq!(profile.executions, 5);
        assert_eq!(profile.total_duration, 150);
        assert_eq!(profile.min_duration, 10);
        assert_eq!(profile.max_duration, 50);
        assert_eq!(profile.avg_duration(), 30);
        assert_eq!(profile.median_duration(), 30);
        assert_eq!(profile.p95_duration(), 50);
    }

    #[test]
    fn build_profile_pairs_start_and_end_by_node_id() {
        let events = vec![
            event("A", EventType::NodeStart, 1_000),
            event("B", EventType::NodeStart, 2_000),
            event("A", EventType::NodeEnd, 3_500), // duration 2_500
            event("A", EventType::NodeEnd, 4_000), // ignored (no matching start)
            event("B", EventType::NodeEnd, 6_000), // duration 4_000
        ];

        let profile = build_profile(&events);
        let a = profile.get("A").expect("node A profile");
        assert_eq!(a.executions, 1);
        assert_eq!(a.total_duration, 2_500);
        assert_eq!(a.min_duration, 2_500);
        assert_eq!(a.max_duration, 2_500);

        let b = profile.get("B").expect("node B profile");
        assert_eq!(b.executions, 1);
        assert_eq!(b.total_duration, 4_000);
    }
}
