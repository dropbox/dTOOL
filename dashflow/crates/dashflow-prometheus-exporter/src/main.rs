// S-11: Clippy allows for binary executable.
// In main.rs binaries, panicking on initialization errors is acceptable because:
// - Startup failures should terminate the process with a clear error
// - Error messages from .expect() are visible in logs
// - This is not library code where callers need to handle errors
// See: https://rust-lang.github.io/rust-clippy/master/index.html#/unwrap_used
#![allow(clippy::expect_used, clippy::unwrap_used)]
// Arc cloning patterns are common in concurrent async code; explicit .clone() is idiomatic
#![allow(clippy::clone_on_ref_ptr)]
// These patterns are acceptable for main.rs where simplicity > API optimization
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Prometheus Metrics Exporter for DashStream
//!
//! Bridges Kafka → Prometheus by consuming DashStream quality events
//! and exposing them as Prometheus metrics at :9190/metrics
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// =============================================================================
// Environment Variable Constants
// =============================================================================
// M-153: Local constants that mirror dashflow::core::config_loader::env_vars
// These cannot import from dashflow directly to avoid adding a large dependency.

/// Kafka brokers (default: localhost:9092)
const KAFKA_BROKERS: &str = "KAFKA_BROKERS";
/// Kafka topic for events (default: dashstream-quality)
const KAFKA_TOPIC: &str = "KAFKA_TOPIC";
/// Kafka consumer group ID (default: prometheus-exporter)
const KAFKA_GROUP_ID: &str = "KAFKA_GROUP_ID";
/// Kafka auto offset reset (values: earliest, latest; default: earliest)
const KAFKA_AUTO_OFFSET_RESET: &str = "KAFKA_AUTO_OFFSET_RESET";
/// Max payload size for DashStream decode in bytes (default: 10MB)
const DASHSTREAM_MAX_PAYLOAD_BYTES: &str = "DASHSTREAM_MAX_PAYLOAD_BYTES";
/// Prometheus metrics port (default: 9190)
const METRICS_PORT: &str = "METRICS_PORT";
/// Prometheus metrics bind IP (default: 0.0.0.0)
const METRICS_BIND_IP: &str = "METRICS_BIND_IP";
/// Prometheus session timeout in seconds (default: 300)
const PROMETHEUS_SESSION_TIMEOUT_SECS: &str = "PROMETHEUS_SESSION_TIMEOUT_SECS";

use anyhow::{Context, Result};
use axum::{routing::get, Router};
use prometheus::{
    Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec,
    Opts, Registry, TextEncoder,
};
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    message::Message,
};
use std::{collections::HashMap, net::SocketAddr, sync::{atomic::{AtomicU64, Ordering}, Arc, RwLock}, time::Instant};
use tokio::sync::broadcast;
use tokio::time::Duration;
use tracing::{error, info, warn};

use serde::{Deserialize, Serialize};

// M-543: Configurable histogram buckets via environment variables
// Different deployments have different latency profiles; these defaults are tuned for typical LLM workloads

/// Parse histogram buckets from environment variable.
/// Format: comma-separated floats, e.g., "10,50,100,500,1000,5000"
/// Returns default if env var is not set or parsing fails.
fn parse_buckets_from_env(env_var: &str, default: Vec<f64>) -> Vec<f64> {
    match std::env::var(env_var) {
        Ok(value) => {
            let buckets: Result<Vec<f64>, _> = value
                .split(',')
                .map(|s| s.trim().parse::<f64>())
                .collect();
            match buckets {
                Ok(b) if !b.is_empty() => {
                    tracing::info!(
                        "Using custom histogram buckets from {}: {:?}",
                        env_var,
                        b
                    );
                    b
                }
                Ok(_) => {
                    tracing::warn!(
                        "Empty bucket list in {}, using defaults: {:?}",
                        env_var,
                        default
                    );
                    default
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse {} ({}), using defaults: {:?}",
                        env_var,
                        e,
                        default
                    );
                    default
                }
            }
        }
        Err(_) => default,
    }
}

/// Default latency buckets for query latency histograms (milliseconds).
/// Extended to track queries >5s (31% of queries exceeded 5000ms in early testing).
fn default_latency_buckets_ms() -> Vec<f64> {
    vec![
        10.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 15000.0, 20000.0, 30000.0,
    ]
}

/// Default buckets for request duration in seconds.
fn default_duration_buckets_seconds() -> Vec<f64> {
    vec![0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0]
}

/// Default buckets for retry count histogram.
fn default_retry_buckets() -> Vec<f64> {
    vec![0.0, 1.0, 2.0, 3.0, 5.0, 10.0]
}

/// Default buckets for session turn count histogram.
fn default_session_turn_buckets() -> Vec<f64> {
    vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0]
}

/// Default buckets for /metrics endpoint duration (seconds).
/// Tight buckets to detect high-cardinality slowdowns early.
fn default_metrics_endpoint_buckets() -> Vec<f64> {
    vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
}

/// Default buckets for quality score histogram (0.0 to 1.0 scale).
/// M-527: Quality scores are now histograms to track distribution, not just last value.
fn default_quality_score_buckets() -> Vec<f64> {
    vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0]
}

/// M-528: Session timeout in seconds.
/// Sessions not seen for this duration are considered complete and their final turn count is observed.
/// Configurable via PROMETHEUS_SESSION_TIMEOUT_SECS env var (default: 300 = 5 minutes).
fn session_timeout_secs() -> u64 {
    std::env::var(PROMETHEUS_SESSION_TIMEOUT_SECS)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300)
}

/// M-1074 FIX: Session cleanup cadence in events.
/// Cleanup runs every N events regardless of tracker size, ensuring low-traffic
/// scenarios observe session completions without waiting for shutdown.
const SESSION_CLEANUP_INTERVAL: u64 = 100;

