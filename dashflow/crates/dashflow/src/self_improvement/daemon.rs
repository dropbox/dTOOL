// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Background Analysis Daemon for Self-Improvement.
//!
//! This module implements a background daemon that continuously monitors:
//! - New execution traces in `.dashflow/traces/`
//! - Prometheus metrics for anomaly detection
//!
//! ## Analysis Triggers
//!
//! The daemon uses specialized triggers to detect issues:
//! - `SlowNodeTrigger`: Nodes taking >10s execution time
//! - `HighErrorRateTrigger`: Error rate exceeds 5%
//! - `RepeatedRetryTrigger`: More than 3 retries for the same operation
//! - `UnusedCapabilityTrigger`: Capabilities available but never used
//!
//! ## Usage
//!
//! ```bash
//! # Start daemon with default 60-second interval
//! dashflow self-improve daemon
//!
//! # Custom interval (in seconds)
//! dashflow self-improve daemon --interval 30
//! ```
//!
//! ## Design Principle
//!
//! The daemon is opt-out by default (per DESIGN_INVARIANTS.md Invariant 6).
//! All monitoring is ON by default. Users disable what they don't want.

use crate::constants::DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY;
#[cfg(feature = "dashstream")]
use crate::constants::DEFAULT_TRIGGER_CHANNEL_CAPACITY;
use crate::core::config_loader::env_vars::{
    env_bool, env_f64, env_string, env_string_or_default, env_u64, env_usize,
    DASHFLOW_SELF_IMPROVE_CLEANUP_ENABLED, DASHFLOW_SELF_IMPROVE_CLEANUP_INTERVAL,
    DASHFLOW_SELF_IMPROVE_ERROR_THRESHOLD, DASHFLOW_SELF_IMPROVE_INTERVAL,
    DASHFLOW_SELF_IMPROVE_METRICS_SOURCE, DASHFLOW_SELF_IMPROVE_MIN_TRACES,
    DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL, DASHFLOW_SELF_IMPROVE_RETRY_THRESHOLD,
    DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS, DASHFLOW_SELF_IMPROVE_TRACES_DIR,
};
use crate::introspection::ExecutionTrace;
use crate::prometheus_client::{
    queries as prom_queries, BlockingPrometheusClient, PrometheusError,
};
use crate::self_improvement::metrics as si_metrics;
use crate::self_improvement::observability::alerts::{Alert, AlertDispatcher};
use crate::self_improvement::performance::cache::MetricsCache;
use crate::self_improvement::storage::IntrospectionStorage;
use crate::self_improvement::streaming_consumer::StreamingConsumerConfig;
use crate::self_improvement::trace_retention::{RetentionPolicy, TraceRetentionManager};
use crate::self_improvement::types::{
    CapabilityGap, ExecutionPlan, GapCategory, GapManifestation, Impact, PlanCategory,
};
use chrono::{DateTime, Utc};
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

// ============================================================================
// Analysis Triggers
// ============================================================================

/// Types of analysis triggers that can fire during daemon monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisTriggerType {
    /// Node execution exceeded threshold (default: 10s)
    SlowNode {
        /// Name of the slow-executing node.
        node_name: String,
        /// Actual duration of the node execution in milliseconds.
        duration_ms: u64,
        /// Configured threshold in milliseconds that was exceeded.
        threshold_ms: u64,
    },

    /// Error rate exceeded threshold (default: 5%)
    HighErrorRate {
        /// Observed error rate (0.0 to 1.0).
        error_rate: f64,
        /// Configured error rate threshold (0.0 to 1.0).
        threshold: f64,
        /// Number of samples used to compute the rate.
        sample_count: usize,
    },

    /// Too many retries for an operation (default: >3)
    RepeatedRetry {
        /// Name or identifier of the operation being retried.
        operation: String,
        /// Observed number of retries.
        retry_count: usize,
        /// Configured retry count threshold.
        threshold: usize,
    },

    /// Capability available but never used in recent executions
    UnusedCapability {
        /// Name of the unused capability.
        capability: String,
        /// Timestamp when the capability became available.
        available_since: DateTime<Utc>,
        /// Number of executions since the capability became available.
        executions_since: usize,
    },
}

impl AnalysisTriggerType {
    /// Get a human-readable description of the trigger.
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::SlowNode {
                node_name,
                duration_ms,
                threshold_ms,
            } => {
                format!(
                    "Slow node '{}': {}ms (threshold: {}ms)",
                    node_name, duration_ms, threshold_ms
                )
            }
            Self::HighErrorRate {
                error_rate,
                threshold,
                sample_count,
            } => {
                format!(
                    "High error rate: {:.1}% (threshold: {:.1}%, samples: {})",
                    error_rate * 100.0,
                    threshold * 100.0,
                    sample_count
                )
            }
            Self::RepeatedRetry {
                operation,
                retry_count,
                threshold,
            } => {
                format!(
                    "Repeated retries for '{}': {} (threshold: {})",
                    operation, retry_count, threshold
                )
            }
            Self::UnusedCapability {
                capability,
                executions_since,
                ..
            } => {
                format!(
                    "Unused capability '{}': not used in {} executions",
                    capability, executions_since
                )
            }
        }
    }

    /// Get the severity score (0.0 - 1.0) for prioritization.
    ///
    /// Returns a normalized severity score where higher values indicate more severe issues.
    /// Handles edge cases gracefully: if thresholds are zero or invalid, returns a safe default.
    #[must_use]
    pub fn severity(&self) -> f64 {
        match self {
            Self::SlowNode {
                duration_ms,
                threshold_ms,
                ..
            } => {
                // Guard against zero threshold (would cause division by zero/NaN)
                if *threshold_ms == 0 {
                    return if *duration_ms > 0 { 1.0 } else { 0.0 };
                }
                // Scale by how much over threshold
                let ratio = *duration_ms as f64 / *threshold_ms as f64;
                (ratio / 2.0).min(1.0) // Cap at 1.0
            }
            Self::HighErrorRate {
                error_rate,
                threshold,
                ..
            } => {
                // Guard against zero threshold (would cause division by zero/NaN)
                if *threshold <= 0.0 || threshold.is_nan() {
                    return if *error_rate > 0.0 { 1.0 } else { 0.0 };
                }
                // Scale by how much over threshold
                let ratio = error_rate / threshold;
                (ratio / 2.0).min(1.0)
            }
            Self::RepeatedRetry {
                retry_count,
                threshold,
                ..
            } => {
                // Guard against zero threshold (would cause division by zero/NaN)
                if *threshold == 0 {
                    return if *retry_count > 0 { 1.0 } else { 0.0 };
                }
                let ratio = *retry_count as f64 / *threshold as f64;
                (ratio / 2.0).min(1.0)
            }
            Self::UnusedCapability { .. } => 0.3, // Lower priority
        }
    }

    /// Get the metric name for this trigger type.
    #[must_use]
    pub const fn metric_name(&self) -> &'static str {
        match self {
            Self::SlowNode { .. } => "slow_node",
            Self::HighErrorRate { .. } => "high_error_rate",
            Self::RepeatedRetry { .. } => "repeated_retry",
            Self::UnusedCapability { .. } => "unused_capability",
        }
    }
}

/// A fired analysis trigger with context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiredTrigger {
    /// Unique identifier for this trigger event
    pub id: Uuid,
    /// When the trigger fired
    pub fired_at: DateTime<Utc>,
    /// Type of trigger
    pub trigger_type: AnalysisTriggerType,
    /// Related trace IDs (if applicable)
    pub trace_ids: Vec<String>,
    /// Whether this trigger has been processed
    pub processed: bool,
}

impl FiredTrigger {
    /// Create a new fired trigger.
    #[must_use]
    pub fn new(trigger_type: AnalysisTriggerType) -> Self {
        Self {
            id: Uuid::new_v4(),
            fired_at: Utc::now(),
            trigger_type,
            trace_ids: Vec::new(),
            processed: false,
        }
    }

    /// Add related trace IDs.
    #[must_use]
    pub fn with_traces(mut self, trace_ids: Vec<String>) -> Self {
        self.trace_ids = trace_ids;
        self
    }
}

// ============================================================================
// Trigger Implementations
// ============================================================================

/// Configuration for slow node detection.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SlowNodeConfig {
    /// Duration threshold in milliseconds (default: 10000)
    pub threshold_ms: u64,
    /// Minimum executions before triggering
    pub min_samples: usize,
}

