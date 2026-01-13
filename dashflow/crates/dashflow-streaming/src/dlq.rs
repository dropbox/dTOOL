// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Dead Letter Queue (DLQ) Handler for DashFlow Streaming
//
// Provides centralized handling of failed messages for forensic analysis.
// Failed messages (decode errors, decompression failures, etc.) are sent
// to a dedicated Kafka DLQ topic with full error context for debugging.

use crate::errors::Result;
use std::sync::LazyLock;
use prometheus::Counter;
use rdkafka::producer::Producer;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

// Prometheus metrics for DLQ operations (M-624: Use centralized constants)
// M-98: Counter metrics include _total suffix for Prometheus naming convention
use crate::metrics_constants::{
    METRIC_DLQ_DROPPED_TOTAL, METRIC_DLQ_SEND_FAILURES_TOTAL, METRIC_DLQ_SEND_RETRIES_TOTAL,
    METRIC_DLQ_SENDS_TOTAL,
};

static DLQ_SENDS_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_DLQ_SENDS_TOTAL,
        "Total number of messages sent to DLQ",
    )
});
static DLQ_SEND_FAILURES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_DLQ_SEND_FAILURES_TOTAL,
        "Total number of DLQ send failures",
    )
});
static DLQ_DROPPED_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_DLQ_DROPPED_TOTAL,
        "Total number of DLQ messages dropped due to backpressure",
    )
});
static DLQ_SEND_RETRIES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_DLQ_SEND_RETRIES_TOTAL,
        "Total number of DLQ send retries",
    )
});

/// Dead Letter Queue message format
///
/// Contains the original failed payload and complete error context
/// for forensic analysis and potential replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqMessage {
    /// Base64-encoded original payload that failed to process
    pub original_payload_base64: String,

    /// Optional: original payload size in bytes (before truncation).
    ///
    /// Present when payload is truncated to keep DLQ messages within broker limits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_payload_size_bytes: Option<u64>,

    /// Optional: number of payload bytes actually included (after truncation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_payload_included_bytes: Option<u64>,

    /// Optional: true if `original_payload_base64` contains a truncated prefix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_payload_truncated: Option<bool>,

    /// Optional: SHA256 of the full original payload (hex).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_payload_sha256: Option<String>,

    /// Error message describing why processing failed
    pub error: String,

    /// Source Kafka topic the message came from
    pub source_topic: String,

    /// Source partition within the topic
    pub source_partition: i32,

    /// Source offset within the partition
    pub source_offset: i64,

    /// ISO 8601 timestamp when the error occurred
    pub timestamp: String,

    /// Consumer ID that encountered the error
    pub consumer_id: String,

    /// Optional: extracted thread_id from malformed message (best-effort)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,

    /// Optional: extracted tenant_id from malformed message (best-effort)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    /// Error classification (decode_error, compression_error, etc.)
    pub error_type: String,

    /// Unique trace ID for correlating logs
    pub trace_id: String,

    /// Optional: node_id where the error occurred (for graph execution context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,

    /// Optional: parent trace ID for distributed tracing linkage
    /// Use this to correlate DLQ messages back to the original execution trace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_trace_id: Option<String>,

    /// Optional: span ID for precise tracing context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
}

impl DlqMessage {
    /// Creates a new DLQ message with required context
    pub fn new(
        original_payload: &[u8],
        error: impl Into<String>,
        source_topic: impl Into<String>,
        source_partition: i32,
        source_offset: i64,
        consumer_id: impl Into<String>,
        error_type: impl Into<String>,
    ) -> Self {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::engine::Engine;

        // Avoid producing DLQ messages that exceed broker message limits by
        // bounding the stored payload (base64 expands by ~33% + JSON overhead).
        //
        // When truncated, we also include metadata and a SHA256 of the full payload
        // to support forensics and correlation.
        const MAX_ORIGINAL_PAYLOAD_BYTES: usize = 512 * 1024; // 512KB
        let full_len = original_payload.len();
        let (payload_to_encode, truncated) = if full_len > MAX_ORIGINAL_PAYLOAD_BYTES {
            (&original_payload[..MAX_ORIGINAL_PAYLOAD_BYTES], true)
        } else {
            (original_payload, false)
        };

        let (
            original_payload_size_bytes,
            original_payload_included_bytes,
            original_payload_truncated,
            original_payload_sha256,
        ) = if truncated {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(original_payload);
            let sha256 = hasher.finalize();
            (
                Some(full_len as u64),
                Some(payload_to_encode.len() as u64),
                Some(true),
                Some(hex::encode(sha256)),
            )
        } else {
            (None, None, None, None)
        };

        Self {
            original_payload_base64: BASE64.encode(payload_to_encode),
            original_payload_size_bytes,
            original_payload_included_bytes,
            original_payload_truncated,
            original_payload_sha256,
            error: error.into(),
            source_topic: source_topic.into(),
            source_partition,
            source_offset,
            timestamp: chrono::Utc::now().to_rfc3339(),
            consumer_id: consumer_id.into(),
            thread_id: None,
            tenant_id: None,
            error_type: error_type.into(),
            trace_id: uuid::Uuid::new_v4().to_string(),
            node_id: None,
            parent_trace_id: None,
            span_id: None,
        }
    }