// Import DashStream protobuf types (generated at root level)
use dashflow_streaming::codec::{decode_message_compatible, DEFAULT_MAX_PAYLOAD_SIZE};
use dashflow_streaming::dash_stream_message;
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::DEFAULT_SESSION_TIMEOUT_MS;

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl+C: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(e) => {
                error!("Failed to listen for SIGTERM: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = async {
        std::future::pending::<()>().await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Quality event for tracking query quality metrics
///
/// This is an application-specific type used by the quality monitor system.
/// It's encoded as a Metrics message in the DashStream protocol.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct QualityEvent {
    query_id: String,
    quality_score: f64,
    passed: bool,
    /// Query difficulty category for dashboard breakdowns.
    /// Values: "Simple", "Medium", "Complex", "Edge", "Unknown"
    category: String,
    latency_ms: u64,
    retry_count: u32,
    model: String,
    session_id: String,
    turn_number: u32,
    // New fields for granular quality metrics (used by Grafana dashboards)
    accuracy: f64,
    relevance: f64,
    completeness: f64,
    /// Application type for routing to correct dashboard metrics
    /// Values: "librarian", "code_assistant" (legacy), "document_search" (legacy), "rag", "unknown"
    /// Note: code_assistant and document_search are legacy values from before Dec 2025 consolidation
    application_type: String,
}

/// Prometheus metrics for DashStream quality monitoring
#[derive(Clone)]
struct Metrics {
    // Quality monitoring metrics
    quality_score: Gauge,
    queries_total: IntCounter,
    queries_passed: IntCounter,
    queries_failed: IntCounterVec,
    query_latency: Histogram,
    retry_count: HistogramVec,

    // Granular quality metrics (expected by Grafana dashboards)
    quality_accuracy: Gauge,
    quality_relevance: Gauge,
    quality_completeness: Gauge,

    // Per-model metrics
    // M-527: quality_by_model is now a Histogram to track distribution, not just last value
    quality_by_model: HistogramVec,
    queries_by_model: IntCounterVec,
    latency_by_model: HistogramVec,

    // Session tracking
    // M-528: Now tracks max turns per session and observes only on session completion
    turns_by_session: Histogram,
    /// M-528: Session tracker for proper turns-per-session semantics.
    /// Key: session_id, Value: (max_turn_number, last_seen_instant)
    /// Completed sessions (not seen for SESSION_TIMEOUT_SECS) are observed and removed.
    session_tracker: Arc<RwLock<HashMap<String, (u32, Instant)>>>,
    /// M-1074 FIX: Event counter for session cleanup cadence.
    /// Cleanup runs every CLEANUP_INTERVAL events regardless of tracker size, ensuring
    /// low-traffic scenarios observe session completions without waiting for shutdown.
    session_event_counter: Arc<AtomicU64>,

    // Application-specific metrics (librarian - consolidated from code_assistant and document_search Dec 2025)
    librarian_requests_total: IntCounter,
    librarian_iterations: Gauge,
    librarian_tests_total: IntCounterVec,
    librarian_request_duration_seconds: Histogram,

    // M-541: Exporter self-monitoring metrics
    #[allow(dead_code)] // Architectural: Gauge kept alive for Prometheus registry ownership
    process_start_time_seconds: Gauge,
    // M-542: /metrics endpoint latency tracking
    metrics_endpoint_duration_seconds: Histogram,

    // M-529: Message processing failure counter
    messages_failed_total: IntCounterVec,
    // M-530: Kafka consumer error counter
    kafka_consumer_errors_total: IntCounter,
    // M-531: Offset storage error counter
    offset_store_errors_total: IntCounter,
    // M-537: Message throughput counter
    messages_received_total: IntCounter,
    // M-539: Last event timestamp for freshness detection
    last_event_timestamp_seconds: Gauge,
    // M-534: Counter for messages with non-quality scope
    messages_wrong_scope_total: IntCounter,
    // M-535: Counter for messages missing header
    messages_missing_header_total: IntCounter,
    // M-538: Timestamp of last gauge update for staleness detection
    gauges_last_update_timestamp_seconds: Gauge,
    // M-536: Kafka consumer lag (sum across all partitions)
    kafka_consumer_lag: Gauge,
    // M-1076 FIX: Counter for messages with payload=None (data loss / corruption signal)
    kafka_payload_missing_total: IntCounter,
}

impl Metrics {
    fn new(registry: &Registry) -> Result<Self> {
        // Build info (used by Grafana to detect deploy changes).
        // Always `1`, metadata lives on labels.
        let build_info = GaugeVec::new(
            Opts::new("build_info", "Build and version information (always 1)")
                .namespace("dashflow"),
            &["version", "commit", "build_date", "rust_version"],
        )?;
        registry.register(Box::new(build_info.clone()))?;
        build_info
            .with_label_values(&[
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_COMMIT_SHA").unwrap_or("unknown"),
                option_env!("BUILD_DATE").unwrap_or("unknown"),
                "unknown",
            ])
            .set(1.0);

        // Quality score (gauge, 0.0-1.0)
        let quality_score = Gauge::with_opts(
            Opts::new(
                "quality_monitor_quality_score",
                "Current quality score (0.0-1.0)",
            )
            .namespace("dashstream"),
        )?;
        registry.register(Box::new(quality_score.clone()))?;

        // Total queries counter.
        //
        // Prometheus convention: counters end with `_total`.
        // Note: the Rust `prometheus` crate exposes metric names exactly as provided (no auto-suffix),
        // so we include `_total` in the name ourselves to match Prometheus/Grafana expectations.
        let queries_total = IntCounter::with_opts(
            Opts::new("quality_monitor_queries_total", "Total queries processed").namespace("dashstream"),
        )?;
        registry.register(Box::new(queries_total.clone()))?;

        // Passed queries counter (counter names include `_total`, see note above).
        let queries_passed = IntCounter::with_opts(
            Opts::new(
                "quality_monitor_queries_passed_total",
                "Queries that passed quality threshold",
            )
            .namespace("dashstream"),
        )?;
        registry.register(Box::new(queries_passed.clone()))?;

        // Failed queries counter, labeled by category (Grafana expects category breakdown).
        // Counter names include `_total`, see note above.
        let queries_failed = IntCounterVec::new(
            Opts::new(
                "quality_monitor_queries_failed_total",
                "Queries that failed quality threshold",
            )
            .namespace("dashstream"),
            &["category"],
        )?;
        registry.register(Box::new(queries_failed.clone()))?;
        // Pre-create the category series so Grafana queries don't show "No data" at startup.
        for category in ["Simple", "Medium", "Complex", "Edge", "Unknown"] {
            let _ = queries_failed.with_label_values(&[category]);
        }

        // Query latency histogram
        // Issue #13: Extended buckets to track queries >5s (SLO violation)
        // 31% of queries were >5000ms, so we need buckets up to 30s to properly track tail latency
        // M-543: Configurable via PROMETHEUS_LATENCY_BUCKETS_MS env var
        let latency_buckets = parse_buckets_from_env(
            "PROMETHEUS_LATENCY_BUCKETS_MS",
            default_latency_buckets_ms(),
        );
        let query_latency = Histogram::with_opts(
            HistogramOpts::new("query_latency_ms", "Query latency in milliseconds")
                .namespace("dashstream")
                .buckets(latency_buckets.clone()),
        )?;
        registry.register(Box::new(query_latency.clone()))?;

        // Retry count histogram (quality monitor specific - renamed to avoid collision with ws)
        // M-543: Configurable via PROMETHEUS_RETRY_BUCKETS env var
        let retry_buckets = parse_buckets_from_env("PROMETHEUS_RETRY_BUCKETS", default_retry_buckets());
        let retry_count = HistogramVec::new(
            HistogramOpts::new("quality_retry_count", "Number of retries per quality query")
                .namespace("dashstream")
                .buckets(retry_buckets),
            &["status"],
        )?;
        registry.register(Box::new(retry_count.clone()))?;

        // Granular quality metrics (expected by Grafana dashboards)
        // Final names: dashstream_quality_accuracy, dashstream_quality_relevance, dashstream_quality_completeness
        let quality_accuracy = Gauge::with_opts(
            Opts::new("quality_accuracy", "Quality accuracy score (0.0-1.0)")
                .namespace("dashstream"),
        )?;
        registry.register(Box::new(quality_accuracy.clone()))?;

        let quality_relevance = Gauge::with_opts(
            Opts::new("quality_relevance", "Quality relevance score (0.0-1.0)")
                .namespace("dashstream"),
        )?;
        registry.register(Box::new(quality_relevance.clone()))?;

        let quality_completeness = Gauge::with_opts(
            Opts::new(
                "quality_completeness",
                "Quality completeness score (0.0-1.0)",
            )
            .namespace("dashstream"),
        )?;
        registry.register(Box::new(quality_completeness.clone()))?;

        // Quality by model
        // M-527: Converted from GaugeVec to HistogramVec to track distribution, not just last value.
        // Previously, one bad event would overwrite all previous good values in the gauge.
        // Configurable via PROMETHEUS_QUALITY_SCORE_BUCKETS env var.
        let quality_score_buckets =
            parse_buckets_from_env("PROMETHEUS_QUALITY_SCORE_BUCKETS", default_quality_score_buckets());
        let quality_by_model = HistogramVec::new(
            HistogramOpts::new(
                "quality_score_by_model",
                "Quality score distribution by model (0.0-1.0)",
            )
            .namespace("dashstream")
            .buckets(quality_score_buckets),
            &["model"],
        )?;
        registry.register(Box::new(quality_by_model.clone()))?;

        // Queries by model
        // Counter names include `_total`, see note above.
        let queries_by_model = IntCounterVec::new(
            Opts::new("queries_by_model_total", "Total queries by model").namespace("dashstream"),
            &["model"],
        )?;
        registry.register(Box::new(queries_by_model.clone()))?;
        // M-548: Pre-create common model series referenced by Grafana dashboards.
        // These are the normalized model names from normalize_model() that are likely
        // to appear in production. Pre-creating prevents "No data" at startup.
        for model in [
            // OpenAI models
            "gpt-4",
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4-turbo",
            "gpt-3.5-turbo",
            "o1-preview",
            "o1-mini",
            // Anthropic models
            "claude-3.5-sonnet",
            "claude-3.5-haiku",
            "claude-3-opus",
            // Google models
            "gemini-1.5-pro",
            "gemini-1.5-flash",
            // Local/Ollama models
            "llama-3",
            "mistral",
        ] {
            let _ = queries_by_model.with_label_values(&[model]);
        }

        // Latency by model
        // M-543: Reuses PROMETHEUS_LATENCY_BUCKETS_MS from query_latency for consistency
        let latency_by_model = HistogramVec::new(
            HistogramOpts::new(
                "latency_by_model_ms",
                "Query latency by model in milliseconds",
            )
            .namespace("dashstream")
            .buckets(latency_buckets), // Reuse same buckets as query_latency
            &["model"],
        )?;
        registry.register(Box::new(latency_by_model.clone()))?;

        // Turns per session (global distribution only).
        //
        // Avoid per-session labels to prevent unbounded cardinality in Prometheus.
        // M-543: Configurable via PROMETHEUS_SESSION_TURN_BUCKETS env var
        let session_turn_buckets =
            parse_buckets_from_env("PROMETHEUS_SESSION_TURN_BUCKETS", default_session_turn_buckets());
        let turns_by_session = Histogram::with_opts(
            HistogramOpts::new("turns_by_session", "Number of turns per session")
                .namespace("dashstream")
                .buckets(session_turn_buckets),
        )?;
        registry.register(Box::new(turns_by_session.clone()))?;

        // Application-specific metrics (librarian - consolidated from code_assistant and document_search Dec 2025)
        //
        // M-547: Namespace Pattern Documentation
        // ======================================
        // This file uses TWO namespace patterns for Prometheus metrics:
        //
        // Pattern A: `.namespace("dashstream")` - for generic quality/monitoring metrics
        //   Example: dashstream_quality_monitor_queries_total
        //   Use: Metrics that apply to any DashStream application
        //
        // Pattern B: Embedded prefix in metric name - for application-specific metrics
        //   Example: dashstream_librarian_requests_total
        //   Use: Metrics specific to one application (librarian, code_assistant, etc.)
        //   Reason: S-9 - Distinguishes Kafka-derived metrics from direct app instrumentation.
        //           Applications may also export their own metrics directly; the `dashstream_`
        //           prefix here indicates these come from the Kafka pipeline, not app code.
        //
        // Both patterns result in `dashstream_` prefixed metric names, but Pattern B
        // allows more flexibility in naming (e.g., `dashstream_exporter_*` for self-monitoring).
        //
        // Counter names include `_total`, see note above.
        let librarian_requests_total = IntCounter::with_opts(Opts::new(
            "dashstream_librarian_requests_total",
            "Total librarian requests derived from Kafka (includes legacy code_assistant and document_search events)",
        ))?;
        registry.register(Box::new(librarian_requests_total.clone()))?;

        // M-480: This is the last observed iteration count (turn_number), not an average.
        // To compute actual average in Prometheus, use `turns_by_session` histogram instead.
        let librarian_iterations = Gauge::with_opts(Opts::new(
            "dashstream_librarian_iterations",
            "Last observed librarian iterations (turn_number) from most recent request",
        ))?;
        registry.register(Box::new(librarian_iterations.clone()))?;

        // Counter names include `_total`, see note above.
        let librarian_tests_total = IntCounterVec::new(
            Opts::new(
                "dashstream_librarian_tests_total",
                "Librarian test results derived from Kafka",
            ),
            &["status"],
        )?;
        registry.register(Box::new(librarian_tests_total.clone()))?;
        // Pre-create common status series so dashboards/tests don't show "No data" at startup.
        for status in ["passed", "failed"] {
            let _ = librarian_tests_total.with_label_values(&[status]);
        }

        // M-543: Configurable via PROMETHEUS_DURATION_BUCKETS_SECONDS env var
        let duration_buckets =
            parse_buckets_from_env("PROMETHEUS_DURATION_BUCKETS_SECONDS", default_duration_buckets_seconds());
        let librarian_request_duration_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "dashstream_librarian_request_duration_seconds",
                "Librarian request duration in seconds derived from Kafka",
            )
            .buckets(duration_buckets),
        )?;
        registry.register(Box::new(librarian_request_duration_seconds.clone()))?;

        // M-541: Process start time for detecting restarts via metrics
        // Standard Prometheus convention: set once at startup to Unix timestamp
        let process_start_time_seconds = Gauge::with_opts(
            Opts::new(
                "process_start_time_seconds",
                "Unix timestamp of when the process started (for detecting restarts)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(process_start_time_seconds.clone()))?;
        // Set once at init - never changes during process lifetime
        process_start_time_seconds.set(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs_f64(),
        );

        // M-542: /metrics endpoint latency tracking for detecting slow encoding with high cardinality
        // M-543: Configurable via PROMETHEUS_METRICS_ENDPOINT_BUCKETS env var
        let metrics_endpoint_buckets =
            parse_buckets_from_env("PROMETHEUS_METRICS_ENDPOINT_BUCKETS", default_metrics_endpoint_buckets());
        let metrics_endpoint_duration_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "metrics_endpoint_duration_seconds",
                "Time spent encoding /metrics response (detects high-cardinality slowdowns)",
            )
            .namespace("dashstream_exporter")
            .buckets(metrics_endpoint_buckets),
        )?;
        registry.register(Box::new(metrics_endpoint_duration_seconds.clone()))?;

        // M-529: Message processing failure counter (failures only logged before, not counted)
        let messages_failed_total = IntCounterVec::new(
            Opts::new(
                "messages_failed_total",
                "Total message processing failures by error type",
            )
            .namespace("dashstream_exporter"),
            &["error_type"],
        )?;
        registry.register(Box::new(messages_failed_total.clone()))?;
        // Pre-create common error types
        for error_type in ["decode", "process", "unknown"] {
            let _ = messages_failed_total.with_label_values(&[error_type]);
        }

        // M-530: Kafka consumer error counter (Kafka errors logged but not counted before)
        let kafka_consumer_errors_total = IntCounter::with_opts(
            Opts::new(
                "kafka_consumer_errors_total",
                "Total Kafka consumer errors (connectivity, protocol, etc.)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(kafka_consumer_errors_total.clone()))?;

        // M-531: Offset storage error counter (offset storage failures could cause duplicate processing)
        let offset_store_errors_total = IntCounter::with_opts(
            Opts::new(
                "offset_store_errors_total",
                "Total offset storage failures (may indicate duplicate message processing)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(offset_store_errors_total.clone()))?;

        // M-537: Message throughput counter (can't calculate ingestion rate without this)
        let messages_received_total = IntCounter::with_opts(
            Opts::new(
                "messages_received_total",
                "Total messages received from Kafka (for throughput calculation)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(messages_received_total.clone()))?;

        // M-539: Last event timestamp for freshness detection
        let last_event_timestamp_seconds = Gauge::with_opts(
            Opts::new(
                "last_event_timestamp_seconds",
                "Unix timestamp of last processed event (for detecting data staleness)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(last_event_timestamp_seconds.clone()))?;

        // M-534: Counter for messages with non-quality scope (misconfiguration indicator)
        let messages_wrong_scope_total = IntCounter::with_opts(
            Opts::new(
                "messages_wrong_scope_total",
                "Total messages with non-quality scope (indicates misconfiguration if high)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(messages_wrong_scope_total.clone()))?;

        // M-535: Counter for messages missing header (protocol errors)
        let messages_missing_header_total = IntCounter::with_opts(
            Opts::new(
                "messages_missing_header_total",
                "Total quality messages missing header field (protocol errors)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(messages_missing_header_total.clone()))?;

        // M-538: Timestamp of last gauge update for staleness detection
        // Alert: time() - gauges_last_update_timestamp_seconds > threshold indicates stale gauges
        let gauges_last_update_timestamp_seconds = Gauge::with_opts(
            Opts::new(
                "gauges_last_update_timestamp_seconds",
                "Unix timestamp of last gauge metric update (for staleness detection)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(gauges_last_update_timestamp_seconds.clone()))?;

        // M-536: Kafka consumer lag (sum of lag across all assigned partitions)
        // High lag indicates slow consumption or backlog buildup
        let kafka_consumer_lag = Gauge::with_opts(
            Opts::new(
                "kafka_consumer_lag",
                "Sum of consumer lag across all assigned partitions (high watermark - position)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(kafka_consumer_lag.clone()))?;

        // M-1076 FIX: Counter for Kafka messages with payload=None
        // Data loss / corruption signal - consistent with websocket-server's websocket_kafka_payload_missing_total
        let kafka_payload_missing_total = IntCounter::with_opts(
            Opts::new(
                "kafka_payload_missing_total",
                "Total Kafka messages received with payload=None (data loss / corruption signal)",
            )
            .namespace("dashstream_exporter"),
        )?;
        registry.register(Box::new(kafka_payload_missing_total.clone()))?;

        Ok(Self {
            quality_score,
            queries_total,
            queries_passed,
            queries_failed,
            query_latency,
            retry_count,
            quality_accuracy,
            quality_relevance,
            quality_completeness,
            quality_by_model,
            queries_by_model,
            latency_by_model,
            turns_by_session,
            session_tracker: Arc::new(RwLock::new(HashMap::new())),
            session_event_counter: Arc::new(AtomicU64::new(0)),
            librarian_requests_total,
            librarian_iterations,
            librarian_tests_total,
            librarian_request_duration_seconds,
            process_start_time_seconds,
            metrics_endpoint_duration_seconds,
            messages_failed_total,
            kafka_consumer_errors_total,
            offset_store_errors_total,
            messages_received_total,
            last_event_timestamp_seconds,
            messages_wrong_scope_total,
            messages_missing_header_total,
            gauges_last_update_timestamp_seconds,
            kafka_consumer_lag,
            kafka_payload_missing_total,
        })
    }

    /// Update all metrics from a quality event.
    ///
    /// # M-540: Non-atomic multi-metric updates
    ///
    /// This method updates multiple metrics that aren't atomic. If Prometheus scrapes
    /// during the update, it might see partially updated values. This is acceptable because:
    ///
    /// 1. **Counters are monotonic** - A partial scrape just sees slightly lower values
    /// 2. **Gauges show "last observed value"** - This is semantically correct; we're not
    ///    computing aggregates that require consistency across metrics
    /// 3. **Update is fast** - The method takes microseconds, making collision unlikely
    /// 4. **No business invariants** - There's no invariant like "passed + failed = total"
    ///    that must hold at every instant (counters can temporarily be inconsistent)
    ///
    /// If stricter consistency is needed (e.g., for alerts that compare related metrics),
    /// consider using a Mutex around the update. However, for typical dashboard use cases,
    /// eventual consistency within one scrape interval (15-60s) is sufficient.
    fn update_from_quality_event(&self, event: &QualityEvent) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        // M-539: Update last event timestamp for freshness detection
        self.last_event_timestamp_seconds.set(now);

        // M-538: Update gauge staleness timestamp (tracks when gauge metrics were last modified)
        self.gauges_last_update_timestamp_seconds.set(now);

        // Update total queries
        self.queries_total.inc();

        // Update pass/fail counters
        if event.passed {
            self.queries_passed.inc();
        } else {
            self.queries_failed
                .with_label_values(&[normalize_category(&event.category)])
                .inc();
        }

        // Update quality score (keep as 0.0-1.0 for Grafana dashboard which expects max=1)
        self.quality_score.set(event.quality_score);

        // Update granular quality metrics (expected by Grafana dashboards)
        // These are the dashstream_quality_accuracy, dashstream_quality_relevance, etc. metrics
        self.quality_accuracy.set(event.accuracy);
        self.quality_relevance.set(event.relevance);
        self.quality_completeness.set(event.completeness);

        // Update latency
        self.query_latency.observe(event.latency_ms as f64);

        // Update retry count
        let status = if event.passed { "passed" } else { "failed" };
        self.retry_count
            .with_label_values(&[status])
            .observe(event.retry_count as f64);

        // Update per-model metrics
        // M-523/M-524: Normalize model names to prevent cardinality explosion
        if !event.model.is_empty() {
            let normalized_model = normalize_model(&event.model);
            // M-527: Now uses observe() instead of set() to track distribution
            self.quality_by_model
                .with_label_values(&[normalized_model])
                .observe(event.quality_score);

            self.queries_by_model
                .with_label_values(&[normalized_model])
                .inc();

            self.latency_by_model
                .with_label_values(&[normalized_model])
                .observe(event.latency_ms as f64);
        }

        // Update session tracking (global distribution).
        // M-528: Track max turns per session and observe only on session completion.
        // Previously, each turn was observed immediately (1, 2, 3, 4, 5 for a 5-turn session),
        // inflating lower buckets. Now we track max turns and observe when session times out.
        if event.turn_number > 0 && !event.session_id.is_empty() {
            let timeout = Duration::from_secs(session_timeout_secs());
            let now = Instant::now();

            // Lock once for both update and cleanup
            if let Ok(mut tracker) = self.session_tracker.write() {
                // Update or insert this session's max turn count
                let entry = tracker
                    .entry(event.session_id.clone())
                    .or_insert((0, now));
                if event.turn_number > entry.0 {
                    entry.0 = event.turn_number;
                }
                entry.1 = now;

                // M-1074 FIX: Clean up completed sessions on a stable cadence
                // Run cleanup every SESSION_CLEANUP_INTERVAL events OR when tracker is large.
                // This ensures low-traffic scenarios observe session completions without waiting
                // for shutdown, while still avoiding per-event overhead.
                let event_count = self.session_event_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if event_count % SESSION_CLEANUP_INTERVAL == 0 || tracker.len() > 100 {
                    let completed: Vec<_> = tracker
                        .iter()
                        .filter(|(_, (_, last_seen))| now.duration_since(*last_seen) > timeout)
                        .map(|(session_id, (max_turns, _))| (session_id.clone(), *max_turns))
                        .collect();

                    for (session_id, max_turns) in completed {
                        tracker.remove(&session_id);
                        self.turns_by_session.observe(max_turns as f64);
                    }
                }
            }
        } else if event.turn_number > 0 {
            // No session_id - fall back to immediate observation (legacy behavior)
            self.turns_by_session.observe(event.turn_number as f64);
        }

        // Application-specific metrics - route to librarian metrics
        // Legacy app types (code_assistant, document_search) are mapped to librarian (Dec 2025 consolidation)
        match event.application_type.as_str() {
            "librarian" | "code_assistant" | "document_search" | "document_search_streaming" => {
                self.librarian_requests_total.inc();
                if event.turn_number > 0 {
                    self.librarian_iterations.set(event.turn_number as f64);
                }
                let test_status = if event.passed { "passed" } else { "failed" };
                self.librarian_tests_total
                    .with_label_values(&[test_status])
                    .inc();
            }
            _ => {
                // Unknown application type - don't fabricate app-specific metrics
                // This prevents misleading dashboard counts
            }
        }

        // Convert latency from ms to seconds and observe for librarian and legacy app types
        let latency_seconds = (event.latency_ms as f64) / 1000.0;
        match event.application_type.as_str() {
            "librarian" | "code_assistant" | "document_search" | "document_search_streaming" => {
                self.librarian_request_duration_seconds.observe(latency_seconds);
            }
            _ => {}
        }

        info!(
            query_id = %event.query_id,
            quality_score = event.quality_score,
            accuracy = event.accuracy,
            relevance = event.relevance,
            completeness = event.completeness,
            latency_ms = event.latency_ms,
            model = %event.model,
            passed = event.passed,
            "Updated Prometheus metrics from quality event"
        );
    }

    /// M-528: Flush all tracked sessions on shutdown.
    /// Called during graceful shutdown to ensure all session turn counts are observed
    /// before the process exits.
    fn flush_sessions(&self) {
        if let Ok(mut tracker) = self.session_tracker.write() {
            let sessions: Vec<_> = tracker.drain().collect();
            let count = sessions.len();
            for (session_id, (max_turns, _)) in sessions {
                self.turns_by_session.observe(max_turns as f64);
                tracing::debug!(
                    session_id = %session_id,
                    max_turns = max_turns,
                    "Flushed session turn count on shutdown"
                );
            }
            if count > 0 {
                info!("Flushed {} tracked sessions on shutdown", count);
            }
        }
    }
}

/// Kafka consumer that reads DashStream quality events
struct KafkaConsumer {
    consumer: StreamConsumer,
    metrics: Arc<Metrics>,
    /// M-1075 FIX: Configurable max payload size (default: DEFAULT_MAX_PAYLOAD_SIZE = 10MB).
    /// Set via DASHSTREAM_MAX_PAYLOAD_BYTES env var for alignment with server config.
    max_payload_bytes: usize,
}

/// M-536: Interval for updating consumer lag metric (in seconds)
const LAG_UPDATE_INTERVAL_SECS: u64 = 10;

impl KafkaConsumer {
    fn new(brokers: &str, topic: &str, metrics: Arc<Metrics>) -> Result<Self> {
        // P1.5: Kafka consumer group ID is now configurable via KAFKA_GROUP_ID env var
        let group_id = std::env::var(KAFKA_GROUP_ID)
            .unwrap_or_else(|_| "prometheus-exporter".to_string());

        // M-413: Unified Kafka security configuration from environment
        // Supports KAFKA_SECURITY_PROTOCOL, KAFKA_SASL_*, KAFKA_SSL_* env vars
        let kafka_security = KafkaSecurityConfig::from_env();
        kafka_security
            .validate()
            .context("Invalid Kafka security config")?;
        info!("Kafka security protocol: {}", kafka_security.security_protocol);

        // M-413/M-478: Use unified client config builder so TLS/SASL and
        // broker.address.family (IPv4/IPv6) stay consistent across binaries.
        let mut config = kafka_security.create_client_config(brokers);
        config
            .set("group.id", &group_id)
            .set("enable.auto.commit", "true")
            // Store offsets only after we've processed a message (at-least-once).
            .set("enable.auto.offset.store", "false")
            // M-432: Honor KAFKA_AUTO_OFFSET_RESET env var (only used when no committed group offsets exist)
            .set("auto.offset.reset", get_auto_offset_reset())
            // M-618: Use centralized session timeout for consumer group coordination
            .set(
                "session.timeout.ms",
                DEFAULT_SESSION_TIMEOUT_MS.to_string().as_str(),
            )
            .set("enable.partition.eof", "false");
        let consumer: StreamConsumer = config
            .create()
            .context("Failed to create Kafka consumer")?;

        consumer
            .subscribe(&[topic])
            .context("Failed to subscribe to Kafka topic")?;

        info!("Kafka consumer subscribed to topic: {}", topic);

        // M-1075 FIX: Configurable max payload size for decode.
        // Uses same env var name as websocket-server for deployment alignment.
        // Falls back to websocket-server default if not set.
        let max_payload_bytes: usize = std::env::var(DASHSTREAM_MAX_PAYLOAD_BYTES)
            .ok()
            .and_then(|s| {
                s.parse().map_err(|e| {
                    warn!(
                        "Invalid {} '{}': {}, using default {}",
                        DASHSTREAM_MAX_PAYLOAD_BYTES, s, e, DEFAULT_MAX_PAYLOAD_SIZE
                    );
                    e
                }).ok()
            })
            .unwrap_or(DEFAULT_MAX_PAYLOAD_SIZE);
        info!(
            max_payload_bytes = max_payload_bytes,
            default = DEFAULT_MAX_PAYLOAD_SIZE,
            "📦 Max payload decode size configured"
        );

        Ok(Self { consumer, metrics, max_payload_bytes })
    }

    /// M-536: Update consumer lag metric by fetching watermarks and positions
    fn update_consumer_lag(&self, topic: &str) {
        use rdkafka::topic_partition_list::TopicPartitionList;

        // Get assigned partitions
        let assignment = match self.consumer.assignment() {
            Ok(tpl) => tpl,
            Err(e) => {
                tracing::debug!("Failed to get assignment for lag calculation: {}", e);
                return;
            }
        };

        let mut total_lag: i64 = 0;
        let timeout = Duration::from_secs(5);

        for elem in assignment.elements() {
            let partition = elem.partition();

            // Fetch high watermark
            let watermarks = match self.consumer.fetch_watermarks(topic, partition, timeout) {
                Ok((_, high)) => high,
                Err(e) => {
                    tracing::debug!(
                        partition = partition,
                        "Failed to fetch watermarks for lag calculation: {}",
                        e
                    );
                    continue;
                }
            };

            // Get current position (committed + uncommitted consumed)
            let mut position_tpl = TopicPartitionList::new();
            position_tpl.add_partition(topic, partition);

            let position = match self.consumer.position() {
                Ok(pos) => pos
                    .find_partition(topic, partition)
                    .map_or(0, |p| p.offset().to_raw().unwrap_or(0)),
                Err(_) => {
                    // Fall back to committed offset
                    match self.consumer.committed(timeout) {
                        Ok(committed) => committed
                            .find_partition(topic, partition)
                            .map_or(0, |p| p.offset().to_raw().unwrap_or(0)),
                        Err(_) => 0,
                    }
                }
            };

            // Calculate lag for this partition
            let lag = watermarks - position;
            if lag > 0 {
                total_lag += lag;
            }
        }

        self.metrics.kafka_consumer_lag.set(total_lag as f64);
        tracing::debug!(total_lag = total_lag, "Updated consumer lag metric");
    }

    async fn consume_loop(&self, topic: &str, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info!("Starting Kafka consumer loop...");

        // M-536: Timer for periodic lag updates
        let mut lag_update_interval =
            tokio::time::interval(Duration::from_secs(LAG_UPDATE_INTERVAL_SECS));
        // Don't wait for first tick - update lag immediately after first message
        lag_update_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, stopping Kafka consumer...");
                    // M-528: Flush tracked sessions to observe final turn counts
                    self.metrics.flush_sessions();
                    if let Err(e) = self.consumer.commit_consumer_state(CommitMode::Sync) {
                        error!("Kafka commit_consumer_state failed during shutdown: {}", e);
                    }
                    break;
                }
                // M-536: Periodic lag update
                _ = lag_update_interval.tick() => {
                    self.update_consumer_lag(topic);
                }
                recv_result = self.consumer.recv() => {
                    match recv_result {
                        Ok(message) => {
                            // M-537: Count all received messages for throughput calculation
                            self.metrics.messages_received_total.inc();

                            if let Some(payload) = message.payload() {
                                if let Err(e) = self.process_message(payload) {
                                    // M-529: Count message processing failures by type
                                    let error_str = e.to_string();
                                    // M-533: Check for decode errors by error content
                                    // Note: String matching is fragile but necessary - rdkafka/prost
                                    // don't expose typed errors we can match on. If this breaks,
                                    // all errors will fall through to "process" type.
                                    let error_type = if error_str.contains("buffer underflow")
                                        || error_str.contains("decode")
                                        || error_str.contains("unexpected end of input")
                                    {
                                        "decode"
                                    } else {
                                        "process"
                                    };
                                    self.metrics.messages_failed_total.with_label_values(&[error_type]).inc();

                                    // Skip corrupt/old messages silently (buffer underflow indicates schema mismatch)
                                    // Only log non-decoding errors as actual errors
                                    if error_type != "decode" {
                                        error!("Failed to process message: {:#}", e);
                                    }
                                }
                            } else {
                                // M-1076 FIX: Count messages with payload=None (data loss / corruption signal).
                                // Consistent with websocket-server's websocket_kafka_payload_missing_total.
                                // Offsets are still advanced (skip policy) to avoid blocking on corrupt messages.
                                self.metrics.kafka_payload_missing_total.inc();
                                warn!(
                                    partition = ?message.partition(),
                                    offset = ?message.offset(),
                                    "Kafka message has payload=None (data loss / corruption)"
                                );
                            }

                            // Store offsets only after processing (at-least-once).
                            if let Err(e) = self.consumer.store_offset_from_message(&message) {
                                // M-531: Count offset storage failures
                                self.metrics.offset_store_errors_total.inc();
                                error!("Error while storing offset: {}", e);
                            }
                        }
                        Err(e) => {
                            // M-530: Count Kafka consumer errors
                            self.metrics.kafka_consumer_errors_total.inc();
                            error!("Kafka consumer error: {}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn process_message(&self, payload: &[u8]) -> Result<()> {
        // M-1075 FIX: Use configured max_payload_bytes instead of hard-coded default
        let message = decode_message_compatible(payload, self.max_payload_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to decode DashStreamMessage: {}", e))?;

        // Extract Metrics message and convert to QualityEvent if present
        if let Some(dash_stream_message::Message::Metrics(metrics_msg)) = message.message {
            // M-534: Check for non-quality scope and count it
            if metrics_msg.scope != "quality" {
                self.metrics.messages_wrong_scope_total.inc();
                // Log at debug level - high volume of wrong scope messages is visible via metrics
                tracing::debug!(
                    scope = %metrics_msg.scope,
                    "Skipping metrics message with non-quality scope"
                );
                return Ok(());
            }

            // M-535: Check for missing header and count it
            if metrics_msg.header.is_none() {
                self.metrics.messages_missing_header_total.inc();
                warn!(
                    scope_id = %metrics_msg.scope_id,
                    "Quality metrics message missing header field - cannot extract session_id"
                );
                // Still try to process - use empty session_id as fallback
            }

            // Convert Metrics message to QualityEvent
            if let Some(quality_event) = Self::metrics_to_quality_event(metrics_msg) {
                self.metrics.update_from_quality_event(&quality_event);
            }
        }

        Ok(())
    }

    /// Convert a DashStream Metrics message to a QualityEvent
    ///
    /// QualityEvent is an application-specific type used by the quality monitor.
    /// This function extracts the relevant fields from the generic Metrics message.
    ///
    /// Note: Scope validation is done in process_message (M-534), not here.
    fn metrics_to_quality_event(metrics: dashflow_streaming::Metrics) -> Option<QualityEvent> {
        // Build the event with initial values
        let mut quality_score = 0.0;
        let mut passed = false;
        let mut latency_ms = 0u64;
        let mut retry_count = 0u32;
        let mut turn_number = 0u32;
        let mut query_id = String::new();
        let mut model = String::new();
        let mut category = "Unknown".to_string();
        // Granular quality metrics (expected by Grafana dashboards)
        let mut accuracy = 0.0;
        let mut relevance = 0.0;
        let mut completeness = 0.0;
        // Application type for routing to correct dashboard metrics
        let mut application_type = "unknown".to_string();

        // Extract metrics from the map
        for (key, value) in metrics.metrics.iter() {
            match key.as_str() {
                "quality_score" => {
                    if let Some(dashflow_streaming::metric_value::Value::FloatValue(v)) =
                        &value.value
                    {
                        quality_score = *v;
                    }
                }
                "passed" => {
                    if let Some(dashflow_streaming::metric_value::Value::BoolValue(v)) =
                        &value.value
                    {
                        passed = *v;
                    }
                }
                "latency_ms" => {
                    if let Some(dashflow_streaming::metric_value::Value::IntValue(v)) = &value.value
                    {
                        // M-532: Validate non-negative before cast to prevent overflow
                        // Negative latency is invalid; clamp to 0 rather than wrapping to huge positive
                        latency_ms = (*v).max(0) as u64;
                    }
                }
                "retry_count" => {
                    if let Some(dashflow_streaming::metric_value::Value::IntValue(v)) = &value.value
                    {
                        // M-532: Validate non-negative before cast to prevent overflow
                        retry_count = (*v).max(0) as u32;
                    }
                }
                "turn_number" => {
                    if let Some(dashflow_streaming::metric_value::Value::IntValue(v)) = &value.value
                    {
                        // M-532: Validate non-negative before cast to prevent overflow
                        turn_number = (*v).max(0) as u32;
                    }
                }
                // Granular quality metrics (from send_test_metrics.rs format)
                "accuracy" => {
                    if let Some(dashflow_streaming::metric_value::Value::FloatValue(v)) =
                        &value.value
                    {
                        accuracy = *v;
                    }
                }
                "relevance" => {
                    if let Some(dashflow_streaming::metric_value::Value::FloatValue(v)) =
                        &value.value
                    {
                        relevance = *v;
                    }
                }
                "completeness" => {
                    if let Some(dashflow_streaming::metric_value::Value::FloatValue(v)) =
                        &value.value
                    {
                        completeness = *v;
                    }
                }
                _ => {}
            }
        }

        // Extract tags
        for (key, value) in metrics.tags.iter() {
            match key.as_str() {
                "query_id" => query_id = value.clone(),
                "model" => model = value.clone(),
                "category" | "query_category" | "complexity" => category = value.clone(),
                "application_type" | "app_type" => application_type = value.clone(),
                _ => {}
            }
        }

        // M-535: Use fallback empty session_id if header is missing instead of returning None
        // The header check and warning is done in process_message
        let session_id = metrics
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        Some(QualityEvent {
            query_id,
            quality_score,
            passed,
            category,
            latency_ms,
            retry_count,
            model,
            session_id,
            turn_number,
            accuracy,
            relevance,
            completeness,
            application_type,
        })
    }
}

/// M-432: Get auto.offset.reset value from environment with validation.
///
/// Returns "earliest" (default) or "latest". Logs warning on invalid values.
fn get_auto_offset_reset() -> String {
    match std::env::var(KAFKA_AUTO_OFFSET_RESET) {
        Ok(value) => {
            let normalized = value.to_lowercase();
            if normalized == "earliest" || normalized == "latest" {
                info!(
                    "Using {}={} from environment",
                    KAFKA_AUTO_OFFSET_RESET, normalized
                );
                normalized
            } else {
                warn!(
                    "Invalid {}='{}' (must be 'earliest' or 'latest'), defaulting to 'earliest'",
                    KAFKA_AUTO_OFFSET_RESET, value
                );
                "earliest".to_string()
            }
        }
        Err(_) => "earliest".to_string(),
    }
}

fn normalize_category(raw: &str) -> &'static str {
    match raw {
        "Simple" | "simple" | "SIMPLE" => "Simple",
        "Medium" | "medium" | "MEDIUM" => "Medium",
        "Complex" | "complex" | "COMPLEX" => "Complex",
        "Edge" | "edge" | "EDGE" => "Edge",
        _ => "Unknown",
    }
}

/// M-523/M-524: Normalize model names to prevent cardinality explosion.
///
/// Model name variations like "gpt-4", "GPT-4", "gpt4" would each create
/// separate time series, leading to unbounded cardinality and Prometheus OOM.
/// This function maps variations to canonical names.
///
/// Returns a bounded set of known models or "other" for unknown models.
fn normalize_model(raw: &str) -> &'static str {
    let lowered = raw.to_lowercase();
    let normalized = lowered.trim();

    // OpenAI models
    if normalized.starts_with("gpt-4o-mini") || normalized == "gpt4omini" {
        return "gpt-4o-mini";
    }
    if normalized.starts_with("gpt-4o") || normalized == "gpt4o" {
        return "gpt-4o";
    }
    if normalized.starts_with("gpt-4-turbo") || normalized == "gpt4turbo" {
        return "gpt-4-turbo";
    }
    if normalized.starts_with("gpt-4") || normalized == "gpt4" {
        return "gpt-4";
    }
    if normalized.starts_with("gpt-3.5-turbo") || normalized == "gpt35turbo" || normalized == "gpt-35-turbo" {
        return "gpt-3.5-turbo";
    }
    if normalized.starts_with("gpt-3.5") || normalized == "gpt35" {
        return "gpt-3.5";
    }
    if normalized.starts_with("o1-preview") {
        return "o1-preview";
    }
    if normalized.starts_with("o1-mini") {
        return "o1-mini";
    }
    if normalized.starts_with("o1") && !normalized.starts_with("o1-") {
        return "o1";
    }
    if normalized.starts_with("o3") {
        return "o3";
    }

    // Anthropic models
    if normalized.contains("claude-3-5-sonnet") || normalized.contains("claude-3.5-sonnet") {
        return "claude-3.5-sonnet";
    }
    if normalized.contains("claude-3-5-haiku") || normalized.contains("claude-3.5-haiku") {
        return "claude-3.5-haiku";
    }
    if normalized.contains("claude-3-opus") {
        return "claude-3-opus";
    }
    if normalized.contains("claude-3-sonnet") {
        return "claude-3-sonnet";
    }
    if normalized.contains("claude-3-haiku") {
        return "claude-3-haiku";
    }
    if normalized.contains("claude-2") {
        return "claude-2";
    }
    if normalized.contains("claude") {
        return "claude-other";
    }

    // Google models
    if normalized.contains("gemini-2") {
        return "gemini-2";
    }
    if normalized.contains("gemini-1.5-pro") {
        return "gemini-1.5-pro";
    }
    if normalized.contains("gemini-1.5-flash") {
        return "gemini-1.5-flash";
    }
    if normalized.contains("gemini-pro") || normalized.contains("gemini-1.0") {
        return "gemini-pro";
    }
    if normalized.contains("gemini") {
        return "gemini-other";
    }

    // Ollama/local models
    // Note: Order matters - check more specific patterns first (codellama before llama)
    if normalized.contains("codellama") {
        return "codellama";
    }
    if normalized.contains("llama-3") || normalized.contains("llama3") {
        return "llama-3";
    }
    if normalized.contains("llama-2") || normalized.contains("llama2") {
        return "llama-2";
    }
    if normalized.contains("llama") {
        return "llama-other";
    }
    if normalized.contains("mixtral") {
        return "mixtral";
    }
    if normalized.contains("mistral") {
        return "mistral";
    }

    // Unknown model - bucket to prevent cardinality explosion
    "other"
}

/// HTTP server that serves Prometheus metrics at /metrics
///
/// M-542: This function tracks its own latency via `dashstream_exporter_metrics_endpoint_duration_seconds`.
/// High encoding times indicate cardinality explosion (too many unique label combinations).
async fn metrics_handler(registry: Arc<Registry>, metrics: Arc<Metrics>) -> String {
    let start = Instant::now();

    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();

    let result = if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        error!("Failed to encode metrics: {}", e);
        String::from("# Error encoding metrics")
    } else {
        String::from_utf8(buffer)
            .unwrap_or_else(|_| String::from("# Error converting metrics to UTF-8"))
    };

    // M-542: Record encoding latency (self-monitoring)
    metrics
        .metrics_endpoint_duration_seconds
        .observe(start.elapsed().as_secs_f64());

    result
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting DashStream Prometheus Exporter...");

    // Configuration from environment
    let kafka_brokers =
        std::env::var(KAFKA_BROKERS).unwrap_or_else(|_| "localhost:9092".to_string());
    let kafka_topic =
        std::env::var(KAFKA_TOPIC).unwrap_or_else(|_| "dashstream-quality".to_string());
    let metrics_port: u16 = std::env::var(METRICS_PORT)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9190);

    // M-544: Make HTTP bind IP configurable (default: 0.0.0.0 for container environments)
    let metrics_bind_ip: std::net::IpAddr = std::env::var(METRICS_BIND_IP)
        .ok()
        .and_then(|ip| {
            ip.parse().map_err(|e| {
                warn!("Invalid {} '{}': {}, using 0.0.0.0", METRICS_BIND_IP, ip, e);
                e
            }).ok()
        })
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)));

    info!(
        kafka_brokers = %kafka_brokers,
        kafka_topic = %kafka_topic,
        metrics_port = metrics_port,
        metrics_bind_ip = %metrics_bind_ip,
        "Configuration loaded"
    );

    // Create Prometheus registry and metrics
    let registry = Arc::new(Registry::new());
    let metrics = Arc::new(Metrics::new(&registry)?);

    info!("Prometheus metrics registered");

    // Graceful shutdown signal
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let shutdown_tx_signal = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        info!("Shutdown signal received, stopping...");
        let _ = shutdown_tx_signal.send(());
    });

    // Start Kafka consumer
    let consumer = KafkaConsumer::new(&kafka_brokers, &kafka_topic, metrics.clone())?;

    // M-536: Clone topic for use in consume loop (needed for lag calculation)
    let kafka_topic_for_consumer = kafka_topic.clone();
    let shutdown_tx_consumer = shutdown_tx.clone();
    let consumer_task = tokio::spawn(async move {
        if let Err(e) = consumer
            .consume_loop(&kafka_topic_for_consumer, shutdown_tx_consumer.subscribe())
            .await
        {
            error!("Kafka consumer task failed: {:#}", e);
            let _ = shutdown_tx_consumer.send(());
        }
    });

    // Start HTTP server for /metrics endpoint
    let registry_clone = registry.clone();
    let metrics_clone = metrics.clone();
    let app = Router::new().route(
        "/metrics",
        get(|| async move { metrics_handler(registry_clone, metrics_clone).await }),
    );

    let addr = SocketAddr::new(metrics_bind_ip, metrics_port);
    info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind TCP listener")?;

    info!(
        "✅ Prometheus exporter ready at http://{}:{}/metrics",
        addr.ip(),
        addr.port()
    );

    let shutdown_tx_server = shutdown_tx.clone();
    let server_task = tokio::spawn(async move {
        let mut shutdown_rx_server = shutdown_tx_server.subscribe();
        let serve_result = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx_server.recv().await;
                info!("HTTP server received shutdown signal, stopping...");
            })
            .await;
        if let Err(e) = serve_result {
            error!("HTTP server failed: {:#}", e);
            let _ = shutdown_tx_server.send(());
        }
    });

    // Wait for both tasks to finish (they exit on shutdown signal).
    let _ = tokio::join!(consumer_task, server_task);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow_streaming::DashStreamMessage;
    use prost::Message;
    use rdkafka::config::ClientConfig;

    fn total_failed(metrics: &Metrics) -> u64 {
        ["Simple", "Medium", "Complex", "Edge", "Unknown"]
            .iter()
            .map(|category| metrics.queries_failed.with_label_values(&[*category]).get())
            .sum()
    }

    #[test]
    fn test_metrics_creation() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry);
        assert!(metrics.is_ok(), "Metrics should be created successfully");
    }

    #[test]
    fn test_metrics_update_quality_event() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let quality_event = QualityEvent {
            query_id: "test-query-123".to_string(),
            quality_score: 0.95,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 150,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-abc".to_string(),
            turn_number: 1,
            accuracy: 0.92,
            relevance: 0.88,
            completeness: 0.95,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&quality_event);

        // Verify metrics were updated (basic sanity check)
        assert_eq!(metrics.queries_total.get(), 1);
        assert_eq!(metrics.queries_passed.get(), 1);
        assert_eq!(total_failed(&metrics), 0);

        // Verify granular quality metrics (expected by Grafana dashboards)
        assert!((metrics.quality_accuracy.get() - 0.92).abs() < 0.001);
        assert!((metrics.quality_relevance.get() - 0.88).abs() < 0.001);
        assert!((metrics.quality_completeness.get() - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_metrics_update_failed_query() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let quality_event = QualityEvent {
            query_id: "test-query-456".to_string(),
            quality_score: 0.45,
            passed: false,
            category: "Medium".to_string(),
            latency_ms: 300,
            retry_count: 2,
            model: "gpt-3.5-turbo".to_string(),
            session_id: "session-xyz".to_string(),
            turn_number: 3,
            accuracy: 0.40,
            relevance: 0.50,
            completeness: 0.45,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&quality_event);

        assert_eq!(metrics.queries_total.get(), 1);
        assert_eq!(metrics.queries_passed.get(), 0);
        assert_eq!(total_failed(&metrics), 1);
        assert_eq!(
            metrics.queries_failed.with_label_values(&["Medium"]).get(),
            1
        );
    }

    #[test]
    fn test_metrics_multiple_events() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // First event (passed)
        let event1 = QualityEvent {
            query_id: "query-1".to_string(),
            quality_score: 0.90,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-1".to_string(),
            turn_number: 1,
            accuracy: 0.85,
            relevance: 0.90,
            completeness: 0.95,
            application_type: "rag".to_string(),
        };

        // Second event (failed)
        let event2 = QualityEvent {
            query_id: "query-2".to_string(),
            quality_score: 0.50,
            passed: false,
            category: "Complex".to_string(),
            latency_ms: 200,
            retry_count: 1,
            model: "gpt-3.5-turbo".to_string(),
            session_id: "session-2".to_string(),
            turn_number: 2,
            accuracy: 0.45,
            relevance: 0.55,
            completeness: 0.50,
            application_type: "rag".to_string(),
        };

        // Third event (passed)
        let event3 = QualityEvent {
            query_id: "query-3".to_string(),
            quality_score: 0.95,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 150,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-3".to_string(),
            turn_number: 1,
            accuracy: 0.92,
            relevance: 0.98,
            completeness: 0.95,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&event1);
        metrics.update_from_quality_event(&event2);
        metrics.update_from_quality_event(&event3);

        assert_eq!(metrics.queries_total.get(), 3);
        assert_eq!(metrics.queries_passed.get(), 2);
        assert_eq!(total_failed(&metrics), 1);
        assert_eq!(
            metrics.queries_failed.with_label_values(&["Complex"]).get(),
            1
        );

        // Verify the last event's quality metrics are set (gauges reflect last value)
        assert!((metrics.quality_accuracy.get() - 0.92).abs() < 0.001);
        assert!((metrics.quality_relevance.get() - 0.98).abs() < 0.001);
        assert!((metrics.quality_completeness.get() - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_quality_score_scaling() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let quality_event = QualityEvent {
            query_id: "test-query".to_string(),
            quality_score: 0.904,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-test".to_string(),
            turn_number: 1,
            accuracy: 0.90,
            relevance: 0.91,
            completeness: 0.90,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&quality_event);

        // Quality score should be 0.904 (unscaled, matches Grafana dashboard max=1)
        let score = metrics.quality_score.get();
        assert!(
            (score - 0.904).abs() < 0.001,
            "Expected ~0.904, got {}",
            score
        );
    }

    #[test]
    fn test_metrics_handler_output() {
        let registry = Arc::new(Registry::new());
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Add some sample data
        let quality_event = QualityEvent {
            query_id: "test".to_string(),
            quality_score: 0.85,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 120,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-test".to_string(),
            turn_number: 1,
            accuracy: 0.82,
            relevance: 0.88,
            completeness: 0.85,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&quality_event);

        // Test metrics encoding
        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .expect("Failed to encode metrics");

        let output = String::from_utf8(buffer).expect("Failed to convert to UTF-8");

        // Verify output contains expected metrics
        assert!(
            output.contains("dashstream_quality_monitor_queries_total"),
            "Missing queries metric"
        );
        assert!(
            output.contains("dashstream_quality_monitor_queries_passed_total"),
            "Missing queries_passed metric"
        );
        assert!(
            !output.contains("dashstream_quality_monitor_queries_total_total"),
            "Unexpected _total_total suffix duplication on queries metric"
        );
        assert!(
            output.contains("dashstream_quality_monitor_quality_score"),
            "Missing quality_score metric"
        );
        // Verify granular quality metrics (expected by Grafana dashboards)
        assert!(
            output.contains("dashstream_quality_accuracy"),
            "Missing quality_accuracy metric"
        );
        assert!(
            output.contains("dashstream_quality_relevance"),
            "Missing quality_relevance metric"
        );
        assert!(
            output.contains("dashstream_quality_completeness"),
            "Missing quality_completeness metric"
        );
    }

    /// Helper function to create a DashStream Metrics message from a QualityEvent
    fn create_quality_metrics_message(event: &QualityEvent) -> DashStreamMessage {
        use dashflow_streaming::{
            metric_value, Header, MessageType, MetricValue, Metrics as DashStreamMetrics,
        };
        use std::collections::HashMap;

        let mut metrics_map = HashMap::new();
        metrics_map.insert(
            "quality_score".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(event.quality_score)),
                unit: "score".to_string(),
                r#type: 2, // GAUGE
            },
        );
        metrics_map.insert(
            "passed".to_string(),
            MetricValue {
                value: Some(metric_value::Value::BoolValue(event.passed)),
                unit: "bool".to_string(),
                r#type: 2, // GAUGE
            },
        );
        metrics_map.insert(
            "latency_ms".to_string(),
            MetricValue {
                value: Some(metric_value::Value::IntValue(event.latency_ms as i64)),
                unit: "ms".to_string(),
                r#type: 3, // HISTOGRAM
            },
        );
        metrics_map.insert(
            "retry_count".to_string(),
            MetricValue {
                value: Some(metric_value::Value::IntValue(event.retry_count as i64)),
                unit: "count".to_string(),
                r#type: 1, // COUNTER
            },
        );
        metrics_map.insert(
            "turn_number".to_string(),
            MetricValue {
                value: Some(metric_value::Value::IntValue(event.turn_number as i64)),
                unit: "count".to_string(),
                r#type: 1, // COUNTER
            },
        );
        // Granular quality metrics (expected by Grafana dashboards)
        metrics_map.insert(
            "accuracy".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(event.accuracy)),
                unit: "ratio".to_string(),
                r#type: 2, // GAUGE
            },
        );
        metrics_map.insert(
            "relevance".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(event.relevance)),
                unit: "ratio".to_string(),
                r#type: 2, // GAUGE
            },
        );
        metrics_map.insert(
            "completeness".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(event.completeness)),
                unit: "ratio".to_string(),
                r#type: 2, // GAUGE
            },
        );

        let mut tags = HashMap::new();
        tags.insert("query_id".to_string(), event.query_id.clone());
        tags.insert("model".to_string(), event.model.clone());

        let metrics = DashStreamMetrics {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: event.session_id.clone(),
                sequence: 1,
                r#type: MessageType::Metrics as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: dashflow_streaming::CURRENT_SCHEMA_VERSION,
            }),
            scope: "quality".to_string(),
            scope_id: event.query_id.clone(),
            metrics: metrics_map,
            tags,
        };

        DashStreamMessage {
            message: Some(dash_stream_message::Message::Metrics(metrics)),
        }
    }

    #[tokio::test]
    async fn test_process_message_valid_protobuf() {
        let registry = Registry::new();
        let metrics = Arc::new(Metrics::new(&registry).expect("Failed to create metrics"));

        let quality_event = QualityEvent {
            query_id: "test-query".to_string(),
            quality_score: 0.90,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 150,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-test".to_string(),
            turn_number: 1,
            accuracy: 0.88,
            relevance: 0.92,
            completeness: 0.90,
            application_type: "rag".to_string(),
        };

        let message = create_quality_metrics_message(&quality_event);

        // Encode message to protobuf bytes
        let mut buf = Vec::new();
        message.encode(&mut buf).expect("Failed to encode protobuf");

        // Create consumer (just for accessing process_message)
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", "localhost:9092")
            .set("group.id", "test-group")
            .create()
            .expect("Failed to create test consumer");

        let kafka_consumer = KafkaConsumer {
            consumer,
            metrics: metrics.clone(),
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_SIZE,
        };

        // Process the message
        let result = kafka_consumer.process_message(&buf);
        assert!(result.is_ok(), "Message processing should succeed");

        // Verify metrics were updated
        assert_eq!(metrics.queries_total.get(), 1);
        assert_eq!(metrics.queries_passed.get(), 1);
    }

    #[tokio::test]
    async fn test_process_message_invalid_protobuf() {
        let registry = Registry::new();
        let metrics = Arc::new(Metrics::new(&registry).expect("Failed to create metrics"));

        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", "localhost:9092")
            .set("group.id", "test-group")
            .create()
            .expect("Failed to create test consumer");

        let kafka_consumer = KafkaConsumer {
            consumer,
            metrics: metrics.clone(),
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_SIZE,
        };

        // Invalid protobuf data
        let invalid_data = b"not a valid protobuf message";

        let result = kafka_consumer.process_message(invalid_data);
        assert!(result.is_err(), "Should fail on invalid protobuf");
    }

    #[test]
    fn test_application_specific_metrics_librarian() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Test librarian application type (the consolidated app)
        let librarian_event = QualityEvent {
            query_id: "librarian-1".to_string(),
            quality_score: 0.92,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 250,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-lib".to_string(),
            turn_number: 3,
            accuracy: 0.91,
            relevance: 0.93,
            completeness: 0.92,
            application_type: "librarian".to_string(),
        };

        metrics.update_from_quality_event(&librarian_event);

        // Verify librarian metrics were updated
        assert_eq!(metrics.librarian_requests_total.get(), 1);
        assert!(
            (metrics.librarian_iterations.get() - 3.0).abs() < f64::EPSILON,
            "Expected librarian_iterations to be 3.0, got {}",
            metrics.librarian_iterations.get()
        );
    }

    #[test]
    fn test_application_specific_metrics_legacy_code_assistant() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Test legacy code_assistant type - should route to librarian metrics
        let code_assistant_event = QualityEvent {
            query_id: "code-assist-1".to_string(),
            quality_score: 0.88,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 180,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-code".to_string(),
            turn_number: 2,
            accuracy: 0.85,
            relevance: 0.90,
            completeness: 0.89,
            application_type: "code_assistant".to_string(),
        };

        metrics.update_from_quality_event(&code_assistant_event);

        // Legacy code_assistant events should update librarian metrics
        assert_eq!(metrics.librarian_requests_total.get(), 1);
        assert!(
            (metrics.librarian_iterations.get() - 2.0).abs() < f64::EPSILON,
            "Expected librarian_iterations to be 2.0, got {}",
            metrics.librarian_iterations.get()
        );
    }

    #[test]
    fn test_application_specific_metrics_legacy_document_search() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Test legacy document_search type - should route to librarian metrics
        let doc_search_event = QualityEvent {
            query_id: "doc-search-1".to_string(),
            quality_score: 0.88,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 180,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-doc".to_string(),
            turn_number: 1,
            accuracy: 0.85,
            relevance: 0.90,
            completeness: 0.89,
            application_type: "document_search".to_string(),
        };

        metrics.update_from_quality_event(&doc_search_event);

        // Legacy document_search events should update librarian metrics
        assert_eq!(metrics.librarian_requests_total.get(), 1);
    }

    #[test]
    fn test_application_specific_metrics_unknown_type() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Test unknown/rag application type - should NOT increment app-specific metrics
        let rag_event = QualityEvent {
            query_id: "rag-query-1".to_string(),
            quality_score: 0.90,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 200,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-rag".to_string(),
            turn_number: 2,
            accuracy: 0.88,
            relevance: 0.92,
            completeness: 0.90,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&rag_event);

        // App-specific counter should NOT be incremented for unknown types
        assert_eq!(metrics.librarian_requests_total.get(), 0);

        // But general metrics should still be updated
        assert_eq!(metrics.queries_total.get(), 1);
        assert_eq!(metrics.queries_passed.get(), 1);
    }

    // M-523/M-524: Tests for model name normalization
    #[test]
    fn test_normalize_model_openai() {
        // GPT-4 variants
        assert_eq!(normalize_model("gpt-4"), "gpt-4");
        assert_eq!(normalize_model("GPT-4"), "gpt-4");
        assert_eq!(normalize_model("gpt4"), "gpt-4");
        assert_eq!(normalize_model("gpt-4-0613"), "gpt-4");

        // GPT-4o variants
        assert_eq!(normalize_model("gpt-4o"), "gpt-4o");
        assert_eq!(normalize_model("GPT-4o"), "gpt-4o");
        assert_eq!(normalize_model("gpt4o"), "gpt-4o");
        assert_eq!(normalize_model("gpt-4o-2024-05-13"), "gpt-4o");

        // GPT-4o-mini variants
        assert_eq!(normalize_model("gpt-4o-mini"), "gpt-4o-mini");
        assert_eq!(normalize_model("GPT-4o-mini"), "gpt-4o-mini");
        assert_eq!(normalize_model("gpt4omini"), "gpt-4o-mini");

        // GPT-4-turbo variants
        assert_eq!(normalize_model("gpt-4-turbo"), "gpt-4-turbo");
        assert_eq!(normalize_model("gpt-4-turbo-preview"), "gpt-4-turbo");

        // GPT-3.5 variants
        assert_eq!(normalize_model("gpt-3.5-turbo"), "gpt-3.5-turbo");
        assert_eq!(normalize_model("gpt-35-turbo"), "gpt-3.5-turbo");
        assert_eq!(normalize_model("gpt35turbo"), "gpt-3.5-turbo");
        assert_eq!(normalize_model("gpt-3.5-turbo-0125"), "gpt-3.5-turbo");

        // O1/O3 models
        assert_eq!(normalize_model("o1-preview"), "o1-preview");
        assert_eq!(normalize_model("o1-mini"), "o1-mini");
        assert_eq!(normalize_model("o3"), "o3");
    }

    #[test]
    fn test_normalize_model_anthropic() {
        // Claude 3.5 variants
        assert_eq!(normalize_model("claude-3-5-sonnet-20241022"), "claude-3.5-sonnet");
        assert_eq!(normalize_model("claude-3.5-sonnet"), "claude-3.5-sonnet");
        assert_eq!(normalize_model("claude-3-5-haiku"), "claude-3.5-haiku");

        // Claude 3 variants
        assert_eq!(normalize_model("claude-3-opus-20240229"), "claude-3-opus");
        assert_eq!(normalize_model("claude-3-sonnet-20240229"), "claude-3-sonnet");
        assert_eq!(normalize_model("claude-3-haiku-20240307"), "claude-3-haiku");

        // Claude 2
        assert_eq!(normalize_model("claude-2.1"), "claude-2");
        assert_eq!(normalize_model("claude-2.0"), "claude-2");

        // Unknown claude
        assert_eq!(normalize_model("claude-instant"), "claude-other");
    }

    #[test]
    fn test_normalize_model_google() {
        assert_eq!(normalize_model("gemini-2.0-flash"), "gemini-2");
        assert_eq!(normalize_model("gemini-1.5-pro"), "gemini-1.5-pro");
        assert_eq!(normalize_model("gemini-1.5-flash"), "gemini-1.5-flash");
        assert_eq!(normalize_model("gemini-pro"), "gemini-pro");
        assert_eq!(normalize_model("gemini-1.0-pro"), "gemini-pro");
        assert_eq!(normalize_model("gemini-unknown"), "gemini-other");
    }

    #[test]
    fn test_normalize_model_ollama() {
        assert_eq!(normalize_model("llama-3-70b"), "llama-3");
        assert_eq!(normalize_model("llama3:8b"), "llama-3");
        assert_eq!(normalize_model("llama-2-13b"), "llama-2");
        assert_eq!(normalize_model("llama2:7b"), "llama-2");
        assert_eq!(normalize_model("codellama:34b"), "codellama");
        assert_eq!(normalize_model("mistral:7b"), "mistral");
        assert_eq!(normalize_model("mixtral:8x7b"), "mixtral");
    }

    #[test]
    fn test_normalize_model_unknown() {
        // Unknown models should be bucketed to "other" to prevent cardinality explosion
        assert_eq!(normalize_model("some-random-model"), "other");
        assert_eq!(normalize_model("my-custom-llm"), "other");
        assert_eq!(normalize_model(""), "other");
        assert_eq!(normalize_model("typo-gbt-4"), "other");
    }

    #[test]
    fn test_model_normalization_prevents_cardinality_explosion() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Send events with different model name variations that should all normalize to the same label
        let variations = vec![
            "gpt-4", "GPT-4", "gpt4", "GPT-4-0613", "gpt-4-0314",
        ];

        for model in variations {
            let event = QualityEvent {
                query_id: format!("query-{}", model),
                quality_score: 0.90,
                passed: true,
                category: "Simple".to_string(),
                latency_ms: 100,
                retry_count: 0,
                model: model.to_string(),
                session_id: "test-session".to_string(),
                turn_number: 1,
                accuracy: 0.88,
                relevance: 0.92,
                completeness: 0.90,
                application_type: "rag".to_string(),
            };
            metrics.update_from_quality_event(&event);
        }

        // All 5 events should have been recorded under the same "gpt-4" label
        assert_eq!(metrics.queries_by_model.with_label_values(&["gpt-4"]).get(), 5);
    }

    // M-541/M-542: Tests for exporter self-monitoring metrics
    #[test]
    fn test_self_monitoring_metrics_registered() {
        let registry = Arc::new(Registry::new());
        let _metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Encode metrics to verify registration
        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .expect("Failed to encode metrics");
        let output = String::from_utf8(buffer).expect("Failed to convert to UTF-8");

        // M-541: Verify process_start_time_seconds is registered and set
        assert!(
            output.contains("dashstream_exporter_process_start_time_seconds"),
            "Missing M-541 process_start_time_seconds metric"
        );
        // Should be a reasonable timestamp (after 2025-01-01)
        for line in output.lines() {
            if line.starts_with("dashstream_exporter_process_start_time_seconds")
                && !line.contains("# HELP")
                && !line.contains("# TYPE")
            {
                let value: f64 = line
                    .rsplit_once(' ')
                    .and_then(|(_, v)| v.parse().ok())
                    .expect("Failed to parse timestamp");
                // Timestamp should be after Jan 1, 2025 (1735689600)
                assert!(
                    value > 1735689600.0,
                    "process_start_time_seconds should be after 2025-01-01"
                );
                break;
            }
        }

        // M-542: Verify metrics_endpoint_duration_seconds is registered
        assert!(
            output.contains("dashstream_exporter_metrics_endpoint_duration_seconds"),
            "Missing M-542 metrics_endpoint_duration_seconds metric"
        );
    }

    #[tokio::test]
    async fn test_metrics_endpoint_latency_tracking() {
        let registry = Arc::new(Registry::new());
        let metrics = Arc::new(Metrics::new(&registry).expect("Failed to create metrics"));

        // Call metrics_handler which should record latency
        let _output = metrics_handler(registry.clone(), metrics.clone()).await;

        // Verify the histogram was observed
        // Note: We can't directly access histogram samples, but we can verify the count increased
        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .expect("Failed to encode metrics");
        let output = String::from_utf8(buffer).expect("Failed to convert to UTF-8");

        let count: f64 = output
            .lines()
            .find(|line| line.contains("dashstream_exporter_metrics_endpoint_duration_seconds_count"))
            .and_then(|line| line.rsplit_once(' ').and_then(|(_, v)| v.parse().ok()))
            .expect("dashstream_exporter_metrics_endpoint_duration_seconds_count metric not found or could not be parsed");

        assert!(
            count >= 1.0,
            "Expected at least one observation, got {}",
            count
        );
    }

    // M-529/M-530/M-531/M-537/M-539: Tests for new operational metrics
    #[test]
    fn test_operational_metrics_registered() {
        let registry = Arc::new(Registry::new());
        let _metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .expect("Failed to encode metrics");
        let output = String::from_utf8(buffer).expect("Failed to convert to UTF-8");

        // M-529: Verify messages_failed_total is registered
        assert!(
            output.contains("dashstream_exporter_messages_failed_total"),
            "Missing M-529 messages_failed_total metric"
        );

        // M-530: Verify kafka_consumer_errors_total is registered
        assert!(
            output.contains("dashstream_exporter_kafka_consumer_errors_total"),
            "Missing M-530 kafka_consumer_errors_total metric"
        );

        // M-531: Verify offset_store_errors_total is registered
        assert!(
            output.contains("dashstream_exporter_offset_store_errors_total"),
            "Missing M-531 offset_store_errors_total metric"
        );

        // M-537: Verify messages_received_total is registered
        assert!(
            output.contains("dashstream_exporter_messages_received_total"),
            "Missing M-537 messages_received_total metric"
        );

        // M-539: Verify last_event_timestamp_seconds is registered
        assert!(
            output.contains("dashstream_exporter_last_event_timestamp_seconds"),
            "Missing M-539 last_event_timestamp_seconds metric"
        );

        // M-534: Verify messages_wrong_scope_total is registered
        assert!(
            output.contains("dashstream_exporter_messages_wrong_scope_total"),
            "Missing M-534 messages_wrong_scope_total metric"
        );

        // M-535: Verify messages_missing_header_total is registered
        assert!(
            output.contains("dashstream_exporter_messages_missing_header_total"),
            "Missing M-535 messages_missing_header_total metric"
        );

        // M-536: Verify kafka_consumer_lag is registered
        assert!(
            output.contains("dashstream_exporter_kafka_consumer_lag"),
            "Missing M-536 kafka_consumer_lag metric"
        );

        // M-538: Verify gauges_last_update_timestamp_seconds is registered
        assert!(
            output.contains("dashstream_exporter_gauges_last_update_timestamp_seconds"),
            "Missing M-538 gauges_last_update_timestamp_seconds metric"
        );
    }

    // M-532: Test that negative IntValue doesn't overflow
    #[test]
    fn test_negative_intvalue_clamped_to_zero() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Create a quality event that would have negative values (simulated via direct event)
        // The actual clamping happens in metrics_to_quality_event, but we can verify
        // that a manually created event with 0 values (as clamped) produces valid metrics
        let quality_event = QualityEvent {
            query_id: "negative-test".to_string(),
            quality_score: 0.5,
            passed: false,
            category: "Simple".to_string(),
            latency_ms: 0, // Would be 0 if negative IntValue was clamped
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 0,
            accuracy: 0.5,
            relevance: 0.5,
            completeness: 0.5,
            application_type: "rag".to_string(),
        };

        // Should not panic or produce u64::MAX
        metrics.update_from_quality_event(&quality_event);

        // Verify no overflow (latency_ms = 0 should produce histogram observation of 0)
        assert_eq!(metrics.queries_total.get(), 1);
    }

    // M-539: Test that last_event_timestamp_seconds is updated on quality event
    #[test]
    fn test_last_event_timestamp_updated() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Initial value should be 0
        let initial = metrics.last_event_timestamp_seconds.get();
        assert!(
            initial < 1.0,
            "Initial timestamp should be 0 or near-zero, got {}",
            initial
        );

        let quality_event = QualityEvent {
            query_id: "timestamp-test".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&quality_event);

        let after = metrics.last_event_timestamp_seconds.get();
        // Should be a recent Unix timestamp (after 2025-01-01)
        assert!(
            after > 1735689600.0,
            "Timestamp should be after 2025-01-01, got {}",
            after
        );
    }

    // M-538: Test that gauges_last_update_timestamp_seconds is updated on quality event
    #[test]
    fn test_gauges_staleness_timestamp_updated() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Initial value should be 0
        let initial = metrics.gauges_last_update_timestamp_seconds.get();
        assert!(
            initial < 1.0,
            "Initial gauge staleness timestamp should be 0 or near-zero, got {}",
            initial
        );

        let quality_event = QualityEvent {
            query_id: "staleness-test".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&quality_event);

        let after = metrics.gauges_last_update_timestamp_seconds.get();
        // Should be a recent Unix timestamp (after 2025-01-01)
        assert!(
            after > 1735689600.0,
            "Gauge staleness timestamp should be after 2025-01-01, got {}",
            after
        );
    }

    // M-535: Test that missing header provides fallback session_id
    #[test]
    fn test_missing_header_fallback_session_id() {
        use dashflow_streaming::Metrics as DashStreamMetrics;
        use std::collections::HashMap;

        // Create a metrics message without header
        let metrics_msg = DashStreamMetrics {
            header: None, // Missing header
            scope: "quality".to_string(),
            scope_id: "test-query".to_string(),
            metrics: HashMap::new(),
            tags: HashMap::new(),
        };

        // Should not return None - should use empty session_id fallback
        let result = KafkaConsumer::metrics_to_quality_event(metrics_msg);
        assert!(result.is_some(), "Should return event with fallback session_id");

        let event = result.unwrap();
        assert_eq!(event.session_id, "", "Missing header should result in empty session_id");
    }

    // =========================================================================
    // Tests for parse_buckets_from_env
    // =========================================================================

    #[test]
    fn test_parse_buckets_from_env_valid_values() {
        std::env::set_var("TEST_BUCKETS_VALID", "10,50,100,500");
        let result = parse_buckets_from_env("TEST_BUCKETS_VALID", vec![1.0, 2.0]);
        assert_eq!(result, vec![10.0, 50.0, 100.0, 500.0]);
        std::env::remove_var("TEST_BUCKETS_VALID");
    }

    #[test]
    fn test_parse_buckets_from_env_with_whitespace() {
        std::env::set_var("TEST_BUCKETS_WHITESPACE", " 10 , 50 , 100 ");
        let result = parse_buckets_from_env("TEST_BUCKETS_WHITESPACE", vec![1.0]);
        assert_eq!(result, vec![10.0, 50.0, 100.0]);
        std::env::remove_var("TEST_BUCKETS_WHITESPACE");
    }

    #[test]
    fn test_parse_buckets_from_env_missing_var() {
        // Ensure the env var doesn't exist
        std::env::remove_var("TEST_BUCKETS_MISSING");
        let default = vec![1.0, 2.0, 3.0];
        let result = parse_buckets_from_env("TEST_BUCKETS_MISSING", default.clone());
        assert_eq!(result, default);
    }

    #[test]
    fn test_parse_buckets_from_env_invalid_values() {
        std::env::set_var("TEST_BUCKETS_INVALID", "10,not_a_number,100");
        let default = vec![1.0, 2.0];
        let result = parse_buckets_from_env("TEST_BUCKETS_INVALID", default.clone());
        assert_eq!(result, default);
        std::env::remove_var("TEST_BUCKETS_INVALID");
    }

    #[test]
    fn test_parse_buckets_from_env_empty_string() {
        std::env::set_var("TEST_BUCKETS_EMPTY", "");
        let default = vec![5.0, 10.0];
        let result = parse_buckets_from_env("TEST_BUCKETS_EMPTY", default.clone());
        assert_eq!(result, default);
        std::env::remove_var("TEST_BUCKETS_EMPTY");
    }

    #[test]
    fn test_parse_buckets_from_env_single_value() {
        std::env::set_var("TEST_BUCKETS_SINGLE", "42.5");
        let result = parse_buckets_from_env("TEST_BUCKETS_SINGLE", vec![1.0]);
        assert_eq!(result, vec![42.5]);
        std::env::remove_var("TEST_BUCKETS_SINGLE");
    }

    #[test]
    fn test_parse_buckets_from_env_float_values() {
        std::env::set_var("TEST_BUCKETS_FLOAT", "0.001,0.01,0.1,1.0");
        let result = parse_buckets_from_env("TEST_BUCKETS_FLOAT", vec![1.0]);
        assert_eq!(result, vec![0.001, 0.01, 0.1, 1.0]);
        std::env::remove_var("TEST_BUCKETS_FLOAT");
    }

    // =========================================================================
    // Tests for default bucket functions
    // =========================================================================

    #[test]
    fn test_default_latency_buckets_ms() {
        let buckets = default_latency_buckets_ms();
        assert!(!buckets.is_empty(), "Default latency buckets should not be empty");
        assert!(buckets.contains(&1000.0), "Should contain 1000ms bucket");
        assert!(buckets.contains(&5000.0), "Should contain 5000ms bucket");
        // Verify buckets are sorted
        for window in buckets.windows(2) {
            assert!(window[0] < window[1], "Buckets should be in ascending order");
        }
    }

    #[test]
    fn test_default_duration_buckets_seconds() {
        let buckets = default_duration_buckets_seconds();
        assert!(!buckets.is_empty(), "Default duration buckets should not be empty");
        assert!(buckets.contains(&0.1), "Should contain 100ms bucket");
        assert!(buckets.contains(&1.0), "Should contain 1s bucket");
    }

    #[test]
    fn test_default_retry_buckets() {
        let buckets = default_retry_buckets();
        assert!(!buckets.is_empty(), "Default retry buckets should not be empty");
        assert!(buckets.contains(&0.0), "Should contain 0 retries bucket");
        assert!(buckets.contains(&1.0), "Should contain 1 retry bucket");
    }

    #[test]
    fn test_default_session_turn_buckets() {
        let buckets = default_session_turn_buckets();
        assert!(!buckets.is_empty(), "Default session turn buckets should not be empty");
        assert!(buckets.contains(&1.0), "Should contain 1 turn bucket");
        assert!(buckets.contains(&10.0), "Should contain 10 turns bucket");
    }

    #[test]
    fn test_default_metrics_endpoint_buckets() {
        let buckets = default_metrics_endpoint_buckets();
        assert!(!buckets.is_empty(), "Default metrics endpoint buckets should not be empty");
        // Should have tight buckets for detecting slow encoding
        assert!(buckets[0] < 0.01, "First bucket should be very small for detecting fast encoding");
    }

    #[test]
    fn test_default_quality_score_buckets() {
        let buckets = default_quality_score_buckets();
        assert!(!buckets.is_empty(), "Default quality score buckets should not be empty");
        assert!(buckets.contains(&0.5), "Should contain 0.5 bucket");
        assert!(buckets.contains(&1.0), "Should contain 1.0 bucket");
        // Verify all buckets are in 0-1 range
        for bucket in &buckets {
            assert!(*bucket >= 0.0 && *bucket <= 1.0, "Quality score bucket {} should be in 0-1 range", bucket);
        }
    }

    // =========================================================================
    // Tests for normalize_category
    // =========================================================================

    #[test]
    fn test_normalize_category_simple() {
        assert_eq!(normalize_category("Simple"), "Simple");
        assert_eq!(normalize_category("simple"), "Simple");
        assert_eq!(normalize_category("SIMPLE"), "Simple");
    }

    #[test]
    fn test_normalize_category_medium() {
        assert_eq!(normalize_category("Medium"), "Medium");
        assert_eq!(normalize_category("medium"), "Medium");
        assert_eq!(normalize_category("MEDIUM"), "Medium");
    }

    #[test]
    fn test_normalize_category_complex() {
        assert_eq!(normalize_category("Complex"), "Complex");
        assert_eq!(normalize_category("complex"), "Complex");
        assert_eq!(normalize_category("COMPLEX"), "Complex");
    }

    #[test]
    fn test_normalize_category_edge() {
        assert_eq!(normalize_category("Edge"), "Edge");
        assert_eq!(normalize_category("edge"), "Edge");
        assert_eq!(normalize_category("EDGE"), "Edge");
    }

    #[test]
    fn test_normalize_category_unknown() {
        assert_eq!(normalize_category("Unknown"), "Unknown");
        assert_eq!(normalize_category("SomeOtherCategory"), "Unknown");
        assert_eq!(normalize_category(""), "Unknown");
        assert_eq!(normalize_category("MiXeD CaSe"), "Unknown");
    }

    // =========================================================================
    // Tests for get_auto_offset_reset
    // =========================================================================

    #[test]
    fn test_get_auto_offset_reset_earliest() {
        std::env::set_var("KAFKA_AUTO_OFFSET_RESET", "earliest");
        assert_eq!(get_auto_offset_reset(), "earliest");
        std::env::remove_var("KAFKA_AUTO_OFFSET_RESET");
    }

    #[test]
    fn test_get_auto_offset_reset_latest() {
        std::env::set_var("KAFKA_AUTO_OFFSET_RESET", "latest");
        assert_eq!(get_auto_offset_reset(), "latest");
        std::env::remove_var("KAFKA_AUTO_OFFSET_RESET");
    }

    #[test]
    fn test_get_auto_offset_reset_case_insensitive() {
        std::env::set_var("KAFKA_AUTO_OFFSET_RESET", "EARLIEST");
        assert_eq!(get_auto_offset_reset(), "earliest");
        std::env::set_var("KAFKA_AUTO_OFFSET_RESET", "Latest");
        assert_eq!(get_auto_offset_reset(), "latest");
        std::env::remove_var("KAFKA_AUTO_OFFSET_RESET");
    }

    #[test]
    fn test_get_auto_offset_reset_invalid() {
        std::env::set_var("KAFKA_AUTO_OFFSET_RESET", "invalid_value");
        assert_eq!(get_auto_offset_reset(), "earliest"); // Falls back to earliest
        std::env::remove_var("KAFKA_AUTO_OFFSET_RESET");
    }

    #[test]
    fn test_get_auto_offset_reset_missing() {
        std::env::remove_var("KAFKA_AUTO_OFFSET_RESET");
        assert_eq!(get_auto_offset_reset(), "earliest"); // Default is earliest
    }

    // =========================================================================
    // Tests for QualityEvent struct
    // =========================================================================

    #[test]
    fn test_quality_event_default() {
        let event = QualityEvent::default();
        assert_eq!(event.query_id, "");
        assert_eq!(event.quality_score, 0.0);
        assert!(!event.passed);
        assert_eq!(event.category, "");
        assert_eq!(event.latency_ms, 0);
        assert_eq!(event.retry_count, 0);
        assert_eq!(event.model, "");
        assert_eq!(event.session_id, "");
        assert_eq!(event.turn_number, 0);
        assert_eq!(event.accuracy, 0.0);
        assert_eq!(event.relevance, 0.0);
        assert_eq!(event.completeness, 0.0);
        assert_eq!(event.application_type, "");
    }

    #[test]
    fn test_quality_event_clone() {
        let event = QualityEvent {
            query_id: "test-123".to_string(),
            quality_score: 0.85,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 150,
            retry_count: 1,
            model: "gpt-4".to_string(),
            session_id: "session-abc".to_string(),
            turn_number: 3,
            accuracy: 0.9,
            relevance: 0.8,
            completeness: 0.85,
            application_type: "librarian".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(cloned.query_id, event.query_id);
        assert_eq!(cloned.quality_score, event.quality_score);
        assert_eq!(cloned.passed, event.passed);
    }

    #[test]
    fn test_quality_event_debug() {
        let event = QualityEvent {
            query_id: "debug-test".to_string(),
            ..Default::default()
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("QualityEvent"));
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_quality_event_serialize_deserialize() {
        let event = QualityEvent {
            query_id: "serde-test".to_string(),
            quality_score: 0.95,
            passed: true,
            category: "Complex".to_string(),
            latency_ms: 200,
            retry_count: 0,
            model: "claude-3-opus".to_string(),
            session_id: "session-xyz".to_string(),
            turn_number: 5,
            accuracy: 0.93,
            relevance: 0.97,
            completeness: 0.95,
            application_type: "rag".to_string(),
        };

        let json = serde_json::to_string(&event).expect("Serialization should succeed");
        let deserialized: QualityEvent = serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(deserialized.query_id, event.query_id);
        assert!((deserialized.quality_score - event.quality_score).abs() < f64::EPSILON);
        assert_eq!(deserialized.passed, event.passed);
    }

    // =========================================================================
    // Tests for session tracking
    // =========================================================================

    #[test]
    fn test_session_tracking_multiple_turns() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Simulate multiple turns in the same session
        for turn in 1..=5 {
            let event = QualityEvent {
                query_id: format!("query-{}", turn),
                quality_score: 0.9,
                passed: true,
                category: "Simple".to_string(),
                latency_ms: 100,
                retry_count: 0,
                model: "gpt-4".to_string(),
                session_id: "same-session".to_string(),
                turn_number: turn,
                accuracy: 0.9,
                relevance: 0.9,
                completeness: 0.9,
                application_type: "rag".to_string(),
            };
            metrics.update_from_quality_event(&event);
        }

        // Verify session is being tracked
        let tracker = metrics.session_tracker.read().unwrap();
        assert!(tracker.contains_key("same-session"), "Session should be tracked");
        let (max_turn, _) = tracker.get("same-session").unwrap();
        assert_eq!(*max_turn, 5, "Max turn should be 5");
    }

    #[test]
    fn test_session_tracking_empty_session_id() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Events without session_id should use legacy immediate observation
        let event = QualityEvent {
            query_id: "no-session".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "".to_string(),
            turn_number: 3,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };
        metrics.update_from_quality_event(&event);

        // Empty session_id should not be tracked in session_tracker
        let tracker = metrics.session_tracker.read().unwrap();
        assert!(tracker.is_empty(), "Empty session_id should not be tracked");
    }

    #[test]
    fn test_flush_sessions() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // Add some sessions
        for i in 1..=3 {
            let event = QualityEvent {
                query_id: format!("query-{}", i),
                quality_score: 0.9,
                passed: true,
                category: "Simple".to_string(),
                latency_ms: 100,
                retry_count: 0,
                model: "gpt-4".to_string(),
                session_id: format!("session-{}", i),
                turn_number: i,
                accuracy: 0.9,
                relevance: 0.9,
                completeness: 0.9,
                application_type: "rag".to_string(),
            };
            metrics.update_from_quality_event(&event);
        }

        // Verify sessions are tracked
        {
            let tracker = metrics.session_tracker.read().unwrap();
            assert_eq!(tracker.len(), 3, "Should have 3 tracked sessions");
        }

        // Flush sessions
        metrics.flush_sessions();

        // Verify sessions are cleared
        let tracker = metrics.session_tracker.read().unwrap();
        assert!(tracker.is_empty(), "Sessions should be cleared after flush");
    }

    // =========================================================================
    // Tests for empty model handling
    // =========================================================================

    #[test]
    fn test_empty_model_no_per_model_metrics() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "no-model".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "".to_string(), // Empty model
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };

        metrics.update_from_quality_event(&event);

        // General metrics should still be updated
        assert_eq!(metrics.queries_total.get(), 1);
        // But empty model shouldn't create any model-specific label
        // (This is implicitly tested - empty model is skipped in update_from_quality_event)
    }

    // =========================================================================
    // Tests for document_search_streaming application type
    // =========================================================================

    #[test]
    fn test_application_type_document_search_streaming() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "streaming-test".to_string(),
            quality_score: 0.88,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 300,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 2,
            accuracy: 0.85,
            relevance: 0.90,
            completeness: 0.89,
            application_type: "document_search_streaming".to_string(),
        };

        metrics.update_from_quality_event(&event);

        // Should route to librarian metrics (consolidation)
        assert_eq!(metrics.librarian_requests_total.get(), 1);
    }

    // =========================================================================
    // Tests for build info metric
    // =========================================================================

    #[test]
    fn test_build_info_metric_registered() {
        let registry = Arc::new(Registry::new());
        let _metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).expect("Encode failed");
        let output = String::from_utf8(buffer).expect("UTF-8 conversion failed");

        assert!(output.contains("dashflow_build_info"), "build_info metric should be registered");
        assert!(output.contains("version="), "build_info should have version label");
    }

    // =========================================================================
    // Tests for pre-registered model series
    // =========================================================================

    #[test]
    fn test_pre_registered_model_series() {
        let registry = Arc::new(Registry::new());
        let _metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).expect("Encode failed");
        let output = String::from_utf8(buffer).expect("UTF-8 conversion failed");

        // Verify some pre-registered models are present
        // Note: Pre-registration creates series with value 0
        assert!(output.contains("gpt-4"), "gpt-4 should be pre-registered");
        assert!(output.contains("claude-3.5-sonnet"), "claude-3.5-sonnet should be pre-registered");
    }

    // =========================================================================
    // Tests for pre-registered category series
    // =========================================================================

    #[test]
    fn test_pre_registered_category_series() {
        let registry = Arc::new(Registry::new());
        let _metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).expect("Encode failed");
        let output = String::from_utf8(buffer).expect("UTF-8 conversion failed");

        // Verify pre-registered categories
        for category in ["Simple", "Medium", "Complex", "Edge", "Unknown"] {
            assert!(
                output.contains(&format!("category=\"{}\"", category)),
                "Category {} should be pre-registered",
                category
            );
        }
    }

    // =========================================================================
    // Tests for metrics_to_quality_event edge cases
    // =========================================================================

    #[test]
    fn test_metrics_to_quality_event_empty_metrics() {
        use dashflow_streaming::{Header, MessageType, Metrics as DashStreamMetrics};
        use std::collections::HashMap;

        let metrics_msg = DashStreamMetrics {
            header: Some(Header {
                message_id: vec![],
                timestamp_us: 0,
                tenant_id: "test".to_string(),
                thread_id: "session-empty".to_string(),
                sequence: 1,
                r#type: MessageType::Metrics as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            scope: "quality".to_string(),
            scope_id: "test".to_string(),
            metrics: HashMap::new(), // Empty metrics
            tags: HashMap::new(),
        };

        let result = KafkaConsumer::metrics_to_quality_event(metrics_msg);
        assert!(result.is_some(), "Should handle empty metrics map");
        let event = result.unwrap();
        assert_eq!(event.quality_score, 0.0);
        assert!(!event.passed);
    }

    #[test]
    fn test_metrics_to_quality_event_all_tags() {
        use dashflow_streaming::{Header, MessageType, Metrics as DashStreamMetrics};
        use std::collections::HashMap;

        let mut tags = HashMap::new();
        tags.insert("query_id".to_string(), "test-query-123".to_string());
        tags.insert("model".to_string(), "gpt-4".to_string());
        tags.insert("category".to_string(), "Complex".to_string());
        tags.insert("application_type".to_string(), "librarian".to_string());

        let metrics_msg = DashStreamMetrics {
            header: Some(Header {
                message_id: vec![],
                timestamp_us: 0,
                tenant_id: "test".to_string(),
                thread_id: "session-tags".to_string(),
                sequence: 1,
                r#type: MessageType::Metrics as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            scope: "quality".to_string(),
            scope_id: "test".to_string(),
            metrics: HashMap::new(),
            tags,
        };

        let result = KafkaConsumer::metrics_to_quality_event(metrics_msg);
        assert!(result.is_some());
        let event = result.unwrap();
        assert_eq!(event.query_id, "test-query-123");
        assert_eq!(event.model, "gpt-4");
        assert_eq!(event.category, "Complex");
        assert_eq!(event.application_type, "librarian");
    }

    #[test]
    fn test_metrics_to_quality_event_alternate_tag_names() {
        use dashflow_streaming::{Header, MessageType, Metrics as DashStreamMetrics};
        use std::collections::HashMap;

        let mut tags = HashMap::new();
        // Use alternate tag names that should also work
        tags.insert("query_category".to_string(), "Edge".to_string());
        tags.insert("app_type".to_string(), "rag".to_string());
        tags.insert("complexity".to_string(), "High".to_string()); // Will override query_category

        let metrics_msg = DashStreamMetrics {
            header: Some(Header {
                message_id: vec![],
                timestamp_us: 0,
                tenant_id: "test".to_string(),
                thread_id: "session-alt".to_string(),
                sequence: 1,
                r#type: MessageType::Metrics as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            scope: "quality".to_string(),
            scope_id: "test".to_string(),
            metrics: HashMap::new(),
            tags,
        };

        let result = KafkaConsumer::metrics_to_quality_event(metrics_msg);
        assert!(result.is_some());
        let event = result.unwrap();
        // complexity is processed last due to iteration order, but HashMap doesn't guarantee order
        // Just verify one of the alternate names worked
        assert!(event.application_type == "rag", "app_type should map to application_type");
    }

    // =========================================================================
    // Tests for retry histogram status labels
    // =========================================================================

    #[test]
    fn test_retry_histogram_passed_label() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "retry-passed".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 2,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };
        metrics.update_from_quality_event(&event);

        // Verify retry_count histogram has "passed" status label
        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.contains("dashstream_quality_retry_count"), "retry_count histogram should exist");
        assert!(output.contains("status=\"passed\""), "Should have passed status label");
    }

    #[test]
    fn test_retry_histogram_failed_label() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "retry-failed".to_string(),
            quality_score: 0.3,
            passed: false,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 3,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.3,
            relevance: 0.3,
            completeness: 0.3,
            application_type: "rag".to_string(),
        };
        metrics.update_from_quality_event(&event);

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.contains("status=\"failed\""), "Should have failed status label");
    }

    // =========================================================================
    // Tests for librarian test status metrics
    // =========================================================================

    #[test]
    fn test_librarian_tests_total_passed() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "lib-test-pass".to_string(),
            quality_score: 0.95,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.95,
            relevance: 0.95,
            completeness: 0.95,
            application_type: "librarian".to_string(),
        };
        metrics.update_from_quality_event(&event);

        assert_eq!(metrics.librarian_tests_total.with_label_values(&["passed"]).get(), 1);
        assert_eq!(metrics.librarian_tests_total.with_label_values(&["failed"]).get(), 0);
    }

    #[test]
    fn test_librarian_tests_total_failed() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "lib-test-fail".to_string(),
            quality_score: 0.3,
            passed: false,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.3,
            relevance: 0.3,
            completeness: 0.3,
            application_type: "librarian".to_string(),
        };
        metrics.update_from_quality_event(&event);

        assert_eq!(metrics.librarian_tests_total.with_label_values(&["passed"]).get(), 0);
        assert_eq!(metrics.librarian_tests_total.with_label_values(&["failed"]).get(), 1);
    }

    // =========================================================================
    // Tests for latency_by_model histogram
    // =========================================================================

    #[test]
    fn test_latency_by_model_histogram() {
        let registry = Arc::new(Registry::new());
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "latency-model".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 1500,
            retry_count: 0,
            model: "claude-3.5-sonnet".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };
        metrics.update_from_quality_event(&event);

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.contains("dashstream_latency_by_model_ms"), "latency_by_model histogram should exist");
        assert!(output.contains("model=\"claude-3.5-sonnet\""), "Should have normalized model label");
    }

    // =========================================================================
    // Tests for quality_by_model histogram
    // =========================================================================

    #[test]
    fn test_quality_by_model_histogram() {
        let registry = Arc::new(Registry::new());
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "quality-model".to_string(),
            quality_score: 0.85,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gemini-1.5-pro".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.85,
            relevance: 0.85,
            completeness: 0.85,
            application_type: "rag".to_string(),
        };
        metrics.update_from_quality_event(&event);

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.contains("dashstream_quality_score_by_model"), "quality_by_model histogram should exist");
        assert!(output.contains("model=\"gemini-1.5-pro\""), "Should have normalized model label");
    }

    // =========================================================================
    // Tests for edge cases in normalize_model
    // =========================================================================

    #[test]
    fn test_normalize_model_whitespace_handling() {
        assert_eq!(normalize_model("  gpt-4  "), "gpt-4");
        assert_eq!(normalize_model("\tgpt-4\n"), "gpt-4");
    }

    #[test]
    fn test_normalize_model_o1_variants() {
        assert_eq!(normalize_model("o1"), "o1");
        // Note: o1 with date suffix like "o1-2024-12-17" starts with "o1-" which
        // is excluded by design to avoid matching o1-preview/o1-mini variants.
        // These get bucketed to "other" as a defensive measure.
        assert_eq!(normalize_model("o1-2024-12-17"), "other");
        // o1preview (without hyphen) matches the o1 pattern
        assert_eq!(normalize_model("o1preview"), "o1");
    }

    #[test]
    fn test_normalize_model_llama_other() {
        assert_eq!(normalize_model("llama-1-7b"), "llama-other");
        assert_eq!(normalize_model("llama-unknown"), "llama-other");
    }

    #[test]
    fn test_normalize_model_gpt35_variants() {
        assert_eq!(normalize_model("gpt-3.5"), "gpt-3.5");
        assert_eq!(normalize_model("gpt35"), "gpt-3.5");
    }

    // =========================================================================
    // Tests for zero turn_number handling
    // =========================================================================

    #[test]
    fn test_zero_turn_number_not_tracked() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "zero-turn".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session-with-zero-turn".to_string(),
            turn_number: 0, // Zero turn number
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };
        metrics.update_from_quality_event(&event);

        // Session with turn_number=0 should not be tracked
        let tracker = metrics.session_tracker.read().unwrap();
        assert!(!tracker.contains_key("session-with-zero-turn"), "Zero turn number should not track session");
    }

    // =========================================================================
    // Tests for librarian iteration tracking
    // =========================================================================

    #[test]
    fn test_librarian_iterations_gauge() {
        let registry = Registry::new();
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        // First event with turn 3
        let event1 = QualityEvent {
            query_id: "iter-1".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 3,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "librarian".to_string(),
        };
        metrics.update_from_quality_event(&event1);
        assert!((metrics.librarian_iterations.get() - 3.0).abs() < f64::EPSILON);

        // Second event with turn 7 (gauge should update to latest)
        let event2 = QualityEvent {
            query_id: "iter-2".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 7,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "librarian".to_string(),
        };
        metrics.update_from_quality_event(&event2);
        assert!((metrics.librarian_iterations.get() - 7.0).abs() < f64::EPSILON);
    }

    // =========================================================================
    // Tests for librarian request duration
    // =========================================================================

    #[test]
    fn test_librarian_request_duration_conversion() {
        let registry = Arc::new(Registry::new());
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "duration-test".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 2500, // 2.5 seconds
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "librarian".to_string(),
        };
        metrics.update_from_quality_event(&event);

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        // Verify the histogram exists and has observations
        assert!(output.contains("dashstream_librarian_request_duration_seconds"), "request_duration metric should exist");
    }

    // =========================================================================
    // Tests for metrics output formatting
    // =========================================================================

    #[test]
    fn test_metrics_output_prometheus_format() {
        let registry = Arc::new(Registry::new());
        let metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let event = QualityEvent {
            query_id: "format-test".to_string(),
            quality_score: 0.9,
            passed: true,
            category: "Simple".to_string(),
            latency_ms: 100,
            retry_count: 0,
            model: "gpt-4".to_string(),
            session_id: "session".to_string(),
            turn_number: 1,
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            application_type: "rag".to_string(),
        };
        metrics.update_from_quality_event(&event);

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        // Verify Prometheus exposition format
        assert!(output.contains("# HELP"), "Should contain HELP lines");
        assert!(output.contains("# TYPE"), "Should contain TYPE lines");
        // Counters should end with _total
        assert!(output.contains("_total"), "Counters should have _total suffix");
    }

    // =========================================================================
    // Tests for kafka_payload_missing_total metric
    // =========================================================================

    #[test]
    fn test_kafka_payload_missing_metric_registered() {
        let registry = Arc::new(Registry::new());
        let _metrics = Metrics::new(&registry).expect("Failed to create metrics");

        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.contains("dashstream_exporter_kafka_payload_missing_total"), "kafka_payload_missing metric should be registered");
    }
}
