//! Prometheus metrics for `LangServe`
//!
//! This module provides comprehensive metrics for monitoring `LangServe` API performance.
//! Metrics follow Prometheus best practices and are compatible with Grafana dashboards.
//!
//! All metrics are registered to the unified `dashflow-observability` global registry,
//! ensuring they appear alongside other DashFlow metrics in a single `/metrics` endpoint.

use dashflow_observability::metrics_registry;
use prometheus::{Encoder, Histogram, HistogramOpts, IntCounterVec, Opts, TextEncoder};
use std::sync::OnceLock;

/// Lazily initialized metrics registered to the global registry
struct LangServeMetrics {
    request_counter: IntCounterVec,
    request_duration: Histogram,
    batch_size: Histogram,
    stream_chunks: Histogram,
    error_counter: IntCounterVec,
}

/// Global metrics instance
static METRICS: OnceLock<LangServeMetrics> = OnceLock::new();

/// Initialize and get the LangServe metrics
#[allow(clippy::expect_used)] // Static metric creation cannot fail with valid options
fn get_or_init_metrics() -> &'static LangServeMetrics {
    METRICS.get_or_init(|| {
        let global_registry = metrics_registry();

        // Create metrics
        let request_counter = IntCounterVec::new(
            Opts::new(
                "langserve_requests_total",
                "Total number of HTTP requests by endpoint and status",
            ),
            &["endpoint", "status"],
        )
        .expect("Failed to create request_counter");

        let request_duration = Histogram::with_opts(
            HistogramOpts::new(
                "langserve_request_duration_seconds",
                "Request duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("Failed to create request_duration");

        let batch_size = Histogram::with_opts(
            HistogramOpts::new("langserve_batch_size", "Number of items in batch requests")
                .buckets(vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0]),
        )
        .expect("Failed to create batch_size");

        let stream_chunks = Histogram::with_opts(
            HistogramOpts::new(
                "langserve_stream_chunks_total",
                "Number of chunks per streaming request",
            )
            .buckets(vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0]),
        )
        .expect("Failed to create stream_chunks");

        let error_counter = IntCounterVec::new(
            Opts::new(
                "langserve_errors_total",
                "Total number of errors by type and endpoint",
            ),
            &["error_type", "endpoint"],
        )
        .expect("Failed to create error_counter");

        // Register all metrics to the global registry
        //
        // Ignore `AlreadyReg` to support idempotent initialization, but log unexpected failures
        // so missing metrics don't go unnoticed.
        let registry = global_registry.registry();
        let register_metric =
            |collector: Box<dyn prometheus::core::Collector>, metric_name: &'static str| {
                if let Err(err) = registry.register(collector) {
                    if !matches!(err, prometheus::Error::AlreadyReg) {
                        tracing::warn!(
                            metric_name,
                            error = %err,
                            "Failed to register Prometheus metric"
                        );
                    }
                }
            };

        register_metric(Box::new(request_counter.clone()), "langserve_requests_total");
        register_metric(
            Box::new(request_duration.clone()),
            "langserve_request_duration_seconds",
        );
        register_metric(Box::new(batch_size.clone()), "langserve_batch_size");
        register_metric(Box::new(stream_chunks.clone()), "langserve_stream_chunks_total");
        register_metric(Box::new(error_counter.clone()), "langserve_errors_total");

        LangServeMetrics {
            request_counter,
            request_duration,
            batch_size,
            stream_chunks,
            error_counter,
        }
    })
}

/// Get metrics in Prometheus text format
///
/// This exports all metrics from the unified global registry, which includes
/// metrics from all DashFlow crates (observability, langserve, streaming, etc.)
pub fn get_metrics() -> Result<String, String> {
    // Ensure our metrics are initialized
    get_or_init_metrics();

    // Export from the unified registry
    dashflow_observability::export_metrics().map_err(|e| e.to_string())
}

