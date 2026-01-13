// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow expect() in this module: used for fallback metric creation where
// the fallback name is guaranteed to be valid (e.g., "{name}_invalid").
#![allow(clippy::expect_used)]

//! Safe Prometheus metric registration helpers.
//!
//! DashFlow Streaming is a library used in multiple binaries. When multiple
//! components register the same metric name, the default Prometheus registry
//! returns an error. The upstream `register_*` macros panic on that error.
//! These helpers instead log and continue, returning an unregistered metric
//! as a fallback.

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use std::sync::LazyLock;
use prometheus::{Counter, CounterVec, Gauge, Histogram, HistogramOpts, HistogramVec, Opts};
use tracing::{debug, warn};

#[derive(Clone)]
struct CounterEntry {
    help: String,
    metric: Counter,
}

#[derive(Clone)]
struct GaugeEntry {
    help: String,
    metric: Gauge,
}

#[derive(Clone)]
struct HistogramEntry {
    help: String,
    buckets: Vec<f64>,
    metric: Histogram,
}

#[derive(Clone)]
struct CounterVecEntry {
    help: String,
    labels: Vec<String>,
    metric: CounterVec,
}

#[derive(Clone)]
struct HistogramVecEntry {
    help: String,
    buckets: Vec<f64>,
    labels: Vec<String>,
    metric: HistogramVec,
}

static COUNTERS: LazyLock<DashMap<String, CounterEntry>> = LazyLock::new(DashMap::new);
static GAUGES: LazyLock<DashMap<String, GaugeEntry>> = LazyLock::new(DashMap::new);
static HISTOGRAMS: LazyLock<DashMap<String, HistogramEntry>> = LazyLock::new(DashMap::new);
static COUNTER_VECS: LazyLock<DashMap<String, CounterVecEntry>> = LazyLock::new(DashMap::new);
static HISTOGRAM_VECS: LazyLock<DashMap<String, HistogramVecEntry>> = LazyLock::new(DashMap::new);

fn label_signature(labels: &[&str]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    labels.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn counter(name: &str, help: &str) -> Counter {
    // M-99: Validate metric naming convention
    warn_invalid_counter_name(name);

    match COUNTERS.entry(name.to_string()) {
        Entry::Occupied(entry) => {
            if entry.get().help != help {
                warn!(
                    metric = name,
                    existing_help = entry.get().help.as_str(),
                    requested_help = help,
                    "Counter help mismatch; reusing existing metric"
                );
            }
            entry.get().metric.clone()
        }
        Entry::Vacant(entry) => {
            let metric = Counter::new(name, help).unwrap_or_else(|e| {
                warn!(metric = name, error = %e, "Failed to create Counter");
                Counter::new(format!("{name}_invalid"), help)
                    .expect("fallback counter name should be valid")
            });

            if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
                match e {
                    prometheus::Error::AlreadyReg => {
                        debug!(metric = name, "Counter already registered; continuing");
                    }
                    other => {
                        warn!(
                            metric = name,
                            error = %other,
                            "Counter registration failed; continuing without global registration"
                        );
                    }
                }
            }

            entry.insert(CounterEntry {
                help: help.to_string(),
                metric: metric.clone(),
            });
            metric
        }
    }
}

pub(crate) fn gauge(name: &str, help: &str) -> Gauge {
    // M-99: Validate metric naming convention
    warn_invalid_gauge_name(name);

    match GAUGES.entry(name.to_string()) {
        Entry::Occupied(entry) => {
            if entry.get().help != help {
                warn!(
                    metric = name,
                    existing_help = entry.get().help.as_str(),
                    requested_help = help,
                    "Gauge help mismatch; reusing existing metric"
                );
            }
            entry.get().metric.clone()
        }
        Entry::Vacant(entry) => {
            let metric = Gauge::new(name, help).unwrap_or_else(|e| {
                warn!(metric = name, error = %e, "Failed to create Gauge");
                Gauge::new(format!("{name}_invalid"), help)
                    .expect("fallback gauge name should be valid")
            });

            if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
                match e {
                    prometheus::Error::AlreadyReg => {
                        debug!(metric = name, "Gauge already registered; continuing");
                    }
                    other => {
                        warn!(
                            metric = name,
                            error = %other,
                            "Gauge registration failed; continuing without global registration"
                        );
                    }
                }
            }

            entry.insert(GaugeEntry {
                help: help.to_string(),
                metric: metric.clone(),
            });
            metric
        }
    }
}

