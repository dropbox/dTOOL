use crate::helpers::{attr_keys, decode_payload, get_string_attr, get_thread_id, get_timestamp_us};
use crate::output::{format_timestamp, print_event, print_info, print_success};
use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dashflow::constants::SHORT_TIMEOUT;
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::{dash_stream_message, EventType};
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    Message,
};

/// Replay execution from a checkpoint (time-travel debugging)
///
/// NOTE: This command is deprecated in favor of `dashflow timeline replay`.
/// The replay command will continue to work but new users should use timeline.
/// M-38: The `--thread-id` flag is deprecated; use `--thread` instead.
#[derive(Args)]
pub struct ReplayArgs {
    /// Kafka bootstrap servers (comma-separated).
    /// Can also be set via KAFKA_BROKERS environment variable.
    #[arg(short, long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    pub bootstrap_servers: String,

    /// Kafka topic to consume from.
    /// Can also be set via KAFKA_TOPIC environment variable.
    /// M-433: Default matches library default (dashstream-events)
    #[arg(short, long, env = "KAFKA_TOPIC", default_value = "dashstream-events")]
    pub topic: String,

    /// Thread ID to replay (use --thread for consistency with timeline commands)
    #[arg(long = "thread", alias = "thread-id", required = true)]
    pub thread_id: String,

    /// Start from timestamp (RFC3339 format or Unix micros)
    #[arg(long)]
    pub from_timestamp: Option<String>,

    /// Stop at timestamp (RFC3339 format or Unix micros)
    #[arg(long)]
    pub to_timestamp: Option<String>,

    /// Start from checkpoint ID
    #[arg(long)]
    pub from_checkpoint: Option<String>,

    /// Playback speed multiplier (1.0 = real-time, 2.0 = 2x speed, 0 = instant)
    #[arg(long, default_value = "0")]
    pub speed: f64,

    /// Show only specific event types (comma-separated)
    #[arg(long)]
    pub events: Option<String>,

    /// Pause on errors
    #[arg(long)]
    pub pause_on_error: bool,
}

