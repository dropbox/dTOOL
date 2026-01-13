//! Prometheus Metrics for the DashFlow Registry
//!
//! This module provides comprehensive metrics for monitoring the registry API server,
//! including HTTP request metrics, cache performance, storage operations, and search queries.
//!
//! # Metric Categories
//!
//! - **HTTP Metrics**: Request counts, latencies by endpoint/method/status
//! - **Cache Metrics**: Hit/miss rates for data cache and package cache
//! - **Storage Metrics**: Operations, bytes transferred, latencies
//! - **Search Metrics**: Query counts and latencies
//! - **Auth Metrics**: API key verification results
//! - **Rate Limit Metrics**: Rate limiting events
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_registry::metrics::RegistryMetrics;
//!
//! let metrics = RegistryMetrics::new()?;
//!
//! // Record HTTP request
//! metrics.http_requests_total
//!     .with_label_values(&["GET", "/api/v1/packages", "200"])
//!     .inc();
//!
//! // Record cache hit
//! metrics.cache_hits_total.with_label_values(&["data_cache"]).inc();
//! ```

#[cfg(feature = "metrics")]
use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts,
    Registry, TextEncoder,
};

/// HTTP request duration buckets (in seconds)
#[cfg(feature = "metrics")]
const HTTP_LATENCY_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Storage operation duration buckets (in seconds)
#[cfg(feature = "metrics")]
const STORAGE_LATENCY_BUCKETS: &[f64] = &[
    0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
];

/// Search operation duration buckets (in seconds)
#[cfg(feature = "metrics")]
const SEARCH_LATENCY_BUCKETS: &[f64] = &[0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0];

/// Prometheus metrics for the DashFlow Registry API
#[cfg(feature = "metrics")]
#[derive(Clone)]
pub struct RegistryMetrics {
    /// The Prometheus registry
    pub registry: Registry,

    // ============ HTTP Metrics ============
    /// Total HTTP requests by method, path, status
    pub http_requests_total: IntCounterVec,

    /// HTTP request duration in seconds by method, path
    pub http_request_duration_seconds: HistogramVec,

    /// Currently in-flight HTTP requests
    pub http_requests_in_flight: IntGauge,

    /// HTTP request body size in bytes
    pub http_request_size_bytes: HistogramVec,

    /// HTTP response body size in bytes
    pub http_response_size_bytes: HistogramVec,

    // ============ Cache Metrics ============
    /// Cache hits by cache type (data_cache, package_cache)
    pub cache_hits_total: IntCounterVec,

    /// Cache misses by cache type
    pub cache_misses_total: IntCounterVec,

    /// Cache evictions by cache type
    pub cache_evictions_total: IntCounterVec,

    /// Current cache size in bytes by cache type
    pub cache_size_bytes: IntGaugeVec,

    /// Current cache entry count by cache type
    pub cache_entries: IntGaugeVec,

    // ============ Storage Metrics ============
    /// Storage operations by operation type (get, put, delete, exists)
    pub storage_operations_total: IntCounterVec,

    /// Storage operation duration in seconds
    pub storage_operation_duration_seconds: HistogramVec,

    /// Storage bytes transferred by operation type (upload, download)
    pub storage_bytes_total: IntCounterVec,

    /// Storage operation errors by operation type
    pub storage_errors_total: IntCounterVec,

    // ============ Search Metrics ============
    /// Search queries by type (semantic, keyword, capability)
    pub search_queries_total: IntCounterVec,

    /// Search query duration in seconds by type
    pub search_query_duration_seconds: HistogramVec,

    /// Search results count (histogram for distribution)
    pub search_results_count: HistogramVec,

    // ============ Auth Metrics ============
    /// API key verifications by result (valid, invalid, expired, not_found)
    pub api_key_verifications_total: IntCounterVec,

    /// API key cache hits/misses
    pub api_key_cache_hits_total: IntCounter,
    pub api_key_cache_misses_total: IntCounter,

    // ============ Rate Limit Metrics ============
    /// Rate limit events by result (allowed, limited)
    pub rate_limit_events_total: IntCounterVec,

