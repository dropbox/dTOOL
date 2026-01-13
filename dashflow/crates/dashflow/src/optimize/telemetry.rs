// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Prometheus telemetry for DashOptimize.
//!
//! This module provides standardized metrics for all optimizers, enabling
//! monitoring of optimization runs, performance, and resource usage.
//!
//! ## Metrics
//!
//! ### Execution Metrics
//! - `dashflow_optimizer_runs_total`: Total optimization runs by optimizer type
//! - `dashflow_optimizer_duration_seconds`: Optimization duration histogram
//! - `dashflow_optimizer_iterations_total`: Total iterations performed
//! - `dashflow_optimizer_candidates_total`: Total candidates evaluated
//!
//! ### Score Metrics
//! - `dashflow_optimizer_initial_score`: Initial score before optimization
//! - `dashflow_optimizer_final_score`: Final score after optimization
//! - `dashflow_optimizer_improvement`: Score improvement (final - initial)
//!
//! ### Error Metrics
//! - `dashflow_optimizer_errors_total`: Errors by optimizer and error type
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::optimize::telemetry::{OptimizerMetrics, record_optimization_start, record_optimization_complete};
//!
//! // Initialize metrics (typically done once at startup)
//! OptimizerMetrics::initialize();
//!
//! // Record optimization lifecycle
//! record_optimization_start("bootstrap");
//! // ... run optimization ...
//! record_optimization_complete("bootstrap", 10, 50, 0.65, 0.92, 15.5);
//! ```

use prometheus::{CounterVec, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry};
use std::sync::OnceLock;

/// Global optimizer metrics instance.
static METRICS: OnceLock<OptimizerMetrics> = OnceLock::new();

/// Collection of Prometheus metrics for the optimization system.
pub struct OptimizerMetrics {
    // Execution metrics
    /// Total optimization runs by optimizer type
    pub runs_total: CounterVec,
    /// Optimization duration histogram
    pub duration_seconds: HistogramVec,
    /// Total iterations performed by optimizer type
    pub iterations_total: CounterVec,
    /// Total candidates evaluated by optimizer type
    pub candidates_total: CounterVec,

    // Score metrics
    /// Initial score before optimization
    pub initial_score: GaugeVec,
    /// Final score after optimization
    pub final_score: GaugeVec,
    /// Score improvement (final - initial)
    pub improvement: GaugeVec,

    // Error metrics
    /// Errors by optimizer and error type
    pub errors_total: CounterVec,

    // Active optimization tracking
    /// Currently running optimizations by type
    pub active_optimizations: GaugeVec,

    // Demo/example metrics
    /// Demos added during optimization
    pub demos_added_total: CounterVec,
    /// Rules generated during optimization
    pub rules_generated_total: CounterVec,
}

