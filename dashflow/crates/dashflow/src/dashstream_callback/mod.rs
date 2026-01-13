// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for streaming callback
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! DashFlow Streaming integration for DashFlow
//!
//! Provides callbacks for streaming DashFlow execution telemetry to Kafka
//! using the DashFlow Streaming protocol.
//!
//! # Features
//!
//! - **Event Streaming**: Sends GraphEvent to Kafka as DashFlow Streaming Event messages
//! - **State Diffing**: Tracks state changes and sends incremental diffs
//! - **Checkpointing**: Periodic full state snapshots for resync after missed diffs (M-671)
//! - **Redaction**: Sensitive data (API keys, tokens) automatically redacted (M-673)
//! - **Performance**: Async, non-blocking callbacks
//! - **Multi-tenancy**: Thread ID isolation for observability
//!
//! # Best-Effort Telemetry Semantics
//!
//! **IMPORTANT**: This callback implements best-effort telemetry with bounded backpressure.
//! Under high load, telemetry events may be dropped to prevent runtime starvation.
//!
//! ## Drop Scenarios
//!
//! Messages can be dropped in two scenarios:
//! 1. **Concurrent send limit reached** (`reason="capacity_limit"`): When `max_concurrent_telemetry_sends`
//!    active sends are in flight, new events are dropped.
//! 2. **Batch queue full** (`reason="queue_full"`): When batching is enabled and the batch queue is full,
//!    new messages (events, state diffs, checkpoints) are dropped.
//!
//! ## Monitoring Drops (M-694)
//!
//! Dropped messages are tracked via:
//! - **Prometheus metric**: `dashstream_telemetry_dropped_total{message_type, reason}` - counter with labels
//!   - `message_type`: "event", "state_diff", "checkpoint"
//!   - `reason`: "queue_full", "capacity_limit"
//! - **Method**: `telemetry_dropped_count()` - returns total drops for this callback instance
//! - **Logs**: Warning logged every 100 drops (to avoid log spam)
//!
//! ## Alerting Recommendation
//!
//! Set up alerting on `dashstream_telemetry_dropped_total`:
//! ```promql
//! # Alert on any drops
//! rate(dashstream_telemetry_dropped_total[5m]) > 0
//!
//! # Alert on specific message types (e.g., state_diff drops are more critical)
//! rate(dashstream_telemetry_dropped_total{message_type="state_diff"}[5m]) > 0
//! ```
//!
//! ## Tuning
//!
//! If drops are occurring, consider:
//! - Increase `max_concurrent_telemetry_sends` (default: 64)
//! - Enable batching with larger `telemetry_batch_size` (default: 1, no batching)
//! - Scale Kafka broker capacity
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::dashstream_callback::DashStreamCallback;
//! use dashflow::StateGraph;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create DashFlow Streaming callback
//!     let callback = DashStreamCallback::new(
//!         "localhost:9092",
//!         "dashstream-events",
//!         "my-tenant",
//!         "session-123"
//!     ).await?;
//!
//!     // Create graph and add nodes
//!     let mut graph = StateGraph::new();
//!     // ... add nodes ...
//!
//!     // Compile with callback and invoke
//!     let compiled = graph.compile()?.with_callback(callback);
//!     compiled.invoke(initial_state).await?;
//!
//!     Ok(())
//! }
//! ```

use crate::constants::{
    DEFAULT_BROADCAST_CHANNEL_CAPACITY, DEFAULT_FLUSH_TIMEOUT_SECS as FLUSH_TIMEOUT_SECS,
    DEFAULT_MAX_CHANNEL_CAPACITY, DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS as TELEMETRY_BATCH_TIMEOUT_MS,
    SHORT_TIMEOUT,
};
use crate::core::config_loader::env_vars::{
    env_bool, env_string_or_default, env_u64, DASHFLOW_FLUSH_TIMEOUT_SECS, DASHFLOW_STATE_REDACT,
    KAFKA_BROKERS, KAFKA_TOPIC,
};
use crate::event::{EdgeType, EventCallback, GraphEvent};
use crate::state::GraphState;
use dashflow_streaming::{
    attribute_value,
    diff::{diff_states, DiffResult},
    metric_value,
    producer::{DashStreamProducer, ProducerConfig},
    AttributeValue, Checkpoint, CompressionInfo, CompressionType, Event, EventBatch, EventType,
    Header, MessageType, MetricValue, Metrics, StateDiff,
};
use parking_lot::Mutex;
use prometheus::{IntCounterVec, IntGauge, Opts};
use regex::Regex;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::task::JoinHandle;

/// M-694: Telemetry drop counter with message_type and reason labels.
/// Message types: "event", "state_diff", "checkpoint"
/// Reasons: "queue_full" (channel try_send failed), "capacity_limit" (semaphore at max)
static TELEMETRY_DROPPED_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let metric = IntCounterVec::new(
        Opts::new(
            "dashstream_telemetry_dropped_total",
            "Total telemetry messages dropped due to backpressure",
        ),
        &["message_type", "reason"],
    )
    .expect("dashstream_telemetry_dropped_total counter name should be valid");
    if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
        match e {
            prometheus::Error::AlreadyReg => {
                tracing::debug!(
                    "dashstream_telemetry_dropped_total already registered; continuing"
                );
            }
            other => {
                tracing::warn!(
                    error = %other,
                    "Failed to register dashstream_telemetry_dropped_total; continuing without global registration"
                );
            }
        }
    }
    metric
});

/// M-713: Kafka send failure counter with message_type label.
/// Tracks actual send failures (Kafka unavailable, network error, etc.)
/// as opposed to drops (which are due to local backpressure).
/// Message types: "event", "event_batch", "state_diff", "checkpoint"
static TELEMETRY_SEND_FAILURES_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let metric = IntCounterVec::new(
        Opts::new(
            "dashstream_telemetry_send_failures_total",
            "Total telemetry messages that failed to send to Kafka",
        ),
        &["message_type"],
    )
    .expect("dashstream_telemetry_send_failures_total counter name should be valid");
    if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
        match e {
            prometheus::Error::AlreadyReg => {
                tracing::debug!(
                    "dashstream_telemetry_send_failures_total already registered; continuing"
                );
            }
            other => {
                tracing::warn!(
                    error = %other,
                    "Failed to register dashstream_telemetry_send_failures_total; continuing without global registration"
                );
            }
        }
    }
    metric
});

/// M-699: State diff degraded mode counter with reason label.
/// Tracks when state diff processing enters a degraded mode:
/// - "state_too_large_precheck": bincode estimate exceeded limit
/// - "serialization_failed": JSON serialization error
/// - "state_too_large_postcheck": actual JSON exceeded limit
/// - "full_state_fallback": diff algorithm determined full state is needed (patch too large/complex)
/// - "patch_serialization_failed": patch_to_proto failed, falling back to full state
static STATE_DIFF_DEGRADED_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let metric = IntCounterVec::new(
        Opts::new(
            "dashstream_state_diff_degraded_total",
            "State diff degraded mode events (fallback to full state, size exceeded, etc.)",
        ),
        &["reason"],
    )
    .expect("dashstream_state_diff_degraded_total counter name should be valid");
    if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
        match e {
            prometheus::Error::AlreadyReg => {
                tracing::debug!(
                    "dashstream_state_diff_degraded_total already registered; continuing"
                );
            }
            other => {
                tracing::warn!(
                    error = %other,
                    "Failed to register dashstream_state_diff_degraded_total; continuing without global registration"
                );
            }
        }
    }
    metric
});

/// M-1040: Self-observability gauge for in-flight telemetry permits.
/// Shows how many of the max_concurrent_telemetry_sends permits are currently in use.
/// High values approaching max indicate backpressure risk - drops may start occurring.
static TELEMETRY_INFLIGHT_PERMITS: LazyLock<IntGauge> = LazyLock::new(|| {
    let metric = IntGauge::new(
        "dashstream_telemetry_inflight_permits",
        "Number of in-flight telemetry permits currently in use (approaching max = backpressure risk)",
    )
    .expect("dashstream_telemetry_inflight_permits gauge name should be valid");
    if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
        match e {
            prometheus::Error::AlreadyReg => {
                tracing::debug!(
                    "dashstream_telemetry_inflight_permits already registered; continuing"
                );
            }
            other => {
                tracing::warn!(
                    error = %other,
                    "Failed to register dashstream_telemetry_inflight_permits; continuing without global registration"
                );
            }
        }
    }
    metric
});

/// M-1040: Self-observability gauge for pending telemetry tasks.
/// Shows how many spawned telemetry tasks are still running (not yet finished).
/// High values indicate telemetry processing backlog.
static TELEMETRY_PENDING_TASKS: LazyLock<IntGauge> = LazyLock::new(|| {
    let metric = IntGauge::new(
        "dashstream_telemetry_pending_tasks",
        "Number of pending telemetry tasks (spawned but not yet completed)",
    )
    .expect("dashstream_telemetry_pending_tasks gauge name should be valid");
    if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
        match e {
            prometheus::Error::AlreadyReg => {
                tracing::debug!(
                    "dashstream_telemetry_pending_tasks already registered; continuing"
                );
            }
            other => {
                tracing::warn!(
                    error = %other,
                    "Failed to register dashstream_telemetry_pending_tasks; continuing without global registration"
                );
            }
        }
    }
    metric
});

/// M-1040: Self-observability gauge for message queue depth.
/// Shows approximate number of messages waiting in the batch queue.
/// High values approaching capacity indicate backpressure risk.
static TELEMETRY_QUEUE_DEPTH: LazyLock<IntGauge> = LazyLock::new(|| {
    let metric = IntGauge::new(
        "dashstream_telemetry_queue_depth",
        "Approximate number of messages in telemetry batch queue (approaching capacity = backpressure risk)",
    )
    .expect("dashstream_telemetry_queue_depth gauge name should be valid");
    if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
        match e {
            prometheus::Error::AlreadyReg => {
                tracing::debug!("dashstream_telemetry_queue_depth already registered; continuing");
            }
            other => {
                tracing::warn!(
                    error = %other,
                    "Failed to register dashstream_telemetry_queue_depth; continuing without global registration"
                );
            }
        }
    }
    metric
});

/// M-1073 FIX: Safely decrement queue_depth by 1 with saturation (prevents underflow to u64::MAX).
/// Uses `fetch_update` instead of `fetch_sub` to ensure the atomic value itself never wraps.
/// Returns the new depth value after decrement and updates the telemetry gauge.
///
/// The previous pattern `fetch_sub(1).saturating_sub(1)` was buggy:
/// - `fetch_sub(1)` wraps 0 → u64::MAX in the atomic
/// - `saturating_sub(1)` only saturates the *returned* previous value
/// - Future reads see the corrupted u64::MAX value
fn decrement_queue_depth_saturating(queue_depth: &AtomicU64) -> u64 {
    // fetch_update returns Ok(prev_value) when closure succeeds
    // Our closure always returns Some, so unwrap is safe
    let prev = queue_depth
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
            Some(x.saturating_sub(1))
        })
        .expect("closure always returns Some");

    let new_depth = prev.saturating_sub(1);
    TELEMETRY_QUEUE_DEPTH.set(new_depth as i64);

    // Debug logging for invariant violation detection (would have underflowed before this fix)
    if prev == 0 {
        tracing::debug!(
            metric = "dashstream_telemetry_queue_depth",
            "queue_depth decrement called when already zero (internal invariant violation)"
        );
    }

    new_depth
}

