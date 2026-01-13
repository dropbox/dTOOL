// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Self-Improvement Metrics Emission.
//!
//! This module provides Prometheus metrics for observing the self-improvement
//! system itself. These metrics enable monitoring of the daemon, storage,
//! and analysis components.
//!
//! ## Metrics
//!
//! ### Daemon Metrics
//! - `dashflow_self_improve_cycles_total`: Total analysis cycles run
//! - `dashflow_self_improve_cycle_duration_seconds`: Cycle duration histogram
//! - `dashflow_self_improve_triggers_fired_total`: Triggers fired by type
//! - `dashflow_self_improve_traces_analyzed_total`: Traces analyzed
//!
//! ### Plan Metrics
//! - `dashflow_self_improve_plans_total`: Plans by status
//! - `dashflow_self_improve_plans_approved_total`: Plans approved
//! - `dashflow_self_improve_plans_implemented_total`: Plans successfully implemented
//!
//! ### Storage Metrics
//! - `dashflow_self_improve_storage_operations_total`: Storage operations
//! - `dashflow_self_improve_storage_size_bytes`: Current storage size
//!
//! ### Cache Metrics
//! - `dashflow_self_improve_cache_hits_total`: Cache hits
//! - `dashflow_self_improve_cache_misses_total`: Cache misses
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::self_improvement::metrics::{SelfImprovementMetrics, record_cycle_complete};
//!
//! // Initialize metrics (typically done once at startup)
//! SelfImprovementMetrics::initialize();
//!
//! // Record metrics
//! record_cycle_complete(10, 2, 0.5);
//! ```

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, Opts, Registry,
};
use std::sync::OnceLock;

/// Global self-improvement metrics instance.
static METRICS: OnceLock<SelfImprovementMetrics> = OnceLock::new();

/// Collection of Prometheus metrics for the self-improvement system.
pub struct SelfImprovementMetrics {
    // Daemon metrics
    /// Total number of self-improvement analysis cycles completed.
    pub cycles_total: Counter,
    /// Histogram of end-to-end analysis cycle durations.
    pub cycle_duration_seconds: Histogram,
    /// Count of triggers fired, labeled by trigger type.
    pub triggers_fired: CounterVec,
    /// Total number of traces analyzed across all cycles.
    pub traces_analyzed: Counter,
    /// Count of errors, labeled by component name.
    pub errors_total: CounterVec,

    // Plan metrics
    /// Total number of improvement plans generated.
    pub plans_generated: Counter,
    /// Total number of plans approved for implementation.
    pub plans_approved: Counter,
    /// Total number of plans successfully implemented.
    pub plans_implemented: Counter,
    /// Total number of plans that failed during implementation.
    pub plans_failed: Counter,
    /// Current plan counts by status (pending/approved/implemented/failed).
    pub plans_by_status: GaugeVec,

    // Storage metrics
    /// Total storage operations, labeled by operation and item type.
    pub storage_operations: CounterVec,
    /// Current storage size on disk, in bytes.
    pub storage_size_bytes: Gauge,
    /// Current stored item counts, labeled by item type.
    pub storage_items: GaugeVec,

    // Cache metrics
    /// Total cache hits.
    pub cache_hits: Counter,
    /// Total cache misses.
    pub cache_misses: Counter,
    /// Current number of cached items.
    pub cache_size: Gauge,

    // Analysis metrics
    /// Histogram of analysis durations, labeled by analysis type.
    pub analysis_duration_seconds: HistogramVec,
}

impl SelfImprovementMetrics {
    /// Create a new metrics instance and register with the provided registry.
    ///
    /// # Errors
    ///
    /// Returns an error if metric registration fails.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        // Daemon metrics
        let cycles_total = Counter::with_opts(Opts::new(
            "dashflow_self_improve_cycles_total",
            "Total number of analysis cycles run",
        ))?;
        registry.register(Box::new(cycles_total.clone()))?;

        let cycle_duration_seconds = Histogram::with_opts(HistogramOpts::new(
            "dashflow_self_improve_cycle_duration_seconds",
            "Duration of analysis cycles in seconds",
        ))?;
        registry.register(Box::new(cycle_duration_seconds.clone()))?;

