//! DashStream header processing for the WebSocket server.
//!
//! This module handles extraction and validation of DashStream protocol headers,
//! including sequence validation and end-to-end latency tracking.

use dashflow_streaming::consumer::{SequenceError, SequenceValidator};
use prometheus::{Histogram, HistogramVec, IntCounter};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Process a DashStream header, validating sequences and tracking latency.
///
/// Returns (thread_id, sequence) tuple for downstream use.
///
/// # Sequence Validation (Issue #11)
///
/// Validates message sequences to detect:
/// - Gaps: Missing messages in a sequence
/// - Duplicates: Same sequence number received twice
/// - Reorders: Out-of-order message delivery
///
/// # End-to-End Latency (Issue #5)
///
/// Calculates latency from producer timestamp to consumer receive time.
/// M-644: Guards against clock skew (negative or >60s latency).
pub async fn process_dashstream_header(
    header: &dashflow_streaming::Header,
    sequence_validator: &Arc<RwLock<SequenceValidator>>,
    sequence_gaps: &Option<IntCounter>,
    sequence_gap_size: &Option<Histogram>, // M-1115: histogram for gap severity
    sequence_duplicates: &Option<IntCounter>,
    sequence_reorders: &Option<IntCounter>,
    e2e_latency_histogram: &Option<HistogramVec>,
    clock_skew_events_total: &Option<IntCounter>,
) -> (Option<String>, Option<u64>) {
    // M-1116: Enforce max thread_id length to prevent DoS via huge thread_ids
    const MAX_THREAD_ID_LENGTH: usize = 256;
    let raw_thread_id = header.thread_id.trim();
    let thread_id = if raw_thread_id.is_empty() {
        None
    } else if raw_thread_id.len() <= MAX_THREAD_ID_LENGTH {
        Some(raw_thread_id.to_string())
    } else {
        // Truncate but maintain uniqueness via hash suffix
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        raw_thread_id.hash(&mut hasher);
        let hash = hasher.finish();
        let prefix = &raw_thread_id[..MAX_THREAD_ID_LENGTH - 20]; // Leave room for hash suffix
        let bounded = format!("{}...{:016x}", prefix, hash);
        tracing::warn!(
            original_len = raw_thread_id.len(),
            bounded_len = bounded.len(),
            "Thread ID exceeded max length, using bounded representation"
        );
        Some(bounded)
    };
    let sequence = (header.sequence > 0).then_some(header.sequence);
    let producer_timestamp_us = header.timestamp_us;

    // Issue #11: Validate sequence for detecting message loss, duplicates, reordering
    if let (Some(thread_id), Some(sequence)) = (thread_id.as_deref(), sequence) {
        let mut validator = sequence_validator.write().await;
        match validator.validate(thread_id, sequence) {
            Ok(()) => {}
            Err(SequenceError::Gap {
                gap_size,
                expected,
                received,
                ..
            }) => {
                tracing::warn!(
                    thread_id = %thread_id,
                    expected = expected,
                    received = received,
                    gap_size = gap_size,
                    "SEQUENCE GAP detected"
                );
                if let Some(ref metric) = sequence_gaps {
                    metric.inc();
                }
                // M-1115: Record gap size in histogram for severity analysis
                if let Some(ref histogram) = sequence_gap_size {
                    histogram.observe(gap_size as f64);
                }
            }
            Err(SequenceError::Duplicate {
                thread_id: ref dup_thread_id,
                sequence,
                expected,
            }) => {
                tracing::warn!(
                    thread_id = %dup_thread_id,
                    sequence = sequence,
                    expected = expected,
                    "DUPLICATE sequence detected"
                );
                if let Some(ref metric) = sequence_duplicates {
                    metric.inc();
                }
            }
            Err(SequenceError::Reordered {
                thread_id: ref reorder_thread_id,
                sequence,
                expected,
            }) => {
                tracing::warn!(
                    thread_id = %reorder_thread_id,
                    sequence = sequence,
                    expected = expected,
                    "REORDERED sequence detected"
                );
                if let Some(ref metric) = sequence_reorders {
                    metric.inc();
                }
            }
            // Handle future SequenceError variants (#[non_exhaustive])
            Err(e) => {
                tracing::warn!("Unknown sequence error: {:?}", e);
            }
        }
    }

    // Issue #5: Calculate E2E latency from header timestamp_us
    // M-644: Guard against clock skew (negative or >60s latency)
    if let Some(ref histogram) = e2e_latency_histogram {
        if producer_timestamp_us > 0 {
            let consume_timestamp_us = chrono::Utc::now().timestamp_micros();
            let latency_us = consume_timestamp_us - producer_timestamp_us;

            const LATENCY_SANITY_THRESHOLD_US: i64 = 60_000_000; // 60 seconds
            if !(0..=LATENCY_SANITY_THRESHOLD_US).contains(&latency_us) {
                if let Some(ref skew_counter) = clock_skew_events_total {
                    skew_counter.inc();
                }
                tracing::warn!(
                    latency_us = latency_us,
                    producer_ts = producer_timestamp_us,
                    consumer_ts = consume_timestamp_us,
                    "Clock skew detected in E2E latency ({})",
                    if latency_us < 0 { "negative" } else { ">60s" }
                );
            } else {
                let latency_ms = (latency_us as f64) / 1000.0;
                histogram
                    .with_label_values(&["producer_to_consumer"])
                    .observe(latency_ms);

                // M-1002: Replace hot-path println with tracing::trace (per-message logging).
                // Use trace level so it's disabled by default; only enable for debugging.
                tracing::trace!(
                    latency_ms = latency_ms,
                    producer_ts_us = producer_timestamp_us,
                    consumer_ts_us = consume_timestamp_us,
                    "E2E latency"
                );
            }
        }
    }

    (thread_id, sequence)
}
