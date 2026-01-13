// Async introspection clippy exceptions:
// - clone_on_ref_ptr: Arc::clone() is idiomatic for shared state in async contexts
// - needless_pass_by_value: async move closures require owned values
// - redundant_clone: Clone before async move prevents use-after-move
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Unified Introspection API - Four Levels, ONE Command
//!
//! This module provides a single entry point for all DashFlow introspection needs,
//! unifying four distinct levels of self-awareness:
//!
//! | Level | Scope | What It Answers | Example |
//! |-------|-------|-----------------|---------|
//! | **Platform** | DashFlow framework | What modules/capabilities exist? | "Is distillation implemented?" |
//! | **Application** | User's project | What graphs/packages do I have? | "What graphs do I have?" |
//! | **Runtime** | Current execution | What's happening? Why did X happen? | "Why did search run 3 times?" |
//! | **Network** | Package ecosystem | What packages exist? Who published? | "What RAG packages exist?" |
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::unified_introspection::DashFlowIntrospection;
//!
//! // Create introspection for current directory
//! let introspection = DashFlowIntrospection::for_cwd();
//!
//! // Ask questions - routes automatically to correct level
//! let response = introspection.ask("Is distillation implemented?");  // Platform
//! let response = introspection.ask("What graphs do I have?");        // Application
//! let response = introspection.ask("Why did search run 3 times?");   // Runtime
//! let response = introspection.ask("What RAG packages exist?");      // Network
//!
//! // Search across all levels
//! let results = introspection.search("optimization");
//! ```

use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::introspection::interface::IntrospectionInterface;
use crate::introspection::ExecutionTrace;
use crate::live_introspection::{ExecutionSummary, ExecutionTracker};
use crate::optimize::auto_optimizer::{
    AutoOptimizer, OptimizationContext, OptimizationOutcome, OptimizerStats, SelectionResult,
    TaskType,
};
use crate::platform_introspection::PlatformIntrospection;
use crate::prometheus_client::PrometheusClient;

// ============================================================================
// Four Levels of Introspection
// ============================================================================

/// The four levels of introspection in DashFlow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntrospectionLevel {
    /// Platform level - DashFlow framework capabilities
    /// "Is distillation implemented?", "What node types exist?"
    Platform,

    /// Application level - User's project configuration
    /// "What graphs do I have?", "What packages are installed?"
    Application,

    /// Runtime level - Execution traces and live state
    /// "Why did search run 3 times?", "What's currently running?"
    Runtime,

    /// Network level - Package ecosystem
    /// "What RAG packages exist?", "Who published sentiment-analyzer?"
    Network,
}

impl std::fmt::Display for IntrospectionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntrospectionLevel::Platform => write!(f, "platform"),
            IntrospectionLevel::Application => write!(f, "application"),
            IntrospectionLevel::Runtime => write!(f, "runtime"),
            IntrospectionLevel::Network => write!(f, "network"),
        }
    }
}

// ============================================================================
// Unified Response Type
// ============================================================================

/// Response from unified introspection queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionResponse {
    /// Which level answered this query
    pub level: IntrospectionLevel,
    /// Natural language answer
    pub answer: String,
    /// Confidence in the classification and answer (0.0-1.0)
    pub confidence: f64,
    /// Additional details
    pub details: Vec<String>,
    /// Suggested follow-up questions
    pub follow_ups: Vec<String>,
    /// Structured data (level-specific)
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

impl IntrospectionResponse {
    /// Create a new response
    #[must_use]
    pub fn new(level: IntrospectionLevel, answer: impl Into<String>) -> Self {
        Self {
            level,
            answer: answer.into(),
            confidence: 1.0,
            details: Vec::new(),
            follow_ups: Vec::new(),
            data: HashMap::new(),
        }
    }

    /// Add confidence score
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add details
    #[must_use]
    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }

    /// Add follow-up questions
    #[must_use]
    pub fn with_follow_ups(mut self, follow_ups: Vec<String>) -> Self {
        self.follow_ups = follow_ups;
        self
    }

    /// Add structured data
    #[must_use]
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Format as human-readable report
    #[must_use]
    pub fn report(&self) -> String {
        let mut lines = vec![
            format!("Level: {}", self.level),
            String::new(),
            self.answer.clone(),
        ];

        if !self.details.is_empty() {
            lines.push(String::new());
            lines.push("Details:".to_string());
            for detail in &self.details {
                lines.push(format!("  - {}", detail));
            }
        }

        if !self.follow_ups.is_empty() {
            lines.push(String::new());
            lines.push("You might also ask:".to_string());
            for q in &self.follow_ups {
                lines.push(format!("  - {}", q));
            }
        }

        lines.push(String::new());
        lines.push(format!("Confidence: {:.0}%", self.confidence * 100.0));

        lines.join("\n")
    }
}

// ============================================================================
// Search Results
// ============================================================================

/// Search result from a specific level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelSearchResult {
    /// Which level this result came from
    pub level: IntrospectionLevel,
    /// Name of the matched item
    pub name: String,
    /// Category/type within the level
    pub category: String,
    /// Description of the match
    pub description: String,
    /// Relevance score (0.0-1.0)
    pub relevance: f64,
}

/// Aggregated search results across all levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    /// Original query
    pub query: String,
    /// Results from all levels
    pub results: Vec<LevelSearchResult>,
    /// Total result count
    pub total: usize,
}

// ============================================================================
// Metrics Snapshot (Runtime Level - Prometheus Integration)
// ============================================================================

/// A snapshot of metrics from Prometheus for a specific trace or time range.
///
/// This enables unified introspection to query the same metrics that humans
/// see in Grafana dashboards, achieving data parity (Principle 5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Trace ID this snapshot is associated with (if any)
    pub trace_id: Option<String>,
    /// Quality score from DashStream quality monitor
    pub quality_score: Option<f64>,
    /// Success rate from DashStream quality monitor
    pub success_rate: Option<f64>,
    /// Error rate over the last 5 minutes
    pub error_rate_5m: Option<f64>,
    /// P99 node duration in milliseconds
    pub node_duration_p99_ms: Option<f64>,
    /// P95 node duration in milliseconds
    pub node_duration_p95_ms: Option<f64>,
    /// Total retries in the last hour
    pub retries_total: Option<f64>,
    /// Whether Prometheus was reachable
    pub prometheus_healthy: bool,
    /// Timestamp when snapshot was taken
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional custom metrics
    #[serde(default)]
    pub custom_metrics: HashMap<String, f64>,
}

