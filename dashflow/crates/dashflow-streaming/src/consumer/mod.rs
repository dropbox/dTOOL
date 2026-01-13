// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Kafka Consumer for DashFlow Streaming Messages
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

//! # DashFlow Streaming Consumer
//!
//! High-performance Kafka consumer for streaming DashFlow Streaming telemetry messages.
//!
//! ## Features
//!
//! - **Async Stream**: Tokio-based async message consumption
//! - **Auto Decompression**: Transparent decompression of compressed messages
//! - **Offset Tracking**: In-memory per-partition offset tracking (no consumer-group commits)
//! - **Error Handling**: Comprehensive error types with retry support
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_streaming::consumer::DashStreamConsumer;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create consumer
//!     let mut consumer = DashStreamConsumer::new(
//!         "localhost:9092",
//!         "dashstream-events",
//!         "my-consumer-group"
//!     ).await?;
//!
//!     // Consume messages
//!     loop {
//!         match consumer
//!             .next_timeout(std::time::Duration::from_secs(30))
//!             .await
//!         {
//!             Some(Ok(msg)) => println!("Received message: {:?}", msg),
//!             Some(Err(e)) => eprintln!("Error: {}", e),
//!             None => {
//!                 // Timeout: keep waiting (break/return here if desired)
//!                 continue;
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use crate::codec::{
    decode_message, decode_message_compatible, decode_message_strict, validate_schema_version,
    SchemaCompatibility, HEADER_COMPRESSED_ZSTD, HEADER_UNCOMPRESSED,
};
use crate::dlq::{DlqHandler, DlqMessage};
use crate::errors::{Error, Result};
use crate::DashStreamMessage;
use std::sync::LazyLock;
use prometheus::{Counter, Histogram, HistogramOpts};
use rdkafka::config::ClientConfig as RdKafkaClientConfig;
use rdkafka::producer::FutureProducer;
use rskafka::client::error::{Error as RsKafkaError, ProtocolError};
use rskafka::client::partition::{OffsetAt, PartitionClient, UnknownTopicHandling};
use rskafka::client::{ClientBuilder, SaslConfig};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{error, info, warn};

// Re-import shared Kafka constants from crate root
use crate::{DEFAULT_DLQ_TIMEOUT_SECS, DEFAULT_DLQ_TOPIC};

// ============================================================================
// Consumer Configuration Constants (M-243: Replace magic numbers)
// ============================================================================
//
// These values are Kafka best practices for streaming telemetry workloads.
// Override via ConsumerConfig fields if your deployment has different requirements.

/// Default auto-commit interval in milliseconds.
/// 5 seconds (5000ms) balances durability with commit overhead.
/// Lower values = more commits, better durability on crash, higher broker load.
/// Higher values = fewer commits, more messages reprocessed on crash.
pub const DEFAULT_AUTO_COMMIT_INTERVAL_MS: u64 = 5000;

/// Default session timeout in milliseconds.
/// 30 seconds (30000ms) is Kafka's default for consumer group coordination.
/// Must be between `group.min.session.timeout.ms` and `group.max.session.timeout.ms` on broker.
/// Lower values = faster failure detection, but risk false positives during GC pauses.
/// Higher values = slower failure detection, but more tolerant of transient issues.
pub const DEFAULT_SESSION_TIMEOUT_MS: u64 = 30000;

/// Default maximum message size in bytes (1 MB).
/// Matches Kafka broker default `message.max.bytes`.
/// Larger messages should use chunking or external storage references.
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Default initial backoff for fetch retries in milliseconds.
/// 100ms provides fast initial retry without overwhelming the broker.
pub const DEFAULT_FETCH_BACKOFF_INITIAL_MS: u64 = 100;

/// Default maximum backoff for fetch retries in seconds.
/// 5 seconds caps exponential backoff to ensure reasonable recovery time.
pub const DEFAULT_FETCH_BACKOFF_MAX_SECS: u64 = 5;

/// Default sleep duration when broker returns no records in milliseconds.
/// 50ms prevents busy-loop while maintaining low latency for new messages.
pub const DEFAULT_IDLE_POLL_SLEEP_MS: u64 = 50;

// ============================================================================
// Sequence Validation
// ============================================================================

/// Policy for handling sequence gaps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GapRecoveryPolicy {
    /// Continue after gap (current behavior, allows data loss)
    Continue,
    /// Halt consumption for this thread (requires manual reset)
    Halt,
    /// Log critical warning and continue (recommended for production)
    #[default]
    WarnAndContinue,
}

/// Errors detected during sequence validation
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SequenceError {
    /// Gap in sequence numbers (message loss)
    Gap {
        /// Thread ID that the sequence belongs to.
        thread_id: String,
        /// Next expected sequence number for the thread.
        expected: u64,
        /// Sequence number received from the stream.
        received: u64,
        /// Number of missing sequence values (`received - expected`).
        gap_size: u64,
    },
    /// Duplicate sequence number
    Duplicate {
        /// Thread ID that the sequence belongs to.
        thread_id: String,
        /// Duplicate sequence number received from the stream.
        sequence: u64,
        /// Next expected sequence number for the thread.
        expected: u64,
    },
    /// Out-of-order sequence number
    Reordered {
        /// Thread ID that the sequence belongs to.
        thread_id: String,
        /// Out-of-order sequence number received from the stream.
        sequence: u64,
        /// Next expected sequence number for the thread.
        expected: u64,
    },
}

impl std::fmt::Display for SequenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SequenceError::Gap {
                thread_id,
                expected,
                received,
                gap_size,
            } => write!(
                f,
                "Sequence gap for thread {}: expected {}, received {} (gap size: {})",
                thread_id, expected, received, gap_size
            ),
            SequenceError::Duplicate {
                thread_id,
                sequence,
                expected,
            } => write!(
                f,
                "Duplicate sequence for thread {}: received {}, expected {}",
                thread_id, sequence, expected
            ),
            SequenceError::Reordered {
                thread_id,
                sequence,
                expected,
            } => write!(
                f,
                "Out-of-order sequence for thread {}: received {}, expected {}",
                thread_id, sequence, expected
            ),
        }
    }
}

impl std::error::Error for SequenceError {}

/// Validates sequence numbers for DashFlow Streaming messages to detect:
/// - Gaps (message loss)
/// - Duplicates (redelivery)
/// - Reordering (out-of-order delivery)
pub struct SequenceValidator {
    /// Map of thread_id -> next expected sequence number
    expected_next: HashMap<String, u64>,
    /// Gap recovery policy
    policy: GapRecoveryPolicy,
    /// Threads that have been halted due to gaps
    halted_threads: std::collections::HashSet<String>,
    /// M-514: Threads that were pruned due to memory pressure.
    /// When a pruned thread sends a new message, we accept it with a warning
    /// instead of falsely detecting a gap from the reset tracking state.
    pruned_threads: std::collections::HashSet<String>,
}

impl SequenceValidator {
    /// Maximum number of threads to track before pruning (prevents unbounded growth).
    const MAX_TRACKED_THREADS: usize = 100_000;
    /// Number of entries to prune when over capacity.
    const PRUNE_BATCH: usize = 1000;

    /// Create a new sequence validator with default policy
    pub fn new() -> Self {
        Self::with_policy(GapRecoveryPolicy::default())
    }

    /// Create a new sequence validator with specific policy
    pub fn with_policy(policy: GapRecoveryPolicy) -> Self {
        Self {
            expected_next: HashMap::new(),
            policy,
            halted_threads: std::collections::HashSet::new(),
            pruned_threads: std::collections::HashSet::new(),
        }
    }