impl Default for SlowNodeConfig {
    fn default() -> Self {
        Self {
            threshold_ms: 10_000, // 10 seconds
            min_samples: 1,
        }
    }
}

/// Detector for slow node execution.
pub struct SlowNodeTrigger {
    config: SlowNodeConfig,
}

impl SlowNodeTrigger {
    /// Create a new slow node trigger with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(SlowNodeConfig::default())
    }

    /// Create with custom config.
    #[must_use]
    pub fn with_config(config: SlowNodeConfig) -> Self {
        Self { config }
    }

    /// Check traces for slow nodes.
    pub fn check(&self, traces: &[ExecutionTrace]) -> Vec<FiredTrigger> {
        let mut triggers = Vec::new();

        for trace in traces {
            // Check node executions for slow nodes
            for node_exec in &trace.nodes_executed {
                if node_exec.duration_ms > self.config.threshold_ms {
                    let trigger = FiredTrigger::new(AnalysisTriggerType::SlowNode {
                        node_name: node_exec.node.clone(),
                        duration_ms: node_exec.duration_ms,
                        threshold_ms: self.config.threshold_ms,
                    })
                    .with_traces(vec![trace.thread_id.clone().unwrap_or_default()]);
                    triggers.push(trigger);
                }
            }
        }

        triggers
    }
}

impl Default for SlowNodeTrigger {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for high error rate detection.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HighErrorRateConfig {
    /// Error rate threshold (default: 0.05 = 5%)
    pub threshold: f64,
    /// Minimum samples before triggering
    pub min_samples: usize,
}

impl Default for HighErrorRateConfig {
    fn default() -> Self {
        Self {
            threshold: 0.05, // 5%
            min_samples: 10,
        }
    }
}

/// Detector for high error rates.
pub struct HighErrorRateTrigger {
    config: HighErrorRateConfig,
}

impl HighErrorRateTrigger {
    /// Create a new high error rate trigger with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(HighErrorRateConfig::default())
    }

    /// Create with custom config.
    #[must_use]
    pub fn with_config(config: HighErrorRateConfig) -> Self {
        Self { config }
    }

    /// Check traces for high error rate.
    ///
    /// Returns empty if there are fewer traces than `min_samples` or if traces is empty.
    pub fn check(&self, traces: &[ExecutionTrace]) -> Vec<FiredTrigger> {
        // Guard against empty traces (would cause division by zero producing NaN)
        if traces.is_empty() || traces.len() < self.config.min_samples {
            return Vec::new();
        }

        let total = traces.len();
        let errors = traces.iter().filter(|t| !t.completed).count();
        let error_rate = errors as f64 / total as f64;

        if error_rate > self.config.threshold {
            let trace_ids: Vec<String> =
                traces.iter().filter_map(|t| t.thread_id.clone()).collect();

            vec![FiredTrigger::new(AnalysisTriggerType::HighErrorRate {
                error_rate,
                threshold: self.config.threshold,
                sample_count: total,
            })
            .with_traces(trace_ids)]
        } else {
            Vec::new()
        }
    }
}

impl Default for HighErrorRateTrigger {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for repeated retry detection.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RepeatedRetryConfig {
    /// Maximum retries before triggering (default: 3)
    pub threshold: usize,
}

impl Default for RepeatedRetryConfig {
    fn default() -> Self {
        Self { threshold: 3 }
    }
}

/// Detector for repeated retries.
pub struct RepeatedRetryTrigger {
    config: RepeatedRetryConfig,
}

impl RepeatedRetryTrigger {
    /// Create a new repeated retry trigger with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(RepeatedRetryConfig::default())
    }

    /// Create with custom config.
    #[must_use]
    pub fn with_config(config: RepeatedRetryConfig) -> Self {
        Self { config }
    }

    /// Check traces for repeated retries.
    ///
    /// Detects retries by counting failed node executions within traces.
    /// If a trace has multiple failures for the same node, it indicates retries.
    pub fn check(&self, traces: &[ExecutionTrace]) -> Vec<FiredTrigger> {
        let mut triggers = Vec::new();

        // Count errors per node across all traces
        let mut error_counts: HashMap<String, (usize, Vec<String>)> = HashMap::new();

        for trace in traces {
            let thread_id = trace.thread_id.clone().unwrap_or_default();

            // Count failed node executions
            for node_exec in &trace.nodes_executed {
                if !node_exec.success {
                    let entry = error_counts
                        .entry(node_exec.node.clone())
                        .or_insert((0, Vec::new()));
                    entry.0 += 1;
                    if !entry.1.contains(&thread_id) {
                        entry.1.push(thread_id.clone());
                    }
                }
            }

            // Also count trace-level errors
            for error in &trace.errors {
                let entry = error_counts
                    .entry(error.node.clone())
                    .or_insert((0, Vec::new()));
                entry.0 += 1;
                if !entry.1.contains(&thread_id) {
                    entry.1.push(thread_id.clone());
                }
            }
        }

        for (operation, (retry_count, trace_ids)) in error_counts {
            if retry_count > self.config.threshold {
                triggers.push(
                    FiredTrigger::new(AnalysisTriggerType::RepeatedRetry {
                        operation,
                        retry_count,
                        threshold: self.config.threshold,
                    })
                    .with_traces(trace_ids),
                );
            }
        }

        triggers
    }
}

impl Default for RepeatedRetryTrigger {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for unused capability detection.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct UnusedCapabilityConfig {
    /// Minimum executions to consider capability unused
    pub min_executions: usize,
    /// Known capabilities to check for usage
    pub known_capabilities: Vec<String>,
}

impl Default for UnusedCapabilityConfig {
    fn default() -> Self {
        Self {
            min_executions: 50,
            known_capabilities: Vec::new(),
        }
    }
}

/// Detector for unused capabilities.
pub struct UnusedCapabilityTrigger {
    config: UnusedCapabilityConfig,
}

impl UnusedCapabilityTrigger {
    /// Create a new unused capability trigger with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(UnusedCapabilityConfig::default())
    }

    /// Create with custom config.
    #[must_use]
    pub fn with_config(config: UnusedCapabilityConfig) -> Self {
        Self { config }
    }

    /// Add known capabilities to check for.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.config.known_capabilities = capabilities;
        self
    }

    /// Check for unused capabilities across traces.
    pub fn check(&self, traces: &[ExecutionTrace]) -> Vec<FiredTrigger> {
        if traces.len() < self.config.min_executions {
            return Vec::new();
        }

        let mut triggers = Vec::new();

        // Collect all used nodes from traces
        let used_nodes: HashSet<String> = traces
            .iter()
            .flat_map(|t| t.nodes_executed.iter().map(|n| n.node.clone()))
            .collect();

        // Check which known capabilities weren't used
        for capability in &self.config.known_capabilities {
            if !used_nodes.contains(capability) {
                // Parse the oldest trace timestamp
                let oldest_time = traces
                    .iter()
                    .filter_map(|t| t.started_at.as_ref())
                    .filter_map(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .min()
                    .unwrap_or_else(Utc::now);

                triggers.push(FiredTrigger::new(AnalysisTriggerType::UnusedCapability {
                    capability: capability.clone(),
                    available_since: oldest_time,
                    executions_since: traces.len(),
                }));
            }
        }

        triggers
    }
}