impl MetricsSnapshot {
    /// Create a snapshot indicating Prometheus is unavailable
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            trace_id: None,
            quality_score: None,
            success_rate: None,
            error_rate_5m: None,
            node_duration_p99_ms: None,
            node_duration_p95_ms: None,
            retries_total: None,
            prometheus_healthy: false,
            timestamp: chrono::Utc::now(),
            custom_metrics: HashMap::new(),
        }
    }

    /// Check if any metrics were successfully retrieved
    #[must_use]
    pub fn has_data(&self) -> bool {
        self.quality_score.is_some()
            || self.success_rate.is_some()
            || self.error_rate_5m.is_some()
            || self.node_duration_p99_ms.is_some()
            || !self.custom_metrics.is_empty()
    }

    /// Format as human-readable summary
    #[must_use]
    pub fn summary(&self) -> String {
        if !self.prometheus_healthy {
            return "Prometheus unavailable".to_string();
        }

        // At most 5 metrics (quality, success, errors, p99, retries)
        let mut parts = Vec::with_capacity(5);

        if let Some(q) = self.quality_score {
            parts.push(format!("Quality: {:.2}", q));
        }
        if let Some(s) = self.success_rate {
            parts.push(format!("Success: {:.1}%", s * 100.0));
        }
        if let Some(e) = self.error_rate_5m {
            parts.push(format!("Errors(5m): {:.2}%", e * 100.0));
        }
        if let Some(p99) = self.node_duration_p99_ms {
            parts.push(format!("P99: {:.0}ms", p99));
        }
        if let Some(retries) = self.retries_total {
            parts.push(format!("Retries(1h): {:.0}", retries));
        }

        if parts.is_empty() {
            "No metrics data available".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

// ============================================================================
// Project Info (Application Level)
// ============================================================================

/// Information about the user's DashFlow project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Project root directory
    pub root: PathBuf,
    /// Project name (from dashflow.toml or directory name)
    pub name: String,
    /// Discovered graph files
    pub graphs: Vec<GraphFileInfo>,
    /// Installed packages
    pub installed_packages: Vec<InstalledPackage>,
    /// Whether dashflow.toml exists
    pub has_config: bool,
}

/// Information about a discovered graph file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphFileInfo {
    /// File path relative to project root
    pub path: PathBuf,
    /// Graph name (from file or internal metadata)
    pub name: String,
    /// Number of nodes (if parseable)
    pub node_count: Option<usize>,
    /// Entry point node name
    pub entry_point: Option<String>,
}

/// Information about an installed package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    /// Package name
    pub name: String,
    /// Installed version
    pub version: String,
    /// Package type (graph-template, node-library, etc.)
    pub package_type: String,
}

impl ProjectInfo {
    /// Discover project info for a directory
    #[must_use]
    pub fn discover(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Check for dashflow.toml
        let config_path = root.join("dashflow.toml");
        let has_config = config_path.exists();

        // Discover graph files (*.graph.json, graphs/*.json)
        let graphs = Self::discover_graphs(&root);

        // Check installed packages in .dashflow/packages/
        let packages_dir = root.join(".dashflow/packages");
        let installed_packages = if packages_dir.exists() {
            Self::list_installed_packages(&packages_dir)
        } else {
            Vec::new()
        };

        Self {
            root,
            name,
            graphs,
            installed_packages,
            has_config,
        }
    }

    fn discover_graphs(root: &Path) -> Vec<GraphFileInfo> {
        let mut graphs = Vec::new();

        // Look for *.graph.json files in root and graphs/ directory
        let patterns = [
            root.to_path_buf(),
            root.join("graphs"),
            root.join("src/graphs"),
        ];

        for dir in &patterns {
            if !dir.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "json") {
                        let name = path
                            .file_stem()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unnamed".to_string());

                        // Try to parse for node count
                        let (node_count, entry_point) = Self::parse_graph_metadata(&path);

                        graphs.push(GraphFileInfo {
                            path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                            name,
                            node_count,
                            entry_point,
                        });
                    }
                }
            }
        }

        graphs
    }

    fn parse_graph_metadata(path: &Path) -> (Option<usize>, Option<String>) {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let node_count = json
                    .get("nodes")
                    .and_then(|n| n.as_object())
                    .map(|o| o.len());

                let entry_point = json
                    .get("entry_point")
                    .and_then(|e| e.as_str())
                    .map(String::from);

                return (node_count, entry_point);
            }
        }
        (None, None)
    }

    fn list_installed_packages(packages_dir: &Path) -> Vec<InstalledPackage> {
        let mut packages = Vec::new();

        if let Ok(entries) = std::fs::read_dir(packages_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Try to read manifest
                    let manifest_path = path.join("manifest.json");
                    let (version, package_type) = if manifest_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                let ver = json
                                    .get("version")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let pt = json
                                    .get("type")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                (ver, pt)
                            } else {
                                ("unknown".to_string(), "unknown".to_string())
                            }
                        } else {
                            ("unknown".to_string(), "unknown".to_string())
                        }
                    } else {
                        ("unknown".to_string(), "unknown".to_string())
                    };

                    packages.push(InstalledPackage {
                        name,
                        version,
                        package_type,
                    });
                }
            }
        }

        packages
    }
}

// ============================================================================
// Trace Store (Runtime Level)
// ============================================================================

/// Cache entry with modification time for invalidation
struct CachedTrace {
    trace: ExecutionTrace,
    mtime: SystemTime,
}

/// Default cache capacity for TraceStore
const TRACE_CACHE_CAPACITY: usize = 100;

/// Store for execution traces with caching.
///
/// Caches loaded traces to avoid repeated disk reads. Uses modification
/// time (mtime) tracking for automatic cache invalidation when files change.
#[derive(Debug)]
pub struct TraceStore {
    /// Directory where traces are stored
    traces_dir: PathBuf,
    /// LRU cache for loaded traces with mtime-based invalidation
    ///
    /// Uses Mutex instead of RefCell to allow TraceStore to be used in async contexts
    /// where it might be shared across threads.
    cache: Mutex<LruCache<PathBuf, CachedTrace>>,
}

impl Clone for TraceStore {
    fn clone(&self) -> Self {
        // Clone creates a new empty cache - caches are not shared
        Self {
            traces_dir: self.traces_dir.clone(),
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(TRACE_CACHE_CAPACITY).unwrap_or(NonZeroUsize::MIN),
            )),
        }
    }
}

