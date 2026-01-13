// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// The blanket #![allow(clippy::unwrap_used)] was removed.
// Targeted allows are used where serialization of internal types is infallible.

//! DashFlow Streaming Event Analytics Tool
//!
//! Analyzes JSON output from `parse_events` and generates aggregate metrics and insights.
//!
//! Usage:
//!   cargo run --bin `parse_events` | cargo run --bin `analyze_events`
//!   cargo run --bin `parse_events` -- --limit 1000 | cargo run --bin `analyze_events` --format markdown
//!
//! Features:
//! - Session-level metrics (total duration, event counts)
//! - Node performance analysis (min/max/avg/p50/p95/p99 durations)
//! - Tool execution statistics (success rate, retry count)
//! - Error frequency and severity distribution
//! - Token usage tracking (if available)
//! - Multiple output formats (JSON, Markdown, Text)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, BufRead};

// ============================================================================
// JSON Input Schema (matches parse_events output)
// ============================================================================

// JUSTIFICATION: Serde deserialization enum. Tag-based enum for deserializing
// parse_events JSON output (lines 249-290 main()). All 7 variants (Event, TokenChunk,
// StateDiff, ToolExecution, Metrics, Error, Checkpoint) must be defined for serde to
// correctly parse {"type": "..."} discriminator. Not all variants actively processed
// (StateDiff, Metrics, Checkpoint just tracked, lines 404-428) but removing them would
// break deserialization. Schema stability: allows parse_events to emit new event types
// without breaking analyze_events binary.
#[allow(dead_code)] // Deserialize: Tagged enum for parse_events JSON message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ParsedMessage {
    Event(EventJson),
    TokenChunk(TokenChunkJson),
    StateDiff(StateDiffJson),
    ToolExecution(ToolExecutionJson),
    Metrics(MetricsJson),
    Error(ErrorJson),
    Checkpoint(CheckpointJson),
}

// JUSTIFICATION: Serde deserialization struct. Must match parse_events Event schema.
// Fields used: header (thread_id, tenant_id, timestamp_us), event_type, node_id, duration_ms,
// llm_request_id (lines 293-331). Field duration_us unused (duration_ms preferred). All fields
// required for serde to successfully deserialize parse_events JSON output. Cannot remove unused
// fields without breaking deserialization when parse_events emits them. Schema stability.
#[allow(dead_code)] // Deserialize: Event struct from parse_events JSON
#[derive(Debug, Deserialize)]
struct EventJson {
    header: HeaderJson,
    event_type: String,
    node_id: Option<String>,
    duration_us: Option<i64>,
    duration_ms: Option<f64>,
    llm_request_id: Option<String>,
}

// JUSTIFICATION: Serde deserialization struct. Must match parse_events TokenChunk schema.
// Fields used: header.thread_id/tenant_id (lines 336-341), stats (prompt_tokens, completion_tokens,
// total_tokens, lines 343-347). Field `text` unused (analytics focus on token counts, not content).
// Cannot remove `text` field without breaking deserialization when parse_events emits it. Schema
// stability: analyze_events extracts metrics, ignores content.
#[allow(dead_code)] // Deserialize: TokenChunk struct from parse_events JSON
#[derive(Debug, Deserialize)]
struct TokenChunkJson {
    header: HeaderJson,
    text: Option<String>,
    stats: Option<TokenStatsJson>,
}