/// M-1040: Self-observability gauge for max concurrent telemetry sends.
/// Shows the configured max_concurrent_telemetry_sends limit for this callback.
/// Combined with inflight_permits, allows computing saturation percentage.
static TELEMETRY_MAX_PERMITS: LazyLock<IntGauge> = LazyLock::new(|| {
    let metric = IntGauge::new(
        "dashstream_telemetry_max_permits",
        "Configured max_concurrent_telemetry_sends limit",
    )
    .expect("dashstream_telemetry_max_permits gauge name should be valid");
    if let Err(e) = prometheus::default_registry().register(Box::new(metric.clone())) {
        match e {
            prometheus::Error::AlreadyReg => {
                tracing::debug!("dashstream_telemetry_max_permits already registered; continuing");
            }
            other => {
                tracing::warn!(
                    error = %other,
                    "Failed to register dashstream_telemetry_max_permits; continuing without global registration"
                );
            }
        }
    }
    metric
});

/// M-673: Secret patterns for state diff redaction
/// These patterns detect sensitive data that should not be sent to Kafka/UI
#[allow(clippy::expect_used)] // SAFETY: All regex literals are hardcoded and validated at compile time
static STATE_DIFF_SECRET_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        // OpenAI API keys
        (
            Regex::new(r"sk-[a-zA-Z0-9]{20,}").expect("Invalid openai regex"),
            "[OPENAI_KEY]",
        ),
        // Anthropic API keys
        (
            Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").expect("Invalid anthropic regex"),
            "[ANTHROPIC_KEY]",
        ),
        // AWS Access Key IDs
        (
            Regex::new(r"(?:AKIA|ABIA|ACCA|ASIA)[A-Z0-9]{16}").expect("Invalid aws regex"),
            "[AWS_KEY]",
        ),
        // GitHub tokens
        (
            Regex::new(r"(?:ghp|gho|ghu|ghs|ghr)_[a-zA-Z0-9]{36,}").expect("Invalid github regex"),
            "[GITHUB_TOKEN]",
        ),
        // Bearer tokens
        (
            Regex::new(r"[Bb]earer\s+[a-zA-Z0-9_.-]{20,}").expect("Invalid bearer regex"),
            "Bearer [TOKEN]",
        ),
        // Generic API keys (api_key=..., apikey:..., etc.)
        (
            Regex::new(r#"(?i)(?:api[_-]?key|apikey)[=:\s]+['"]?[a-zA-Z0-9_-]{20,}['"]?"#)
                .expect("Invalid api_key regex"),
            "[API_KEY]",
        ),
        // URL passwords (://user:password@)
        (
            Regex::new(r"://[^:]+:([^@]{8,})@").expect("Invalid url_password regex"),
            "://[CREDENTIALS]@",
        ),
        // Private key markers
        (
            Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----")
                .expect("Invalid private_key regex"),
            "[PRIVATE_KEY]",
        ),
        // JWT tokens (eyJ...)
        (
            Regex::new(r"eyJ[a-zA-Z0-9_-]{20,}\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+")
                .expect("Invalid jwt regex"),
            "[JWT_TOKEN]",
        ),
        // Google API keys
        (
            Regex::new(r"AIza[a-zA-Z0-9_-]{35}").expect("Invalid google api key regex"),
            "[GOOGLE_API_KEY]",
        ),
        // Stripe keys
        (
            Regex::new(r"sk_(?:live|test)_[a-zA-Z0-9]{24,}").expect("Invalid stripe regex"),
            "[STRIPE_KEY]",
        ),
    ]
});

/// Check if state diff redaction is enabled via environment variable (M-673)
///
/// Controlled by `DASHFLOW_STATE_REDACT` env var:
/// - "true", "1", "yes", "on" -> enabled (default)
/// - "false", "0", "no", "off" -> disabled
fn is_state_redaction_enabled() -> bool {
    env_bool(DASHFLOW_STATE_REDACT, true) // Default ON for security
}

/// Redact sensitive data from a JSON value (M-673: security fix)
///
/// Recursively traverses JSON and redacts any string values matching secret patterns.
/// This prevents sensitive data like API keys, tokens, and passwords from being
/// sent to Kafka or displayed in the observability UI.
fn redact_json_value(value: &mut serde_json::Value) {
    if !is_state_redaction_enabled() {
        return;
    }

    match value {
        serde_json::Value::String(s) => {
            let mut result = s.clone();
            for (pattern, replacement) in STATE_DIFF_SECRET_PATTERNS.iter() {
                result = pattern.replace_all(&result, *replacement).to_string();
            }
            if result != *s {
                tracing::debug!(
                    original_len = s.len(),
                    redacted_len = result.len(),
                    "Redacted sensitive data from state diff"
                );
                *s = result;
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_json_value(item);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, v) in obj.iter_mut() {
                redact_json_value(v);
            }
        }
        _ => {} // Numbers, bools, null don't need redaction
    }
}

/// M-1065: Redact sensitive data from a string using secret patterns.
///
/// This is the core redaction logic shared by JSON value and AttributeValue redaction.
/// Returns Some(redacted_string) if any pattern matched, None if no redaction needed.
fn redact_string(s: &str) -> Option<String> {
    if !is_state_redaction_enabled() {
        return None;
    }

    let mut result = s.to_string();
    let mut redacted = false;

    for (pattern, replacement) in STATE_DIFF_SECRET_PATTERNS.iter() {
        let new_result = pattern.replace_all(&result, *replacement).to_string();
        if new_result != result {
            redacted = true;
            result = new_result;
        }
    }

    if redacted {
        Some(result)
    } else {
        None
    }
}

/// M-1065: Redact sensitive data from an AttributeValue (security fix)
///
/// Recursively traverses AttributeValue and redacts any string values matching secret patterns.
/// This prevents sensitive data like API keys, tokens, and passwords from being
/// sent to Kafka or displayed in the observability UI.
fn redact_attribute_value(attr: &mut AttributeValue) {
    if !is_state_redaction_enabled() {
        return;
    }

    if let Some(ref mut value) = attr.value {
        match value {
            attribute_value::Value::StringValue(s) => {
                if let Some(redacted) = redact_string(s) {
                    tracing::debug!(
                        original_len = s.len(),
                        redacted_len = redacted.len(),
                        "Redacted sensitive data from event attribute"
                    );
                    *s = redacted;
                }
            }
            attribute_value::Value::ArrayValue(arr) => {
                for item in arr.values.iter_mut() {
                    redact_attribute_value(item);
                }
            }
            attribute_value::Value::MapValue(map) => {
                for (_, v) in map.values.iter_mut() {
                    redact_attribute_value(v);
                }
            }
            // Int, Float, Bool, Bytes don't need redaction
            _ => {}
        }
    }
}

/// M-1065: Redact sensitive data from all event attributes (security fix)
///
/// Applies redaction to all AttributeValue entries in the attributes HashMap.
/// This ensures event telemetry doesn't leak secrets like API keys, tokens, etc.
fn redact_attributes(attributes: &mut std::collections::HashMap<String, AttributeValue>) {
    if !is_state_redaction_enabled() {
        return;
    }

    for (_, attr) in attributes.iter_mut() {
        redact_attribute_value(attr);
    }
}

/// Convert Duration to microseconds as i64 with saturation on overflow.
///
/// Duration::as_micros() returns u128 which can exceed i64::MAX for very long durations.
/// This function saturates to i64::MAX (which represents ~292,000 years) on overflow.
/// In practice, this is safe for all realistic telemetry timestamps and durations.
#[inline]
fn duration_to_micros_i64(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

/// Default maximum state size for diffing (10 MB)
/// States larger than this will skip diffing to prevent OOM
pub const DEFAULT_MAX_STATE_DIFF_SIZE: usize = 10 * 1024 * 1024;

/// Serialize state to JSON if it's within the size limit.
/// Returns None if the state is too large (prevents memory exhaustion).
///
/// M-672 fix: End-to-end safe size checking
/// - bincode often underestimates JSON size (by 2-3x for string-heavy data)
/// - First do a fast bincode pre-check with 3x multiplier for early rejection
/// - Then verify actual JSON size after serialization for accurate enforcement
fn serialize_state_with_limit<S: Serialize>(
    state: &S,
    max_size: usize,
    context: &str,
) -> Option<serde_json::Value> {
    // Skip size check if limit is 0 (disabled)
    if max_size > 0 {
        // Pre-check using bincode with conservative multiplier (M-672 fix)
        // bincode underestimates JSON size - use 3x multiplier for early rejection
        // This avoids expensive JSON serialization for obviously huge states
        const BINCODE_TO_JSON_MULTIPLIER: usize = 3;
        match bincode::serialized_size(state) {
            Ok(estimated_size) => {
                let bincode_size = estimated_size as usize;
                if bincode_size > max_size / BINCODE_TO_JSON_MULTIPLIER {
                    tracing::warn!(
                        context = context,
                        bincode_size_bytes = bincode_size,
                        estimated_json_bytes = bincode_size * BINCODE_TO_JSON_MULTIPLIER,
                        max_size_bytes = max_size,
                        "State likely too large for diffing (bincode pre-check), skipping. \
                         Consider increasing max_state_diff_size or reducing state size."
                    );
                    // M-699: Track degraded mode - state too large (pre-check)
                    STATE_DIFF_DEGRADED_TOTAL
                        .with_label_values(&["state_too_large_precheck"])
                        .inc();
                    return None;
                }
            }
            Err(e) => {
                tracing::debug!(
                    context = context,
                    error = %e,
                    "bincode pre-check failed, proceeding with JSON serialization"
                );
            }
        }
    }

    // Serialize to JSON
    let json = match serde_json::to_value(state) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!(context = context, error = %e, "Failed to serialize state to JSON");
            // M-699: Track degraded mode - serialization failed
            STATE_DIFF_DEGRADED_TOTAL
                .with_label_values(&["serialization_failed"])
                .inc();
            return None;
        }
    };

    // M-672 fix: Verify actual JSON size after serialization
    if max_size > 0 {
        // Estimate JSON byte size without re-serializing (approximation)
        // For accurate check, we'd need to serialize to string, but that's expensive
        // Use a heuristic: JSON string length is roughly proportional to byte size
        // This is only an estimate - actual protobuf encoding may differ
        if let Ok(json_str) = serde_json::to_string(&json) {
            let json_size = json_str.len();
            if json_size > max_size {
                tracing::warn!(
                    context = context,
                    json_size_bytes = json_size,
                    max_size_bytes = max_size,
                    "State JSON exceeds size limit, skipping. \
                     Consider increasing max_state_diff_size or reducing state size."
                );
                // M-699: Track degraded mode - state too large (post-check)
                STATE_DIFF_DEGRADED_TOTAL
                    .with_label_values(&["state_too_large_postcheck"])
                    .inc();
                return None;
            }
        }
    }

    // M-673 fix: Redact sensitive data before returning
    // This prevents API keys, tokens, and passwords from being sent to Kafka/UI
    let mut redacted_json = json;
    redact_json_value(&mut redacted_json);

    Some(redacted_json)
}

fn maybe_insert_initial_state_json_attribute<S: Serialize>(
    enable_state_diff: bool,
    max_state_diff_size: usize,
    initial_state: &S,
    attributes: &mut std::collections::HashMap<String, AttributeValue>,
) -> Option<serde_json::Value> {
    if !enable_state_diff {
        return None;
    }

    let state_json =
        serialize_state_with_limit(initial_state, max_state_diff_size, "initial_state")?;

    if let Ok(state_str) = serde_json::to_string(&state_json) {
        attributes.insert(
            "initial_state_json".to_string(),
            AttributeValue {
                value: Some(attribute_value::Value::StringValue(state_str)),
            },
        );
    }

    Some(state_json)
}