impl Default for UnusedCapabilityTrigger {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Metrics Source
// ============================================================================

/// Source for metrics data in the daemon.
///
/// Allows bypassing Prometheus HTTP calls when running locally by computing
/// metrics directly from cached traces.
#[derive(Debug, Clone, Default)]
pub enum MetricsSource {
    /// Fetch metrics from Prometheus via HTTP (default).
    #[default]
    Http,
    /// Compute metrics from in-process cached traces (no HTTP).
    /// Useful for local development and testing.
    InProcess,
    /// Disable metrics fetching entirely.
    Disabled,
}

// ============================================================================
// Daemon Configuration and State
// ============================================================================

/// Configuration for the background analysis daemon.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DaemonConfig {
    /// Interval between analysis runs (default: 60 seconds)
    pub interval: Duration,
    /// Directory to watch for new traces
    pub traces_dir: PathBuf,
    /// Prometheus endpoint for metrics (optional, used when metrics_source is Http)
    pub prometheus_endpoint: Option<String>,
    /// Metrics source: Http (default), InProcess, or Disabled
    pub metrics_source: MetricsSource,
    /// Slow node threshold in milliseconds
    pub slow_node_threshold_ms: u64,
    /// Error rate threshold
    pub error_rate_threshold: f64,
    /// Retry threshold
    pub retry_threshold: usize,
    /// Known capabilities to track
    pub known_capabilities: Vec<String>,
    /// Minimum traces before analysis
    pub min_traces_for_analysis: usize,
    /// Optional streaming configuration for Kafka-based analysis
    /// When set, the daemon consumes from Kafka instead of watching trace files.
    pub streaming_config: Option<StreamingConsumerConfig>,
    /// Enable automatic cleanup (default: true)
    pub cleanup_enabled: bool,
    /// Cycles between cleanup runs (default: 10, meaning cleanup every 10th cycle)
    pub cleanup_interval_cycles: usize,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(60),
            traces_dir: PathBuf::from(".dashflow/traces"),
            prometheus_endpoint: Some("http://localhost:9090".to_string()),
            metrics_source: MetricsSource::Http,
            slow_node_threshold_ms: 10_000,
            error_rate_threshold: 0.05,
            retry_threshold: 3,
            known_capabilities: Vec::new(),
            min_traces_for_analysis: 5,
            streaming_config: None,
            cleanup_enabled: true,
            cleanup_interval_cycles: 10, // Cleanup every 10 cycles
        }
    }
}

impl DaemonConfig {
    /// Create config with custom interval.
    #[must_use]
    pub fn with_interval(mut self, seconds: u64) -> Self {
        self.interval = Duration::from_secs(seconds);
        self
    }

    /// Set Prometheus endpoint.
    #[must_use]
    pub fn with_prometheus(mut self, endpoint: impl Into<String>) -> Self {
        self.prometheus_endpoint = Some(endpoint.into());
        self
    }

    /// Disable Prometheus metrics.
    #[must_use]
    pub fn without_prometheus(mut self) -> Self {
        self.prometheus_endpoint = None;
        self
    }

    /// Set metrics source.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Use in-process metrics (no HTTP to Prometheus)
    /// let config = DaemonConfig::default()
    ///     .with_metrics_source(MetricsSource::InProcess);
    ///
    /// // Disable metrics entirely
    /// let config = DaemonConfig::default()
    ///     .with_metrics_source(MetricsSource::Disabled);
    /// ```
    #[must_use]
    pub fn with_metrics_source(mut self, source: MetricsSource) -> Self {
        self.metrics_source = source;
        self
    }

    /// Use in-process metrics instead of Prometheus HTTP.
    ///
    /// Computes metrics directly from cached traces, avoiding HTTP roundtrips.
    /// Useful for local development and testing.
    #[must_use]
    pub fn with_in_process_metrics(mut self) -> Self {
        self.metrics_source = MetricsSource::InProcess;
        self
    }

    /// Set known capabilities to track.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.known_capabilities = capabilities;
        self
    }

    /// Enable streaming mode with Kafka consumer.
    ///
    /// When streaming is enabled, the daemon consumes from Kafka instead of
    /// watching trace files. This enables real-time analysis of streaming metrics.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = DaemonConfig::default()
    ///     .with_streaming(StreamingConsumerConfig {
    ///         bootstrap_servers: "localhost:9092".to_string(),
    ///         topic: "dashstream-metrics".to_string(),
    ///         ..Default::default()
    ///     });
    /// ```
    #[must_use]
    pub fn with_streaming(mut self, config: StreamingConsumerConfig) -> Self {
        self.streaming_config = Some(config);
        self
    }

    /// Check if streaming mode is enabled.
    #[must_use]
    pub fn is_streaming_enabled(&self) -> bool {
        self.streaming_config.is_some()
    }

    /// Create configuration from environment variables.
    ///
    /// # Environment Variables
    ///
    /// | Variable | Type | Default | Description |
    /// |----------|------|---------|-------------|
    /// | `DASHFLOW_SELF_IMPROVE_INTERVAL` | u64 | 60 | Interval in seconds between analysis runs |
    /// | `DASHFLOW_SELF_IMPROVE_TRACES_DIR` | path | `.dashflow/traces` | Directory to watch for traces |
    /// | `DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL` | URL | `http://localhost:9090` | Prometheus endpoint |
    /// | `DASHFLOW_SELF_IMPROVE_METRICS_SOURCE` | string | `http` | `http`, `in_process`, or `disabled` |
    /// | `DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS` | u64 | 10000 | Slow node threshold in ms |
    /// | `DASHFLOW_SELF_IMPROVE_ERROR_THRESHOLD` | f64 | 0.05 | Error rate threshold (0.0-1.0) |
    /// | `DASHFLOW_SELF_IMPROVE_RETRY_THRESHOLD` | usize | 3 | Retry count threshold |
    /// | `DASHFLOW_SELF_IMPROVE_MIN_TRACES` | usize | 5 | Min traces before analysis |
    /// | `DASHFLOW_SELF_IMPROVE_CLEANUP_ENABLED` | bool | true | Enable automatic cleanup |
    /// | `DASHFLOW_SELF_IMPROVE_CLEANUP_INTERVAL` | usize | 10 | Cycles between cleanups |
    ///
    /// # Example
    ///
    /// ```bash
    /// export DASHFLOW_SELF_IMPROVE_INTERVAL=30
    /// export DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS=5000
    /// export DASHFLOW_SELF_IMPROVE_METRICS_SOURCE=in_process
    /// ```
    #[must_use]
    pub fn from_env() -> Self {
        let interval_secs = env_u64(DASHFLOW_SELF_IMPROVE_INTERVAL, 60);

        let traces_dir = PathBuf::from(env_string_or_default(
            DASHFLOW_SELF_IMPROVE_TRACES_DIR,
            ".dashflow/traces",
        ));

        let prometheus_endpoint = Some(env_string_or_default(
            DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL,
            "http://localhost:9090",
        ));

        let metrics_source = env_string(DASHFLOW_SELF_IMPROVE_METRICS_SOURCE)
            .map(|s| match s.to_lowercase().as_str() {
                "in_process" | "inprocess" => MetricsSource::InProcess,
                "disabled" | "none" | "off" => MetricsSource::Disabled,
                _ => MetricsSource::Http,
            })
            .unwrap_or_default();

        let slow_node_threshold_ms = env_u64(DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS, 10_000);
        let error_rate_threshold = env_f64(DASHFLOW_SELF_IMPROVE_ERROR_THRESHOLD, 0.05);
        let retry_threshold = env_usize(DASHFLOW_SELF_IMPROVE_RETRY_THRESHOLD, 3);
        let min_traces_for_analysis = env_usize(DASHFLOW_SELF_IMPROVE_MIN_TRACES, 5);
        let cleanup_enabled = env_bool(DASHFLOW_SELF_IMPROVE_CLEANUP_ENABLED, true);
        let cleanup_interval_cycles = env_usize(DASHFLOW_SELF_IMPROVE_CLEANUP_INTERVAL, 10);

        Self {
            interval: Duration::from_secs(interval_secs),
            traces_dir,
            prometheus_endpoint,
            metrics_source,
            slow_node_threshold_ms,
            error_rate_threshold,
            retry_threshold,
            known_capabilities: Vec::new(), // Set via with_capabilities() or config file
            min_traces_for_analysis,
            streaming_config: None, // Set via with_streaming()
            cleanup_enabled,
            cleanup_interval_cycles,
        }
    }
}

/// Result of a daemon analysis cycle.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DaemonCycleResult {
    /// When the cycle started
    pub started_at: DateTime<Utc>,
    /// When the cycle completed
    pub completed_at: DateTime<Utc>,
    /// Number of traces analyzed
    pub traces_analyzed: usize,
    /// Triggers that fired
    pub triggers_fired: Vec<FiredTrigger>,
    /// Plans generated (if any)
    pub plans_generated: Vec<ExecutionPlan>,
    /// Errors encountered
    pub errors: Vec<String>,
}

/// The background analysis daemon.
pub struct AnalysisDaemon {
    config: DaemonConfig,
    storage: IntrospectionStorage,
    slow_node_trigger: SlowNodeTrigger,
    error_rate_trigger: HighErrorRateTrigger,
    retry_trigger: RepeatedRetryTrigger,
    unused_capability_trigger: UnusedCapabilityTrigger,
    last_processed_trace: Option<DateTime<Utc>>,
    running: Arc<AtomicBool>,
    /// Optional alert dispatcher for firing alerts
    alert_dispatcher: Option<Arc<AlertDispatcher>>,
    /// In-memory cache for traces to avoid repeated disk reads
    metrics_cache: MetricsCache,
    /// Cycle counter for cleanup scheduling
    cycle_count: usize,
}

