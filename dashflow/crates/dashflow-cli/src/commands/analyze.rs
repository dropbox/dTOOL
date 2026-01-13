// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Offline analysis of exported streaming events.
//!
//! This command provides profile, cost, and flamegraph analysis from
//! exported JSON files, eliminating the Kafka dependency for offline analysis.

use crate::output::{create_table, format_duration, print_info, print_success};
use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Args, Subcommand};
use colored::Colorize;
use inferno::flamegraph::{self, Options};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Cursor, Write as IoWrite};

/// Analyze exported streaming data offline (no Kafka required)
#[derive(Args)]
pub struct AnalyzeArgs {
    #[command(subcommand)]
    pub command: AnalyzeCommand,
}

#[derive(Subcommand)]
pub enum AnalyzeCommand {
    /// Profile execution performance from exported JSON
    Profile(AnalyzeProfileArgs),

    /// Analyze token costs from exported JSON
    Costs(AnalyzeCostsArgs),

    /// Generate flamegraph from exported JSON
    Flamegraph(AnalyzeFlamegraphArgs),

    /// Show summary statistics from exported JSON
    Summary(AnalyzeSummaryArgs),

    /// Generate interactive HTML dashboard from exported JSON
    Dashboard(AnalyzeDashboardArgs),
}

/// Profile execution performance from exported JSON
#[derive(Args)]
pub struct AnalyzeProfileArgs {
    /// Path to exported JSON file
    #[arg(short, long)]
    input: String,

    /// Show detailed breakdown (min/max/median/p95)
    #[arg(long)]
    detailed: bool,

    /// Show top N slowest operations
    #[arg(long, default_value = "10")]
    top: usize,
}

/// Analyze token costs from exported JSON
#[derive(Args)]
pub struct AnalyzeCostsArgs {
    /// Path to exported JSON file
    #[arg(short, long)]
    input: String,

    /// Group by node
    #[arg(long)]
    by_node: bool,

    /// Cost per 1M input tokens (USD)
    #[arg(long, default_value = "0.25")]
    input_cost_per_million: f64,

    /// Cost per 1M output tokens (USD)
    #[arg(long, default_value = "1.25")]
    output_cost_per_million: f64,
}

/// Generate flamegraph from exported JSON
#[derive(Args)]
pub struct AnalyzeFlamegraphArgs {
    /// Path to exported JSON file
    #[arg(short, long)]
    input: String,

    /// Output file for flamegraph
    #[arg(short, long, default_value = "flamegraph.svg")]
    output: String,

    /// Output format (folded, svg)
    #[arg(long, default_value = "svg")]
    format: String,

    /// Chart title
    #[arg(long)]
    title: Option<String>,
}

/// Show summary statistics from exported JSON
#[derive(Args)]
pub struct AnalyzeSummaryArgs {
    /// Path to exported JSON file
    #[arg(short, long)]
    input: String,

    /// Show detailed event breakdown
    #[arg(long)]
    verbose: bool,
}

/// Generate interactive HTML dashboard from exported JSON
#[derive(Args)]
pub struct AnalyzeDashboardArgs {
    /// Path to exported JSON file
    #[arg(short, long)]
    input: String,

    /// Output file for HTML dashboard
    #[arg(short, long, default_value = "dashboard.html")]
    output: String,

    /// Chart title
    #[arg(long)]
    title: Option<String>,

    /// Cost per 1M input tokens (USD)
    #[arg(long, default_value = "0.25")]
    input_cost_per_million: f64,

    /// Cost per 1M output tokens (USD)
    #[arg(long, default_value = "1.25")]
    output_cost_per_million: f64,

    /// Open dashboard in browser after generation
    #[arg(long)]
    open: bool,
}

// Data structures matching export.rs format
#[derive(Debug, Deserialize, Serialize)]
struct ExportOutput {
    thread_id: String,
    total_events: usize,
    start_time: i64,
    end_time: i64,
    duration_micros: i64,
    events: Vec<ExportedEvent>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)] // Deserialize: Event fields from analyze_events JSON export
struct ExportedEvent {
    #[serde(default)]
    message_id: String,
    #[serde(default)]
    sequence: u64,
    timestamp_micros: i64,
    #[serde(default)]
    tenant_id: String,
    #[serde(default)]
    thread_id: String,
    event_type: String,
    #[serde(default)]
    node_id: String,
    #[serde(default)]
    duration_us: i64,
    #[serde(default)]
    llm_request_id: String,
    #[serde(default)]
    attributes: serde_json::Value,
}

// Profile statistics
#[derive(Debug)]
struct NodeProfile {
    executions: usize,
    total_duration: i64,
    min_duration: i64,
    max_duration: i64,
    durations: Vec<i64>,
}

impl Default for NodeProfile {
    fn default() -> Self {
        Self {
            executions: 0,
            total_duration: 0,
            min_duration: i64::MAX,
            max_duration: 0,
            durations: Vec::new(),
        }
    }
}

