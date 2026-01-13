use crate::helpers::{attr_keys, get_header, get_string_attr, get_timestamp_us};
use chrono::DateTime;
use clap::ValueEnum;
use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, Table};
use dashflow_streaming::{Event, EventType};

/// Output format for CLI commands.
///
/// Provides consistent output formatting across all CLI commands.
/// Defaults to human-readable table format.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table output with colors
    #[default]
    Table,
    /// Machine-readable JSON output
    Json,
}

/// Pretty print an event with colored output
pub fn print_event(event: &Event) {
    let timestamp = get_timestamp_us(event).map_or_else(|| "N/A".to_string(), format_timestamp);
    let event_type = format_event_type(event.event_type());
    let message_id = get_header(event).map_or_else(
        || "unknown".to_string(),
        |h| hex::encode(&h.message_id[..8]),
    );

    println!(
        "{} {} {} {}",
        timestamp.dimmed(),
        event_type,
        message_id.bright_black(),
        format_event_details(event)
    );
}

/// Format timestamp for display
pub fn format_timestamp(micros: i64) -> String {
    let secs = micros / 1_000_000;
    let nanos = ((micros % 1_000_000) * 1000) as u32;

    if let Some(dt) = DateTime::from_timestamp(secs, nanos) {
        dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    } else {
        format!("{micros}")
    }
}

/// Format event type with color
pub fn format_event_type(event_type: EventType) -> String {
    use EventType::{
        CheckpointDelete, CheckpointLoad, CheckpointSave, ConditionalBranch, EdgeTraversal,
        GraphEnd, GraphError, GraphStart, LlmEnd, LlmError, LlmRetry, LlmStart, MemoryLoad,
        MemorySave, NodeEnd, NodeError, NodeStart, ParallelEnd, ParallelStart, ToolEnd, ToolError,
        ToolStart,
    };

    match event_type {
        GraphStart => "GRAPH_START".bright_green().bold().to_string(),
        GraphEnd => "GRAPH_END".bright_green().to_string(),
        GraphError => "GRAPH_ERROR".bright_red().bold().to_string(),
        NodeStart => "NODE_START".bright_blue().to_string(),
        NodeEnd => "NODE_END".blue().to_string(),
        NodeError => "NODE_ERROR".bright_red().bold().to_string(),
        EdgeTraversal => "EDGE".cyan().to_string(),
        ConditionalBranch => "BRANCH".bright_cyan().to_string(),
        ParallelStart => "PARALLEL_START".magenta().to_string(),
        ParallelEnd => "PARALLEL_END".magenta().to_string(),
        ToolStart => "TOOL_START".yellow().to_string(),
        ToolEnd => "TOOL_END".yellow().to_string(),
        ToolError => "TOOL_ERROR".bright_red().to_string(),
        LlmStart => "LLM_START".bright_yellow().to_string(),
        LlmEnd => "LLM_END".yellow().to_string(),
        LlmError => "LLM_ERROR".bright_red().to_string(),
        LlmRetry => "LLM_RETRY".yellow().to_string(),
        CheckpointSave => "CHECKPOINT_SAVE".bright_magenta().to_string(),
        CheckpointLoad => "CHECKPOINT_LOAD".bright_magenta().to_string(),
        CheckpointDelete => "CHECKPOINT_DELETE".bright_magenta().to_string(),
        MemorySave => "MEMORY_SAVE".bright_purple().to_string(),
        MemoryLoad => "MEMORY_LOAD".bright_purple().to_string(),
        _ => format!("UNKNOWN({})", event_type as i32).red().to_string(),
    }
}

/// Format event-specific details
fn format_event_details(event: &Event) -> String {
    if !event.node_id.is_empty() {
        format!("node={}", event.node_id.bright_white())
    } else if let Some(from) = get_string_attr(event, attr_keys::EDGE_FROM) {
        if let Some(to) = get_string_attr(event, attr_keys::EDGE_TO) {
            format!("{} → {}", from.white(), to.white())
        } else {
            from.white().to_string()
        }
    } else if let Some(tool) = get_string_attr(event, attr_keys::TOOL_NAME) {
        format!("tool={}", tool.yellow())
    } else {
        String::new()
    }
}

