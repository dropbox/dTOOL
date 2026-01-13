// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Streaming Metrics Constants
//
// M-624: Centralize all metric name strings to prevent duplication and typos.
// All dashstream_* metrics should be defined here and imported where needed.

//! Prometheus metric name constants for DashFlow Streaming.
//!
//! This module centralizes all metric names to ensure consistency across the crate.
//! Metrics follow Prometheus naming conventions:
//! - Counters end with `_total`
//! - Histograms end with `_ms`, `_bytes`, `_seconds`, etc. (unit suffix)
//! - Gauges have no special suffix
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_streaming::metrics_constants::*;
//!
//! let counter = crate::metrics_utils::counter(
//!     METRIC_MESSAGES_SENT_TOTAL,
//!     "Total messages sent",
//! );
//! ```

// ============================================================================
// Counter Metrics (_total suffix)
// ============================================================================

/// Total number of messages successfully sent to Kafka.
pub const METRIC_MESSAGES_SENT_TOTAL: &str = "dashstream_messages_sent_total";

/// Total number of messages successfully received from Kafka.
pub const METRIC_MESSAGES_RECEIVED_TOTAL: &str = "dashstream_messages_received_total";

/// Total number of Kafka send failures.
pub const METRIC_SEND_FAILURES_TOTAL: &str = "dashstream_send_failures_total";

/// Total number of Kafka send retries.
pub const METRIC_SEND_RETRIES_TOTAL: &str = "dashstream_send_retries_total";

/// Total number of message decode failures.
pub const METRIC_DECODE_FAILURES_TOTAL: &str = "dashstream_decode_failures_total";

/// Total number of Kafka fetch failures.
pub const METRIC_FETCH_FAILURES_TOTAL: &str = "dashstream_fetch_failures_total";

/// Total number of invalid message payloads (missing/oversized).
pub const METRIC_INVALID_PAYLOADS_TOTAL: &str = "dashstream_invalid_payloads_total";

/// Total sequence gaps detected (message loss).
pub const METRIC_SEQUENCE_GAPS_TOTAL: &str = "dashstream_sequence_gaps_total";

/// Total duplicate sequences detected.
pub const METRIC_SEQUENCE_DUPLICATES_TOTAL: &str = "dashstream_sequence_duplicates_total";

/// Total out-of-order sequences detected.
pub const METRIC_SEQUENCE_REORDERS_TOTAL: &str = "dashstream_sequence_reorders_total";

/// Total number of successful local offset checkpoint writes.
pub const METRIC_OFFSET_CHECKPOINT_WRITES_TOTAL: &str = "dashstream_offset_checkpoint_writes_total";

/// Total number of local offset checkpoint write failures.
pub const METRIC_OFFSET_CHECKPOINT_FAILURES_TOTAL: &str =
    "dashstream_offset_checkpoint_failures_total";

/// Total number of message compression failures (fell back to uncompressed).
pub const METRIC_COMPRESSION_FAILURES_TOTAL: &str = "dashstream_compression_failures_total";

/// Total number of messages sent to DLQ.
pub const METRIC_DLQ_SENDS_TOTAL: &str = "dashstream_dlq_sends_total";

/// Total number of DLQ send failures.
pub const METRIC_DLQ_SEND_FAILURES_TOTAL: &str = "dashstream_dlq_send_failures_total";

/// Total number of DLQ messages dropped due to backpressure.
pub const METRIC_DLQ_DROPPED_TOTAL: &str = "dashstream_dlq_dropped_total";

/// Total number of DLQ send retries.
pub const METRIC_DLQ_SEND_RETRIES_TOTAL: &str = "dashstream_dlq_send_retries_total";

/// Total messages rejected due to rate limiting.
pub const METRIC_RATE_LIMIT_EXCEEDED_TOTAL: &str = "dashstream_rate_limit_exceeded_total";

/// Total messages allowed by rate limiter.
pub const METRIC_RATE_LIMIT_ALLOWED_TOTAL: &str = "dashstream_rate_limit_allowed_total";

/// Total Redis connection errors in rate limiting (M-647: component-scoped).
pub const METRIC_RATE_LIMITER_REDIS_ERRORS_TOTAL: &str =
    "dashstream_rate_limiter_redis_errors_total";

/// Total StateDiff operations with unsupported encodings (MSGPACK/PROTOBUF).
pub const METRIC_UNSUPPORTED_ENCODING_TOTAL: &str = "dashstream_unsupported_encoding_total";

// ============================================================================
// Gauge Metrics (no _total suffix)
// ============================================================================

/// DEPRECATED: Batch size gauge (M-697).
/// This metric is defined but never actually set by any code path.
/// Use dashflow-observability's langserve_batch_size or component-specific metrics instead.
#[deprecated(
    since = "1.11.0",
    note = "Metric defined but never implemented - use component-specific metrics (M-697)"
)]
pub const METRIC_BATCH_SIZE: &str = "dashstream_batch_size";