pub async fn run(args: ReplayArgs) -> Result<()> {
    print_info(&format!(
        "Replaying thread '{}' from topic '{}'...",
        args.thread_id, args.topic
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-replay-{}", uuid::Uuid::new_v4()),
        )
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false");
    let consumer: StreamConsumer = client_config
        .create()
        .context("Failed to create Kafka consumer")?;

    consumer
        .subscribe(&[&args.topic])
        .context("Failed to subscribe to topic")?;

    // Parse timestamp filters
    let from_micros = parse_timestamp(&args.from_timestamp)?;
    let to_micros = parse_timestamp(&args.to_timestamp)?;

    // Parse event type filter
    let event_filter = parse_event_filter(&args.events);

    print_info("Reading events from Kafka...");

    // Collect all events for this thread
    let mut events = Vec::new();
    let timeout = SHORT_TIMEOUT;

    // M-498: Track whether we've passed the checkpoint
    // Once we find an event with the matching checkpoint ID, include all subsequent events
    let mut passed_checkpoint = args.from_checkpoint.is_none();

    loop {
        match tokio::time::timeout(timeout, consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    if let Ok(msg) = decode_payload(payload) {
                        if let Some(dash_stream_message::Message::Event(event)) = msg.message {
                            if get_thread_id(&event) == Some(args.thread_id.as_str()) {
                                // Apply filters
                                if let Some(timestamp) = get_timestamp_us(&event) {
                                    if let Some(from) = from_micros {
                                        if timestamp < from {
                                            continue;
                                        }
                                    }
                                    if let Some(to) = to_micros {
                                        if timestamp > to {
                                            continue;
                                        }
                                    }
                                }

                                // M-498: Check if we've passed the checkpoint
                                if !passed_checkpoint {
                                    if let Some(checkpoint) = &args.from_checkpoint {
                                        // Check if this event has the checkpoint ID
                                        if get_string_attr(&event, attr_keys::CHECKPOINT_ID)
                                            .as_deref()
                                            == Some(checkpoint)
                                        {
                                            // Found the checkpoint - include this and all subsequent events
                                            passed_checkpoint = true;
                                        } else {
                                            // Still looking for checkpoint - skip this event
                                            continue;
                                        }
                                    }
                                }

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
        anyhow::bail!("No events found matching the criteria");
    }

    print_success(&format!("Found {} events to replay", events.len()));
    println!();

    // Display replay info
    if let (Some(first), Some(last)) = (events.first(), events.last()) {
        if let (Some(first_ts), Some(last_ts)) = (get_timestamp_us(first), get_timestamp_us(last)) {
            println!("{}", "Replay Window".bright_cyan().bold());
            println!("  Start: {}", format_timestamp(first_ts));
            println!("  End:   {}", format_timestamp(last_ts));
            let speed_str = if args.speed == 0.0 {
                "instant".to_string()
            } else {
                format!("{}", args.speed)
            };
            println!("  Speed: {speed_str}x");
            println!();
        }
    }

    println!("{}", "Event Stream".bright_cyan().bold());
    println!("{}", "─".repeat(80).bright_cyan());

    // Replay events
    let mut last_timestamp: Option<i64> = None;

    for event in events {
        // Apply event type filter
        if let Some(filter) = &event_filter {
            if !filter.contains(&event.event_type()) {
                continue;
            }
        }

        // Calculate delay for real-time replay
        if args.speed > 0.0 {
            if let Some(event_ts) = get_timestamp_us(&event) {
                if let Some(last_ts) = last_timestamp {
                    let delta_micros = event_ts - last_ts;
                    if delta_micros > 0 {
                        let delay_micros = (delta_micros as f64 / args.speed) as u64;
                        let delay = tokio::time::Duration::from_micros(delay_micros);
                        tokio::time::sleep(delay).await;
                    }
                }
                last_timestamp = Some(event_ts);
            }
        }

        // Print event
        print_event(&event);

        // Pause on error if requested
        if args.pause_on_error && event.event_type() == EventType::NodeError {
            println!(
                "\n{}",
                "ERROR DETECTED - Press Enter to continue..."
                    .bright_red()
                    .bold()
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
        }
    }

    println!();
    print_success("Replay complete");

    Ok(())
}

fn parse_timestamp(timestamp_str: &Option<String>) -> Result<Option<i64>> {
    match timestamp_str {
        None => Ok(None),
        Some(s) => {
            // Try parsing as Unix micros first
            if let Ok(micros) = s.parse::<i64>() {
                return Ok(Some(micros));
            }

            // Try parsing as RFC3339
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                let micros = dt.timestamp() * 1_000_000 + i64::from(dt.timestamp_subsec_micros());
                return Ok(Some(micros));
            }

            anyhow::bail!("Invalid timestamp format. Use Unix micros or RFC3339.");
        }
    }
}

fn parse_event_filter(events_str: &Option<String>) -> Option<Vec<EventType>> {
    events_str.as_ref().map(|s| {
        s.split(',')
            .filter_map(|name| match name.trim().to_uppercase().as_str() {
                "GRAPH_START" => Some(EventType::GraphStart),
                "GRAPH_END" => Some(EventType::GraphEnd),
                "NODE_START" => Some(EventType::NodeStart),
                "NODE_END" => Some(EventType::NodeEnd),
                "NODE_ERROR" => Some(EventType::NodeError),
                "EDGE_TRAVERSAL" | "EDGE" => Some(EventType::EdgeTraversal),
                "CONDITIONAL_BRANCH" | "BRANCH" => Some(EventType::ConditionalBranch),
                "PARALLEL_START" => Some(EventType::ParallelStart),
                "PARALLEL_END" => Some(EventType::ParallelEnd),
                "TOOL_START" | "TOOL_INVOCATION" | "TOOL_CALL" => Some(EventType::ToolStart),
                "TOOL_END" | "TOOL_RESULT" => Some(EventType::ToolEnd),
                "LLM_START" => Some(EventType::LlmStart),
                "LLM_END" => Some(EventType::LlmEnd),
                "CHECKPOINT_SAVE" => Some(EventType::CheckpointSave),
                "CHECKPOINT_LOAD" => Some(EventType::CheckpointLoad),
                _ => None,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        replay: ReplayArgs,
    }

    #[test]
    fn test_replay_args_required_thread_id() {
        let cli = Cli::parse_from(["test", "--thread-id", "thread-abc"]);

        assert_eq!(cli.replay.thread_id, "thread-abc");
        assert_eq!(cli.replay.bootstrap_servers, "localhost:9092");
        assert_eq!(cli.replay.topic, "dashstream-events");
        assert!(cli.replay.from_timestamp.is_none());
        assert!(cli.replay.to_timestamp.is_none());
        assert!(cli.replay.from_checkpoint.is_none());
        assert!((cli.replay.speed - 0.0).abs() < f64::EPSILON);
        assert!(cli.replay.events.is_none());
        assert!(!cli.replay.pause_on_error);
    }

    #[test]
    fn test_replay_args_with_time_filters() {
        let cli = Cli::parse_from([
            "test",
            "--thread-id", "t1",
            "--from-timestamp", "1234567890",
            "--to-timestamp", "1234567899",
        ]);

        assert_eq!(cli.replay.from_timestamp, Some("1234567890".to_string()));
        assert_eq!(cli.replay.to_timestamp, Some("1234567899".to_string()));
    }

    #[test]
    fn test_replay_args_with_checkpoint() {
        let cli = Cli::parse_from([
            "test",
            "--thread-id", "t1",
            "--from-checkpoint", "checkpoint-abc",
        ]);

        assert_eq!(cli.replay.from_checkpoint, Some("checkpoint-abc".to_string()));
    }

    #[test]
    fn test_replay_args_with_speed_and_filters() {
        let cli = Cli::parse_from([
            "test",
            "--thread-id", "t1",
            "--speed", "2.0",
            "--events", "NODE_START,NODE_END",
            "--pause-on-error",
        ]);

        assert!((cli.replay.speed - 2.0).abs() < f64::EPSILON);
        assert_eq!(cli.replay.events, Some("NODE_START,NODE_END".to_string()));
        assert!(cli.replay.pause_on_error);
    }

    #[test]
    fn test_parse_timestamp_none() {
        assert_eq!(parse_timestamp(&None).unwrap(), None);
    }

    #[test]
    fn test_parse_timestamp_unix_micros() {
        let result = parse_timestamp(&Some("1234567890123456".to_string())).unwrap();
        assert_eq!(result, Some(1234567890123456));
    }

    #[test]
    fn test_parse_timestamp_rfc3339() {
        let result = parse_timestamp(&Some("2025-01-15T10:30:00Z".to_string())).unwrap();
        assert!(result.is_some());
        // Just verify it parsed without error
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let result = parse_timestamp(&Some("not-a-timestamp".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_event_filter_none() {
        assert_eq!(parse_event_filter(&None), None);
    }

    #[test]
    fn test_parse_event_filter_single() {
        let result = parse_event_filter(&Some("NODE_START".to_string()));
        assert_eq!(result, Some(vec![EventType::NodeStart]));
    }

    #[test]
    fn test_parse_event_filter_multiple() {
        let result = parse_event_filter(&Some("NODE_START,NODE_END,GRAPH_START".to_string()));
        let expected = vec![
            EventType::NodeStart,
            EventType::NodeEnd,
            EventType::GraphStart,
        ];
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_event_filter_case_insensitive() {
        let result = parse_event_filter(&Some("node_start,Node_End".to_string()));
        let expected = vec![EventType::NodeStart, EventType::NodeEnd];
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_event_filter_aliases() {
        // Test EDGE alias
        let result = parse_event_filter(&Some("EDGE".to_string()));
        assert_eq!(result, Some(vec![EventType::EdgeTraversal]));

        // Test BRANCH alias
        let result = parse_event_filter(&Some("BRANCH".to_string()));
        assert_eq!(result, Some(vec![EventType::ConditionalBranch]));

        // Test TOOL_CALL alias
        let result = parse_event_filter(&Some("TOOL_CALL".to_string()));
        assert_eq!(result, Some(vec![EventType::ToolStart]));
    }

    #[test]
    fn test_parse_event_filter_invalid_skipped() {
        let result = parse_event_filter(&Some("NODE_START,INVALID,NODE_END".to_string()));
        let expected = vec![EventType::NodeStart, EventType::NodeEnd];
        assert_eq!(result, Some(expected));
    }
}