#[derive(Debug, Deserialize)]
struct TokenStatsJson {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct StateDiffJson {
    header: HeaderJson,
}

// JUSTIFICATION: Serde deserialization struct. Must match parse_events ToolExecution schema.
// Fields used: header.thread_id/tenant_id (lines 352-357), tool_name (line 361), stage (line 365),
// retry_count (line 371), duration_ms (line 377). Field `error` unused (only stage="FAILED" tracked).
// Field duration_us unused (duration_ms preferred). Cannot remove unused fields without breaking
// deserialization. Schema stability.
#[allow(dead_code)] // Deserialize: ToolExecution struct from parse_events JSON
#[derive(Debug, Deserialize)]
struct ToolExecutionJson {
    header: HeaderJson,
    tool_name: String,
    stage: String,
    duration_us: Option<i64>,
    duration_ms: Option<f64>,
    retry_count: Option<u32>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MetricsJson {
    header: HeaderJson,
}

#[derive(Debug, Deserialize)]
struct ErrorJson {
    header: HeaderJson,
    severity: String,
    code: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CheckpointJson {
    header: HeaderJson,
}

// JUSTIFICATION: Serde deserialization struct. Common header in all parse_events messages.
// Fields used: thread_id (required, used everywhere lines 295-406), tenant_id (optional, lines 298-419),
// timestamp_us (optional, lines 309-316), timestamp_iso (optional, line 396). Fields message_id and
// sequence unused in current analytics but required for deserialization when parse_events emits them.
// Cannot remove unused fields without breaking schema. Header shared across all message types.
#[allow(dead_code)] // Deserialize: Common header struct from parse_events JSON
#[derive(Debug, Deserialize)]
struct HeaderJson {
    message_id: Option<String>,
    timestamp_us: Option<i64>,
    timestamp_iso: Option<String>,
    tenant_id: Option<String>,
    thread_id: String,
    sequence: u64,
}

// ============================================================================
// Analytics Data Structures
// ============================================================================

// JUSTIFICATION: Internal analytics struct. Aggregates metrics during processing (lines
// 291-428 process_message) but not all fields appear in final report output (lines 436-547
// generate_report). Fields collected but not reported: event_type_counts (line 305), unique_llm_requests
// (line 330). These are architectural fields reserved for future analytics enhancements. Removing would
// require recalculating if report format changes. Low cost to maintain, high cost to recompute later.
#[allow(dead_code)] // Architectural: Analytics aggregation struct - some fields reserved for future reports
#[derive(Debug, Default)]
struct SessionMetrics {
    thread_id: String,
    tenant_id: String,

    // Event counts
    total_events: usize,
    event_type_counts: HashMap<String, usize>,

    // Timing
    first_timestamp_us: Option<i64>,
    last_timestamp_us: Option<i64>,
    total_duration_ms: f64,

    // Node performance
    node_durations: HashMap<String, Vec<f64>>, // node_id -> [durations in ms]

    // Tool execution
    tool_executions: HashMap<String, ToolStats>,

    // Errors
    error_counts: HashMap<String, usize>, // severity -> count
    errors: Vec<ErrorInfo>,

    // Tokens
    total_prompt_tokens: u64,
    total_completion_tokens: u64,
    total_tokens: u64,

    // LLM requests
    unique_llm_requests: std::collections::HashSet<String>,
}

#[derive(Debug, Default)]
struct ToolStats {
    total_calls: usize,
    completed: usize,
    failed: usize,
    retries: usize,
    durations: Vec<f64>, // milliseconds
}

// JUSTIFICATION: Internal error tracking struct. Created at line 395 when processing
// Error messages, stored in SessionMetrics.errors. All fields (timestamp_iso, severity, code,
// message) populated from ErrorJson (lines 396-399) but not all fields used in final error
// summary output. Architectural: complete error data preserved for potential detailed error
// reporting. Low memory cost, enables richer error analysis without reprocessing.
#[allow(dead_code)] // Architectural: Complete error data preserved for detailed error reporting
#[derive(Debug)]
struct ErrorInfo {
    timestamp_iso: String,
    severity: String,
    code: Option<String>,
    message: String,
}

#[derive(Debug, Serialize)]
struct AnalyticsReport {
    summary: SummaryStats,
    sessions: Vec<SessionReport>,
    node_performance: Vec<NodePerformanceReport>,
    tool_performance: Vec<ToolPerformanceReport>,
    errors: Vec<ErrorSummary>,
}

#[derive(Debug, Serialize)]
struct SummaryStats {
    total_sessions: usize,
    total_events: usize,
    total_duration_ms: f64,
    total_prompt_tokens: u64,
    total_completion_tokens: u64,
    total_tokens: u64,
    total_errors: usize,
}

#[derive(Debug, Serialize)]
struct SessionReport {
    thread_id: String,
    tenant_id: String,
    event_count: usize,
    duration_ms: f64,
    node_count: usize,
    tool_calls: usize,
    error_count: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Debug, Serialize)]
struct NodePerformanceReport {
    node_id: String,
    execution_count: usize,
    total_duration_ms: f64,
    avg_duration_ms: f64,
    min_duration_ms: f64,
    max_duration_ms: f64,
    p50_duration_ms: f64,
    p95_duration_ms: f64,
    p99_duration_ms: f64,
}

#[derive(Debug, Serialize)]
struct ToolPerformanceReport {
    tool_name: String,
    total_calls: usize,
    completed: usize,
    failed: usize,
    success_rate: f64,
    retry_rate: f64,
    avg_duration_ms: f64,
    p95_duration_ms: f64,
}

#[derive(Debug, Serialize)]
struct ErrorSummary {
    severity: String,
    count: usize,
    percentage: f64,
}

// ============================================================================
// Main Logic
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let format = if args.len() > 1 && args[1] == "--format" && args.len() > 2 {
        args[2].as_str()
    } else {
        "text"
    };