    /// Validate a sequence number for a given thread
    ///
    /// Returns:
    /// - `Ok(())` if sequence is exactly what we expected
    /// - `Err(SequenceError)` if gap, duplicate, or reordering detected
    pub fn validate(
        &mut self,
        thread_id: &str,
        sequence: u64,
    ) -> std::result::Result<(), SequenceError> {
        // Prevent unbounded growth from high-cardinality thread IDs.
        if self.expected_next.len() > Self::MAX_TRACKED_THREADS {
            self.prune_state(thread_id);
        }

        // M-514: Check if this thread was previously pruned.
        // If so, we can't validate because we lost tracking state.
        // Accept the message and restart tracking from here to avoid false gap detection.
        if self.pruned_threads.remove(thread_id) {
            warn!(
                thread_id = %thread_id,
                sequence = sequence,
                "Pruned thread reappeared - accepting message and resetting tracking (validation skipped)"
            );
            self.expected_next.insert(thread_id.to_string(), sequence + 1);
            return Ok(());
        }

        // M-1114: Treat first-seen sequence as baseline instead of defaulting to 1.
        // After server restart, threads will resume midstream (seq >> 1), and treating
        // seq=1 as expected would report false gaps for every active thread.
        // On first message for a thread, set expected = sequence + 1 (no gap).
        let is_first_seen = !self.expected_next.contains_key(thread_id);
        let expected = self.expected_next.entry(thread_id.to_string()).or_insert(sequence);
        if is_first_seen {
            // First message for this thread - accept as baseline, no gap
            tracing::debug!(
                thread_id = %thread_id,
                baseline_sequence = sequence,
                "New thread initialized - sequence tracking started"
            );
            *expected = sequence + 1;
            return Ok(());
        }

        if sequence < *expected {
            // Duplicate or reordered message
            if sequence == *expected - 1 {
                return Err(SequenceError::Duplicate {
                    thread_id: thread_id.to_string(),
                    sequence,
                    expected: *expected,
                });
            } else {
                return Err(SequenceError::Reordered {
                    thread_id: thread_id.to_string(),
                    sequence,
                    expected: *expected,
                });
            }
        }

        if sequence > *expected {
            // Gap detected - missing messages
            let gap_size = sequence - *expected;
            let error = SequenceError::Gap {
                thread_id: thread_id.to_string(),
                expected: *expected,
                received: sequence,
                gap_size,
            };

            // Apply recovery policy
            match self.policy {
                GapRecoveryPolicy::Continue => {
                    // Jump forward, accept data loss
                    *expected = sequence + 1;
                }
                GapRecoveryPolicy::Halt => {
                    // Halt this thread - requires manual intervention
                    self.halted_threads.insert(thread_id.to_string());
                    error!(
                        thread_id = %thread_id,
                        expected = expected,
                        received = sequence,
                        messages_lost = gap_size,
                        "Thread halted due to sequence gap - manual reset required"
                    );
                    // Don't update expected - stay stuck at gap
                }
                GapRecoveryPolicy::WarnAndContinue => {
                    // Critical warning but continue
                    warn!(
                        thread_id = %thread_id,
                        expected = expected,
                        received = sequence,
                        messages_lost = gap_size,
                        "Critical sequence gap detected - continuing with data loss"
                    );
                    *expected = sequence + 1;
                }
            }

            return Err(error);
        }

        // Sequence is exactly what we expected
        *expected = sequence + 1;
        Ok(())
    }

    /// Reset the expected sequence for a thread (e.g., after restart)
    pub fn reset(&mut self, thread_id: &str) {
        self.expected_next.remove(thread_id);
    }

    /// Check if a thread is halted due to gap
    pub fn is_halted(&self, thread_id: &str) -> bool {
        self.halted_threads.contains(thread_id)
    }

    /// Manually reset a halted thread (for operators)
    pub fn reset_halted(&mut self, thread_id: &str) {
        self.halted_threads.remove(thread_id);
        self.expected_next.remove(thread_id);
        info!(thread_id = %thread_id, "Reset halted thread");
    }

    /// Get all halted threads
    pub fn get_halted_threads(&self) -> Vec<String> {
        self.halted_threads.iter().cloned().collect()
    }

    /// Get the expected sequence number for a thread
    pub fn expected_for_thread(&self, thread_id: &str) -> Option<u64> {
        self.expected_next.get(thread_id).copied()
    }

    /// Clear all tracked sequences
    pub fn clear(&mut self) {
        self.expected_next.clear();
        self.halted_threads.clear();
        self.pruned_threads.clear();
    }

    /// M-514: Maximum pruned threads to track (10% of MAX_TRACKED_THREADS).
    /// Beyond this, we stop tracking pruned state (oldest entries forgotten).
    const MAX_PRUNED_THREADS: usize = 10_000;

    fn prune_state(&mut self, current_thread_id: &str) {
        let mut removed = 0usize;
        let keys: Vec<String> = self
            .expected_next
            .keys()
            .filter(|k| k.as_str() != current_thread_id)
            .take(Self::PRUNE_BATCH)
            .cloned()
            .collect();

        for key in keys {
            self.expected_next.remove(&key);
            self.halted_threads.remove(&key);
            // M-514: Track that this thread was pruned so we can handle its reappearance
            self.pruned_threads.insert(key);
            removed += 1;
        }

        // M-514: Cap pruned_threads to prevent unbounded growth
        // If we exceed the limit, forget oldest pruned threads (they'll get false gap detection,
        // but this is better than OOM). HashSet iteration order is arbitrary, which is acceptable.
        while self.pruned_threads.len() > Self::MAX_PRUNED_THREADS {
            let to_remove: Vec<String> = self
                .pruned_threads
                .iter()
                .take(Self::PRUNE_BATCH)
                .cloned()
                .collect();
            if to_remove.is_empty() {
                break; // Safety: should never happen, but prevents infinite loop
            }
            for key in to_remove {
                self.pruned_threads.remove(&key);
            }
        }

        if removed > 0 {
            warn!(
                removed = removed,
                remaining = self.expected_next.len(),
                pruned_tracked = self.pruned_threads.len(),
                max_tracked = Self::MAX_TRACKED_THREADS,
                "Pruned SequenceValidator state to cap memory"
            );
        }
    }
}

impl Default for SequenceValidator {
    fn default() -> Self {
        Self::new()
    }
}

// Prometheus metrics (M-624: Use centralized constants)
use crate::metrics_constants::{
    METRIC_DECODE_FAILURES_TOTAL, METRIC_FETCH_FAILURES_TOTAL, METRIC_INVALID_PAYLOADS_TOTAL,
    METRIC_MESSAGES_RECEIVED_TOTAL, METRIC_OFFSET_CHECKPOINT_FAILURES_TOTAL,
    METRIC_OFFSET_CHECKPOINT_WRITES_TOTAL, METRIC_SEQUENCE_DUPLICATES_TOTAL,
    METRIC_SEQUENCE_GAP_SIZE, METRIC_SEQUENCE_GAPS_TOTAL, METRIC_SEQUENCE_REORDERS_TOTAL,
};

static MESSAGES_RECEIVED_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_MESSAGES_RECEIVED_TOTAL,
        "Total number of messages successfully received from Kafka",
    )
});
static DECODE_FAILURES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_DECODE_FAILURES_TOTAL,
        "Total number of message decode failures",
    )
});
// M-98: Counter metrics include _total suffix for Prometheus naming convention
static FETCH_FAILURES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_FETCH_FAILURES_TOTAL,
        "Total number of Kafka fetch failures",
    )
});
static INVALID_PAYLOADS_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_INVALID_PAYLOADS_TOTAL,
        "Total number of invalid message payloads (missing/oversized)",
    )
});
static SEQUENCE_GAPS_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_SEQUENCE_GAPS_TOTAL,
        "Total sequence gaps detected (message loss)",
    )
});
static SEQUENCE_DUPLICATES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_SEQUENCE_DUPLICATES_TOTAL,
        "Total duplicate sequences detected",
    )
});
static SEQUENCE_REORDERS_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_SEQUENCE_REORDERS_TOTAL,
        "Total out-of-order sequences detected",
    )
});
static SEQUENCE_GAP_SIZE: LazyLock<Histogram> = LazyLock::new(|| {
    crate::metrics_utils::histogram(HistogramOpts::new(
        METRIC_SEQUENCE_GAP_SIZE,
        "Size of detected sequence gaps",
    ))
});
static OFFSET_CHECKPOINT_WRITES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_OFFSET_CHECKPOINT_WRITES_TOTAL,
        "Total number of successful local offset checkpoint writes",
    )
});
static OFFSET_CHECKPOINT_FAILURES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_OFFSET_CHECKPOINT_FAILURES_TOTAL,
        "Total number of local offset checkpoint write failures",
    )
});

