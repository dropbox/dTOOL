// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! CLI commands for querying the EventStore.
//!
//! Provides commands to list, show, and inspect executions stored in the WAL.
//!
//! # Output Formats
//!
//! All executions subcommands support `--format` for output format selection:
//! - `--format table` (default): Human-readable colored table output
//! - `--format json`: Machine-readable JSON output for automation
//!
//! # Examples
//!
//! ```bash
//! # List recent executions
//! dashflow executions list
//! dashflow executions list --format json
//!
//! # Show execution details
//! dashflow executions show exec-abc123
//! dashflow executions show exec-abc123 --format json
//!
//! # Show events for an execution
//! dashflow executions events exec-abc123
//! dashflow executions events exec-abc123 --format json
//! ```

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;

use crate::output::{create_table, format_duration, print_error, print_info, print_success, OutputFormat};
use dashflow::wal::{EventStore, EventStoreConfig};

/// Query persisted execution data from the EventStore
#[derive(Args)]
pub struct ExecutionsArgs {
    #[command(subcommand)]
    command: ExecutionsCommand,
}

#[derive(Subcommand)]
enum ExecutionsCommand {
    /// List recent executions
    List(ListArgs),

    /// Show details for a specific execution
    Show(ShowArgs),

    /// Show events for an execution
    Events(EventsArgs),
}

/// Arguments for listing executions
#[derive(Args)]
struct ListArgs {
    /// Maximum number of executions to show
    #[arg(short, long, default_value = "20")]
    limit: usize,

    /// Filter by thread ID
    #[arg(long)]
    thread_id: Option<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,
}

/// Arguments for showing execution details
#[derive(Args)]
struct ShowArgs {
    /// Execution ID to show
    execution_id: String,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,
}

/// Arguments for showing execution events
#[derive(Args)]
struct EventsArgs {
    /// Execution ID to show events for
    execution_id: String,

    /// Maximum number of events to show (0 = all)
    #[arg(short, long, default_value = "50")]
    limit: usize,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,
}

pub async fn run(args: ExecutionsArgs) -> Result<()> {
    match args.command {
        ExecutionsCommand::List(args) => run_list(args).await,
        ExecutionsCommand::Show(args) => run_show(args).await,
        ExecutionsCommand::Events(args) => run_events(args).await,
    }
}

async fn run_list(args: ListArgs) -> Result<()> {
    let store = EventStore::new(EventStoreConfig::from_env().without_compaction())
        .context("Failed to open EventStore")?;

    let executions = if let Some(ref thread_id) = args.thread_id {
        print_info(&format!(
            "Listing executions for thread '{}'...",
            thread_id
        ));
        store
            .executions_by_thread(thread_id, args.limit)
            .context("Failed to query executions by thread")?
    } else {
        print_info(&format!("Listing recent {} executions...", args.limit));
        store
            .recent_executions(args.limit)
            .context("Failed to query recent executions")?
    };

    if executions.is_empty() {
        print_info("No executions found.");
        return Ok(());
    }

    if matches!(args.format, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&executions)
                .context("Failed to serialize executions to JSON")?
        );
        return Ok(());
    }

    print_success(&format!("Found {} executions", executions.len()));
    println!();

    println!("{}", "Executions".bright_cyan().bold());
    println!("{}", "=".repeat(100).bright_cyan());

    let mut table = create_table();
    table.set_header(vec![
        "ID",
        "Thread",
        "Status",
        "Duration",
        "Tokens",
        "Nodes",
        "Errors",
        "Started At",
    ]);

    for exec in &executions {
        let status = if exec.completed {
            "completed".bright_green().to_string()
        } else {
            "running".bright_yellow().to_string()
        };

        let thread = exec
            .thread_id
            .as_deref()
            .unwrap_or("-")
            .chars()
            .take(20)
            .collect::<String>();

        let started_at = exec.started_at_ms.map_or_else(
            || "-".to_string(),
            |ms| {
                chrono::DateTime::from_timestamp_millis(ms)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "-".to_string())
            },
        );

        // Truncate execution ID for display (first 8 chars + ... + last 4 chars if > 20)
        let exec_id = if exec.execution_id.len() > 20 {
            format!(
                "{}...{}",
                &exec.execution_id[..8],
                &exec.execution_id[exec.execution_id.len() - 4..]
            )
        } else {
            exec.execution_id.clone()
        };

        let errors = if exec.error_count > 0 {
            exec.error_count.to_string().bright_red().to_string()
        } else {
            "0".to_string()
        };

        table.add_row(vec![
            exec_id,
            thread,
            status,
            format_duration(exec.duration_ms * 1000), // Convert ms to microseconds for format_duration
            exec.total_tokens.to_string(),
            exec.node_count.to_string(),
            errors,
            started_at,
        ]);
    }

    println!("{table}");

    Ok(())
}