    let mut sessions: HashMap<String, SessionMetrics> = HashMap::new();
    let stdin = io::stdin();
    let reader = stdin.lock();

    // Read and parse JSON lines
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.trim().is_empty() {
            continue;
        }

        let message: ParsedMessage = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };

        process_message(message, &mut sessions);
    }

    // Generate report
    let report = generate_report(&sessions);

    // Output in requested format
    match format {
        "json" => output_json(&report),
        "markdown" => output_markdown(&report),
        _ => output_text(&report),
    }
}

fn process_message(message: ParsedMessage, sessions: &mut HashMap<String, SessionMetrics>) {
    match message {
        ParsedMessage::Event(event) => {
            let session = sessions
                .entry(event.header.thread_id.clone())
                .or_insert_with(|| SessionMetrics {
                    thread_id: event.header.thread_id.clone(),
                    tenant_id: event.header.tenant_id.clone().unwrap_or_default(),
                    ..Default::default()
                });

            session.total_events += 1;
            *session
                .event_type_counts
                .entry(event.event_type.clone())
                .or_insert(0) += 1;

            // Update timestamps
            if let Some(ts) = event.header.timestamp_us {
                if session.first_timestamp_us.map_or(true, |first| ts < first) {
                    session.first_timestamp_us = Some(ts);
                }
                if session.last_timestamp_us.map_or(true, |last| ts > last) {
                    session.last_timestamp_us = Some(ts);
                }
            }

            // Track node durations
            if let (Some(node_id), Some(duration_ms)) = (event.node_id, event.duration_ms) {
                session
                    .node_durations
                    .entry(node_id)
                    .or_default()
                    .push(duration_ms);
            }

            // Track LLM requests
            if let Some(llm_id) = event.llm_request_id {
                session.unique_llm_requests.insert(llm_id);
            }
        }

        ParsedMessage::TokenChunk(chunk) => {
            let session = sessions
                .entry(chunk.header.thread_id.clone())
                .or_insert_with(|| SessionMetrics {
                    thread_id: chunk.header.thread_id.clone(),
                    tenant_id: chunk.header.tenant_id.clone().unwrap_or_default(),
                    ..Default::default()
                });

            if let Some(stats) = chunk.stats {
                session.total_prompt_tokens += u64::from(stats.prompt_tokens.unwrap_or(0));
                session.total_completion_tokens += u64::from(stats.completion_tokens.unwrap_or(0));
                session.total_tokens += u64::from(stats.total_tokens.unwrap_or(0));
            }
        }

        ParsedMessage::ToolExecution(tool) => {
            let session = sessions
                .entry(tool.header.thread_id.clone())
                .or_insert_with(|| SessionMetrics {
                    thread_id: tool.header.thread_id.clone(),
                    tenant_id: tool.header.tenant_id.clone().unwrap_or_default(),
                    ..Default::default()
                });

            let tool_stats = session
                .tool_executions
                .entry(tool.tool_name.clone())
                .or_default();
            tool_stats.total_calls += 1;

            match tool.stage.as_str() {
                "COMPLETED" => tool_stats.completed += 1,
                "FAILED" => tool_stats.failed += 1,
                _ => {}
            }

            if let Some(retry_count) = tool.retry_count {
                if retry_count > 0 {
                    tool_stats.retries += 1;
                }
            }

            if let Some(duration_ms) = tool.duration_ms {
                tool_stats.durations.push(duration_ms);
            }
        }

        ParsedMessage::Error(error) => {
            let session = sessions
                .entry(error.header.thread_id.clone())
                .or_insert_with(|| SessionMetrics {
                    thread_id: error.header.thread_id.clone(),
                    tenant_id: error.header.tenant_id.clone().unwrap_or_default(),
                    ..Default::default()
                });

            *session
                .error_counts
                .entry(error.severity.clone())
                .or_insert(0) += 1;
            session.errors.push(ErrorInfo {
                timestamp_iso: error.header.timestamp_iso.unwrap_or_default(),
                severity: error.severity,
                code: error.code,
                message: error.message,
            });
        }

        // Other message types - just count them
        ParsedMessage::StateDiff(diff) => {
            sessions
                .entry(diff.header.thread_id.clone())
                .or_insert_with(|| SessionMetrics {
                    thread_id: diff.header.thread_id.clone(),
                    tenant_id: diff.header.tenant_id.clone().unwrap_or_default(),
                    ..Default::default()
                });
        }

        ParsedMessage::Metrics(metrics) => {
            sessions
                .entry(metrics.header.thread_id.clone())
                .or_insert_with(|| SessionMetrics {
                    thread_id: metrics.header.thread_id.clone(),
                    tenant_id: metrics.header.tenant_id.clone().unwrap_or_default(),
                    ..Default::default()
                });
        }

        ParsedMessage::Checkpoint(checkpoint) => {
            sessions
                .entry(checkpoint.header.thread_id.clone())
                .or_insert_with(|| SessionMetrics {
                    thread_id: checkpoint.header.thread_id.clone(),
                    tenant_id: checkpoint.header.tenant_id.clone().unwrap_or_default(),
                    ..Default::default()
                });
        }
    }
}