impl AnalysisDaemon {
    /// Create a new daemon with default configuration.
    #[must_use]
    pub fn new(storage: IntrospectionStorage) -> Self {
        Self::with_config(storage, DaemonConfig::default())
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(storage: IntrospectionStorage, config: DaemonConfig) -> Self {
        let slow_node_trigger = SlowNodeTrigger::with_config(SlowNodeConfig {
            threshold_ms: config.slow_node_threshold_ms,
            min_samples: 1,
        });

        let error_rate_trigger = HighErrorRateTrigger::with_config(HighErrorRateConfig {
            threshold: config.error_rate_threshold,
            min_samples: config.min_traces_for_analysis,
        });

        let retry_trigger = RepeatedRetryTrigger::with_config(RepeatedRetryConfig {
            threshold: config.retry_threshold,
        });

        let unused_capability_trigger =
            UnusedCapabilityTrigger::new().with_capabilities(config.known_capabilities.clone());

        Self {
            config,
            storage,
            slow_node_trigger,
            error_rate_trigger,
            retry_trigger,
            unused_capability_trigger,
            last_processed_trace: None,
            running: Arc::new(AtomicBool::new(false)),
            alert_dispatcher: None,
            metrics_cache: MetricsCache::default(),
            cycle_count: 0,
        }
    }

    /// Attach an alert dispatcher to fire alerts when triggers are detected.
    ///
    /// When configured, the daemon will automatically convert triggers to alerts
    /// and dispatch them through the provided dispatcher.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::self_improvement::{
    ///     AnalysisDaemon, AlertDispatcher, ConsoleAlertHandler, FileAlertHandler,
    /// };
    ///
    /// let dispatcher = AlertDispatcher::new()
    ///     .with_handler(Box::new(ConsoleAlertHandler::new()))
    ///     .with_handler(Box::new(FileAlertHandler::new(".dashflow/alerts.log")));
    ///
    /// let daemon = AnalysisDaemon::new(storage)
    ///     .with_alert_dispatcher(dispatcher);
    /// ```
    #[must_use]
    pub fn with_alert_dispatcher(mut self, dispatcher: AlertDispatcher) -> Self {
        self.alert_dispatcher = Some(Arc::new(dispatcher));
        self
    }

    /// Check if the daemon has an alert dispatcher configured.
    #[must_use]
    pub fn has_alert_dispatcher(&self) -> bool {
        self.alert_dispatcher.is_some()
    }

    /// Get a handle to stop the daemon.
    #[must_use]
    pub fn stop_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Check if the daemon is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Run a single analysis cycle.
    #[tracing::instrument(skip(self), fields(cycle = self.cycle_count))]
    pub fn run_cycle(&mut self) -> DaemonCycleResult {
        let started_at = Utc::now();
        let mut errors = Vec::new();
        let mut all_triggers = Vec::new();

        // Load recent traces
        let traces = match self.load_recent_traces() {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("Failed to load traces: {}", e));
                return DaemonCycleResult {
                    started_at,
                    completed_at: Utc::now(),
                    traces_analyzed: 0,
                    triggers_fired: Vec::new(),
                    plans_generated: Vec::new(),
                    errors,
                };
            }
        };

        let traces_analyzed = traces.len();

        if traces.len() >= self.config.min_traces_for_analysis {
            // Run all triggers in parallel
            // Use rayon::join for parallel execution of trigger checks
            let (slow_results, (error_results, (retry_results, unused_results))) = rayon::join(
                || self.slow_node_trigger.check(&traces),
                || {
                    rayon::join(
                        || self.error_rate_trigger.check(&traces),
                        || {
                            rayon::join(
                                || self.retry_trigger.check(&traces),
                                || self.unused_capability_trigger.check(&traces),
                            )
                        },
                    )
                },
            );

            all_triggers.extend(slow_results);
            all_triggers.extend(error_results);
            all_triggers.extend(retry_results);
            all_triggers.extend(unused_results);
        }

        // Fetch metrics based on configured source
        match &self.config.metrics_source {
            MetricsSource::Http => {
                if let Some(ref endpoint) = self.config.prometheus_endpoint {
                    match self.fetch_prometheus_metrics(endpoint) {
                        Ok(additional_triggers) => all_triggers.extend(additional_triggers),
                        Err(e) => {
                            errors.push(format!("Failed to fetch Prometheus metrics: {}", e));
                        }
                    }
                }
            }
            MetricsSource::InProcess => {
                // Compute metrics from cached traces instead of HTTP
                match self.compute_metrics_from_traces(&traces) {
                    Ok(additional_triggers) => all_triggers.extend(additional_triggers),
                    Err(e) => {
                        errors.push(format!("Failed to compute in-process metrics: {}", e));
                    }
                }
            }
            MetricsSource::Disabled => {
                // No metrics fetching
            }
        }

        // Generate plans from triggered issues
        let plans_generated = self.generate_plans_from_triggers(&all_triggers);

        // Save plans to storage
        for plan in &plans_generated {
            if let Err(e) = self.storage.save_plan(plan) {
                errors.push(format!("Failed to save plan {}: {}", plan.id, e));
            }
        }

        // Update last processed timestamp
        let latest = traces
            .iter()
            .filter_map(|t| t.ended_at.as_ref())
            .filter_map(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .max();
        if let Some(latest) = latest {
            self.last_processed_trace = Some(latest);
        }

        // Run cleanup every N cycles
        self.cycle_count += 1;
        if self.config.cleanup_enabled
            && self.config.cleanup_interval_cycles > 0
            && self.cycle_count % self.config.cleanup_interval_cycles == 0
        {
            if let Err(e) = self.run_cleanup() {
                errors.push(format!("Cleanup failed: {}", e));
            }
        }

        let completed_at = Utc::now();

        // Record metrics
        let duration_seconds = (completed_at - started_at).num_milliseconds() as f64 / 1000.0;
        si_metrics::record_cycle_complete(
            traces_analyzed as u64,
            all_triggers.len(),
            duration_seconds,
        );

        // Record individual trigger types
        for trigger in &all_triggers {
            si_metrics::record_trigger_fired(trigger.trigger_type.metric_name());
        }

        // Record errors
        if !errors.is_empty() {
            for _ in &errors {
                si_metrics::record_error("daemon");
            }
        }

        // Record plans generated
        for _ in &plans_generated {
            si_metrics::record_plan_generated();
        }

        DaemonCycleResult {
            started_at,
            completed_at,
            traces_analyzed,
            triggers_fired: all_triggers,
            plans_generated,
            errors,
        }
    }

    /// Start the daemon in background mode with file watching.
    ///
    /// The daemon uses `notify` to watch the traces directory for new files,
    /// providing instant detection of new traces. Falls back to
    /// interval-based polling if file watching fails to start.
    ///
    /// Returns a handle that can be used to stop the daemon.
    ///
    /// When an `AlertDispatcher` is configured via `with_alert_dispatcher()`,
    /// fired triggers are automatically converted to alerts and dispatched.
    pub async fn start(&mut self) -> Arc<AtomicBool> {
        self.running.store(true, Ordering::SeqCst);
        let stop_handle = Arc::clone(&self.running);

        // Try to set up file watching for instant trace detection
        let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY);
        let watcher_result = self.setup_file_watcher(tx);

        let use_file_watching = watcher_result.is_ok();
        let _watcher = watcher_result.ok(); // Keep watcher alive

        if use_file_watching {
            tracing::info!(
                "Daemon using file watching on {:?} (instant detection)",
                self.config.traces_dir
            );
        } else {
            tracing::warn!(
                "File watching unavailable, falling back to {:?} polling interval",
                self.config.interval
            );
        }