impl TraceStore {
    /// Create a new trace store
    #[must_use]
    pub fn new(traces_dir: impl AsRef<Path>) -> Self {
        Self {
            traces_dir: traces_dir.as_ref().to_path_buf(),
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(TRACE_CACHE_CAPACITY).unwrap_or(NonZeroUsize::MIN),
            )),
        }
    }

    /// Get the most recent trace
    #[must_use]
    pub fn latest(&self) -> Option<ExecutionTrace> {
        self.list_traces()
            .into_iter()
            .max_by_key(|(_, modified)| *modified)
            .and_then(|(path, _)| self.load_trace(&path))
    }

    /// Default limit for list_traces to prevent unbounded memory growth
    pub const DEFAULT_TRACES_LIMIT: usize = 1000;

    /// List available traces with optional limit
    ///
    /// # Arguments
    /// * `limit` - Maximum number of traces to return. If None, uses DEFAULT_TRACES_LIMIT.
    ///
    /// Returns traces sorted by modification time (newest first).
    #[must_use]
    pub fn list_traces_limited(&self, limit: Option<usize>) -> Vec<(PathBuf, SystemTime)> {
        let limit = limit.unwrap_or(Self::DEFAULT_TRACES_LIMIT);
        let mut traces = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.traces_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            traces.push((path, modified));
                        }
                    }
                }
            }
        }

        traces.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
        traces.truncate(limit);
        traces
    }

    /// List available traces (limited to DEFAULT_TRACES_LIMIT)
    ///
    /// Use `list_traces_limited` for custom limits.
    #[must_use]
    pub fn list_traces(&self) -> Vec<(PathBuf, SystemTime)> {
        self.list_traces_limited(None)
    }

    /// Load a specific trace with caching.
    ///
    /// Returns cached trace if available and file hasn't been modified.
    /// Otherwise reads from disk and caches the result.
    #[must_use]
    pub fn load_trace(&self, path: &Path) -> Option<ExecutionTrace> {
        let path_buf = path.to_path_buf();

        // Check file modification time
        let current_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

        // Check cache - return if entry exists and mtime matches
        {
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(cached) = cache.get(&path_buf) {
                if current_mtime.is_some_and(|mtime| cached.mtime == mtime) {
                    return Some(cached.trace.clone());
                }
                // Cache entry stale or no mtime available - will re-read below
            }
        }

        // Cache miss or stale - read from disk
        let content = std::fs::read_to_string(path).ok()?;
        let trace: ExecutionTrace = serde_json::from_str(&content).ok()?;

        // Cache the result if we have mtime
        if let Some(mtime) = current_mtime {
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            cache.put(
                path_buf,
                CachedTrace {
                    trace: trace.clone(),
                    mtime,
                },
            );
        }

        Some(trace)
    }

    /// Get trace by ID
    #[must_use]
    pub fn get_by_id(&self, id: &str) -> Option<ExecutionTrace> {
        // Look for trace file matching the ID
        let pattern = format!("{}.json", id);
        let path = self.traces_dir.join(&pattern);

        if path.exists() {
            return self.load_trace(&path);
        }

        // Search for trace containing this ID
        for (trace_path, _) in self.list_traces() {
            if trace_path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().contains(id))
            {
                return self.load_trace(&trace_path);
            }
        }

        None
    }

    /// Get cache statistics.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Clear the trace cache.
    pub fn clear_cache(&self) {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

// ============================================================================
// Local Metrics Aggregation
// ============================================================================

/// Percentile statistics for a set of values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Percentiles {
    /// Minimum value
    pub min: f64,
    /// 50th percentile (median)
    pub p50: f64,
    /// 75th percentile
    pub p75: f64,
    /// 90th percentile
    pub p90: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
    /// Maximum value
    pub max: f64,
    /// Number of samples
    pub count: usize,
}

impl Percentiles {
    /// Calculate percentiles from a slice of values.
    #[must_use]
    pub fn from_values(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self::default();
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = sorted.len();
        Self {
            min: sorted[0],
            p50: Self::percentile(&sorted, 0.50),
            p75: Self::percentile(&sorted, 0.75),
            p90: Self::percentile(&sorted, 0.90),
            p95: Self::percentile(&sorted, 0.95),
            p99: Self::percentile(&sorted, 0.99),
            max: sorted[len - 1],
            count: len,
        }
    }

    fn percentile(sorted: &[f64], p: f64) -> f64 {
        let idx = (sorted.len() as f64 * p).ceil() as usize - 1;
        sorted[idx.min(sorted.len() - 1)]
    }
}

/// Aggregated metrics from execution traces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    /// Total number of traces analyzed
    pub total_traces: usize,
    /// Number of successful executions
    pub successful_traces: usize,
    /// Number of failed executions
    pub failed_traces: usize,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Total duration percentiles (in milliseconds)
    pub duration_percentiles: Percentiles,
    /// Per-node duration percentiles (node_name -> percentiles)
    pub node_percentiles: HashMap<String, Percentiles>,
    /// Total token usage
    pub total_tokens: u64,
    /// Average tokens per execution
    pub avg_tokens: f64,
    /// Total number of node executions
    pub total_node_executions: usize,
    /// Number of failed node executions (retries)
    pub failed_node_executions: usize,
    /// Retry rate (0.0 - 1.0)
    pub retry_rate: f64,
}

/// Local metrics aggregator for computing statistics from traces.
///
/// Enables AI self-reflection without requiring Prometheus.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::unified_introspection::LocalAggregator;
///
/// let aggregator = LocalAggregator::default();
/// let metrics = aggregator.aggregate_from_traces(&traces);
///
/// println!("Success rate: {:.1}%", metrics.success_rate * 100.0);
/// println!("P99 duration: {:.0}ms", metrics.duration_percentiles.p99);
/// ```
#[derive(Debug, Clone, Default)]
pub struct LocalAggregator;

impl LocalAggregator {
    /// Create a new local aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Aggregate metrics from a collection of execution traces.
    #[must_use]
    pub fn aggregate_from_traces(&self, traces: &[ExecutionTrace]) -> AggregatedMetrics {
        if traces.is_empty() {
            return AggregatedMetrics::default();
        }

        let total_traces = traces.len();
        let successful_traces = traces.iter().filter(|t| t.completed).count();
        let failed_traces = total_traces - successful_traces;
        let success_rate = successful_traces as f64 / total_traces as f64;

        // Collect total durations
        let durations: Vec<f64> = traces.iter().map(|t| t.total_duration_ms as f64).collect();
        let duration_percentiles = Percentiles::from_values(&durations);

        // Collect per-node durations
        let mut node_durations: HashMap<String, Vec<f64>> = HashMap::new();
        let mut total_tokens: u64 = 0;
        let mut total_node_executions = 0;
        let mut failed_node_executions = 0;

        for trace in traces {
            for node_exec in &trace.nodes_executed {
                node_durations
                    .entry(node_exec.node.clone())
                    .or_default()
                    .push(node_exec.duration_ms as f64);

                total_tokens += node_exec.tokens_used;
                total_node_executions += 1;

                if !node_exec.success {
                    failed_node_executions += 1;
                }
            }
        }

        let node_percentiles: HashMap<String, Percentiles> = node_durations
            .into_iter()
            .map(|(name, durations)| (name, Percentiles::from_values(&durations)))
            .collect();

        let avg_tokens = if total_traces > 0 {
            total_tokens as f64 / total_traces as f64
        } else {
            0.0
        };

        let retry_rate = if total_node_executions > 0 {
            failed_node_executions as f64 / total_node_executions as f64
        } else {
            0.0
        };

        AggregatedMetrics {
            total_traces,
            successful_traces,
            failed_traces,
            success_rate,
            duration_percentiles,
            node_percentiles,
            total_tokens,
            avg_tokens,
            total_node_executions,
            failed_node_executions,
            retry_rate,
        }
    }

