//! Advanced observability module for DashFlow
//!
//! This module provides extensible observability features including:
//! - Custom metrics API for user-defined metrics
//! - OpenTelemetry integration with W3C trace context
//! - Distributed tracing support
//! - `LangSmith` integration
//! - Log aggregation utilities
//!
//! # Examples
//!
//! ## Custom Metrics
//!
//! ```rust
//! use dashflow::core::observability::CustomMetricsRegistry;
//!
//! // Create a custom counter
//! let registry = CustomMetricsRegistry::global();
//! registry.register_counter(
//!     "my_custom_events_total",
//!     "Total number of custom events",
//!     &["event_type", "user_id"]
//! ).unwrap();
//!
//! // Record a custom metric
//! registry.increment_counter(
//!     "my_custom_events_total",
//!     &[("event_type", "login"), ("user_id", "123")]
//! ).unwrap();
//! ```
//!
//! ## OpenTelemetry Integration
//!
//! ```no_run
//! # // OpenTelemetry integration is a planned feature
//! # // This example shows the intended API design
//! # use dashflow::core::observability;
//! #
//! # // Configure OpenTelemetry with W3C trace context (planned)
//! # // let config = TraceConfig::default()
//! # //     .with_service_name("dashflow-app")
//! # //     .with_jaeger_endpoint("http://jaeger:14268/api/traces");
//! # //
//! # // let tracer = TraceContext::init(config).unwrap();
//! ```

use crate::core::config_loader::env_vars::{
    env_string, DASHFLOW_INSTANCE_ID, DASHFLOW_METRICS_REDACT,
};
use prometheus::{
    CounterVec, Encoder, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, OnceLock, RwLock};
use tracing::debug;

// ========== Instance ID for Multi-Instance Deployments (M-300) ==========

/// Global instance identifier for multi-instance deployments.
///
/// This ID is used as a const label (`instance_id`) on all metrics, enabling
/// Grafana dashboards to break down metrics by instance.
///
/// # Environment Variable
///
/// Set `DASHFLOW_INSTANCE_ID` to provide a custom instance ID:
/// - Pod name in Kubernetes: `DASHFLOW_INSTANCE_ID=$(POD_NAME)`
/// - Container ID: `DASHFLOW_INSTANCE_ID=$(hostname)`
/// - Custom identifier: `DASHFLOW_INSTANCE_ID=worker-1`
///
/// If not set, a random UUID is generated on first access.
///
/// # Example
///
/// ```bash
/// # Set a custom instance ID
/// export DASHFLOW_INSTANCE_ID="worker-east-1"
///
/// # Or use pod name in Kubernetes
/// export DASHFLOW_INSTANCE_ID=$(POD_NAME)
/// ```
pub static INSTANCE_ID: LazyLock<String> = LazyLock::new(|| {
    env_string(DASHFLOW_INSTANCE_ID).unwrap_or_else(|| {
        // Generate a short random ID (first 8 chars of UUID) for easier readability
        let uuid = uuid::Uuid::new_v4().to_string();
        uuid[..8].to_string()
    })
});

/// Get the current instance ID.
///
/// This is the value used as the `instance_id` const label on all metrics.
#[must_use]
pub fn instance_id() -> &'static str {
    &INSTANCE_ID
}

/// Global metrics registry for custom user-defined metrics
pub static CUSTOM_REGISTRY: OnceLock<CustomMetricsRegistry> = OnceLock::new();

// ========== Metrics Redaction (M-223) ==========

/// Regex pattern to match label values in Prometheus text format.
// SAFETY: Regex literal is hardcoded and compile-time valid
#[allow(clippy::expect_used)]
static LABEL_VALUE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(\w+)="((?:[^"\\]|\\.)*)""#).expect("Invalid regex for label values")
});

/// Secret patterns for metrics redaction
// SAFETY: All regex literals are hardcoded and compile-time valid
#[allow(clippy::unwrap_used)]
static SECRET_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap(), "[OPENAI_KEY]"),
        (
            Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").unwrap(),
            "[ANTHROPIC_KEY]",
        ),
        (
            Regex::new(r"(?:AKIA|ABIA|ACCA|ASIA)[A-Z0-9]{16}").unwrap(),
            "[AWS_KEY]",
        ),
        (
            Regex::new(r"(?:ghp|gho|ghu|ghs|ghr)_[a-zA-Z0-9]{36,}").unwrap(),
            "[GITHUB_TOKEN]",
        ),
        (
            Regex::new(r"[Bb]earer\s+[a-zA-Z0-9_.-]{20,}").unwrap(),
            "Bearer [TOKEN]",
        ),
        (
            Regex::new(r"://[^:]+:([^@]{8,})@").unwrap(),
            "://[CREDENTIALS]@",
        ),
        (
            Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
            "[EMAIL]",
        ),
        (
            Regex::new(r"eyJ[a-zA-Z0-9_-]{20,}\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+").unwrap(),
            "[JWT_TOKEN]",
        ),
    ]
});

/// Check if metrics redaction is enabled via environment variable.
fn is_metrics_redaction_enabled() -> bool {
    match env_string(DASHFLOW_METRICS_REDACT) {
        Some(val) => !matches!(val.to_lowercase().as_str(), "false" | "0" | "no" | "off"),
        None => true, // Default ON for security
    }
}

/// Redact sensitive data from a string using built-in patterns.
fn redact_string(text: &str) -> String {
    let mut result = text.to_string();
    for (pattern, replacement) in SECRET_PATTERNS.iter() {
        result = pattern.replace_all(&result, *replacement).to_string();
    }
    result
}

