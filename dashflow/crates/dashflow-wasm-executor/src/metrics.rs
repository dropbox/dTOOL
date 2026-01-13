// Note: Audited - only 1 expect() in production (line 335, Default impl for metrics registration).
// Metrics registration with static labels should never fail.

//! Prometheus metrics module for WASM executor
//!
//! Implements SOC2 CC7.2 - Detection and Monitoring
//!
//! Key metrics tracked:
//! - Execution counters (total, success, failure, timeout)
//! - Execution duration histograms (latency)
//! - Fuel consumed histograms (CPU usage)
//! - Memory peak histograms (memory usage)
//! - Authentication counters (success, failure)
//! - Authorization counters (granted, denied)
//! - Concurrent execution gauge (in-flight operations)

use prometheus::{
    register_counter_vec, register_gauge, register_histogram_vec, CounterVec, Gauge, HistogramVec,
    Opts, Registry,
};
use std::sync::Arc;

/// Monitoring metrics for WASM executor
///
/// This struct holds all Prometheus metrics for the WASM executor.
/// Metrics are registered with the default Prometheus registry and can be
/// exported via the /metrics endpoint.
#[derive(Clone)]
pub struct Metrics {
    /// Total WASM executions by status (success, failure, timeout)
    pub executions_total: Arc<CounterVec>,

    /// WASM execution duration in seconds (histogram with quantiles)
    pub execution_duration_seconds: Arc<HistogramVec>,

    /// Fuel consumed per execution (histogram with quantiles)
    pub fuel_consumed: Arc<HistogramVec>,

    /// Peak memory usage per execution in bytes (histogram with quantiles)
    pub memory_peak_bytes: Arc<HistogramVec>,

    /// Number of concurrent WASM executions (gauge)
    pub concurrent_executions: Arc<Gauge>,

    /// Total authentication attempts by result (success, failure)
    pub auth_attempts_total: Arc<CounterVec>,

    /// Total access denied events by reason (auth, authz, `rate_limit`)
    pub access_denied_total: Arc<CounterVec>,
}

impl Metrics {
    /// Create new metrics instance
    ///
    /// Registers all metrics with the default Prometheus registry.
    ///
    /// # Errors
    /// Returns error if metrics cannot be registered (e.g., duplicate registration)
    pub fn new() -> Result<Self, prometheus::Error> {
        // Execution counter
        let executions_total = register_counter_vec!(
            Opts::new("wasm_executions_total", "Total number of WASM executions"),
            &["status"] // Labels: success, failure, timeout
        )?;

        // Execution duration histogram
        let execution_duration_seconds = register_histogram_vec!(
            "wasm_execution_duration_seconds",
            "WASM execution duration in seconds",
            &["status"],
            vec![0.001, 0.005, 0.010, 0.050, 0.100, 0.500, 1.0, 5.0, 10.0, 30.0]
        )?;

        // Fuel consumed histogram
        let fuel_consumed = register_histogram_vec!(
            "wasm_fuel_consumed",
            "Fuel consumed per WASM execution",
            &["status"],
            vec![
                1_000.0,
                10_000.0,
                100_000.0,
                1_000_000.0,
                10_000_000.0,
                100_000_000.0,
            ]
        )?;

        // Memory peak histogram
        let memory_peak_bytes = register_histogram_vec!(
            "wasm_memory_peak_bytes",
            "Peak memory usage per WASM execution in bytes",
            &["status"],
            vec![
                1024.0,        // 1 KB
                10_240.0,      // 10 KB
                102_400.0,     // 100 KB
                1_048_576.0,   // 1 MB
                10_485_760.0,  // 10 MB
                104_857_600.0, // 100 MB
                268_435_456.0, // 256 MB
            ]
        )?;

        // Concurrent executions gauge
        let concurrent_executions = register_gauge!(Opts::new(
            "wasm_concurrent_executions",
            "Number of WASM executions currently in progress"
        ))?;

        // Authentication attempts counter
        let auth_attempts_total = register_counter_vec!(
            Opts::new("auth_attempts_total", "Total authentication attempts"),
            &["result"] // Labels: success, failure
        )?;

        // Access denied counter
        let access_denied_total = register_counter_vec!(
            Opts::new("access_denied_total", "Total access denied events"),
            &["reason"] // Labels: auth, authz, rate_limit
        )?;

        Ok(Self {
            executions_total: Arc::new(executions_total),
            execution_duration_seconds: Arc::new(execution_duration_seconds),
            fuel_consumed: Arc::new(fuel_consumed),
            memory_peak_bytes: Arc::new(memory_peak_bytes),
            concurrent_executions: Arc::new(concurrent_executions),
            auth_attempts_total: Arc::new(auth_attempts_total),
            access_denied_total: Arc::new(access_denied_total),
        })
    }