/// Default maximum concurrent telemetry sends
///
/// Limits runtime pressure from telemetry spikes.
/// Uses `DEFAULT_BROADCAST_CHANNEL_CAPACITY` (64) from centralized constants.
pub const DEFAULT_MAX_CONCURRENT_TELEMETRY_SENDS: usize = DEFAULT_BROADCAST_CHANNEL_CAPACITY;

/// Default telemetry batch size (number of events to batch before sending)
/// Set to 1 for no batching (current behavior). Higher values reduce scheduler overhead.
pub const DEFAULT_TELEMETRY_BATCH_SIZE: usize = 1;

/// Default telemetry batch timeout in milliseconds
///
/// Events are flushed after this timeout even if batch is not full.
/// Uses `DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS` (100ms) from centralized constants.
pub const DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS: u64 = TELEMETRY_BATCH_TIMEOUT_MS;

/// Default checkpoint interval (number of state diffs between checkpoints)
/// Set to 0 to disable automatic checkpoints (default).
/// Recommended value for production: 50-100 (balance between recovery speed and bandwidth).
/// Lower values = faster resync after corruption, higher bandwidth usage.
/// Higher values = slower resync, lower bandwidth usage.
pub const DEFAULT_CHECKPOINT_INTERVAL: u64 = 0;

/// Default flush timeout in seconds (M-519: configurable to prevent event loss)
///
/// Used when flushing pending events during shutdown.
/// If the channel is full and timeout expires, pending batched events may be lost.
/// Uses `DEFAULT_FLUSH_TIMEOUT_SECS` (5s) from centralized constants.
pub const DEFAULT_FLUSH_TIMEOUT_SECS: u64 = FLUSH_TIMEOUT_SECS;

/// Messages sent to the batch worker.
///
/// Batching uses a command channel so `flush()` can request an explicit flush
/// without relying on dropping all senders (callbacks are Clone, so multiple
/// senders may exist).
///
/// Note: Event variant is intentionally stored inline (not boxed) since it's
/// the hot path in message passing. Boxing would add heap allocation overhead.
#[allow(clippy::large_enum_variant)] // Intentional: Event is hot path, boxing adds heap allocation
enum BatchMessage {
    Event(Event),
    /// StateDiff messages bypass batching but use the same ordered queue
    /// to guarantee Event/StateDiff ordering per-thread (M-666 fix)
    StateDiff(StateDiff),
    /// Checkpoint messages for full state snapshots (M-671: resync support)
    /// Sent periodically (based on checkpoint_interval) to enable UI resync after corruption
    Checkpoint(Checkpoint),
    Flush(oneshot::Sender<()>),
}

/// Configuration for DashFlow Streaming callback
#[derive(Debug, Clone)]
pub struct DashStreamConfig {
    /// Kafka bootstrap servers
    pub bootstrap_servers: String,

    /// Kafka topic name
    pub topic: String,

    /// Tenant ID for multi-tenancy
    pub tenant_id: String,

    /// Thread/session ID
    pub thread_id: String,

    /// Enable state diffing (default: true)
    pub enable_state_diff: bool,

    /// Compression threshold in bytes (default: 512)
    pub compression_threshold: usize,

    /// Maximum state size in bytes for diffing (default: 10MB)
    /// States larger than this will skip diffing to prevent memory exhaustion
    /// Set to 0 to disable size limit (not recommended for production)
    pub max_state_diff_size: usize,

    /// Maximum concurrent telemetry sends (default: 64)
    /// Flow control to prevent runtime starvation during telemetry spikes.
    /// When limit is reached, new telemetry is dropped (best-effort semantics).
    /// Monitor drops via `dashstream_telemetry_dropped_total` metric.
    /// See module docs for alerting recommendations.
    pub max_concurrent_telemetry_sends: usize,

    /// Telemetry batch size (default: 1, no batching)
    /// Number of events to accumulate before sending as an EventBatch.
    /// Set to 1 for immediate sending (original behavior).
    /// Higher values (e.g., 10-100) reduce scheduler overhead at high telemetry volumes.
    /// Events are sent when batch is full OR timeout expires, whichever comes first.
    /// Note: If batch queue fills, new events are dropped (best-effort semantics).
    /// Monitor via `dashstream_telemetry_dropped_total` metric.
    pub telemetry_batch_size: usize,

    /// Telemetry batch timeout in milliseconds (default: 100ms)
    /// Events are flushed after this timeout even if the batch is not full.
    /// Only relevant when telemetry_batch_size > 1.
    pub telemetry_batch_timeout_ms: u64,

    /// Checkpoint interval (number of state diffs between checkpoints)
    /// Set to 0 to disable automatic checkpoints (default).
    /// When enabled, a full state snapshot is sent every N state diffs.
    /// This enables UI resync after dropped StateDiff messages (M-671 fix).
    /// Recommended value for production: 50-100.
    pub checkpoint_interval: u64,

    /// Flush timeout in seconds (M-519: configurable via DASHFLOW_FLUSH_TIMEOUT_SECS)
    /// Used when flushing pending events during shutdown.
    /// If the channel is full and timeout expires, pending batched events may be lost.
    /// Increase this value if you have high telemetry volume and need more time to drain.
    /// Default: 5 seconds.
    pub flush_timeout_secs: u64,
}

impl Default for DashStreamConfig {
    fn default() -> Self {
        Self {
            // Read KAFKA_BROKERS from environment with localhost:9092 fallback for local dev
            bootstrap_servers: env_string_or_default(KAFKA_BROKERS, "localhost:9092"),
            // Read KAFKA_TOPIC from environment with dashstream-events fallback
            topic: env_string_or_default(KAFKA_TOPIC, "dashstream-events"),
            tenant_id: "default".to_string(),
            thread_id: uuid::Uuid::new_v4().to_string(),
            enable_state_diff: true,
            compression_threshold: 512,
            max_state_diff_size: DEFAULT_MAX_STATE_DIFF_SIZE,
            max_concurrent_telemetry_sends: DEFAULT_MAX_CONCURRENT_TELEMETRY_SENDS,
            telemetry_batch_size: DEFAULT_TELEMETRY_BATCH_SIZE,
            telemetry_batch_timeout_ms: DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS,
            checkpoint_interval: DEFAULT_CHECKPOINT_INTERVAL,
            // M-519: Read flush timeout from environment with default fallback
            flush_timeout_secs: env_u64(DASHFLOW_FLUSH_TIMEOUT_SECS, DEFAULT_FLUSH_TIMEOUT_SECS),
        }
    }
}

impl DashStreamConfig {
    /// Create a new `DashStreamConfig` with default values.
    ///
    /// Uses environment variables for Kafka settings when available:
    /// - `KAFKA_BROKERS` (default: "localhost:9092")
    /// - `KAFKA_TOPIC` (default: "dashstream-events")
    /// - `DASHFLOW_FLUSH_TIMEOUT_SECS` (default: 5)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the Kafka bootstrap servers.
    ///
    /// # Example
    /// ```rust,ignore
    /// let config = DashStreamConfig::new()
    ///     .with_bootstrap_servers("kafka1:9092,kafka2:9092");
    /// ```
    #[must_use]
    pub fn with_bootstrap_servers(mut self, servers: impl Into<String>) -> Self {
        self.bootstrap_servers = servers.into();
        self
    }

    /// Set the Kafka topic name.
    ///
    /// # Example
    /// ```rust,ignore
    /// let config = DashStreamConfig::new()
    ///     .with_topic("my-events");
    /// ```
    #[must_use]
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = topic.into();
        self
    }

    /// Set the tenant ID for multi-tenancy.
    ///
    /// # Example
    /// ```rust,ignore
    /// let config = DashStreamConfig::new()
    ///     .with_tenant_id("tenant-123");
    /// ```
    #[must_use]
    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = tenant_id.into();
        self
    }

    /// Set the thread/session ID.
    ///
    /// # Example
    /// ```rust,ignore
    /// let config = DashStreamConfig::new()
    ///     .with_thread_id("session-abc");
    /// ```
    #[must_use]
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = thread_id.into();
        self
    }

    /// Enable or disable state diffing.
    ///
    /// When enabled (default), only state changes are sent.
    /// When disabled, full state is sent with each event.
    #[must_use]
    pub fn with_enable_state_diff(mut self, enable: bool) -> Self {
        self.enable_state_diff = enable;
        self
    }

    /// Set the compression threshold in bytes.
    ///
    /// Messages larger than this threshold will be compressed.
    /// Default: 512 bytes.
    #[must_use]
    pub fn with_compression_threshold(mut self, threshold: usize) -> Self {
        self.compression_threshold = threshold;
        self
    }

    /// Set the maximum state size in bytes for diffing.
    ///
    /// States larger than this will skip diffing to prevent memory exhaustion.
    /// Default: 10MB. Set to 0 to disable (not recommended for production).
    #[must_use]
    pub fn with_max_state_diff_size(mut self, size: usize) -> Self {
        self.max_state_diff_size = size;
        self
    }

    /// Set the maximum concurrent telemetry sends.
    ///
    /// Flow control to prevent runtime starvation during telemetry spikes.
    /// When limit is reached, new telemetry is dropped (best-effort semantics).
    /// Default: 64.
    #[must_use]
    pub fn with_max_concurrent_telemetry_sends(mut self, max: usize) -> Self {
        self.max_concurrent_telemetry_sends = max;
        self
    }

    /// Set the telemetry batch size.
    ///
    /// Number of events to accumulate before sending as an EventBatch.
    /// Set to 1 for immediate sending (default).
    /// Higher values (10-100) reduce scheduler overhead at high telemetry volumes.
    #[must_use]
    pub fn with_telemetry_batch_size(mut self, size: usize) -> Self {
        self.telemetry_batch_size = size;
        self
    }

    /// Set the telemetry batch timeout in milliseconds.
    ///
    /// Events are flushed after this timeout even if the batch is not full.
    /// Only relevant when telemetry_batch_size > 1.
    /// Default: 100ms.
    #[must_use]
    pub fn with_telemetry_batch_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.telemetry_batch_timeout_ms = timeout_ms;
        self
    }

    /// Set the checkpoint interval.
    ///
    /// Number of state diffs between checkpoints (full state snapshots).
    /// Set to 0 to disable automatic checkpoints (default).
    /// Recommended value for production: 50-100.
    #[must_use]
    pub fn with_checkpoint_interval(mut self, interval: u64) -> Self {
        self.checkpoint_interval = interval;
        self
    }

    /// Set the flush timeout in seconds.
    ///
    /// Used when flushing pending events during shutdown.
    /// Default: 5 seconds.
    #[must_use]
    pub fn with_flush_timeout_secs(mut self, timeout: u64) -> Self {
        self.flush_timeout_secs = timeout;
        self
    }
}