    /// Sets optional thread_id (extracted from malformed message)
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Sets optional tenant_id (extracted from malformed message)
    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Sets a custom trace_id (instead of generating one)
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = trace_id.into();
        self
    }

    /// Sets the node_id where the error occurred (for graph execution context)
    ///
    /// This helps operators identify which node in a graph execution failed,
    /// enabling faster triage and correlation with execution logs.
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    /// Sets the parent trace ID for distributed tracing linkage
    ///
    /// Use this to link DLQ messages back to the original execution trace.
    /// Typically extracted from OpenTelemetry context or similar.
    pub fn with_parent_trace_id(mut self, parent_trace_id: impl Into<String>) -> Self {
        self.parent_trace_id = Some(parent_trace_id.into());
        self
    }

    /// Sets the span ID for precise tracing context
    ///
    /// Combined with parent_trace_id, this enables exact correlation
    /// to the specific span where the failure occurred.
    pub fn with_span_id(mut self, span_id: impl Into<String>) -> Self {
        self.span_id = Some(span_id.into());
        self
    }

    /// Captures tracing context from the current OpenTelemetry span
    ///
    /// Automatically extracts trace_id and span_id from the current
    /// OpenTelemetry context, enabling seamless distributed tracing.
    pub fn with_current_trace_context(mut self) -> Self {
        use opentelemetry::trace::TraceContextExt;

        let context = opentelemetry::Context::current();
        let span = context.span();
        let span_context = span.span_context();

        if span_context.is_valid() {
            self.parent_trace_id = Some(span_context.trace_id().to_string());
            self.span_id = Some(span_context.span_id().to_string());
        }

        self
    }

    /// Converts to JSON string for Kafka payload
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::errors::Error::Serialization(format!("Failed to serialize DLQ message: {}", e))
        })
    }

    /// S-15: Validates the SHA256 hash of the original payload on replay.
    ///
    /// When a DLQ message was created from a truncated payload, the full original
    /// payload's SHA256 hash is stored in `original_payload_sha256`. This method
    /// allows forensic validation that a replayed payload matches the original.
    ///
    /// # Arguments
    ///
    /// * `full_payload` - The full original payload bytes to validate
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Hash matches, or payload was not truncated (no hash to check)
    /// * `Ok(false)` - Hash does not match (data integrity issue)
    /// * `Err(_)` - Payload was truncated but no SHA256 was stored (inconsistent state)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let dlq_msg: DlqMessage = serde_json::from_str(&json)?;
    /// if dlq_msg.is_truncated() {
    ///     // Retrieve full payload from archive/backup
    ///     let full_payload = retrieve_from_archive(&dlq_msg.trace_id)?;
    ///     match dlq_msg.validate_payload_sha256(&full_payload) {
    ///         Ok(true) => println!("Payload integrity verified"),
    ///         Ok(false) => eprintln!("WARNING: Payload hash mismatch!"),
    ///         Err(e) => eprintln!("Cannot validate: {}", e),
    ///     }
    /// }
    /// ```
    pub fn validate_payload_sha256(&self, full_payload: &[u8]) -> Result<bool> {
        // If not truncated, nothing to validate
        if self.original_payload_truncated != Some(true) {
            return Ok(true);
        }

        // If truncated, we must have a SHA256 to validate against
        let expected_sha256 = self.original_payload_sha256.as_ref().ok_or_else(|| {
            crate::errors::Error::InvalidFormat(
                "DLQ message marked as truncated but missing original_payload_sha256".to_string(),
            )
        })?;

        // Compute SHA256 of the provided full payload
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(full_payload);
        let computed_sha256 = hex::encode(hasher.finalize());

        Ok(computed_sha256 == *expected_sha256)
    }

    /// Returns true if the original payload was truncated when this DLQ message was created.
    ///
    /// Truncated payloads have `original_payload_sha256` stored for forensic validation
    /// using `validate_payload_sha256()`.
    pub fn is_truncated(&self) -> bool {
        self.original_payload_truncated == Some(true)
    }

    /// Returns the expected size of the original payload in bytes, if known.
    ///
    /// This is only populated when the payload was truncated.
    pub fn original_size(&self) -> Option<u64> {
        self.original_payload_size_bytes
    }
}

/// Default maximum concurrent fire-and-forget DLQ sends
pub const DEFAULT_MAX_CONCURRENT_DLQ_SENDS: usize = 100;

/// Default number of retry attempts for DLQ sends
pub const DEFAULT_DLQ_RETRY_ATTEMPTS: u32 = 3;

/// Default base delay for exponential backoff (milliseconds)
pub const DEFAULT_DLQ_RETRY_BASE_DELAY_MS: u64 = 100;

/// Default maximum delay for exponential backoff (milliseconds)
pub const DEFAULT_DLQ_RETRY_MAX_DELAY_MS: u64 = 5000;