pub(crate) fn histogram(opts: HistogramOpts) -> Histogram {
    let name = opts.common_opts.name.clone();
    let help = opts.common_opts.help.clone();
    let buckets = opts.buckets.clone();

    match HISTOGRAMS.entry(name.clone()) {
        Entry::Occupied(entry) => {
            if entry.get().help != help {
                warn!(
                    metric = %name,
                    existing_help = entry.get().help.as_str(),
                    requested_help = help,
                    "Histogram help mismatch; reusing existing metric"
                );
            }
            if entry.get().buckets != buckets {
                warn!(
                    metric = %name,
                    "Histogram bucket mismatch; reusing existing metric"
                );
            }
            entry.get().metric.clone()
        }
        Entry::Vacant(entry) => {
            let metric = Histogram::with_opts(opts).unwrap_or_else(|e| {
                warn!(metric = %name, error = %e, "Failed to create Histogram");
                Histogram::with_opts(HistogramOpts::new(
                    format!("{name}_invalid"),
                    "invalid histogram",
                ))
                .expect("fallback histogram should be valid")
            });

            if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
                match e {
                    prometheus::Error::AlreadyReg => {
                        debug!(metric = %name, "Histogram already registered; continuing");
                    }
                    other => {
                        warn!(
                            metric = %name,
                            error = %other,
                            "Histogram registration failed; continuing without global registration"
                        );
                    }
                }
            }

            entry.insert(HistogramEntry {
                help,
                buckets,
                metric: metric.clone(),
            });
            metric
        }
    }
}

pub(crate) fn counter_vec(opts: Opts, labels: &[&str]) -> CounterVec {
    let name = opts.name.clone();
    let help = opts.help.clone();
    let requested_labels: Vec<String> = labels.iter().map(|s| (*s).to_string()).collect();

    // M-99: Validate metric naming convention
    warn_invalid_counter_name(&name);

    match COUNTER_VECS.entry(name.clone()) {
        Entry::Occupied(entry) => {
            if entry.get().labels != requested_labels {
                let sig = label_signature(labels);
                warn!(
                    metric = %name,
                    signature = format!("{sig:016x}"),
                    "CounterVec label names mismatch; using a fallback metric"
                );
                let fallback_name = format!("{name}_invalid_{sig:016x}");
                let metric =
                    CounterVec::new(Opts::new(fallback_name.clone(), "invalid counter vec"), labels)
                        .expect("fallback counter vec should be valid");
                if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
                    if !matches!(e, prometheus::Error::AlreadyReg) {
                        warn!(
                            metric = %fallback_name,
                            error = %e,
                            "Fallback CounterVec registration failed"
                        );
                    }
                }
                return metric;
            }
            if entry.get().help != help {
                warn!(
                    metric = %name,
                    existing_help = entry.get().help.as_str(),
                    requested_help = help.as_str(),
                    "CounterVec help mismatch; reusing existing metric"
                );
            }
            entry.get().metric.clone()
        }
        Entry::Vacant(entry) => {
            let metric = CounterVec::new(opts, labels).unwrap_or_else(|e| {
                warn!(metric = %name, error = %e, "Failed to create CounterVec");
                CounterVec::new(
                    Opts::new(format!("{name}_invalid"), "invalid counter vec"),
                    labels,
                )
                .expect("fallback counter vec should be valid")
            });

            if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
                match e {
                    prometheus::Error::AlreadyReg => {
                        debug!(metric = %name, "CounterVec already registered; continuing");
                    }
                    other => {
                        warn!(
                            metric = %name,
                            error = %other,
                            "CounterVec registration failed; continuing without global registration"
                        );
                    }
                }
            }

            entry.insert(CounterVecEntry {
                help,
                labels: requested_labels,
                metric: metric.clone(),
            });
            metric
        }
    }
}

