use crate::helpers::{
    decode_payload, get_message_id, get_sequence, get_tenant_id, get_thread_id, get_timestamp_us,
};
use crate::output::{print_info, print_success};
use anyhow::{Context, Result};
use clap::Args;
use dashflow::constants::SHORT_TIMEOUT;
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::{dash_stream_message, Event};
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    Message,
};
use serde::Serialize;

/// Export thread data to JSON
#[derive(Args)]
pub struct ExportArgs {
    /// Kafka bootstrap servers (comma-separated)
    /// M-631: Support KAFKA_BROKERS env var for consistency
    #[arg(short, long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    /// M-631: Support KAFKA_TOPIC env var for consistency
    #[arg(short, long, env = "KAFKA_TOPIC", default_value = "dashstream-events")]
    topic: String,

    /// Thread ID to export
    #[arg(long = "thread", alias = "thread-id", required = true)]
    thread_id: String,

    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<String>,

    /// Export format (json, jsonl, csv)
    #[arg(short, long, default_value = "json")]
    format: String,

    /// Pretty print JSON
    #[arg(long)]
    pretty: bool,
    // M-504: Removed unused `include_diffs` flag. Users who need diffs can process
    // the JSON output themselves or use `dashflow diff` command.
}

#[derive(Serialize)]
struct ExportedEvent {
    message_id: String,
    sequence: u64,
    timestamp_micros: i64,
    tenant_id: String,
    thread_id: String,
    event_type: String,
    node_id: String,
    duration_us: i64,
    llm_request_id: String,
    attributes: serde_json::Value,
}

#[derive(Serialize)]
struct ExportOutput {
    thread_id: String,
    total_events: usize,
    start_time: i64,
    end_time: i64,
    duration_micros: i64,
    events: Vec<ExportedEvent>,
}

pub async fn run(args: ExportArgs) -> Result<()> {
    print_info(&format!(
        "Exporting thread '{}' from topic '{}'...",
        args.thread_id, args.topic
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-export-{}", uuid::Uuid::new_v4()),
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

    // Convert to export format
    let exported_events: Vec<ExportedEvent> = events.iter().map(event_to_exported).collect();

    let start_time = events.first().and_then(get_timestamp_us).unwrap_or(0);
    let end_time = events.last().and_then(get_timestamp_us).unwrap_or(0);

    let output_data = ExportOutput {
        thread_id: args.thread_id.clone(),
        total_events: events.len(),
        start_time,
        end_time,
        duration_micros: end_time - start_time,
        events: exported_events,
    };

    // Write output
    match args.format.as_str() {
        "json" => {
            let json = if args.pretty {
                serde_json::to_string_pretty(&output_data)?
            } else {
                serde_json::to_string(&output_data)?
            };
            write_output(&json, &args.output).await?;
        }
        "jsonl" => {
            let mut lines = String::new();
            for event in &output_data.events {
                lines.push_str(&serde_json::to_string(event)?);
                lines.push('\n');
            }
            write_output(&lines, &args.output).await?;
        }
        "csv" => {
            let csv = events_to_csv(&output_data.events)?;
            write_output(&csv, &args.output).await?;
        }
        _ => {
            anyhow::bail!("Unsupported format: {}", args.format);
        }
    }

    if let Some(path) = &args.output {
        print_success(&format!("Exported to {path}"));
    }

    Ok(())
}

fn event_to_exported(event: &Event) -> ExportedEvent {
    // Convert attributes to JSON
    let mut attr_map = serde_json::Map::new();
    for (key, value) in &event.attributes {
        if let Some(val) = &value.value {
            use dashflow_streaming::attribute_value::Value;
            let json_val = match val {
                Value::StringValue(s) => serde_json::Value::String(s.clone()),
                Value::IntValue(i) => serde_json::Value::Number((*i).into()),
                Value::FloatValue(f) => serde_json::json!(f),
                Value::BoolValue(b) => serde_json::Value::Bool(*b),
                Value::BytesValue(b) => serde_json::Value::String(hex::encode(b)),
                _ => serde_json::Value::Null,
            };
            attr_map.insert(key.clone(), json_val);
        }
    }

    ExportedEvent {
        message_id: get_message_id(event).map(hex::encode).unwrap_or_default(),
        sequence: get_sequence(event).unwrap_or(0),
        timestamp_micros: get_timestamp_us(event).unwrap_or(0),
        tenant_id: get_tenant_id(event).unwrap_or("").to_string(),
        thread_id: get_thread_id(event).unwrap_or("").to_string(),
        event_type: format!("{:?}", event.event_type()),
        node_id: event.node_id.clone(),
        duration_us: event.duration_us,
        llm_request_id: event.llm_request_id.clone(),
        attributes: serde_json::Value::Object(attr_map),
    }
}

/// Escape a CSV field per RFC 4180.
/// M-500: Prevents CSV injection and handles special characters.
/// Fields containing commas, quotes, newlines, or starting with =, +, -, @ are quoted.
fn escape_csv_field(field: &str) -> String {
    // Check if field needs quoting
    let needs_quoting = field.contains(',')
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r')
        || field.starts_with('=')
        || field.starts_with('+')
        || field.starts_with('-')
        || field.starts_with('@')
        || field.starts_with('\t')
        || field.starts_with(' ');

    if needs_quoting {
        // Quote the field and escape any internal quotes by doubling them
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn events_to_csv(events: &[ExportedEvent]) -> Result<String> {
    let mut csv = String::new();

    // Header
    csv.push_str("message_id,sequence,timestamp_micros,tenant_id,thread_id,event_type,node_id,duration_us,llm_request_id\n");

    // Rows - M-500: Properly escape all string fields
    for event in events {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            escape_csv_field(&event.message_id),
            event.sequence,
            event.timestamp_micros,
            escape_csv_field(&event.tenant_id),
            escape_csv_field(&event.thread_id),
            escape_csv_field(&event.event_type),
            escape_csv_field(&event.node_id),
            event.duration_us,
            escape_csv_field(&event.llm_request_id),
        ));
    }

    Ok(csv)
}

async fn write_output(content: &str, output_path: &Option<String>) -> Result<()> {
    match output_path {
        Some(path) => {
            tokio::fs::write(path, content.as_bytes())
                .await
                .context(format!("Failed to write output file: {path}"))?;
        }
        None => {
            println!("{content}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_csv_field_quotes_special_chars_and_injection_prefixes() {
        assert_eq!(escape_csv_field("simple"), "simple");
        assert_eq!(escape_csv_field("a,b"), "\"a,b\"");
        assert_eq!(escape_csv_field("a\"b"), "\"a\"\"b\"");
        assert_eq!(escape_csv_field("a\nb"), "\"a\nb\"");
        assert_eq!(escape_csv_field("=1+2"), "\"=1+2\"");
        assert_eq!(escape_csv_field(" leading"), "\" leading\"");
    }

    #[test]
    fn events_to_csv_writes_header_and_escapes_fields() {
        let events = vec![ExportedEvent {
            message_id: "id".to_string(),
            sequence: 1,
            timestamp_micros: 2,
            tenant_id: "t".to_string(),
            thread_id: "th".to_string(),
            event_type: "NodeStart".to_string(),
            node_id: "a,b".to_string(),
            duration_us: 3,
            llm_request_id: "=cmd".to_string(),
            attributes: serde_json::Value::Null,
        }];

        let csv = events_to_csv(&events).expect("events_to_csv");
        assert!(csv.starts_with("message_id,sequence,timestamp_micros"));
        assert!(csv.contains("id,1,2,t,th,NodeStart,\"a,b\",3,\"=cmd\""));
    }
}