/// Get metrics in Prometheus text format from this module only
///
/// This is useful for testing or when you want to see only LangServe metrics.
/// For production, prefer `get_metrics()` which exports from the unified registry.
pub fn get_langserve_metrics_only() -> Result<String, String> {
    let metrics = get_or_init_metrics();

    // Create a temporary registry for just our metrics
    let temp_registry = prometheus::Registry::new();
    let register_metric =
        |collector: Box<dyn prometheus::core::Collector>, metric_name: &'static str| {
            temp_registry.register(collector).map_err(|err| {
                format!("Failed to register Prometheus metric {metric_name}: {err}")
            })
        };

    register_metric(
        Box::new(metrics.request_counter.clone()),
        "langserve_requests_total",
    )?;
    register_metric(
        Box::new(metrics.request_duration.clone()),
        "langserve_request_duration_seconds",
    )?;
    register_metric(Box::new(metrics.batch_size.clone()), "langserve_batch_size")?;
    register_metric(
        Box::new(metrics.stream_chunks.clone()),
        "langserve_stream_chunks_total",
    )?;
    register_metric(Box::new(metrics.error_counter.clone()), "langserve_errors_total")?;

    let encoder = TextEncoder::new();
    let metric_families = temp_registry.gather();
    let mut buffer = Vec::new();

    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|e| format!("Failed to encode metrics: {e}"))?;

    String::from_utf8(buffer).map_err(|e| format!("Failed to convert metrics to string: {e}"))
}

/// Helper to record a successful request
pub fn record_request(endpoint: &str, duration_seconds: f64) {
    let metrics = get_or_init_metrics();
    metrics
        .request_counter
        .with_label_values(&[endpoint, "success"])
        .inc();
    metrics.request_duration.observe(duration_seconds);
}

/// Helper to record an error
pub fn record_error(endpoint: &str, error_type: &str) {
    let metrics = get_or_init_metrics();
    metrics
        .request_counter
        .with_label_values(&[endpoint, "error"])
        .inc();
    metrics
        .error_counter
        .with_label_values(&[error_type, endpoint])
        .inc();
}

/// Helper to record batch size
pub fn record_batch_size(size: usize) {
    let metrics = get_or_init_metrics();
    metrics.batch_size.observe(size as f64);
}

/// Helper to record stream chunks
pub fn record_stream_chunks(chunks: usize) {
    let metrics = get_or_init_metrics();
    metrics.stream_chunks.observe(chunks as f64);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registration() {
        // Trigger metric creation by recording test data
        record_request("test", 0.1);
        record_error("test", "test_error");
        record_batch_size(10);
        record_stream_chunks(5);

        // Get metrics from the module-specific export
        let metrics = get_langserve_metrics_only().unwrap();
        assert!(!metrics.is_empty(), "Metrics should not be empty");

        // Verify LangServe-specific metrics are present
        assert!(
            metrics.contains("langserve_requests_total"),
            "Missing langserve_requests_total"
        );
        assert!(
            metrics.contains("langserve_request_duration_seconds"),
            "Missing langserve_request_duration_seconds"
        );
        assert!(
            metrics.contains("langserve_batch_size"),
            "Missing langserve_batch_size"
        );
        assert!(
            metrics.contains("langserve_stream_chunks_total"),
            "Missing langserve_stream_chunks_total"
        );
        assert!(
            metrics.contains("langserve_errors_total"),
            "Missing langserve_errors_total"
        );
    }

    #[test]
    fn test_record_request() {
        record_request("test_endpoint", 0.123);

        let metrics = get_langserve_metrics_only().unwrap();
        assert!(metrics.contains("langserve_requests_total"));
        assert!(metrics.contains("test_endpoint"));
        assert!(metrics.contains("success"));
    }

    #[test]
    fn test_record_error() {
        record_error("test_endpoint", "validation_error");

        let metrics = get_langserve_metrics_only().unwrap();
        assert!(metrics.contains("langserve_errors_total"));
        assert!(metrics.contains("validation_error"));
    }

    #[test]
    fn test_record_batch_size() {
        record_batch_size(10);

        let metrics = get_langserve_metrics_only().unwrap();
        assert!(metrics.contains("langserve_batch_size"));
    }

    #[test]
    fn test_record_stream_chunks() {
        record_stream_chunks(50);

        let metrics = get_langserve_metrics_only().unwrap();
        assert!(metrics.contains("langserve_stream_chunks_total"));
    }

    #[test]
    fn test_get_metrics_format() {
        let metrics = get_langserve_metrics_only().unwrap();

        // Check Prometheus format
        assert!(metrics.contains("# HELP"));
        assert!(metrics.contains("# TYPE"));
        assert!(metrics.contains("langserve_"));
    }

    #[test]
    fn test_unified_registry_export() {
        // Record some metrics
        record_request("unified_test", 0.5);

        // Get metrics from the unified registry
        let unified_metrics = get_metrics().unwrap();

        // Should contain LangServe metrics
        assert!(
            unified_metrics.contains("langserve_"),
            "Unified export should contain langserve metrics"
        );
    }
}