        let triggers_fired = CounterVec::new(
            Opts::new(
                "dashflow_self_improve_triggers_fired_total",
                "Total triggers fired by type",
            ),
            &["trigger_type"],
        )?;
        registry.register(Box::new(triggers_fired.clone()))?;

        let traces_analyzed = Counter::with_opts(Opts::new(
            "dashflow_self_improve_traces_analyzed_total",
            "Total number of traces analyzed",
        ))?;
        registry.register(Box::new(traces_analyzed.clone()))?;

        let errors_total = CounterVec::new(
            Opts::new(
                "dashflow_self_improve_errors_total",
                "Total errors by component",
            ),
            &["component"],
        )?;
        registry.register(Box::new(errors_total.clone()))?;

        // Plan metrics
        let plans_generated = Counter::with_opts(Opts::new(
            "dashflow_self_improve_plans_generated_total",
            "Total plans generated",
        ))?;
        registry.register(Box::new(plans_generated.clone()))?;

        let plans_approved = Counter::with_opts(Opts::new(
            "dashflow_self_improve_plans_approved_total",
            "Total plans approved",
        ))?;
        registry.register(Box::new(plans_approved.clone()))?;

        let plans_implemented = Counter::with_opts(Opts::new(
            "dashflow_self_improve_plans_implemented_total",
            "Total plans successfully implemented",
        ))?;
        registry.register(Box::new(plans_implemented.clone()))?;

        let plans_failed = Counter::with_opts(Opts::new(
            "dashflow_self_improve_plans_failed_total",
            "Total plans that failed",
        ))?;
        registry.register(Box::new(plans_failed.clone()))?;

        let plans_by_status = GaugeVec::new(
            Opts::new(
                "dashflow_self_improve_plans_by_status",
                "Current plan count by status",
            ),
            &["status"],
        )?;
        registry.register(Box::new(plans_by_status.clone()))?;

        // Storage metrics
        let storage_operations = CounterVec::new(
            Opts::new(
                "dashflow_self_improve_storage_operations_total",
                "Total storage operations by type",
            ),
            &["operation", "item_type"],
        )?;
        registry.register(Box::new(storage_operations.clone()))?;

        let storage_size_bytes = Gauge::with_opts(Opts::new(
            "dashflow_self_improve_storage_size_bytes",
            "Current storage size in bytes",
        ))?;
        registry.register(Box::new(storage_size_bytes.clone()))?;

        let storage_items = GaugeVec::new(
            Opts::new(
                "dashflow_self_improve_storage_items",
                "Current item count by type",
            ),
            &["item_type"],
        )?;
        registry.register(Box::new(storage_items.clone()))?;

        // Cache metrics
        let cache_hits = Counter::with_opts(Opts::new(
            "dashflow_self_improve_cache_hits_total",
            "Total cache hits",
        ))?;
        registry.register(Box::new(cache_hits.clone()))?;

        let cache_misses = Counter::with_opts(Opts::new(
            "dashflow_self_improve_cache_misses_total",
            "Total cache misses",
        ))?;
        registry.register(Box::new(cache_misses.clone()))?;

        let cache_size = Gauge::with_opts(Opts::new(
            "dashflow_self_improve_cache_size",
            "Current number of items in cache",
        ))?;
        registry.register(Box::new(cache_size.clone()))?;

        // Analysis metrics
        let analysis_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "dashflow_self_improve_analysis_duration_seconds",
                "Duration of analysis by type",
            ),
            &["analysis_type"],
        )?;
        registry.register(Box::new(analysis_duration_seconds.clone()))?;

        Ok(Self {
            cycles_total,
            cycle_duration_seconds,
            triggers_fired,
            traces_analyzed,
            errors_total,
            plans_generated,
            plans_approved,
            plans_implemented,
            plans_failed,
            plans_by_status,
            storage_operations,
            storage_size_bytes,
            storage_items,
            cache_hits,
            cache_misses,
            cache_size,
            analysis_duration_seconds,
        })
    }

    /// Initialize the global metrics instance.
    ///
    /// This should be called once at application startup. Subsequent calls
    /// are no-ops.
    pub fn initialize() {
        // Delegate to global() which handles initialization
        let _ = Self::global();
    }

    /// Get the global metrics instance, initializing if necessary.
    pub fn global() -> &'static Self {
        METRICS.get_or_init(|| {
            let registry = prometheus::default_registry();
            Self::new(registry).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to initialize self-improvement metrics");
                // Return a dummy metrics instance that won't record anything.
                // SAFETY: A fresh Registry::new() has no existing metrics, so registration
                // cannot fail due to conflicts. If this panics, it indicates a programming
                // bug in metric definitions (e.g., invalid metric names), not a runtime issue.
                #[allow(clippy::expect_used)]
                Self::new(&Registry::new()).expect("Fresh registry cannot have metric conflicts")
            })
        })
    }
}