    /// Calculate percentiles from raw duration values (in milliseconds).
    #[must_use]
    pub fn calculate_percentiles(&self, durations_ms: &[u64]) -> Percentiles {
        let as_f64: Vec<f64> = durations_ms.iter().map(|&d| d as f64).collect();
        Percentiles::from_values(&as_f64)
    }
}

// ============================================================================
// Unified Introspection API
// ============================================================================

/// Unified introspection for all four levels
///
/// This is the main entry point for DashFlow self-awareness. It provides:
/// - Platform level: Framework capabilities (modules, node types, features)
/// - Application level: Project-specific info (graphs, installed packages)
/// - Runtime level: Execution traces and live state
/// - Network level: Package ecosystem queries
pub struct DashFlowIntrospection {
    // Platform level
    platform: PlatformIntrospection,

    // Application level
    project: ProjectInfo,

    // Runtime level
    traces: TraceStore,
    runtime_interface: IntrospectionInterface,
    live_tracker: Option<ExecutionTracker>,

    // Metrics integration
    prometheus: Option<PrometheusClient>,

    // Local metrics aggregation
    aggregator: LocalAggregator,

    // Optimizer selection
    auto_optimizer: AutoOptimizer,
    // Network level - currently not connected to registry
    // Future: Add RegistryClient when dashflow-registry is a dependency
}