async fn run_show(args: ShowArgs) -> Result<()> {
    let store = EventStore::new(EventStoreConfig::from_env().without_compaction())
        .context("Failed to open EventStore")?;

    print_info(&format!(
        "Looking up execution '{}'...",
        args.execution_id
    ));

    let execution = store
        .execution_by_id(&args.execution_id)
        .context("Failed to query execution")?;

    match execution {
        Some(exec) => {
            if matches!(args.format, OutputFormat::Json) {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&exec)
                        .context("Failed to serialize execution to JSON")?
                );
                return Ok(());
            }

            print_success("Found execution");
            println!();

            println!("{}", "Execution Details".bright_cyan().bold());
            println!("{}", "=".repeat(80).bright_cyan());

            let mut table = create_table();
            table.set_header(vec!["Property", "Value"]);

            table.add_row(vec!["Execution ID", &exec.execution_id]);

            if let Some(ref thread_id) = exec.thread_id {
                table.add_row(vec!["Thread ID", thread_id]);
            }

            if let Some(ref graph_name) = exec.graph_name {
                table.add_row(vec!["Graph Name", graph_name]);
            }

            let status = if exec.completed {
                "completed".bright_green().to_string()
            } else {
                "running".bright_yellow().to_string()
            };
            table.add_row(vec!["Status".to_string(), status]);

            table.add_row(vec![
                "Duration".to_string(),
                format_duration(exec.duration_ms * 1000),
            ]);
            table.add_row(vec!["Total Tokens".to_string(), exec.total_tokens.to_string()]);
            table.add_row(vec!["Nodes Executed".to_string(), exec.node_count.to_string()]);
            table.add_row(vec!["Errors".to_string(), exec.error_count.to_string()]);

            if let Some(started_at) = exec.started_at_ms {
                let formatted = chrono::DateTime::from_timestamp_millis(started_at)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
                    .unwrap_or_else(|| "-".to_string());
                table.add_row(vec!["Started At".to_string(), formatted]);
            }

            if let Some(ended_at) = exec.ended_at_ms {
                let formatted = chrono::DateTime::from_timestamp_millis(ended_at)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
                    .unwrap_or_else(|| "-".to_string());
                table.add_row(vec!["Ended At".to_string(), formatted]);
            }

            if let Some(ref segment_path) = exec.segment_path {
                table.add_row(vec!["Segment Path".to_string(), segment_path.clone()]);
            }

            println!("{table}");

            Ok(())
        }
        None => {
            print_error(&format!(
                "Execution '{}' not found",
                args.execution_id
            ));
            std::process::exit(1);
        }
    }
}

async fn run_events(args: EventsArgs) -> Result<()> {
    let store = EventStore::new(EventStoreConfig::from_env().without_compaction())
        .context("Failed to open EventStore")?;

    print_info(&format!(
        "Loading events for execution '{}'...",
        args.execution_id
    ));

    let events = store
        .execution_events(&args.execution_id)
        .context("Failed to query execution events")?;

    if events.is_empty() {
        print_info("No events found for this execution.");
        return Ok(());
    }

    let display_events: Vec<_> = if args.limit > 0 && events.len() > args.limit {
        print_info(&format!(
            "Showing first {} of {} events (use --limit 0 for all)",
            args.limit,
            events.len()
        ));
        events.into_iter().take(args.limit).collect()
    } else {
        print_success(&format!("Found {} events", events.len()));
        events
    };

    if matches!(args.format, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&display_events)
                .context("Failed to serialize events to JSON")?
        );
        return Ok(());
    }

    println!();
    println!("{}", "Execution Events".bright_cyan().bold());
    println!("{}", "=".repeat(100).bright_cyan());

    let mut table = create_table();
    table.set_header(vec!["#", "Timestamp", "Event Type", "Details"]);

    for (i, event) in display_events.iter().enumerate() {
        let timestamp = chrono::DateTime::from_timestamp_millis(event.timestamp_ms as i64)
            .map(|dt| dt.format("%H:%M:%S%.3f").to_string())
            .unwrap_or_else(|| "-".to_string());

        let event_type = format_wal_event_type(&event.event_type);

        // Extract details from payload
        let details = format_event_payload(&event.payload);

        table.add_row(vec![
            (i + 1).to_string(),
            timestamp,
            event_type,
            details,
        ]);
    }

    println!("{table}");

    Ok(())
}