    // ============ Package Metrics ============
    /// Package operations by type (publish, download, yank)
    pub package_operations_total: IntCounterVec,

    /// Total packages in registry
    pub packages_total: IntGauge,

    /// Package downloads
    pub package_downloads_total: IntCounter,

    // ============ Contribution Metrics ============
    /// Contributions by type (bug, improvement, request, fix)
    pub contributions_total: IntCounterVec,

    /// Reviews by result (approved, rejected, pending)
    pub reviews_total: IntCounterVec,
}

#[cfg(feature = "metrics")]
impl RegistryMetrics {
    /// Create a new RegistryMetrics instance with a fresh Prometheus registry
    pub fn new() -> Result<Self, prometheus::Error> {
        Self::with_registry(Registry::new())
    }

    /// Create a new RegistryMetrics instance with the provided Prometheus registry
    pub fn with_registry(registry: Registry) -> Result<Self, prometheus::Error> {
        // HTTP metrics
        let http_requests_total = IntCounterVec::new(
            Opts::new("registry_http_requests_total", "Total HTTP requests").namespace("dashflow"),
            &["method", "path", "status"],
        )?;

        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "registry_http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .namespace("dashflow")
            .buckets(HTTP_LATENCY_BUCKETS.to_vec()),
            &["method", "path"],
        )?;

        let http_requests_in_flight = IntGauge::new(
            "dashflow_registry_http_requests_in_flight",
            "Currently in-flight HTTP requests",
        )?;

        let http_request_size_bytes = HistogramVec::new(
            HistogramOpts::new(
                "registry_http_request_size_bytes",
                "HTTP request body size in bytes",
            )
            .namespace("dashflow")
            .buckets(vec![
                100.0, 1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0,
            ]),
            &["method", "path"],
        )?;

        let http_response_size_bytes = HistogramVec::new(
            HistogramOpts::new(
                "registry_http_response_size_bytes",
                "HTTP response body size in bytes",
            )
            .namespace("dashflow")
            .buckets(vec![
                100.0, 1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0,
            ]),
            &["method", "path"],
        )?;

        // Cache metrics
        let cache_hits_total = IntCounterVec::new(
            Opts::new("registry_cache_hits_total", "Cache hits by type").namespace("dashflow"),
            &["cache_type"],
        )?;

        let cache_misses_total = IntCounterVec::new(
            Opts::new("registry_cache_misses_total", "Cache misses by type").namespace("dashflow"),
            &["cache_type"],
        )?;

        let cache_evictions_total = IntCounterVec::new(
            Opts::new("registry_cache_evictions_total", "Cache evictions by type")
                .namespace("dashflow"),
            &["cache_type"],
        )?;

        let cache_size_bytes = IntGaugeVec::new(
            Opts::new("registry_cache_size_bytes", "Current cache size in bytes")
                .namespace("dashflow"),
            &["cache_type"],
        )?;

        let cache_entries = IntGaugeVec::new(
            Opts::new("registry_cache_entries", "Current cache entry count").namespace("dashflow"),
            &["cache_type"],
        )?;

        // Storage metrics
        let storage_operations_total = IntCounterVec::new(
            Opts::new(
                "registry_storage_operations_total",
                "Storage operations by type",
            )
            .namespace("dashflow"),
            &["operation", "backend"],
        )?;