impl DashFlowIntrospection {
    /// Create introspection for the current working directory
    #[must_use]
    pub fn for_cwd() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::for_project(&cwd)
    }

    /// Create introspection for a specific project directory
    #[must_use]
    pub fn for_project(project_root: &Path) -> Self {
        let platform = PlatformIntrospection::discover();
        let project = ProjectInfo::discover(project_root);
        let traces_dir = project_root.join(".dashflow/traces");
        let traces = TraceStore::new(traces_dir);
        let runtime_interface = IntrospectionInterface::new();
        let optimizer_dir = project_root.join(".dashflow/optimization_history");
        let auto_optimizer = AutoOptimizer::with_storage_dir(optimizer_dir);

        Self {
            platform,
            project,
            traces,
            runtime_interface,
            live_tracker: None,
            prometheus: None,
            aggregator: LocalAggregator::new(),
            auto_optimizer,
        }
    }

    /// Attach a live execution tracker
    #[must_use]
    pub fn with_tracker(mut self, tracker: ExecutionTracker) -> Self {
        self.live_tracker = Some(tracker);
        self
    }

    /// Attach a Prometheus client for metrics queries
    ///
    /// This enables data parity by allowing introspection to query the same
    /// metrics that humans see in Grafana dashboards.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::unified_introspection::DashFlowIntrospection;
    /// use dashflow::prometheus_client::PrometheusClient;
    ///
    /// let introspection = DashFlowIntrospection::for_cwd()
    ///     .with_prometheus(PrometheusClient::new("http://localhost:9090"));
    ///
    /// // Now metrics queries work
    /// let snapshot = introspection.query_metrics(None).await;
    /// println!("Quality: {:?}", snapshot.quality_score);
    /// ```
    #[must_use]
    pub fn with_prometheus(mut self, client: PrometheusClient) -> Self {
        self.prometheus = Some(client);
        self
    }

    /// Query metrics from Prometheus
    ///
    /// Returns a snapshot of current metrics. If a trace_id is provided,
    /// attempts to correlate metrics with that specific execution (future enhancement).
    ///
    /// This method enables AI introspection to see the same data as humans
    /// viewing Grafana dashboards (data parity per Principle 5).
    ///
    /// # Arguments
    ///
    /// * `trace_id` - Optional trace ID for correlation (currently unused, future enhancement)
    ///
    /// # Returns
    ///
    /// A `MetricsSnapshot` containing current metric values from Prometheus.
    /// If Prometheus is unavailable, returns a snapshot with `prometheus_healthy = false`.
    pub async fn query_metrics(&self, trace_id: Option<&str>) -> MetricsSnapshot {
        let client = match &self.prometheus {
            Some(c) => c,
            None => return MetricsSnapshot::unavailable(),
        };

        // Check if Prometheus is healthy
        if !client.is_healthy().await {
            return MetricsSnapshot::unavailable();
        }

        let mut snapshot = MetricsSnapshot {
            trace_id: trace_id.map(String::from),
            quality_score: None,
            success_rate: None,
            error_rate_5m: None,
            node_duration_p99_ms: None,
            node_duration_p95_ms: None,
            retries_total: None,
            prometheus_healthy: true,
            timestamp: chrono::Utc::now(),
            custom_metrics: HashMap::new(),
        };

        // Query DashStream quality metrics
        if let Ok(values) = client
            .query(crate::prometheus_client::queries::QUALITY_SCORE)
            .await
        {
            if let Some(first) = values.first() {
                snapshot.quality_score = Some(first.value);
            }
        }

        if let Ok(values) = client
            .query(crate::prometheus_client::queries::SUCCESS_RATE)
            .await
        {
            if let Some(first) = values.first() {
                snapshot.success_rate = Some(first.value);
            }
        }

        // Query error rate
        if let Ok(values) = client
            .query(crate::prometheus_client::queries::ERROR_RATE_5M)
            .await
        {
            if let Some(first) = values.first() {
                snapshot.error_rate_5m = Some(first.value);
            }
        }

        // Query node duration P99 (convert from seconds to milliseconds)
        if let Ok(values) = client
            .query(crate::prometheus_client::queries::NODE_DURATION_P99)
            .await
        {
            if let Some(first) = values.first() {
                snapshot.node_duration_p99_ms = Some(first.value * 1000.0);
            }
        }

        // Query node duration P95 (convert from seconds to milliseconds)
        if let Ok(values) = client
            .query(crate::prometheus_client::queries::NODE_DURATION_P95)
            .await
        {
            if let Some(first) = values.first() {
                snapshot.node_duration_p95_ms = Some(first.value * 1000.0);
            }
        }

        // Query retries total
        if let Ok(values) = client
            .query(crate::prometheus_client::queries::RETRIES_TOTAL)
            .await
        {
            if let Some(first) = values.first() {
                snapshot.retries_total = Some(first.value);
            }
        }

        snapshot
    }

    /// Query a custom metric from Prometheus
    ///
    /// # Arguments
    ///
    /// * `promql` - PromQL query string
    ///
    /// # Returns
    ///
    /// The first metric value if found, or None if unavailable.
    pub async fn query_custom_metric(&self, promql: &str) -> Option<f64> {
        let client = self.prometheus.as_ref()?;

        if let Ok(values) = client.query(promql).await {
            if let Some(first) = values.first() {
                return Some(first.value);
            }
        }

        None
    }

    /// Check if Prometheus integration is configured
    #[must_use]
    pub fn has_prometheus(&self) -> bool {
        self.prometheus.is_some()
    }

    // ========================================================================
    // Local Metrics
    // ========================================================================

    /// Get a summary of performance metrics from recent traces.
    ///
    /// Aggregates metrics locally from the trace store without requiring
    /// Prometheus. Useful for AI self-reflection in any environment.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let introspection = DashFlowIntrospection::for_cwd();
    /// let metrics = introspection.metrics_summary(100);
    ///
    /// println!("Success rate: {:.1}%", metrics.success_rate * 100.0);
    /// println!("P99 latency: {:.0}ms", metrics.duration_percentiles.p99);
    /// ```
    #[must_use]
    pub fn metrics_summary(&self, limit: usize) -> AggregatedMetrics {
        let trace_list = self.traces.list_traces();
        let traces: Vec<ExecutionTrace> = trace_list
            .into_iter()
            .take(limit)
            .filter_map(|(path, _)| self.traces.load_trace(&path))
            .collect();

        self.aggregator.aggregate_from_traces(&traces)
    }

    /// Query a specific metric by name.
    ///
    /// Supported metrics:
    /// - "success_rate" - Success rate (0.0-1.0)
    /// - "error_rate" - Error rate (0.0-1.0)
    /// - "retry_rate" - Retry rate (0.0-1.0)
    /// - "total_traces" - Number of traces analyzed
    /// - "avg_tokens" - Average tokens per execution
    /// - "p50_duration" - Median execution duration (ms)
    /// - "p95_duration" - 95th percentile duration (ms)
    /// - "p99_duration" - 99th percentile duration (ms)
    ///
    /// # Arguments
    ///
    /// * `metric` - Metric name (case-insensitive)
    /// * `limit` - Maximum number of traces to analyze
    #[must_use]
    pub fn query_metric(&self, metric: &str, limit: usize) -> Option<f64> {
        let summary = self.metrics_summary(limit);

        match metric.to_lowercase().as_str() {
            "success_rate" => Some(summary.success_rate),
            "error_rate" => Some(1.0 - summary.success_rate),
            "retry_rate" => Some(summary.retry_rate),
            "total_traces" => Some(summary.total_traces as f64),
            "avg_tokens" => Some(summary.avg_tokens),
            "p50_duration" | "median_duration" => Some(summary.duration_percentiles.p50),
            "p75_duration" => Some(summary.duration_percentiles.p75),
            "p90_duration" => Some(summary.duration_percentiles.p90),
            "p95_duration" => Some(summary.duration_percentiles.p95),
            "p99_duration" => Some(summary.duration_percentiles.p99),
            "min_duration" => Some(summary.duration_percentiles.min),
            "max_duration" => Some(summary.duration_percentiles.max),
            _ => None,
        }
    }

    /// Get per-node duration percentiles.
    ///
    /// Returns duration statistics for each node in the graph.
    #[must_use]
    pub fn node_metrics(&self, limit: usize) -> HashMap<String, Percentiles> {
        self.metrics_summary(limit).node_percentiles
    }

    /// Classify which level should handle a question
    #[must_use]
    pub fn classify_question(&self, question: &str) -> IntrospectionLevel {
        let q = question.to_lowercase();

        // Network level indicators
        if q.contains("package")
            || q.contains("registry")
            || q.contains("published")
            || q.contains("ecosystem")
            || q.contains("install")
            || q.contains("download")
        {
            // Check if it's about installed packages (Application) vs available (Network)
            if q.contains("installed") || q.contains("my package") {
                return IntrospectionLevel::Application;
            }
            if q.contains("available")
                || q.contains("exist")
                || q.contains("find")
                || q.contains("search")
            {
                return IntrospectionLevel::Network;
            }
        }

        // Runtime level indicators - execution-related
        if q.contains("why did")
            || q.contains("what if")
            || q.contains("how can i")
            || q.contains("am i")
            || q.contains("currently running")
            || q.contains("execution")
            || q.contains("trace")
            || q.contains("ran")
            || q.contains("executed")
            || q.contains("took")
            || q.contains("time")
            || q.contains("performance")
        {
            return IntrospectionLevel::Runtime;
        }

        // Application level indicators - project-specific
        if q.contains("my graph")
            || q.contains("my project")
            || q.contains("what graphs")
            || q.contains("graphs do i have")
            || q.contains("project")
            || q.contains("configured")
        {
            return IntrospectionLevel::Application;
        }

        // Platform level indicators - framework capabilities
        if q.contains("implemented")
            || q.contains("capability")
            || q.contains("module")
            || q.contains("feature")
            || q.contains("node type")
            || q.contains("edge type")
            || q.contains("template")
            || q.contains("support")
            || q.contains("dashflow")
        {
            return IntrospectionLevel::Platform;
        }

        // Default to Platform for capability questions
        IntrospectionLevel::Platform
    }

    /// Ask a question - automatically routes to the correct level
    #[must_use]
    pub fn ask(&self, question: &str) -> IntrospectionResponse {
        let level = self.classify_question(question);

        match level {
            IntrospectionLevel::Platform => self.ask_platform(question),
            IntrospectionLevel::Application => self.ask_application(question),
            IntrospectionLevel::Runtime => self.ask_runtime(question),
            IntrospectionLevel::Network => self.ask_network(question),
        }
    }

    /// Ask with explicit level override
    #[must_use]
    pub fn ask_at_level(&self, level: IntrospectionLevel, question: &str) -> IntrospectionResponse {
        match level {
            IntrospectionLevel::Platform => self.ask_platform(question),
            IntrospectionLevel::Application => self.ask_application(question),
            IntrospectionLevel::Runtime => self.ask_runtime(question),
            IntrospectionLevel::Network => self.ask_network(question),
        }
    }

    /// Search across all four levels
    #[must_use]
    pub fn search(&self, query: &str) -> SearchResults {
        let mut results = Vec::new();

        // Platform search
        results.extend(self.search_platform(query));

        // Application search
        results.extend(self.search_application(query));

        // Runtime search (limited - just trace names)
        results.extend(self.search_runtime(query));

        // Network search - currently returns empty (no registry connection)
        results.extend(self.search_network(query));

        // Sort by relevance
        results.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = results.len();
        SearchResults {
            query: query.to_string(),
            results,
            total,
        }
    }

    // ========================================================================
    // Platform Level
    // ========================================================================

    fn ask_platform(&self, question: &str) -> IntrospectionResponse {
        let q = question.to_lowercase();

        // Try to find a capability match
        if let Some(cap) = self.platform.query_capability(&q) {
            return IntrospectionResponse::new(
                IntrospectionLevel::Platform,
                format!(
                    "Yes, {} is implemented. Category: {}. {}",
                    cap.name, cap.category, cap.description
                ),
            )
            .with_confidence(0.9)
            .with_follow_ups(vec![
                format!("What other {} features exist?", cap.category),
                "What node types are available?".to_string(),
            ]);
        }

        // Check for specific keywords
        if q.contains("distillation") || q.contains("distill") {
            return IntrospectionResponse::new(
                IntrospectionLevel::Platform,
                "Yes, distillation is implemented in the optimize/distillation module. \
                 It provides knowledge distillation for model compression.",
            )
            .with_confidence(0.85)
            .with_details(vec![
                "Module: dashflow::optimize::distillation".to_string(),
                "Status: Stable".to_string(),
            ]);
        }

        // General platform info
        let version_info = self.platform.version_info();
        let feature_count = self.platform.available_features().len();
        let node_type_count = self.platform.supported_node_types().len();

        IntrospectionResponse::new(
            IntrospectionLevel::Platform,
            format!(
                "DashFlow {} provides {} features and {} node types. \
                 Try asking about specific capabilities like 'checkpointing' or 'streaming'.",
                version_info.version, feature_count, node_type_count
            ),
        )
        .with_confidence(0.7)
        .with_follow_ups(vec![
            "What features are available?".to_string(),
            "What node types exist?".to_string(),
            "Is checkpointing implemented?".to_string(),
        ])
    }

    fn search_platform(&self, query: &str) -> Vec<LevelSearchResult> {
        let q = query.to_lowercase();
        let features = self.platform.available_features();
        let node_types = self.platform.supported_node_types();
        // Capacity hint: worst case is all features + all node_types match
        let mut results = Vec::with_capacity(features.len() + node_types.len());

        // Search features
        for feature in features {
            let name_match = feature.name.to_lowercase().contains(&q);
            let desc_match = feature.description.to_lowercase().contains(&q);

            if name_match || desc_match {
                results.push(LevelSearchResult {
                    level: IntrospectionLevel::Platform,
                    name: feature.name.clone(),
                    category: "feature".to_string(),
                    description: feature.description.clone(),
                    relevance: if name_match { 1.0 } else { 0.7 },
                });
            }
        }

        // Search node types
        for node_type in node_types {
            let name_match = node_type.name.to_lowercase().contains(&q);
            let desc_match = node_type.description.to_lowercase().contains(&q);

            if name_match || desc_match {
                results.push(LevelSearchResult {
                    level: IntrospectionLevel::Platform,
                    name: node_type.name.clone(),
                    category: "node_type".to_string(),
                    description: node_type.description.clone(),
                    relevance: if name_match { 1.0 } else { 0.7 },
                });
            }
        }

        results
    }

    // ========================================================================
    // Application Level
    // ========================================================================

    fn ask_application(&self, question: &str) -> IntrospectionResponse {
        let q = question.to_lowercase();

        // Questions about graphs
        if q.contains("graph") {
            let graph_count = self.project.graphs.len();
            if graph_count == 0 {
                return IntrospectionResponse::new(
                    IntrospectionLevel::Application,
                    format!(
                        "No graph files found in project '{}'. \
                         Graph files should be *.json files in the root, graphs/, or src/graphs/ directories.",
                        self.project.name
                    ),
                )
                .with_confidence(0.9);
            }

            let graph_list: Vec<String> = self
                .project
                .graphs
                .iter()
                .map(|g| {
                    let nodes = g
                        .node_count
                        .map(|n| format!(" ({} nodes)", n))
                        .unwrap_or_default();
                    format!("- {}{}", g.name, nodes)
                })
                .collect();

            return IntrospectionResponse::new(
                IntrospectionLevel::Application,
                format!(
                    "Project '{}' has {} graph(s):\n{}",
                    self.project.name,
                    graph_count,
                    graph_list.join("\n")
                ),
            )
            .with_confidence(0.95)
            .with_data(
                "graphs",
                serde_json::to_value(&self.project.graphs).unwrap_or_default(),
            );
        }

        // Questions about installed packages
        if q.contains("package") || q.contains("installed") {
            let pkg_count = self.project.installed_packages.len();
            if pkg_count == 0 {
                return IntrospectionResponse::new(
                    IntrospectionLevel::Application,
                    format!(
                        "No packages installed in project '{}'. \
                         Use 'dashflow pkg install <name>' to install packages.",
                        self.project.name
                    ),
                )
                .with_confidence(0.9);
            }

            let pkg_list: Vec<String> = self
                .project
                .installed_packages
                .iter()
                .map(|p| format!("- {} v{} ({})", p.name, p.version, p.package_type))
                .collect();

            return IntrospectionResponse::new(
                IntrospectionLevel::Application,
                format!(
                    "Project '{}' has {} installed package(s):\n{}",
                    self.project.name,
                    pkg_count,
                    pkg_list.join("\n")
                ),
            )
            .with_confidence(0.95);
        }

        // General project info
        IntrospectionResponse::new(
            IntrospectionLevel::Application,
            format!(
                "Project '{}' at {}:\n\
                 - Config: {}\n\
                 - Graphs: {}\n\
                 - Packages: {}",
                self.project.name,
                self.project.root.display(),
                if self.project.has_config {
                    "dashflow.toml found"
                } else {
                    "no dashflow.toml"
                },
                self.project.graphs.len(),
                self.project.installed_packages.len()
            ),
        )
        .with_confidence(0.9)
    }

    fn search_application(&self, query: &str) -> Vec<LevelSearchResult> {
        let q = query.to_lowercase();
        // Capacity hint: worst case is all graphs + all packages match
        let mut results =
            Vec::with_capacity(self.project.graphs.len() + self.project.installed_packages.len());

        // Search graphs
        for graph in &self.project.graphs {
            if graph.name.to_lowercase().contains(&q) {
                results.push(LevelSearchResult {
                    level: IntrospectionLevel::Application,
                    name: graph.name.clone(),
                    category: "graph".to_string(),
                    description: format!("Graph at {}", graph.path.display()),
                    relevance: 1.0,
                });
            }
        }

        // Search installed packages
        for pkg in &self.project.installed_packages {
            if pkg.name.to_lowercase().contains(&q) {
                results.push(LevelSearchResult {
                    level: IntrospectionLevel::Application,
                    name: pkg.name.clone(),
                    category: "installed_package".to_string(),
                    description: format!("v{} ({})", pkg.version, pkg.package_type),
                    relevance: 1.0,
                });
            }
        }

        results
    }

    // ========================================================================
    // Runtime Level
    // ========================================================================

    fn ask_runtime(&self, question: &str) -> IntrospectionResponse {
        // Try to load the latest trace
        let trace = match self.traces.latest() {
            Some(t) => t,
            None => {
                return IntrospectionResponse::new(
                    IntrospectionLevel::Runtime,
                    "No execution traces found. Run a graph first to generate traces, \
                     or ensure traces are saved to .dashflow/traces/",
                )
                .with_confidence(0.9)
                .with_follow_ups(vec![
                    "How do I enable trace saving?".to_string(),
                    "What graphs do I have?".to_string(),
                ]);
            }
        };

        // Use IntrospectionInterface for the actual query
        let response = self.runtime_interface.ask(&trace, question);

        IntrospectionResponse::new(IntrospectionLevel::Runtime, response.answer.clone())
            .with_confidence(response.confidence)
            .with_details(response.details.clone())
            .with_follow_ups(response.follow_ups.clone())
    }

    fn search_runtime(&self, query: &str) -> Vec<LevelSearchResult> {
        let q = query.to_lowercase();
        let traces = self.traces.list_traces();
        // Capacity hint: worst case is all traces match (live executions typically few)
        let mut results = Vec::with_capacity(traces.len());

        // Search trace files by name
        for (path, _) in traces {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if name.to_lowercase().contains(&q) {
                results.push(LevelSearchResult {
                    level: IntrospectionLevel::Runtime,
                    name: name.clone(),
                    category: "trace".to_string(),
                    description: format!("Execution trace: {}", path.display()),
                    relevance: 0.8,
                });
            }
        }

        // Also search live executions if tracker attached
        if let Some(tracker) = &self.live_tracker {
            for summary in tracker.active_executions() {
                if summary.graph_name.to_lowercase().contains(&q)
                    || summary.current_node.to_lowercase().contains(&q)
                {
                    results.push(LevelSearchResult {
                        level: IntrospectionLevel::Runtime,
                        name: summary.execution_id.to_string(),
                        category: "live_execution".to_string(),
                        description: format!(
                            "Running: {} at node '{}'",
                            summary.graph_name, summary.current_node
                        ),
                        relevance: 1.0,
                    });
                }
            }
        }

        results
    }

    // ========================================================================
    // Network Level
    // ========================================================================

    fn ask_network(&self, _question: &str) -> IntrospectionResponse {
        // Network level queries require registry connection
        // Currently not implemented - return helpful guidance
        IntrospectionResponse::new(
            IntrospectionLevel::Network,
            "Package registry queries are not yet connected. \
             To search for packages, use 'dashflow pkg search <query>' which connects to the registry directly. \
             Future versions will integrate registry queries into the unified introspection API.",
        )
        .with_confidence(0.5)
        .with_follow_ups(vec![
            "What packages are installed locally?".to_string(),
            "How do I install a package?".to_string(),
        ])
    }

    fn search_network(&self, _query: &str) -> Vec<LevelSearchResult> {
        // Network-level introspection requires a package registry connection.
        //
        // Requirements for network introspection:
        // 1. Running registry server (see crates/dashflow-registry for implementation)
        // 2. Registry client configured with server URL
        // 3. Network connectivity to registry server or local colony peers
        //
        // The registry provides:
        // - Content-addressed package storage (SHA-256 hashes)
        // - Semantic search via vector embeddings
        // - Ed25519 signature verification
        // - Colony-first P2P distribution
        //
        // To use: `dashflow pkg search <query>` for direct registry access.
        // Future: Integrate RegistryClient into UnifiedIntrospection for seamless queries.
        Vec::new()
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get platform introspection
    #[must_use]
    pub fn platform(&self) -> &PlatformIntrospection {
        &self.platform
    }

    /// Get project info
    #[must_use]
    pub fn project(&self) -> &ProjectInfo {
        &self.project
    }

    /// Get trace store
    #[must_use]
    pub fn traces(&self) -> &TraceStore {
        &self.traces
    }

    /// Get live execution summaries (if tracker attached)
    #[must_use]
    pub fn live_executions(&self) -> Vec<ExecutionSummary> {
        self.live_tracker
            .as_ref()
            .map(|t| t.active_executions())
            .unwrap_or_default()
    }

    // ========================================================================
    // Optimizer Selection
    // ========================================================================

    /// Select the best optimizer for a given context
    ///
    /// This integrates the AutoOptimizer with introspection, enabling AI agents
    /// to query DashFlow's knowledge about which optimizer to use.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::unified_introspection::DashFlowIntrospection;
    /// use dashflow::optimize::auto_optimizer::{OptimizationContext, TaskType};
    ///
    /// let introspection = DashFlowIntrospection::for_cwd();
    /// let context = OptimizationContext::builder()
    ///     .num_examples(100)
    ///     .task_type(TaskType::QuestionAnswering)
    ///     .build();
    ///
    /// let selection = introspection.select_optimizer(&context);
    /// println!("Use: {} ({})", selection.optimizer_name, selection.reason);
    /// ```
    #[must_use]
    pub fn select_optimizer(&self, context: &OptimizationContext) -> SelectionResult {
        AutoOptimizer::select(context)
    }

    /// Get a detailed explanation of optimizer selection
    ///
    /// Returns a human-readable string explaining why a particular optimizer
    /// was selected for the given context.
    #[must_use]
    pub fn explain_selection(&self, context: &OptimizationContext) -> String {
        let selection = AutoOptimizer::select(context);

        let mut explanation = format!(
            "Optimizer Selection for {} examples, task type: {}\n\n",
            context.num_examples, context.task_type
        );

        explanation.push_str(&format!(
            "Selected: {} (confidence: {:.0}%)\n",
            selection.optimizer_name,
            selection.confidence * 100.0
        ));
        explanation.push_str(&format!("Reason: {}\n", selection.reason));

        if let Some(tier) = selection.tier {
            explanation.push_str(&format!(
                "Tier: {} (1=recommended, 2=specialized, 3=niche)\n",
                tier
            ));
        }

        if let Some(citation) = &selection.citation {
            explanation.push_str(&format!("Citation: {}\n", citation));
        }

        if !selection.alternatives.is_empty() {
            explanation.push_str("\nAlternatives:\n");
            for alt in &selection.alternatives {
                explanation.push_str(&format!(
                    "  - {}: {} (confidence: {:.0}%)\n",
                    alt.name,
                    alt.reason,
                    alt.confidence * 100.0
                ));
            }
        }

        explanation
    }

    /// Get historical performance statistics for optimizers
    ///
    /// Returns statistics on how optimizers have performed in past runs,
    /// optionally filtered by task type.
    ///
    /// # Errors
    ///
    /// Returns error if reading historical data fails.
    pub async fn historical_performance(
        &self,
        task_type: Option<TaskType>,
    ) -> crate::Result<Vec<OptimizerStats>> {
        let stats = self.auto_optimizer.historical_stats().await?;

        // Filter by task type if specified
        if let Some(target_type) = task_type {
            Ok(stats
                .into_iter()
                .filter(|s| s.best_task_types.contains(&target_type))
                .collect())
        } else {
            Ok(stats)
        }
    }

    /// Record an optimization outcome for future learning
    ///
    /// This enables the introspection system to learn from past optimization
    /// runs and improve future recommendations.
    ///
    /// # Errors
    ///
    /// Returns error if writing the outcome fails.
    pub async fn record_outcome(&self, outcome: &OptimizationOutcome) -> crate::Result<()> {
        self.auto_optimizer.record_outcome(outcome).await
    }

    /// Get optimizer-specific statistics
    ///
    /// Returns historical statistics for a specific optimizer, or None if
    /// no data is available.
    ///
    /// # Errors
    ///
    /// Returns error if reading historical data fails.
    pub async fn optimizer_stats(
        &self,
        optimizer_name: &str,
    ) -> crate::Result<Option<OptimizerStats>> {
        self.auto_optimizer
            .stats_for_optimizer(optimizer_name)
            .await
    }

    /// Get the auto optimizer instance for direct access
    #[must_use]
    pub fn auto_optimizer(&self) -> &AutoOptimizer {
        &self.auto_optimizer
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_introspection_level_display() {
        assert_eq!(IntrospectionLevel::Platform.to_string(), "platform");
        assert_eq!(IntrospectionLevel::Application.to_string(), "application");
        assert_eq!(IntrospectionLevel::Runtime.to_string(), "runtime");
        assert_eq!(IntrospectionLevel::Network.to_string(), "network");
    }

    #[test]
    fn test_question_classification() {
        let introspection = DashFlowIntrospection::for_cwd();

        // Platform questions
        assert_eq!(
            introspection.classify_question("Is distillation implemented?"),
            IntrospectionLevel::Platform
        );
        assert_eq!(
            introspection.classify_question("What node types exist?"),
            IntrospectionLevel::Platform
        );

        // Application questions
        assert_eq!(
            introspection.classify_question("What graphs do I have?"),
            IntrospectionLevel::Application
        );
        assert_eq!(
            introspection.classify_question("What packages are installed?"),
            IntrospectionLevel::Application
        );

        // Runtime questions
        assert_eq!(
            introspection.classify_question("Why did search run 3 times?"),
            IntrospectionLevel::Runtime
        );
        assert_eq!(
            introspection.classify_question("What is currently running?"),
            IntrospectionLevel::Runtime
        );

        // Network questions
        assert_eq!(
            introspection.classify_question("What RAG packages exist?"),
            IntrospectionLevel::Network
        );
        assert_eq!(
            introspection.classify_question("Find packages for sentiment analysis"),
            IntrospectionLevel::Network
        );
    }

    #[test]
    fn test_introspection_response() {
        let response = IntrospectionResponse::new(IntrospectionLevel::Platform, "Test answer")
            .with_confidence(0.9)
            .with_details(vec!["Detail 1".to_string()])
            .with_follow_ups(vec!["Follow up?".to_string()]);

        assert_eq!(response.level, IntrospectionLevel::Platform);
        assert_eq!(response.answer, "Test answer");
        assert_eq!(response.confidence, 0.9);
        assert_eq!(response.details.len(), 1);
        assert_eq!(response.follow_ups.len(), 1);
    }

    #[test]
    fn test_project_info_discover() {
        // Test with current directory (should not panic)
        let project = ProjectInfo::discover(".");
        assert!(!project.name.is_empty());
    }

    #[test]
    fn test_search_results() {
        let introspection = DashFlowIntrospection::for_cwd();
        let results = introspection.search("checkpoint");

        // Should find at least the checkpointing feature
        assert!(results
            .results
            .iter()
            .any(|r| r.level == IntrospectionLevel::Platform));
    }

    // ========================================================================
    // Prometheus Integration Tests
    // ========================================================================

    #[test]
    fn test_metrics_snapshot_unavailable() {
        let snapshot = MetricsSnapshot::unavailable();
        assert!(!snapshot.prometheus_healthy);
        assert!(!snapshot.has_data());
        assert_eq!(snapshot.summary(), "Prometheus unavailable");
    }

    #[test]
    fn test_metrics_snapshot_summary() {
        let snapshot = MetricsSnapshot {
            trace_id: Some("test-123".to_string()),
            quality_score: Some(0.87),
            success_rate: Some(0.95),
            error_rate_5m: Some(0.02),
            node_duration_p99_ms: Some(150.0),
            node_duration_p95_ms: Some(100.0),
            retries_total: Some(5.0),
            prometheus_healthy: true,
            timestamp: chrono::Utc::now(),
            custom_metrics: HashMap::new(),
        };

        assert!(snapshot.prometheus_healthy);
        assert!(snapshot.has_data());

        let summary = snapshot.summary();
        assert!(summary.contains("Quality: 0.87"));
        assert!(summary.contains("Success: 95.0%"));
        assert!(summary.contains("Errors(5m): 2.00%"));
        assert!(summary.contains("P99: 150ms"));
        assert!(summary.contains("Retries(1h): 5"));
    }

    #[test]
    fn test_metrics_snapshot_partial_data() {
        let snapshot = MetricsSnapshot {
            trace_id: None,
            quality_score: Some(0.92),
            success_rate: None,
            error_rate_5m: None,
            node_duration_p99_ms: None,
            node_duration_p95_ms: None,
            retries_total: None,
            prometheus_healthy: true,
            timestamp: chrono::Utc::now(),
            custom_metrics: HashMap::new(),
        };

        assert!(snapshot.has_data());
        let summary = snapshot.summary();
        assert!(summary.contains("Quality: 0.92"));
        assert!(!summary.contains("Success"));
    }

    #[test]
    fn test_has_prometheus() {
        let introspection = DashFlowIntrospection::for_cwd();
        assert!(!introspection.has_prometheus());

        let introspection_with_prometheus = DashFlowIntrospection::for_cwd().with_prometheus(
            crate::prometheus_client::PrometheusClient::new("http://localhost:9090"),
        );
        assert!(introspection_with_prometheus.has_prometheus());
    }

    #[tokio::test]
    async fn test_query_metrics_without_prometheus() {
        let introspection = DashFlowIntrospection::for_cwd();

        // Without Prometheus configured, should return unavailable
        let snapshot = introspection.query_metrics(None).await;
        assert!(!snapshot.prometheus_healthy);
        assert!(!snapshot.has_data());
    }

    #[tokio::test]
    async fn test_query_custom_metric_without_prometheus() {
        let introspection = DashFlowIntrospection::for_cwd();

        // Without Prometheus configured, should return None
        let result = introspection.query_custom_metric("up").await;
        assert!(result.is_none());
    }
}