/// Redact sensitive data from Prometheus text format metrics.
fn redact_prometheus_text(metrics_text: &str) -> String {
    let mut result = String::with_capacity(metrics_text.len());

    for line in metrics_text.lines() {
        if line.starts_with('#') {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if let (Some(start), Some(end)) = (line.find('{'), line.find('}')) {
            let before_labels = &line[..=start];
            let labels_content = &line[start + 1..end];
            let after_labels = &line[end..];

            let redacted_labels =
                LABEL_VALUE_PATTERN.replace_all(labels_content, |caps: &regex::Captures| {
                    let label_name = &caps[1];
                    let value = &caps[2];
                    format!("{}=\"{}\"", label_name, redact_string(value))
                });

            result.push_str(before_labels);
            result.push_str(&redacted_labels);
            result.push_str(after_labels);
            result.push('\n');
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    if !metrics_text.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Custom metrics registry that allows users to register and use their own metrics
#[derive(Debug, Clone)]
pub struct CustomMetricsRegistry {
    registry: Arc<Registry>,
    counters: Arc<RwLock<HashMap<String, CounterVec>>>,
    gauges: Arc<RwLock<HashMap<String, GaugeVec>>>,
    histograms: Arc<RwLock<HashMap<String, HistogramVec>>>,
}

impl CustomMetricsRegistry {
    /// Create a new custom metrics registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Registry::new()),
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the global metrics registry
    pub fn global() -> &'static Self {
        CUSTOM_REGISTRY.get_or_init(CustomMetricsRegistry::new)
    }

    /// Register a new counter metric
    ///
    /// # Arguments
    ///
    /// * `name` - Metric name (must be unique)
    /// * `help` - Help text describing the metric
    /// * `labels` - Array of label names
    ///
    /// # Instance ID Label (M-300)
    ///
    /// All metrics automatically include an `instance_id` const label for
    /// multi-instance deployment support. This enables Grafana dashboards to
    /// break down metrics by instance. Set `DASHFLOW_INSTANCE_ID` env var to
    /// customize the instance ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow::core::observability::CustomMetricsRegistry;
    ///
    /// let registry = CustomMetricsRegistry::global();
    /// registry.register_counter(
    ///     "llm_calls_total",
    ///     "Total number of LLM API calls",
    ///     &["provider", "model"]
    /// ).unwrap();
    /// // Metric will have instance_id="<value>" const label
    /// ```
    pub fn register_counter(&self, name: &str, help: &str, labels: &[&str]) -> Result<(), String> {
        let opts = Opts::new(name, help).const_label("instance_id", instance_id());
        let counter = CounterVec::new(opts, labels).map_err(|e| e.to_string())?;

        self.registry
            .register(Box::new(counter.clone()))
            .map_err(|e| e.to_string())?;

        self.counters
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), counter);

        Ok(())
    }

    /// Register a new gauge metric
    ///
    /// # Arguments
    ///
    /// * `name` - Metric name (must be unique)
    /// * `help` - Help text describing the metric
    /// * `labels` - Array of label names
    ///
    /// # Instance ID Label (M-300)
    ///
    /// All metrics automatically include an `instance_id` const label for
    /// multi-instance deployment support. This enables Grafana dashboards to
    /// break down metrics by instance. Set `DASHFLOW_INSTANCE_ID` env var to
    /// customize the instance ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow::core::observability::CustomMetricsRegistry;
    ///
    /// let registry = CustomMetricsRegistry::global();
    /// registry.register_gauge(
    ///     "active_sessions",
    ///     "Number of active user sessions",
    ///     &["session_type"]
    /// ).unwrap();
    /// // Metric will have instance_id="<value>" const label
    /// ```
    pub fn register_gauge(&self, name: &str, help: &str, labels: &[&str]) -> Result<(), String> {
        let opts = Opts::new(name, help).const_label("instance_id", instance_id());
        let gauge = GaugeVec::new(opts, labels).map_err(|e| e.to_string())?;

        self.registry
            .register(Box::new(gauge.clone()))
            .map_err(|e| e.to_string())?;

        self.gauges
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), gauge);

        Ok(())
    }

    /// Register a new histogram metric
    ///
    /// # Arguments
    ///
    /// * `name` - Metric name (must be unique)
    /// * `help` - Help text describing the metric
    /// * `labels` - Array of label names
    /// * `buckets` - Optional custom buckets (defaults to exponential buckets)
    ///
    /// # Instance ID Label (M-300)
    ///
    /// All metrics automatically include an `instance_id` const label for
    /// multi-instance deployment support. This enables Grafana dashboards to
    /// break down metrics by instance. Set `DASHFLOW_INSTANCE_ID` env var to
    /// customize the instance ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow::core::observability::CustomMetricsRegistry;
    ///
    /// let registry = CustomMetricsRegistry::global();
    /// registry.register_histogram(
    ///     "llm_token_count",
    ///     "Distribution of token counts per request",
    ///     &["provider"],
    ///     Some(vec![10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0])
    /// ).unwrap();
    /// // Metric will have instance_id="<value>" const label
    /// ```
    pub fn register_histogram(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
        buckets: Option<Vec<f64>>,
    ) -> Result<(), String> {
        let opts = if let Some(buckets) = buckets {
            HistogramOpts::new(name, help)
                .buckets(buckets)
                .const_label("instance_id", instance_id())
        } else {
            HistogramOpts::new(name, help).const_label("instance_id", instance_id())
        };

        let histogram = HistogramVec::new(opts, labels).map_err(|e| e.to_string())?;

        self.registry
            .register(Box::new(histogram.clone()))
            .map_err(|e| e.to_string())?;

        self.histograms
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), histogram);

        Ok(())
    }

    /// Increment a counter metric
    ///
    /// # Arguments
    ///
    /// * `name` - Metric name
    /// * `labels` - Label key-value pairs
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow::core::observability::CustomMetricsRegistry;
    ///
    /// let registry = CustomMetricsRegistry::new();
    /// registry.register_counter(
    ///     "llm_calls_total",
    ///     "Total LLM calls",
    ///     &["provider", "model"]
    /// ).unwrap();
    /// registry.increment_counter(
    ///     "llm_calls_total",
    ///     &[("provider", "openai"), ("model", "gpt-4")]
    /// ).unwrap();
    /// ```
    pub fn increment_counter(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), String> {
        let counters = self.counters.read().unwrap_or_else(|e| e.into_inner());
        let counter = counters
            .get(name)
            .ok_or_else(|| format!("Counter '{name}' not found"))?;

        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        counter.with_label_values(&label_values).inc();

        Ok(())
    }

    /// Add a value to a counter metric
    ///
    /// # Arguments
    ///
    /// * `name` - Metric name
    /// * `labels` - Label key-value pairs
    /// * `value` - Amount to add
    pub fn add_counter(
        &self,
        name: &str,
        labels: &[(&str, &str)],
        value: f64,
    ) -> Result<(), String> {
        let counters = self.counters.read().unwrap_or_else(|e| e.into_inner());
        let counter = counters
            .get(name)
            .ok_or_else(|| format!("Counter '{name}' not found"))?;

        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        counter.with_label_values(&label_values).inc_by(value);

        Ok(())
    }

    /// Set a gauge metric value
    ///
    /// # Arguments
    ///
    /// * `name` - Metric name
    /// * `labels` - Label key-value pairs
    /// * `value` - Value to set
    pub fn set_gauge(&self, name: &str, labels: &[(&str, &str)], value: f64) -> Result<(), String> {
        let gauges = self.gauges.read().unwrap_or_else(|e| e.into_inner());
        let gauge = gauges
            .get(name)
            .ok_or_else(|| format!("Gauge '{name}' not found"))?;

        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        gauge.with_label_values(&label_values).set(value);

        Ok(())
    }

    /// Increment a gauge metric
    pub fn increment_gauge(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), String> {
        let gauges = self.gauges.read().unwrap_or_else(|e| e.into_inner());
        let gauge = gauges
            .get(name)
            .ok_or_else(|| format!("Gauge '{name}' not found"))?;

        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        gauge.with_label_values(&label_values).inc();

        Ok(())
    }

    /// Decrement a gauge metric
    pub fn decrement_gauge(&self, name: &str, labels: &[(&str, &str)]) -> Result<(), String> {
        let gauges = self.gauges.read().unwrap_or_else(|e| e.into_inner());
        let gauge = gauges
            .get(name)
            .ok_or_else(|| format!("Gauge '{name}' not found"))?;

        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        gauge.with_label_values(&label_values).dec();

        Ok(())
    }

    /// Observe a histogram metric value
    ///
    /// # Arguments
    ///
    /// * `name` - Metric name
    /// * `labels` - Label key-value pairs
    /// * `value` - Value to observe
    pub fn observe_histogram(
        &self,
        name: &str,
        labels: &[(&str, &str)],
        value: f64,
    ) -> Result<(), String> {
        let histograms = self.histograms.read().unwrap_or_else(|e| e.into_inner());
        let histogram = histograms
            .get(name)
            .ok_or_else(|| format!("Histogram '{name}' not found"))?;

        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        histogram.with_label_values(&label_values).observe(value);

        Ok(())
    }

    /// Get all custom metrics in Prometheus text format
    ///
    /// By default, sensitive data in metric label values is redacted for security.
    /// This behavior is controlled by the `DASHFLOW_METRICS_REDACT` environment variable:
    /// - Default (unset): redaction enabled
    /// - "true", "1", "yes", "on": redaction enabled
    /// - "false", "0", "no", "off": redaction disabled
    pub fn get_metrics(&self) -> Result<String, String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();

        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|e| format!("Failed to encode metrics: {e}"))?;

        let metrics_text = String::from_utf8(buffer)
            .map_err(|e| format!("Failed to convert metrics to string: {e}"))?;

        // Apply redaction if enabled (default: ON for security)
        if is_metrics_redaction_enabled() {
            Ok(redact_prometheus_text(&metrics_text))
        } else {
            Ok(metrics_text)
        }
    }

    /// Get the underlying Prometheus registry
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl Default for CustomMetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper struct for common LLM metrics
pub struct LLMMetrics {
    registry: &'static CustomMetricsRegistry,
}