/// DEPRECATED: Queue depth gauge (M-697).
/// This metric is defined but never actually set by any code path.
/// Use dashflow-observability's queue_depth gauge instead.
#[deprecated(
    since = "1.11.0",
    note = "Metric defined but never implemented - use dashflow-observability queue_depth (M-697)"
)]
pub const METRIC_QUEUE_DEPTH: &str = "dashstream_queue_depth";

/// DEPRECATED: Consumer lag gauge (M-697).
/// This metric is defined but never actually set by any code path.
/// Use websocket_kafka_consumer_lag from dashflow-observability instead.
#[deprecated(
    since = "1.11.0",
    note = "Metric defined but never implemented - use websocket_kafka_consumer_lag (M-697)"
)]
pub const METRIC_CONSUMER_LAG: &str = "dashstream_consumer_lag";

/// DEPRECATED: Process-local loss rate (M-649).
/// This metric is meaningless in distributed deployments.
#[deprecated(
    since = "1.11.0",
    note = "Process-local loss computation is meaningless in distributed systems (M-649)"
)]
pub const METRIC_MESSAGE_LOSS_RATE: &str = "dashstream_message_loss_rate";

// ============================================================================
// Histogram Metrics (unit suffix: _ms, _bytes, _seconds, etc.)
// ============================================================================

/// Size distribution of detected sequence gaps.
pub const METRIC_SEQUENCE_GAP_SIZE: &str = "dashstream_sequence_gap_size";

/// Redis operation latency in milliseconds for rate limiting (M-647: component-scoped).
pub const METRIC_RATE_LIMITER_REDIS_LATENCY_MS: &str = "dashstream_rate_limiter_redis_latency_ms";

// ============================================================================
// WebSocket Server Metrics (defined in dashflow-observability)
// These constants are provided for reference but the actual metrics are
// defined in websocket_server.rs. If you need to reference these from
// dashflow-streaming, use these constants.
// ============================================================================

/// Total Redis errors in WebSocket server (M-647: component-scoped).
pub const METRIC_WEBSOCKET_REDIS_ERRORS_TOTAL: &str = "dashstream_websocket_redis_errors_total";

/// Redis latency histogram in WebSocket server (M-647: component-scoped).
pub const METRIC_WEBSOCKET_REDIS_LATENCY_MS: &str = "dashstream_websocket_redis_latency_ms";

/// WebSocket retry count histogram.
pub const METRIC_WS_RETRY_COUNT: &str = "dashstream_ws_retry_count";

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify all counter names end with _total (Prometheus convention).
    #[test]
    fn test_counter_names_end_with_total() {
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
            METRIC_RATE_LIMITER_REDIS_ERRORS_TOTAL,
            METRIC_UNSUPPORTED_ENCODING_TOTAL,
            METRIC_WEBSOCKET_REDIS_ERRORS_TOTAL,
        ];

        for name in &counters {
            assert!(
                name.ends_with("_total"),
                "Counter '{}' must end with '_total'",
                name
            );
        }
    }

    /// Verify gauge names do NOT end with _total.
    /// M-697: Allow deprecated metrics for validation
    #[test]
    #[allow(deprecated)]
    fn test_gauge_names_not_total() {
        let gauges = [
            METRIC_BATCH_SIZE,
            METRIC_QUEUE_DEPTH,
            METRIC_CONSUMER_LAG,
            METRIC_MESSAGE_LOSS_RATE,
        ];

        for name in &gauges {
            assert!(
                !name.ends_with("_total"),
                "Gauge '{}' must NOT end with '_total'",
                name
            );
        }
    }

    /// Verify all metric names start with dashstream_ prefix.
    /// M-697: Allow deprecated metrics for validation
    #[test]
    #[allow(deprecated)]
    fn test_all_metrics_have_prefix() {
        let all_metrics = [
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
            METRIC_RATE_LIMITER_REDIS_ERRORS_TOTAL,
            METRIC_UNSUPPORTED_ENCODING_TOTAL,
            METRIC_BATCH_SIZE,
            METRIC_QUEUE_DEPTH,
            METRIC_CONSUMER_LAG,
            METRIC_MESSAGE_LOSS_RATE,
            METRIC_SEQUENCE_GAP_SIZE,
            METRIC_RATE_LIMITER_REDIS_LATENCY_MS,
            METRIC_WEBSOCKET_REDIS_ERRORS_TOTAL,
            METRIC_WEBSOCKET_REDIS_LATENCY_MS,
            METRIC_WS_RETRY_COUNT,
        ];

        for name in &all_metrics {
            assert!(
                name.starts_with("dashstream_"),
                "Metric '{}' must start with 'dashstream_' prefix",
                name
            );
        }
    }
}