        while self.running.load(Ordering::SeqCst) {
            // Wait for either a file event or timeout
            let should_run_cycle = if use_file_watching {
                // Use select to wait for file event OR timeout
                tokio::select! {
                    event = rx.recv() => {
                        // New file event - check if it's a create event
                        match event {
                            Some(Ok(e)) if e.kind.is_create() => {
                                tracing::debug!("New trace file detected: {:?}", e.paths);
                                true
                            }
                            Some(Ok(_)) => false, // Other events (modify, remove) - skip
                            Some(Err(e)) => {
                                tracing::warn!("File watcher error: {}", e);
                                false
                            }
                            None => {
                                tracing::warn!("File watcher channel closed");
                                false
                            }
                        }
                    }
                    _ = tokio::time::sleep(self.config.interval) => {
                        // Timeout - run cycle anyway for Prometheus checks
                        true
                    }
                }
            } else {
                // No file watching - just sleep
                tokio::time::sleep(self.config.interval).await;
                true
            };

            if !should_run_cycle {
                continue;
            }

            // Use block_in_place to run blocking code (file I/O, rayon parallel work)
            // without starving the tokio runtime. This allows keeping &mut self.
            let result = tokio::task::block_in_place(|| self.run_cycle());

            // Dispatch alerts if dispatcher is configured
            if let Some(ref dispatcher) = self.alert_dispatcher {
                for trigger in &result.triggers_fired {
                    let alert = Alert::from_trigger(trigger);
                    // Log but don't fail if dispatch errors
                    if let Err(e) = dispatcher.dispatch(&alert).await {
                        tracing::warn!(error = %e, "Failed to dispatch alert");
                    }
                }
            }
        }