impl LLMMetrics {
    /// Initialize standard LLM metrics
    ///
    /// This registers commonly-used metrics for LLM applications:
    /// - `llm_calls_total`: Total number of LLM API calls
    /// - `llm_tokens_total`: Total number of tokens processed
    /// - `llm_call_duration_seconds`: Duration of LLM API calls
    /// - `llm_errors_total`: Total number of LLM errors
    /// - `llm_cache_hits_total`: Total number of cache hits
    /// - `llm_active_requests`: Number of active LLM requests
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow::core::observability::LLMMetrics;
    ///
    /// // Initialize standard LLM metrics
    /// LLMMetrics::init().unwrap();
    /// ```
    pub fn init() -> Result<Self, String> {
        let registry = CustomMetricsRegistry::global();

        // Helper to log non-idempotent registration failures
        // "already registered" errors are expected when init() is called multiple times
        fn log_registration_error(metric_name: &str, err: &str) {
            // Prometheus errors for duplicate registration contain "Duplicate" or "already"
            if !err.to_lowercase().contains("duplicate") && !err.to_lowercase().contains("already")
            {
                debug!(
                    metric = metric_name,
                    "Metric registration failed (non-duplicate): {}", err
                );
            }
        }

        // Attempt to register metrics - expected to fail with "already registered" on re-init
        // This makes init() idempotent for testing purposes
        if let Err(e) = registry.register_counter(
            "llm_calls_total",
            "Total number of LLM API calls by provider and model",
            &["provider", "model", "status"],
        ) {
            log_registration_error("llm_calls_total", &e);
        }

        if let Err(e) = registry.register_counter(
            "llm_tokens_total",
            "Total number of tokens processed by provider and model",
            &["provider", "model", "type"],
        ) {
            log_registration_error("llm_tokens_total", &e);
        }

        if let Err(e) = registry.register_histogram(
            "llm_call_duration_seconds",
            "Duration of LLM API calls in seconds",
            &["provider", "model"],
            Some(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
        ) {
            log_registration_error("llm_call_duration_seconds", &e);
        }

        if let Err(e) = registry.register_counter(
            "llm_errors_total",
            "Total number of LLM errors by provider and error type",
            &["provider", "model", "error_type"],
        ) {
            log_registration_error("llm_errors_total", &e);
        }

        if let Err(e) = registry.register_counter(
            "llm_cache_hits_total",
            "Total number of LLM cache hits",
            &["provider", "model"],
        ) {
            log_registration_error("llm_cache_hits_total", &e);
        }

        if let Err(e) = registry.register_gauge(
            "llm_active_requests",
            "Number of active LLM requests",
            &["provider", "model"],
        ) {
            log_registration_error("llm_active_requests", &e);
        }

        Ok(Self { registry })
    }

    /// Record an LLM API call
    pub fn record_call(
        &self,
        provider: &str,
        model: &str,
        duration_seconds: f64,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Result<(), String> {
        // Increment call counter
        self.registry.increment_counter(
            "llm_calls_total",
            &[
                ("provider", provider),
                ("model", model),
                ("status", "success"),
            ],
        )?;

        // Record duration
        self.registry.observe_histogram(
            "llm_call_duration_seconds",
            &[("provider", provider), ("model", model)],
            duration_seconds,
        )?;

        // Record token usage
        self.registry.add_counter(
            "llm_tokens_total",
            &[("provider", provider), ("model", model), ("type", "prompt")],
            f64::from(prompt_tokens),
        )?;
        self.registry.add_counter(
            "llm_tokens_total",
            &[
                ("provider", provider),
                ("model", model),
                ("type", "completion"),
            ],
            f64::from(completion_tokens),
        )?;

        Ok(())
    }

    /// Record an LLM error
    pub fn record_error(
        &self,
        provider: &str,
        model: &str,
        error_type: &str,
    ) -> Result<(), String> {
        self.registry.increment_counter(
            "llm_calls_total",
            &[
                ("provider", provider),
                ("model", model),
                ("status", "error"),
            ],
        )?;

        self.registry.increment_counter(
            "llm_errors_total",
            &[
                ("provider", provider),
                ("model", model),
                ("error_type", error_type),
            ],
        )?;

        Ok(())
    }

    /// Record a cache hit
    pub fn record_cache_hit(&self, provider: &str, model: &str) -> Result<(), String> {
        self.registry.increment_counter(
            "llm_cache_hits_total",
            &[("provider", provider), ("model", model)],
        )
    }

    /// Start tracking an active request
    pub fn start_request(&self, provider: &str, model: &str) -> Result<(), String> {
        self.registry.increment_gauge(
            "llm_active_requests",
            &[("provider", provider), ("model", model)],
        )
    }

    /// Stop tracking an active request
    pub fn end_request(&self, provider: &str, model: &str) -> Result<(), String> {
        self.registry.decrement_gauge(
            "llm_active_requests",
            &[("provider", provider), ("model", model)],
        )
    }
}

// ============================================================================
// M-36: Deterministic Event Emitter for Observability Tests
// ============================================================================

/// Metric name used by the deterministic event emitter for testing.
pub const TEST_EVENT_METRIC_NAME: &str = "dashflow_test_events_total";

/// Help text for the test event metric.
const TEST_EVENT_METRIC_HELP: &str = "Deterministic test events for observability verification";

/// Deterministic event emitter for observability tests (M-36).
///
/// This test helper allows observability tests to prove that metrics *change*,
/// not just that UIs load. It provides a way to:
///
/// 1. Emit a deterministic event with known label values
/// 2. Query the metric value before and after emission
/// 3. Assert that the metric delta matches the expected change
///
/// # Design Goals
///
/// - **Deterministic**: Each emitter instance has a unique `test_id` that isolates
///   test runs from each other, preventing cross-test interference.
/// - **Observable**: The emitter uses a well-known metric (`dashflow_test_events_total`)
///   that can be queried via Prometheus to verify changes.
/// - **Simple**: Minimal API surface - emit events and check if they were recorded.
///
/// # Usage in Integration Tests
///
/// ```rust,ignore
/// use dashflow::core::observability::DeterministicEventEmitter;
///
/// #[test]
/// fn test_observability_records_events() {
///     // Create an emitter with a unique test ID
///     let emitter = DeterministicEventEmitter::new("my_test").unwrap();
///
///     // Get the initial metric value
///     let before = emitter.current_count();
///
///     // Emit a deterministic event
///     emitter.emit().unwrap();
///
///     // Verify the metric changed
///     let after = emitter.current_count();
///     assert_eq!(after - before, 1, "Metric should have incremented by 1");
/// }
/// ```
///
/// # For E2E Dashboard Tests
///
/// The emitter can be used with `PrometheusClient` to verify metrics flow through
/// the entire observability stack:
///
/// ```rust,ignore
/// // 1. Query Prometheus for initial value
/// let before = prometheus_client.query(&format!(
///     "{}{{test_id=\"{}\"}}",
///     TEST_EVENT_METRIC_NAME,
///     emitter.test_id()
/// )).await?;
///
/// // 2. Emit deterministic event
/// emitter.emit()?;
///
/// // 3. Wait for scrape interval (typically 15s)
/// tokio::time::sleep(Duration::from_secs(20)).await;
///
/// // 4. Query again and verify change
/// let after = prometheus_client.query(...).await?;
/// assert!(after > before, "Metric must have increased");
/// ```
#[derive(Debug, Clone)]
pub struct DeterministicEventEmitter {
    /// Unique test identifier for isolating test runs
    test_id: String,
    /// The metrics registry (uses its own isolated registry in tests)
    registry: CustomMetricsRegistry,
}

impl DeterministicEventEmitter {
    /// Create a new deterministic event emitter with the given test ID.
    ///
    /// The test ID is used as a label value to isolate metrics from different
    /// test runs. Use a unique, descriptive name for each test.
    ///
    /// # Arguments
    ///
    /// * `test_name` - A descriptive name for the test (e.g., "dashboard_e2e_test")
    ///
    /// # Returns
    ///
    /// A new emitter instance with a unique test ID combining the test name
    /// and a timestamp for uniqueness.
    pub fn new(test_name: &str) -> Result<Self, String> {
        let registry = CustomMetricsRegistry::new();

        // Register the test metric
        registry.register_counter(
            TEST_EVENT_METRIC_NAME,
            TEST_EVENT_METRIC_HELP,
            &["test_id", "event_type"],
        )?;

        // Create a unique test ID by combining name + timestamp
        let test_id = format!(
            "{}_{}",
            test_name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );

        Ok(Self { test_id, registry })
    }

    /// Create a new emitter with an explicit test ID.
    ///
    /// Use this when you need a predictable test ID (e.g., for coordinating
    /// between processes or for querying Prometheus directly).
    pub fn with_test_id(test_id: impl Into<String>) -> Result<Self, String> {
        let registry = CustomMetricsRegistry::new();

        registry.register_counter(
            TEST_EVENT_METRIC_NAME,
            TEST_EVENT_METRIC_HELP,
            &["test_id", "event_type"],
        )?;

        Ok(Self {
            test_id: test_id.into(),
            registry,
        })
    }

    /// Create a new emitter that uses the global metrics registry.
    ///
    /// Use this when you need metrics to be visible in Prometheus/Grafana
    /// for E2E testing. The metric is registered idempotently in the global
    /// registry.
    pub fn with_global_registry(test_name: &str) -> Result<Self, String> {
        let registry = CustomMetricsRegistry::global();

        // Register idempotently (ignore "already exists" errors)
        let _ = registry.register_counter(
            TEST_EVENT_METRIC_NAME,
            TEST_EVENT_METRIC_HELP,
            &["test_id", "event_type"],
        );

        let test_id = format!(
            "{}_{}",
            test_name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );

        Ok(Self {
            test_id,
            registry: registry.clone(),
        })
    }

    /// Get the test ID for this emitter.
    ///
    /// Use this when constructing PromQL queries to filter by test ID.
    #[must_use]
    pub fn test_id(&self) -> &str {
        &self.test_id
    }

    /// Emit a single deterministic test event.
    ///
    /// This increments the `dashflow_test_events_total` counter with:
    /// - `test_id`: The unique test identifier
    /// - `event_type`: "deterministic_test_event"
    ///
    /// Returns `Ok(())` on success, or an error if the metric update failed.
    pub fn emit(&self) -> Result<(), String> {
        self.emit_with_type("deterministic_test_event")
    }

    /// Emit a test event with a custom event type.
    ///
    /// # Arguments
    ///
    /// * `event_type` - Custom event type label value
    pub fn emit_with_type(&self, event_type: &str) -> Result<(), String> {
        self.registry.increment_counter(
            TEST_EVENT_METRIC_NAME,
            &[("test_id", &self.test_id), ("event_type", event_type)],
        )
    }

    /// Emit multiple test events at once.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of events to emit
    pub fn emit_n(&self, count: u64) -> Result<(), String> {
        self.registry.add_counter(
            TEST_EVENT_METRIC_NAME,
            &[
                ("test_id", &self.test_id),
                ("event_type", "deterministic_test_event"),
            ],
            count as f64,
        )
    }

    /// Get the current count for this test's events.
    ///
    /// This parses the Prometheus text format to extract the current value.
    /// Returns 0 if the metric hasn't been emitted yet.
    ///
    /// # Note
    ///
    /// For E2E tests querying actual Prometheus, use `PrometheusClient` instead.
    /// This method only works for in-process registry queries.
    #[must_use]
    pub fn current_count(&self) -> u64 {
        self.current_count_for_type("deterministic_test_event")
    }

    /// Get the current count for a specific event type.
    #[must_use]
    pub fn current_count_for_type(&self, event_type: &str) -> u64 {
        let metrics = match self.registry.get_metrics() {
            Ok(m) => m,
            Err(_) => return 0,
        };

        // Parse the Prometheus text format to find our metric
        // Looking for: dashflow_test_events_total{...,test_id="...",event_type="...",...} VALUE
        // Note: The metric also has an instance_id label due to M-300, so we check for
        // presence of both required labels rather than exact combined string.
        let test_id_label = format!("test_id=\"{}\"", self.test_id);
        let event_type_label = format!("event_type=\"{}\"", event_type);

        for line in metrics.lines() {
            if line.starts_with(TEST_EVENT_METRIC_NAME)
                && line.contains(&test_id_label)
                && line.contains(&event_type_label)
            {
                // Extract the value (last space-separated token)
                if let Some(value_str) = line.split_whitespace().last() {
                    if let Ok(value) = value_str.parse::<f64>() {
                        return value as u64;
                    }
                }
            }
        }

        0
    }

    /// Get all metrics in Prometheus text format (for debugging).
    pub fn get_metrics(&self) -> Result<String, String> {
        self.registry.get_metrics()
    }

    /// Create a PromQL query string to fetch this test's events from Prometheus.
    ///
    /// # Arguments
    ///
    /// * `event_type` - Optional event type filter (defaults to all events for this test)
    ///
    /// # Returns
    ///
    /// A PromQL query string suitable for use with `PrometheusClient`.
    #[must_use]
    pub fn prometheus_query(&self, event_type: Option<&str>) -> String {
        match event_type {
            Some(et) => format!(
                "{}{{test_id=\"{}\",event_type=\"{}\"}}",
                TEST_EVENT_METRIC_NAME, self.test_id, et
            ),
            None => format!("{}{{test_id=\"{}\"}}", TEST_EVENT_METRIC_NAME, self.test_id),
        }
    }
}

/// Result of a metric change verification.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricChangeResult {
    /// Metric value before the operation
    pub before: u64,
    /// Metric value after the operation
    pub after: u64,
    /// The delta (after - before)
    pub delta: i64,
}

impl MetricChangeResult {
    /// Check if the metric increased by the expected amount.
    #[must_use]
    pub fn increased_by(&self, expected: u64) -> bool {
        self.delta == expected as i64
    }