/// Configuration for DLQ retry behavior
#[derive(Debug, Clone)]
pub struct DlqRetryConfig {
    /// Maximum number of retry attempts (default: 3)
    pub max_attempts: u32,
    /// Base delay for exponential backoff in milliseconds (default: 100ms)
    pub base_delay_ms: u64,
    /// Maximum delay cap for exponential backoff in milliseconds (default: 5000ms)
    pub max_delay_ms: u64,
}

impl Default for DlqRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_DLQ_RETRY_ATTEMPTS,
            base_delay_ms: DEFAULT_DLQ_RETRY_BASE_DELAY_MS,
            max_delay_ms: DEFAULT_DLQ_RETRY_MAX_DELAY_MS,
        }
    }
}

/// Handler for sending failed messages to Dead Letter Queue
///
/// Provides centralized error handling with full forensic context.
/// All consumers can use this to report failures consistently.
///
/// # Backpressure
///
/// Fire-and-forget sends are limited by a semaphore to prevent unbounded task spawning.
/// When the limit is reached, new sends are dropped to avoid unbounded memory growth.
///
/// # Retry Support
///
/// Use `send_with_retry()` for automatic retry with exponential backoff.
/// This is recommended for critical DLQ messages where loss is unacceptable.
pub struct DlqHandler {
    producer: FutureProducer,
    topic: String,
    timeout: Duration,
    /// Semaphore for backpressure on fire-and-forget sends
    send_semaphore: Arc<Semaphore>,
    max_concurrent: usize,
    /// Retry configuration
    retry_config: DlqRetryConfig,
}

impl DlqHandler {
    /// Creates a new DLQ handler with default backpressure and retry settings
    ///
    /// # Arguments
    ///
    /// * `producer` - Kafka producer for sending to DLQ topic
    /// * `topic` - DLQ topic name (e.g., "dashstream-dlq")
    /// * `timeout` - Timeout for DLQ sends (default: 5 seconds)
    pub fn new(producer: FutureProducer, topic: impl Into<String>, timeout: Duration) -> Self {
        Self::with_config(
            producer,
            topic,
            timeout,
            DEFAULT_MAX_CONCURRENT_DLQ_SENDS,
            DlqRetryConfig::default(),
        )
    }

    /// Creates a new DLQ handler with custom backpressure limit
    ///
    /// # Arguments
    ///
    /// * `producer` - Kafka producer for sending to DLQ topic
    /// * `topic` - DLQ topic name (e.g., "dashstream-dlq")
    /// * `timeout` - Timeout for DLQ sends
    /// * `max_concurrent` - Maximum concurrent fire-and-forget sends
    pub fn with_max_concurrent(
        producer: FutureProducer,
        topic: impl Into<String>,
        timeout: Duration,
        max_concurrent: usize,
    ) -> Self {
        Self::with_config(
            producer,
            topic,
            timeout,
            max_concurrent,
            DlqRetryConfig::default(),
        )
    }

    /// Creates a new DLQ handler with full configuration
    ///
    /// # Arguments
    ///
    /// * `producer` - Kafka producer for sending to DLQ topic
    /// * `topic` - DLQ topic name (e.g., "dashstream-dlq")
    /// * `timeout` - Timeout for DLQ sends
    /// * `max_concurrent` - Maximum concurrent fire-and-forget sends
    /// * `retry_config` - Configuration for retry behavior
    pub fn with_config(
        producer: FutureProducer,
        topic: impl Into<String>,
        timeout: Duration,
        max_concurrent: usize,
        mut retry_config: DlqRetryConfig,
    ) -> Self {
        let max_concurrent = max_concurrent.max(1);
        retry_config.max_attempts = retry_config.max_attempts.max(1);
        if retry_config.base_delay_ms == 0 {
            retry_config.base_delay_ms = DEFAULT_DLQ_RETRY_BASE_DELAY_MS;
        }
        if retry_config.max_delay_ms == 0 {
            retry_config.max_delay_ms = DEFAULT_DLQ_RETRY_MAX_DELAY_MS;
        }
        if retry_config.max_delay_ms < retry_config.base_delay_ms {
            retry_config.max_delay_ms = retry_config.base_delay_ms;
        }

        Self {
            producer,
            topic: topic.into(),
            timeout,
            send_semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
            retry_config,
        }
    }