impl NodeProfile {
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

// Token usage tracking
#[derive(Debug, Default)]
struct TokenUsage {
    input_tokens: i64,
    output_tokens: i64,
    llm_calls: usize,
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

pub async fn run(args: AnalyzeArgs) -> Result<()> {
    match args.command {
        AnalyzeCommand::Profile(profile_args) => run_profile(profile_args).await,
        AnalyzeCommand::Costs(costs_args) => run_costs(costs_args).await,
        AnalyzeCommand::Flamegraph(flamegraph_args) => run_flamegraph(flamegraph_args).await,
        AnalyzeCommand::Summary(summary_args) => run_summary(summary_args).await,
        AnalyzeCommand::Dashboard(dashboard_args) => run_dashboard(dashboard_args).await,
    }
}

fn load_export_file(path: &str) -> Result<ExportOutput> {
    print_info(&format!("Loading exported data from '{}'...", path));

    let file = File::open(path).context(format!("Failed to open file: {}", path))?;
    let reader = BufReader::new(file);
    let data: ExportOutput =
        serde_json::from_reader(reader).context("Failed to parse JSON export file")?;

    print_success(&format!(
        "Loaded {} events for thread '{}'",
        data.total_events, data.thread_id
    ));

    Ok(data)
}

async fn run_profile(args: AnalyzeProfileArgs) -> Result<()> {
    let input = args.input.clone();
    let data = tokio::task::spawn_blocking(move || load_export_file(&input)).await??;

    if data.events.is_empty() {
        anyhow::bail!("No events found in export file");
    }

    // Build performance profile
    let profiles = build_profile(&data.events);

    // Display overall statistics
    println!();
    display_overall_stats(&data);

    // Display node performance
    println!();
    display_node_performance(&profiles, args.top, args.detailed);

    // Display edge traversals
    println!();
    display_edge_performance(&data.events);

    Ok(())
}

fn build_profile(events: &[ExportedEvent]) -> HashMap<String, NodeProfile> {
    let mut profiles: HashMap<String, NodeProfile> = HashMap::new();
    let mut node_starts: HashMap<String, i64> = HashMap::new();

    for event in events {
        if !event.node_id.is_empty() {
            match event.event_type.as_str() {
                "NodeStart" => {
                    node_starts.insert(event.node_id.clone(), event.timestamp_micros);
                }
                "NodeEnd" => {
                    if let Some(start_time) = node_starts.remove(&event.node_id) {
                        let duration = event.timestamp_micros - start_time;
                        profiles
                            .entry(event.node_id.clone())
                            .or_default()
                            .add_execution(duration);
                    }
                }
                _ => {}
            }
        }
    }

    profiles
}

fn display_overall_stats(data: &ExportOutput) {
    println!("{}", "Overall Performance".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec!["Metric", "Value"]);

    table.add_row(vec!["Thread ID", &data.thread_id]);
    table.add_row(vec!["Total Events", &data.total_events.to_string()]);
    table.add_row(vec![
        "Total Duration",
        &format_duration(data.duration_micros),
    ]);

    // Count event types
    let node_ends = data
        .events
        .iter()
        .filter(|e| e.event_type == "NodeEnd")
        .count();
    let edges = data
        .events
        .iter()
        .filter(|e| e.event_type == "EdgeTraversal")
        .count();
    let errors = data
        .events
        .iter()
        .filter(|e| e.event_type == "NodeError")
        .count();
    let llm_calls = data
        .events
        .iter()
        .filter(|e| e.event_type == "LlmEnd")
        .count();

    table.add_row(vec!["Nodes Executed", &node_ends.to_string()]);
    table.add_row(vec!["Edges Traversed", &edges.to_string()]);
    table.add_row(vec!["LLM Calls", &llm_calls.to_string()]);
    table.add_row(vec!["Errors", &errors.to_string()]);

    println!("{table}");
}

fn display_node_performance(profiles: &HashMap<String, NodeProfile>, top: usize, detailed: bool) {
    if profiles.is_empty() {
        return;
    }

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

fn display_edge_performance(events: &[ExportedEvent]) {
    let mut edge_counts: HashMap<(String, String), usize> = HashMap::new();

    for event in events {
        if event.event_type == "EdgeTraversal" {
            if let (Some(from), Some(to)) = (
                event.attributes.get("edge_from").and_then(|v| v.as_str()),
                event.attributes.get("edge_to").and_then(|v| v.as_str()),
            ) {
                *edge_counts
                    .entry((from.to_string(), to.to_string()))
                    .or_insert(0) += 1;
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

async fn run_costs(args: AnalyzeCostsArgs) -> Result<()> {
    let input = args.input.clone();
    let data = tokio::task::spawn_blocking(move || load_export_file(&input)).await??;

    if data.events.is_empty() {
        anyhow::bail!("No events found in export file");
    }

    println!();

    if args.by_node {
        analyze_costs_by_node(&data.events, &args)?;
    } else {
        analyze_costs_overall(&data.events, &args)?;
    }

    Ok(())
}

fn analyze_costs_overall(events: &[ExportedEvent], args: &AnalyzeCostsArgs) -> Result<()> {
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
    table.add_row(vec!["Estimated Cost", &format!("${:.4}", cost)]);

    println!("{table}");

    // Show pricing breakdown
    println!("\n{}", "Pricing".bright_cyan().bold());
    println!("  Input:  ${:.2}/1M tokens", args.input_cost_per_million);
    println!("  Output: ${:.2}/1M tokens", args.output_cost_per_million);

    Ok(())
}

fn analyze_costs_by_node(events: &[ExportedEvent], args: &AnalyzeCostsArgs) -> Result<()> {
    let mut node_usage: HashMap<String, TokenUsage> = HashMap::new();

    for event in events {
        if !event.node_id.is_empty() && event.event_type == "LlmEnd" {
            let usage = node_usage.entry(event.node_id.clone()).or_default();
            usage.llm_calls += 1;

            // Try to get actual token counts from attributes
            if let Some(input_tokens) = event
                .attributes
                .get("input_tokens")
                .and_then(|v| v.as_i64())
            {
                usage.input_tokens += input_tokens;
            } else {
                // Estimate from duration
                let estimated = event.duration_us / 1000;
                usage.input_tokens += estimated / 2;
            }

            if let Some(output_tokens) = event
                .attributes
                .get("output_tokens")
                .and_then(|v| v.as_i64())
            {
                usage.output_tokens += output_tokens;
            } else {
                let estimated = event.duration_us / 1000;
                usage.output_tokens += estimated / 2;
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
        // Use Ordering::Equal for NaN comparisons to avoid panic
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

fn calculate_total_usage(events: &[ExportedEvent]) -> TokenUsage {
    let mut usage = TokenUsage::default();

    for event in events {
        if event.event_type == "LlmEnd" {
            usage.llm_calls += 1;

            if let Some(input_tokens) = event
                .attributes
                .get("input_tokens")
                .and_then(|v| v.as_i64())
            {
                usage.input_tokens += input_tokens;
            } else {
                let estimated = event.duration_us / 1000;
                usage.input_tokens += estimated / 2;
            }

            if let Some(output_tokens) = event
                .attributes
                .get("output_tokens")
                .and_then(|v| v.as_i64())
            {
                usage.output_tokens += output_tokens;
            } else {
                let estimated = event.duration_us / 1000;
                usage.output_tokens += estimated / 2;
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

async fn run_flamegraph(args: AnalyzeFlamegraphArgs) -> Result<()> {
    let input = args.input.clone();
    let data = tokio::task::spawn_blocking(move || load_export_file(&input)).await??;

    if data.events.is_empty() {
        anyhow::bail!("No events found in export file");
    }

    // Build call stack from events
    let folded = build_folded_stacks(&data.events);

    if folded.is_empty() {
        anyhow::bail!("No timing data found for flamegraph generation");
    }

    let title = args
        .title
        .unwrap_or_else(|| format!("DashFlow Flamegraph - Thread {}", data.thread_id));

    match args.format.as_str() {
        "folded" => {
            // Wrap blocking file I/O in spawn_blocking
            let output_path = args.output.clone();
            let folded_data = folded.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                let mut file = File::create(&output_path)
                    .context(format!("Failed to create output file: {}", output_path))?;
                file.write_all(folded_data.as_bytes())
                    .context("Failed to write flamegraph data")?;
                Ok(())
            })
            .await
            .context("spawn_blocking panicked")??;

            print_success(&format!("Folded stack data written to {}", args.output));
            println!("\nTo generate SVG flamegraph, run:");
            println!("  inferno-flamegraph {} > flamegraph.svg", args.output);
        }
        "svg" => {
            // Generate SVG directly using inferno
            let mut svg_output = Vec::new();
            let mut options = Options::default();
            options.title = title;
            options.count_name = "microseconds".to_string();

            let reader = BufReader::new(Cursor::new(folded.as_bytes()));

            flamegraph::from_reader(&mut options, reader, &mut svg_output)
                .context("Failed to generate SVG flamegraph")?;

            // Wrap blocking file I/O in spawn_blocking
            let output_path = args.output.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                let mut file = File::create(&output_path)
                    .context(format!("Failed to create output file: {}", output_path))?;
                file.write_all(&svg_output)
                    .context("Failed to write SVG flamegraph")?;
                Ok(())
            })
            .await
            .context("spawn_blocking panicked")??;

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

fn build_folded_stacks(events: &[ExportedEvent]) -> String {
    let mut output = String::new();
    let mut frame_durations: HashMap<String, i64> = HashMap::new();
    let mut node_starts: HashMap<String, i64> = HashMap::new();

    for event in events {
        match event.event_type.as_str() {
            "NodeStart" => {
                if !event.node_id.is_empty() {
                    node_starts.insert(event.node_id.clone(), event.timestamp_micros);
                }
            }
            "NodeEnd" => {
                if !event.node_id.is_empty() {
                    if let Some(start_time) = node_starts.remove(&event.node_id) {
                        let duration = event.timestamp_micros - start_time;
                        *frame_durations.entry(event.node_id.clone()).or_insert(0) += duration;
                    }
                }
            }
            "LlmStart" => {
                if !event.node_id.is_empty() {
                    let name = format!("{}::LLM", event.node_id);
                    node_starts.insert(name, event.timestamp_micros);
                }
            }
            "LlmEnd" => {
                if !event.node_id.is_empty() {
                    let name = format!("{}::LLM", event.node_id);
                    if let Some(start_time) = node_starts.remove(&name) {
                        let duration = event.timestamp_micros - start_time;
                        *frame_durations.entry(name).or_insert(0) += duration;
                    }
                }
            }
            "ToolStart" => {
                if let Some(tool_name) = event.attributes.get("tool_name").and_then(|v| v.as_str())
                {
                    let name = format!("Tool::{}", tool_name);
                    node_starts.insert(name, event.timestamp_micros);
                }
            }
            "ToolEnd" => {
                if let Some(tool_name) = event.attributes.get("tool_name").and_then(|v| v.as_str())
                {
                    let name = format!("Tool::{}", tool_name);
                    if let Some(start_time) = node_starts.remove(&name) {
                        let duration = event.timestamp_micros - start_time;
                        *frame_durations.entry(name).or_insert(0) += duration;
                    }
                }
            }
            _ => {}
        }
    }

    // Generate folded stack format
    for (name, duration) in &frame_durations {
        output.push_str(&format!("{} {}\n", name, duration));
    }

    output
}

async fn run_summary(args: AnalyzeSummaryArgs) -> Result<()> {
    let input = args.input.clone();
    let data = tokio::task::spawn_blocking(move || load_export_file(&input)).await??;

    println!();
    println!("{}", "Export Summary".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    let mut table = create_table();
    table.set_header(vec!["Metric", "Value"]);

    table.add_row(vec!["Thread ID", &data.thread_id]);
    table.add_row(vec!["Total Events", &data.total_events.to_string()]);
    table.add_row(vec!["Start Time", &format!("{}μs", data.start_time)]);
    table.add_row(vec!["End Time", &format!("{}μs", data.end_time)]);
    table.add_row(vec!["Duration", &format_duration(data.duration_micros)]);

    println!("{table}");

    if args.verbose {
        // Event type breakdown
        let mut event_types: HashMap<String, usize> = HashMap::new();
        for event in &data.events {
            *event_types.entry(event.event_type.clone()).or_insert(0) += 1;
        }

        println!();
        println!("{}", "Event Type Breakdown".bright_cyan().bold());
        println!("{}", "═".repeat(80).bright_cyan());

        let mut event_table = create_table();
        event_table.set_header(vec!["Event Type", "Count", "Percentage"]);

        let total = data.events.len() as f64;
        let mut sorted_types: Vec<_> = event_types.iter().collect();
        sorted_types.sort_by(|a, b| b.1.cmp(a.1));

        for (event_type, count) in sorted_types {
            let pct = (*count as f64 / total) * 100.0;
            event_table.add_row(vec![
                event_type.clone(),
                count.to_string(),
                format!("{:.1}%", pct),
            ]);
        }

        println!("{event_table}");

        // Node breakdown
        let mut nodes: HashMap<String, usize> = HashMap::new();
        for event in &data.events {
            if !event.node_id.is_empty() {
                *nodes.entry(event.node_id.clone()).or_insert(0) += 1;
            }
        }

        if !nodes.is_empty() {
            println!();
            println!("{}", "Node Activity".bright_cyan().bold());
            println!("{}", "═".repeat(80).bright_cyan());

            let mut node_table = create_table();
            node_table.set_header(vec!["Node ID", "Events"]);

            let mut sorted_nodes: Vec<_> = nodes.iter().collect();
            sorted_nodes.sort_by(|a, b| b.1.cmp(a.1));

            for (node_id, count) in sorted_nodes.iter().take(20) {
                node_table.add_row(vec![(*node_id).clone(), count.to_string()]);
            }

            println!("{node_table}");
        }
    }

    Ok(())
}

async fn run_dashboard(args: AnalyzeDashboardArgs) -> Result<()> {
    let input = args.input.clone();
    let data = tokio::task::spawn_blocking(move || load_export_file(&input)).await??;

    if data.events.is_empty() {
        anyhow::bail!("No events found in export file");
    }

    let title = args
        .title
        .clone()
        .unwrap_or_else(|| format!("DashFlow Dashboard - Thread {}", data.thread_id));

    // Build analysis data
    let profiles = build_profile(&data.events);
    let total_usage = calculate_total_usage(&data.events);
    let node_costs = calculate_node_costs(&data.events, &args);

    // Generate HTML dashboard
    let html = generate_dashboard_html(&title, &data, &profiles, &total_usage, &node_costs, &args);

    // Write to file - wrap blocking I/O in spawn_blocking
    let output_path = args.output.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut file = File::create(&output_path)
            .context(format!("Failed to create output file: {}", output_path))?;
        file.write_all(html.as_bytes())
            .context("Failed to write dashboard HTML")?;
        Ok(())
    })
    .await
    .context("spawn_blocking panicked")??;

    print_success(&format!("Dashboard written to {}", args.output));

    if args.open {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&args.output).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(&args.output)
                .spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(&args.output)
                .spawn();
        }
    }

    Ok(())
}

fn calculate_node_costs(
    events: &[ExportedEvent],
    args: &AnalyzeDashboardArgs,
) -> Vec<(String, f64)> {
    let mut node_usage: HashMap<String, TokenUsage> = HashMap::new();

    for event in events {
        if !event.node_id.is_empty() && event.event_type == "LlmEnd" {
            let usage = node_usage.entry(event.node_id.clone()).or_default();
            usage.llm_calls += 1;

            if let Some(input_tokens) = event
                .attributes
                .get("input_tokens")
                .and_then(|v| v.as_i64())
            {
                usage.input_tokens += input_tokens;
            } else {
                let estimated = event.duration_us / 1000;
                usage.input_tokens += estimated / 2;
            }

            if let Some(output_tokens) = event
                .attributes
                .get("output_tokens")
                .and_then(|v| v.as_i64())
            {
                usage.output_tokens += output_tokens;
            } else {
                let estimated = event.duration_us / 1000;
                usage.output_tokens += estimated / 2;
            }
        }
    }

    let mut costs: Vec<_> = node_usage
        .into_iter()
        .map(|(node, usage)| {
            let cost = usage.cost(args.input_cost_per_million, args.output_cost_per_million);
            (node, cost)
        })
        .collect();

    // Use Ordering::Equal for NaN comparisons to avoid panic
    costs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    costs
}

fn generate_dashboard_html(
    title: &str,
    data: &ExportOutput,
    profiles: &HashMap<String, NodeProfile>,
    total_usage: &TokenUsage,
    node_costs: &[(String, f64)],
    args: &AnalyzeDashboardArgs,
) -> String {
    // Sort profiles by total duration
    let mut sorted_profiles: Vec<_> = profiles.iter().collect();
    sorted_profiles.sort_by(|a, b| b.1.total_duration.cmp(&a.1.total_duration));

    // Build JSON data for charts
    let perf_labels: Vec<_> = sorted_profiles
        .iter()
        .take(10)
        .map(|(k, _)| format!("\"{}\"", k))
        .collect();
    let perf_data: Vec<_> = sorted_profiles
        .iter()
        .take(10)
        .map(|(_, v)| v.total_duration / 1000)
        .collect();
    let perf_avg_data: Vec<_> = sorted_profiles
        .iter()
        .take(10)
        .map(|(_, v)| v.avg_duration() / 1000)
        .collect();

    let cost_labels: Vec<_> = node_costs
        .iter()
        .take(10)
        .map(|(k, _)| format!("\"{}\"", k))
        .collect();
    let cost_data: Vec<_> = node_costs
        .iter()
        .take(10)
        .map(|(_, v)| format!("{:.4}", v))
        .collect();

    // Build event timeline data
    let mut timeline_events: Vec<String> = Vec::new();
    for event in data.events.iter().take(100) {
        let ts = (event.timestamp_micros - data.start_time) / 1000; // ms offset
        let color = match event.event_type.as_str() {
            "NodeStart" | "NodeEnd" => "#3b82f6",
            "LlmStart" | "LlmEnd" => "#f59e0b",
            "ToolStart" | "ToolEnd" => "#10b981",
            "EdgeTraversal" => "#8b5cf6",
            _ if event.event_type.contains("Error") => "#ef4444",
            _ => "#6b7280",
        };
        let node_label = if event.node_id.is_empty() {
            event.event_type.clone()
        } else {
            format!("{}: {}", event.event_type, event.node_id)
        };
        timeline_events.push(format!(
            "{{x: {}, y: 1, label: \"{}\", color: \"{}\"}}",
            ts, node_label, color
        ));
    }

    // Event type breakdown
    let mut event_types: HashMap<String, usize> = HashMap::new();
    for event in &data.events {
        *event_types.entry(event.event_type.clone()).or_insert(0) += 1;
    }
    let mut sorted_event_types: Vec<_> = event_types.iter().collect();
    sorted_event_types.sort_by(|a, b| b.1.cmp(a.1));
    let event_labels: Vec<_> = sorted_event_types
        .iter()
        .take(8)
        .map(|(k, _)| format!("\"{}\"", k))
        .collect();
    let event_counts: Vec<_> = sorted_event_types
        .iter()
        .take(8)
        .map(|(_, v)| v.to_string())
        .collect();

    let total_cost = total_usage.cost(args.input_cost_per_million, args.output_cost_per_million);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: linear-gradient(135deg, #1e1e2e 0%, #2d2d44 100%);
            color: #e0e0e0;
            min-height: 100vh;
            padding: 20px;
        }}
        .container {{ max-width: 1400px; margin: 0 auto; }}
        h1 {{
            text-align: center;
            margin-bottom: 30px;
            font-size: 2em;
            background: linear-gradient(90deg, #60a5fa, #a78bfa);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }}
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        .stat-card {{
            background: rgba(255,255,255,0.05);
            border-radius: 12px;
            padding: 20px;
            text-align: center;
            border: 1px solid rgba(255,255,255,0.1);
        }}
        .stat-card h3 {{ color: #9ca3af; font-size: 0.9em; margin-bottom: 8px; }}
        .stat-card .value {{ font-size: 1.8em; font-weight: bold; color: #60a5fa; }}
        .stat-card .value.cost {{ color: #10b981; }}
        .stat-card .value.error {{ color: #ef4444; }}
        .charts-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(450px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        .chart-card {{
            background: rgba(255,255,255,0.05);
            border-radius: 12px;
            padding: 20px;
            border: 1px solid rgba(255,255,255,0.1);
        }}
        .chart-card h2 {{
            color: #e0e0e0;
            margin-bottom: 15px;
            font-size: 1.2em;
            display: flex;
            align-items: center;
            gap: 8px;
        }}
        .chart-card h2::before {{
            content: '';
            width: 4px;
            height: 20px;
            background: #60a5fa;
            border-radius: 2px;
        }}
        canvas {{ max-height: 300px; }}
        .timeline-section {{
            background: rgba(255,255,255,0.05);
            border-radius: 12px;
            padding: 20px;
            border: 1px solid rgba(255,255,255,0.1);
            margin-bottom: 30px;
        }}
        .timeline-section h2 {{
            color: #e0e0e0;
            margin-bottom: 15px;
            font-size: 1.2em;
            display: flex;
            align-items: center;
            gap: 8px;
        }}
        .timeline-section h2::before {{
            content: '';
            width: 4px;
            height: 20px;
            background: #a78bfa;
            border-radius: 2px;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            margin-top: 15px;
        }}
        th, td {{
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid rgba(255,255,255,0.1);
        }}
        th {{ color: #9ca3af; font-weight: 500; }}
        tr:hover {{ background: rgba(255,255,255,0.03); }}
        .bar {{
            background: linear-gradient(90deg, #60a5fa, #3b82f6);
            height: 20px;
            border-radius: 4px;
            min-width: 4px;
        }}
        .footer {{
            text-align: center;
            padding: 20px;
            color: #6b7280;
            font-size: 0.9em;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>{title}</h1>

        <div class="stats-grid">
            <div class="stat-card">
                <h3>Thread ID</h3>
                <div class="value" style="font-size: 1em; word-break: break-all;">{thread_id}</div>
            </div>
            <div class="stat-card">
                <h3>Total Events</h3>
                <div class="value">{total_events}</div>
            </div>
            <div class="stat-card">
                <h3>Duration</h3>
                <div class="value">{duration}</div>
            </div>
            <div class="stat-card">
                <h3>LLM Calls</h3>
                <div class="value">{llm_calls}</div>
            </div>
            <div class="stat-card">
                <h3>Total Tokens</h3>
                <div class="value">{total_tokens}</div>
            </div>
            <div class="stat-card">
                <h3>Estimated Cost</h3>
                <div class="value cost">${cost:.4}</div>
            </div>
        </div>

        <div class="charts-grid">
            <div class="chart-card">
                <h2>Node Execution Time (ms)</h2>
                <canvas id="perfChart"></canvas>
            </div>
            <div class="chart-card">
                <h2>Cost by Node ($)</h2>
                <canvas id="costChart"></canvas>
            </div>
            <div class="chart-card">
                <h2>Event Distribution</h2>
                <canvas id="eventChart"></canvas>
            </div>
            <div class="chart-card">
                <h2>Avg vs Total Duration (ms)</h2>
                <canvas id="avgChart"></canvas>
            </div>
        </div>

        <div class="timeline-section">
            <h2>Node Performance Details</h2>
            <table>
                <thead>
                    <tr>
                        <th>Node</th>
                        <th>Executions</th>
                        <th>Total Time</th>
                        <th>Avg Time</th>
                        <th>% of Total</th>
                        <th>Visual</th>
                    </tr>
                </thead>
                <tbody>
                    {table_rows}
                </tbody>
            </table>
        </div>

        <div class="footer">
            Generated by DashFlow CLI &bull; {timestamp}
        </div>
    </div>

    <script>
        const chartColors = {{
            blue: 'rgba(96, 165, 250, 0.8)',
            green: 'rgba(16, 185, 129, 0.8)',
            purple: 'rgba(167, 139, 250, 0.8)',
            yellow: 'rgba(245, 158, 11, 0.8)',
            red: 'rgba(239, 68, 68, 0.8)',
            gray: 'rgba(107, 114, 128, 0.8)',
        }};

        const chartOptions = {{
            responsive: true,
            maintainAspectRatio: true,
            plugins: {{
                legend: {{ labels: {{ color: '#9ca3af' }} }}
            }},
            scales: {{
                x: {{ ticks: {{ color: '#9ca3af' }}, grid: {{ color: 'rgba(255,255,255,0.05)' }} }},
                y: {{ ticks: {{ color: '#9ca3af' }}, grid: {{ color: 'rgba(255,255,255,0.05)' }} }}
            }}
        }};

        // Performance Chart
        new Chart(document.getElementById('perfChart'), {{
            type: 'bar',
            data: {{
                labels: [{perf_labels}],
                datasets: [{{
                    label: 'Total Time (ms)',
                    data: [{perf_data}],
                    backgroundColor: chartColors.blue,
                    borderRadius: 4
                }}]
            }},
            options: chartOptions
        }});

        // Cost Chart
        new Chart(document.getElementById('costChart'), {{
            type: 'doughnut',
            data: {{
                labels: [{cost_labels}],
                datasets: [{{
                    data: [{cost_data}],
                    backgroundColor: [
                        chartColors.blue, chartColors.green, chartColors.purple,
                        chartColors.yellow, chartColors.red, chartColors.gray,
                        'rgba(34, 211, 238, 0.8)', 'rgba(249, 115, 22, 0.8)',
                        'rgba(139, 92, 246, 0.8)', 'rgba(236, 72, 153, 0.8)'
                    ]
                }}]
            }},
            options: {{
                responsive: true,
                plugins: {{
                    legend: {{ position: 'right', labels: {{ color: '#9ca3af' }} }}
                }}
            }}
        }});

        // Event Distribution Chart
        new Chart(document.getElementById('eventChart'), {{
            type: 'pie',
            data: {{
                labels: [{event_labels}],
                datasets: [{{
                    data: [{event_counts}],
                    backgroundColor: [
                        chartColors.blue, chartColors.green, chartColors.purple,
                        chartColors.yellow, chartColors.red, chartColors.gray,
                        'rgba(34, 211, 238, 0.8)', 'rgba(249, 115, 22, 0.8)'
                    ]
                }}]
            }},
            options: {{
                responsive: true,
                plugins: {{
                    legend: {{ position: 'right', labels: {{ color: '#9ca3af' }} }}
                }}
            }}
        }});

        // Avg vs Total Chart
        new Chart(document.getElementById('avgChart'), {{
            type: 'bar',
            data: {{
                labels: [{perf_labels}],
                datasets: [
                    {{
                        label: 'Total (ms)',
                        data: [{perf_data}],
                        backgroundColor: chartColors.blue,
                        borderRadius: 4
                    }},
                    {{
                        label: 'Avg (ms)',
                        data: [{perf_avg_data}],
                        backgroundColor: chartColors.green,
                        borderRadius: 4
                    }}
                ]
            }},
            options: chartOptions
        }});
    </script>
</body>
</html>"#,
        title = title,
        thread_id = data.thread_id,
        total_events = data.total_events,
        duration = format_duration_html(data.duration_micros),
        llm_calls = total_usage.llm_calls,
        total_tokens = format_tokens(total_usage.total_tokens()),
        cost = total_cost,
        perf_labels = perf_labels.join(", "),
        perf_data = perf_data
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        perf_avg_data = perf_avg_data
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        cost_labels = cost_labels.join(", "),
        cost_data = cost_data.join(", "),
        event_labels = event_labels.join(", "),
        event_counts = event_counts.join(", "),
        table_rows = generate_table_rows(&sorted_profiles),
        timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    )
}

fn generate_table_rows(profiles: &[(&String, &NodeProfile)]) -> String {
    let total_time: i64 = profiles.iter().map(|(_, p)| p.total_duration).sum();
    let max_duration = profiles
        .iter()
        .map(|(_, p)| p.total_duration)
        .max()
        .unwrap_or(1);

    profiles
        .iter()
        .take(20)
        .map(|(name, profile)| {
            let pct = if total_time > 0 {
                (profile.total_duration as f64 / total_time as f64) * 100.0
            } else {
                0.0
            };
            let bar_width = (profile.total_duration as f64 / max_duration as f64 * 100.0) as u32;
            format!(
                r#"<tr>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{:.1}%</td>
                    <td><div class="bar" style="width: {}%"></div></td>
                </tr>"#,
                name,
                profile.executions,
                format_duration_html(profile.total_duration),
                format_duration_html(profile.avg_duration()),
                pct,
                bar_width
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_duration_html(micros: i64) -> String {
    if micros < 1_000 {
        format!("{}μs", micros)
    } else if micros < 1_000_000 {
        format!("{:.2}ms", micros as f64 / 1_000.0)
    } else if micros < 60_000_000 {
        format!("{:.2}s", micros as f64 / 1_000_000.0)
    } else {
        let minutes = micros / 60_000_000;
        let seconds = (micros % 60_000_000) / 1_000_000;
        format!("{}m {}s", minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_export() -> (NamedTempFile, String) {
        let export = ExportOutput {
            thread_id: "test-thread-123".to_string(),
            total_events: 6,
            start_time: 1000000,
            end_time: 2500000,
            duration_micros: 1500000,
            events: vec![
                ExportedEvent {
                    message_id: "msg1".to_string(),
                    sequence: 1,
                    timestamp_micros: 1000000,
                    tenant_id: "tenant1".to_string(),
                    thread_id: "test-thread-123".to_string(),
                    event_type: "NodeStart".to_string(),
                    node_id: "node_a".to_string(),
                    duration_us: 0,
                    llm_request_id: "".to_string(),
                    attributes: serde_json::json!({}),
                },
                ExportedEvent {
                    message_id: "msg2".to_string(),
                    sequence: 2,
                    timestamp_micros: 1500000,
                    tenant_id: "tenant1".to_string(),
                    thread_id: "test-thread-123".to_string(),
                    event_type: "NodeEnd".to_string(),
                    node_id: "node_a".to_string(),
                    duration_us: 500000,
                    llm_request_id: "".to_string(),
                    attributes: serde_json::json!({}),
                },
                ExportedEvent {
                    message_id: "msg3".to_string(),
                    sequence: 3,
                    timestamp_micros: 1500000,
                    tenant_id: "tenant1".to_string(),
                    thread_id: "test-thread-123".to_string(),
                    event_type: "LlmStart".to_string(),
                    node_id: "node_b".to_string(),
                    duration_us: 0,
                    llm_request_id: "llm1".to_string(),
                    attributes: serde_json::json!({}),
                },
                ExportedEvent {
                    message_id: "msg4".to_string(),
                    sequence: 4,
                    timestamp_micros: 2000000,
                    tenant_id: "tenant1".to_string(),
                    thread_id: "test-thread-123".to_string(),
                    event_type: "LlmEnd".to_string(),
                    node_id: "node_b".to_string(),
                    duration_us: 500000,
                    llm_request_id: "llm1".to_string(),
                    attributes: serde_json::json!({
                        "input_tokens": 100,
                        "output_tokens": 50
                    }),
                },
                ExportedEvent {
                    message_id: "msg5".to_string(),
                    sequence: 5,
                    timestamp_micros: 2000000,
                    tenant_id: "tenant1".to_string(),
                    thread_id: "test-thread-123".to_string(),
                    event_type: "EdgeTraversal".to_string(),
                    node_id: "".to_string(),
                    duration_us: 0,
                    llm_request_id: "".to_string(),
                    attributes: serde_json::json!({
                        "edge_from": "node_a",
                        "edge_to": "node_b"
                    }),
                },
                ExportedEvent {
                    message_id: "msg6".to_string(),
                    sequence: 6,
                    timestamp_micros: 2500000,
                    tenant_id: "tenant1".to_string(),
                    thread_id: "test-thread-123".to_string(),
                    event_type: "NodeEnd".to_string(),
                    node_id: "node_b".to_string(),
                    duration_us: 1000000,
                    llm_request_id: "".to_string(),
                    attributes: serde_json::json!({}),
                },
            ],
        };

        let json = serde_json::to_string_pretty(&export).expect("test: serialize export");
        let mut file = NamedTempFile::new().expect("test: create temp file");
        file.write_all(json.as_bytes())
            .expect("test: write export file");
        let path = file.path().to_string_lossy().to_string();

        (file, path)
    }

    #[test]
    fn test_load_export_file() {
        let (_file, path) = create_test_export();
        let data = load_export_file(&path).expect("test: load export file");

        assert_eq!(data.thread_id, "test-thread-123");
        assert_eq!(data.total_events, 6);
        assert_eq!(data.duration_micros, 1500000);
    }

    #[test]
    fn test_build_profile() {
        let (_file, path) = create_test_export();
        let data = load_export_file(&path).expect("test: load export");
        let profiles = build_profile(&data.events);

        assert!(profiles.contains_key("node_a"));
        let node_a = profiles.get("node_a").expect("test: get node_a profile");
        assert_eq!(node_a.executions, 1);
        assert_eq!(node_a.total_duration, 500000);
    }

    #[test]
    fn test_calculate_total_usage() {
        let (_file, path) = create_test_export();
        let data = load_export_file(&path).expect("test: load export");
        let usage = calculate_total_usage(&data.events);

        assert_eq!(usage.llm_calls, 1);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
    }

    #[test]
    fn test_token_usage_cost() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            llm_calls: 10,
        };

        let cost = usage.cost(0.25, 1.25);
        // Input: 1M * $0.25/M = $0.25
        // Output: 0.5M * $1.25/M = $0.625
        // Total: $0.875
        assert!((cost - 0.875).abs() < 0.001);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.50M");
    }

    #[test]
    fn test_build_folded_stacks() {
        let (_file, path) = create_test_export();
        let data = load_export_file(&path).expect("test: load export");
        let folded = build_folded_stacks(&data.events);

        assert!(folded.contains("node_a"));
        // Should have timing data
        assert!(!folded.is_empty());
    }

    #[test]
    fn test_node_profile_statistics() {
        let mut profile = NodeProfile::default();
        profile.add_execution(100);
        profile.add_execution(200);
        profile.add_execution(300);

        assert_eq!(profile.executions, 3);
        assert_eq!(profile.total_duration, 600);
        assert_eq!(profile.avg_duration(), 200);
        assert_eq!(profile.min_duration, 100);
        assert_eq!(profile.max_duration, 300);
        assert_eq!(profile.median_duration(), 200);
    }

    #[tokio::test]
    async fn test_run_summary() {
        let (_file, path) = create_test_export();
        let args = AnalyzeSummaryArgs {
            input: path,
            verbose: false,
        };

        // Should not panic
        run_summary(args).await.expect("test: run_summary");
    }

    #[tokio::test]
    async fn test_run_profile() {
        let (_file, path) = create_test_export();
        let args = AnalyzeProfileArgs {
            input: path,
            detailed: true,
            top: 10,
        };

        // Should not panic
        run_profile(args).await.expect("test: run_profile");
    }

    #[tokio::test]
    async fn test_run_costs() {
        let (_file, path) = create_test_export();
        let args = AnalyzeCostsArgs {
            input: path,
            by_node: true,
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
        };

        // Should not panic
        run_costs(args).await.expect("test: run_costs");
    }

    #[tokio::test]
    async fn test_run_flamegraph_folded() {
        let (_file, path) = create_test_export();
        let output_file = NamedTempFile::new().expect("test: create temp file");
        let output_path = output_file.path().to_string_lossy().to_string();

        let args = AnalyzeFlamegraphArgs {
            input: path,
            output: output_path.clone(),
            format: "folded".to_string(),
            title: Some("Test Flamegraph".to_string()),
        };

        run_flamegraph(args)
            .await
            .expect("test: run_flamegraph folded");

        // Verify output file was created
        let content = std::fs::read_to_string(&output_path).expect("test: read output");
        assert!(!content.is_empty());
    }

    #[tokio::test]
    async fn test_run_flamegraph_svg() {
        let (_file, path) = create_test_export();
        let output_file = NamedTempFile::new().expect("test: create temp file");
        let output_path = output_file.path().to_string_lossy().to_string();

        let args = AnalyzeFlamegraphArgs {
            input: path,
            output: output_path.clone(),
            format: "svg".to_string(),
            title: None,
        };

        run_flamegraph(args)
            .await
            .expect("test: run_flamegraph svg");

        // Verify SVG output file was created
        let content = std::fs::read_to_string(&output_path).expect("test: read output");
        assert!(content.contains("<svg"));
    }

    #[test]
    fn test_calculate_node_costs() {
        let (_file, path) = create_test_export();
        let data = load_export_file(&path).expect("test: load export");
        let args = AnalyzeDashboardArgs {
            input: path,
            output: "test.html".to_string(),
            title: None,
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
            open: false,
        };
        let costs = calculate_node_costs(&data.events, &args);

        // Should have cost for node_b (has LlmEnd event)
        assert!(!costs.is_empty());
        let node_b_cost = costs.iter().find(|(k, _)| k == "node_b");
        assert!(node_b_cost.is_some());
    }

    #[test]
    fn test_generate_dashboard_html() {
        let (_file, path) = create_test_export();
        let data = load_export_file(&path).expect("test: load export");
        let profiles = build_profile(&data.events);
        let total_usage = calculate_total_usage(&data.events);
        let args = AnalyzeDashboardArgs {
            input: path,
            output: "test.html".to_string(),
            title: Some("Test Dashboard".to_string()),
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
            open: false,
        };
        let node_costs = calculate_node_costs(&data.events, &args);

        let html = generate_dashboard_html(
            "Test Dashboard",
            &data,
            &profiles,
            &total_usage,
            &node_costs,
            &args,
        );

        // Verify HTML structure
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>Test Dashboard</title>"));
        assert!(html.contains("chart.js"));
        assert!(html.contains("test-thread-123"));
        assert!(html.contains("Total Events"));
        assert!(html.contains("LLM Calls"));
        assert!(html.contains("Estimated Cost"));
    }

    #[test]
    fn test_generate_table_rows() {
        let mut profiles: HashMap<String, NodeProfile> = HashMap::new();
        let mut profile = NodeProfile::default();
        profile.add_execution(100000);
        profile.add_execution(200000);
        profiles.insert("test_node".to_string(), profile);

        let sorted_profiles: Vec<_> = profiles.iter().collect();
        let rows = generate_table_rows(&sorted_profiles);

        assert!(rows.contains("test_node"));
        assert!(rows.contains("<tr>"));
        assert!(rows.contains("</tr>"));
    }

    #[test]
    fn test_format_duration_html() {
        assert_eq!(format_duration_html(500), "500μs");
        assert_eq!(format_duration_html(1500), "1.50ms");
        assert_eq!(format_duration_html(1_500_000), "1.50s");
        assert_eq!(format_duration_html(90_000_000), "1m 30s");
    }

    #[tokio::test]
    async fn test_run_dashboard() {
        let (_file, path) = create_test_export();
        let output_file = NamedTempFile::new().expect("test: create temp file");
        let output_path = output_file.path().to_string_lossy().to_string();

        let args = AnalyzeDashboardArgs {
            input: path,
            output: output_path.clone(),
            title: Some("Test Dashboard".to_string()),
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
            open: false,
        };

        run_dashboard(args).await.expect("test: run_dashboard");

        // Verify HTML output file was created
        let content = std::fs::read_to_string(&output_path).expect("test: read output");
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Test Dashboard"));
        assert!(content.contains("chart.js"));
    }

    #[tokio::test]
    async fn test_run_dashboard_default_title() {
        let (_file, path) = create_test_export();
        let output_file = NamedTempFile::new().expect("test: create temp file");
        let output_path = output_file.path().to_string_lossy().to_string();

        let args = AnalyzeDashboardArgs {
            input: path,
            output: output_path.clone(),
            title: None, // Use default title
            input_cost_per_million: 0.25,
            output_cost_per_million: 1.25,
            open: false,
        };

        run_dashboard(args).await.expect("test: run_dashboard");

        // Verify default title is used
        let content = std::fs::read_to_string(&output_path).expect("test: read output");
        assert!(content.contains("DashFlow Dashboard - Thread test-thread-123"));
    }
}