        stop_handle
    }

    /// Start the daemon in streaming mode.
    ///
    /// Consumes from Kafka instead of watching trace files, enabling real-time
    /// analysis of streaming metrics. Requires the `dashstream` feature.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = DaemonConfig::default()
    ///     .with_streaming(StreamingConsumerConfig::default());
    /// let mut daemon = AnalysisDaemon::with_config(storage, config);
    /// daemon.start_streaming().await;
    /// ```
    #[cfg(feature = "dashstream")]
    pub async fn start_streaming(&mut self) -> Result<Arc<AtomicBool>, String> {
        use crate::self_improvement::streaming_consumer::start_streaming_consumer;

        let streaming_config = self.config.streaming_config.clone().ok_or_else(|| {
            "Streaming config not set. Use DaemonConfig::with_streaming()".to_string()
        })?;

        self.running.store(true, Ordering::SeqCst);
        let stop_handle = Arc::clone(&self.running);

        // Create channel for triggers from streaming consumer
        let (trigger_tx, mut trigger_rx) = mpsc::channel::<Vec<FiredTrigger>>(DEFAULT_TRIGGER_CHANNEL_CAPACITY);

        // Start streaming consumer in background task
        let consumer_stop = Arc::clone(&self.running);
        let consumer_config = streaming_config.clone();
        tokio::spawn(async move {
            if let Err(e) =
                start_streaming_consumer(consumer_config, trigger_tx, consumer_stop).await
            {
                tracing::error!("Streaming consumer error: {}", e);
            }
        });

        tracing::info!(
            "Daemon started in streaming mode: {}:{}",
            streaming_config.bootstrap_servers,
            streaming_config.topic
        );

        // Process triggers from streaming consumer
        let alert_dispatcher = self.alert_dispatcher.clone();
        while self.running.load(Ordering::SeqCst) {
            tokio::select! {
                Some(triggers) = trigger_rx.recv() => {
                    // Dispatch alerts if configured
                    if let Some(ref dispatcher) = alert_dispatcher {
                        for trigger in &triggers {
                            let alert = Alert::from_trigger(trigger);
                            if let Err(e) = dispatcher.dispatch(&alert).await {
                                tracing::warn!(error = %e, "Failed to dispatch alert");
                            }
                        }
                    }

                    tracing::debug!("Received {} triggers from streaming", triggers.len());
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // Periodic check - keep running
                }
            }
        }

        Ok(stop_handle)
    }

    /// Check if streaming mode is available and configured.
    #[must_use]
    pub fn is_streaming_mode(&self) -> bool {
        self.config.streaming_config.is_some()
    }

    /// Set up file watcher for the traces directory.
    ///
    /// Returns the watcher on success, which must be kept alive for watching to work.
    fn setup_file_watcher(
        &self,
        tx: mpsc::Sender<notify::Result<Event>>,
    ) -> Result<RecommendedWatcher, notify::Error> {
        // Create directory if it doesn't exist
        if !self.config.traces_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&self.config.traces_dir) {
                tracing::warn!(
                    path = %self.config.traces_dir.display(),
                    error = %e,
                    "Failed to create traces directory; file watching may fail"
                );
            }
        }

        // Create the watcher with a channel callback
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                // Send event to async channel (blocking send is fine in callback)
                let _ = tx.blocking_send(res);
            },
            NotifyConfig::default(),
        )?;

        // Watch the traces directory (non-recursive since traces are flat files)
        watcher.watch(&self.config.traces_dir, RecursiveMode::NonRecursive)?;

        Ok(watcher)
    }

    /// Run the daemon synchronously for a specified number of cycles.
    ///
    /// Useful for testing or single-run scenarios.
    pub fn run_cycles(&mut self, count: usize) -> Vec<DaemonCycleResult> {
        let mut results = Vec::new();

        for _ in 0..count {
            results.push(self.run_cycle());
            std::thread::sleep(self.config.interval);
        }

        results
    }

    /// Run cleanup for both traces and storage.
    ///
    /// This method is called automatically every N cycles based on
    /// `cleanup_interval_cycles` configuration. It can also be called
    /// manually.
    ///
    /// # Errors
    ///
    /// Returns error if cleanup fails.
    pub fn run_cleanup(&self) -> Result<(), String> {
        tracing::debug!("Running automatic cleanup (cycle {})", self.cycle_count);

        // Cleanup traces
        let trace_manager =
            TraceRetentionManager::new(&self.config.traces_dir, RetentionPolicy::from_env());
        let trace_stats = trace_manager
            .cleanup()
            .map_err(|e| format!("Trace cleanup failed: {}", e))?;

        if trace_stats.deleted_count > 0 || trace_stats.compressed_count > 0 {
            tracing::info!(
                "Trace cleanup: deleted {} traces, compressed {}, freed {} bytes",
                trace_stats.deleted_count,
                trace_stats.compressed_count,
                trace_stats.freed_bytes + trace_stats.compression_saved_bytes
            );
        }

        // Cleanup storage (reports, plans, hypotheses)
        let storage_stats = self
            .storage
            .cleanup()
            .map_err(|e| format!("Storage cleanup failed: {}", e))?;

        if storage_stats.total_deleted > 0 {
            tracing::info!(
                "Storage cleanup: deleted {} items (reports: {}, plans: {}, hypotheses: {}), freed {} bytes",
                storage_stats.total_deleted,
                storage_stats.reports_deleted,
                storage_stats.plans_deleted,
                storage_stats.hypotheses_deleted,
                storage_stats.bytes_freed
            );
        }

        Ok(())
    }

    // ========================================================================
    // Private Helper Methods
    // ========================================================================

    /// Load recent traces from the traces directory with caching.
    ///
    /// Uses an LRU cache to avoid re-reading trace files from disk on every
    /// analysis cycle. Only reads files that are not already cached.
    #[tracing::instrument(skip(self), fields(dir = %self.config.traces_dir.display()))]
    fn load_recent_traces(&mut self) -> Result<Vec<ExecutionTrace>, String> {
        let traces_dir = &self.config.traces_dir;

        if !traces_dir.exists() {
            return Ok(Vec::new());
        }

        let entries = std::fs::read_dir(traces_dir)
            .map_err(|e| format!("Failed to read traces directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let path_str = path.to_string_lossy().to_string();

                // Skip if already cached
                if self.metrics_cache.contains(&path_str) {
                    continue;
                }

                // Check if this trace is newer than last processed
                if let Some(last) = self.last_processed_trace {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            let modified_time: DateTime<Utc> = modified.into();
                            if modified_time <= last {
                                continue;
                            }
                        }
                    }
                }

                // Read from disk and cache
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<ExecutionTrace>(&content) {
                        Ok(trace) => {
                            self.metrics_cache.put(path_str, trace);
                        }
                        Err(e) => {
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "Failed to parse trace"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to read trace"
                        );
                    }
                }
            }
        }

        // Return recent traces from cache for aggregation
        Ok(self
            .metrics_cache
            .all_recent_traces()
            .into_iter()
            .cloned()
            .collect())
    }

    /// Get cache statistics for monitoring.
    #[must_use]
    pub fn cache_stats(&self) -> (u64, u64, f64) {
        (
            self.metrics_cache.hits(),
            self.metrics_cache.misses(),
            self.metrics_cache.hit_rate(),
        )
    }

    /// Clear the metrics cache.
    pub fn clear_cache(&mut self) {
        self.metrics_cache.clear();
    }

    /// Compute metrics from cached traces (in-process bypass).
    ///
    /// Generates the same triggers as `fetch_prometheus_metrics` but computes
    /// metrics directly from trace data instead of querying Prometheus.
    fn compute_metrics_from_traces(
        &self,
        traces: &[ExecutionTrace],
    ) -> Result<Vec<FiredTrigger>, String> {
        let mut triggers = Vec::new();

        if traces.is_empty() {
            return Ok(triggers);
        }

        // Compute p99 node duration from traces
        let mut node_durations: HashMap<String, Vec<u64>> = HashMap::new();
        for trace in traces {
            for node_exec in &trace.nodes_executed {
                node_durations
                    .entry(node_exec.node.clone())
                    .or_default()
                    .push(node_exec.duration_ms);
            }
        }

        for (node_name, mut durations) in node_durations {
            if durations.is_empty() {
                continue;
            }
            durations.sort();
            // p99: 99th percentile
            let p99_index = (durations.len() as f64 * 0.99).ceil() as usize - 1;
            let p99_index = p99_index.min(durations.len() - 1);
            let duration_ms = durations[p99_index];

            let threshold_ms = self.config.slow_node_threshold_ms;
            if duration_ms > threshold_ms {
                triggers.push(FiredTrigger::new(AnalysisTriggerType::SlowNode {
                    node_name,
                    duration_ms,
                    threshold_ms,
                }));
            }
        }

        // Compute error rate from traces
        let total = traces.len();
        let errors = traces.iter().filter(|t| !t.completed).count();
        if total > 0 {
            let error_rate = errors as f64 / total as f64;
            let threshold = self.config.error_rate_threshold;

            if error_rate > threshold {
                triggers.push(FiredTrigger::new(AnalysisTriggerType::HighErrorRate {
                    error_rate,
                    threshold,
                    sample_count: total,
                }));
            }
        }

        // Compute retry count from traces (count failed node executions)
        let mut retry_counts: HashMap<String, usize> = HashMap::new();
        for trace in traces {
            for node_exec in &trace.nodes_executed {
                if !node_exec.success {
                    *retry_counts.entry(node_exec.node.clone()).or_default() += 1;
                }
            }
        }

        let threshold = self.config.retry_threshold;
        for (operation, retry_count) in retry_counts {
            if retry_count > threshold {
                triggers.push(FiredTrigger::new(AnalysisTriggerType::RepeatedRetry {
                    operation,
                    retry_count,
                    threshold,
                }));
            }
        }

        Ok(triggers)
    }

    /// Fetch metrics from Prometheus and generate triggers.
    fn fetch_prometheus_metrics(&self, endpoint: &str) -> Result<Vec<FiredTrigger>, String> {
        let client = BlockingPrometheusClient::with_timeout(endpoint, Duration::from_secs(5));

        // Check if Prometheus is reachable
        if !client.is_healthy() {
            return Err(format!("Prometheus at {} is not healthy", endpoint));
        }

        let mut triggers = Vec::new();

        // Query node duration p99 for slow node detection
        match client.query(prom_queries::NODE_DURATION_P99) {
            Ok(metrics) => {
                for metric in metrics {
                    // p99 duration in seconds, convert to ms
                    let duration_ms = (metric.value * 1000.0) as u64;
                    let threshold_ms = self.config.slow_node_threshold_ms;

                    if duration_ms > threshold_ms {
                        let node_name = metric
                            .labels
                            .get("node")
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());

                        triggers.push(FiredTrigger::new(AnalysisTriggerType::SlowNode {
                            node_name,
                            duration_ms,
                            threshold_ms,
                        }));
                    }
                }
            }
            Err(PrometheusError::QueryError { .. }) => {
                // Metric doesn't exist yet - not an error, just no data
            }
            Err(e) => {
                return Err(format!("Failed to query node duration: {}", e));
            }
        }

        // Query error rate
        match client.query(prom_queries::ERROR_RATE_5M) {
            Ok(metrics) => {
                for metric in metrics {
                    let error_rate = metric.value;
                    let threshold = self.config.error_rate_threshold;

                    if error_rate > threshold && !error_rate.is_nan() {
                        triggers.push(FiredTrigger::new(AnalysisTriggerType::HighErrorRate {
                            error_rate,
                            threshold,
                            sample_count: 1, // From Prometheus, sample count not directly available
                        }));
                    }
                }
            }
            Err(PrometheusError::QueryError { .. }) => {
                // Metric doesn't exist yet - not an error
            }
            Err(e) => {
                return Err(format!("Failed to query error rate: {}", e));
            }
        }

        // Query retry count
        match client.query(prom_queries::RETRIES_TOTAL) {
            Ok(metrics) => {
                for metric in metrics {
                    let retry_count = metric.value as usize;
                    let threshold = self.config.retry_threshold;

                    // Only trigger if retries exceed threshold
                    if retry_count > threshold {
                        let operation = metric
                            .labels
                            .get("operation")
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());

                        triggers.push(FiredTrigger::new(AnalysisTriggerType::RepeatedRetry {
                            operation,
                            retry_count,
                            threshold,
                        }));
                    }
                }
            }
            Err(PrometheusError::QueryError { .. }) => {
                // Metric doesn't exist yet - not an error
            }
            Err(e) => {
                return Err(format!("Failed to query retries: {}", e));
            }
        }

        Ok(triggers)
    }

    /// Generate improvement plans from fired triggers.
    fn generate_plans_from_triggers(&self, triggers: &[FiredTrigger]) -> Vec<ExecutionPlan> {
        let mut plans = Vec::new();

        for trigger in triggers {
            let plan = match &trigger.trigger_type {
                AnalysisTriggerType::SlowNode {
                    node_name,
                    duration_ms,
                    ..
                } => {
                    let gap = CapabilityGap::new(
                        format!("Performance issue: '{}' node is slow ({}ms)", node_name, duration_ms),
                        GapCategory::PerformanceGap {
                            bottleneck: node_name.clone(),
                        },
                        GapManifestation::SuboptimalPaths {
                            description: format!(
                                "Node '{}' taking {}ms, exceeding 10s threshold",
                                node_name, duration_ms
                            ),
                        },
                    )
                    .with_impact(Impact::medium("Reduce node execution time"))
                    .with_confidence(trigger.trigger_type.severity());

                    ExecutionPlan::new(
                        format!("Optimize slow node '{}'", node_name),
                        PlanCategory::Optimization,
                    )
                    .with_description(gap.description.clone())
                    .with_priority(2)
                    .with_estimated_commits(1)
                    .with_success_criteria(vec![format!(
                        "Node '{}' execution time < 10s",
                        node_name
                    )])
                    .validated(trigger.trigger_type.severity())
                }

                AnalysisTriggerType::HighErrorRate {
                    error_rate,
                    sample_count,
                    ..
                } => ExecutionPlan::new("Reduce high error rate", PlanCategory::ApplicationImprovement)
                    .with_description(format!(
                        "Error rate of {:.1}% detected over {} executions",
                        error_rate * 100.0,
                        sample_count
                    ))
                    .with_priority(1) // High priority
                    .with_estimated_commits(2)
                    .with_success_criteria(vec!["Error rate < 5%".to_string()])
                    .validated(trigger.trigger_type.severity()),

                AnalysisTriggerType::RepeatedRetry {
                    operation,
                    retry_count,
                    ..
                } => ExecutionPlan::new(
                    format!("Fix repeated retries in '{}'", operation),
                    PlanCategory::ApplicationImprovement,
                )
                .with_description(format!(
                    "Operation '{}' has {} retries, indicating potential instability",
                    operation, retry_count
                ))
                .with_priority(2)
                .with_estimated_commits(1)
                .with_success_criteria(vec![format!(
                    "Operation '{}' retry count < 3",
                    operation
                )])
                .validated(trigger.trigger_type.severity()),

                AnalysisTriggerType::UnusedCapability {
                    capability,
                    executions_since,
                    ..
                } => ExecutionPlan::new(
                    format!("Review unused capability '{}'", capability),
                    PlanCategory::ProcessImprovement,
                )
                .with_description(format!(
                    "Capability '{}' has not been used in {} executions. Consider removing or documenting.",
                    capability, executions_since
                ))
                .with_priority(4) // Lower priority
                .with_estimated_commits(1)
                .with_success_criteria(vec![
                    format!("Capability '{}' either used or removed", capability)
                ])
                .validated(trigger.trigger_type.severity()),
            };

            plans.push(plan);
        }

        plans
    }
}