    /// Wait for all in-flight fire-and-forget sends to complete (best-effort).
    ///
    /// Returns `true` if all permits were acquired (drained) within the timeout.
    pub async fn drain_in_flight(&self, timeout: Duration) -> bool {
        match tokio::time::timeout(
            timeout,
            self.send_semaphore.acquire_many(self.max_concurrent as u32),
        )
        .await
        {
            Ok(Ok(permit)) => {
                drop(permit);
                true
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "DLQ drain failed");
                false
            }
            Err(_) => false,
        }
    }

    /// Flush the underlying Kafka producer (best-effort).
    pub async fn flush(&self, timeout: Duration) -> Result<()> {
        let producer = self.producer.clone();
        tokio::task::spawn_blocking(move || producer.flush(Timeout::After(timeout)))
            .await
            .map_err(|e| {
                crate::errors::Error::Io(std::io::Error::other(format!(
                    "Failed to join DLQ flush task: {}",
                    e
                )))
            })?
            .map_err(|e| {
                crate::errors::Error::Io(std::io::Error::other(format!(
                    "Failed to flush DLQ producer: {}",
                    e
                )))
            })
    }

    /// Drain in-flight fire-and-forget sends then flush the producer.
    pub async fn drain_and_flush(&self, drain_timeout: Duration, flush_timeout: Duration) {
        if !self.drain_in_flight(drain_timeout).await {
            tracing::warn!(
                timeout_ms = drain_timeout.as_millis(),
                "Timed out waiting for in-flight DLQ sends"
            );
        }
        if let Err(e) = self.flush(flush_timeout).await {
            tracing::warn!(error = %e, "DLQ producer flush failed");
        }
    }

    /// Sends a failed message to the DLQ
    ///
    /// Returns Ok(()) if successfully sent to DLQ, or Err if DLQ itself fails.
    /// Failures to send to DLQ should be tracked with separate metrics.
    ///
    /// # Arguments
    ///
    /// * `message` - DlqMessage with full error context
    pub async fn send(&self, message: &DlqMessage) -> Result<()> {
        let payload = message.to_json()?;

        let record = FutureRecord::to(&self.topic)
            .payload(&payload)
            .key(&message.trace_id);

        match self
            .producer
            .send(record, Timeout::After(self.timeout))
            .await
        {
            Ok(_) => {
                DLQ_SENDS_TOTAL.inc();
                Ok(())
            }
            Err((e, _)) => {
                DLQ_SEND_FAILURES_TOTAL.inc();
                Err(crate::errors::Error::InvalidFormat(format!(
                    "Failed to send to DLQ: {}",
                    e
                )))
            }
        }
    }

    /// Sends a failed message to the DLQ with automatic retry on failure
    ///
    /// Uses exponential backoff with jitter for transient failures.
    /// This is recommended for critical DLQ messages where loss is unacceptable.
    ///
    /// # Arguments
    ///
    /// * `message` - DlqMessage with full error context
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if send succeeded (possibly after retries), or `Err` if
    /// all retry attempts were exhausted.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let msg = DlqMessage::new(...)
    ///     .with_node_id("processor-node")
    ///     .with_current_trace_context();
    ///
    /// // Will retry up to 3 times with exponential backoff
    /// dlq_handler.send_with_retry(&msg).await?;
    /// ```
    pub async fn send_with_retry(&self, message: &DlqMessage) -> Result<()> {
        let payload = message.to_json()?;
        let mut last_error = None;
        let max_attempts = self.retry_config.max_attempts.max(1);

        for attempt in 0..max_attempts {
            let record = FutureRecord::to(&self.topic)
                .payload(&payload)
                .key(&message.trace_id);

            match self
                .producer
                .send(record, Timeout::After(self.timeout))
                .await
            {
                Ok(_) => {
                    DLQ_SENDS_TOTAL.inc();
                    if attempt > 0 {
                        tracing::info!(
                            attempt = attempt + 1,
                            trace_id = %message.trace_id,
                            "DLQ send succeeded after retry"
                        );
                    }
                    return Ok(());
                }
                Err((e, _)) => {
                    DLQ_SEND_RETRIES_TOTAL.inc();

                    // Don't sleep after the last attempt
                    if attempt + 1 < max_attempts {
                        // Calculate delay with exponential backoff and jitter
                        let exp = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
                        let base_delay = self.retry_config.base_delay_ms.saturating_mul(exp);
                        let delay = std::cmp::min(base_delay, self.retry_config.max_delay_ms);
                        // Add jitter (0-25% of delay)
                        let jitter = (delay as f64 * 0.25 * rand::random::<f64>()) as u64;
                        let total_delay = delay + jitter;

                        tracing::warn!(
                            attempt = attempt + 1,
                            max_attempts = max_attempts,
                            delay_ms = total_delay,
                            trace_id = %message.trace_id,
                            error = %e,
                            "DLQ send failed, retrying"
                        );

                        tokio::time::sleep(Duration::from_millis(total_delay)).await;
                    }
                    last_error = Some(e);
                }
            }
        }

        // All retries exhausted
        DLQ_SEND_FAILURES_TOTAL.inc();
        let err = last_error.unwrap_or_else(|| {
            rdkafka::error::KafkaError::MessageProduction(rdkafka::error::RDKafkaErrorCode::Unknown)
        });
        tracing::error!(
            attempts = max_attempts,
            trace_id = %message.trace_id,
            error = %err,
            "DLQ send failed after all retries"
        );

        Err(crate::errors::Error::InvalidFormat(format!(
            "Failed to send to DLQ after {} attempts: {}",
            max_attempts, err
        )))
    }

    /// Sends a failed message to the DLQ without waiting for confirmation
    ///
    /// Spawns an async task to send the message. Use this in hot paths
    /// where you don't want to block on DLQ confirmation.
    ///
    /// # Backpressure
    ///
    /// This method uses a semaphore to limit concurrent sends. When the limit
    /// is reached (default: 100), the message is dropped to avoid unbounded task
    /// spawning under high load.
    ///
    /// Note: This provides no feedback on whether the DLQ send succeeded.
    /// Use `send()` or `send_fire_and_forget_with_retry()` for better reliability.
    pub fn send_fire_and_forget(&self, message: DlqMessage) {
        let permit = match Arc::clone(&self.send_semaphore).try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                // M-515: Upgraded to error since dropping forensic messages is data loss.
                // Use send_fire_and_forget_blocking() if message loss is unacceptable.
                tracing::error!(
                    trace_id = %message.trace_id,
                    "DLQ backpressure limit reached - DROPPING forensic message (use blocking variant if loss unacceptable)"
                );
                DLQ_DROPPED_TOTAL.inc();
                return;
            }
        };

        let producer = self.producer.clone();
        let topic = self.topic.clone();
        let timeout = self.timeout;

        tokio::spawn(async move {
            let _permit = permit; // Keep permit alive until send completes

            let payload = match message.to_json() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize DLQ message");
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    return;
                }
            };

            let record = FutureRecord::to(&topic)
                .payload(&payload)
                .key(&message.trace_id);

            match producer.send(record, Timeout::After(timeout)).await {
                Ok(_) => {
                    DLQ_SENDS_TOTAL.inc();
                }
                Err((e, _)) => {
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    tracing::warn!(error = %e, "Failed to send to DLQ");
                }
            }
            // Permit is dropped here, releasing the semaphore slot
        });
    }

    /// Sends a failed message to the DLQ without waiting, with automatic retry
    ///
    /// Spawns an async task that will retry with exponential backoff on failure.
    /// This is recommended for critical DLQ messages where loss is unacceptable
    /// but you don't want to block the calling code.
    ///
    /// # Backpressure
    ///
    /// This method uses a semaphore to limit concurrent sends. When the limit
    /// is reached (default: 100), the message is dropped to avoid unbounded task
    /// spawning under high load.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let msg = DlqMessage::new(...)
    ///     .with_node_id("processor-node")
    ///     .with_current_trace_context();
    ///
    /// // Will retry in background with exponential backoff
    /// dlq_handler.send_fire_and_forget_with_retry(msg);
    /// ```
    pub fn send_fire_and_forget_with_retry(&self, message: DlqMessage) {
        let permit = match Arc::clone(&self.send_semaphore).try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                // M-515: Upgraded to error since dropping forensic messages is data loss.
                // Use send_fire_and_forget_with_retry_blocking() if message loss is unacceptable.
                tracing::error!(
                    trace_id = %message.trace_id,
                    "DLQ backpressure limit reached - DROPPING forensic message with retry (use blocking variant if loss unacceptable)"
                );
                DLQ_DROPPED_TOTAL.inc();
                return;
            }
        };

        let producer = self.producer.clone();
        let topic = self.topic.clone();
        let timeout = self.timeout;
        let retry_config = self.retry_config.clone();

        tokio::spawn(async move {
            let _permit = permit; // Keep permit alive until send completes

            let payload = match message.to_json() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize DLQ message");
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    return;
                }
            };

            let mut last_error = None;
            let max_attempts = retry_config.max_attempts.max(1);
            for attempt in 0..max_attempts {
                let record = FutureRecord::to(&topic)
                    .payload(&payload)
                    .key(&message.trace_id);

                match producer.send(record, Timeout::After(timeout)).await {
                    Ok(_) => {
                        DLQ_SENDS_TOTAL.inc();
                        if attempt > 0 {
                            tracing::info!(
                                attempt = attempt + 1,
                                trace_id = %message.trace_id,
                                "DLQ fire-and-forget send succeeded after retry"
                            );
                        }
                        return;
                    }
                    Err((e, _)) => {
                        DLQ_SEND_RETRIES_TOTAL.inc();

                        // Don't sleep after the last attempt
                        if attempt + 1 < max_attempts {
                            // Calculate delay with exponential backoff and jitter
                            let exp = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
                            let base_delay = retry_config.base_delay_ms.saturating_mul(exp);
                            let delay = std::cmp::min(base_delay, retry_config.max_delay_ms);
                            // Add jitter (0-25% of delay)
                            let jitter = (delay as f64 * 0.25 * rand::random::<f64>()) as u64;
                            let total_delay = delay + jitter;

                            tracing::warn!(
                                attempt = attempt + 1,
                                max_attempts = max_attempts,
                                delay_ms = total_delay,
                                trace_id = %message.trace_id,
                                error = %e,
                                "DLQ fire-and-forget send failed, retrying"
                            );

                            tokio::time::sleep(Duration::from_millis(total_delay)).await;
                        }
                        last_error = Some(e);
                    }
                }
            }

            // All retries exhausted
            DLQ_SEND_FAILURES_TOTAL.inc();
            tracing::error!(
                attempts = max_attempts,
                trace_id = %message.trace_id,
                error = ?last_error,
                "DLQ fire-and-forget send failed after all retries"
            );
            // Permit is dropped here, releasing the semaphore slot
        });
    }

    /// Try to send a message to DLQ without blocking
    ///
    /// Returns `false` if backpressure limit is reached (no permit available).
    /// Use this when you need non-blocking behavior and can handle dropped messages.
    pub fn try_send_fire_and_forget(&self, message: DlqMessage) -> bool {
        let permit = match Arc::clone(&self.send_semaphore).try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                // At capacity - would need to block
                DLQ_DROPPED_TOTAL.inc();
                return false;
            }
        };

        let producer = self.producer.clone();
        let topic = self.topic.clone();
        let timeout = self.timeout;

        tokio::spawn(async move {
            let _permit = permit; // Keep permit alive until send completes

            let payload = match message.to_json() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to serialize DLQ message");
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    return;
                }
            };

            let record = FutureRecord::to(&topic)
                .payload(&payload)
                .key(&message.trace_id);

            match producer.send(record, Timeout::After(timeout)).await {
                Ok(_) => {
                    DLQ_SENDS_TOTAL.inc();
                }
                Err((e, _)) => {
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    tracing::warn!(error = %e, "Failed to send to DLQ");
                }
            }
        });

        true
    }

    /// M-515: Send a message to DLQ, waiting for a permit if backpressure limit is reached.
    ///
    /// Unlike `send_fire_and_forget`, this method will block until a semaphore permit
    /// is available, ensuring the message is never dropped due to backpressure.
    ///
    /// Use this for forensic messages that must not be lost.
    ///
    /// # Warning
    ///
    /// This can cause cascading backpressure if the DLQ producer is slow.
    /// Only use when message preservation is critical.
    pub async fn send_fire_and_forget_blocking(&self, message: DlqMessage) {
        // Block until a permit is available (never drops)
        let permit = Arc::clone(&self.send_semaphore).acquire_owned().await;
        let permit = match permit {
            Ok(p) => p,
            Err(_) => {
                // Semaphore closed - this should never happen in normal operation
                tracing::error!("DLQ semaphore closed unexpectedly");
                return;
            }
        };

        let producer = self.producer.clone();
        let topic = self.topic.clone();
        let timeout = self.timeout;

        tokio::spawn(async move {
            let _permit = permit; // Keep permit alive until send completes

            let payload = match message.to_json() {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to serialize forensic DLQ message");
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    return;
                }
            };

            let record = FutureRecord::to(&topic)
                .payload(&payload)
                .key(&message.trace_id);

            match producer.send(record, Timeout::After(timeout)).await {
                Ok(_) => {
                    DLQ_SENDS_TOTAL.inc();
                }
                Err((e, _)) => {
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    tracing::error!(error = %e, "Failed to send forensic DLQ message");
                }
            }
        });
    }

    /// M-515: Send a message to DLQ with retry, waiting for a permit if needed.
    ///
    /// Unlike `send_fire_and_forget_with_retry`, this method will block until a
    /// semaphore permit is available, ensuring the message is never dropped.
    ///
    /// Use this for forensic messages that must not be lost.
    pub async fn send_fire_and_forget_with_retry_blocking(&self, message: DlqMessage) {
        // Block until a permit is available (never drops)
        let permit = Arc::clone(&self.send_semaphore).acquire_owned().await;
        let permit = match permit {
            Ok(p) => p,
            Err(_) => {
                tracing::error!("DLQ semaphore closed unexpectedly");
                return;
            }
        };

        let producer = self.producer.clone();
        let topic = self.topic.clone();
        let timeout = self.timeout;
        let retry_config = self.retry_config.clone();

        tokio::spawn(async move {
            let _permit = permit;

            let payload = match message.to_json() {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to serialize forensic DLQ message");
                    DLQ_SEND_FAILURES_TOTAL.inc();
                    return;
                }
            };

            let mut last_error = None;
            let max_attempts = retry_config.max_attempts.max(1);
            for attempt in 0..max_attempts {
                let record = FutureRecord::to(&topic)
                    .payload(&payload)
                    .key(&message.trace_id);

                match producer.send(record, Timeout::After(timeout)).await {
                    Ok(_) => {
                        DLQ_SENDS_TOTAL.inc();
                        if attempt > 0 {
                            tracing::info!(
                                attempt = attempt + 1,
                                trace_id = %message.trace_id,
                                "Forensic DLQ send succeeded after retry"
                            );
                        }
                        return;
                    }
                    Err((e, _)) => {
                        DLQ_SEND_RETRIES_TOTAL.inc();

                        // Don't sleep after the last attempt
                        if attempt + 1 < max_attempts {
                            // M-1051: Use checked_shl + jitter for consistency with other retry methods
                            let exp = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
                            let base_delay = retry_config.base_delay_ms.saturating_mul(exp);
                            let delay = std::cmp::min(base_delay, retry_config.max_delay_ms);
                            // Add jitter (0-25% of delay) to prevent thundering herd
                            let jitter = (delay as f64 * 0.25 * rand::random::<f64>()) as u64;
                            let total_delay = delay + jitter;

                            tracing::warn!(
                                attempt = attempt + 1,
                                max_attempts = max_attempts,
                                delay_ms = total_delay,
                                trace_id = %message.trace_id,
                                error = %e,
                                "Forensic DLQ send failed, retrying"
                            );

                            tokio::time::sleep(Duration::from_millis(total_delay)).await;
                        }
                        last_error = Some(e.to_string());
                    }
                }
            }

            DLQ_SEND_FAILURES_TOTAL.inc();
            tracing::error!(
                attempts = max_attempts,
                trace_id = %message.trace_id,
                error = ?last_error,
                "Forensic DLQ message send failed after all retries"
            );
        });
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlq_message_creation() {
        let payload = b"test payload";
        let msg = DlqMessage::new(
            payload,
            "decode failed",
            "dashstream-events",
            0,
            12345,
            "websocket-server-1",
            "decode_error",
        );

        assert_eq!(msg.error, "decode failed");
        assert_eq!(msg.source_topic, "dashstream-events");
        assert_eq!(msg.source_partition, 0);
        assert_eq!(msg.source_offset, 12345);
        assert_eq!(msg.consumer_id, "websocket-server-1");
        assert_eq!(msg.error_type, "decode_error");
        assert!(msg.thread_id.is_none());
        assert!(msg.tenant_id.is_none());
        assert!(!msg.trace_id.is_empty());
    }

    #[test]
    fn test_dlq_message_with_optional_fields() {
        let payload = b"test payload";
        let msg = DlqMessage::new(
            payload,
            "decode failed",
            "dashstream-events",
            0,
            12345,
            "websocket-server-1",
            "decode_error",
        )
        .with_thread_id("session-123")
        .with_tenant_id("tenant-456")
        .with_trace_id("custom-trace-id");

        assert_eq!(msg.thread_id, Some("session-123".to_string()));
        assert_eq!(msg.tenant_id, Some("tenant-456".to_string()));
        assert_eq!(msg.trace_id, "custom-trace-id");
    }

    #[test]
    fn test_dlq_message_base64_encoding() {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::engine::Engine;

        let payload = b"hello world";
        let msg = DlqMessage::new(
            payload,
            "test error",
            "dashstream-events",
            0,
            100,
            "test-consumer",
            "test_error",
        );

        // Verify base64 encoding
        let expected_base64 = BASE64.encode(payload);
        assert_eq!(msg.original_payload_base64, expected_base64);

        // Verify we can decode it back
        let decoded = BASE64.decode(&msg.original_payload_base64).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_dlq_message_to_json() {
        let payload = b"test";
        let msg = DlqMessage::new(payload, "error", "topic", 0, 100, "consumer", "error_type");

        let json = msg.to_json().expect("Should serialize to JSON");

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse as JSON");

        assert_eq!(parsed["error"], "error");
        assert_eq!(parsed["source_topic"], "topic");
        assert_eq!(parsed["source_partition"], 0);
        assert_eq!(parsed["source_offset"], 100);
        assert_eq!(parsed["consumer_id"], "consumer");
        assert_eq!(parsed["error_type"], "error_type");

        // Optional fields should be omitted if None
        assert!(parsed.get("thread_id").is_none());
        assert!(parsed.get("tenant_id").is_none());
    }

    #[test]
    fn test_dlq_message_json_with_optional_fields() {
        let payload = b"test";
        let msg = DlqMessage::new(payload, "error", "topic", 0, 100, "consumer", "error_type")
            .with_thread_id("thread-1")
            .with_tenant_id("tenant-1");

        let json = msg.to_json().expect("Should serialize to JSON");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse as JSON");

        // Optional fields should be included if Some
        assert_eq!(parsed["thread_id"], "thread-1");
        assert_eq!(parsed["tenant_id"], "tenant-1");
    }

    #[test]
    fn test_dlq_message_timestamp_format() {
        let payload = b"test";
        let msg = DlqMessage::new(payload, "error", "topic", 0, 100, "consumer", "error_type");

        // Verify timestamp is ISO 8601 / RFC3339 format
        assert!(
            chrono::DateTime::parse_from_rfc3339(&msg.timestamp).is_ok(),
            "Timestamp should be valid RFC3339: {}",
            msg.timestamp
        );
    }

    #[test]
    fn test_dlq_message_trace_id_is_uuid() {
        let payload = b"test";
        let msg = DlqMessage::new(payload, "error", "topic", 0, 100, "consumer", "error_type");

        // Verify trace_id is a valid UUID
        assert!(
            uuid::Uuid::parse_str(&msg.trace_id).is_ok(),
            "Trace ID should be valid UUID: {}",
            msg.trace_id
        );
    }

    #[test]
    fn test_dlq_message_preserves_binary_payload() {
        // Test with binary data (not UTF-8)
        let payload = vec![0xFF, 0xFE, 0xFD, 0x00, 0x01, 0x02];
        let msg = DlqMessage::new(&payload, "error", "topic", 0, 100, "consumer", "error_type");

        // Decode and verify
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::engine::Engine;
        let decoded = BASE64.decode(&msg.original_payload_base64).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_dlq_message_truncates_large_payloads() {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::engine::Engine;

        const MAX_ORIGINAL_PAYLOAD_BYTES: usize = 512 * 1024;
        let payload = vec![0xAB; MAX_ORIGINAL_PAYLOAD_BYTES + 10];
        let msg = DlqMessage::new(&payload, "error", "topic", 0, 100, "consumer", "error_type");

        let decoded = BASE64.decode(&msg.original_payload_base64).unwrap();
        assert_eq!(decoded.len(), MAX_ORIGINAL_PAYLOAD_BYTES);
        assert_eq!(&decoded[..], &payload[..MAX_ORIGINAL_PAYLOAD_BYTES]);
        assert_eq!(msg.original_payload_truncated, Some(true));
        assert_eq!(msg.original_payload_size_bytes, Some(payload.len() as u64));
        assert_eq!(
            msg.original_payload_included_bytes,
            Some(MAX_ORIGINAL_PAYLOAD_BYTES as u64)
        );
        assert!(
            msg.original_payload_sha256
                .as_ref()
                .is_some_and(|h| h.len() == 64),
            "Expected SHA256 hex digest"
        );
    }

    // S-15: Tests for SHA256 validation on replay
    #[test]
    fn test_dlq_validate_sha256_non_truncated() {
        let payload = b"small payload";
        let msg = DlqMessage::new(payload, "error", "topic", 0, 100, "consumer", "error_type");

        // Non-truncated messages should always return Ok(true)
        assert!(!msg.is_truncated());
        assert!(msg.validate_payload_sha256(payload).unwrap());
        assert!(msg.validate_payload_sha256(b"different payload").unwrap()); // No validation for non-truncated
    }

    #[test]
    fn test_dlq_validate_sha256_truncated_valid() {
        const MAX_ORIGINAL_PAYLOAD_BYTES: usize = 512 * 1024;
        let payload = vec![0xAB; MAX_ORIGINAL_PAYLOAD_BYTES + 100];
        let msg = DlqMessage::new(&payload, "error", "topic", 0, 100, "consumer", "error_type");

        // Should be truncated
        assert!(msg.is_truncated());
        assert_eq!(msg.original_size(), Some(payload.len() as u64));

        // Validate with correct full payload should succeed
        assert!(msg.validate_payload_sha256(&payload).unwrap());
    }

    #[test]
    fn test_dlq_validate_sha256_truncated_invalid() {
        const MAX_ORIGINAL_PAYLOAD_BYTES: usize = 512 * 1024;
        let payload = vec![0xAB; MAX_ORIGINAL_PAYLOAD_BYTES + 100];
        let msg = DlqMessage::new(&payload, "error", "topic", 0, 100, "consumer", "error_type");

        // Validate with wrong payload should fail
        let wrong_payload = vec![0xCD; MAX_ORIGINAL_PAYLOAD_BYTES + 100];
        assert!(!msg.validate_payload_sha256(&wrong_payload).unwrap());
    }

    #[test]
    fn test_dlq_validate_sha256_missing_hash() {
        let payload = b"small payload";
        let mut msg = DlqMessage::new(payload, "error", "topic", 0, 100, "consumer", "error_type");

        // Artificially mark as truncated without setting SHA256 (inconsistent state)
        msg.original_payload_truncated = Some(true);

        // Should return an error due to missing SHA256
        assert!(msg.validate_payload_sha256(payload).is_err());
    }

    #[test]
    fn test_dlq_message_with_node_and_trace_context() {
        let payload = b"test payload";
        let msg = DlqMessage::new(
            payload,
            "processing error",
            "dashstream-events",
            0,
            12345,
            "processor-1",
            "processing_error",
        )
        .with_node_id("transform-node")
        .with_parent_trace_id("abc123def456")
        .with_span_id("span789");

        assert_eq!(msg.node_id, Some("transform-node".to_string()));
        assert_eq!(msg.parent_trace_id, Some("abc123def456".to_string()));
        assert_eq!(msg.span_id, Some("span789".to_string()));

        // Verify JSON serialization includes the new fields
        let json = msg.to_json().expect("Should serialize to JSON");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse as JSON");

        assert_eq!(parsed["node_id"], "transform-node");
        assert_eq!(parsed["parent_trace_id"], "abc123def456");
        assert_eq!(parsed["span_id"], "span789");
    }

    #[test]
    fn test_dlq_message_optional_fields_omitted_in_json() {
        let payload = b"test";
        let msg = DlqMessage::new(payload, "error", "topic", 0, 100, "consumer", "error_type");

        let json = msg.to_json().expect("Should serialize to JSON");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse as JSON");

        // These optional fields should NOT be present in JSON when None
        assert!(parsed.get("node_id").is_none());
        assert!(parsed.get("parent_trace_id").is_none());
        assert!(parsed.get("span_id").is_none());
    }

    #[test]
    fn test_dlq_retry_config_default() {
        let config = DlqRetryConfig::default();
        assert_eq!(config.max_attempts, DEFAULT_DLQ_RETRY_ATTEMPTS);
        assert_eq!(config.base_delay_ms, DEFAULT_DLQ_RETRY_BASE_DELAY_MS);
        assert_eq!(config.max_delay_ms, DEFAULT_DLQ_RETRY_MAX_DELAY_MS);
    }

    #[test]
    fn test_dlq_retry_config_custom() {
        let config = DlqRetryConfig {
            max_attempts: 5,
            base_delay_ms: 200,
            max_delay_ms: 10000,
        };
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay_ms, 200);
        assert_eq!(config.max_delay_ms, 10000);
    }
}