/// DashFlow Streaming callback for DashFlow graph events
///
/// Sends graph execution events and state diffs to Kafka.
#[derive(Clone)]
pub struct DashStreamCallback<S>
where
    S: GraphState + Serialize,
{
    producer: Arc<DashStreamProducer>,
    config: DashStreamConfig,
    /// Sequence counter uses AtomicU64 for lock-free increments
    sequence: Arc<AtomicU64>,
    previous_state: Arc<Mutex<Option<serde_json::Value>>>,
    /// Pending telemetry tasks for graceful shutdown
    /// Tasks are tracked to ensure they complete before shutdown.
    pending_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    /// Flow control semaphore for bounded telemetry sends
    /// Prevents runtime starvation during telemetry spikes
    telemetry_semaphore: Arc<Semaphore>,
    /// Counter for dropped telemetry messages
    /// Exposed via telemetry_dropped_count() for monitoring
    telemetry_dropped: Arc<AtomicU64>,
    /// Message sender for ordered telemetry delivery.
    /// All telemetry (Events, StateDiffs) routes through this queue to guarantee
    /// per-thread ordering regardless of batch_size setting (M-666 fix).
    message_sender: mpsc::Sender<BatchMessage>,
    /// Handle for the message worker task (for graceful shutdown)
    message_worker: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Counter for state diffs since last checkpoint (M-671: resync support)
    /// Used with checkpoint_interval to trigger automatic checkpoints
    diffs_since_checkpoint: Arc<AtomicU64>,
    /// Most recent checkpoint ID (M-671: resync support)
    /// StateDiffs reference this for delta chain reconstruction
    last_checkpoint_id: Arc<Mutex<Vec<u8>>>,
    /// M-1040: Approximate queue depth counter for self-observability
    /// Tracked via atomic since mpsc::Sender doesn't expose queue length
    queue_depth: Arc<AtomicU64>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S> DashStreamCallback<S>
where
    S: GraphState + Serialize,
{
    /// Create new DashFlow Streaming callback
    pub async fn new(
        bootstrap_servers: &str,
        topic: &str,
        tenant_id: &str,
        thread_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config = DashStreamConfig {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
            tenant_id: tenant_id.to_string(),
            thread_id: thread_id.to_string(),
            ..Default::default()
        };

        Self::with_config(config).await
    }

    /// Create new DashFlow Streaming callback with custom configuration
    pub async fn with_config(
        mut config: DashStreamConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // ------------------------------------------------------------------
        // Config validation / clamping (stability hardening)
        // ------------------------------------------------------------------
        if config.bootstrap_servers.trim().is_empty() {
            return Err("bootstrap_servers must be non-empty".into());
        }
        if config.topic.trim().is_empty() {
            return Err("topic must be non-empty".into());
        }
        if config.tenant_id.trim().is_empty() {
            return Err("tenant_id must be non-empty".into());
        }
        if config.thread_id.trim().is_empty() {
            return Err("thread_id must be non-empty".into());
        }

        // Protect against accidental drop-all behavior.
        if config.max_concurrent_telemetry_sends == 0 {
            tracing::warn!(
                provided = config.max_concurrent_telemetry_sends,
                fallback = DEFAULT_MAX_CONCURRENT_TELEMETRY_SENDS,
                "max_concurrent_telemetry_sends must be > 0; using default"
            );
            config.max_concurrent_telemetry_sends = DEFAULT_MAX_CONCURRENT_TELEMETRY_SENDS;
        }

        // Prevent panics (tokio::mpsc::channel requires capacity > 0) and avoid
        // unbounded buffering/memory when batch size is set too high.
        const MAX_TELEMETRY_BATCH_SIZE: usize = 1000;
        if config.telemetry_batch_size == 0 {
            tracing::warn!("telemetry_batch_size must be >= 1; disabling batching (size=1)");
            config.telemetry_batch_size = 1;
        } else if config.telemetry_batch_size > MAX_TELEMETRY_BATCH_SIZE {
            tracing::warn!(
                provided = config.telemetry_batch_size,
                clamped = MAX_TELEMETRY_BATCH_SIZE,
                "telemetry_batch_size too large; clamping"
            );
            config.telemetry_batch_size = MAX_TELEMETRY_BATCH_SIZE;
        }

        // Avoid busy loops when batching is enabled (timeout=0ms).
        const MAX_TELEMETRY_BATCH_TIMEOUT_MS: u64 = 60_000;
        if config.telemetry_batch_size > 1 {
            if config.telemetry_batch_timeout_ms == 0 {
                tracing::warn!(
                    fallback = DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS,
                    "telemetry_batch_timeout_ms must be > 0 when batching; using default"
                );
                config.telemetry_batch_timeout_ms = DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS;
            } else if config.telemetry_batch_timeout_ms > MAX_TELEMETRY_BATCH_TIMEOUT_MS {
                tracing::warn!(
                    provided = config.telemetry_batch_timeout_ms,
                    clamped = MAX_TELEMETRY_BATCH_TIMEOUT_MS,
                    "telemetry_batch_timeout_ms too large; clamping"
                );
                config.telemetry_batch_timeout_ms = MAX_TELEMETRY_BATCH_TIMEOUT_MS;
            }
        }

        let producer_config = ProducerConfig {
            bootstrap_servers: config.bootstrap_servers.clone(),
            topic: config.topic.clone(),
            tenant_id: config.tenant_id.clone(),
            compression_threshold: config.compression_threshold,
            ..Default::default()
        };
        let producer = Arc::new(DashStreamProducer::with_config(producer_config).await?);
        let semaphore_permits = config.max_concurrent_telemetry_sends;

        // Always create message queue for ordering guarantee (M-666 fix).
        // When batch_size > 1: events are accumulated and sent as EventBatch
        // When batch_size = 1: events are sent immediately but still ordered
        let capacity = config
            .telemetry_batch_size
            .saturating_mul(4)
            .clamp(DEFAULT_BROADCAST_CHANNEL_CAPACITY, DEFAULT_MAX_CHANNEL_CAPACITY);
        let (message_sender, rx) = mpsc::channel::<BatchMessage>(capacity);
        // M-1040: Create queue depth counter for self-observability
        let queue_depth = Arc::new(AtomicU64::new(0));
        let message_worker = Self::spawn_message_worker(
            rx,
            producer.clone(),
            config.telemetry_batch_size,
            config.telemetry_batch_timeout_ms,
            config.thread_id.clone(),
            config.tenant_id.clone(),
            queue_depth.clone(),
        );
        let message_worker = Arc::new(Mutex::new(Some(message_worker)));

        // M-1040: Set max_permits gauge for saturation calculation
        TELEMETRY_MAX_PERMITS.set(semaphore_permits as i64);

        Ok(Self {
            producer,
            config,
            sequence: Arc::new(AtomicU64::new(0)),
            previous_state: Arc::new(Mutex::new(None)),
            pending_tasks: Arc::new(Mutex::new(Vec::new())),
            telemetry_semaphore: Arc::new(Semaphore::new(semaphore_permits)),
            telemetry_dropped: Arc::new(AtomicU64::new(0)),
            message_sender,
            message_worker,
            diffs_since_checkpoint: Arc::new(AtomicU64::new(0)),
            last_checkpoint_id: Arc::new(Mutex::new(Vec::new())),
            queue_depth,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a callback from existing components (for testing)
    ///
    /// Test infrastructure function: Allows creating DashStreamCallback from components.
    ///
    /// Reserved for future unit tests that need to construct callback with mock components.
    /// Currently unused (integration tests use new() constructor at line 75-90).
    /// #[cfg(test)] ensures zero runtime cost in production builds.
    /// Cannot remove without limiting future testability of callback internals.
    #[cfg(test)]
    #[allow(dead_code)]
    fn from_parts(
        producer: Arc<DashStreamProducer>,
        config: DashStreamConfig,
        sequence: Arc<AtomicU64>,
        previous_state: Arc<Mutex<Option<serde_json::Value>>>,
    ) -> Self {
        let semaphore_permits = config.max_concurrent_telemetry_sends;
        // Create message queue that drains to producer (test mode still routes through queue)
        let (message_sender, rx) = mpsc::channel::<BatchMessage>(DEFAULT_BROADCAST_CHANNEL_CAPACITY);
        // M-1040: Create queue depth counter for self-observability
        let queue_depth = Arc::new(AtomicU64::new(0));
        let message_worker = Self::spawn_message_worker(
            rx,
            producer.clone(),
            config.telemetry_batch_size,
            config.telemetry_batch_timeout_ms,
            config.thread_id.clone(),
            config.tenant_id.clone(),
            queue_depth.clone(),
        );
        Self {
            producer,
            config,
            sequence,
            previous_state,
            pending_tasks: Arc::new(Mutex::new(Vec::new())),
            telemetry_semaphore: Arc::new(Semaphore::new(semaphore_permits)),
            telemetry_dropped: Arc::new(AtomicU64::new(0)),
            message_sender,
            message_worker: Arc::new(Mutex::new(Some(message_worker))),
            diffs_since_checkpoint: Arc::new(AtomicU64::new(0)),
            last_checkpoint_id: Arc::new(Mutex::new(Vec::new())),
            queue_depth,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Spawn the message worker task for ordered telemetry delivery.
    ///
    /// M-666 fix: ALL telemetry routes through this single queue to guarantee
    /// per-thread Event/StateDiff ordering regardless of batch_size setting.
    ///
    /// - batch_size = 1: Events sent directly (no EventBatch wrapper overhead)
    /// - batch_size > 1: Events accumulated and sent as EventBatch
    /// - StateDiff: Always sent immediately, but queue ensures ordering
    fn spawn_message_worker(
        mut rx: mpsc::Receiver<BatchMessage>,
        producer: Arc<DashStreamProducer>,
        batch_size: usize,
        timeout_ms: u64,
        thread_id: String,
        tenant_id: String,
        queue_depth: Arc<AtomicU64>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            // For batch_size=1, we send events directly without accumulation
            let use_batching = batch_size > 1;
            let timeout = Duration::from_millis(timeout_ms);
            let mut batch: Vec<Event> = if use_batching {
                Vec::with_capacity(batch_size)
            } else {
                Vec::new() // Not used when batch_size=1
            };
            let mut flush_deadline: Option<tokio::time::Instant> = None;

            loop {
                // M-812 FIX: Removed unreachable timer initialization check.
                // The condition (deadline.is_none && !batch.is_empty) was impossible because
                // we always set flush_deadline when pushing the first event to an empty batch
                // (see line ~838). Keeping this comment to explain why the safety check isn't needed.

                if let Some(deadline) = flush_deadline.filter(|_| use_batching) {
                    tokio::select! {
                        msg = rx.recv() => {
                            match msg {
                                Some(BatchMessage::Event(event)) => {
                                    // M-1040: Decrement queue depth (M-1073: saturating to prevent underflow)
                                    let _depth = decrement_queue_depth_saturating(&queue_depth);
                                    if batch.is_empty() {
                                        flush_deadline = Some(tokio::time::Instant::now() + timeout);
                                    }
                                    batch.push(event);
                                    if batch.len() >= batch_size {
                                        Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id).await;
                                        flush_deadline = None;
                                    }
                                }
                                Some(BatchMessage::StateDiff(state_diff)) => {
                                    // M-1040: Decrement queue depth (M-1073: saturating to prevent underflow)
                                    let _depth = decrement_queue_depth_saturating(&queue_depth);
                                    // Flush pending Events first to preserve ordering (M-666 fix)
                                    if !batch.is_empty() {
                                        Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id).await;
                                    }
                                    flush_deadline = None;
                                    // Send StateDiff immediately (never batched)
                                    if let Err(e) = producer.send_state_diff(state_diff).await {
                                        // M-713: Count send failures for alerting
                                        TELEMETRY_SEND_FAILURES_TOTAL.with_label_values(&["state_diff"]).inc();
                                        tracing::warn!(
                                            thread_id = %thread_id,
                                            metric = "dashstream_telemetry_send_failures_total",
                                            message_type = "state_diff",
                                            "Failed to send state diff telemetry: {e}"
                                        );
                                    }
                                }
                                Some(BatchMessage::Checkpoint(checkpoint)) => {
                                    // M-1040: Decrement queue depth (M-1073: saturating to prevent underflow)
                                    let _depth = decrement_queue_depth_saturating(&queue_depth);
                                    // Flush pending Events first to preserve ordering (M-671 fix)
                                    if !batch.is_empty() {
                                        Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id).await;
                                    }
                                    flush_deadline = None;
                                    // Send Checkpoint immediately (never batched)
                                    if let Err(e) = producer.send_checkpoint(checkpoint).await {
                                        // M-713: Count send failures for alerting
                                        TELEMETRY_SEND_FAILURES_TOTAL.with_label_values(&["checkpoint"]).inc();
                                        tracing::warn!(
                                            thread_id = %thread_id,
                                            metric = "dashstream_telemetry_send_failures_total",
                                            message_type = "checkpoint",
                                            "Failed to send checkpoint telemetry: {e}"
                                        );
                                    }
                                }
                                Some(BatchMessage::Flush(ack)) => {
                                    // Flush doesn't affect queue depth - it's a control message
                                    if !batch.is_empty() {
                                        Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id).await;
                                    }
                                    flush_deadline = None;
                                    let _ = ack.send(());
                                }
                                None => {
                                    if !batch.is_empty() {
                                        Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id).await;
                                    }
                                    break;
                                }
                            }
                        }
                        _ = tokio::time::sleep_until(deadline) => {
                            if !batch.is_empty() {
                                Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id).await;
                            }
                            flush_deadline = None;
                        }
                    }
                } else {
                    // No batching (batch_size=1) or no pending deadline
                    match rx.recv().await {
                        Some(BatchMessage::Event(event)) => {
                            // M-1040: Decrement queue depth (M-1073: saturating to prevent underflow)
                            let _depth = decrement_queue_depth_saturating(&queue_depth);
                            if use_batching {
                                batch.push(event);
                                flush_deadline = Some(tokio::time::Instant::now() + timeout);
                                if batch.len() >= batch_size {
                                    Self::flush_batch(
                                        &producer, &mut batch, &thread_id, &tenant_id,
                                    )
                                    .await;
                                    flush_deadline = None;
                                }
                            } else {
                                // batch_size=1: send event directly (no EventBatch overhead)
                                if let Err(e) = producer.send_event(event).await {
                                    // M-713: Count send failures for alerting
                                    TELEMETRY_SEND_FAILURES_TOTAL
                                        .with_label_values(&["event"])
                                        .inc();
                                    tracing::warn!(
                                        thread_id = %thread_id,
                                        metric = "dashstream_telemetry_send_failures_total",
                                        message_type = "event",
                                        "Failed to send event telemetry: {e}"
                                    );
                                }
                            }
                        }
                        Some(BatchMessage::StateDiff(state_diff)) => {
                            // M-1040: Decrement queue depth (M-1073: saturating to prevent underflow)
                            let _depth = decrement_queue_depth_saturating(&queue_depth);
                            // Flush pending Events first to preserve ordering (M-666 fix)
                            if use_batching && !batch.is_empty() {
                                Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id)
                                    .await;
                            }
                            flush_deadline = None;
                            // Send StateDiff immediately (never batched)
                            if let Err(e) = producer.send_state_diff(state_diff).await {
                                // M-713: Count send failures for alerting
                                TELEMETRY_SEND_FAILURES_TOTAL
                                    .with_label_values(&["state_diff"])
                                    .inc();
                                tracing::warn!(
                                    thread_id = %thread_id,
                                    metric = "dashstream_telemetry_send_failures_total",
                                    message_type = "state_diff",
                                    "Failed to send state diff telemetry: {e}"
                                );
                            }
                        }
                        Some(BatchMessage::Checkpoint(checkpoint)) => {
                            // M-1040: Decrement queue depth (M-1073: saturating to prevent underflow)
                            let _depth = decrement_queue_depth_saturating(&queue_depth);
                            // Flush pending Events first to preserve ordering (M-671 fix)
                            if use_batching && !batch.is_empty() {
                                Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id)
                                    .await;
                            }
                            flush_deadline = None;
                            // Send Checkpoint immediately (never batched)
                            if let Err(e) = producer.send_checkpoint(checkpoint).await {
                                // M-713: Count send failures for alerting
                                TELEMETRY_SEND_FAILURES_TOTAL
                                    .with_label_values(&["checkpoint"])
                                    .inc();
                                tracing::warn!(
                                    thread_id = %thread_id,
                                    metric = "dashstream_telemetry_send_failures_total",
                                    message_type = "checkpoint",
                                    "Failed to send checkpoint telemetry: {e}"
                                );
                            }
                        }
                        Some(BatchMessage::Flush(ack)) => {
                            // Flush doesn't affect queue depth - it's a control message
                            if use_batching && !batch.is_empty() {
                                Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id)
                                    .await;
                            }
                            flush_deadline = None;
                            let _ = ack.send(());
                        }
                        None => {
                            if use_batching && !batch.is_empty() {
                                Self::flush_batch(&producer, &mut batch, &thread_id, &tenant_id)
                                    .await;
                            }
                            break;
                        }
                    }
                }
            }
        })
    }

    /// Flush accumulated events as an EventBatch
    async fn flush_batch(
        producer: &DashStreamProducer,
        batch: &mut Vec<Event>,
        thread_id: &str,
        tenant_id: &str,
    ) {
        if batch.is_empty() {
            return;
        }

        let events: Vec<Event> = std::mem::take(batch);
        let event_count = events.len();

        // Create batch header
        // M-813 FIX: Add consistent error logging (matches create_header pattern)
        let timestamp_us = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => duration_to_micros_i64(duration),
            Err(e) => {
                // System clock is before UNIX epoch - this indicates a serious system configuration issue
                tracing::error!(
                    error = %e,
                    thread_id = %thread_id,
                    "System clock is before UNIX epoch - batch header timestamp will be incorrect"
                );
                0
            }
        };

        let header = Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us,
            tenant_id: tenant_id.to_string(),
            thread_id: thread_id.to_string(),
            // M-736: Batch header sequence is intentionally 0 because:
            // 1. Sequence validation happens on individual event headers, not the batch
            // 2. The server extracts max(inner_event.sequence) for cursor tracking
            // 3. If we assigned a batch sequence, it would create gaps in per-thread sequence space
            // The websocket server warns if any inner event has sequence=0 (unexpected).
            sequence: 0,
            r#type: MessageType::EventBatch as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: dashflow_streaming::CURRENT_SCHEMA_VERSION,
        };

        let event_batch = EventBatch {
            header: Some(header),
            events,
        };

        if let Err(e) = producer.send_event_batch(event_batch).await {
            // M-713: Count send failures for alerting (count as 1 failure even though batch has N events)
            TELEMETRY_SEND_FAILURES_TOTAL
                .with_label_values(&["event_batch"])
                .inc();
            tracing::warn!(
                thread_id = %thread_id,
                event_count = event_count,
                metric = "dashstream_telemetry_send_failures_total",
                message_type = "event_batch",
                "Failed to send event batch: {e}"
            );
        }
    }

    /// Get the next sequence number (lock-free atomic increment)
    fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Get count of dropped telemetry messages due to flow control
    ///
    /// Use this to monitor telemetry backpressure. High values indicate
    /// telemetry is being generated faster than it can be sent.
    pub fn telemetry_dropped_count(&self) -> u64 {
        self.telemetry_dropped.load(Ordering::Relaxed)
    }

    /// Spawn a telemetry task with flow control
    ///
    /// This method wraps `tokio::spawn` with bounded concurrency to prevent
    /// runtime starvation during telemetry spikes. Uses try_acquire for
    /// non-blocking flow control - if at capacity, the message is dropped
    /// and counted.
    ///
    /// Also tracks the JoinHandle for graceful shutdown via flush().
    fn spawn_tracked<F>(&self, message_type: &'static str, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        // Try to acquire a permit (non-blocking)
        match self.telemetry_semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                // M-1040: Track inflight permits (increment before spawning)
                TELEMETRY_INFLIGHT_PERMITS.inc();
                // Wrap future to release permit and update gauge when done
                let handle = tokio::spawn(async move {
                    future.await;
                    // M-1040: Decrement inflight gauge when task completes
                    TELEMETRY_INFLIGHT_PERMITS.dec();
                    drop(permit); // Release permit when done
                });
                // Add to pending tasks, cleaning up completed ones
                let mut tasks = self.pending_tasks.lock();
                // Remove completed tasks to prevent unbounded growth
                tasks.retain(|h| !h.is_finished());
                tasks.push(handle);
                // M-1040: Update pending tasks gauge
                TELEMETRY_PENDING_TASKS.set(tasks.len() as i64);
            }
            Err(_) => {
                // At capacity - drop this telemetry message (best-effort semantics)
                // M-694: Drops tracked with message_type and reason labels
                let dropped = self.telemetry_dropped.fetch_add(1, Ordering::Relaxed) + 1;
                TELEMETRY_DROPPED_TOTAL
                    .with_label_values(&[message_type, "capacity_limit"])
                    .inc();
                // Log periodically (every 100 drops) to avoid log spam
                if dropped % 100 == 1 {
                    tracing::warn!(
                        thread_id = %self.config.thread_id,
                        dropped_count = dropped,
                        max_concurrent = self.config.max_concurrent_telemetry_sends,
                        metric = "dashstream_telemetry_dropped_total",
                        message_type = message_type,
                        reason = "capacity_limit",
                        "Telemetry dropped: at max_concurrent_telemetry_sends limit. \
                         Monitor dashstream_telemetry_dropped_total metric for alerting."
                    );
                }
            }
        }
    }

    /// Create a header for messages
    fn create_header(&self, message_type: MessageType) -> Header {
        let timestamp_us = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => duration_to_micros_i64(duration),
            Err(e) => {
                // System clock is before UNIX epoch - this indicates a serious system configuration issue
                tracing::error!(
                    error = %e,
                    "System clock is before UNIX epoch - telemetry timestamps will be incorrect"
                );
                0
            }
        };
        Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us,
            tenant_id: self.config.tenant_id.clone(),
            thread_id: self.config.thread_id.clone(),
            sequence: self.next_sequence(),
            r#type: message_type as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: dashflow_streaming::CURRENT_SCHEMA_VERSION,
        }
    }

    /// Convert DashFlow graph event to streaming protocol event and send
    fn send_graph_event(&self, graph_event: &GraphEvent<S>) {
        // Note: producer is accessed via self.message_sender channel, not directly
        let config = self.config.clone();
        let previous_state = self.previous_state.clone();

        // Clone the event for async processing
        // Returns (event_type, node_id, duration_us, timestamp, attributes)
        let event_data = match graph_event {
            GraphEvent::GraphStart {
                timestamp,
                initial_state,
                manifest,
            } => {
                let event_type = EventType::GraphStart;
                let node_id = "".to_string();
                let duration_us = 0i64;
                let ts = *timestamp;

                // Include graph manifest in telemetry attributes
                let mut attributes = std::collections::HashMap::new();
                // M-1089: Maximum size for manifest/schema attributes to prevent payload_too_large errors.
                // Typical Kafka limit is 1MB; we use 500KB to leave room for other attributes.
                const MAX_MANIFEST_JSON_BYTES: usize = 500 * 1024;
                if let Some(ref m) = manifest {
                    // Serialize manifest to compact JSON for telemetry
                    if let Ok(manifest_json) = m.to_json_compact() {
                        // M-1089 FIX: Skip oversized manifests instead of causing decode failures
                        if manifest_json.len() <= MAX_MANIFEST_JSON_BYTES {
                            attributes.insert(
                                "graph_manifest".to_string(),
                                AttributeValue {
                                    value: Some(attribute_value::Value::StringValue(manifest_json)),
                                },
                            );
                        } else {
                            tracing::warn!(
                                "Skipping oversized graph_manifest ({} bytes > {} max)",
                                manifest_json.len(),
                                MAX_MANIFEST_JSON_BYTES
                            );
                        }
                    }

                    // Emit UI-friendly GraphSchema JSON (arrays, not maps)
                    // This is the recommended format for UI consumption
                    let schema = m.to_schema();
                    if let Ok(schema_json) = schema.to_json() {
                        // M-1089 FIX: Skip oversized schemas instead of causing decode failures
                        if schema_json.len() <= MAX_MANIFEST_JSON_BYTES {
                            attributes.insert(
                                "graph_schema_json".to_string(),
                                AttributeValue {
                                    value: Some(attribute_value::Value::StringValue(schema_json)),
                                },
                            );
                        } else {
                            tracing::warn!(
                                "Skipping oversized graph_schema_json ({} bytes > {} max)",
                                schema_json.len(),
                                MAX_MANIFEST_JSON_BYTES
                            );
                        }
                    }

                    // Emit content-addressed schema_id for version tracking
                    let schema_id = m.compute_schema_id();
                    attributes.insert(
                        "schema_id".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::StringValue(schema_id)),
                        },
                    );

                    // Also include key manifest fields as separate attributes for easier querying
                    attributes.insert(
                        "graph_entry_point".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::StringValue(m.entry_point.clone())),
                        },
                    );
                    if let Some(name) = &m.graph_name {
                        attributes.insert(
                            "graph_name".to_string(),
                            AttributeValue {
                                value: Some(attribute_value::Value::StringValue(name.clone())),
                            },
                        );
                    }
                    attributes.insert(
                        "graph_node_count".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::IntValue(m.nodes.len() as i64)),
                        },
                    );
                    attributes.insert(
                        "graph_edge_count".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::IntValue(
                                m.edges.values().map(|v| v.len()).sum::<usize>() as i64,
                            )),
                        },
                    );
                }

                // Emit initial state only when state diffs are enabled.
                // When diffs are disabled, the UI cannot apply patches anyway, so emitting
                // baseline state is misleading/no-op for state reconstruction.
                if let Some(state_json) = maybe_insert_initial_state_json_attribute(
                    config.enable_state_diff,
                    config.max_state_diff_size,
                    initial_state,
                    &mut attributes,
                ) {
                    *previous_state.lock() = Some(state_json);
                }

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::GraphEnd {
                timestamp,
                final_state,
                duration,
                execution_path: _,
            } => {
                let event_type = EventType::GraphEnd;
                let node_id = "".to_string();
                let duration_us = duration_to_micros_i64(*duration);
                let ts = *timestamp;
                let attributes = std::collections::HashMap::new();

                // Send final state diff (with size limit check) via message queue (M-666 fix)
                if config.enable_state_diff {
                    if let Some(new_state_json) = serialize_state_with_limit(
                        final_state,
                        config.max_state_diff_size,
                        "final_state",
                    ) {
                        let old_state_opt = previous_state.lock().clone();
                        if let Some(old_state) = old_state_opt {
                            if let Ok(diff_result) = diff_states(&old_state, &new_state_json) {
                                // M-521: create_state_diff now returns Option to handle serialization failures
                                if let Some(state_diff) =
                                    self.create_state_diff(&diff_result, &new_state_json)
                                {
                                    // Route through message queue for ordering guarantee (M-666 fix)
                                    match self
                                        .message_sender
                                        .try_send(BatchMessage::StateDiff(state_diff))
                                    {
                                        Ok(()) => {
                                            // M-1040: Increment queue depth on successful send
                                            let depth =
                                                self.queue_depth.fetch_add(1, Ordering::Relaxed)
                                                    + 1;
                                            TELEMETRY_QUEUE_DEPTH.set(depth as i64);
                                        }
                                        Err(e) => {
                                            // M-694: Track StateDiff drops with message_type label
                                            self.telemetry_dropped.fetch_add(1, Ordering::Relaxed);
                                            TELEMETRY_DROPPED_TOTAL
                                                .with_label_values(&["state_diff", "queue_full"])
                                                .inc();
                                            tracing::warn!(
                                                thread_id = %config.thread_id,
                                                error = %e,
                                                metric = "dashstream_telemetry_dropped_total",
                                                message_type = "state_diff",
                                                reason = "queue_full",
                                                "Failed to queue final state diff (channel full or closed)"
                                            );
                                        }
                                    }
                                }
                                // M-671: Maybe emit checkpoint for resync support
                                self.maybe_emit_checkpoint(&new_state_json);
                            }
                        }
                    }
                }

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::NodeStart {
                timestamp,
                node,
                state: _,
                node_config,
            } => {
                let event_type = EventType::NodeStart;
                let node_id = node.clone();
                let duration_us = 0i64;
                let ts = *timestamp;
                let mut attributes = std::collections::HashMap::new();

                // Config Versioning - add config version and hash to telemetry
                if let Some(config) = node_config {
                    attributes.insert(
                        "config_version".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::IntValue(config.version as i64)),
                        },
                    );
                    attributes.insert(
                        "config_hash".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::StringValue(
                                config.config_hash.clone(),
                            )),
                        },
                    );
                    if let Some(updated_by) = &config.updated_by {
                        attributes.insert(
                            "config_updated_by".to_string(),
                            AttributeValue {
                                value: Some(attribute_value::Value::StringValue(
                                    updated_by.clone(),
                                )),
                            },
                        );
                    }
                }

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::NodeEnd {
                timestamp,
                node,
                state,
                duration,
                node_config,
            } => {
                let event_type = EventType::NodeEnd;
                let node_id = node.clone();
                let duration_us = duration_to_micros_i64(*duration);
                let ts = *timestamp;
                let mut attributes = std::collections::HashMap::new();

                // Config Versioning - add config version and hash to telemetry
                if let Some(cfg) = node_config {
                    attributes.insert(
                        "config_version".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::IntValue(cfg.version as i64)),
                        },
                    );
                    attributes.insert(
                        "config_hash".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::StringValue(
                                cfg.config_hash.clone(),
                            )),
                        },
                    );
                    if let Some(updated_by) = &cfg.updated_by {
                        attributes.insert(
                            "config_updated_by".to_string(),
                            AttributeValue {
                                value: Some(attribute_value::Value::StringValue(
                                    updated_by.clone(),
                                )),
                            },
                        );
                    }
                }

                // Send state diff after node execution (with size limit check) via message queue (M-666 fix)
                if config.enable_state_diff {
                    if let Some(new_state_json) = serialize_state_with_limit(
                        state,
                        config.max_state_diff_size,
                        &format!("node_end:{}", node),
                    ) {
                        // Snapshot previous state and update quickly to avoid holding a sync lock
                        // across potentially expensive diffing.
                        let old_state_opt = {
                            let mut prev_state = previous_state.lock();
                            let old = prev_state.clone();
                            *prev_state = Some(new_state_json.clone());
                            old
                        };

                        if let Some(old_state) = old_state_opt {
                            if let Ok(diff_result) = diff_states(&old_state, &new_state_json) {
                                // M-521: create_state_diff now returns Option to handle serialization failures
                                if let Some(state_diff) =
                                    self.create_state_diff(&diff_result, &new_state_json)
                                {
                                    // Route through message queue for ordering guarantee (M-666 fix)
                                    match self
                                        .message_sender
                                        .try_send(BatchMessage::StateDiff(state_diff))
                                    {
                                        Ok(()) => {
                                            // M-1040: Increment queue depth on successful send
                                            let depth =
                                                self.queue_depth.fetch_add(1, Ordering::Relaxed)
                                                    + 1;
                                            TELEMETRY_QUEUE_DEPTH.set(depth as i64);
                                        }
                                        Err(e) => {
                                            // M-694: Track StateDiff drops with message_type label
                                            self.telemetry_dropped.fetch_add(1, Ordering::Relaxed);
                                            TELEMETRY_DROPPED_TOTAL
                                                .with_label_values(&["state_diff", "queue_full"])
                                                .inc();
                                            tracing::warn!(
                                                thread_id = %config.thread_id,
                                                node_id = %node_id,
                                                error = %e,
                                                metric = "dashstream_telemetry_dropped_total",
                                                message_type = "state_diff",
                                                reason = "queue_full",
                                                "Failed to queue node state diff (channel full or closed)"
                                            );
                                        }
                                    }
                                }
                                // M-671: Maybe emit checkpoint for resync support
                                self.maybe_emit_checkpoint(&new_state_json);
                            }
                        }
                    }
                }

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::NodeError {
                timestamp,
                node,
                error,
                state: _,
            } => {
                let event_type = EventType::NodeError;
                let node_id = node.clone();
                let duration_us = 0i64;
                let ts = *timestamp;

                // Include error details in attributes for observability
                let mut attributes = std::collections::HashMap::new();
                attributes.insert(
                    "error".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::StringValue(error.clone())),
                    },
                );

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::EdgeTraversal {
                timestamp,
                from,
                to: _,
                edge_type,
            } => {
                let event_type = match edge_type {
                    EdgeType::Simple => EventType::EdgeTraversal,
                    EdgeType::Conditional { .. } => EventType::ConditionalBranch,
                    EdgeType::Parallel => EventType::EdgeTraversal,
                };
                let node_id = from.clone();
                let duration_us = 0i64;
                let ts = *timestamp;
                let attributes = std::collections::HashMap::new();

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::ParallelStart { timestamp, nodes } => {
                let event_type = EventType::ParallelStart;
                let node_id = nodes.join(",");
                let duration_us = 0i64;
                let ts = *timestamp;
                let attributes = std::collections::HashMap::new();

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::ParallelEnd {
                timestamp,
                nodes,
                duration,
            } => {
                let event_type = EventType::ParallelEnd;
                let node_id = nodes.join(",");
                let duration_us = duration_to_micros_i64(*duration);
                let ts = *timestamp;
                let attributes = std::collections::HashMap::new();

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            // Optimization Telemetry - meta-learning events
            GraphEvent::OptimizationStart {
                timestamp,
                optimization_id,
                target_node,
                target_param,
                strategy,
            } => {
                let event_type = EventType::OptimizationStart;
                let node_id = target_node.clone();
                let duration_us = 0i64;
                let ts = *timestamp;
                let mut attributes = std::collections::HashMap::new();

                attributes.insert(
                    "optimization_id".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::StringValue(optimization_id.clone())),
                    },
                );
                attributes.insert(
                    "target_param".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::StringValue(target_param.clone())),
                    },
                );
                if let Some(strat) = strategy {
                    attributes.insert(
                        "strategy".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::StringValue(strat.clone())),
                        },
                    );
                }

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            GraphEvent::OptimizationEnd { timestamp, trace } => {
                let event_type = EventType::OptimizationEnd;
                let node_id = trace.target_node.clone();
                let duration_us = (trace.total_duration_ms * 1000) as i64; // ms to us
                let ts = *timestamp;
                let mut attributes = std::collections::HashMap::new();

                attributes.insert(
                    "optimization_id".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::StringValue(
                            trace.optimization_id.clone(),
                        )),
                    },
                );
                attributes.insert(
                    "variants_tested".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::IntValue(
                            trace.variants_tested.len() as i64,
                        )),
                    },
                );
                attributes.insert(
                    "improvement_delta".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::FloatValue(trace.improvement_delta)),
                    },
                );
                attributes.insert(
                    "termination_reason".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::StringValue(
                            trace.termination_reason.description(),
                        )),
                    },
                );
                if let Some(strat) = &trace.strategy {
                    attributes.insert(
                        "strategy".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::StringValue(strat.clone())),
                        },
                    );
                }
                if let Some(best) = &trace.best_variant {
                    attributes.insert(
                        "best_score".to_string(),
                        AttributeValue {
                            value: Some(attribute_value::Value::FloatValue(best.score)),
                        },
                    );
                }
                attributes.insert(
                    "found_improvement".to_string(),
                    AttributeValue {
                        value: Some(attribute_value::Value::BoolValue(trace.found_improvement())),
                    },
                );

                Some((event_type, node_id, duration_us, ts, attributes))
            }
            // Observability Phase 3/4 events - handled by WAL system, not DashStream
            // These events are for local historical queries and Learning Corpus,
            // not for real-time streaming to the UI.
            GraphEvent::EdgeEvaluated { .. }
            | GraphEvent::StateChanged { .. }
            | GraphEvent::DecisionMade { .. }
            | GraphEvent::OutcomeObserved { .. } => None,
        };

        if let Some((event_type, node_id, duration_us, _timestamp, mut attributes)) = event_data {
            // M-1065: Redact sensitive data from event attributes before sending
            // This ensures API keys, tokens, and passwords are not leaked to Kafka/UI
            redact_attributes(&mut attributes);

            let event = Event {
                header: Some(self.create_header(MessageType::Event)),
                event_type: event_type as i32,
                node_id: node_id.clone(),
                attributes,
                duration_us,
                llm_request_id: "".to_string(),
            };

            // Route through message queue for ordering guarantee (M-666 fix)
            // Queue event - non-blocking try_send
            match self.message_sender.try_send(BatchMessage::Event(event)) {
                Ok(()) => {
                    // M-1040: Increment queue depth on successful send
                    let depth = self.queue_depth.fetch_add(1, Ordering::Relaxed) + 1;
                    TELEMETRY_QUEUE_DEPTH.set(depth as i64);
                }
                Err(_) => {
                    // Channel full or closed - drop event (best-effort semantics)
                    // M-694: Drops tracked with message_type and reason labels
                    let dropped = self.telemetry_dropped.fetch_add(1, Ordering::Relaxed) + 1;
                    TELEMETRY_DROPPED_TOTAL
                        .with_label_values(&["event", "queue_full"])
                        .inc();
                    if dropped % 100 == 1 {
                        tracing::warn!(
                            thread_id = %config.thread_id,
                            dropped_count = dropped,
                            batch_size = self.config.telemetry_batch_size,
                            metric = "dashstream_telemetry_dropped_total",
                            message_type = "event",
                            reason = "queue_full",
                            "Telemetry dropped: message queue full. \
                             Monitor dashstream_telemetry_dropped_total metric for alerting."
                        );
                    }
                }
            }
        }
    }

    /// Create a StateDiff message from DiffResult
    ///
    /// Handles serialization errors gracefully:
    /// - If diff serialization fails, falls back to full state with warning
    /// - If full state serialization fails, logs error and returns None (M-521 fix)
    fn create_state_diff(
        &self,
        diff_result: &DiffResult,
        new_state_json: &serde_json::Value,
    ) -> Option<StateDiff> {
        use dashflow_streaming::diff::protobuf::patch_to_proto;

        // Try to serialize the diff, fall back to full state on error
        let (operations, use_full_state) = if diff_result.use_full_state {
            // M-699: Track degraded mode - diff algorithm determined full state is needed
            STATE_DIFF_DEGRADED_TOTAL
                .with_label_values(&["full_state_fallback"])
                .inc();
            (vec![], true)
        } else {
            match patch_to_proto(&diff_result.patch) {
                Ok(ops) => (ops, false),
                Err(e) => {
                    tracing::warn!(
                        thread_id = %self.config.thread_id,
                        "Failed to serialize state diff, falling back to full state: {e}"
                    );
                    // M-699: Track degraded mode - patch serialization failed
                    STATE_DIFF_DEGRADED_TOTAL
                        .with_label_values(&["patch_serialization_failed"])
                        .inc();
                    (vec![], true) // Fall back to full state
                }
            }
        };

        // Serialize full state if needed
        // M-521: Return None if full state serialization fails (instead of invalid empty message)
        let full_state = if use_full_state {
            match serde_json::to_vec(new_state_json) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::error!(
                        thread_id = %self.config.thread_id,
                        "Failed to serialize full state - StateDiff omitted (M-521): {e}"
                    );
                    // M-521: Track full state serialization failures
                    STATE_DIFF_DEGRADED_TOTAL
                        .with_label_values(&["full_state_serialization_failed"])
                        .inc();
                    // M-521: Return None instead of sending invalid empty message
                    return None;
                }
            }
        } else {
            vec![]
        };

        // Convert hex hash string to bytes with proper error handling.
        // Do not silently drop invalid bytes: treat as invalid and omit state_hash.
        let state_hash = match hex::decode(&diff_result.state_hash) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(
                    thread_id = %self.config.thread_id,
                    error = %e,
                    "Invalid state hash hex; omitting state_hash bytes"
                );
                Vec::new()
            }
        };

        // Get the base checkpoint ID for delta chain reconstruction (M-671 fix)
        let base_checkpoint_id = self.last_checkpoint_id.lock().clone();

        Some(StateDiff {
            header: Some(self.create_header(MessageType::StateDiff)),
            base_checkpoint_id,
            operations,
            state_hash,
            full_state,
        })
    }

    /// Create a checkpoint message with full state snapshot (M-671: resync support)
    ///
    /// Checkpoints enable UI resync after dropped StateDiff messages by providing
    /// complete state snapshots at regular intervals.
    fn create_checkpoint(&self, state_json: &serde_json::Value) -> Option<Checkpoint> {
        use sha2::{Digest, Sha256};

        // Generate new checkpoint ID
        let checkpoint_id = uuid::Uuid::new_v4().as_bytes().to_vec();

        // Serialize state to JSON bytes
        let state_bytes = match serde_json::to_vec(state_json) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(
                    thread_id = %self.config.thread_id,
                    error = %e,
                    "Failed to serialize checkpoint state"
                );
                return None;
            }
        };

        // Calculate checksum
        let mut hasher = Sha256::new();
        hasher.update(&state_bytes);
        let checksum = hasher.finalize().to_vec();

        // Optionally compress the state (using zstd if available)
        let (final_state, compression_info) =
            if state_bytes.len() >= self.config.compression_threshold {
                match zstd::bulk::compress(&state_bytes, 3) {
                    Ok(compressed) => {
                        let compression_ratio = if !compressed.is_empty() {
                            state_bytes.len() as f32 / compressed.len() as f32
                        } else {
                            1.0
                        };
                        (
                            compressed.clone(),
                            Some(CompressionInfo {
                                r#type: CompressionType::CompressionZstd as i32,
                                compressed_size: compressed.len() as u64,
                                uncompressed_size: state_bytes.len() as u64,
                                compression_ratio,
                            }),
                        )
                    }
                    Err(e) => {
                        tracing::debug!(
                            thread_id = %self.config.thread_id,
                            error = %e,
                            "Compression failed, using uncompressed state"
                        );
                        (state_bytes.clone(), None)
                    }
                }
            } else {
                (state_bytes.clone(), None)
            };

        // Update last checkpoint ID for subsequent StateDiffs (M-671 fix)
        *self.last_checkpoint_id.lock() = checkpoint_id.clone();

        Some(Checkpoint {
            header: Some(self.create_header(MessageType::Checkpoint)),
            checkpoint_id,
            state: final_state,
            state_type: "serde_json::Value".to_string(),
            checksum,
            storage_uri: String::new(), // Not using external storage
            compression_info,
            metadata: Default::default(),
        })
    }

    /// Check if a checkpoint should be emitted and emit it if needed (M-671: resync support)
    ///
    /// This is called after each state diff. If checkpoint_interval is set and
    /// enough diffs have occurred since the last checkpoint, emit a checkpoint.
    ///
    /// M-811 fix: Uses compare_exchange_weak to atomically claim checkpoint emission slot,
    /// preventing duplicate checkpoints under concurrent calls.
    fn maybe_emit_checkpoint(&self, state_json: &serde_json::Value) {
        if self.config.checkpoint_interval == 0 {
            return; // Checkpointing disabled
        }

        let old = self.diffs_since_checkpoint.fetch_add(1, Ordering::Relaxed);
        let new = old + 1;
        if new >= self.config.checkpoint_interval {
            // M-811 fix: Atomically claim the checkpoint emission slot.
            // Only the thread that successfully resets the counter emits a checkpoint.
            // This prevents duplicate checkpoints when multiple threads cross the threshold.
            if self
                .diffs_since_checkpoint
                .compare_exchange_weak(new, 0, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                if let Some(checkpoint) = self.create_checkpoint(state_json) {
                    match self
                        .message_sender
                        .try_send(BatchMessage::Checkpoint(checkpoint))
                    {
                        Ok(()) => {
                            // M-1040: Increment queue depth on successful send
                            let depth = self.queue_depth.fetch_add(1, Ordering::Relaxed) + 1;
                            TELEMETRY_QUEUE_DEPTH.set(depth as i64);
                            tracing::debug!(
                                thread_id = %self.config.thread_id,
                                diffs_since_last = new,
                                "Emitted checkpoint for resync support"
                            );
                        }
                        Err(e) => {
                            // M-694: Track Checkpoint drops with message_type label
                            self.telemetry_dropped.fetch_add(1, Ordering::Relaxed);
                            TELEMETRY_DROPPED_TOTAL
                                .with_label_values(&["checkpoint", "queue_full"])
                                .inc();
                            tracing::warn!(
                                thread_id = %self.config.thread_id,
                                error = %e,
                                metric = "dashstream_telemetry_dropped_total",
                                message_type = "checkpoint",
                                reason = "queue_full",
                                "Failed to queue checkpoint (channel full or closed)"
                            );
                        }
                    }
                }
            }
            // else: another thread already claimed and reset the counter - that's fine,
            // they will emit the checkpoint instead
        }
    }

    /// Emit quality metrics to Kafka for the quality aggregator.
    ///
    /// This method sends a `Metrics` message with `scope="quality"` that the
    /// quality aggregator can process. The aggregator expects metrics with
    /// quality scores (accuracy, relevance, completeness) that it will expose
    /// to Prometheus for Grafana dashboards.
    ///
    /// # Arguments
    ///
    /// * `accuracy` - Quality accuracy score (0.0 to 1.0)
    /// * `relevance` - Quality relevance score (0.0 to 1.0)
    /// * `completeness` - Quality completeness score (0.0 to 1.0)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // After quality gate evaluation
    /// callback.emit_quality_metrics(0.95, 0.87, 0.92, true).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the Kafka producer fails to send the message.
    pub async fn emit_quality_metrics(
        &self,
        accuracy: f64,
        relevance: f64,
        completeness: f64,
        passed: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::collections::HashMap;

        // Build metrics map with quality scores
        let mut metrics: HashMap<String, MetricValue> = HashMap::new();

        // Add accuracy metric
        metrics.insert(
            "accuracy".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(accuracy)),
                unit: "ratio".to_string(),
                r#type: metric_value::MetricType::Gauge as i32,
            },
        );

        // Add relevance metric
        metrics.insert(
            "relevance".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(relevance)),
                unit: "ratio".to_string(),
                r#type: metric_value::MetricType::Gauge as i32,
            },
        );

        // Add completeness metric
        metrics.insert(
            "completeness".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(completeness)),
                unit: "ratio".to_string(),
                r#type: metric_value::MetricType::Gauge as i32,
            },
        );

        // Add passed boolean - critical for prometheus-exporter to count passed/failed queries
        metrics.insert(
            "passed".to_string(),
            MetricValue {
                value: Some(metric_value::Value::BoolValue(passed)),
                unit: "bool".to_string(),
                r#type: metric_value::MetricType::Gauge as i32,
            },
        );

        // Add quality_score - average of accuracy, relevance, completeness for prometheus-exporter
        let quality_score = (accuracy + relevance + completeness) / 3.0;
        metrics.insert(
            "quality_score".to_string(),
            MetricValue {
                value: Some(metric_value::Value::FloatValue(quality_score)),
                unit: "ratio".to_string(),
                r#type: metric_value::MetricType::Gauge as i32,
            },
        );

        // Build tags for filtering/grouping
        let mut tags: HashMap<String, String> = HashMap::new();
        tags.insert("thread_id".to_string(), self.config.thread_id.clone());
        tags.insert("tenant_id".to_string(), self.config.tenant_id.clone());

        // Create Metrics message with quality scope
        let metrics_msg = Metrics {
            header: Some(self.create_header(MessageType::Metrics)),
            scope: "quality".to_string(),
            scope_id: self.config.thread_id.clone(),
            metrics,
            tags,
        };

        // Send via producer
        // M-1000: Count metrics send failures for alerting (count before propagating error)
        if let Err(e) = self.producer.send_metrics(metrics_msg).await {
            TELEMETRY_SEND_FAILURES_TOTAL
                .with_label_values(&["metrics"])
                .inc();
            return Err(e.into());
        }

        tracing::debug!(
            thread_id = %self.config.thread_id,
            accuracy = accuracy,
            relevance = relevance,
            completeness = completeness,
            "Emitted quality metrics to Kafka"
        );

        Ok(())
    }

    /// Emit quality metrics asynchronously (non-blocking).
    ///
    /// This method spawns a tracked task to send quality metrics without blocking
    /// the caller. Use this when you don't need to wait for confirmation that
    /// the metrics were sent.
    ///
    /// # Arguments
    ///
    /// * `accuracy` - Quality accuracy score (0.0 to 1.0)
    /// * `relevance` - Quality relevance score (0.0 to 1.0)
    /// * `completeness` - Quality completeness score (0.0 to 1.0)
    /// * `passed` - Whether the quality check passed the threshold
    pub fn emit_quality_metrics_async(
        &self,
        accuracy: f64,
        relevance: f64,
        completeness: f64,
        passed: bool,
    ) {
        let producer = self.producer.clone();
        let thread_id = self.config.thread_id.clone();
        let tenant_id = self.config.tenant_id.clone();
        let header = self.create_header(MessageType::Metrics);

        self.spawn_tracked("metrics", async move {
            use std::collections::HashMap;

            let mut metrics: HashMap<String, MetricValue> = HashMap::new();

            metrics.insert(
                "accuracy".to_string(),
                MetricValue {
                    value: Some(metric_value::Value::FloatValue(accuracy)),
                    unit: "ratio".to_string(),
                    r#type: metric_value::MetricType::Gauge as i32,
                },
            );

            metrics.insert(
                "relevance".to_string(),
                MetricValue {
                    value: Some(metric_value::Value::FloatValue(relevance)),
                    unit: "ratio".to_string(),
                    r#type: metric_value::MetricType::Gauge as i32,
                },
            );

            metrics.insert(
                "completeness".to_string(),
                MetricValue {
                    value: Some(metric_value::Value::FloatValue(completeness)),
                    unit: "ratio".to_string(),
                    r#type: metric_value::MetricType::Gauge as i32,
                },
            );

            // Add passed boolean - critical for prometheus-exporter to count passed/failed queries
            metrics.insert(
                "passed".to_string(),
                MetricValue {
                    value: Some(metric_value::Value::BoolValue(passed)),
                    unit: "bool".to_string(),
                    r#type: metric_value::MetricType::Gauge as i32,
                },
            );

            // Add quality_score - average of accuracy, relevance, completeness for prometheus-exporter
            let quality_score = (accuracy + relevance + completeness) / 3.0;
            metrics.insert(
                "quality_score".to_string(),
                MetricValue {
                    value: Some(metric_value::Value::FloatValue(quality_score)),
                    unit: "ratio".to_string(),
                    r#type: metric_value::MetricType::Gauge as i32,
                },
            );

            let mut tags: HashMap<String, String> = HashMap::new();
            tags.insert("thread_id".to_string(), thread_id.clone());
            tags.insert("tenant_id".to_string(), tenant_id);

            let metrics_msg = Metrics {
                header: Some(header),
                scope: "quality".to_string(),
                scope_id: thread_id.clone(),
                metrics,
                tags,
            };

            if let Err(e) = producer.send_metrics(metrics_msg).await {
                // M-1000: Count metrics send failures for alerting
                TELEMETRY_SEND_FAILURES_TOTAL
                    .with_label_values(&["metrics"])
                    .inc();
                tracing::warn!(
                    thread_id = %thread_id,
                    accuracy = accuracy,
                    relevance = relevance,
                    completeness = completeness,
                    metric = "dashstream_telemetry_send_failures_total",
                    message_type = "metrics",
                    "Failed to send quality metrics: {e}"
                );
            }
        });
    }

    /// Flush pending telemetry tasks and Kafka messages
    ///
    /// This method ensures all spawned telemetry tasks complete before returning,
    /// preventing data loss on shutdown. Call this before dropping the callback
    /// to ensure all telemetry is delivered.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // After graph execution
    /// callback.flush().await?;
    /// // Now safe to shutdown
    /// ```
    pub async fn flush(&self) -> Result<(), Box<dyn std::error::Error>> {
        // First, wait for all pending telemetry tasks to complete
        // Take the tasks out of the mutex to avoid holding the lock during await
        let tasks: Vec<JoinHandle<()>> = {
            let mut pending = self.pending_tasks.lock();
            std::mem::take(&mut *pending)
        };

        let task_count = tasks.len();
        if task_count > 0 {
            tracing::debug!(
                thread_id = %self.config.thread_id,
                task_count = task_count,
                "Waiting for pending telemetry tasks to complete"
            );

            // Bound flush time so shutdown doesn't hang forever on a stuck task.
            // Uses SHORT_TIMEOUT (5s) from centralized constants - appropriate for fast operations
            let deadline = tokio::time::Instant::now() + SHORT_TIMEOUT;

            let mut failed = 0usize;
            let mut still_pending: Vec<JoinHandle<()>> = Vec::new();

            for mut handle in tasks {
                let remaining = deadline
                    .checked_duration_since(tokio::time::Instant::now())
                    .unwrap_or_default();
                if remaining.is_zero() {
                    still_pending.push(handle);
                    continue;
                }

                tokio::select! {
                    res = &mut handle => {
                        if res.is_err() {
                            failed += 1;
                        }
                    }
                    _ = tokio::time::sleep(remaining) => {
                        still_pending.push(handle);
                    }
                };
            }

            if failed > 0 {
                tracing::warn!(
                    thread_id = %self.config.thread_id,
                    failed_count = failed,
                    total_count = task_count,
                    "Some telemetry tasks failed during flush"
                );
            }

            if !still_pending.is_empty() {
                let timed_out = still_pending.len();
                {
                    let mut pending = self.pending_tasks.lock();
                    pending.extend(still_pending);
                }
                tracing::warn!(
                    thread_id = %self.config.thread_id,
                    timed_out_count = timed_out,
                    total_count = task_count,
                    "Timed out waiting for telemetry tasks; some tasks still running"
                );
            }
        }

        // Request an explicit message queue flush.
        // We cannot rely on dropping all senders because DashStreamCallback is Clone.
        let (ack_tx, ack_rx) = oneshot::channel();
        tracing::debug!(
            thread_id = %self.config.thread_id,
            "Requesting message worker flush"
        );

        // Send flush request with a bounded timeout to avoid hanging on full channels.
        // M-519: Use configurable timeout (DASHFLOW_FLUSH_TIMEOUT_SECS env var)
        let flush_timeout = Duration::from_secs(self.config.flush_timeout_secs);
        match tokio::time::timeout(
            flush_timeout,
            self.message_sender.send(BatchMessage::Flush(ack_tx)),
        )
        .await
        {
            Err(_) => {
                tracing::warn!(
                    thread_id = %self.config.thread_id,
                    timeout_secs = self.config.flush_timeout_secs,
                    "Timed out sending flush request to message worker"
                );
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    thread_id = %self.config.thread_id,
                    error = %e,
                    "Failed to send flush request to message worker"
                );
            }
            Ok(Ok(())) => match tokio::time::timeout(flush_timeout, ack_rx).await {
                Err(_) => tracing::warn!(
                    thread_id = %self.config.thread_id,
                    "Message worker did not acknowledge flush within timeout"
                ),
                Ok(Err(e)) => tracing::warn!(
                    thread_id = %self.config.thread_id,
                    error = %e,
                    "Message worker flush acknowledgment failed"
                ),
                Ok(Ok(())) => {}
            },
        }

        // Then flush the Kafka producer with same configurable timeout
        self.producer
            .flush(std::time::Duration::from_secs(
                self.config.flush_timeout_secs,
            ))
            .await?;
        Ok(())
    }

    /// Get the number of pending telemetry tasks
    ///
    /// Useful for monitoring backpressure or debugging.
    #[must_use]
    pub fn pending_task_count(&self) -> usize {
        let tasks = self.pending_tasks.lock();
        tasks.iter().filter(|h| !h.is_finished()).count()
    }
}