// ============================================================================
// CLI Support Functions
// ============================================================================

/// Run the daemon from CLI.
///
/// # Arguments
///
/// * `interval_seconds` - Custom interval between cycles (defaults to 60s)
/// * `storage_path` - Custom storage path (defaults to `.dashflow/introspection`)
/// * `once` - If true, runs a single cycle and returns. If false, runs an infinite loop
///   that **never returns** (designed for CLI daemon mode; stop via Ctrl+C).
///
/// # Returns
///
/// - When `once=true`: Returns `Ok(DaemonCycleResult)` after completing one cycle
/// - When `once=false`: **Never returns** - runs infinite loop until process is killed
///
/// # Errors
///
/// Returns error if storage initialization fails. Does not return for continuous mode.
pub fn run_daemon_cli(
    interval_seconds: Option<u64>,
    storage_path: Option<&str>,
    once: bool,
) -> Result<DaemonCycleResult, String> {
    let storage = match storage_path {
        Some(path) => IntrospectionStorage::at_path(path),
        None => IntrospectionStorage::default(),
    };

    // Initialize storage if needed
    storage
        .initialize()
        .map_err(|e| format!("Failed to initialize storage: {}", e))?;

    let mut config = DaemonConfig::default();
    if let Some(interval) = interval_seconds {
        config = config.with_interval(interval);
    }

    let mut daemon = AnalysisDaemon::with_config(storage, config);

    if once {
        // Run a single cycle
        Ok(daemon.run_cycle())
    } else {
        // Run continuously - for CLI we use a blocking loop
        println!("Starting background analysis daemon...");
        println!("Interval: {} seconds", daemon.config.interval.as_secs());
        println!("Press Ctrl+C to stop.");
        println!();

        loop {
            let result = daemon.run_cycle();

            println!(
                "[{}] Analyzed {} traces, {} triggers fired, {} plans generated",
                result.completed_at.format("%H:%M:%S"),
                result.traces_analyzed,
                result.triggers_fired.len(),
                result.plans_generated.len()
            );

            if !result.errors.is_empty() {
                for error in &result.errors {
                    tracing::warn!(error = error, "Daemon cycle error");
                }
            }

            for trigger in &result.triggers_fired {
                println!("  Trigger: {}", trigger.trigger_type.description());
            }

            std::thread::sleep(daemon.config.interval);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ExecutionTrace, NodeExecution};
    use tempfile::tempdir;

    fn create_test_trace(thread_id: &str, success: bool) -> ExecutionTrace {
        let mut trace = ExecutionTrace::builder().thread_id(thread_id).build();
        trace.completed = success;
        trace
    }

    fn create_slow_trace(thread_id: &str, node: &str, duration_ms: u64) -> ExecutionTrace {
        let mut trace = ExecutionTrace::builder().thread_id(thread_id).build();
        trace.nodes_executed.push(NodeExecution {
            node: node.to_string(),
            duration_ms,
            tokens_used: 0,
            state_before: None,
            state_after: None,
            tools_called: Vec::new(),
            success: true,
            error_message: None,
            index: 0,
            started_at: None,
            metadata: HashMap::new(),
        });
        trace
    }

    #[test]
    fn test_slow_node_trigger() {
        let trigger = SlowNodeTrigger::new();

        // Create traces with a slow node
        let traces = vec![create_slow_trace("t1", "llm_node", 15_000)]; // 15 seconds

        let fired = trigger.check(&traces);
        assert_eq!(fired.len(), 1);

        if let AnalysisTriggerType::SlowNode {
            node_name,
            duration_ms,
            ..
        } = &fired[0].trigger_type
        {
            assert_eq!(node_name, "llm_node");
            assert_eq!(*duration_ms, 15_000);
        } else {
            panic!("Expected SlowNode trigger");
        }
    }

    #[test]
    fn test_slow_node_trigger_no_fire() {
        let trigger = SlowNodeTrigger::new();

        // Create traces with a fast node
        let traces = vec![create_slow_trace("t1", "fast_node", 100)]; // 100ms

        let fired = trigger.check(&traces);
        assert!(fired.is_empty());
    }

    #[test]
    fn test_high_error_rate_trigger() {
        let trigger = HighErrorRateTrigger::with_config(HighErrorRateConfig {
            threshold: 0.05,
            min_samples: 5,
        });

        // Create traces with high error rate (3 out of 5 = 60%)
        let traces = vec![
            create_test_trace("t1", true),
            create_test_trace("t2", false),
            create_test_trace("t3", false),
            create_test_trace("t4", false),
            create_test_trace("t5", true),
        ];

        let fired = trigger.check(&traces);
        assert_eq!(fired.len(), 1);

        if let AnalysisTriggerType::HighErrorRate {
            error_rate,
            threshold,
            sample_count,
        } = &fired[0].trigger_type
        {
            assert!((*error_rate - 0.6).abs() < 0.01);
            assert_eq!(*threshold, 0.05);
            assert_eq!(*sample_count, 5);
        } else {
            panic!("Expected HighErrorRate trigger");
        }
    }

    #[test]
    fn test_high_error_rate_trigger_not_enough_samples() {
        let trigger = HighErrorRateTrigger::with_config(HighErrorRateConfig {
            threshold: 0.05,
            min_samples: 10,
        });

        // Only 5 samples, but all failing
        let traces: Vec<ExecutionTrace> = (0..5)
            .map(|i| create_test_trace(&format!("t{}", i), false))
            .collect();

        let fired = trigger.check(&traces);
        assert!(fired.is_empty()); // Not enough samples
    }

    #[test]
    fn test_repeated_retry_trigger() {
        let trigger = RepeatedRetryTrigger::new();

        // Create traces with multiple failed node executions
        let mut trace = create_test_trace("t1", true);

        // Add 5 failed executions for the same node (exceeds threshold of 3)
        for i in 0..5 {
            trace.nodes_executed.push(NodeExecution {
                node: "search_node".to_string(),
                duration_ms: 100,
                tokens_used: 0,
                state_before: None,
                state_after: None,
                tools_called: Vec::new(),
                success: false, // Failed execution
                error_message: Some("Connection timeout".to_string()),
                index: i,
                started_at: None,
                metadata: HashMap::new(),
            });
        }

        let fired = trigger.check(&[trace]);
        assert_eq!(fired.len(), 1);

        if let AnalysisTriggerType::RepeatedRetry {
            operation,
            retry_count,
            ..
        } = &fired[0].trigger_type
        {
            assert_eq!(operation, "search_node");
            assert_eq!(*retry_count, 5);
        } else {
            panic!("Expected RepeatedRetry trigger");
        }
    }

    #[test]
    fn test_unused_capability_trigger() {
        let trigger = UnusedCapabilityTrigger::with_config(UnusedCapabilityConfig {
            min_executions: 3,
            known_capabilities: vec!["sentiment_node".to_string(), "search_node".to_string()],
        });

        // Create traces that only use search_node
        let traces: Vec<ExecutionTrace> = (0..5)
            .map(|i| create_slow_trace(&format!("t{}", i), "search_node", 100))
            .collect();

        let fired = trigger.check(&traces);
        assert_eq!(fired.len(), 1); // Only sentiment_node is unused

        if let AnalysisTriggerType::UnusedCapability { capability, .. } = &fired[0].trigger_type {
            assert_eq!(capability, "sentiment_node");
        } else {
            panic!("Expected UnusedCapability trigger");
        }
    }

    #[test]
    fn test_trigger_severity() {
        // SlowNode: 20s when threshold is 10s = ratio of 2, severity = 1.0
        let slow = AnalysisTriggerType::SlowNode {
            node_name: "test".to_string(),
            duration_ms: 20_000,
            threshold_ms: 10_000,
        };
        assert!(slow.severity() >= 0.9);

        // HighErrorRate: 10% when threshold is 5% = ratio of 2, severity = 1.0
        let error = AnalysisTriggerType::HighErrorRate {
            error_rate: 0.10,
            threshold: 0.05,
            sample_count: 100,
        };
        assert!(error.severity() >= 0.9);

        // UnusedCapability always has lower severity
        let unused = AnalysisTriggerType::UnusedCapability {
            capability: "test".to_string(),
            available_since: Utc::now(),
            executions_since: 100,
        };
        assert!(unused.severity() < 0.5);
    }

    #[test]
    fn test_daemon_cycle() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let config = DaemonConfig {
            interval: Duration::from_millis(100),
            traces_dir: dir.path().join("traces"),
            prometheus_endpoint: None, // Disable Prometheus for test
            min_traces_for_analysis: 1,
            ..DaemonConfig::default()
        };

        // Create traces directory
        std::fs::create_dir_all(&config.traces_dir).unwrap();

        // Write a slow trace
        let trace = create_slow_trace("t1", "slow_node", 15_000);
        let trace_path = config.traces_dir.join("t1.json");
        std::fs::write(&trace_path, serde_json::to_string(&trace).unwrap()).unwrap();

        let mut daemon = AnalysisDaemon::with_config(storage, config);

        let result = daemon.run_cycle();

        assert_eq!(result.traces_analyzed, 1);
        assert!(!result.triggers_fired.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_daemon_config_builder() {
        let config = DaemonConfig::default()
            .with_interval(30)
            .with_prometheus("http://prom:9090")
            .with_capabilities(vec!["cap1".to_string(), "cap2".to_string()]);

        assert_eq!(config.interval, Duration::from_secs(30));
        assert_eq!(
            config.prometheus_endpoint,
            Some("http://prom:9090".to_string())
        );
        assert_eq!(config.known_capabilities.len(), 2);
    }

    #[test]
    fn test_generate_plans_from_triggers() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let daemon = AnalysisDaemon::new(storage);

        let triggers = vec![
            FiredTrigger::new(AnalysisTriggerType::SlowNode {
                node_name: "llm".to_string(),
                duration_ms: 15_000,
                threshold_ms: 10_000,
            }),
            FiredTrigger::new(AnalysisTriggerType::HighErrorRate {
                error_rate: 0.15,
                threshold: 0.05,
                sample_count: 100,
            }),
        ];

        let plans = daemon.generate_plans_from_triggers(&triggers);

        assert_eq!(plans.len(), 2);
        assert!(plans[0].title.contains("slow node"));
        assert!(plans[1].title.contains("error rate"));
    }

    #[test]
    fn test_metrics_source_default() {
        let source = MetricsSource::default();
        assert!(matches!(source, MetricsSource::Http));
    }

    #[test]
    fn test_daemon_config_with_metrics_source() {
        let config = DaemonConfig::default().with_metrics_source(MetricsSource::InProcess);
        assert!(matches!(config.metrics_source, MetricsSource::InProcess));

        let config = DaemonConfig::default().with_in_process_metrics();
        assert!(matches!(config.metrics_source, MetricsSource::InProcess));
    }

    #[test]
    fn test_compute_metrics_from_traces_slow_node() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let daemon =
            AnalysisDaemon::with_config(storage, DaemonConfig::default().with_in_process_metrics());

        // Create traces with a slow node (p99 > threshold)
        let traces = vec![create_slow_trace("t1", "slow_llm", 15_000)];

        let triggers = daemon.compute_metrics_from_traces(&traces).unwrap();

        // Should detect the slow node
        assert!(!triggers.is_empty());
        let slow_trigger = triggers
            .iter()
            .find(|t| matches!(&t.trigger_type, AnalysisTriggerType::SlowNode { .. }));
        assert!(slow_trigger.is_some());
    }

    #[test]
    fn test_compute_metrics_from_traces_high_error_rate() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let daemon =
            AnalysisDaemon::with_config(storage, DaemonConfig::default().with_in_process_metrics());

        // Create traces with 60% failure rate (exceeds 5% threshold)
        let traces = vec![
            create_test_trace("t1", true),
            create_test_trace("t2", false),
            create_test_trace("t3", false),
            create_test_trace("t4", false),
            create_test_trace("t5", true),
        ];

        let triggers = daemon.compute_metrics_from_traces(&traces).unwrap();

        // Should detect high error rate
        let error_trigger = triggers
            .iter()
            .find(|t| matches!(&t.trigger_type, AnalysisTriggerType::HighErrorRate { .. }));
        assert!(error_trigger.is_some());
    }

    #[test]
    fn test_compute_metrics_from_traces_empty() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let daemon =
            AnalysisDaemon::with_config(storage, DaemonConfig::default().with_in_process_metrics());

        // Empty traces should return no triggers
        let triggers = daemon.compute_metrics_from_traces(&[]).unwrap();
        assert!(triggers.is_empty());
    }

    // =========================================================================
    // Additional Tests
    // =========================================================================

    #[test]
    fn test_fired_trigger_creation() {
        let trigger = FiredTrigger::new(AnalysisTriggerType::SlowNode {
            node_name: "test_node".to_string(),
            duration_ms: 15_000,
            threshold_ms: 10_000,
        });

        assert!(trigger.fired_at <= Utc::now());
        assert!(matches!(
            trigger.trigger_type,
            AnalysisTriggerType::SlowNode { .. }
        ));
    }

    #[test]
    fn test_fired_trigger_severity() {
        let trigger = FiredTrigger::new(AnalysisTriggerType::HighErrorRate {
            error_rate: 0.20,
            threshold: 0.05,
            sample_count: 100,
        });

        // High error rate (4x threshold) should have high severity
        assert!(trigger.trigger_type.severity() > 0.8);
    }

    #[test]
    fn test_daemon_cycle_result_defaults() {
        let result = DaemonCycleResult {
            started_at: Utc::now(),
            completed_at: Utc::now(),
            traces_analyzed: 10,
            triggers_fired: vec![],
            plans_generated: vec![],
            errors: vec![],
        };

        assert_eq!(result.traces_analyzed, 10);
        assert!(result.triggers_fired.is_empty());
        assert!(result.plans_generated.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_daemon_config_defaults() {
        let config = DaemonConfig::default();

        assert!(config.interval.as_secs() > 0);
        assert!(config.min_traces_for_analysis > 0);
        assert!(config.cleanup_enabled);
    }

    #[test]
    fn test_slow_node_config_defaults() {
        let config = SlowNodeConfig::default();
        assert!(config.threshold_ms > 0);
    }

    #[test]
    fn test_high_error_rate_config_defaults() {
        let config = HighErrorRateConfig::default();
        assert!(config.threshold > 0.0);
        assert!(config.min_samples > 0);
    }

    #[test]
    fn test_repeated_retry_config_defaults() {
        let config = RepeatedRetryConfig::default();
        assert!(config.threshold > 0);
    }

    #[test]
    fn test_unused_capability_config_defaults() {
        let config = UnusedCapabilityConfig::default();
        assert!(config.min_executions > 0);
    }

    #[test]
    fn test_analysis_trigger_type_description() {
        let slow = AnalysisTriggerType::SlowNode {
            node_name: "llm".to_string(),
            duration_ms: 15_000,
            threshold_ms: 10_000,
        };
        assert!(slow.description().contains("llm"));

        let error = AnalysisTriggerType::HighErrorRate {
            error_rate: 0.10,
            threshold: 0.05,
            sample_count: 100,
        };
        assert!(error.description().contains("10"));

        let retry = AnalysisTriggerType::RepeatedRetry {
            operation: "search".to_string(),
            retry_count: 5,
            threshold: 3,
        };
        assert!(retry.description().contains("search"));

        let unused = AnalysisTriggerType::UnusedCapability {
            capability: "sentiment".to_string(),
            available_since: Utc::now(),
            executions_since: 100,
        };
        assert!(unused.description().contains("sentiment"));
    }

    #[test]
    fn test_metrics_source_variants() {
        let _http = MetricsSource::Http;
        let _in_process = MetricsSource::InProcess;
        let _disabled = MetricsSource::Disabled;
    }

    #[test]
    fn test_daemon_with_cleanup_config() {
        let config = DaemonConfig::default();
        assert!(config.cleanup_enabled);
        assert!(config.cleanup_interval_cycles > 0);
    }

    #[test]
    fn test_daemon_config_traces_dir_field() {
        let mut config = DaemonConfig::default();
        let dir = std::path::PathBuf::from("/tmp/test_traces");
        config.traces_dir = dir.clone();
        assert_eq!(config.traces_dir, dir);
    }
}