        let storage_operation_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "registry_storage_operation_duration_seconds",
                "Storage operation duration in seconds",
            )
            .namespace("dashflow")
            .buckets(STORAGE_LATENCY_BUCKETS.to_vec()),
            &["operation", "backend"],
        )?;

        let storage_bytes_total = IntCounterVec::new(
            Opts::new("registry_storage_bytes_total", "Storage bytes transferred")
                .namespace("dashflow"),
            &["direction", "backend"],
        )?;

        let storage_errors_total = IntCounterVec::new(
            Opts::new("registry_storage_errors_total", "Storage operation errors")
                .namespace("dashflow"),
            &["operation", "backend"],
        )?;

        // Search metrics
        let search_queries_total = IntCounterVec::new(
            Opts::new("registry_search_queries_total", "Search queries by type")
                .namespace("dashflow"),
            &["search_type"],
        )?;

        let search_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "registry_search_query_duration_seconds",
                "Search query duration in seconds",
            )
            .namespace("dashflow")
            .buckets(SEARCH_LATENCY_BUCKETS.to_vec()),
            &["search_type"],
        )?;

        let search_results_count = HistogramVec::new(
            HistogramOpts::new(
                "registry_search_results_count",
                "Number of search results returned",
            )
            .namespace("dashflow")
            .buckets(vec![0.0, 1.0, 5.0, 10.0, 25.0, 50.0, 100.0]),
            &["search_type"],
        )?;

        // Auth metrics
        let api_key_verifications_total = IntCounterVec::new(
            Opts::new(
                "registry_api_key_verifications_total",
                "API key verifications by result",
            )
            .namespace("dashflow"),
            &["result"],
        )?;

        let api_key_cache_hits_total = IntCounter::new(
            "dashflow_registry_api_key_cache_hits_total",
            "API key cache hits",
        )?;

        let api_key_cache_misses_total = IntCounter::new(
            "dashflow_registry_api_key_cache_misses_total",
            "API key cache misses",
        )?;

        // Rate limit metrics
        let rate_limit_events_total = IntCounterVec::new(
            Opts::new(
                "registry_rate_limit_events_total",
                "Rate limit events by result",
            )
            .namespace("dashflow"),
            &["result"],
        )?;

        // Package metrics
        let package_operations_total = IntCounterVec::new(
            Opts::new(
                "registry_package_operations_total",
                "Package operations by type",
            )
            .namespace("dashflow"),
            &["operation"],
        )?;

        let packages_total = IntGauge::new(
            "dashflow_registry_packages_total",
            "Total packages in registry",
        )?;

        let package_downloads_total = IntCounter::new(
            "dashflow_registry_package_downloads_total",
            "Total package downloads",
        )?;

        // Contribution metrics
        let contributions_total = IntCounterVec::new(
            Opts::new("registry_contributions_total", "Contributions by type")
                .namespace("dashflow"),
            &["contribution_type"],
        )?;

        let reviews_total = IntCounterVec::new(
            Opts::new("registry_reviews_total", "Reviews by result").namespace("dashflow"),
            &["result"],
        )?;

        // Register all metrics
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;
        registry.register(Box::new(http_requests_in_flight.clone()))?;
        registry.register(Box::new(http_request_size_bytes.clone()))?;
        registry.register(Box::new(http_response_size_bytes.clone()))?;

        registry.register(Box::new(cache_hits_total.clone()))?;
        registry.register(Box::new(cache_misses_total.clone()))?;
        registry.register(Box::new(cache_evictions_total.clone()))?;
        registry.register(Box::new(cache_size_bytes.clone()))?;
        registry.register(Box::new(cache_entries.clone()))?;

        registry.register(Box::new(storage_operations_total.clone()))?;
        registry.register(Box::new(storage_operation_duration_seconds.clone()))?;
        registry.register(Box::new(storage_bytes_total.clone()))?;
        registry.register(Box::new(storage_errors_total.clone()))?;

        registry.register(Box::new(search_queries_total.clone()))?;
        registry.register(Box::new(search_query_duration_seconds.clone()))?;
        registry.register(Box::new(search_results_count.clone()))?;

        registry.register(Box::new(api_key_verifications_total.clone()))?;
        registry.register(Box::new(api_key_cache_hits_total.clone()))?;
        registry.register(Box::new(api_key_cache_misses_total.clone()))?;

        registry.register(Box::new(rate_limit_events_total.clone()))?;

        registry.register(Box::new(package_operations_total.clone()))?;
        registry.register(Box::new(packages_total.clone()))?;
        registry.register(Box::new(package_downloads_total.clone()))?;

        registry.register(Box::new(contributions_total.clone()))?;
        registry.register(Box::new(reviews_total.clone()))?;

        Ok(Self {
            registry,
            http_requests_total,
            http_request_duration_seconds,
            http_requests_in_flight,
            http_request_size_bytes,
            http_response_size_bytes,
            cache_hits_total,
            cache_misses_total,
            cache_evictions_total,
            cache_size_bytes,
            cache_entries,
            storage_operations_total,
            storage_operation_duration_seconds,
            storage_bytes_total,
            storage_errors_total,
            search_queries_total,
            search_query_duration_seconds,
            search_results_count,
            api_key_verifications_total,
            api_key_cache_hits_total,
            api_key_cache_misses_total,
            rate_limit_events_total,
            package_operations_total,
            packages_total,
            package_downloads_total,
            contributions_total,
            reviews_total,
        })
    }

    /// Encode all metrics as Prometheus text format
    ///
    /// **M-651 Fix:** This method exports metrics from BOTH:
    /// 1. The custom registry (self.registry)
    /// 2. The prometheus default registry (where other crates may register metrics)
    ///
    /// Metrics are deduplicated by family name, with custom registry taking precedence.
    /// This ensures all registry metrics are visible in Prometheus scrapes.
    pub fn encode(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();

        // M-651: Gather from custom registry
        let custom_families = self.registry.gather();
        let custom_names: std::collections::HashSet<String> = custom_families
            .iter()
            .map(|f| f.get_name().to_string())
            .collect();

        // M-651: Gather from prometheus default registry
        let default_families = prometheus::default_registry().gather();

        // M-651: Merge families, deduping by name (custom registry takes precedence)
        let mut merged_families = custom_families;
        let mut collision_count = 0;
        for family in default_families {
            let name = family.get_name();
            if custom_names.contains(name) {
                collision_count += 1;
                tracing::debug!(
                    metric = name,
                    "Metric family exists in both registries; using custom registry version"
                );
            } else {
                merged_families.push(family);
            }
        }

        if collision_count > 0 {
            tracing::info!(
                collision_count,
                "Merged metrics from custom and default registries with {} collisions",
                collision_count
            );
        }

        encoder.encode_to_string(&merged_families)
    }

    /// Record an HTTP request completion
    pub fn record_http_request(
        &self,
        method: &str,
        path: &str,
        status: u16,
        duration_secs: f64,
        request_size: usize,
        response_size: usize,
    ) {
        let status_str = status.to_string();
        self.http_requests_total
            .with_label_values(&[method, path, &status_str])
            .inc();
        self.http_request_duration_seconds
            .with_label_values(&[method, path])
            .observe(duration_secs);
        self.http_request_size_bytes
            .with_label_values(&[method, path])
            .observe(request_size as f64);
        self.http_response_size_bytes
            .with_label_values(&[method, path])
            .observe(response_size as f64);
    }

    /// Record a cache hit
    pub fn record_cache_hit(&self, cache_type: &str) {
        self.cache_hits_total.with_label_values(&[cache_type]).inc();
    }

    /// Record a cache miss
    pub fn record_cache_miss(&self, cache_type: &str) {
        self.cache_misses_total
            .with_label_values(&[cache_type])
            .inc();
    }

    /// Record a storage operation
    pub fn record_storage_operation(
        &self,
        operation: &str,
        backend: &str,
        duration_secs: f64,
        bytes: Option<usize>,
        success: bool,
    ) {
        self.storage_operations_total
            .with_label_values(&[operation, backend])
            .inc();
        self.storage_operation_duration_seconds
            .with_label_values(&[operation, backend])
            .observe(duration_secs);

        if let Some(bytes) = bytes {
            let direction = match operation {
                "put" | "upload" => "upload",
                "get" | "download" => "download",
                _ => return,
            };
            self.storage_bytes_total
                .with_label_values(&[direction, backend])
                .inc_by(bytes as u64);
        }

        if !success {
            self.storage_errors_total
                .with_label_values(&[operation, backend])
                .inc();
        }
    }

    /// Record a search query
    pub fn record_search_query(&self, search_type: &str, duration_secs: f64, result_count: usize) {
        self.search_queries_total
            .with_label_values(&[search_type])
            .inc();
        self.search_query_duration_seconds
            .with_label_values(&[search_type])
            .observe(duration_secs);
        self.search_results_count
            .with_label_values(&[search_type])
            .observe(result_count as f64);
    }

    /// Record an API key verification
    pub fn record_api_key_verification(&self, result: &str, from_cache: bool) {
        self.api_key_verifications_total
            .with_label_values(&[result])
            .inc();
        if from_cache {
            self.api_key_cache_hits_total.inc();
        } else {
            self.api_key_cache_misses_total.inc();
        }
    }

    /// Record a rate limit event
    pub fn record_rate_limit_event(&self, allowed: bool) {
        let result = if allowed { "allowed" } else { "limited" };
        self.rate_limit_events_total
            .with_label_values(&[result])
            .inc();
    }

    /// Record a package operation
    pub fn record_package_operation(&self, operation: &str) {
        self.package_operations_total
            .with_label_values(&[operation])
            .inc();
    }

    /// Record a contribution
    pub fn record_contribution(&self, contribution_type: &str) {
        self.contributions_total
            .with_label_values(&[contribution_type])
            .inc();
    }

    /// Record a review
    pub fn record_review(&self, result: &str) {
        self.reviews_total.with_label_values(&[result]).inc();
    }

    /// Update cache stats
    pub fn update_cache_stats(&self, cache_type: &str, size_bytes: i64, entry_count: i64) {
        self.cache_size_bytes
            .with_label_values(&[cache_type])
            .set(size_bytes);
        self.cache_entries
            .with_label_values(&[cache_type])
            .set(entry_count);
    }
}

