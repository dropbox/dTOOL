use crate::helpers::decode_payload;
use crate::output::{format_bytes, print_info, print_success};
use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dashflow::constants::SHORT_TIMEOUT;
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::{dash_stream_message, diff_operation::OpType, StateDiff};
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    Message,
};
use serde_json::Value;

/// Compare two checkpoints
#[derive(Args)]
pub struct DiffArgs {
    /// Kafka bootstrap servers (comma-separated)
    #[arg(short, long, default_value = "localhost:9092")]
    bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    #[arg(short, long, default_value = "dashstream-events")]
    topic: String,

    /// Thread ID
    #[arg(long = "thread", alias = "thread-id", required = true)]
    thread_id: String,

    /// First checkpoint ID
    #[arg(long, required = true)]
    checkpoint1: String,

    /// Second checkpoint ID
    #[arg(long, required = true)]
    checkpoint2: String,

    /// Show detailed diff operations
    #[arg(long)]
    detailed: bool,

    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    format: String,
}

pub async fn run(args: DiffArgs) -> Result<()> {
    print_info(&format!(
        "Comparing checkpoints '{}' and '{}' for thread '{}'...",
        args.checkpoint1, args.checkpoint2, args.thread_id
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-diff-{}", uuid::Uuid::new_v4()),
        )
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false");
    let consumer: StreamConsumer = client_config
        .create()
        .context("Failed to create Kafka consumer")?;

    consumer
        .subscribe(&[&args.topic])
        .context("Failed to subscribe to topic")?;

    print_info("Reading state diffs from Kafka...");

    // Find state diffs for the two checkpoints
    let mut state_diffs: Vec<StateDiff> = Vec::new();
    let timeout = SHORT_TIMEOUT;

    loop {
        match tokio::time::timeout(timeout, consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    if let Ok(msg) = decode_payload(payload) {
                        if let Some(dash_stream_message::Message::StateDiff(diff)) = msg.message {
                            if diff.header.as_ref().map(|h| h.thread_id.as_str())
                                == Some(args.thread_id.as_str())
                            {
                                state_diffs.push(diff);
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

    if state_diffs.is_empty() {
        println!("\n{}", "No state diffs found for this thread.".yellow());
        return Ok(());
    }

    // Find the specific diffs for our checkpoints
    // Note: This logic may need to be adjusted based on how checkpoints are actually stored
    let diff1 = state_diffs
        .iter()
        .find(|d| hex::encode(&d.base_checkpoint_id) == args.checkpoint1)
        .context("Checkpoint 1 not found")?;

    let diff2 = state_diffs
        .iter()
        .find(|d| hex::encode(&d.base_checkpoint_id) == args.checkpoint2)
        .context("Checkpoint 2 not found")?;

    print_success(&format!("Found {} total state diffs", state_diffs.len()));
    println!();

    // Display comparison
    if args.format == "json" {
        display_json_diff(diff1, diff2)?;
    } else {
        display_text_diff(diff1, diff2, args.detailed)?;
    }

    Ok(())
}

fn display_text_diff(diff1: &StateDiff, diff2: &StateDiff, detailed: bool) -> Result<()> {
    println!("{}", "Checkpoint Comparison".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    // Checkpoint 1 info
    println!("\n{}", "Checkpoint 1".bright_green().bold());
    println!("  Base ID:     {}", hex::encode(&diff1.base_checkpoint_id));
    println!("  State Hash:  {}", hex::encode(&diff1.state_hash));
    println!("  Operations:  {}", diff1.operations.len());

    if diff1.full_state.is_empty() {
        println!("  Full State:  {}", "No (incremental)".yellow());
    } else {
        println!(
            "  Full State:  {} ({})",
            "Yes".green(),
            format_bytes(diff1.full_state.len())
        );
    }

    // Checkpoint 2 info
    println!("\n{}", "Checkpoint 2".bright_green().bold());
    println!("  Base ID:     {}", hex::encode(&diff2.base_checkpoint_id));
    println!("  State Hash:  {}", hex::encode(&diff2.state_hash));
    println!("  Operations:  {}", diff2.operations.len());

    if diff2.full_state.is_empty() {
        println!("  Full State:  {}", "No (incremental)".yellow());
    } else {
        println!(
            "  Full State:  {} ({})",
            "Yes".green(),
            format_bytes(diff2.full_state.len())
        );
    }

    // Show operations if detailed
    if detailed {
        println!(
            "\n{}",
            "Diff Operations (Checkpoint 1)".bright_cyan().bold()
        );
        println!("{}", "─".repeat(80).bright_cyan());
        display_operations(&diff1.operations);

        println!(
            "\n{}",
            "Diff Operations (Checkpoint 2)".bright_cyan().bold()
        );
        println!("{}", "─".repeat(80).bright_cyan());
        display_operations(&diff2.operations);
    }

    // Compare the two states if we have full states
    if !diff1.full_state.is_empty() && !diff2.full_state.is_empty() {
        println!("\n{}", "State Comparison".bright_cyan().bold());
        println!("{}", "─".repeat(80).bright_cyan());

        let state1: Value =
            serde_json::from_slice(&diff1.full_state).context("Failed to parse state 1 as JSON")?;
        let state2: Value =
            serde_json::from_slice(&diff2.full_state).context("Failed to parse state 2 as JSON")?;

        let patch = json_patch::diff(&state1, &state2);

        println!("  Changes: {} operations", patch.0.len());
        if detailed {
            for op in patch.0 {
                println!("    {}", serde_json::to_string(&op)?);
            }
        }
    }

    Ok(())
}

fn display_operations(operations: &[dashflow_streaming::DiffOperation]) {
    for (i, op) in operations.iter().enumerate() {
        let op_type = OpType::try_from(op.op).unwrap_or(OpType::Add);
        let value_str = String::from_utf8_lossy(&op.value);

        let op_str = match op_type {
            OpType::Add => {
                format!(
                    "{} ADD    {} = {}",
                    "+".green().bold(),
                    op.path.cyan(),
                    truncate_value(&value_str, 50)
                )
            }
            OpType::Remove => {
                format!("{} REMOVE {}", "-".red().bold(), op.path.cyan())
            }
            OpType::Replace => {
                format!(
                    "{} REPLACE {} = {}",
                    "~".yellow().bold(),
                    op.path.cyan(),
                    truncate_value(&value_str, 50)
                )
            }
            OpType::Move => {
                format!(
                    "{} MOVE    {} → {}",
                    "→".blue().bold(),
                    op.from.cyan(),
                    op.path.cyan()
                )
            }
            OpType::Copy => {
                format!(
                    "{} COPY    {} → {}",
                    "⇒".blue().bold(),
                    op.from.cyan(),
                    op.path.cyan()
                )
            }
            OpType::Test => {
                format!(
                    "{} TEST    {} = {}",
                    "?".white().bold(),
                    op.path.cyan(),
                    truncate_value(&value_str, 50)
                )
            }
        };

        println!("  {}: {}", (i + 1).to_string().dimmed(), op_str);
    }
}

fn display_json_diff(diff1: &StateDiff, diff2: &StateDiff) -> Result<()> {
    let json_output = serde_json::json!({
        "checkpoint1": {
            "base_id": hex::encode(&diff1.base_checkpoint_id),
            "state_hash": hex::encode(&diff1.state_hash),
            "operations": diff1.operations.len(),
            "has_full_state": !diff1.full_state.is_empty(),
            "state_size": diff1.full_state.len(),
        },
        "checkpoint2": {
            "base_id": hex::encode(&diff2.base_checkpoint_id),
            "state_hash": hex::encode(&diff2.state_hash),
            "operations": diff2.operations.len(),
            "has_full_state": !diff2.full_state.is_empty(),
            "state_size": diff2.full_state.len(),
        },
    });

    println!("{}", serde_json::to_string_pretty(&json_output)?);
    Ok(())
}

fn truncate_value(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        format!("{}...", &value[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_value_short_string() {
        assert_eq!(truncate_value("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_value_exact_length() {
        assert_eq!(truncate_value("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_value_long_string() {
        assert_eq!(truncate_value("hello world", 5), "hello...");
    }

    #[test]
    fn test_truncate_value_empty_string() {
        assert_eq!(truncate_value("", 10), "");
    }

    #[test]
    fn test_diff_args_parse() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            diff: DiffArgs,
        }

        let cli = Cli::parse_from([
            "test",
            "--thread-id", "thread-123",
            "--checkpoint1", "abc123",
            "--checkpoint2", "def456",
        ]);

        assert_eq!(cli.diff.thread_id, "thread-123");
        assert_eq!(cli.diff.checkpoint1, "abc123");
        assert_eq!(cli.diff.checkpoint2, "def456");
        assert_eq!(cli.diff.bootstrap_servers, "localhost:9092"); // default
        assert!(!cli.diff.detailed);
        assert_eq!(cli.diff.format, "text");
    }

    #[test]
    fn test_diff_args_with_options() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            diff: DiffArgs,
        }

        let cli = Cli::parse_from([
            "test",
            "--thread-id", "t1",
            "--checkpoint1", "c1",
            "--checkpoint2", "c2",
            "--detailed",
            "--format", "json",
            "--bootstrap-servers", "kafka:9093",
        ]);

        assert!(cli.diff.detailed);
        assert_eq!(cli.diff.format, "json");
        assert_eq!(cli.diff.bootstrap_servers, "kafka:9093");
    }
}