impl OptimizerMetrics {
    /// Create a new metrics instance and register with the provided registry.
    ///
    /// # Errors
    ///
    /// Returns an error if metric registration fails.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        // Execution metrics
        let runs_total = CounterVec::new(
            Opts::new(
                "dashflow_optimizer_runs_total",
                "Total number of optimization runs by optimizer type",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(runs_total.clone()))?;

        let duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "dashflow_optimizer_duration_seconds",
                "Duration of optimization runs in seconds",
            )
            .buckets(vec![
                1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
            ]),
            &["optimizer"],
        )?;
        registry.register(Box::new(duration_seconds.clone()))?;

        let iterations_total = CounterVec::new(
            Opts::new(
                "dashflow_optimizer_iterations_total",
                "Total iterations performed by optimizer type",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(iterations_total.clone()))?;

        let candidates_total = CounterVec::new(
            Opts::new(
                "dashflow_optimizer_candidates_total",
                "Total candidates evaluated by optimizer type",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(candidates_total.clone()))?;

        // Score metrics
        let initial_score = GaugeVec::new(
            Opts::new(
                "dashflow_optimizer_initial_score",
                "Initial score before optimization (0.0-1.0)",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(initial_score.clone()))?;

        let final_score = GaugeVec::new(
            Opts::new(
                "dashflow_optimizer_final_score",
                "Final score after optimization (0.0-1.0)",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(final_score.clone()))?;

        let improvement = GaugeVec::new(
            Opts::new(
                "dashflow_optimizer_improvement",
                "Score improvement (final - initial)",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(improvement.clone()))?;

        // Error metrics
        let errors_total = CounterVec::new(
            Opts::new(
                "dashflow_optimizer_errors_total",
                "Total errors by optimizer and error type",
            ),
            &["optimizer", "error_type"],
        )?;
        registry.register(Box::new(errors_total.clone()))?;

        // Active optimization tracking
        let active_optimizations = GaugeVec::new(
            Opts::new(
                "dashflow_optimizer_active",
                "Currently running optimizations by type",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(active_optimizations.clone()))?;

        // Demo/example metrics
        let demos_added_total = CounterVec::new(
            Opts::new(
                "dashflow_optimizer_demos_added_total",
                "Demos/examples added during optimization",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(demos_added_total.clone()))?;

        let rules_generated_total = CounterVec::new(
            Opts::new(
                "dashflow_optimizer_rules_generated_total",
                "Rules generated during optimization",
            ),
            &["optimizer"],
        )?;
        registry.register(Box::new(rules_generated_total.clone()))?;

        Ok(Self {
            runs_total,
            duration_seconds,
            iterations_total,
            candidates_total,
            initial_score,
            final_score,
            improvement,
            errors_total,
            active_optimizations,
            demos_added_total,
            rules_generated_total,
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
                tracing::warn!(error = %e, "Failed to initialize optimizer metrics");
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

/// Record the start of an optimization run.
pub fn record_optimization_start(optimizer: &str) {
    let metrics = OptimizerMetrics::global();
    metrics.runs_total.with_label_values(&[optimizer]).inc();
    metrics
        .active_optimizations
        .with_label_values(&[optimizer])
        .inc();
}

/// Maximum safe integer for f64 (2^53). Values above this may lose precision in f64 casts.
const MAX_SAFE_F64_INT: u64 = 9_007_199_254_740_992;

/// Sanitize a score value for metrics recording.
///
/// Returns 0.0 for NaN/infinite values; otherwise returns the input unchanged.
/// This ensures gauges never receive invalid f64 values.
#[inline]
fn sanitize_score(score: f64) -> f64 {
    if score.is_nan() || score.is_infinite() {
        tracing::warn!(
            value = ?score,
            "Invalid score value (NaN/inf) sanitized to 0.0 for metrics"
        );
        0.0
    } else {
        score
    }
}

/// Convert u64 to f64 with a warning if precision loss may occur.
///
/// For values > 2^53, f64 cannot represent all integers exactly.
/// This logs a warning but still performs the conversion.
#[inline]
fn u64_to_f64_checked(value: u64, context: &str) -> f64 {
    if value > MAX_SAFE_F64_INT {
        tracing::warn!(
            value,
            context,
            max_safe = MAX_SAFE_F64_INT,
            "u64 value exceeds MAX_SAFE_F64_INT; precision may be lost in metrics"
        );
    }
    value as f64
}

/// Record completion of an optimization run.
///
/// # Arguments
/// * `optimizer` - Name of the optimizer (e.g., "bootstrap", "simba", "copro")
/// * `iterations` - Number of iterations performed
/// * `candidates` - Number of candidates evaluated
/// * `initial_score` - Score before optimization (0.0-1.0). NaN/inf values are sanitized to 0.0.
/// * `final_score` - Score after optimization (0.0-1.0). NaN/inf values are sanitized to 0.0.
/// * `duration_seconds` - Total duration in seconds. NaN/inf values are sanitized to 0.0.
///
/// # Note
///
/// For `iterations` and `candidates` values > 2^53, precision loss may occur when
/// converting to f64 for Prometheus counters. A warning is logged in such cases.
pub fn record_optimization_complete(
    optimizer: &str,
    iterations: u64,
    candidates: u64,
    initial_score: f64,
    final_score: f64,
    duration_seconds: f64,
) {
    let metrics = OptimizerMetrics::global();

    // Sanitize score inputs (M-839: protect against NaN/inf)
    let initial_score = sanitize_score(initial_score);
    let final_score = sanitize_score(final_score);
    let duration_seconds = sanitize_score(duration_seconds);
    let improvement = final_score - initial_score;

    metrics
        .active_optimizations
        .with_label_values(&[optimizer])
        .dec();
    // M-840: Check for precision loss on u64 to f64 conversion
    metrics
        .iterations_total
        .with_label_values(&[optimizer])
        .inc_by(u64_to_f64_checked(iterations, "iterations"));
    metrics
        .candidates_total
        .with_label_values(&[optimizer])
        .inc_by(u64_to_f64_checked(candidates, "candidates"));
    metrics
        .initial_score
        .with_label_values(&[optimizer])
        .set(initial_score);
    metrics
        .final_score
        .with_label_values(&[optimizer])
        .set(final_score);
    metrics
        .improvement
        .with_label_values(&[optimizer])
        .set(improvement);
    metrics
        .duration_seconds
        .with_label_values(&[optimizer])
        .observe(duration_seconds);

    tracing::info!(
        optimizer,
        iterations,
        candidates,
        initial_score = %format!("{:.4}", initial_score),
        final_score = %format!("{:.4}", final_score),
        improvement = %format!("{:.4}", final_score - initial_score),
        duration_secs = %format!("{:.2}", duration_seconds),
        "Optimization completed"
    );
}

/// Record an optimization error.
///
/// # Concurrency Note (M-841)
///
/// The decrement of `active_optimizations` is not atomic with the check.
/// Under concurrent errors for the same optimizer, the active count may
/// briefly go negative. This is acceptable for Prometheus gauges which
/// support negative values, and the count will self-correct when normal
/// operations resume. The alternative (no decrement on error) would leave
/// stale active counts indefinitely.
pub fn record_error(optimizer: &str, error_type: &str) {
    let metrics = OptimizerMetrics::global();
    metrics
        .errors_total
        .with_label_values(&[optimizer, error_type])
        .inc();

    // Decrement active count if error occurred during optimization.
    // Note: This is a check-then-act pattern that is not atomic. In concurrent
    // error scenarios, the gauge may briefly become negative. We accept this
    // because:
    // 1. Prometheus gauges support negative values
    // 2. The count self-corrects with normal operations
    // 3. Leaving stale positive counts is worse than brief negative counts
    let gauge = metrics.active_optimizations.with_label_values(&[optimizer]);
    let active = gauge.get();
    if active > 0.0 {
        gauge.dec();
    } else {
        // Log when we skip decrement to aid debugging
        tracing::debug!(
            optimizer,
            error_type,
            active,
            "Skipping active_optimizations decrement (already zero or negative)"
        );
    }
}

/// Record demos added during optimization.
///
/// # Note
///
/// For values > 2^53, precision loss may occur when converting to f64
/// for Prometheus counters. A warning is logged in such cases.
pub fn record_demos_added(optimizer: &str, count: u64) {
    let metrics = OptimizerMetrics::global();
    metrics
        .demos_added_total
        .with_label_values(&[optimizer])
        .inc_by(u64_to_f64_checked(count, "demos_added"));
}

/// Record rules generated during optimization.
///
/// # Note
///
/// For values > 2^53, precision loss may occur when converting to f64
/// for Prometheus counters. A warning is logged in such cases.
pub fn record_rules_generated(optimizer: &str, count: u64) {
    let metrics = OptimizerMetrics::global();
    metrics
        .rules_generated_total
        .with_label_values(&[optimizer])
        .inc_by(u64_to_f64_checked(count, "rules_generated"));
}

/// Record iteration progress (for long-running optimizations).
pub fn record_iteration(optimizer: &str) {
    let metrics = OptimizerMetrics::global();
    metrics
        .iterations_total
        .with_label_values(&[optimizer])
        .inc();
}

/// Record candidate evaluation.
pub fn record_candidate_evaluated(optimizer: &str) {
    let metrics = OptimizerMetrics::global();
    metrics
        .candidates_total
        .with_label_values(&[optimizer])
        .inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let registry = Registry::new();
        let metrics = OptimizerMetrics::new(&registry).unwrap();

        // Verify counters work
        metrics.runs_total.with_label_values(&["bootstrap"]).inc();
        assert_eq!(
            metrics.runs_total.with_label_values(&["bootstrap"]).get(),
            1.0
        );
    }

    #[test]
    fn test_duration_histogram() {
        let registry = Registry::new();
        let metrics = OptimizerMetrics::new(&registry).unwrap();

        metrics
            .duration_seconds
            .with_label_values(&["simba"])
            .observe(45.5);
        metrics
            .duration_seconds
            .with_label_values(&["simba"])
            .observe(120.0);

        let count = metrics
            .duration_seconds
            .with_label_values(&["simba"])
            .get_sample_count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_score_gauges() {
        let registry = Registry::new();
        let metrics = OptimizerMetrics::new(&registry).unwrap();

        metrics
            .initial_score
            .with_label_values(&["copro"])
            .set(0.65);
        metrics.final_score.with_label_values(&["copro"]).set(0.92);
        metrics.improvement.with_label_values(&["copro"]).set(0.27);

        assert_eq!(
            metrics.initial_score.with_label_values(&["copro"]).get(),
            0.65
        );
        assert_eq!(
            metrics.final_score.with_label_values(&["copro"]).get(),
            0.92
        );
        assert_eq!(
            metrics.improvement.with_label_values(&["copro"]).get(),
            0.27
        );
    }

    #[test]
    fn test_error_tracking() {
        let registry = Registry::new();
        let metrics = OptimizerMetrics::new(&registry).unwrap();

        metrics
            .errors_total
            .with_label_values(&["grpo", "llm_error"])
            .inc();
        metrics
            .errors_total
            .with_label_values(&["grpo", "metric_error"])
            .inc();
        metrics
            .errors_total
            .with_label_values(&["grpo", "llm_error"])
            .inc();

        assert_eq!(
            metrics
                .errors_total
                .with_label_values(&["grpo", "llm_error"])
                .get(),
            2.0
        );
        assert_eq!(
            metrics
                .errors_total
                .with_label_values(&["grpo", "metric_error"])
                .get(),
            1.0
        );
    }

    #[test]
    fn test_active_optimizations() {
        let registry = Registry::new();
        let metrics = OptimizerMetrics::new(&registry).unwrap();

        metrics
            .active_optimizations
            .with_label_values(&["mipro"])
            .inc();
        metrics
            .active_optimizations
            .with_label_values(&["mipro"])
            .inc();
        assert_eq!(
            metrics
                .active_optimizations
                .with_label_values(&["mipro"])
                .get(),
            2.0
        );

        metrics
            .active_optimizations
            .with_label_values(&["mipro"])
            .dec();
        assert_eq!(
            metrics
                .active_optimizations
                .with_label_values(&["mipro"])
                .get(),
            1.0
        );
    }

    #[test]
    fn test_demos_and_rules_counters() {
        let registry = Registry::new();
        let metrics = OptimizerMetrics::new(&registry).unwrap();

        metrics
            .demos_added_total
            .with_label_values(&["bootstrap"])
            .inc_by(5.0);
        metrics
            .rules_generated_total
            .with_label_values(&["simba"])
            .inc_by(3.0);

        assert_eq!(
            metrics
                .demos_added_total
                .with_label_values(&["bootstrap"])
                .get(),
            5.0
        );
        assert_eq!(
            metrics
                .rules_generated_total
                .with_label_values(&["simba"])
                .get(),
            3.0
        );
    }

    // M-839: Tests for sanitize_score function
    #[test]
    fn test_sanitize_score_normal_values() {
        assert_eq!(sanitize_score(0.5), 0.5);
        assert_eq!(sanitize_score(0.0), 0.0);
        assert_eq!(sanitize_score(1.0), 1.0);
        assert_eq!(sanitize_score(-0.5), -0.5);
        assert_eq!(sanitize_score(100.0), 100.0);
    }

    #[test]
    fn test_sanitize_score_nan() {
        let result = sanitize_score(f64::NAN);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_sanitize_score_positive_infinity() {
        let result = sanitize_score(f64::INFINITY);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_sanitize_score_negative_infinity() {
        let result = sanitize_score(f64::NEG_INFINITY);
        assert_eq!(result, 0.0);
    }

    // M-840: Tests for u64_to_f64_checked function
    #[test]
    fn test_u64_to_f64_checked_small_values() {
        assert_eq!(u64_to_f64_checked(0, "test"), 0.0);
        assert_eq!(u64_to_f64_checked(100, "test"), 100.0);
        assert_eq!(u64_to_f64_checked(1_000_000, "test"), 1_000_000.0);
    }

    #[test]
    fn test_u64_to_f64_checked_boundary() {
        // MAX_SAFE_F64_INT = 2^53 = 9_007_199_254_740_992
        // Values at or below this threshold should convert exactly
        let max_safe = MAX_SAFE_F64_INT;
        let result = u64_to_f64_checked(max_safe, "test");
        assert_eq!(result as u64, max_safe);
    }

    #[test]
    fn test_u64_to_f64_checked_above_threshold() {
        // Values above MAX_SAFE_F64_INT still convert (with warning logged)
        let above_safe = MAX_SAFE_F64_INT + 1;
        let result = u64_to_f64_checked(above_safe, "test");
        // The function still returns a value, just may not be exact
        assert!(result > 0.0);
    }

    #[test]
    fn test_u64_to_f64_checked_max_u64() {
        // Even u64::MAX should convert (with warning), not panic
        let result = u64_to_f64_checked(u64::MAX, "test");
        assert!(result > 0.0);
    }
}