impl<S> EventCallback<S> for DashStreamCallback<S>
where
    S: GraphState + Serialize,
{
    fn on_event(&self, event: &GraphEvent<S>) {
        self.send_graph_event(event);
    }

    /// Expose producer for intra-node streaming
    fn get_producer(&self) -> Option<Arc<dashflow_streaming::producer::DashStreamProducer>> {
        Some(self.producer.clone())
    }

    /// Expose thread and tenant IDs
    fn get_ids(&self) -> Option<(String, String)> {
        Some((self.config.thread_id.clone(), self.config.tenant_id.clone()))
    }
}

/// Graceful cleanup for DashStreamCallback on drop
///
/// This ensures that pending telemetry tasks and batch workers are signaled
/// to shut down when the callback is dropped without calling `flush()`.
/// Since Drop cannot be async, this does a best-effort synchronous cleanup:
///
/// 1. Aborts pending telemetry tasks (they will be cancelled)
/// 2. Aborts the batch worker if running (sender drops afterward)
/// 3. Does NOT wait for workers to finish (would require async)
///
/// For graceful shutdown that waits for all telemetry to be sent,
/// call `flush().await` before dropping the callback.
impl<S> Drop for DashStreamCallback<S>
where
    S: GraphState + Serialize,
{
    fn drop(&mut self) {
        // Abort pending telemetry tasks (best effort - avoids task leaks)
        // Note: parking_lot::Mutex::lock() returns MutexGuard directly (no Result - poison-free)
        let mut tasks = self.pending_tasks.lock();
        let task_count = tasks.len();
        for handle in tasks.drain(..) {
            handle.abort();
        }
        if task_count > 0 {
            tracing::debug!(
                thread_id = %self.config.thread_id,
                aborted_tasks = task_count,
                "Aborted pending telemetry tasks on drop (call flush() for graceful shutdown)"
            );
        }
        drop(tasks); // Release lock before acquiring next

        // Abort batch worker if still running.
        // If not aborted, dropping the sender(s) would eventually signal shutdown.
        let mut worker = self.message_worker.lock();
        if let Some(handle) = worker.take() {
            handle.abort();
            tracing::debug!(
                thread_id = %self.config.thread_id,
                "Aborted batch worker on drop (call flush() for graceful shutdown)"
            );
        }
    }
}

#[cfg(test)]
mod tests;
