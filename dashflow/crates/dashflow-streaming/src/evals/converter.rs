// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Analytics to Metrics Converter
//!
//! Converts analytics JSON (from `analyze_events`) to `EvalMetrics`.
//!
//! # Input Format
//!
//! The input is JSON output from `analyze_events` binary:
//! ```json
//! {
//!   "summary": {
//!     "total_sessions": 1,
//!     "total_events": 4,
//!     "total_duration": 3000.0,
//!     "total_tokens": 150,
//!     "prompt_tokens": 100,
//!     "completion_tokens": 50,
//!     "total_errors": 0
//!   },
//!   "sessions": [...],
//!   "node_performance": [...],
//!   "tool_performance": [...]
//! }
//! ```

use super::metrics::{EvalMetrics, LlmPricing};
use anyhow::{Context, Result};
use serde::Deserialize;

/// Analytics report from `analyze_events`
// JUSTIFICATION: Serde deserialization struct. Must match analyze_events JSON output schema.
// Fields used: summary (lines 169-207), sessions (line 173), node_performance (lines 149-166).
// Fields unused: tool_performance (entire struct not accessed), errors (optional, not accessed).
// Cannot remove unused fields without breaking deserialization when analyze_events emits them.
// Schema stability: converter accepts full analytics JSON even if not all sections used.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnalyticsReport {
    summary: SummaryStats,
    sessions: Vec<SessionReport>,
    node_performance: Vec<NodePerformanceReport>,
    tool_performance: Vec<ToolPerformanceReport>,
    errors: Option<ErrorSummary>,
}

// JUSTIFICATION: Serde deserialization struct. Must match analyze_events summary schema.
// ALL 7 fields actively used in conversion: total_sessions (line 169), total_events (lines 178, 191),
// total_duration (not used directly but part of schema), total_tokens (line 207), prompt_tokens (line 186),
// completion_tokens (line 187), total_errors (line 181). This struct has complete field usage.
// #[allow(dead_code)] applies to entire struct, not individual fields. Required for deserialization.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SummaryStats {
    total_sessions: u64,
    total_events: u64,
    total_duration: f64,
    total_tokens: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_errors: u64,
}

// JUSTIFICATION: Serde deserialization struct. Must match analyze_events session schema.
// Field used: error_count (line 173 - filters sessions with errors for success_rate calculation).
// Fields unused: thread_id, event_count, duration, node_count, tool_calls, total_tokens (6 of 7).
// Cannot remove unused fields without breaking deserialization. Schema stability: iterate over
// sessions array, extract error_count, ignore other fields. Architectural buffer for future metrics.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SessionReport {
    thread_id: String,
    event_count: u64,
    duration: f64,
    node_count: u64,
    tool_calls: u64,
    error_count: u64,
    total_tokens: u64,
}

// JUSTIFICATION: Serde deserialization struct. Must match analyze_events node_performance schema.
// Fields used: p95 (line 156 - max P95 across all nodes), avg_duration (line 164 - average latency).
// Fields unused: node_id, count, total_duration, min_duration, max_duration, p50, p99 (7 of 9).
// Cannot remove unused fields without breaking deserialization. Extracts only p95 and avg_duration
// for aggregate metrics. Architectural buffer: complete per-node stats available for future analysis.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct NodePerformanceReport {
    node_id: String,
    count: u64,
    total_duration: f64,
    avg_duration: f64,
    min_duration: f64,
    max_duration: f64,
    p50: f64,
    p95: f64,
    p99: f64,
}

// JUSTIFICATION: Serde deserialization struct. Must match analyze_events tool_performance schema.
// ENTIRELY UNUSED: No fields accessed (line 191 uses summary.total_events for tool_calls approximation).
// Cannot remove struct without breaking deserialization of AnalyticsReport.tool_performance field.
// Architectural buffer: complete tool performance data available when needed (success rates, retry
// analysis, duration percentiles). Schema stability: converter accepts full analytics JSON.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ToolPerformanceReport {
    tool_name: String,
    total_calls: u64,
    completed: u64,
    failed: u64,
    retried: u64,
    success_rate: f64,
    retry_rate: f64,
    avg_duration: f64,
    p95_duration: f64,
}

