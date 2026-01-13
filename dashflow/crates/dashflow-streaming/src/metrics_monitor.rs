// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Metrics Monitoring and Loss Detection for DashFlow Streaming
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

//! # DashFlow Streaming Metrics Monitoring
//!
//! Provides Prometheus metrics export for DashFlow Streaming.
//!
//! ## Features
//!
//! - **Metrics Aggregation**: Gather producer and consumer metrics
//! - **Prometheus Export**: Export metrics for Prometheus scraping
//!
//! ## Deprecation Notice (M-649)
//!
//! The `dashstream_message_loss_rate` metric and associated functions (`calculate_loss_rate`,
//! `check_for_high_loss`) are **deprecated** and will be removed in a future release.
//!
//! **Reason:** In a distributed system, the producer and consumer run in different processes.
//! This metric computes loss from whatever counters exist in the current process, which is
//! meaningless for end-to-end loss detection. The producer process sees `sent` but not
//! `received`; the consumer process sees `received` but not `sent`.
//!
//! **Alternative:** Use Prometheus-level alerting that compares producer and consumer metrics
//! across processes, or implement consumer-side sequence gap detection.
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_streaming::metrics_monitor::get_metrics_text;
//!
//! // Get current metrics
//! let metrics = get_metrics_text();
//! println!("{}", metrics);
//! ```

use std::sync::LazyLock;
use prometheus::{Encoder, Gauge, TextEncoder};
use tracing::{info, warn};

// M-624: Use centralized constants
#[allow(deprecated)]
use crate::metrics_constants::{
    METRIC_DECODE_FAILURES_TOTAL, METRIC_MESSAGE_LOSS_RATE, METRIC_MESSAGES_RECEIVED_TOTAL,
    METRIC_MESSAGES_SENT_TOTAL, METRIC_SEND_FAILURES_TOTAL,
};

/// Deprecated: This metric is process-local and meaningless in distributed deployments.
/// See module documentation for details. Will be removed in a future release.
#[deprecated(
    since = "1.11.0",
    note = "Process-local loss computation is meaningless in distributed systems (M-649)"
)]
#[allow(deprecated)]
static MESSAGE_LOSS_RATE: LazyLock<Gauge> = LazyLock::new(|| {
    crate::metrics_utils::gauge(
        METRIC_MESSAGE_LOSS_RATE,
        "DEPRECATED: Process-local loss rate (0.0-1.0) - meaningless in distributed deployments",
    )
});

/// Get all metrics in Prometheus text format
pub fn get_metrics_text() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        warn!(error = %e, "Failed to encode Prometheus metrics");
        return "# Error encoding metrics".to_string();
    }
    match String::from_utf8(buffer) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "Failed to convert metrics to UTF-8");
            "# Error converting metrics to UTF-8".to_string()
        }
    }
}

/// Calculate message loss rate (sent - received) / sent
/// Returns value between 0.0 and 1.0
///
/// # Deprecation Warning
///
/// This function is **deprecated** because it computes loss from process-local counters,
/// which is meaningless in distributed systems where producer and consumer run in
/// different processes. Use Prometheus-level alerting that compares metrics across
/// processes instead.
#[deprecated(
    since = "1.11.0",
    note = "Process-local loss computation is meaningless in distributed systems (M-649)"
)]
pub fn calculate_loss_rate() -> f64 {
    let metric_families = prometheus::gather();

    let mut sent = 0.0;
    let mut received = 0.0;
    let mut send_failures = 0.0;
    let mut decode_failures = 0.0;

    for mf in &metric_families {
        let name = mf.get_name();
        if name == METRIC_MESSAGES_SENT_TOTAL {
            sent = mf
                .get_metric()
                .iter()
                .map(|m| m.get_counter().get_value())
                .sum();
        } else if name == METRIC_MESSAGES_RECEIVED_TOTAL {
            received = mf
                .get_metric()
                .iter()
                .map(|m| m.get_counter().get_value())
                .sum();
        } else if name == METRIC_SEND_FAILURES_TOTAL {
            send_failures = mf
                .get_metric()
                .iter()
                .map(|m| m.get_counter().get_value())
                .sum();
        } else if name == METRIC_DECODE_FAILURES_TOTAL {
            decode_failures = mf
                .get_metric()
                .iter()
                .map(|m| m.get_counter().get_value())
                .sum();
        }
    }

    // Calculate loss rate.
    // Treat send failures as attempted-but-not-delivered messages.
    let total_attempted = sent + send_failures;
    let total_successful = received;

    let loss_rate = if total_attempted > 0.0 {
        let ratio = total_successful / total_attempted;
        if ratio >= 1.0 {
            0.0
        } else {
            1.0 - ratio
        }
    } else {
        0.0
    };

    // Update the gauge metric (deprecated but still populated for backwards compatibility)
    #[allow(deprecated)]
    MESSAGE_LOSS_RATE.set(loss_rate);

    info!(
        sent_successful = sent,
        send_failures = send_failures,
        attempted_total = total_attempted,
        received = received,
        decode_failures = decode_failures,
        loss_rate_percent = format!("{:.2}", loss_rate * 100.0),
        "Message loss metrics calculated"
    );

    loss_rate
}