fn generate_report(sessions: &HashMap<String, SessionMetrics>) -> AnalyticsReport {
    // Summary stats
    let total_sessions = sessions.len();
    let total_events: usize = sessions.values().map(|s| s.total_events).sum();
    let total_prompt_tokens: u64 = sessions.values().map(|s| s.total_prompt_tokens).sum();
    let total_completion_tokens: u64 = sessions.values().map(|s| s.total_completion_tokens).sum();
    let total_tokens: u64 = sessions.values().map(|s| s.total_tokens).sum();
    let total_errors: usize = sessions.values().map(|s| s.errors.len()).sum();

    // Calculate session durations
    let mut total_duration_ms = 0.0;
    for session in sessions.values() {
        if let (Some(first), Some(last)) = (session.first_timestamp_us, session.last_timestamp_us) {
            total_duration_ms += (last - first) as f64 / 1000.0;
        }
    }

    // Session reports
    let mut session_reports: Vec<SessionReport> = sessions
        .values()
        .map(|s| {
            let duration_ms =
                if let (Some(first), Some(last)) = (s.first_timestamp_us, s.last_timestamp_us) {
                    (last - first) as f64 / 1000.0
                } else {
                    0.0
                };

            SessionReport {
                thread_id: s.thread_id.clone(),
                tenant_id: s.tenant_id.clone(),
                event_count: s.total_events,
                duration_ms,
                node_count: s.node_durations.len(),
                tool_calls: s.tool_executions.values().map(|t| t.total_calls).sum(),
                error_count: s.errors.len(),
                prompt_tokens: s.total_prompt_tokens,
                completion_tokens: s.total_completion_tokens,
                total_tokens: s.total_tokens,
            }
        })
        .collect();
    session_reports.sort_by(|a, b| b.event_count.cmp(&a.event_count));

    // Node performance (aggregate across all sessions)
    let mut all_node_durations: HashMap<String, Vec<f64>> = HashMap::new();
    for session in sessions.values() {
        for (node_id, durations) in &session.node_durations {
            all_node_durations
                .entry(node_id.clone())
                .or_default()
                .extend(durations);
        }
    }

    let mut node_reports: Vec<NodePerformanceReport> = all_node_durations
        .into_iter()
        .map(|(node_id, mut durations)| {
            durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let total: f64 = durations.iter().sum();
            let count = durations.len();
            let avg = total / count as f64;
            let min = durations.first().copied().unwrap_or(0.0);
            let max = durations.last().copied().unwrap_or(0.0);
            let p50 = percentile(&durations, 0.50);
            let p95 = percentile(&durations, 0.95);
            let p99 = percentile(&durations, 0.99);

            NodePerformanceReport {
                node_id,
                execution_count: count,
                total_duration_ms: total,
                avg_duration_ms: avg,
                min_duration_ms: min,
                max_duration_ms: max,
                p50_duration_ms: p50,
                p95_duration_ms: p95,
                p99_duration_ms: p99,
            }
        })
        .collect();
    node_reports.sort_by(|a, b| {
        b.total_duration_ms
            .partial_cmp(&a.total_duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Tool performance (aggregate across all sessions)
    let mut all_tool_stats: HashMap<String, ToolStats> = HashMap::new();
    for session in sessions.values() {
        for (tool_name, stats) in &session.tool_executions {
            let entry = all_tool_stats.entry(tool_name.clone()).or_default();
            entry.total_calls += stats.total_calls;
            entry.completed += stats.completed;
            entry.failed += stats.failed;
            entry.retries += stats.retries;
            entry.durations.extend(&stats.durations);
        }
    }

    let mut tool_reports: Vec<ToolPerformanceReport> = all_tool_stats
        .into_iter()
        .map(|(tool_name, mut stats)| {
            stats
                .durations
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let success_rate = if stats.total_calls > 0 {
                stats.completed as f64 / stats.total_calls as f64 * 100.0
            } else {
                0.0
            };
            let retry_rate = if stats.total_calls > 0 {
                stats.retries as f64 / stats.total_calls as f64 * 100.0
            } else {
                0.0
            };
            let avg_duration = if stats.durations.is_empty() {
                0.0
            } else {
                stats.durations.iter().sum::<f64>() / stats.durations.len() as f64
            };
            let p95_duration = percentile(&stats.durations, 0.95);

            ToolPerformanceReport {
                tool_name,
                total_calls: stats.total_calls,
                completed: stats.completed,
                failed: stats.failed,
                success_rate,
                retry_rate,
                avg_duration_ms: avg_duration,
                p95_duration_ms: p95_duration,
            }
        })
        .collect();
    tool_reports.sort_by(|a, b| b.total_calls.cmp(&a.total_calls));

    // Error summary (aggregate across all sessions)
    let mut all_error_counts: HashMap<String, usize> = HashMap::new();
    for session in sessions.values() {
        for (severity, count) in &session.error_counts {
            *all_error_counts.entry(severity.clone()).or_insert(0) += count;
        }
    }

    let mut error_summaries: Vec<ErrorSummary> = all_error_counts
        .into_iter()
        .map(|(severity, count)| ErrorSummary {
            severity,
            count,
            percentage: if total_errors > 0 {
                count as f64 / total_errors as f64 * 100.0
            } else {
                0.0
            },
        })
        .collect();
    error_summaries.sort_by(|a, b| b.count.cmp(&a.count));

    AnalyticsReport {
        summary: SummaryStats {
            total_sessions,
            total_events,
            total_duration_ms,
            total_prompt_tokens,
            total_completion_tokens,
            total_tokens,
            total_errors,
        },
        sessions: session_reports,
        node_performance: node_reports,
        tool_performance: tool_reports,
        errors: error_summaries,
    }
}

fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    let idx = (sorted_data.len() as f64 * p).ceil() as usize - 1;
    sorted_data[idx.min(sorted_data.len() - 1)]
}

// ============================================================================
// Output Formatters
// ============================================================================

// SAFETY: AnalyticsReport derives Serialize with standard serde types only (no custom
// serialization logic). Serialization can only fail if the type definition is invalid,
// which would be caught at development time.
#[allow(clippy::unwrap_used)]
fn output_json(report: &AnalyticsReport) {
    println!("{}", serde_json::to_string_pretty(report).unwrap());
}

fn output_markdown(report: &AnalyticsReport) {
    println!("# DashFlow Streaming Analytics Report\n");

    println!("## Summary\n");
    println!("| Metric | Value |");
    println!("|--------|-------|");
    println!("| Total Sessions | {} |", report.summary.total_sessions);
    println!("| Total Events | {} |", report.summary.total_events);
    println!(
        "| Total Duration | {:.2} ms |",
        report.summary.total_duration_ms
    );
    println!("| Total Tokens | {} |", report.summary.total_tokens);
    println!("| Prompt Tokens | {} |", report.summary.total_prompt_tokens);
    println!(
        "| Completion Tokens | {} |",
        report.summary.total_completion_tokens
    );
    println!("| Total Errors | {} |", report.summary.total_errors);
    println!();

    if !report.sessions.is_empty() {
        println!("## Session Details\n");
        println!("| Thread ID | Events | Duration (ms) | Nodes | Tool Calls | Errors | Tokens |");
        println!("|-----------|--------|---------------|-------|------------|--------|--------|");
        for session in &report.sessions {
            println!(
                "| {} | {} | {:.2} | {} | {} | {} | {} |",
                session.thread_id,
                session.event_count,
                session.duration_ms,
                session.node_count,
                session.tool_calls,
                session.error_count,
                session.total_tokens
            );
        }
        println!();
    }

    if !report.node_performance.is_empty() {
        println!("## Node Performance\n");
        println!(
            "| Node ID | Count | Avg (ms) | P50 (ms) | P95 (ms) | P99 (ms) | Min (ms) | Max (ms) |"
        );
        println!(
            "|---------|-------|----------|----------|----------|----------|----------|----------|"
        );
        for node in &report.node_performance {
            println!(
                "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |",
                node.node_id,
                node.execution_count,
                node.avg_duration_ms,
                node.p50_duration_ms,
                node.p95_duration_ms,
                node.p99_duration_ms,
                node.min_duration_ms,
                node.max_duration_ms
            );
        }
        println!();
    }

    if !report.tool_performance.is_empty() {
        println!("## Tool Performance\n");
        println!("| Tool Name | Calls | Success Rate | Avg Duration (ms) | P95 (ms) |");
        println!("|-----------|-------|--------------|-------------------|----------|");
        for tool in &report.tool_performance {
            println!(
                "| {} | {} | {:.1}% | {:.2} | {:.2} |",
                tool.tool_name,
                tool.total_calls,
                tool.success_rate,
                tool.avg_duration_ms,
                tool.p95_duration_ms
            );
        }
        println!();
    }

    if !report.errors.is_empty() {
        println!("## Error Summary\n");
        println!("| Severity | Count | Percentage |");
        println!("|----------|-------|------------|");
        for error in &report.errors {
            println!(
                "| {} | {} | {:.1}% |",
                error.severity, error.count, error.percentage
            );
        }
        println!();
    }
}

fn output_text(report: &AnalyticsReport) {
    println!("=== DashFlow Streaming Analytics Report ===\n");

    println!("SUMMARY");
    println!("  Total Sessions:      {}", report.summary.total_sessions);
    println!("  Total Events:        {}", report.summary.total_events);
    println!(
        "  Total Duration:      {:.2} ms",
        report.summary.total_duration_ms
    );
    println!("  Total Tokens:        {}", report.summary.total_tokens);
    println!(
        "  Prompt Tokens:       {}",
        report.summary.total_prompt_tokens
    );
    println!(
        "  Completion Tokens:   {}",
        report.summary.total_completion_tokens
    );
    println!("  Total Errors:        {}", report.summary.total_errors);
    println!();

    if !report.sessions.is_empty() {
        println!("SESSIONS (Top {}):", report.sessions.len().min(10));
        for (i, session) in report.sessions.iter().take(10).enumerate() {
            println!("  {}. {}", i + 1, session.thread_id);
            println!("     Events: {}, Duration: {:.2} ms, Nodes: {}, Tool Calls: {}, Errors: {}, Tokens: {}",
                session.event_count, session.duration_ms, session.node_count,
                session.tool_calls, session.error_count, session.total_tokens);
        }
        println!();
    }

    if !report.node_performance.is_empty() {
        println!("NODE PERFORMANCE (Top 10 by total duration):");
        for (i, node) in report.node_performance.iter().take(10).enumerate() {
            println!(
                "  {}. {} (count: {})",
                i + 1,
                node.node_id,
                node.execution_count
            );
            println!(
                "     Avg: {:.2} ms, P50: {:.2} ms, P95: {:.2} ms, P99: {:.2} ms",
                node.avg_duration_ms,
                node.p50_duration_ms,
                node.p95_duration_ms,
                node.p99_duration_ms
            );
        }
        println!();
    }

    if !report.tool_performance.is_empty() {
        println!("TOOL PERFORMANCE:");
        for tool in &report.tool_performance {
            println!("  {} (calls: {})", tool.tool_name, tool.total_calls);
            println!(
                "     Success Rate: {:.1}%, Avg Duration: {:.2} ms, P95: {:.2} ms",
                tool.success_rate, tool.avg_duration_ms, tool.p95_duration_ms
            );
        }
        println!();
    }

    if !report.errors.is_empty() {
        println!("ERRORS:");
        for error in &report.errors {
            println!(
                "  {}: {} ({:.1}%)",
                error.severity, error.count, error.percentage
            );
        }
        println!();
    }
}