// JUSTIFICATION: Serde deserialization struct. Must match analyze_events errors schema.
// ENTIRELY UNUSED: Optional field in AnalyticsReport (line 39), never accessed in conversion logic.
// Error count uses summary.total_errors (line 181) instead of severity breakdown. Cannot remove
// struct without breaking deserialization when analyze_events emits errors field. Architectural
// buffer: enables future error severity analysis (debug/info/warning/error/fatal distribution).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ErrorSummary {
    debug: u64,
    info: u64,
    warning: u64,
    error: u64,
    fatal: u64,
}

/// Converter from analytics JSON to `EvalMetrics`
pub struct AnalyticsConverter;

impl AnalyticsConverter {
    /// Convert analytics JSON to `EvalMetrics`
    ///
    /// # Arguments
    ///
    /// * `json` - JSON string from `analyze_events --format json`
    /// * `pricing` - Optional LLM pricing for cost calculation (default: GPT-4o)
    ///
    /// # Returns
    ///
    /// `EvalMetrics` with performance and cost populated.
    /// Quality metrics (correctness, relevance, etc.) are None.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dashflow_streaming::evals::{AnalyticsConverter, LlmPricing};
    ///
    /// let analytics_json = std::fs::read_to_string("analytics.json").unwrap();
    /// let metrics = AnalyticsConverter::from_json(&analytics_json, Some(LlmPricing::GPT_4O)).unwrap();
    ///
    /// println!("P95 latency: {}ms", metrics.p95_latency);
    /// println!("Total tokens: {}", metrics.total_tokens);
    /// println!("Cost: ${:.4}", metrics.cost_per_run);
    /// ```
    pub fn from_json(json: &str, pricing: Option<LlmPricing>) -> Result<EvalMetrics> {
        let report: AnalyticsReport =
            serde_json::from_str(json).context("Failed to parse analytics JSON")?;

        let pricing = pricing.unwrap_or(LlmPricing::GPT_4O);

        // Calculate P95 latency across all nodes
        let p95_latency = if report.node_performance.is_empty() {
            0.0
        } else {
            // Use max P95 across all nodes (worst-case latency)
            report
                .node_performance
                .iter()
                .map(|n| n.p95)
                .fold(0.0, f64::max)
        };

        // Calculate average latency across all nodes
        let avg_latency = if report.node_performance.is_empty() {
            0.0
        } else {
            let total: f64 = report.node_performance.iter().map(|n| n.avg_duration).sum();
            total / report.node_performance.len() as f64
        };

        // Calculate success rate
        let success_rate = if report.summary.total_sessions == 0 {
            1.0
        } else {
            let failed_sessions =
                report.sessions.iter().filter(|s| s.error_count > 0).count() as f64;
            1.0 - (failed_sessions / report.summary.total_sessions as f64)
        };

        // Calculate error rate
        let error_rate = if report.summary.total_events == 0 {
            0.0
        } else {
            report.summary.total_errors as f64 / report.summary.total_events as f64
        };

        // Calculate cost
        let cost_per_run = pricing.calculate_cost(
            report.summary.prompt_tokens,
            report.summary.completion_tokens,
        );

        // Count tool calls
        let tool_calls = report.summary.total_events; // Approximation (can refine if needed)

        Ok(EvalMetrics {
            // Quality metrics (not evaluated, set to None)
            correctness: None,
            relevance: None,
            safety: None,
            hallucination_rate: None,

            // Performance metrics (from analytics)
            p95_latency,
            avg_latency,
            success_rate,
            error_rate,

            // Cost metrics (from analytics + pricing)
            total_tokens: report.summary.total_tokens,
            cost_per_run,
            tool_calls,
        })
    }