pub(crate) fn histogram_vec(opts: HistogramOpts, labels: &[&str]) -> HistogramVec {
    let name = opts.common_opts.name.clone();
    let help = opts.common_opts.help.clone();
    let buckets = opts.buckets.clone();
    let requested_labels: Vec<String> = labels.iter().map(|s| (*s).to_string()).collect();

    match HISTOGRAM_VECS.entry(name.clone()) {
        Entry::Occupied(entry) => {
            if entry.get().labels != requested_labels {
                let sig = label_signature(labels);
                warn!(
                    metric = %name,
                    signature = format!("{sig:016x}"),
                    "HistogramVec label names mismatch; using a fallback metric"
                );
                let fallback_name = format!("{name}_invalid_{sig:016x}");
                let metric = HistogramVec::new(
                    HistogramOpts::new(fallback_name.clone(), "invalid histogram vec"),
                    labels,
                )
                .expect("fallback histogram vec should be valid");
                if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
                    if !matches!(e, prometheus::Error::AlreadyReg) {
                        warn!(
                            metric = %fallback_name,
                            error = %e,
                            "Fallback HistogramVec registration failed"
                        );
                    }
                }
                return metric;
            }
            if entry.get().help != help {
                warn!(
                    metric = %name,
                    existing_help = entry.get().help.as_str(),
                    requested_help = help.as_str(),
                    "HistogramVec help mismatch; reusing existing metric"
                );
            }
            if entry.get().buckets != buckets {
                warn!(
                    metric = %name,
                    "HistogramVec bucket mismatch; reusing existing metric"
                );
            }
            entry.get().metric.clone()
        }
        Entry::Vacant(entry) => {
            let metric = HistogramVec::new(opts, labels).unwrap_or_else(|e| {
                warn!(metric = %name, error = %e, "Failed to create HistogramVec");
                HistogramVec::new(
                    HistogramOpts::new(format!("{name}_invalid"), "invalid histogram vec"),
                    labels,
                )
                .expect("fallback histogram vec should be valid")
            });

            if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
                match e {
                    prometheus::Error::AlreadyReg => {
                        debug!(metric = %name, "HistogramVec already registered; continuing");
                    }
                    other => {
                        warn!(
                            metric = %name,
                            error = %other,
                            "HistogramVec registration failed; continuing without global registration"
                        );
                    }
                }
            }

            entry.insert(HistogramVecEntry {
                help,
                buckets,
                labels: requested_labels,
                metric: metric.clone(),
            });
            metric
        }
    }
}

// M-99: Static metric validation helpers
// These functions validate Prometheus metric naming conventions

/// Validates that a counter metric name follows Prometheus conventions.
/// Counter names should end with `_total`.
pub(crate) fn validate_counter_name(name: &str) -> bool {
    name.ends_with("_total")
}

/// Validates that a gauge metric name follows Prometheus conventions.
/// Gauge names should NOT end with `_total`.
pub(crate) fn validate_gauge_name(name: &str) -> bool {
    !name.ends_with("_total")
}