/// Configuration for DashFlow Streaming consumer
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Kafka bootstrap servers (comma-separated)
    pub bootstrap_servers: String,

    /// Topic name for DashFlow Streaming messages
    pub topic: String,

    /// Kafka partition to consume from (default: 0).
    ///
    /// DashStreamConsumer is built on rskafka PartitionClient, so it consumes a single
    /// partition. For multi-partition topics, create one consumer per partition.
    pub partition: i32,

    /// Consumer group ID.
    ///
    /// # Deprecated
    /// rskafka does not support consumer groups; this field is unused.
    /// Retained for API parity and potential future group-consumer support.
    /// For multi-partition topics, create one `DashStreamConsumer` per partition instead.
    #[deprecated(
        since = "1.11.0",
        note = "rskafka does not support consumer groups; field is unused. For multi-partition topics, create one consumer per partition."
    )]
    pub group_id: String,

    /// Auto offset reset policy (earliest, latest)
    pub auto_offset_reset: String,

    /// Enable auto-commit.
    ///
    /// When `offset_checkpoint_path` is set, this enables local offset checkpointing
    /// (best-effort persistence of `current_offset` to disk) as a substitute for
    /// Kafka consumer-group commits (rskafka does not support them).
    ///
    /// # M-516: At-Least-Once Delivery Semantics
    ///
    /// **IMPORTANT:** Checkpoints are written AFTER messages are returned to the caller.
    /// This means messages may be re-delivered on restart if a crash occurs between
    /// returning a message and writing the checkpoint.
    ///
    /// Your message processing must be **idempotent** to handle duplicates safely.
    /// Consider using message IDs or sequence numbers for deduplication.
    pub enable_auto_commit: bool,

    /// Auto-commit interval (ms).
    ///
    /// Only applies when `offset_checkpoint_path` is set and `enable_auto_commit=true`.
    ///
    /// Lower values reduce duplicate risk on crash but increase disk I/O.
    pub auto_commit_interval_ms: u64,

    /// Session timeout (ms).
    ///
    /// # Deprecated
    /// Currently unused because rskafka does not support consumer-group sessions.
    /// This field has no effect on behavior.
    #[deprecated(
        since = "1.11.0",
        note = "rskafka does not support consumer-group sessions; field is unused and has no effect"
    )]
    pub session_timeout_ms: u64,

    /// Optional file path for persisting offsets across restarts.
    ///
    /// When set, `DashStreamConsumer` will load a checkpointed offset at startup and resume
    /// from there. Offsets are written periodically when `enable_auto_commit=true` or when
    /// calling `commit()`.
    ///
    /// # M-516: Delivery Guarantee
    ///
    /// This provides **at-least-once delivery**. A message may be re-delivered if:
    /// 1. Message is returned from `next()` / stream iterator
    /// 2. Crash occurs before checkpoint is written
    /// 3. On restart, consumer resumes from last checkpoint
    ///
    /// To avoid data loss/corruption, ensure your processing is idempotent
    /// (e.g., use message_id for deduplication, or use idempotent DB operations).
    pub offset_checkpoint_path: Option<String>,

    /// Enable decompression
    pub enable_decompression: bool,

    /// Maximum message size in bytes (default: 1MB)
    pub max_message_size: usize,

    /// Enable TLS/SSL for secure connections
    /// When true, requires ssl_ca_location or uses system root certificates
    pub enable_tls: bool,

    /// Path to CA certificate file for SSL/TLS
    /// Required when enable_tls is true unless using system root certificates
    pub ssl_ca_location: Option<String>,

    /// Path to client certificate file for mutual TLS (mTLS)
    pub ssl_certificate_location: Option<String>,

    /// Path to client private key file for mutual TLS (mTLS)
    pub ssl_key_location: Option<String>,

    /// SASL username for PLAIN authentication
    /// When set along with sasl_password, enables SASL PLAIN authentication
    pub sasl_username: Option<String>,

    /// SASL password for PLAIN authentication
    pub sasl_password: Option<String>,

    /// Enable strict schema/format validation (security mode)
    /// When true: Rejects messages without valid compression header (0x00/0x01)
    /// When false: Accepts legacy messages for backward compatibility
    /// Default: true (security by default)
    pub enable_strict_validation: bool,

    /// Schema compatibility policy for validating message headers.
    /// Default: Exact (reject schema mismatches).
    pub schema_compatibility: SchemaCompatibility,

    /// Enable per-thread sequence validation.
    ///
    /// When enabled, the consumer checks `Header.sequence` for gaps, duplicates,
    /// and reordering and updates `dashstream_sequence_*` metrics accordingly.
    /// Legacy messages with missing/zero sequence are skipped.
    pub enable_sequence_validation: bool,

    /// Policy for handling sequence gaps when validation is enabled.
    pub gap_recovery_policy: GapRecoveryPolicy,

    /// Enable Dead Letter Queue (DLQ) for failed decodes/validation.
    /// When enabled, malformed messages are sent to `dlq_topic` for forensics.
    pub enable_dlq: bool,

    /// Kafka topic for DLQ messages.
    pub dlq_topic: String,

    /// Timeout for DLQ sends.
    pub dlq_timeout: Duration,

    /// Initial fetch backoff duration when fetching records fails (M-214).
    /// Default: 100ms
    pub fetch_backoff_initial: Duration,

    /// Maximum fetch backoff duration (M-214).
    /// Backoff grows exponentially from initial to max on consecutive failures.
    /// Default: 5 seconds
    pub fetch_backoff_max: Duration,

    /// Sleep duration when broker returns no records (M-214).
    /// Prevents busy-loop when partition is empty or caught up.
    /// Default: 50ms
    pub idle_poll_sleep: Duration,
}

#[allow(deprecated)] // S-8, S-12: Deprecated fields must still be initialized in Default
impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic: "dashstream-events".to_string(),
            partition: 0,
            group_id: "dashstream-consumer".to_string(),
            auto_offset_reset: "earliest".to_string(),
            enable_auto_commit: true,
            auto_commit_interval_ms: DEFAULT_AUTO_COMMIT_INTERVAL_MS,
            session_timeout_ms: DEFAULT_SESSION_TIMEOUT_MS,
            offset_checkpoint_path: None,
            enable_decompression: true,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            enable_tls: false,
            ssl_ca_location: None,
            ssl_certificate_location: None,
            ssl_key_location: None,
            sasl_username: None,
            sasl_password: None,
            enable_strict_validation: true, // Security by default
            schema_compatibility: SchemaCompatibility::Exact,
            enable_sequence_validation: true,
            gap_recovery_policy: GapRecoveryPolicy::default(),
            enable_dlq: true,
            dlq_topic: DEFAULT_DLQ_TOPIC.to_string(),
            dlq_timeout: Duration::from_secs(DEFAULT_DLQ_TIMEOUT_SECS),
            fetch_backoff_initial: Duration::from_millis(DEFAULT_FETCH_BACKOFF_INITIAL_MS),
            fetch_backoff_max: Duration::from_secs(DEFAULT_FETCH_BACKOFF_MAX_SECS),
            idle_poll_sleep: Duration::from_millis(DEFAULT_IDLE_POLL_SLEEP_MS),
        }
    }
}

