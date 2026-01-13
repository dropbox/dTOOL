use crate::helpers::{attr_keys, decode_payload, get_int_attr, get_tenant_id, get_thread_id};
use crate::output::{create_table, print_info, print_success};
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

/// Analyze token costs across executions.
///
/// # Token Estimation (M-503)
///
/// When LLM events don't include actual token counts (INPUT_TOKENS, OUTPUT_TOKENS
/// attributes), this command falls back to a duration-based estimation:
///
/// `estimated_tokens = duration_ms / 2` (split equally between input/output)
///
/// **WARNING:** This estimation is extremely rough and should NOT be used for:
/// - Accurate billing/cost projections
/// - Comparing token efficiency between models
/// - Performance benchmarking
///
/// The heuristic assumes ~1 token/ms which varies wildly based on:
/// - Model (GPT-4 is slower than GPT-3.5)
/// - Network latency and API load
/// - Prompt complexity and batching
///
/// For accurate costs, ensure your LLM provider emits INPUT_TOKENS/OUTPUT_TOKENS
/// in telemetry attributes. Most providers (OpenAI, Anthropic, etc.) include these
/// in their responses.
#[derive(Args)]
pub struct CostsArgs {
    /// Kafka bootstrap servers (comma-separated)
    #[arg(short, long, default_value = "localhost:9092")]
    bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    #[arg(short, long, default_value = "dashstream-events")]
    topic: String,

    /// Thread ID to analyze (optional - analyzes all threads if not specified)
    #[arg(long = "thread", alias = "thread-id")]
    thread_id: Option<String>,

    /// Group by node
    #[arg(long)]
    by_node: bool,

    /// Group by tenant
    #[arg(long)]
    by_tenant: bool,

    /// Cost per 1M input tokens (USD)
    #[arg(long, default_value = "0.25")]
    input_cost_per_million: f64,

    /// Cost per 1M output tokens (USD)
    #[arg(long, default_value = "1.25")]
    output_cost_per_million: f64,
}

#[derive(Debug, Default)]
struct TokenUsage {
    input_tokens: i64,
    output_tokens: i64,
    llm_calls: usize,
    /// M-503: Track how many events used estimation fallback
    estimated_calls: usize,
}

impl TokenUsage {
    fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }

    fn cost(&self, input_cost: f64, output_cost: f64) -> f64 {
        (self.input_tokens as f64 / 1_000_000.0 * input_cost)
            + (self.output_tokens as f64 / 1_000_000.0 * output_cost)
    }
}

