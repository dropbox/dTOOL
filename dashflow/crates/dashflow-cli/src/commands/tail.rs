use crate::helpers::{decode_payload, get_tenant_id, get_thread_id};
use crate::output::{print_error, print_event, print_info};
use anyhow::{Context, Result};
use clap::Args;
use dashflow_streaming::dash_stream_message;
use dashflow_streaming::kafka::KafkaSecurityConfig;
use rdkafka::message::BorrowedMessage;
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    Message,
};

/// Stream live events from Kafka
#[derive(Args)]
pub struct TailArgs {
    /// Kafka bootstrap servers (comma-separated)
    /// M-631: Support KAFKA_BROKERS env var for consistency
    #[arg(short, long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    /// M-631: Support KAFKA_TOPIC env var for consistency
    #[arg(short, long, env = "KAFKA_TOPIC", default_value = "dashstream-events")]
    topic: String,

    /// Filter by tenant ID
    #[arg(long)]
    tenant: Option<String>,

    /// Filter by thread ID
    #[arg(long)]
    thread: Option<String>,

    /// Start from beginning of topic
    #[arg(short = 'B', long)]
    from_beginning: bool,

    /// Follow mode (like tail -f)
    #[arg(short = 'f', long, default_value = "true")]
    follow: bool,

    /// Limit number of messages to display
    #[arg(short, long)]
    limit: Option<usize>,

    /// Filter by event type (e.g., "`NODE_START`", "`NODE_END`")
    #[arg(long)]
    event_type: Option<String>,

    /// Commit offsets to Kafka (default: false)
    ///
    /// By default, tail does not commit offsets since it's a debug/monitoring tool
    /// that shouldn't mutate Kafka state. Enable this if you want to track read position.
    #[arg(long, default_value = "false")]
    commit: bool,
}

pub async fn run(args: TailArgs) -> Result<()> {
    print_info(&format!(
        "Connecting to Kafka at {} topic '{}'...",
        args.bootstrap_servers, args.topic
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    // M-501: Generate unique group ID per invocation to prevent data loss
    // Using a fixed group ID causes concurrent tail commands to share partitions,
    // meaning each instance only sees a subset of messages.
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-tail-{}", uuid::Uuid::new_v4()),
        )
        .set(
            "auto.offset.reset",
            if args.from_beginning {
                "earliest"
            } else {
                "latest"
            },
        )
        // M-477: Default to not committing offsets (debug tool shouldn't mutate Kafka state)
        .set(
            "enable.auto.commit",
            if args.commit { "true" } else { "false" },
        );
    let consumer: StreamConsumer = client_config
        .create()
        .context("Failed to create Kafka consumer")?;

    consumer
        .subscribe(&[&args.topic])
        .context("Failed to subscribe to topic")?;

    print_info(&format!(
        "Streaming events from '{}' (Ctrl+C to stop)...",
        args.topic
    ));
    println!();

    let mut count = 0;
    loop {
        match consumer.recv().await {
            Ok(message) => {
                if let Err(e) = process_message(&message, &args) {
                    print_error(&format!("Failed to process message: {e}"));
                    continue;
                }

                count += 1;
                if let Some(limit) = args.limit {
                    if count >= limit {
                        print_info(&format!("Reached limit of {limit} messages"));
                        break;
                    }
                }
            }
            Err(e) => {
                print_error(&format!("Kafka error: {e}"));
                if !args.follow {
                    break;
                }
                // In follow mode, continue on errors
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }

    Ok(())
}

fn process_message(message: &BorrowedMessage, args: &TailArgs) -> Result<()> {
    let payload = message.payload().context("Empty message payload")?;

    let msg = decode_payload(payload).context("Failed to decode DashStream message")?;

    // Extract event from message
    let event = match msg.message {
        Some(dash_stream_message::Message::Event(event)) => event,
        Some(dash_stream_message::Message::StateDiff(_)) => {
            // Skip state diffs unless explicitly requested
            return Ok(());
        }
        _ => return Ok(()), // Skip other message types
    };

    // Apply filters
    if let Some(tenant_filter) = &args.tenant {
        if get_tenant_id(&event) != Some(tenant_filter.as_str()) {
            return Ok(());
        }
    }

    if let Some(thread_filter) = &args.thread {
        if get_thread_id(&event) != Some(thread_filter.as_str()) {
            return Ok(());
        }
    }

    if let Some(event_type_filter) = &args.event_type {
        let event_type_name = format!("{:?}", event.event_type());
        if !event_type_name
            .to_uppercase()
            .contains(&event_type_filter.to_uppercase())
        {
            return Ok(());
        }
    }

    // Print the event
    print_event(&event);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        tail: TailArgs,
    }

    #[test]
    fn test_tail_args_defaults() {
        let cli = Cli::parse_from(["test"]);

        assert_eq!(cli.tail.bootstrap_servers, "localhost:9092");
        assert_eq!(cli.tail.topic, "dashstream-events");
        assert!(cli.tail.tenant.is_none());
        assert!(cli.tail.thread.is_none());
        assert!(!cli.tail.from_beginning);
        assert!(cli.tail.follow);
        assert!(cli.tail.limit.is_none());
        assert!(cli.tail.event_type.is_none());
        assert!(!cli.tail.commit);
    }

    #[test]
    fn test_tail_args_with_filters() {
        let cli = Cli::parse_from([
            "test",
            "--tenant", "tenant-123",
            "--thread", "thread-456",
            "--event-type", "NODE_START",
            "--limit", "100",
        ]);

        assert_eq!(cli.tail.tenant, Some("tenant-123".to_string()));
        assert_eq!(cli.tail.thread, Some("thread-456".to_string()));
        assert_eq!(cli.tail.event_type, Some("NODE_START".to_string()));
        assert_eq!(cli.tail.limit, Some(100));
    }

    #[test]
    fn test_tail_args_kafka_options() {
        let cli = Cli::parse_from([
            "test",
            "-b", "kafka1:9092,kafka2:9092",
            "-t", "custom-topic",
            "-B", // from-beginning short form
            "--commit",
        ]);

        assert_eq!(cli.tail.bootstrap_servers, "kafka1:9092,kafka2:9092");
        assert_eq!(cli.tail.topic, "custom-topic");
        assert!(cli.tail.from_beginning);
        assert!(cli.tail.commit);
    }

    #[test]
    fn test_tail_args_follow_default_is_true() {
        // The follow flag defaults to true per the struct definition
        let cli = Cli::parse_from(["test"]);
        assert!(cli.tail.follow);
    }
}