// ============================================================================
// Convenience Functions for Recording Metrics
// ============================================================================

/// Record completion of an analysis cycle.
pub fn record_cycle_complete(traces_analyzed: u64, triggers_fired: usize, duration_seconds: f64) {
    let metrics = SelfImprovementMetrics::global();
    metrics.cycles_total.inc();
    metrics.traces_analyzed.inc_by(traces_analyzed as f64);
    metrics.cycle_duration_seconds.observe(duration_seconds);

    // Update trigger count
    if triggers_fired > 0 {
        tracing::debug!(
            traces = traces_analyzed,
            triggers = triggers_fired,
            duration_s = duration_seconds,
            "Analysis cycle completed"
        );
    }
}

/// Record a trigger being fired.
pub fn record_trigger_fired(trigger_type: &str) {
    let metrics = SelfImprovementMetrics::global();
    metrics
        .triggers_fired
        .with_label_values(&[trigger_type])
        .inc();
}

/// Record an error by component.
pub fn record_error(component: &str) {
    let metrics = SelfImprovementMetrics::global();
    metrics.errors_total.with_label_values(&[component]).inc();
}

/// Record plan generation.
pub fn record_plan_generated() {
    let metrics = SelfImprovementMetrics::global();
    metrics.plans_generated.inc();
}

/// Record plan approval.
pub fn record_plan_approved() {
    let metrics = SelfImprovementMetrics::global();
    metrics.plans_approved.inc();
}

/// Record plan implementation.
pub fn record_plan_implemented() {
    let metrics = SelfImprovementMetrics::global();
    metrics.plans_implemented.inc();
}

/// Record plan failure.
pub fn record_plan_failed() {
    let metrics = SelfImprovementMetrics::global();
    metrics.plans_failed.inc();
}

/// Update plan status gauge.
pub fn update_plan_counts(pending: u64, approved: u64, implemented: u64, failed: u64) {
    let metrics = SelfImprovementMetrics::global();
    metrics
        .plans_by_status
        .with_label_values(&["pending"])
        .set(pending as f64);
    metrics
        .plans_by_status
        .with_label_values(&["approved"])
        .set(approved as f64);
    metrics
        .plans_by_status
        .with_label_values(&["implemented"])
        .set(implemented as f64);
    metrics
        .plans_by_status
        .with_label_values(&["failed"])
        .set(failed as f64);
}

/// Record a storage operation.
pub fn record_storage_operation(operation: &str, item_type: &str) {
    let metrics = SelfImprovementMetrics::global();
    metrics
        .storage_operations
        .with_label_values(&[operation, item_type])
        .inc();
}

/// Update storage size.
pub fn update_storage_size(size_bytes: u64) {
    let metrics = SelfImprovementMetrics::global();
    metrics.storage_size_bytes.set(size_bytes as f64);
}

/// Update storage item counts.
pub fn update_storage_items(reports: u64, plans: u64, hypotheses: u64) {
    let metrics = SelfImprovementMetrics::global();
    metrics
        .storage_items
        .with_label_values(&["reports"])
        .set(reports as f64);
    metrics
        .storage_items
        .with_label_values(&["plans"])
        .set(plans as f64);
    metrics
        .storage_items
        .with_label_values(&["hypotheses"])
        .set(hypotheses as f64);
}

/// Record a cache hit.
pub fn record_cache_hit() {
    let metrics = SelfImprovementMetrics::global();
    metrics.cache_hits.inc();
}