    /// Create metrics instance with custom registry
    ///
    /// Useful for testing or when you need to control the registry.
    ///
    /// # Arguments
    /// * `registry` - Custom Prometheus registry
    ///
    /// # Errors
    /// Returns error if metrics cannot be registered
    pub fn new_with_registry(registry: &Registry) -> Result<Self, prometheus::Error> {
        // Execution counter
        let executions_total = CounterVec::new(
            Opts::new("wasm_executions_total", "Total number of WASM executions"),
            &["status"],
        )?;
        registry.register(Box::new(executions_total.clone()))?;

        // Execution duration histogram
        let execution_duration_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "wasm_execution_duration_seconds",
                "WASM execution duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.010, 0.050, 0.100, 0.500, 1.0, 5.0, 10.0, 30.0,
            ]),
            &["status"],
        )?;
        registry.register(Box::new(execution_duration_seconds.clone()))?;

        // Fuel consumed histogram
        let fuel_consumed = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "wasm_fuel_consumed",
                "Fuel consumed per WASM execution",
            )
            .buckets(vec![
                1_000.0,
                10_000.0,
                100_000.0,
                1_000_000.0,
                10_000_000.0,
                100_000_000.0,
            ]),
            &["status"],
        )?;
        registry.register(Box::new(fuel_consumed.clone()))?;

        // Memory peak histogram
        let memory_peak_bytes = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "wasm_memory_peak_bytes",
                "Peak memory usage per WASM execution in bytes",
            )
            .buckets(vec![
                1024.0,
                10_240.0,
                102_400.0,
                1_048_576.0,
                10_485_760.0,
                104_857_600.0,
                268_435_456.0,
            ]),
            &["status"],
        )?;
        registry.register(Box::new(memory_peak_bytes.clone()))?;

        // Concurrent executions gauge
        let concurrent_executions = Gauge::new(
            "wasm_concurrent_executions",
            "Number of WASM executions currently in progress",
        )?;
        registry.register(Box::new(concurrent_executions.clone()))?;

        // Authentication attempts counter
        let auth_attempts_total = CounterVec::new(
            Opts::new("auth_attempts_total", "Total authentication attempts"),
            &["result"],
        )?;
        registry.register(Box::new(auth_attempts_total.clone()))?;

        // Access denied counter
        let access_denied_total = CounterVec::new(
            Opts::new("access_denied_total", "Total access denied events"),
            &["reason"],
        )?;
        registry.register(Box::new(access_denied_total.clone()))?;

        Ok(Self {
            executions_total: Arc::new(executions_total),
            execution_duration_seconds: Arc::new(execution_duration_seconds),
            fuel_consumed: Arc::new(fuel_consumed),
            memory_peak_bytes: Arc::new(memory_peak_bytes),
            concurrent_executions: Arc::new(concurrent_executions),
            auth_attempts_total: Arc::new(auth_attempts_total),
            access_denied_total: Arc::new(access_denied_total),
        })
    }

    /// Record successful execution
    ///
    /// # Arguments
    /// * `duration_secs` - Execution duration in seconds
    /// * `fuel_consumed` - Fuel consumed during execution
    /// * `memory_peak` - Peak memory usage in bytes
    pub fn record_success(&self, duration_secs: f64, fuel_consumed: u64, memory_peak: u64) {
        self.executions_total.with_label_values(&["success"]).inc();
        self.execution_duration_seconds
            .with_label_values(&["success"])
            .observe(duration_secs);
        self.fuel_consumed
            .with_label_values(&["success"])
            .observe(fuel_consumed as f64);
        self.memory_peak_bytes
            .with_label_values(&["success"])
            .observe(memory_peak as f64);
    }

    /// Record failed execution
    ///
    /// # Arguments
    /// * `duration_secs` - Execution duration in seconds before failure
    /// * `fuel_consumed` - Fuel consumed before failure
    /// * `memory_peak` - Peak memory usage before failure in bytes
    pub fn record_failure(&self, duration_secs: f64, fuel_consumed: u64, memory_peak: u64) {
        self.executions_total.with_label_values(&["failure"]).inc();
        self.execution_duration_seconds
            .with_label_values(&["failure"])
            .observe(duration_secs);
        self.fuel_consumed
            .with_label_values(&["failure"])
            .observe(fuel_consumed as f64);
        self.memory_peak_bytes
            .with_label_values(&["failure"])
            .observe(memory_peak as f64);
    }

    /// Record timeout
    ///
    /// # Arguments
    /// * `duration_secs` - Execution duration in seconds before timeout
    pub fn record_timeout(&self, duration_secs: f64) {
        self.executions_total.with_label_values(&["timeout"]).inc();
        self.execution_duration_seconds
            .with_label_values(&["timeout"])
            .observe(duration_secs);
        // Fuel and memory not available for timeouts
    }

    /// Increment concurrent executions counter
    ///
    /// Call this when starting a WASM execution
    pub fn execution_started(&self) {
        self.concurrent_executions.inc();
    }

    /// Decrement concurrent executions counter
    ///
    /// Call this when finishing a WASM execution (success or failure)
    pub fn execution_finished(&self) {
        self.concurrent_executions.dec();
    }

    /// Record successful authentication
    pub fn record_auth_success(&self) {
        self.auth_attempts_total
            .with_label_values(&["success"])
            .inc();
    }

    /// Record failed authentication
    pub fn record_auth_failure(&self) {
        self.auth_attempts_total
            .with_label_values(&["failure"])
            .inc();
    }

    /// Record access denied due to authentication failure
    pub fn record_access_denied_auth(&self) {
        self.access_denied_total.with_label_values(&["auth"]).inc();
    }

    /// Record access denied due to authorization failure
    pub fn record_access_denied_authz(&self) {
        self.access_denied_total.with_label_values(&["authz"]).inc();
    }

    /// Record access denied due to rate limiting
    pub fn record_access_denied_rate_limit(&self) {
        self.access_denied_total
            .with_label_values(&["rate_limit"])
            .inc();
    }
}