#[allow(deprecated)] // S-8, S-12: Deprecated fields must still be initialized
impl ConsumerConfig {
    /// M-476: Load consumer configuration from environment variables.
    ///
    /// This reads both consumer-specific settings and Kafka security configuration
    /// from environment variables, enabling secure Kafka connections without code changes.
    ///
    /// **Note:** `DashStreamConsumer` uses rskafka which has different security support
    /// than rdkafka. TLS is fully supported, but SASL mechanisms are limited.
    ///
    /// # Environment Variables
    ///
    /// **Consumer-specific:**
    /// - `KAFKA_BROKERS` / `KAFKA_BOOTSTRAP_SERVERS` - Kafka bootstrap servers (default: "localhost:9092")
    /// - `KAFKA_TOPIC` or `DASHSTREAM_TOPIC` - Kafka topic (default: "dashstream-events")
    /// - `KAFKA_PARTITION` - Partition to consume (default: 0)
    /// - `KAFKA_AUTO_OFFSET_RESET` - Offset reset policy (default: "earliest")
    ///
    /// **Security (compatible with `KafkaSecurityConfig` env vars):**
    /// - `KAFKA_SECURITY_PROTOCOL` - Security protocol (plaintext, ssl, sasl_plaintext, sasl_ssl)
    /// - `KAFKA_SASL_USERNAME` - SASL username (for PLAIN auth)
    /// - `KAFKA_SASL_PASSWORD` - SASL password (for PLAIN auth)
    /// - `KAFKA_SSL_CA_LOCATION` - Path to CA certificate
    /// - `KAFKA_SSL_CERTIFICATE_LOCATION` - Path to client certificate (mTLS)
    /// - `KAFKA_SSL_KEY_LOCATION` - Path to client key (mTLS)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow_streaming::consumer::ConsumerConfig;
    ///
    /// // Set environment variables for secure Kafka
    /// std::env::set_var("KAFKA_BROKERS", "kafka.example.com:9093");
    /// std::env::set_var("KAFKA_SECURITY_PROTOCOL", "sasl_ssl");
    /// std::env::set_var("KAFKA_SASL_USERNAME", "user");
    /// std::env::set_var("KAFKA_SASL_PASSWORD", "secret");
    /// std::env::set_var("KAFKA_SSL_CA_LOCATION", "/path/to/ca.pem");
    ///
    /// let config = ConsumerConfig::from_env();
    /// assert!(config.enable_tls);
    /// ```
    #[must_use]
    pub fn from_env() -> Self {
        use crate::kafka::KafkaSecurityConfig;
        use crate::env_vars;

        // Load security config from standard env vars
        let security = KafkaSecurityConfig::from_env();

        // Consumer-specific env vars
        // Prefer KAFKA_BROKERS for consistency with deployed services (websocket-server/exporter),
        // but keep KAFKA_BOOTSTRAP_SERVERS for backwards compatibility.
        let bootstrap_servers = env_vars::env_string_one_of_or_default(
            env_vars::KAFKA_BROKERS,
            env_vars::KAFKA_BOOTSTRAP_SERVERS,
            "localhost:9092",
        );
        let topic = env_vars::env_string_one_of_or_default(
            env_vars::KAFKA_TOPIC,
            env_vars::DASHSTREAM_TOPIC,
            "dashstream-events",
        );
        let partition: i32 = env_vars::env_i32_or_default(env_vars::KAFKA_PARTITION, 0);
        let auto_offset_reset =
            env_vars::env_string_or_default(env_vars::KAFKA_AUTO_OFFSET_RESET, "earliest");

        // Determine TLS enablement from security protocol
        let enable_tls = security.security_protocol.contains("ssl")
            || security.security_protocol.contains("SSL");

        Self {
            bootstrap_servers,
            topic,
            partition,
            auto_offset_reset,
            enable_tls,
            ssl_ca_location: security.ssl_ca_location,
            ssl_certificate_location: security.ssl_certificate_location,
            ssl_key_location: security.ssl_key_location,
            sasl_username: security.sasl_username,
            sasl_password: security.sasl_password,
            ..Default::default()
        }
    }
}

/// DashFlow Streaming Kafka consumer
pub struct DashStreamConsumer {
    partition_client: PartitionClient,
    config: ConsumerConfig,
    current_offset: i64,
    last_checkpoint_at: Option<Instant>,
    dlq_handler: Option<DlqHandler>,
    sequence_validator: Option<SequenceValidator>,
    buffered_records: VecDeque<rskafka::record::RecordAndOffset>,
    fetch_backoff: Duration,
    next_fetch_allowed_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct OffsetCheckpoint {
    topic: String,
    partition: i32,
    offset: i64,
}

fn load_offset_checkpoint(
    path: &Path,
    config: &ConsumerConfig,
) -> Result<Option<OffsetCheckpoint>> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(Error::Io(std::io::Error::other(format!(
                "Failed to read offset checkpoint '{}': {}",
                path.display(),
                e
            ))))
        }
    };

    let raw = raw.trim();
    if raw.is_empty() {
        return Err(Error::InvalidFormat(format!(
            "Offset checkpoint '{}' is empty",
            path.display()
        )));
    }

    // Structured checkpoint format (preferred).
    if let Ok(checkpoint) = serde_json::from_str::<OffsetCheckpoint>(raw) {
        return Ok(Some(checkpoint));
    }

    // Legacy checkpoint format: a raw integer offset.
    if let Ok(offset) = raw.parse::<i64>() {
        return Ok(Some(OffsetCheckpoint {
            topic: config.topic.clone(),
            partition: config.partition,
            offset,
        }));
    }

    Err(Error::InvalidFormat(format!(
        "Invalid offset checkpoint '{}'; expected JSON or integer offset",
        path.display()
    )))
}

fn ensure_parent_dir_exists(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(parent).map_err(|e| {
        Error::Io(std::io::Error::other(format!(
            "Failed to create checkpoint directory '{}': {}",
            parent.display(),
            e
        )))
    })?;
    Ok(())
}