/// Create a formatted table
pub fn create_table() -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table
}

/// Format duration in human-readable form
pub fn format_duration(micros: i64) -> String {
    if micros < 1_000 {
        format!("{micros}μs")
    } else if micros < 1_000_000 {
        format!("{:.2}ms", micros as f64 / 1_000.0)
    } else if micros < 60_000_000 {
        format!("{:.2}s", micros as f64 / 1_000_000.0)
    } else {
        let minutes = micros / 60_000_000;
        let seconds = (micros % 60_000_000) / 1_000_000;
        format!("{minutes}m {seconds}s")
    }
}

/// Format byte size in human-readable form
pub fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.2}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Print error message
pub fn print_error(msg: &str) {
    eprintln!("{} {}", "ERROR:".bright_red().bold(), msg);
}

/// Print warning message
#[allow(dead_code)] // Architectural: Reserved for deprecation warnings and non-fatal issues
pub fn print_warning(msg: &str) {
    eprintln!("{} {}", "WARNING:".bright_yellow().bold(), msg);
}

/// Print success message
pub fn print_success(msg: &str) {
    println!("{} {}", "✓".bright_green().bold(), msg);
}

/// Print info message
pub fn print_info(msg: &str) {
    println!("{} {}", "ℹ".bright_blue().bold(), msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::attr_keys;
    use dashflow_streaming::{attribute_value, AttributeValue, Event, EventType, Header, MessageType};
    use std::collections::HashMap;

    fn no_color() {
        colored::control::set_override(false);
    }

    fn make_event() -> Event {
        Event {
            header: Some(Header {
                message_id: vec![1; 16],
                timestamp_us: 1_000,
                tenant_id: "t".to_string(),
                thread_id: "th".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::NodeStart as i32,
            node_id: String::new(),
            attributes: HashMap::new(),
            duration_us: 0,
            llm_request_id: String::new(),
        }
    }

    #[test]
    fn format_timestamp_handles_epoch_and_out_of_range() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00:00.000");
        assert_eq!(format_timestamp(i64::MAX), format!("{}", i64::MAX));
    }

    #[test]
    fn format_duration_formats_units() {
        assert_eq!(format_duration(999), "999μs");
        assert_eq!(format_duration(1_000), "1.00ms");
        assert_eq!(format_duration(12_345), "12.35ms");
        assert_eq!(format_duration(1_234_567), "1.23s");
        assert_eq!(format_duration(60_000_000), "1m 0s");
    }

    #[test]
    fn format_bytes_formats_units() {
        assert_eq!(format_bytes(1023), "1023B");
        assert_eq!(format_bytes(1024), "1.00KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00GB");
    }

    #[test]
    fn format_event_type_returns_label_or_unknown() {
        no_color();
        assert_eq!(format_event_type(EventType::GraphStart), "GRAPH_START");
        assert_eq!(format_event_type(EventType::HumanInterrupt), "UNKNOWN(70)");
    }

    #[test]
    fn format_event_details_prefers_node_id_then_attrs() {
        no_color();

        let mut event = make_event();
        event.node_id = "node-1".to_string();
        assert_eq!(format_event_details(&event), "node=node-1");

        let mut event = make_event();
        event.attributes.insert(
            attr_keys::EDGE_FROM.to_string(),
            AttributeValue {
                value: Some(attribute_value::Value::StringValue("a".to_string())),
            },
        );
        event.attributes.insert(
            attr_keys::EDGE_TO.to_string(),
            AttributeValue {
                value: Some(attribute_value::Value::StringValue("b".to_string())),
            },
        );
        assert_eq!(format_event_details(&event), "a → b");

        let mut event = make_event();
        event.attributes.insert(
            attr_keys::TOOL_NAME.to_string(),
            AttributeValue {
                value: Some(attribute_value::Value::StringValue("tool".to_string())),
            },
        );
        assert_eq!(format_event_details(&event), "tool=tool");
    }
}