pub async fn run(args: CostsArgs) -> Result<()> {
    let filter_msg = if let Some(thread_id) = &args.thread_id {
        format!("thread '{thread_id}'")
    } else {
        "all threads".to_string()
    };

    print_info(&format!(
        "Analyzing token costs for {} from topic '{}'...",
        filter_msg, args.topic
    ));

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&args.bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashflow-cli-costs-{}", uuid::Uuid::new_v4()),
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

    // Collect all LLM events
    let mut events = Vec::new();
    let timeout = SHORT_TIMEOUT;

    loop {
        match tokio::time::timeout(timeout, consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    if let Ok(msg) = decode_payload(payload) {
                        if let Some(dash_stream_message::Message::Event(event)) = msg.message {
                            // Filter by thread if specified
                            if let Some(thread_id) = &args.thread_id {
                                if get_thread_id(&event) != Some(thread_id.as_str()) {
                                    continue;
                                }
                            }

                            // Only collect LLM events
                            if matches!(event.event_type(), EventType::LlmStart | EventType::LlmEnd)
                            {
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
        println!("\n{}", "No LLM events found.".yellow());
        return Ok(());
    }

    print_success(&format!("Found {} LLM events", events.len()));
    println!();

    // Analyze costs
    if args.by_node {
        analyze_by_node(&events, &args)?;
    } else if args.by_tenant {
        analyze_by_tenant(&events, &args)?;
    } else {
        analyze_overall(&events, &args)?;
    }

    Ok(())
}

fn analyze_overall(events: &[Event], args: &CostsArgs) -> Result<()> {
    let usage = calculate_total_usage(events);

    println!("{}", "Overall Token Usage & Costs".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec!["Metric", "Value"]);

    table.add_row(vec!["LLM Calls", &usage.llm_calls.to_string()]);
    table.add_row(vec!["Input Tokens", &format_tokens(usage.input_tokens)]);
    table.add_row(vec!["Output Tokens", &format_tokens(usage.output_tokens)]);
    table.add_row(vec!["Total Tokens", &format_tokens(usage.total_tokens())]);

    let cost = usage.cost(args.input_cost_per_million, args.output_cost_per_million);
    table.add_row(vec!["Estimated Cost", &format!("${cost:.4}")]);

    println!("{table}");

    // Show pricing breakdown
    println!("\n{}", "Pricing".bright_cyan().bold());
    println!("  Input:  ${:.2}/1M tokens", args.input_cost_per_million);
    println!("  Output: ${:.2}/1M tokens", args.output_cost_per_million);

    // M-503: Warn if any events used estimation
    if usage.estimated_calls > 0 {
        println!();
        println!(
            "{}",
            format!(
                "Warning: {}/{} LLM calls used duration-based token estimation.",
                usage.estimated_calls, usage.llm_calls
            )
            .yellow()
        );
        println!(
            "{}",
            "         Estimates are rough (~1 token/ms). For accurate costs, ensure".yellow()
        );
        println!(
            "{}",
            "         your LLM provider emits INPUT_TOKENS/OUTPUT_TOKENS attributes.".yellow()
        );
    }

    Ok(())
}

fn analyze_by_node(events: &[Event], args: &CostsArgs) -> Result<()> {
    let mut node_usage: HashMap<String, TokenUsage> = HashMap::new();

    for event in events {
        if !event.node_id.is_empty() {
            let usage = node_usage.entry(event.node_id.clone()).or_default();

            if event.event_type() == EventType::LlmEnd {
                usage.llm_calls += 1;

                // M-503: Try to get actual token counts from attributes, otherwise estimate from duration
                let mut used_estimate = false;
                if let Some(input_tokens) = get_int_attr(event, attr_keys::INPUT_TOKENS) {
                    usage.input_tokens += input_tokens;
                } else {
                    // M-503: Fallback estimation (~1 token/ms, split 50/50)
                    let estimated_tokens = event.duration_us / 1000;
                    usage.input_tokens += estimated_tokens / 2;
                    used_estimate = true;
                }

                if let Some(output_tokens) = get_int_attr(event, attr_keys::OUTPUT_TOKENS) {
                    usage.output_tokens += output_tokens;
                } else {
                    // M-503: Fallback estimation (~1 token/ms, split 50/50)
                    let estimated_tokens = event.duration_us / 1000;
                    usage.output_tokens += estimated_tokens / 2;
                    used_estimate = true;
                }

                if used_estimate {
                    usage.estimated_calls += 1;
                }
            }
        }
    }

    println!("{}", "Token Usage & Costs by Node".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec![
        "Node",
        "LLM Calls",
        "Input Tokens",
        "Output Tokens",
        "Total Tokens",
        "Cost",
    ]);

    // Sort by cost (descending)
    let mut node_vec: Vec<_> = node_usage.iter().collect();
    node_vec.sort_by(|a, b| {
        let cost_a =
            a.1.cost(args.input_cost_per_million, args.output_cost_per_million);
        let cost_b =
            b.1.cost(args.input_cost_per_million, args.output_cost_per_million);
        cost_b
            .partial_cmp(&cost_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (node_name, usage) in node_vec {
        let cost = usage.cost(args.input_cost_per_million, args.output_cost_per_million);
        table.add_row(vec![
            node_name.clone(),
            usage.llm_calls.to_string(),
            format_tokens(usage.input_tokens),
            format_tokens(usage.output_tokens),
            format_tokens(usage.total_tokens()),
            format!("${:.4}", cost),
        ]);
    }

    println!("{table}");

    Ok(())
}

fn analyze_by_tenant(events: &[Event], args: &CostsArgs) -> Result<()> {
    let mut tenant_usage: HashMap<String, TokenUsage> = HashMap::new();

    for event in events {
        if let Some(tenant_id) = get_tenant_id(event) {
            let usage = tenant_usage.entry(tenant_id.to_string()).or_default();

            if event.event_type() == EventType::LlmEnd {
                usage.llm_calls += 1;

                // M-503: Try to get actual token counts from attributes, otherwise estimate from duration
                let mut used_estimate = false;
                if let Some(input_tokens) = get_int_attr(event, attr_keys::INPUT_TOKENS) {
                    usage.input_tokens += input_tokens;
                } else {
                    // M-503: Fallback estimation (~1 token/ms, split 50/50)
                    let estimated_tokens = event.duration_us / 1000;
                    usage.input_tokens += estimated_tokens / 2;
                    used_estimate = true;
                }

                if let Some(output_tokens) = get_int_attr(event, attr_keys::OUTPUT_TOKENS) {
                    usage.output_tokens += output_tokens;
                } else {
                    // M-503: Fallback estimation (~1 token/ms, split 50/50)
                    let estimated_tokens = event.duration_us / 1000;
                    usage.output_tokens += estimated_tokens / 2;
                    used_estimate = true;
                }

                if used_estimate {
                    usage.estimated_calls += 1;
                }
            }
        }
    }

    println!("{}", "Token Usage & Costs by Tenant".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec![
        "Tenant ID",
        "LLM Calls",
        "Input Tokens",
        "Output Tokens",
        "Total Tokens",
        "Cost",
    ]);

    // Sort by cost (descending)
    let mut tenant_vec: Vec<_> = tenant_usage.iter().collect();
    tenant_vec.sort_by(|a, b| {
        let cost_a =
            a.1.cost(args.input_cost_per_million, args.output_cost_per_million);
        let cost_b =
            b.1.cost(args.input_cost_per_million, args.output_cost_per_million);
        cost_b
            .partial_cmp(&cost_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (tenant_id, usage) in tenant_vec {
        let cost = usage.cost(args.input_cost_per_million, args.output_cost_per_million);
        table.add_row(vec![
            tenant_id.clone(),
            usage.llm_calls.to_string(),
            format_tokens(usage.input_tokens),
            format_tokens(usage.output_tokens),
            format_tokens(usage.total_tokens()),
            format!("${:.4}", cost),
        ]);
    }

    println!("{table}");

    Ok(())
}

fn calculate_total_usage(events: &[Event]) -> TokenUsage {
    let mut usage = TokenUsage::default();

    for event in events {
        if event.event_type() == EventType::LlmEnd {
            usage.llm_calls += 1;

            // M-503: Try to get actual token counts from attributes, otherwise estimate from duration
            let mut used_estimate = false;
            if let Some(input_tokens) = get_int_attr(event, attr_keys::INPUT_TOKENS) {
                usage.input_tokens += input_tokens;
            } else {
                // M-503: Fallback estimation (~1 token/ms, split 50/50)
                let estimated_tokens = event.duration_us / 1000;
                usage.input_tokens += estimated_tokens / 2;
                used_estimate = true;
            }

            if let Some(output_tokens) = get_int_attr(event, attr_keys::OUTPUT_TOKENS) {
                usage.output_tokens += output_tokens;
            } else {
                // M-503: Fallback estimation (~1 token/ms, split 50/50)
                let estimated_tokens = event.duration_us / 1000;
                usage.output_tokens += estimated_tokens / 2;
                used_estimate = true;
            }

            if used_estimate {
                usage.estimated_calls += 1;
            }
        }
    }

    usage
}

fn format_tokens(tokens: i64) -> String {
    if tokens < 1_000 {
        tokens.to_string()
    } else if tokens < 1_000_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{:.2}M", tokens as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TokenUsage tests
    // ========================================================================

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.llm_calls, 0);
        assert_eq!(usage.estimated_calls, 0);
    }

    #[test]
    fn test_token_usage_total_tokens() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            llm_calls: 1,
            estimated_calls: 0,
        };
        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn test_token_usage_total_tokens_zero() {
        let usage = TokenUsage::default();
        assert_eq!(usage.total_tokens(), 0);
    }

    #[test]
    fn test_token_usage_cost_basic() {
        let usage = TokenUsage {
            input_tokens: 1_000_000, // 1M input tokens
            output_tokens: 500_000,  // 0.5M output tokens
            llm_calls: 10,
            estimated_calls: 0,
        };
        // Cost = 1M * 0.25/1M + 0.5M * 1.25/1M = 0.25 + 0.625 = 0.875
        let cost = usage.cost(0.25, 1.25);
        assert!((cost - 0.875).abs() < 0.0001);
    }

    #[test]
    fn test_token_usage_cost_zero() {
        let usage = TokenUsage::default();
        let cost = usage.cost(0.25, 1.25);
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_usage_cost_input_only() {
        let usage = TokenUsage {
            input_tokens: 2_000_000, // 2M input tokens
            output_tokens: 0,
            llm_calls: 5,
            estimated_calls: 0,
        };
        // Cost = 2M * 0.25/1M = 0.50
        let cost = usage.cost(0.25, 1.25);
        assert!((cost - 0.5).abs() < 0.0001);
    }

    #[test]
    fn test_token_usage_cost_output_only() {
        let usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 1_000_000, // 1M output tokens
            llm_calls: 5,
            estimated_calls: 0,
        };
        // Cost = 1M * 1.25/1M = 1.25
        let cost = usage.cost(0.25, 1.25);
        assert!((cost - 1.25).abs() < 0.0001);
    }

    #[test]
    fn test_token_usage_cost_custom_rates() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            llm_calls: 1,
            estimated_calls: 0,
        };
        // With custom rates: 0.5/1M input, 2.0/1M output
        // Cost = 1M * 0.5/1M + 1M * 2.0/1M = 0.5 + 2.0 = 2.5
        let cost = usage.cost(0.5, 2.0);
        assert!((cost - 2.5).abs() < 0.0001);
    }

    // ========================================================================
    // format_tokens tests
    // ========================================================================

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(1), "1");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1.0K");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(10_000), "10.0K");
        assert_eq!(format_tokens(999_999), "1000.0K");
    }

    #[test]
    fn test_format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1.00M");
        assert_eq!(format_tokens(2_500_000), "2.50M");
        assert_eq!(format_tokens(10_000_000), "10.00M");
    }

    #[test]
    fn test_format_tokens_boundary_cases() {
        // Just under 1K
        assert_eq!(format_tokens(999), "999");
        // Exactly 1K
        assert_eq!(format_tokens(1_000), "1.0K");
        // Just under 1M
        assert_eq!(format_tokens(999_999), "1000.0K");
        // Exactly 1M
        assert_eq!(format_tokens(1_000_000), "1.00M");
    }

    #[test]
    fn test_format_tokens_negative() {
        // Negative tokens (edge case, shouldn't happen in practice)
        // Note: function uses < comparison, so negative values always go to first branch
        assert_eq!(format_tokens(-100), "-100");
        assert_eq!(format_tokens(-1_500), "-1500"); // < 1000 is true for negatives
    }

    // ========================================================================
    // TokenUsage Debug trait test
    // ========================================================================

    #[test]
    fn test_token_usage_debug() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 200,
            llm_calls: 5,
            estimated_calls: 2,
        };
        let debug = format!("{:?}", usage);
        assert!(debug.contains("TokenUsage"));
        assert!(debug.contains("100"));
        assert!(debug.contains("200"));
    }
}