fn store_offset_checkpoint_atomic(path: &Path, checkpoint: &OffsetCheckpoint) -> Result<()> {
    ensure_parent_dir_exists(path)?;

    let mut payload = serde_json::to_vec(checkpoint).map_err(|e| {
        Error::InvalidFormat(format!(
            "Failed to serialize offset checkpoint for '{}': {}",
            path.display(),
            e
        ))
    })?;
    payload.push(b'\n');

    let pid = std::process::id();
    let nonce = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(e) => {
            warn!(error = %e, "System time before UNIX_EPOCH; using zero nonce for checkpoint temp file");
            0
        }
    };
    let tmp_path = PathBuf::from(format!("{}.tmp.{}.{}", path.display(), pid, nonce));

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
        .map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create checkpoint temp file '{}': {}",
                tmp_path.display(),
                e
            )))
        })?;

    use std::io::Write;
    file.write_all(&payload).map_err(|e| {
        Error::Io(std::io::Error::other(format!(
            "Failed to write checkpoint temp file '{}': {}",
            tmp_path.display(),
            e
        )))
    })?;
    file.sync_all().map_err(|e| {
        Error::Io(std::io::Error::other(format!(
            "Failed to fsync checkpoint temp file '{}': {}",
            tmp_path.display(),
            e
        )))
    })?;

    if let Err(rename_err) = std::fs::rename(&tmp_path, path) {
        // SAFETY: Remove target file to retry rename - failure is OK since we're about
        // to attempt another rename which will also fail if path isn't removable.
        let _ = std::fs::remove_file(path);
        if let Err(second_err) = std::fs::rename(&tmp_path, path) {
            // SAFETY: Cleanup temp file on failure - best effort, original error is
            // what matters for the caller.
            let _ = std::fs::remove_file(&tmp_path);
            return Err(Error::Io(std::io::Error::other(format!(
                "Failed to atomically write offset checkpoint '{}' (rename errors: '{}', '{}')",
                path.display(),
                rename_err,
                second_err
            ))));
        }
    }

    #[cfg(unix)]
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Ok(dir) = std::fs::File::open(parent) {
            // SAFETY: fsync on directory is best-effort durability - failure doesn't
            // affect checkpoint correctness, only crash recovery guarantees.
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

#[allow(deprecated)] // S-8, S-12: Consumer uses deprecated fields for backward compatibility
impl DashStreamConsumer {
    /// Create a new consumer with default configuration
    pub async fn new(bootstrap_servers: &str, topic: &str, group_id: &str) -> Result<Self> {
        let config = ConsumerConfig {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
            group_id: group_id.to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Create a new consumer for a specific partition.
    ///
    /// Group semantics are not supported; `group_id` is set to the default.
    pub async fn new_for_partition(
        bootstrap_servers: &str,
        topic: &str,
        partition: i32,
    ) -> Result<Self> {
        let config = ConsumerConfig {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
            partition,
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Create a new consumer with custom configuration
    pub async fn with_config(mut config: ConsumerConfig) -> Result<Self> {
        if config.partition < 0 {
            return Err(Error::InvalidFormat(format!(
                "Invalid partition {} (must be >= 0)",
                config.partition
            )));
        }
        if config.bootstrap_servers.trim().is_empty() {
            return Err(Error::InvalidFormat(
                "bootstrap_servers must be non-empty".to_string(),
            ));
        }
        if config.topic.trim().is_empty() {
            return Err(Error::InvalidFormat("topic must be non-empty".to_string()));
        }
        if config.group_id.trim().is_empty() {
            return Err(Error::InvalidFormat(
                "group_id must be non-empty".to_string(),
            ));
        }
        if config.max_message_size == 0 {
            return Err(Error::InvalidFormat(
                "max_message_size must be > 0".to_string(),
            ));
        }
        if let Some(ref path) = config.offset_checkpoint_path {
            if path.trim().is_empty() {
                return Err(Error::InvalidFormat(
                    "offset_checkpoint_path must be non-empty when set".to_string(),
                ));
            }
        }
        if config.sasl_username.is_some() ^ config.sasl_password.is_some() {
            return Err(Error::InvalidFormat(
                "Both sasl_username and sasl_password must be set together".to_string(),
            ));
        }
        if config.ssl_certificate_location.is_some() ^ config.ssl_key_location.is_some() {
            return Err(Error::InvalidFormat(
                "Both ssl_certificate_location and ssl_key_location must be set together"
                    .to_string(),
            ));
        }
        if config.enable_dlq && config.dlq_topic.trim().is_empty() {
            return Err(Error::InvalidFormat(
                "dlq_topic must be non-empty when enable_dlq=true".to_string(),
            ));
        }

        // Parse + normalize bootstrap servers.
        let brokers: Vec<String> = config
            .bootstrap_servers
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if brokers.is_empty() {
            return Err(Error::InvalidFormat(
                "bootstrap_servers must contain at least one broker".to_string(),
            ));
        }
        config.bootstrap_servers = brokers.join(",");

        // Create rskafka client builder
        let mut builder = ClientBuilder::new(brokers);

        // Apply TLS configuration if enabled
        // Use spawn_blocking to avoid blocking async runtime with file I/O
        if config.enable_tls {
            let tls_config_data = config.clone();
            let tls_config = tokio::task::spawn_blocking(move || {
                Self::build_tls_config(&tls_config_data)
            })
            .await
            .map_err(|e| Error::Io(std::io::Error::other(format!("TLS config task panicked: {}", e))))??;
            builder = builder.tls_config(Arc::new(tls_config));
        }

        // Apply SASL configuration if credentials provided
        if let (Some(ref username), Some(ref password)) =
            (&config.sasl_username, &config.sasl_password)
        {
            builder = builder.sasl_config(SaslConfig::Plain {
                username: username.clone(),
                password: password.clone(),
            });
        }

        let client = builder.build().await.map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create Kafka client: {}",
                e
            )))
        })?;

        // Get partition client for configured partition.
        // Note: For multi-partition topics, users should create multiple consumers.
        let partition_client = client
            .partition_client(&config.topic, config.partition, UnknownTopicHandling::Error)
            .await
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to get partition client for topic {} partition {}: {}",
                    config.topic, config.partition, e
                )))
            })?;

        // Determine starting offset.
        // Use broker-reported earliest/latest offsets to remain correct under retention/compaction.
        let earliest_offset = partition_client
            .get_offset(OffsetAt::Earliest)
            .await
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to get earliest offset: {}",
                    e
                )))
            })?;
        let latest_offset = partition_client
            .get_offset(OffsetAt::Latest)
            .await
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to get latest offset: {}",
                    e
                )))
            })?;

        let mut current_offset = match config.auto_offset_reset.as_str() {
            "earliest" => earliest_offset,
            "latest" => latest_offset,
            other => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid auto_offset_reset '{}'; expected 'earliest' or 'latest'",
                    other
                )));
            }
        };

        // Load offset checkpoint in blocking task to avoid blocking async runtime
        if let Some(ref checkpoint_path) = config.offset_checkpoint_path {
            let checkpoint_path_owned = checkpoint_path.clone();
            let config_clone = config.clone();
            let checkpoint_result = tokio::task::spawn_blocking(move || {
                load_offset_checkpoint(Path::new(&checkpoint_path_owned), &config_clone)
            })
            .await
            .map_err(|e| Error::Io(std::io::Error::other(format!("Checkpoint load task panicked: {}", e))))?;

            match checkpoint_result {
                Ok(Some(checkpoint)) => {
                    if checkpoint.topic != config.topic || checkpoint.partition != config.partition
                    {
                        warn!(
                            path = checkpoint_path,
                            expected_topic = config.topic,
                            expected_partition = config.partition,
                            checkpoint_topic = checkpoint.topic,
                            checkpoint_partition = checkpoint.partition,
                            "Offset checkpoint does not match configured topic/partition; ignoring"
                        );
                    } else if checkpoint.offset < 0 {
                        warn!(
                            path = checkpoint_path,
                            offset = checkpoint.offset,
                            "Offset checkpoint is negative; ignoring"
                        );
                    } else {
                        let clamped = checkpoint.offset.clamp(earliest_offset, latest_offset);
                        if clamped != checkpoint.offset {
                            warn!(
                                path = checkpoint_path,
                                checkpoint_offset = checkpoint.offset,
                                earliest_offset,
                                latest_offset,
                                resumed_offset = clamped,
                                "Offset checkpoint out of range; clamping to broker offset bounds"
                            );
                        }
                        current_offset = clamped;
                        info!(
                            path = checkpoint_path,
                            resumed_offset = current_offset,
                            "Loaded local offset checkpoint"
                        );
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(
                        error = %e,
                        path = checkpoint_path,
                        "Failed to load local offset checkpoint; using auto_offset_reset"
                    );
                }
            }
        }

        // Optional DLQ handler for failed decodes/validation.
        let dlq_handler = if config.enable_dlq {
            let mut client_config = RdKafkaClientConfig::new();
            client_config
                .set("bootstrap.servers", &config.bootstrap_servers)
                .set(
                    "message.timeout.ms",
                    config.dlq_timeout.as_millis().to_string(),
                );

            let has_sasl = config.sasl_username.is_some() && config.sasl_password.is_some();
            let security_protocol = match (config.enable_tls, has_sasl) {
                (true, true) => "sasl_ssl",
                (true, false) => "ssl",
                (false, true) => "sasl_plaintext",
                (false, false) => "plaintext",
            };
            client_config.set("security.protocol", security_protocol);
            // M-478: Use configurable address family instead of hardcoded v4
            client_config.set(
                "broker.address.family",
                crate::kafka::get_broker_address_family(&config.bootstrap_servers),
            );

            if config.enable_tls {
                if let Some(ref ca_location) = config.ssl_ca_location {
                    client_config.set("ssl.ca.location", ca_location);
                }
                if let Some(ref cert_location) = config.ssl_certificate_location {
                    client_config.set("ssl.certificate.location", cert_location);
                }
                if let Some(ref key_location) = config.ssl_key_location {
                    client_config.set("ssl.key.location", key_location);
                }
            }

            if has_sasl {
                client_config.set("sasl.mechanism", "PLAIN");
                if let Some(ref username) = config.sasl_username {
                    client_config.set("sasl.username", username);
                }
                if let Some(ref password) = config.sasl_password {
                    client_config.set("sasl.password", password);
                }
            }

            let producer: FutureProducer = client_config.create().map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to create DLQ producer: {}",
                    e
                )))
            })?;
            Some(DlqHandler::new(
                producer,
                config.dlq_topic.clone(),
                config.dlq_timeout,
            ))
        } else {
            None
        };

        let sequence_validator = if config.enable_sequence_validation {
            Some(SequenceValidator::with_policy(config.gap_recovery_policy))
        } else {
            None
        };

        Ok(Self {
            partition_client,
            config,
            current_offset,
            last_checkpoint_at: None,
            dlq_handler,
            sequence_validator,
            buffered_records: VecDeque::new(),
            fetch_backoff: Duration::from_millis(0),
            next_fetch_allowed_at: None,
        })
    }

    /// Receive the next message
    pub async fn next(&mut self) -> Option<Result<DashStreamMessage>> {
        loop {
            // Drain buffered records before fetching again.
            let record_and_offset = if let Some(buf) = self.buffered_records.pop_front() {
                buf
            } else {
                // Backoff after recent fetch failures to avoid log spam and hot loops
                // when Kafka is unavailable.
                if let Some(until) = self.next_fetch_allowed_at {
                    let delay = until.saturating_duration_since(Instant::now());
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    self.next_fetch_allowed_at = None;
                }

                // Fetch records from current offset
                // Fetch up to the configured max message size. A too-small max_bytes can
                // prevent brokers from returning large valid records.
                let max_bytes_usize = self
                    .config
                    .max_message_size
                    .max(100_000)
                    .min(i32::MAX as usize);
                let max_bytes = max_bytes_usize as i32;
                let (records, _high_water_mark) = match self
                    .partition_client
                    .fetch_records(self.current_offset, 1..max_bytes, 1000)
                    .await
                {
                    Ok(result) => result,
                    Err(e) => {
                        FETCH_FAILURES_TOTAL.inc();
                        if matches!(
                            &e,
                            RsKafkaError::ServerError {
                                protocol_error: ProtocolError::OffsetOutOfRange,
                                ..
                            }
                        ) {
                            let offset_at = match self.config.auto_offset_reset.as_str() {
                                "earliest" => OffsetAt::Earliest,
                                "latest" => OffsetAt::Latest,
                                _ => OffsetAt::Earliest,
                            };
                            match self.partition_client.get_offset(offset_at).await {
                                Ok(new_offset) => {
                                    warn!(
                                        old_offset = self.current_offset,
                                        new_offset = new_offset,
                                        offset_at = ?offset_at,
                                        "Consumer offset out of range; resetting"
                                    );
                                    self.current_offset = new_offset;
                                    self.fetch_backoff = Duration::from_millis(0);
                                    self.next_fetch_allowed_at = None;
                                    continue;
                                }
                                Err(reset_err) => {
                                    error!(
                                        error = %reset_err,
                                        offset = self.current_offset,
                                        "Failed to reset offset after OffsetOutOfRange"
                                    );
                                    FETCH_FAILURES_TOTAL.inc();
                                    return Some(Err(Error::Io(std::io::Error::other(format!(
                                        "Failed to reset offset after OffsetOutOfRange: {}",
                                        reset_err
                                    )))));
                                }
                            }
                        }

                        // Exponential backoff for subsequent attempts (M-214: configurable).
                        let backoff_initial = self.config.fetch_backoff_initial;
                        let backoff_max = self.config.fetch_backoff_max;
                        self.fetch_backoff = if self.fetch_backoff.is_zero() {
                            backoff_initial
                        } else {
                            std::cmp::min(
                                self.fetch_backoff
                                    .checked_mul(2)
                                    .unwrap_or(backoff_max),
                                backoff_max,
                            )
                        };
                        self.next_fetch_allowed_at = Some(Instant::now() + self.fetch_backoff);

                        error!(
                            error = %e,
                            offset = self.current_offset,
                            backoff_ms = self.fetch_backoff.as_millis(),
                            "Failed to fetch Kafka records"
                        );
                        return Some(Err(Error::Io(std::io::Error::other(format!(
                            "Failed to fetch Kafka records: {}",
                            e
                        )))));
                    }
                };
                // Reset backoff on any successful fetch.
                self.fetch_backoff = Duration::from_millis(0);
                self.next_fetch_allowed_at = None;

                if records.is_empty() {
                    // Idle backoff to avoid busy loop if broker returns immediately (M-214: configurable).
                    tokio::time::sleep(self.config.idle_poll_sleep).await;
                    continue;
                }

                self.buffered_records.extend(records);
                continue;
            };

            let record = &record_and_offset.record;
            let record_offset = record_and_offset.offset;
            let next_offset = record_offset.saturating_add(1);

            // Get payload
            let payload = match &record.value {
                Some(p) => {
                    let bytes = p.as_slice();

                    // Check size limit before decoding
                    let framed_max = self.config.max_message_size.saturating_add(1);
                    if bytes.len() > framed_max {
                        let err = Error::InvalidFormat(format!(
                            "Message size {} bytes exceeds maximum {} bytes",
                            bytes.len(),
                            framed_max
                        ));
                        // Decode/schema failures are not recoverable via retry; advance offset to
                        // avoid getting stuck on a single bad payload.
                        self.current_offset = next_offset;
                        self.send_to_dlq(bytes, &err, record_offset);
                        INVALID_PAYLOADS_TOTAL.inc();
                        DECODE_FAILURES_TOTAL.inc();
                        return Some(Err(err));
                    }

                    bytes
                }
                None => {
                    let err = Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Message has no payload",
                    ));
                    // Decode/schema failures are not recoverable via retry; advance offset to
                    // avoid getting stuck on a single bad payload.
                    self.current_offset = next_offset;
                    self.send_to_dlq(&[], &err, record_offset);
                    INVALID_PAYLOADS_TOTAL.inc();
                    DECODE_FAILURES_TOTAL.inc();
                    return Some(Err(err));
                }
            };

            // Decode message (automatic compression detection via header byte)
            // Use config.max_message_size for decompression limit to prevent
            // compressed payloads from expanding beyond the configured limit
            let decoded = match (
                self.config.enable_decompression,
                self.config.enable_strict_validation,
            ) {
                // Strict mode: reject unknown header bytes (security mode)
                (true, true) => decode_message_strict(payload, self.config.max_message_size),
                // Compatibility mode: accept framed messages and best-effort legacy unframed input.
                (true, false) => decode_message_compatible(payload, self.config.max_message_size),
                // No decompression with strict validation: accept only uncompressed framed messages.
                (false, true) => {
                    if payload.is_empty() {
                        Err(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Empty message payload",
                        )))
                    } else {
                        match payload[0] {
                            HEADER_UNCOMPRESSED => decode_message(&payload[1..]),
                            HEADER_COMPRESSED_ZSTD => Err(Error::InvalidFormat(
                                "Compressed message received but decompression is disabled"
                                    .to_string(),
                            )),
                            invalid_byte => Err(Error::InvalidFormat(format!(
                                "Invalid compression header byte: 0x{:02X}. Expected 0x00 (uncompressed) or 0x01 (zstd). \
                                 Strict validation rejects unknown headers.",
                                invalid_byte
                            ))),
                        }
                    }
                }
                // No decompression with legacy mode: accept framed and unframed messages.
                (false, false) => {
                    if payload.is_empty() {
                        Err(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Empty message payload",
                        )))
                    } else {
                        match payload[0] {
                            HEADER_UNCOMPRESSED => decode_message(&payload[1..]),
                            HEADER_COMPRESSED_ZSTD => Err(Error::InvalidFormat(
                                "Compressed message received but decompression is disabled"
                                    .to_string(),
                            )),
                            _ => decode_message(payload),
                        }
                    }
                }
            };

            // Validate schema version(s) on successfully decoded messages.
            let decoded = decoded.and_then(|msg| {
                validate_message_schema(
                    &msg,
                    self.config.schema_compatibility,
                    self.config.enable_strict_validation,
                )?;
                Ok(msg)
            });

            // If decode/validation failed, send to DLQ for forensics.
            if let Err(ref e) = decoded {
                self.send_to_dlq(payload, e, record_offset);
            }

            let mut final_result = decoded;
            let mut decode_failed = final_result.is_err();
            let mut fatal_sequence_error = false;

            // Sequence validation for successfully decoded messages.
            if let Ok(ref msg) = final_result {
                if let Err(seq_err) = self.validate_sequences(msg) {
                    final_result = Err(seq_err);
                    decode_failed = false;
                    fatal_sequence_error = true;
                }
            }

            // Update offset for next fetch using the actual record offset.
            //
            // This is critical when the earliest offset is non-zero due to retention/compaction.
            // M-518: do NOT advance offsets on fatal sequence-validation errors (GapRecoveryPolicy::Halt),
            // so callers can retry/reset instead of silently skipping.
            if !fatal_sequence_error {
                self.current_offset = next_offset;
            }

            // Update metrics.
            // - Decode/validation errors (protobuf, compression, schema, size) are decode failures.
            // - Sequence validation errors are counted via dashstream_sequence_* metrics.
            // - Successful delivery increments messages_received_total.
            if decode_failed {
                DECODE_FAILURES_TOTAL.inc();
            }
            if final_result.is_ok() {
                MESSAGES_RECEIVED_TOTAL.inc();
            }

            if self.config.enable_auto_commit && self.config.offset_checkpoint_path.is_some() {
                let interval = Duration::from_millis(self.config.auto_commit_interval_ms);
                let should_checkpoint = self
                    .last_checkpoint_at
                    .map_or(true, |t| t.elapsed() >= interval);
                if should_checkpoint {
                    // Use block_in_place to avoid blocking async runtime with checkpoint file I/O
                    if let Err(e) = tokio::task::block_in_place(|| self.commit()) {
                        warn!(error = %e, "Failed to write local offset checkpoint");
                    }
                    self.last_checkpoint_at = Some(Instant::now());
                }
            }

            return Some(final_result);
        }
    }

    /// Receive messages with a timeout
    pub async fn next_timeout(&mut self, timeout: Duration) -> Option<Result<DashStreamMessage>> {
        // Returns `None` if the timeout elapses before a message is received.
        tokio::time::timeout(timeout, self.next())
            .await
            .unwrap_or_default()
    }

    /// Get the consumer group metadata
    pub fn group_id(&self) -> &str {
        &self.config.group_id
    }

    /// Get the subscribed topic
    pub fn topic(&self) -> &str {
        &self.config.topic
    }

    /// Get the current offset
    pub fn current_offset(&self) -> i64 {
        self.current_offset
    }

    /// Commit offsets manually.
    ///
    /// Note: `DashStreamConsumer` uses rskafka's `PartitionClient` (single partition) and does not
    /// integrate with Kafka consumer-group offset commits. When `offset_checkpoint_path` is set,
    /// this persists the current offset to a local file (best-effort) so restarts can resume.
    pub fn commit(&self) -> Result<()> {
        let Some(ref checkpoint_path) = self.config.offset_checkpoint_path else {
            return Ok(());
        };

        let checkpoint = OffsetCheckpoint {
            topic: self.config.topic.clone(),
            partition: self.config.partition,
            offset: self.current_offset,
        };

        match store_offset_checkpoint_atomic(Path::new(checkpoint_path), &checkpoint) {
            Ok(()) => {
                OFFSET_CHECKPOINT_WRITES_TOTAL.inc();
                Ok(())
            }
            Err(e) => {
                OFFSET_CHECKPOINT_FAILURES_TOTAL.inc();
                Err(e)
            }
        }
    }

    /// Best-effort shutdown helper for the internal DLQ handler.
    ///
    /// This waits for in-flight fire-and-forget DLQ sends (bounded by semaphore) and then
    /// flushes the underlying Kafka producer. Call this during graceful shutdown when DLQ
    /// delivery matters.
    pub async fn drain_dlq_and_flush(&self, drain_timeout: Duration, flush_timeout: Duration) {
        if let Some(ref handler) = self.dlq_handler {
            handler.drain_and_flush(drain_timeout, flush_timeout).await;
        }
    }

    /// Best-effort DLQ emission for malformed messages.
    fn send_to_dlq(&self, payload: &[u8], error: &Error, offset: i64) {
        if let Some(ref handler) = self.dlq_handler {
            let error_type = classify_dlq_error_type(error);
            let dlq_message = DlqMessage::new(
                payload,
                error.to_string(),
                self.config.topic.clone(),
                self.config.partition,
                offset,
                self.config.group_id.clone(),
                error_type,
            )
            .with_current_trace_context();
            handler.send_fire_and_forget_with_retry(dlq_message);
        }
    }

    /// Build rustls TLS configuration from ConsumerConfig
    fn build_tls_config(config: &ConsumerConfig) -> Result<rustls::ClientConfig> {
        use rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore};
        use std::fs::File;
        use std::io::BufReader;

        // Load root certificates
        let mut root_store = RootCertStore::empty();

        if let Some(ref ca_path) = config.ssl_ca_location {
            // Load custom CA certificate
            let ca_file = File::open(ca_path).map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to open CA certificate file '{}': {}",
                    ca_path, e
                )))
            })?;
            let mut ca_reader = BufReader::new(ca_file);
            let certs = rustls_pemfile::certs(&mut ca_reader).map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to parse CA certificate: {}",
                    e
                )))
            })?;
            for cert in certs {
                root_store.add(&Certificate(cert)).map_err(|e| {
                    Error::Io(std::io::Error::other(format!(
                        "Failed to add CA certificate to root store: {}",
                        e
                    )))
                })?;
            }
        } else {
            // Use system root certificates
            root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }));
        }

        let builder = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store);

        // Check for mutual TLS (client certificate + key)
        let tls_config = if let (Some(ref cert_path), Some(ref key_path)) =
            (&config.ssl_certificate_location, &config.ssl_key_location)
        {
            // Load client certificate
            let cert_file = File::open(cert_path).map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to open client certificate file '{}': {}",
                    cert_path, e
                )))
            })?;
            let mut cert_reader = BufReader::new(cert_file);
            let certs: Vec<Certificate> = rustls_pemfile::certs(&mut cert_reader)
                .map_err(|e| {
                    Error::Io(std::io::Error::other(format!(
                        "Failed to parse client certificate: {}",
                        e
                    )))
                })?
                .into_iter()
                .map(Certificate)
                .collect();

            // Load client private key
            let key_file = File::open(key_path).map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to open client key file '{}': {}",
                    key_path, e
                )))
            })?;
            let mut key_reader = BufReader::new(key_file);
            let key_bytes = {
                use std::io::Read;
                let mut buf = Vec::new();
                key_reader.read_to_end(&mut buf).map_err(|e| {
                    Error::Io(std::io::Error::other(format!(
                        "Failed to read client key file '{}': {}",
                        key_path, e
                    )))
                })?;
                buf
            };

            let mut cursor = BufReader::new(std::io::Cursor::new(&key_bytes));
            let mut keys = rustls_pemfile::pkcs8_private_keys(&mut cursor).map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to parse client private key: {}",
                    e
                )))
            })?;
            if keys.is_empty() {
                let mut cursor = BufReader::new(std::io::Cursor::new(&key_bytes));
                keys = rustls_pemfile::rsa_private_keys(&mut cursor).map_err(|e| {
                    Error::Io(std::io::Error::other(format!(
                        "Failed to parse client RSA private key: {}",
                        e
                    )))
                })?;
            }
            let key = keys.into_iter().next().ok_or_else(|| {
                Error::Io(std::io::Error::other(
                    "No private key found in key file".to_string(),
                ))
            })?;

            builder
                .with_client_auth_cert(certs, PrivateKey(key))
                .map_err(|e| {
                    Error::Io(std::io::Error::other(format!(
                        "Failed to configure client authentication: {}",
                        e
                    )))
                })?
        } else {
            builder.with_no_client_auth()
        };

        Ok(tls_config)
    }

    fn validate_sequences(&mut self, message: &DashStreamMessage) -> Result<()> {
        let Some(validator) = self.sequence_validator.as_mut() else {
            return Ok(());
        };

        let mut fatal = None::<Error>;

        let mut validate_header = |header: &crate::Header| {
            let thread_id = header.thread_id.as_str();
            let sequence = header.sequence;

            // Skip validation for:
            // - Empty thread_id: malformed message, handled elsewhere
            // - sequence == 0: intentionally unsequenced messages like EventBatch headers
            //   and ExecutionTrace summaries (individual events within batches have their own sequences)
            if thread_id.is_empty() || sequence == 0 {
                return;
            }

            if validator.is_halted(thread_id) {
                fatal = Some(Error::InvalidFormat(format!(
                    "Thread {} is halted due to prior sequence gap",
                    thread_id
                )));
                return;
            }

            match validator.validate(thread_id, sequence) {
                Ok(()) => {}
                Err(err) => match &err {
                    SequenceError::Gap { gap_size, .. } => {
                        SEQUENCE_GAPS_TOTAL.inc();
                        SEQUENCE_GAP_SIZE.observe(*gap_size as f64);
                        if validator.policy == GapRecoveryPolicy::Halt {
                            fatal = Some(Error::InvalidFormat(format!(
                                "Sequence gap detected and thread halted: {}",
                                err
                            )));
                        }
                    }
                    SequenceError::Duplicate { .. } => {
                        SEQUENCE_DUPLICATES_TOTAL.inc();
                        warn!(error = %err, "Duplicate sequence detected");
                    }
                    SequenceError::Reordered { .. } => {
                        SEQUENCE_REORDERS_TOTAL.inc();
                        warn!(error = %err, "Out-of-order sequence detected");
                    }
                },
            }
        };

        match &message.message {
            Some(crate::dash_stream_message::Message::Event(e)) => {
                if let Some(h) = &e.header {
                    validate_header(h);
                }
            }
            Some(crate::dash_stream_message::Message::TokenChunk(t)) => {
                if let Some(h) = &t.header {
                    validate_header(h);
                }
            }
            Some(crate::dash_stream_message::Message::StateDiff(s)) => {
                if let Some(h) = &s.header {
                    validate_header(h);
                }
            }
            Some(crate::dash_stream_message::Message::ToolExecution(te)) => {
                if let Some(h) = &te.header {
                    validate_header(h);
                }
            }
            Some(crate::dash_stream_message::Message::Checkpoint(c)) => {
                if let Some(h) = &c.header {
                    validate_header(h);
                }
            }
            Some(crate::dash_stream_message::Message::Metrics(m)) => {
                if let Some(h) = &m.header {
                    validate_header(h);
                }
            }
            Some(crate::dash_stream_message::Message::Error(err)) => {
                if let Some(h) = &err.header {
                    validate_header(h);
                }
            }
            Some(crate::dash_stream_message::Message::EventBatch(batch)) => {
                if let Some(h) = &batch.header {
                    validate_header(h);
                }
                for event in &batch.events {
                    if let Some(h) = &event.header {
                        validate_header(h);
                    }
                }
            }
            Some(crate::dash_stream_message::Message::ExecutionTrace(trace)) => {
                if let Some(h) = &trace.header {
                    validate_header(h);
                }
            }
            None => {}
        }

        if let Some(err) = fatal {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// M-617: Check if the consumer is healthy and can communicate with Kafka.
    ///
    /// This performs a lightweight metadata fetch to verify broker connectivity.
    /// Useful for health endpoints and readiness probes.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Consumer is healthy and can reach the broker
    /// * `Err(Error)` - Consumer cannot reach the broker or is in an unhealthy state
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_streaming::consumer::DashStreamConsumer;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let consumer = DashStreamConsumer::new("localhost:9092", "topic", "group").await?;
    /// if consumer.health_check().await.is_ok() {
    ///     println!("Consumer is healthy");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health_check(&self) -> Result<()> {
        // Attempt to fetch the latest offset - this validates broker connectivity
        // without consuming any messages
        self.partition_client
            .get_offset(OffsetAt::Latest)
            .await
            .map_err(|e| Error::Kafka(format!("Health check failed: {}", e)))?;
        Ok(())
    }
}