/// Check for high message loss and print alert
///
/// # Deprecation Warning
///
/// This function is **deprecated** because it relies on `calculate_loss_rate()` which
/// computes loss from process-local counters. See `calculate_loss_rate()` for details.
#[deprecated(
    since = "1.11.0",
    note = "Process-local loss computation is meaningless in distributed systems (M-649)"
)]
pub fn check_for_high_loss(threshold: f64) {
    #[allow(deprecated)]
    let loss_rate = calculate_loss_rate();

    if loss_rate > threshold {
        warn!(
            loss_rate_percent = format!("{:.1}", loss_rate * 100.0),
            threshold_percent = format!("{:.1}", threshold * 100.0),
            "High message loss detected"
        );
    } else {
        info!(
            loss_rate_percent = format!("{:.2}", loss_rate * 100.0),
            threshold_percent = format!("{:.1}", threshold * 100.0),
            "Message loss within acceptable range"
        );
    }
}

/// Snapshot of selected Prometheus counters and derived metrics.
pub struct MetricsSnapshot {
    /// Total messages successfully sent by this process.
    pub messages_sent: f64,
    /// Total messages successfully received by this process.
    pub messages_received: f64,
    /// Total send failures observed by this process.
    pub send_failures: f64,
    /// Total decode failures observed by this process.
    pub decode_failures: f64,
    /// Derived loss rate computed from send/receive/failure counters.
    pub loss_rate: f64,
}

impl MetricsSnapshot {
    /// Gather a point-in-time snapshot from the global Prometheus registry.
    pub fn new() -> Self {
        let metric_families = prometheus::gather();

        let mut sent = 0.0;
        let mut received = 0.0;
        let mut send_failures = 0.0;
        let mut decode_failures = 0.0;

        for mf in &metric_families {
            let name = mf.get_name();
            if name == METRIC_MESSAGES_SENT_TOTAL {
                sent = mf
                    .get_metric()
                    .iter()
                    .map(|m| m.get_counter().get_value())
                    .sum();
            } else if name == METRIC_MESSAGES_RECEIVED_TOTAL {
                received = mf
                    .get_metric()
                    .iter()
                    .map(|m| m.get_counter().get_value())
                    .sum();
            } else if name == METRIC_SEND_FAILURES_TOTAL {
                send_failures = mf
                    .get_metric()
                    .iter()
                    .map(|m| m.get_counter().get_value())
                    .sum();
            } else if name == METRIC_DECODE_FAILURES_TOTAL {
                decode_failures = mf
                    .get_metric()
                    .iter()
                    .map(|m| m.get_counter().get_value())
                    .sum();
            }
        }

        let attempted = sent + send_failures;
        let loss_rate = if attempted > 0.0 {
            let ratio = received / attempted;
            if ratio >= 1.0 {
                0.0
            } else {
                1.0 - ratio
            }
        } else {
            0.0
        };

        Self {
            messages_sent: sent,
            messages_received: received,
            send_failures,
            decode_failures,
            loss_rate,
        }
    }
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use super::*;

    #[test]
    fn test_metrics_text_format() {
        let metrics = get_metrics_text();
        assert!(!metrics.is_empty(), "Metrics should not be empty");
        // Basic smoke test - should produce valid Prometheus text format
        // Note: dashstream_message_loss_rate is deprecated (M-649) but still exported
    }

    #[test]
    #[allow(deprecated)]
    fn test_loss_rate_calculation_no_messages() {
        // Test deprecated function still works (will be removed in future)
        // With no messages, loss rate should be 0
        let loss_rate = calculate_loss_rate();
        assert!((0.0..=1.0).contains(&loss_rate));
    }

    #[test]
    fn test_metrics_snapshot() {
        let snapshot = MetricsSnapshot::new();
        assert!((0.0..=1.0).contains(&snapshot.loss_rate));
    }
}