/// Validates that a histogram metric name follows Prometheus conventions.
/// Histogram names should end with a unit suffix like `_seconds`, `_bytes`, `_count`, etc.
/// Returns true if valid, false otherwise.
/// Note: This is intentionally permissive - Prometheus allows many unit suffixes.
#[allow(dead_code)] // Test infrastructure: Used in tests; may be integrated into histogram() in future
pub(crate) fn validate_histogram_name(name: &str) -> bool {
    const VALID_HISTOGRAM_SUFFIXES: &[&str] = &[
        "_seconds",
        "_bytes",
        "_count",
        "_ratio",
        "_percent",
        "_ms",
        "_messages",
        "_items",
        "_requests",
        "_operations",
        "_events",
        "_errors",
    ];
    VALID_HISTOGRAM_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// Validate a counter name and log a warning if it doesn't follow conventions.
/// This is called at metric creation time in debug builds.
#[inline]
pub(crate) fn warn_invalid_counter_name(name: &str) {
    if !validate_counter_name(name) {
        warn!(
            metric = name,
            "Counter metric name should end with '_total' per Prometheus conventions"
        );
    }
}

/// Validate a gauge name and log a warning if it doesn't follow conventions.
#[inline]
pub(crate) fn warn_invalid_gauge_name(name: &str) {
    if !validate_gauge_name(name) {
        warn!(
            metric = name,
            "Gauge metric name should NOT end with '_total' per Prometheus conventions"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_name_validation() {
        // Valid counter names
        assert!(validate_counter_name("dashstream_messages_sent_total"));
        assert!(validate_counter_name("http_requests_total"));
        assert!(validate_counter_name("errors_total"));

        // Invalid counter names (missing _total suffix)
        assert!(!validate_counter_name("dashstream_messages_sent"));
        assert!(!validate_counter_name("http_requests"));
        assert!(!validate_counter_name("errors"));
    }

    #[test]
    fn test_gauge_name_validation() {
        // Valid gauge names
        assert!(validate_gauge_name("dashstream_queue_depth"));
        assert!(validate_gauge_name("http_connections"));
        assert!(validate_gauge_name("memory_usage_bytes"));

        // Invalid gauge names (should not end with _total)
        assert!(!validate_gauge_name("dashstream_queue_depth_total"));
        assert!(!validate_gauge_name("http_connections_total"));
    }

    #[test]
    fn test_histogram_name_validation() {
        // Valid histogram names
        assert!(validate_histogram_name("http_request_duration_seconds"));
        assert!(validate_histogram_name("message_size_bytes"));
        assert!(validate_histogram_name("batch_size_count"));
        assert!(validate_histogram_name("latency_ms"));

        // Invalid histogram names (missing unit suffix)
        assert!(!validate_histogram_name("http_request_duration"));
        assert!(!validate_histogram_name("message_size"));
        assert!(!validate_histogram_name("batch"));
    }

    /// M-99: Validate all dashflow-streaming metrics follow Prometheus conventions.
    /// This test documents all metrics and validates their naming.
    /// M-624: Now uses centralized constants from metrics_constants module.
    /// M-697: Allow deprecated metrics (BATCH_SIZE, QUEUE_DEPTH, CONSUMER_LAG) for validation.
    #[test]
    #[allow(deprecated)]
    fn test_all_dashflow_streaming_metrics_follow_conventions() {
        use crate::metrics_constants::*;

        // Counter metrics (should end with _total)
        let counters = [
            METRIC_MESSAGES_SENT_TOTAL,
            METRIC_MESSAGES_RECEIVED_TOTAL,
            METRIC_SEND_FAILURES_TOTAL,
            METRIC_SEND_RETRIES_TOTAL,
            METRIC_DECODE_FAILURES_TOTAL,
            METRIC_FETCH_FAILURES_TOTAL,
            METRIC_INVALID_PAYLOADS_TOTAL,
            METRIC_SEQUENCE_GAPS_TOTAL,
            METRIC_SEQUENCE_DUPLICATES_TOTAL,
            METRIC_SEQUENCE_REORDERS_TOTAL,
            METRIC_OFFSET_CHECKPOINT_WRITES_TOTAL,
            METRIC_OFFSET_CHECKPOINT_FAILURES_TOTAL,
            METRIC_COMPRESSION_FAILURES_TOTAL,
            METRIC_DLQ_SENDS_TOTAL,
            METRIC_DLQ_SEND_FAILURES_TOTAL,
            METRIC_DLQ_DROPPED_TOTAL,
            METRIC_DLQ_SEND_RETRIES_TOTAL,
            METRIC_RATE_LIMIT_EXCEEDED_TOTAL,
            METRIC_RATE_LIMIT_ALLOWED_TOTAL,
            // M-647: Redis metrics renamed to component-scoped names
            METRIC_RATE_LIMITER_REDIS_ERRORS_TOTAL,
        ];

        for name in &counters {
            assert!(
                validate_counter_name(name),
                "Counter '{}' should end with '_total'",
                name
            );
        }

        // Gauge metrics (should NOT end with _total)
        let gauges = [
            METRIC_BATCH_SIZE,
            METRIC_QUEUE_DEPTH,
            METRIC_CONSUMER_LAG,
        ];

        for name in &gauges {
            assert!(
                validate_gauge_name(name),
                "Gauge '{}' should NOT end with '_total'",
                name
            );
        }
    }
}