/// Extract schema version from any DashStreamMessage (mirrors codec logic).
fn extract_schema_version(message: &DashStreamMessage) -> Option<u32> {
    match &message.message {
        Some(msg) => match msg {
            crate::dash_stream_message::Message::Event(e) => {
                e.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::TokenChunk(t) => {
                t.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::StateDiff(s) => {
                s.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::ToolExecution(te) => {
                te.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::Checkpoint(c) => {
                c.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::Metrics(m) => {
                m.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::Error(err) => {
                err.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::EventBatch(batch) => {
                batch.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::ExecutionTrace(trace) => {
                trace.header.as_ref().map(|h| h.schema_version)
            }
        },
        None => None,
    }
}

/// Validate schema versions on a decoded message (and EventBatch inner events).
fn validate_message_schema(
    message: &DashStreamMessage,
    policy: SchemaCompatibility,
    require_header: bool,
) -> Result<()> {
    if let Some(version) = extract_schema_version(message) {
        validate_schema_version(version, policy)?;
    } else if require_header {
        return Err(Error::InvalidFormat(
            "Message missing required header with schema version".to_string(),
        ));
    }

    // Validate events within EventBatch for consistent enforcement.
    if let Some(crate::dash_stream_message::Message::EventBatch(batch)) = &message.message {
        for (index, event) in batch.events.iter().enumerate() {
            if let Some(header) = &event.header {
                validate_schema_version(header.schema_version, policy).map_err(|e| {
                    Error::InvalidFormat(format!(
                        "EventBatch event[{}] schema validation failed: {}",
                        index, e
                    ))
                })?;
            } else if require_header {
                return Err(Error::InvalidFormat(format!(
                    "EventBatch event[{}] missing required header with schema version",
                    index
                )));
            }
        }
    }

    Ok(())
}

fn classify_dlq_error_type(error: &Error) -> &'static str {
    match error {
        Error::ProtobufDecode(_) => "decode_error",
        Error::Decompression(_) => "decompression_error",
        Error::InvalidFormat(msg)
            if msg.contains("Schema version") || msg.contains("schema version") =>
        {
            "schema_error"
        }
        Error::InvalidFormat(_) => "invalid_format",
        Error::Io(_) => "io_error",
        _ => "processing_error",
    }
}


#[cfg(test)]
mod tests;