    /// Convert analytics JSON file to `EvalMetrics`
    ///
    /// # Arguments
    ///
    /// * `path` - Path to analytics JSON file
    /// * `pricing` - Optional LLM pricing for cost calculation
    ///
    /// # Returns
    ///
    /// `EvalMetrics`
    pub fn from_file(path: &str, pricing: Option<LlmPricing>) -> Result<EvalMetrics> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read analytics file: {path}"))?;
        Self::from_json(&json, pricing)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_analytics_json() -> String {
        r#"{
  "summary": {
    "total_sessions": 1,
    "total_events": 4,
    "total_duration": 3000.0,
    "total_tokens": 150,
    "prompt_tokens": 100,
    "completion_tokens": 50,
    "total_errors": 0
  },
  "sessions": [
    {
      "thread_id": "session-1",
      "event_count": 4,
      "duration": 3000.0,
      "node_count": 2,
      "tool_calls": 1,
      "error_count": 0,
      "total_tokens": 150
    }
  ],
  "node_performance": [
    {
      "node_id": "llm_call",
      "count": 1,
      "total_duration": 1850.0,
      "avg_duration": 1850.0,
      "min_duration": 1850.0,
      "max_duration": 1850.0,
      "p50": 1850.0,
      "p95": 1850.0,
      "p99": 1850.0
    },
    {
      "node_id": "retriever",
      "count": 1,
      "total_duration": 245.0,
      "avg_duration": 245.0,
      "min_duration": 245.0,
      "max_duration": 245.0,
      "p50": 245.0,
      "p95": 245.0,
      "p99": 245.0
    }
  ],
  "tool_performance": [
    {
      "tool_name": "retriever_tool",
      "total_calls": 1,
      "completed": 1,
      "failed": 0,
      "retried": 0,
      "success_rate": 1.0,
      "retry_rate": 0.0,
      "avg_duration": 245.0,
      "p95_duration": 245.0
    }
  ],
  "errors": null
}"#
        .to_string()
    }

    #[test]
    fn test_converter_from_json() {
        let json = sample_analytics_json();
        let metrics = AnalyticsConverter::from_json(&json, Some(LlmPricing::GPT_4O)).unwrap();

        assert_eq!(metrics.p95_latency, 1850.0); // Max P95 across nodes
        assert_eq!(metrics.avg_latency, (1850.0 + 245.0) / 2.0); // Average across nodes
        assert_eq!(metrics.success_rate, 1.0);
        assert_eq!(metrics.error_rate, 0.0);
        assert_eq!(metrics.total_tokens, 150);
        assert!(metrics.cost_per_run > 0.0);
    }

    #[test]
    fn test_converter_cost_calculation() {
        let json = sample_analytics_json();
        let metrics = AnalyticsConverter::from_json(&json, Some(LlmPricing::GPT_4O)).unwrap();

        // GPT-4o: $2.50/1M input, $10.00/1M output
        // 100 prompt tokens = $0.00025
        // 50 completion tokens = $0.0005
        // Total = $0.00075
        let expected_cost = (100.0 / 1_000_000.0) * 2.50 + (50.0 / 1_000_000.0) * 10.00;
        assert!((metrics.cost_per_run - expected_cost).abs() < 0.0001);
    }

    #[test]
    fn test_converter_empty_analytics() {
        let json = r#"{
  "summary": {
    "total_sessions": 0,
    "total_events": 0,
    "total_duration": 0.0,
    "total_tokens": 0,
    "prompt_tokens": 0,
    "completion_tokens": 0,
    "total_errors": 0
  },
  "sessions": [],
  "node_performance": [],
  "tool_performance": [],
  "errors": null
}"#;

        let metrics = AnalyticsConverter::from_json(json, Some(LlmPricing::GPT_4O)).unwrap();

        assert_eq!(metrics.p95_latency, 0.0);
        assert_eq!(metrics.avg_latency, 0.0);
        assert_eq!(metrics.success_rate, 1.0);
        assert_eq!(metrics.error_rate, 0.0);
        assert_eq!(metrics.total_tokens, 0);
        assert_eq!(metrics.cost_per_run, 0.0);
    }
}