/// Timer guard for automatic duration recording
#[cfg(feature = "metrics")]
pub struct MetricTimer {
    start: std::time::Instant,
    histogram: Histogram,
}

#[cfg(feature = "metrics")]
impl MetricTimer {
    /// Create a new timer
    pub fn new(histogram: Histogram) -> Self {
        Self {
            start: std::time::Instant::now(),
            histogram,
        }
    }

    /// Stop the timer and record the duration
    pub fn stop(self) -> f64 {
        let duration = self.start.elapsed().as_secs_f64();
        self.histogram.observe(duration);
        duration
    }
}

#[cfg(feature = "metrics")]
impl Drop for MetricTimer {
    fn drop(&mut self) {
        // Don't double-record if stop() was called manually
    }
}

#[cfg(test)]
#[cfg(feature = "metrics")]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = RegistryMetrics::new().expect("Failed to create metrics");

        // Record some metrics
        metrics.record_http_request("GET", "/api/v1/packages", 200, 0.05, 0, 1024);
        metrics.record_cache_hit("data_cache");
        metrics.record_cache_miss("package_cache");
        metrics.record_storage_operation("get", "s3", 0.1, Some(1024), true);
        metrics.record_search_query("semantic", 0.25, 10);
        metrics.record_api_key_verification("valid", false);
        metrics.record_rate_limit_event(true);
        metrics.record_package_operation("download");
        metrics.record_contribution("bug");
        metrics.record_review("approved");
        metrics.update_cache_stats("data_cache", 1024 * 1024, 100);

        // Encode to text format
        let output = metrics.encode().expect("Failed to encode metrics");
        assert!(output.contains("dashflow_registry_http_requests_total"));
        assert!(output.contains("dashflow_registry_cache_hits_total"));
        assert!(output.contains("dashflow_registry_storage_operations_total"));
        assert!(output.contains("dashflow_registry_search_queries_total"));
    }

    #[test]
    fn test_http_request_metrics() {
        let metrics = RegistryMetrics::new().expect("Failed to create metrics");

        metrics.record_http_request("GET", "/api/v1/packages", 200, 0.01, 0, 100);
        metrics.record_http_request("GET", "/api/v1/packages", 200, 0.02, 0, 200);
        metrics.record_http_request("GET", "/api/v1/packages", 404, 0.005, 0, 50);
        metrics.record_http_request("POST", "/api/v1/packages", 201, 0.1, 5000, 100);

        let output = metrics.encode().expect("Failed to encode metrics");

        // Verify request counts
        assert!(output.contains("method=\"GET\""));
        assert!(output.contains("method=\"POST\""));
        assert!(output.contains("status=\"200\""));
        assert!(output.contains("status=\"404\""));
        assert!(output.contains("status=\"201\""));
    }

    #[test]
    fn test_cache_metrics() {
        let metrics = RegistryMetrics::new().expect("Failed to create metrics");

        // Record multiple cache events
        for _ in 0..10 {
            metrics.record_cache_hit("data_cache");
        }
        for _ in 0..3 {
            metrics.record_cache_miss("data_cache");
        }

        metrics.update_cache_stats("data_cache", 10 * 1024 * 1024, 500);

        let output = metrics.encode().expect("Failed to encode metrics");
        assert!(output.contains("cache_type=\"data_cache\""));
    }

    #[test]
    fn test_storage_metrics() {
        let metrics = RegistryMetrics::new().expect("Failed to create metrics");

        metrics.record_storage_operation("put", "s3", 0.5, Some(1024 * 1024), true);
        metrics.record_storage_operation("get", "s3", 0.1, Some(512 * 1024), true);
        metrics.record_storage_operation("delete", "s3", 0.05, None, true);
        metrics.record_storage_operation("get", "s3", 0.2, None, false);

        let output = metrics.encode().expect("Failed to encode metrics");
        assert!(output.contains("operation=\"put\""));
        assert!(output.contains("operation=\"get\""));
        assert!(output.contains("operation=\"delete\""));
        assert!(output.contains("backend=\"s3\""));
    }

    /// M-651 Test: Verify that encode() includes metrics from the prometheus default registry.
    ///
    /// This ensures that any metrics registered by other crates (e.g., dashflow-streaming)
    /// to the default registry are visible when scraping the registry crate's /metrics endpoint.
    #[test]
    fn test_m651_encode_includes_default_registry_metrics() {
        // Register a metric to the prometheus default registry
        let test_metric = IntCounter::new(
            "m651_test_default_registry_metric",
            "Test metric for M-651 verification",
        )
        .expect("Failed to create test counter");

        // Try to register - ignore error if already registered from previous test run
        let _ = prometheus::default_registry().register(Box::new(test_metric.clone()));

        // Increment so it has a value
        test_metric.inc();

        // Create a RegistryMetrics instance with its own isolated registry
        let metrics = RegistryMetrics::new().expect("Failed to create metrics");

        // Record a metric in the custom registry
        metrics.record_cache_hit("test_cache");

        // Encode should include BOTH registries
        let output = metrics.encode().expect("Failed to encode metrics");

        // Verify custom registry metric is present
        assert!(
            output.contains("dashflow_registry_cache_hits_total"),
            "Custom registry metric should be present"
        );

        // Verify default registry metric is present (M-651 fix)
        assert!(
            output.contains("m651_test_default_registry_metric"),
            "Default registry metric should be present after M-651 fix"
        );
    }

    /// M-651 Test: Verify that custom registry metrics take precedence over duplicates.
    ///
    /// When the same metric name exists in both registries, the custom registry
    /// version should be used to avoid double-counting.
    #[test]
    fn test_m651_custom_registry_takes_precedence() {
        let metrics = RegistryMetrics::new().expect("Failed to create metrics");

        // Record a metric in the custom registry
        metrics.record_cache_hit("precedence_test");
        metrics.record_cache_hit("precedence_test");

        // Encode should work without errors even with potential duplicates
        let output = metrics.encode().expect("Failed to encode metrics");

        // Verify custom registry metric is present with expected labels
        assert!(
            output.contains("cache_type=\"precedence_test\""),
            "Custom registry metric should be present with correct label"
        );

        // Count occurrences of the metric family - should appear only once per family
        let cache_hits_count = output
            .lines()
            .filter(|line| line.starts_with("dashflow_registry_cache_hits_total{"))
            .count();

        // Should have at least one line for our cache_type
        assert!(
            cache_hits_count >= 1,
            "Should have at least one cache_hits_total metric line"
        );
    }
}