/// Record a cache miss.
pub fn record_cache_miss() {
    let metrics = SelfImprovementMetrics::global();
    metrics.cache_misses.inc();
}

/// Update cache size.
pub fn update_cache_size(size: u64) {
    let metrics = SelfImprovementMetrics::global();
    metrics.cache_size.set(size as f64);
}

/// Record analysis duration by type.
pub fn record_analysis_duration(analysis_type: &str, duration_seconds: f64) {
    let metrics = SelfImprovementMetrics::global();
    metrics
        .analysis_duration_seconds
        .with_label_values(&[analysis_type])
        .observe(duration_seconds);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        // Create a fresh registry for testing
        let registry = Registry::new();
        let metrics = SelfImprovementMetrics::new(&registry).unwrap();

        // Verify counters work
        metrics.cycles_total.inc();
        assert_eq!(metrics.cycles_total.get(), 1.0);

        metrics.plans_generated.inc();
        assert_eq!(metrics.plans_generated.get(), 1.0);
    }

    #[test]
    fn test_trigger_metrics() {
        let registry = Registry::new();
        let metrics = SelfImprovementMetrics::new(&registry).unwrap();

        metrics
            .triggers_fired
            .with_label_values(&["slow_node"])
            .inc();
        metrics
            .triggers_fired
            .with_label_values(&["high_error_rate"])
            .inc();
        metrics
            .triggers_fired
            .with_label_values(&["slow_node"])
            .inc();

        let slow_node_count = metrics
            .triggers_fired
            .with_label_values(&["slow_node"])
            .get();
        assert_eq!(slow_node_count, 2.0);

        let error_rate_count = metrics
            .triggers_fired
            .with_label_values(&["high_error_rate"])
            .get();
        assert_eq!(error_rate_count, 1.0);
    }

    #[test]
    fn test_plan_status_gauges() {
        let registry = Registry::new();
        let metrics = SelfImprovementMetrics::new(&registry).unwrap();

        metrics
            .plans_by_status
            .with_label_values(&["pending"])
            .set(5.0);
        metrics
            .plans_by_status
            .with_label_values(&["approved"])
            .set(3.0);
        metrics
            .plans_by_status
            .with_label_values(&["implemented"])
            .set(10.0);

        assert_eq!(
            metrics
                .plans_by_status
                .with_label_values(&["pending"])
                .get(),
            5.0
        );
        assert_eq!(
            metrics
                .plans_by_status
                .with_label_values(&["approved"])
                .get(),
            3.0
        );
        assert_eq!(
            metrics
                .plans_by_status
                .with_label_values(&["implemented"])
                .get(),
            10.0
        );
    }

    #[test]
    fn test_storage_operations() {
        let registry = Registry::new();
        let metrics = SelfImprovementMetrics::new(&registry).unwrap();

        metrics
            .storage_operations
            .with_label_values(&["save", "report"])
            .inc();
        metrics
            .storage_operations
            .with_label_values(&["save", "plan"])
            .inc();
        metrics
            .storage_operations
            .with_label_values(&["load", "plan"])
            .inc();

        let save_reports = metrics
            .storage_operations
            .with_label_values(&["save", "report"])
            .get();
        assert_eq!(save_reports, 1.0);
    }

    #[test]
    fn test_cache_metrics() {
        let registry = Registry::new();
        let metrics = SelfImprovementMetrics::new(&registry).unwrap();

        metrics.cache_hits.inc_by(10.0);
        metrics.cache_misses.inc_by(2.0);
        metrics.cache_size.set(100.0);

        assert_eq!(metrics.cache_hits.get(), 10.0);
        assert_eq!(metrics.cache_misses.get(), 2.0);
        assert_eq!(metrics.cache_size.get(), 100.0);
    }

    #[test]
    fn test_histogram_metrics() {
        let registry = Registry::new();
        let metrics = SelfImprovementMetrics::new(&registry).unwrap();

        metrics.cycle_duration_seconds.observe(0.5);
        metrics.cycle_duration_seconds.observe(1.0);
        metrics.cycle_duration_seconds.observe(0.3);

        // Just verify it doesn't panic
        let _count = metrics.cycle_duration_seconds.get_sample_count();
    }
}