    /// Check if the metric increased at all.
    #[must_use]
    pub fn increased(&self) -> bool {
        self.delta > 0
    }

    /// Check if the metric stayed the same.
    #[must_use]
    pub fn unchanged(&self) -> bool {
        self.delta == 0
    }
}

/// Builder for verifying metric changes around an operation.
///
/// # Example
///
/// ```rust,ignore
/// let emitter = DeterministicEventEmitter::new("test")?;
/// let result = MetricChangeVerifier::new(&emitter)
///     .around(|| {
///         emitter.emit()?;
///         emitter.emit()?;
///         Ok(())
///     })?;
///
/// assert!(result.increased_by(2));
/// ```
pub struct MetricChangeVerifier<'a> {
    emitter: &'a DeterministicEventEmitter,
    event_type: Option<String>,
}

impl<'a> MetricChangeVerifier<'a> {
    /// Create a new verifier for the given emitter.
    #[must_use]
    pub fn new(emitter: &'a DeterministicEventEmitter) -> Self {
        Self {
            emitter,
            event_type: None,
        }
    }

    /// Filter by event type.
    #[must_use]
    pub fn for_event_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_type = Some(event_type.into());
        self
    }

    /// Execute an operation and measure the metric change.
    ///
    /// # Arguments
    ///
    /// * `operation` - A closure that performs the operation being tested
    ///
    /// # Returns
    ///
    /// A `MetricChangeResult` showing the before/after values and delta.
    pub fn around<F, E>(self, operation: F) -> Result<MetricChangeResult, E>
    where
        F: FnOnce() -> Result<(), E>,
    {
        let before = match &self.event_type {
            Some(et) => self.emitter.current_count_for_type(et),
            None => self.emitter.current_count(),
        };

        operation()?;

        let after = match &self.event_type {
            Some(et) => self.emitter.current_count_for_type(et),
            None => self.emitter.current_count(),
        };

        Ok(MetricChangeResult {
            before,
            after,
            delta: after as i64 - before as i64,
        })
    }
}