impl Default for Metrics {
    #[allow(clippy::expect_used)] // Static labels should never fail registration
    fn default() -> Self {
        // Use custom registry for test isolation
        // Each executor gets its own metrics registry to avoid conflicts
        let registry = Registry::new();
        Self::new_with_registry(&registry).expect("Failed to create default metrics")
    }
}

// Prometheus counter values are exact f64 integers (1.0, 2.0), safe to compare with ==
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        // Create metrics with custom registry to avoid conflicts with other tests
        let registry = Registry::new();
        let metrics = Metrics::new_with_registry(&registry);
        assert!(metrics.is_ok(), "Failed to create metrics");
    }

    #[test]
    fn test_record_success() {
        let registry = Registry::new();
        let metrics = Metrics::new_with_registry(&registry).unwrap();

        metrics.record_success(0.5, 1_000_000, 1_048_576);

        // Verify counter was incremented
        let counter = metrics
            .executions_total
            .with_label_values(&["success"])
            .get();
        assert_eq!(counter, 1.0);
    }

    #[test]
    fn test_record_failure() {
        let registry = Registry::new();
        let metrics = Metrics::new_with_registry(&registry).unwrap();

        metrics.record_failure(0.1, 50_000, 102_400);

        // Verify counter was incremented
        let counter = metrics
            .executions_total
            .with_label_values(&["failure"])
            .get();
        assert_eq!(counter, 1.0);
    }

    #[test]
    fn test_record_timeout() {
        let registry = Registry::new();
        let metrics = Metrics::new_with_registry(&registry).unwrap();

        metrics.record_timeout(30.0);

        // Verify counter was incremented
        let counter = metrics
            .executions_total
            .with_label_values(&["timeout"])
            .get();
        assert_eq!(counter, 1.0);
    }

    #[test]
    fn test_concurrent_executions() {
        let registry = Registry::new();
        let metrics = Metrics::new_with_registry(&registry).unwrap();

        metrics.execution_started();
        assert_eq!(metrics.concurrent_executions.get(), 1.0);

        metrics.execution_started();
        assert_eq!(metrics.concurrent_executions.get(), 2.0);

        metrics.execution_finished();
        assert_eq!(metrics.concurrent_executions.get(), 1.0);

        metrics.execution_finished();
        assert_eq!(metrics.concurrent_executions.get(), 0.0);
    }

    #[test]
    fn test_auth_metrics() {
        let registry = Registry::new();
        let metrics = Metrics::new_with_registry(&registry).unwrap();

        metrics.record_auth_success();
        metrics.record_auth_success();
        metrics.record_auth_failure();

        let success = metrics
            .auth_attempts_total
            .with_label_values(&["success"])
            .get();
        let failure = metrics
            .auth_attempts_total
            .with_label_values(&["failure"])
            .get();

        assert_eq!(success, 2.0);
        assert_eq!(failure, 1.0);
    }

    #[test]
    fn test_access_denied_metrics() {
        let registry = Registry::new();
        let metrics = Metrics::new_with_registry(&registry).unwrap();

        metrics.record_access_denied_auth();
        metrics.record_access_denied_authz();
        metrics.record_access_denied_authz();
        metrics.record_access_denied_rate_limit();

        let auth = metrics
            .access_denied_total
            .with_label_values(&["auth"])
            .get();
        let authz = metrics
            .access_denied_total
            .with_label_values(&["authz"])
            .get();
        let rate_limit = metrics
            .access_denied_total
            .with_label_values(&["rate_limit"])
            .get();

        assert_eq!(auth, 1.0);
        assert_eq!(authz, 2.0);
        assert_eq!(rate_limit, 1.0);
    }
}