/// Format WAL event type with color
fn format_wal_event_type(event_type: &dashflow::wal::WALEventType) -> String {
    use dashflow::wal::WALEventType;

    match event_type {
        WALEventType::ExecutionStart => "EXEC_START".bright_green().bold().to_string(),
        WALEventType::ExecutionEnd => "EXEC_END".bright_green().to_string(),
        WALEventType::NodeStart => "NODE_START".bright_blue().to_string(),
        WALEventType::NodeEnd => "NODE_END".blue().to_string(),
        WALEventType::NodeError => "NODE_ERROR".bright_red().bold().to_string(),
        WALEventType::EdgeTraversal => "EDGE".cyan().to_string(),
        WALEventType::EdgeEvaluated => "EDGE_EVAL".bright_cyan().to_string(),
        WALEventType::StateChanged => "STATE_CHG".bright_magenta().to_string(),
        WALEventType::DecisionMade => "DECISION".bright_yellow().to_string(),
        WALEventType::OutcomeObserved => "OUTCOME".yellow().to_string(),
        WALEventType::ExecutionTrace => "TRACE".white().to_string(),
        WALEventType::LlmCallCompleted => "LLM_CALL".bright_purple().to_string(),
    }
}

/// Format event payload for display
fn format_event_payload(payload: &serde_json::Value) -> String {
    // Try to extract meaningful summary from payload
    let mut parts = Vec::new();

    if let Some(node) = payload.get("node").and_then(|v| v.as_str()) {
        parts.push(format!("node={}", node));
    }

    if let Some(from) = payload.get("from_node").and_then(|v| v.as_str()) {
        if let Some(to) = payload.get("to_node").and_then(|v| v.as_str()) {
            parts.push(format!("{} -> {}", from, to));
        }
    }

    if let Some(decision_type) = payload.get("decision_type").and_then(|v| v.as_str()) {
        parts.push(format!("type={}", decision_type));
    }

    if let Some(chosen) = payload.get("chosen_option").and_then(|v| v.as_str()) {
        parts.push(format!("chose={}", chosen));
    }

    if let Some(success) = payload.get("success").and_then(|v| v.as_bool()) {
        parts.push(if success {
            "success".bright_green().to_string()
        } else {
            "failed".bright_red().to_string()
        });
    }

    if let Some(summary) = payload.get("summary").and_then(|v| v.as_str()) {
        parts.push(summary.to_string());
    }

    if let Some(duration) = payload.get("duration_ms").and_then(|v| v.as_i64()) {
        parts.push(format!("{}ms", duration));
    }

    if parts.is_empty() {
        // Fallback: show first few keys
        if let Some(obj) = payload.as_object() {
            let keys: Vec<_> = obj.keys().take(3).cloned().collect();
            if !keys.is_empty() {
                return format!("{{{}}}", keys.join(", "));
            }
        }
        "-".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        command: TestCommands,
    }

    #[derive(Subcommand)]
    enum TestCommands {
        Executions(ExecutionsArgs),
    }

    #[test]
    fn test_list_args_defaults() {
        let cli = Cli::parse_from(["test", "executions", "list"]);
        if let TestCommands::Executions(ExecutionsArgs {
            command: ExecutionsCommand::List(args),
        }) = cli.command
        {
            assert_eq!(args.limit, 20);
            assert!(args.thread_id.is_none());
            assert!(matches!(args.format, OutputFormat::Table));
        } else {
            panic!("Expected List command");
        }
    }

    #[test]
    fn test_list_args_with_options() {
        let cli = Cli::parse_from([
            "test",
            "executions",
            "list",
            "--limit",
            "50",
            "--thread-id",
            "thread-123",
            "--format",
            "json",
        ]);
        if let TestCommands::Executions(ExecutionsArgs {
            command: ExecutionsCommand::List(args),
        }) = cli.command
        {
            assert_eq!(args.limit, 50);
            assert_eq!(args.thread_id, Some("thread-123".to_string()));
            assert!(matches!(args.format, OutputFormat::Json));
        } else {
            panic!("Expected List command");
        }
    }

    #[test]
    fn test_show_args() {
        let cli = Cli::parse_from(["test", "executions", "show", "exec-abc123"]);
        if let TestCommands::Executions(ExecutionsArgs {
            command: ExecutionsCommand::Show(args),
        }) = cli.command
        {
            assert_eq!(args.execution_id, "exec-abc123");
            assert!(matches!(args.format, OutputFormat::Table));
        } else {
            panic!("Expected Show command");
        }
    }

    #[test]
    fn test_show_args_with_format_json() {
        let cli = Cli::parse_from(["test", "executions", "show", "exec-abc123", "--format", "json"]);
        if let TestCommands::Executions(ExecutionsArgs {
            command: ExecutionsCommand::Show(args),
        }) = cli.command
        {
            assert_eq!(args.execution_id, "exec-abc123");
            assert!(matches!(args.format, OutputFormat::Json));
        } else {
            panic!("Expected Show command");
        }
    }

    #[test]
    fn test_events_args_defaults() {
        let cli = Cli::parse_from(["test", "executions", "events", "exec-xyz"]);
        if let TestCommands::Executions(ExecutionsArgs {
            command: ExecutionsCommand::Events(args),
        }) = cli.command
        {
            assert_eq!(args.execution_id, "exec-xyz");
            assert_eq!(args.limit, 50);
            assert!(matches!(args.format, OutputFormat::Table));
        } else {
            panic!("Expected Events command");
        }
    }

    #[test]
    fn test_events_args_with_options() {
        let cli = Cli::parse_from([
            "test",
            "executions",
            "events",
            "exec-xyz",
            "--limit",
            "0",
            "--format",
            "json",
        ]);
        if let TestCommands::Executions(ExecutionsArgs {
            command: ExecutionsCommand::Events(args),
        }) = cli.command
        {
            assert_eq!(args.execution_id, "exec-xyz");
            assert_eq!(args.limit, 0);
            assert!(matches!(args.format, OutputFormat::Json));
        } else {
            panic!("Expected Events command");
        }
    }

    #[test]
    fn test_format_wal_event_type() {
        use dashflow::wal::WALEventType;

        // Just check that each variant returns a non-empty string
        assert!(!format_wal_event_type(&WALEventType::ExecutionStart).is_empty());
        assert!(!format_wal_event_type(&WALEventType::NodeStart).is_empty());
        assert!(!format_wal_event_type(&WALEventType::NodeEnd).is_empty());
        assert!(!format_wal_event_type(&WALEventType::NodeError).is_empty());
        assert!(!format_wal_event_type(&WALEventType::EdgeTraversal).is_empty());
        assert!(!format_wal_event_type(&WALEventType::EdgeEvaluated).is_empty());
        assert!(!format_wal_event_type(&WALEventType::StateChanged).is_empty());
        assert!(!format_wal_event_type(&WALEventType::DecisionMade).is_empty());
        assert!(!format_wal_event_type(&WALEventType::OutcomeObserved).is_empty());
        assert!(!format_wal_event_type(&WALEventType::ExecutionTrace).is_empty());
    }

    #[test]
    fn test_format_event_payload_node() {
        let payload = serde_json::json!({
            "node": "my_node"
        });
        let result = format_event_payload(&payload);
        assert!(result.contains("node=my_node"));
    }

    #[test]
    fn test_format_event_payload_edge() {
        let payload = serde_json::json!({
            "from_node": "node_a",
            "to_node": "node_b"
        });
        let result = format_event_payload(&payload);
        assert!(result.contains("node_a -> node_b"));
    }

    #[test]
    fn test_format_event_payload_decision() {
        let payload = serde_json::json!({
            "decision_type": "tool_selection",
            "chosen_option": "search_tool"
        });
        let result = format_event_payload(&payload);
        assert!(result.contains("type=tool_selection"));
        assert!(result.contains("chose=search_tool"));
    }

    #[test]
    fn test_format_event_payload_empty() {
        let payload = serde_json::json!({});
        let result = format_event_payload(&payload);
        assert_eq!(result, "-");
    }
}