impl<'a> MetricChangeVerifier<'a> {
    /// Execute an async operation and measure the metric change.
    ///
    /// This method is available when the tokio runtime is present.
    pub async fn around_async<F, Fut, E>(self, operation: F) -> Result<MetricChangeResult, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), E>>,
    {
        let before = match &self.event_type {
            Some(et) => self.emitter.current_count_for_type(et),
            None => self.emitter.current_count(),
        };

        operation().await?;

        let after = match &self.event_type {
            Some(et) => self.emitter.current_count_for_type(et),
            None => self.emitter.current_count(),
        };

        Ok(MetricChangeResult {
            before,
            after,
            delta: after as i64 - before as i64,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_register_counter() {
        let registry = CustomMetricsRegistry::new();
        let result = registry.register_counter("test_counter", "Test counter", &["label1"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_increment_counter() {
        let registry = CustomMetricsRegistry::new();
        registry
            .register_counter("test_counter", "Test counter", &["label1"])
            .unwrap();

        let result = registry.increment_counter("test_counter", &[("label1", "value1")]);
        assert!(result.is_ok());

        let metrics = registry.get_metrics().unwrap();
        assert!(metrics.contains("test_counter"));
    }

    #[test]
    fn test_register_gauge() {
        let registry = CustomMetricsRegistry::new();
        let result = registry.register_gauge("test_gauge", "Test gauge", &["label1"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_gauge() {
        let registry = CustomMetricsRegistry::new();
        registry
            .register_gauge("test_gauge", "Test gauge", &["label1"])
            .unwrap();

        let result = registry.set_gauge("test_gauge", &[("label1", "value1")], 42.0);
        assert!(result.is_ok());

        let metrics = registry.get_metrics().unwrap();
        assert!(metrics.contains("test_gauge"));
    }

    #[test]
    fn test_register_histogram() {
        let registry = CustomMetricsRegistry::new();
        let result = registry.register_histogram(
            "test_histogram",
            "Test histogram",
            &["label1"],
            Some(vec![1.0, 5.0, 10.0]),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_observe_histogram() {
        let registry = CustomMetricsRegistry::new();
        registry
            .register_histogram("test_histogram", "Test histogram", &["label1"], None)
            .unwrap();

        let result = registry.observe_histogram("test_histogram", &[("label1", "value1")], 5.5);
        assert!(result.is_ok());

        let metrics = registry.get_metrics().unwrap();
        assert!(metrics.contains("test_histogram"));
    }

    // ========================================================================
    // M-300: Instance ID Label Tests
    // ========================================================================

    use super::instance_id;

    #[test]
    fn test_instance_id_exists() {
        // Verify INSTANCE_ID is generated (either from env or random)
        let id = instance_id();
        assert!(!id.is_empty(), "Instance ID should not be empty");
        // Should be 8 chars (first 8 of UUID) unless set via env var
        // We can't assert exact length since env var could be set
    }

    #[test]
    fn test_counter_has_instance_id_label() {
        let registry = CustomMetricsRegistry::new();
        registry
            .register_counter("test_counter_with_instance", "Test counter", &["label1"])
            .unwrap();
        registry
            .increment_counter("test_counter_with_instance", &[("label1", "value1")])
            .unwrap();

        let metrics = registry.get_metrics().unwrap();

        // Verify instance_id label is present in the output
        assert!(
            metrics.contains("instance_id="),
            "Counter metrics should contain instance_id label. Got: {}",
            metrics
        );
    }

    #[test]
    fn test_gauge_has_instance_id_label() {
        let registry = CustomMetricsRegistry::new();
        registry
            .register_gauge("test_gauge_with_instance", "Test gauge", &["label1"])
            .unwrap();
        registry
            .set_gauge("test_gauge_with_instance", &[("label1", "value1")], 42.0)
            .unwrap();

        let metrics = registry.get_metrics().unwrap();

        // Verify instance_id label is present in the output
        assert!(
            metrics.contains("instance_id="),
            "Gauge metrics should contain instance_id label. Got: {}",
            metrics
        );
    }

    #[test]
    fn test_histogram_has_instance_id_label() {
        let registry = CustomMetricsRegistry::new();
        registry
            .register_histogram(
                "test_histogram_with_instance",
                "Test histogram",
                &["label1"],
                None,
            )
            .unwrap();
        registry
            .observe_histogram("test_histogram_with_instance", &[("label1", "value1")], 5.5)
            .unwrap();

        let metrics = registry.get_metrics().unwrap();

        // Verify instance_id label is present in the output
        assert!(
            metrics.contains("instance_id="),
            "Histogram metrics should contain instance_id label. Got: {}",
            metrics
        );
    }

    #[test]
    fn test_instance_id_matches_in_metrics_output() {
        let registry = CustomMetricsRegistry::new();
        registry
            .register_counter("test_counter_instance_match", "Test counter", &["label1"])
            .unwrap();
        registry
            .increment_counter("test_counter_instance_match", &[("label1", "value1")])
            .unwrap();

        let metrics = registry.get_metrics().unwrap();
        let expected_label = format!("instance_id=\"{}\"", instance_id());

        // Verify the exact instance_id value is present
        assert!(
            metrics.contains(&expected_label),
            "Metrics should contain instance_id=\"{}\". Got: {}",
            instance_id(),
            metrics
        );
    }

    #[test]
    fn test_llm_metrics_init() {
        let result = LLMMetrics::init();
        assert!(result.is_ok());
    }

    #[test]
    fn test_llm_metrics_record_call() {
        let llm_metrics = LLMMetrics::init().unwrap();
        let result = llm_metrics.record_call("openai", "gpt-4", 1.5, 100, 50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_llm_metrics_record_error() {
        let llm_metrics = LLMMetrics::init().unwrap();
        let result = llm_metrics.record_error("openai", "gpt-4", "timeout");
        assert!(result.is_ok());
    }

    #[test]
    fn test_llm_metrics_cache_hit() {
        let llm_metrics = LLMMetrics::init().unwrap();
        let result = llm_metrics.record_cache_hit("openai", "gpt-4");
        assert!(result.is_ok());
    }

    #[test]
    fn test_llm_metrics_active_requests() {
        let llm_metrics = LLMMetrics::init().unwrap();

        llm_metrics.start_request("openai", "gpt-4").unwrap();
        llm_metrics.end_request("openai", "gpt-4").unwrap();

        // Should not panic
    }

    // ========================================================================
    // M-36: DeterministicEventEmitter Tests
    // ========================================================================

    use super::{
        DeterministicEventEmitter, MetricChangeResult, MetricChangeVerifier, TEST_EVENT_METRIC_NAME,
    };

    #[test]
    fn test_deterministic_emitter_new() {
        let emitter = DeterministicEventEmitter::new("test_new").unwrap();
        assert!(emitter.test_id().starts_with("test_new_"));
    }

    #[test]
    fn test_deterministic_emitter_with_test_id() {
        let emitter = DeterministicEventEmitter::with_test_id("explicit_id").unwrap();
        assert_eq!(emitter.test_id(), "explicit_id");
    }

    #[test]
    fn test_deterministic_emitter_emit() {
        let emitter = DeterministicEventEmitter::new("test_emit").unwrap();

        // Initial count should be 0
        assert_eq!(emitter.current_count(), 0);

        // Emit a single event
        emitter.emit().unwrap();

        // Count should be 1
        assert_eq!(emitter.current_count(), 1);
    }

    #[test]
    fn test_deterministic_emitter_emit_multiple() {
        let emitter = DeterministicEventEmitter::new("test_emit_multi").unwrap();

        // Emit multiple events
        emitter.emit().unwrap();
        emitter.emit().unwrap();
        emitter.emit().unwrap();

        // Count should be 3
        assert_eq!(emitter.current_count(), 3);
    }

    #[test]
    fn test_deterministic_emitter_emit_n() {
        let emitter = DeterministicEventEmitter::new("test_emit_n").unwrap();

        // Emit 5 events at once
        emitter.emit_n(5).unwrap();

        // Count should be 5
        assert_eq!(emitter.current_count(), 5);
    }

    #[test]
    fn test_deterministic_emitter_emit_with_type() {
        let emitter = DeterministicEventEmitter::new("test_emit_type").unwrap();

        // Emit events with different types
        emitter.emit_with_type("type_a").unwrap();
        emitter.emit_with_type("type_a").unwrap();
        emitter.emit_with_type("type_b").unwrap();

        // Check counts by type
        assert_eq!(emitter.current_count_for_type("type_a"), 2);
        assert_eq!(emitter.current_count_for_type("type_b"), 1);
        assert_eq!(emitter.current_count_for_type("type_c"), 0);
    }

    #[test]
    fn test_deterministic_emitter_isolation() {
        // Create two emitters with different test IDs
        let emitter1 = DeterministicEventEmitter::with_test_id("isolation_1").unwrap();
        let emitter2 = DeterministicEventEmitter::with_test_id("isolation_2").unwrap();

        // Emit to emitter1
        emitter1.emit().unwrap();
        emitter1.emit().unwrap();

        // Emit to emitter2
        emitter2.emit().unwrap();

        // Counts should be isolated
        assert_eq!(emitter1.current_count(), 2);
        assert_eq!(emitter2.current_count(), 1);
    }

    #[test]
    fn test_deterministic_emitter_prometheus_query() {
        let emitter = DeterministicEventEmitter::with_test_id("query_test").unwrap();

        // Generate query without event type filter
        let query = emitter.prometheus_query(None);
        assert_eq!(
            query,
            format!("{}{{test_id=\"query_test\"}}", TEST_EVENT_METRIC_NAME)
        );

        // Generate query with event type filter
        let query_typed = emitter.prometheus_query(Some("custom_event"));
        assert_eq!(
            query_typed,
            format!(
                "{}{{test_id=\"query_test\",event_type=\"custom_event\"}}",
                TEST_EVENT_METRIC_NAME
            )
        );
    }

    #[test]
    fn test_deterministic_emitter_get_metrics() {
        let emitter = DeterministicEventEmitter::with_test_id("metrics_test").unwrap();
        emitter.emit().unwrap();

        let metrics = emitter.get_metrics().unwrap();

        // Should contain the metric name
        assert!(metrics.contains(TEST_EVENT_METRIC_NAME));
        // Should contain the test ID
        assert!(metrics.contains("metrics_test"));
    }

    // ========================================================================
    // MetricChangeResult Tests
    // ========================================================================

    #[test]
    fn test_metric_change_result_increased_by() {
        let result = MetricChangeResult {
            before: 5,
            after: 8,
            delta: 3,
        };

        assert!(result.increased_by(3));
        assert!(!result.increased_by(2));
        assert!(!result.increased_by(4));
    }

    #[test]
    fn test_metric_change_result_increased() {
        let increased = MetricChangeResult {
            before: 5,
            after: 8,
            delta: 3,
        };
        let unchanged = MetricChangeResult {
            before: 5,
            after: 5,
            delta: 0,
        };
        let decreased = MetricChangeResult {
            before: 8,
            after: 5,
            delta: -3,
        };

        assert!(increased.increased());
        assert!(!unchanged.increased());
        assert!(!decreased.increased());
    }

    #[test]
    fn test_metric_change_result_unchanged() {
        let unchanged = MetricChangeResult {
            before: 5,
            after: 5,
            delta: 0,
        };
        let changed = MetricChangeResult {
            before: 5,
            after: 8,
            delta: 3,
        };

        assert!(unchanged.unchanged());
        assert!(!changed.unchanged());
    }

    // ========================================================================
    // MetricChangeVerifier Tests
    // ========================================================================

    #[test]
    fn test_metric_change_verifier_around() {
        let emitter = DeterministicEventEmitter::new("verifier_test").unwrap();

        let result = MetricChangeVerifier::new(&emitter)
            .around(|| {
                emitter.emit().unwrap();
                emitter.emit().unwrap();
                Ok::<(), String>(())
            })
            .unwrap();

        assert_eq!(result.before, 0);
        assert_eq!(result.after, 2);
        assert_eq!(result.delta, 2);
        assert!(result.increased_by(2));
    }

    #[test]
    fn test_metric_change_verifier_with_event_type() {
        let emitter = DeterministicEventEmitter::new("verifier_typed").unwrap();

        // First emit some events of type_a
        emitter.emit_with_type("type_a").unwrap();

        // Now verify changes to type_b only
        let result = MetricChangeVerifier::new(&emitter)
            .for_event_type("type_b")
            .around(|| {
                emitter.emit_with_type("type_a").unwrap(); // Should NOT count
                emitter.emit_with_type("type_b").unwrap(); // Should count
                Ok::<(), String>(())
            })
            .unwrap();

        // Only type_b events counted
        assert_eq!(result.delta, 1);
    }

    #[test]
    fn test_metric_change_verifier_no_change() {
        let emitter = DeterministicEventEmitter::new("verifier_no_change").unwrap();

        let result = MetricChangeVerifier::new(&emitter)
            .around(|| {
                // Don't emit anything
                Ok::<(), String>(())
            })
            .unwrap();

        assert!(result.unchanged());
        assert_eq!(result.delta, 0);
    }

    #[test]
    fn test_metric_change_verifier_error_propagation() {
        let emitter = DeterministicEventEmitter::new("verifier_error").unwrap();

        let result: std::result::Result<MetricChangeResult, String> =
            MetricChangeVerifier::new(&emitter).around(|| {
                emitter.emit().unwrap();
                Err("simulated error".to_string())
            });

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "simulated error");
    }

    /// Integration test demonstrating the full M-36 pattern:
    /// proving metrics *change*, not just that they exist.
    #[test]
    fn test_m36_pattern_prove_metric_changes() {
        // 1. Create emitter with deterministic test ID
        let emitter = DeterministicEventEmitter::with_test_id("m36_integration").unwrap();

        // 2. Get baseline metric value
        let before = emitter.current_count();
        assert_eq!(before, 0, "Baseline should be 0 for new emitter");

        // 3. Perform operation that should emit events
        emitter.emit().unwrap();

        // 4. Verify metric CHANGED (not just exists)
        let after = emitter.current_count();
        assert_eq!(after, 1, "Metric should have changed from 0 to 1");
        assert_eq!(
            after - before,
            1,
            "Delta should be exactly 1 - proving the metric CHANGED"
        );

        // 5. For E2E tests, construct the PromQL query
        let promql = emitter.prometheus_query(None);
        assert!(
            promql.contains("m36_integration"),
            "PromQL query should filter by our test ID"
        );
    }
}
