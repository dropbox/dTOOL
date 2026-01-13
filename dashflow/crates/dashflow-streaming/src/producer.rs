// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Kafka Producer for DashFlow Streaming Messages
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

//! # DashFlow Streaming Producer
//!
//! High-performance Kafka producer for streaming DashFlow Streaming telemetry messages.
//!
//! ## Features
//!
//! - **Thread-based Partitioning**: Messages with the same thread_id go to the same partition
//! - **Compression Support**: Optional Zstd compression for messages >512 bytes
//! - **Async/Await**: Full tokio integration
//! - **Error Handling**: Comprehensive error types with retry support
//!
//! ## S-7: Delivery Semantics and Duplicate Risk
//!
//! **IMPORTANT**: This producer uses application-level retry with Kafka idempotence enabled.
//! While `enable.idempotence=true` prevents duplicates from broker-side retries for the same
//! produce sequence, it does NOT prevent duplicates across separate application sends.
//!
//! **Scenario that can cause duplicates:**
//! 1. Application sends message A
//! 2. Broker receives and persists message A
//! 3. Network timeout before acknowledgment reaches client
//! 4. Application retries, sending message A again as a NEW message
//! 5. Broker now has TWO copies of message A (different sequence numbers)
//!
//! **Mitigation strategies:**
//! - **Consumer-side deduplication**: Use `message_id` from the Header to detect duplicates
//! - **Idempotent processing**: Design consumers to handle duplicate messages gracefully
//! - **Accept at-least-once**: Document that exactly-once requires Kafka transactions
//!
//! **Configuration:**
//! - `retry_config.max_attempts`: Limits application-level retries (default: 3)
//! - `enable_idempotence`: Should remain true to prevent broker-side retry duplicates
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_streaming::producer::DashStreamProducer;
//! use dashflow_streaming::{Event, Header, EventType, MessageType};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create producer
//!     let producer = DashStreamProducer::new(
//!         "localhost:9092",
//!         "dashstream-events"
//!     ).await?;
//!
//!     // Create an event
//!     let event = Event {
//!         header: Some(Header {
//!             message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
//!             timestamp_us: chrono::Utc::now().timestamp_micros(),
//!             tenant_id: "my-tenant".to_string(),
//!             thread_id: "session-123".to_string(),
//!             sequence: 1,
//!             r#type: MessageType::Event as i32,
//!             parent_id: vec![],
//!             compression: 0,
//!             schema_version: 1,
//!         }),
//!         event_type: EventType::GraphStart as i32,
//!         node_id: "".to_string(),
//!         attributes: Default::default(),
//!         duration_us: 0,
//!         llm_request_id: "".to_string(),
//!     };
//!
//!     // Send event
//!     producer.send_event(event).await?;
//!
//!     Ok(())
//! }
//! ```

use crate::codec::{
    encode_message_with_compression_config, DEFAULT_COMPRESSION_LEVEL,
    DEFAULT_COMPRESSION_THRESHOLD,
};
use crate::dlq::{DlqHandler, DlqMessage};
use crate::errors::{Error, Result};
use crate::{
    Checkpoint, DashStreamMessage, Event, Metrics, StateDiff, TokenChunk, ToolExecution,
    // M-243: Shared Kafka configuration constants
    DEFAULT_DLQ_TIMEOUT_SECS, DEFAULT_DLQ_TOPIC,
};
use dashmap::DashMap;
use std::sync::LazyLock;
use prometheus::Counter;
use prost::Message;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// Issue #14: Distributed tracing
use opentelemetry::propagation::{Injector, TextMapPropagator};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing::instrument;

// ============================================================================
// Producer Configuration Constants (M-243: Replace magic numbers)
// ============================================================================
//
// These values are tuned for streaming telemetry workloads.
// Override via ProducerConfig fields if your deployment has different requirements.

/// Default producer send timeout in seconds.
/// 30 seconds provides generous time for broker acknowledgment while detecting failures.
/// Increase for high-latency networks or heavily loaded brokers.
pub const DEFAULT_PRODUCER_TIMEOUT_SECS: u64 = 30;

/// Default maximum message size in bytes (1 MB).
/// Matches Kafka broker default `message.max.bytes`.
/// Must match consumer's max_message_size for compatibility.
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 1_048_576;

// Prometheus metrics (M-624: Use centralized constants)
use crate::metrics_constants::{
    METRIC_MESSAGES_SENT_TOTAL, METRIC_SEND_FAILURES_TOTAL, METRIC_SEND_RETRIES_TOTAL,
};

static MESSAGES_SENT_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_MESSAGES_SENT_TOTAL,
        "Total number of messages successfully sent to Kafka",
    )
});
static SEND_FAILURES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_SEND_FAILURES_TOTAL,
        "Total number of Kafka send failures",
    )
});
static SEND_RETRIES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_SEND_RETRIES_TOTAL,
        "Total number of Kafka send retries",
    )
});

// Issue #14: Helper struct for injecting trace context into Kafka headers
struct KafkaHeaderInjector {
    headers: HashMap<String, String>,
}

impl KafkaHeaderInjector {
    fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }
}

impl Injector for KafkaHeaderInjector {
    fn set(&mut self, key: &str, value: String) {
        self.headers.insert(key.to_string(), value);
    }
}

/// Configuration for DashFlow Streaming producer
#[derive(Debug, Clone)]
pub struct ProducerConfig {
    /// Kafka bootstrap servers (comma-separated)
    pub bootstrap_servers: String,

    /// Topic name for DashFlow Streaming messages
    pub topic: String,

    /// Enable message-level compression for messages larger than `compression_threshold`.
    pub enable_compression: bool,

    /// Minimum uncompressed size (bytes) to attempt compression.
    ///
    /// Defaults to `codec::DEFAULT_COMPRESSION_THRESHOLD` (512 bytes).
    pub compression_threshold: usize,

    /// Zstd compression level to use when compression is enabled.
    ///
    /// Defaults to `codec::DEFAULT_COMPRESSION_LEVEL` (level 3).
    pub compression_level: i32,

    /// Message send timeout
    pub timeout: Duration,

    /// Enable idempotent producer (exactly-once semantics)
    pub enable_idempotence: bool,

    /// Max in-flight requests per connection
    pub max_in_flight: i32,

    /// Compression type for Kafka (none, gzip, snappy, lz4, zstd)
    pub kafka_compression: String,

    /// Tenant ID for multi-tenant deployments
    pub tenant_id: String,

    /// Maximum message size in bytes (default: 1MB)
    /// Messages exceeding this size will be rejected
    pub max_message_size: usize,

    /// Security protocol (plaintext, ssl, sasl_plaintext, sasl_ssl)
    /// Default: "plaintext"
    pub security_protocol: String,

    /// Path to CA certificate file for SSL/TLS
    /// Required when security_protocol is "ssl" or "sasl_ssl"
    pub ssl_ca_location: Option<String>,

    /// Path to client certificate file for mutual TLS
    pub ssl_certificate_location: Option<String>,

    /// Path to client private key file for mutual TLS
    pub ssl_key_location: Option<String>,

    /// Password for the client private key
    pub ssl_key_password: Option<String>,

    /// SASL mechanism (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI, OAUTHBEARER)
    pub sasl_mechanism: Option<String>,

    /// SASL username for PLAIN and SCRAM mechanisms
    pub sasl_username: Option<String>,

    /// SASL password for PLAIN and SCRAM mechanisms
    pub sasl_password: Option<String>,

    /// SSL endpoint identification algorithm for hostname verification
    /// Values: "https" (verify hostname), "none" (skip verification - insecure)
    /// Default: "https" (recommended for production)
    pub ssl_endpoint_identification_algorithm: Option<String>,

    /// Retry configuration for transient send failures
    /// Default: 3 attempts with exponential backoff (100ms base, 5s max)
    pub retry_config: RetryConfig,

    /// Enable Dead Letter Queue (DLQ) for producer send failures.
    ///
    /// When enabled, messages that fail all retries are emitted to `dlq_topic`
    /// for later forensic analysis.
    pub enable_dlq: bool,

    /// Kafka topic for producer DLQ messages.
    pub dlq_topic: String,

    /// Timeout for producer DLQ sends.
    pub dlq_timeout: Duration,
}

/// Configuration for producer retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3)
    pub max_attempts: u32,
    /// Base delay for exponential backoff in milliseconds (default: 100ms)
    pub base_delay_ms: u64,
    /// Maximum delay cap for exponential backoff in milliseconds (default: 5000ms)
    pub max_delay_ms: u64,
    /// Enable retry (default: true)
    pub enabled: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            enabled: true,
        }
    }
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic: "dashstream-events".to_string(),
            enable_compression: true,
            compression_threshold: DEFAULT_COMPRESSION_THRESHOLD,
            compression_level: DEFAULT_COMPRESSION_LEVEL,
            timeout: Duration::from_secs(DEFAULT_PRODUCER_TIMEOUT_SECS),
            enable_idempotence: true,
            max_in_flight: 5,
            kafka_compression: "none".to_string(), // We do compression at message level
            tenant_id: "default".to_string(),      // Default tenant for backward compatibility
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            security_protocol: "plaintext".to_string(),
            ssl_ca_location: None,
            ssl_certificate_location: None,
            ssl_key_location: None,
            ssl_key_password: None,
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
            ssl_endpoint_identification_algorithm: Some("https".to_string()), // Hostname verification enabled by default
            retry_config: RetryConfig::default(),
            enable_dlq: true,
            dlq_topic: DEFAULT_DLQ_TOPIC.to_string(),
            dlq_timeout: Duration::from_secs(DEFAULT_DLQ_TIMEOUT_SECS),
        }
    }
}

impl ProducerConfig {
    /// M-475: Load producer configuration from environment variables.
    ///
    /// This reads both producer-specific settings and Kafka security configuration
    /// from environment variables, enabling secure Kafka connections without code changes.
    ///
    /// # Environment Variables
    ///
    /// **Producer-specific:**
    /// - `KAFKA_BROKERS` / `KAFKA_BOOTSTRAP_SERVERS` - Kafka bootstrap servers (default: "localhost:9092")
    /// - `KAFKA_TOPIC` or `DASHSTREAM_TOPIC` - Kafka topic (default: "dashstream-events")
    /// - `KAFKA_TENANT_ID` - Tenant ID for multi-tenant deployments (default: "default")
    ///
    /// **Security (via `KafkaSecurityConfig::from_env()`):**
    /// - `KAFKA_SECURITY_PROTOCOL` - Security protocol (plaintext, ssl, sasl_plaintext, sasl_ssl)
    /// - `KAFKA_SASL_MECHANISM` - SASL mechanism (PLAIN, SCRAM-SHA-256, etc.)
    /// - `KAFKA_SASL_USERNAME` - SASL username
    /// - `KAFKA_SASL_PASSWORD` - SASL password
    /// - `KAFKA_SSL_CA_LOCATION` - Path to CA certificate
    /// - `KAFKA_SSL_CERTIFICATE_LOCATION` - Path to client certificate
    /// - `KAFKA_SSL_KEY_LOCATION` - Path to client key
    /// - `KAFKA_SSL_KEY_PASSWORD` - Client key password
    /// - `KAFKA_SSL_ENDPOINT_ALGORITHM` - Endpoint identification algorithm (default: "https")
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow_streaming::producer::ProducerConfig;
    ///
    /// // Set environment variables for secure Kafka
    /// std::env::set_var("KAFKA_BROKERS", "kafka.example.com:9093");
    /// std::env::set_var("KAFKA_SECURITY_PROTOCOL", "sasl_ssl");
    /// std::env::set_var("KAFKA_SASL_MECHANISM", "PLAIN");
    /// std::env::set_var("KAFKA_SASL_USERNAME", "user");
    /// std::env::set_var("KAFKA_SASL_PASSWORD", "secret");
    /// std::env::set_var("KAFKA_SSL_CA_LOCATION", "/path/to/ca.pem");
    ///
    /// let config = ProducerConfig::from_env();
    /// assert_eq!(config.security_protocol, "sasl_ssl");
    /// ```
    #[must_use]
    pub fn from_env() -> Self {
        use crate::kafka::KafkaSecurityConfig;
        use crate::env_vars;

        // Load security config from standard env vars
        let security = KafkaSecurityConfig::from_env();

        // Producer-specific env vars
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
        let tenant_id = env_vars::env_string_or_default(env_vars::KAFKA_TENANT_ID, "default");

        Self {
            bootstrap_servers,
            topic,
            tenant_id,
            security_protocol: security.security_protocol,
            ssl_ca_location: security.ssl_ca_location,
            ssl_certificate_location: security.ssl_certificate_location,
            ssl_key_location: security.ssl_key_location,
            ssl_key_password: security.ssl_key_password,
            sasl_mechanism: security.sasl_mechanism,
            sasl_username: security.sasl_username,
            sasl_password: security.sasl_password,
            ssl_endpoint_identification_algorithm: security.ssl_endpoint_identification_algorithm,
            ..Default::default()
        }
    }
}

/// Per-thread sequence number state for monotonic sequencing.
struct ThreadSequenceCounter {
    counter: AtomicU64,
    last_used_tick: AtomicU64,
}

impl ThreadSequenceCounter {
    fn new(initial_tick: u64) -> Self {
        Self {
            counter: AtomicU64::new(0),
            last_used_tick: AtomicU64::new(initial_tick),
        }
    }
}

/// DashFlow Streaming Kafka producer.
pub struct DashStreamProducer {
    producer: FutureProducer,
    config: ProducerConfig,
    /// Per-thread sequence number tracking using lock-free concurrent map
    /// Each thread gets an AtomicU64 counter for non-blocking increments
    sequence_counters: Arc<DashMap<String, ThreadSequenceCounter>>,
    sequence_counter_clock: AtomicU64,
    /// Optional rate limiter for multi-tenant deployments
    rate_limiter: Option<Arc<crate::rate_limiter::TenantRateLimiter>>,
    /// Optional shared DLQ handler for producer failures.
    /// Reused to enforce global backpressure on DLQ fire-and-forget sends.
    dlq_handler: Option<DlqHandler>,
}

impl DashStreamProducer {
    /// Create a new producer with default configuration
    pub async fn new(bootstrap_servers: &str, topic: &str) -> Result<Self> {
        let config = ProducerConfig {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Create a new producer with specific tenant ID
    ///
    /// # Arguments
    ///
    /// * `bootstrap_servers` - Kafka bootstrap servers (comma-separated)
    /// * `topic` - Kafka topic name
    /// * `tenant_id` - Tenant ID for multi-tenant deployments
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_streaming::producer::DashStreamProducer;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let producer = DashStreamProducer::new_with_tenant(
    ///     "localhost:9092",
    ///     "dashstream-events",
    ///     "customer-123"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new_with_tenant(
        bootstrap_servers: &str,
        topic: &str,
        tenant_id: &str,
    ) -> Result<Self> {
        let config = ProducerConfig {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
            tenant_id: tenant_id.to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Create a new producer with custom configuration
    pub async fn with_config(mut config: ProducerConfig) -> Result<Self> {
        // ------------------------------------------------------------------
        // Config validation / clamping (reliability hardening)
        // ------------------------------------------------------------------
        if config.bootstrap_servers.trim().is_empty() {
            return Err(Error::InvalidFormat(
                "bootstrap_servers must be non-empty".to_string(),
            ));
        }
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
        if config.topic.trim().is_empty() {
            return Err(Error::InvalidFormat("topic must be non-empty".to_string()));
        }
        if config.tenant_id.trim().is_empty() {
            return Err(Error::InvalidFormat(
                "tenant_id must be non-empty".to_string(),
            ));
        }
        if config.max_message_size == 0 {
            return Err(Error::InvalidFormat(
                "max_message_size must be > 0".to_string(),
            ));
        }
        if config.compression_threshold == 0 {
            config.compression_threshold = DEFAULT_COMPRESSION_THRESHOLD;
        }
        // Clamp Zstd compression level to valid range.
        const MIN_ZSTD_LEVEL: i32 = 1;
        const MAX_ZSTD_LEVEL: i32 = 22;
        if config.compression_level < MIN_ZSTD_LEVEL {
            tracing::warn!(
                provided = config.compression_level,
                fallback = DEFAULT_COMPRESSION_LEVEL,
                "compression_level too low; using default"
            );
            config.compression_level = DEFAULT_COMPRESSION_LEVEL;
        } else if config.compression_level > MAX_ZSTD_LEVEL {
            tracing::warn!(
                provided = config.compression_level,
                clamped = MAX_ZSTD_LEVEL,
                "compression_level too high; clamping"
            );
            config.compression_level = MAX_ZSTD_LEVEL;
        }
        if config.max_in_flight <= 0 {
            tracing::warn!(
                provided = config.max_in_flight,
                "max_in_flight must be > 0; using 1"
            );
            config.max_in_flight = 1;
        }
        if config.enable_idempotence && config.max_in_flight > 5 {
            tracing::warn!(
                provided = config.max_in_flight,
                clamped = 5,
                "enable_idempotence requires max_in_flight <= 5; clamping"
            );
            config.max_in_flight = 5;
        }
        match config.security_protocol.as_str() {
            "plaintext" | "ssl" | "sasl_plaintext" | "sasl_ssl" => {}
            other => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid security_protocol '{}'; expected plaintext|ssl|sasl_plaintext|sasl_ssl",
                    other
                )));
            }
        }
        if config.sasl_username.is_some() ^ config.sasl_password.is_some() {
            return Err(Error::InvalidFormat(
                "Both sasl_username and sasl_password must be set together".to_string(),
            ));
        }
        if config.enable_dlq && config.dlq_topic.trim().is_empty() {
            return Err(Error::InvalidFormat(
                "dlq_topic must be non-empty when enable_dlq=true".to_string(),
            ));
        }

        // Retry config hardening.
        if config.retry_config.max_attempts == 0 {
            tracing::warn!(
                provided = config.retry_config.max_attempts,
                "retry_config.max_attempts must be > 0; using 1"
            );
            config.retry_config.max_attempts = 1;
        }
        const MAX_RETRY_ATTEMPTS: u32 = 20;
        if config.retry_config.max_attempts > MAX_RETRY_ATTEMPTS {
            tracing::warn!(
                provided = config.retry_config.max_attempts,
                clamped = MAX_RETRY_ATTEMPTS,
                "retry_config.max_attempts too large; clamping"
            );
            config.retry_config.max_attempts = MAX_RETRY_ATTEMPTS;
        }
        if config.retry_config.base_delay_ms == 0 {
            tracing::warn!(
                provided = config.retry_config.base_delay_ms,
                fallback = RetryConfig::default().base_delay_ms,
                "retry_config.base_delay_ms must be > 0; using default"
            );
            config.retry_config.base_delay_ms = RetryConfig::default().base_delay_ms;
        }
        if config.retry_config.max_delay_ms == 0 {
            tracing::warn!(
                provided = config.retry_config.max_delay_ms,
                fallback = RetryConfig::default().max_delay_ms,
                "retry_config.max_delay_ms must be > 0; using default"
            );
            config.retry_config.max_delay_ms = RetryConfig::default().max_delay_ms;
        }
        if config.retry_config.max_delay_ms < config.retry_config.base_delay_ms {
            tracing::warn!(
                base_delay_ms = config.retry_config.base_delay_ms,
                max_delay_ms = config.retry_config.max_delay_ms,
                "retry_config.max_delay_ms < base_delay_ms; aligning max_delay_ms to base_delay_ms"
            );
            config.retry_config.max_delay_ms = config.retry_config.base_delay_ms;
        }

        // Kafka producer timeout hardening. librdkafka expects an i32 ms config value.
        if config.timeout.is_zero() {
            tracing::warn!(fallback_ms = 30_000u64, "timeout must be > 0; using 30s");
            config.timeout = Duration::from_secs(30);
        }
        let timeout_ms_u128 = config.timeout.as_millis();
        let timeout_ms_i32 = timeout_ms_u128.min(i32::MAX as u128) as i32;

        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("message.timeout.ms", timeout_ms_i32.to_string())
            .set("enable.idempotence", config.enable_idempotence.to_string())
            .set(
                "max.in.flight.requests.per.connection",
                config.max_in_flight.to_string(),
            )
            .set("compression.type", &config.kafka_compression)
            .set("acks", "all") // Wait for all replicas
            .set("security.protocol", &config.security_protocol)
            // M-478: Use configurable address family instead of hardcoded v4
            .set(
                "broker.address.family",
                crate::kafka::get_broker_address_family(&config.bootstrap_servers),
            );

        // Apply SSL/TLS settings if configured
        if let Some(ref ca_location) = config.ssl_ca_location {
            client_config.set("ssl.ca.location", ca_location);
        }
        if let Some(ref cert_location) = config.ssl_certificate_location {
            client_config.set("ssl.certificate.location", cert_location);
        }
        if let Some(ref key_location) = config.ssl_key_location {
            client_config.set("ssl.key.location", key_location);
        }
        if let Some(ref key_password) = config.ssl_key_password {
            client_config.set("ssl.key.password", key_password);
        }
        // Only set SSL endpoint identification if using SSL-based security
        if config.security_protocol.contains("ssl") || config.security_protocol.contains("SSL") {
            if let Some(ref algorithm) = config.ssl_endpoint_identification_algorithm {
                client_config.set("ssl.endpoint.identification.algorithm", algorithm);
            }
        }

        // Apply SASL settings if configured
        if let Some(ref mechanism) = config.sasl_mechanism {
            client_config.set("sasl.mechanism", mechanism);
        }
        if let Some(ref username) = config.sasl_username {
            client_config.set("sasl.username", username);
        }
        if let Some(ref password) = config.sasl_password {
            client_config.set("sasl.password", password);
        }

        let producer: FutureProducer = client_config.create().map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create Kafka producer: {}",
                e
            )))
        })?;

        let dlq_handler = if config.enable_dlq {
            Some(DlqHandler::new(
                producer.clone(),
                config.dlq_topic.clone(),
                config.dlq_timeout,
            ))
        } else {
            None
        };

        Ok(Self {
            producer,
            config,
            sequence_counters: Arc::new(DashMap::new()),
            sequence_counter_clock: AtomicU64::new(0),
            rate_limiter: None,
            dlq_handler,
        })
    }

    /// Create producer with rate limiting enabled
    ///
    /// # Arguments
    ///
    /// * `bootstrap_servers` - Kafka bootstrap servers
    /// * `topic` - Kafka topic name
    /// * `rate_limit` - Rate limit configuration (per-tenant)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_streaming::producer::DashStreamProducer;
    /// # use dashflow_streaming::rate_limiter::RateLimit;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let producer = DashStreamProducer::new_with_rate_limiting(
    ///     "localhost:9092",
    ///     "dashstream-events",
    ///     RateLimit {
    ///         messages_per_second: 100.0,
    ///         burst_capacity: 1000,
    ///     },
    ///     None,  // Use in-memory rate limiting
    /// ).await?;
    ///
    /// // Or with Redis for distributed rate limiting across multiple servers:
    /// let producer = DashStreamProducer::new_with_rate_limiting(
    ///     "localhost:9092",
    ///     "dashstream-events",
    ///     RateLimit {
    ///         messages_per_second: 100.0,
    ///         burst_capacity: 1000,
    ///     },
    ///     Some("redis://localhost:6379"),  // Distributed rate limiting
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new_with_rate_limiting(
        bootstrap_servers: &str,
        topic: &str,
        rate_limit: crate::rate_limiter::RateLimit,
        redis_url: Option<&str>,
    ) -> Result<Self> {
        let config = ProducerConfig {
            bootstrap_servers: bootstrap_servers.to_string(),
            topic: topic.to_string(),
            ..Default::default()
        };

        let mut base_producer = Self::with_config(config).await?;

        // Create rate limiter (Redis or in-memory)
        let rate_limiter = if let Some(redis) = redis_url {
            crate::rate_limiter::TenantRateLimiter::new_with_redis(rate_limit, redis)
                .await
                .map_err(|e| Error::InvalidFormat(format!("Failed to connect to Redis: {}", e)))?
        } else {
            crate::rate_limiter::TenantRateLimiter::new(rate_limit)
        };

        base_producer.rate_limiter = Some(Arc::new(rate_limiter));
        Ok(base_producer)
    }

    /// Send a DashFlow Streaming message (Issue #14: with distributed tracing)
    #[instrument(skip(self, message), fields(thread_id = %thread_id))]
    async fn send_message(&self, message: DashStreamMessage, thread_id: &str) -> Result<()> {
        self.send_message_with_timeout(message, thread_id, None)
            .await
    }

    /// Send a DashFlow Streaming message with optional per-call timeout override
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send
    /// * `thread_id` - Thread ID for partition key
    /// * `timeout` - Optional timeout override. If None, uses the config default.
    #[instrument(skip(self, message, timeout), fields(thread_id = %thread_id))]
    async fn send_message_with_timeout(
        &self,
        message: DashStreamMessage,
        thread_id: &str,
        timeout: Option<Duration>,
    ) -> Result<()> {
        let effective_timeout = timeout.unwrap_or(self.config.timeout);

        if thread_id.trim().is_empty() {
            return Err(Error::InvalidFormat(
                "thread_id must be non-empty for partitioning".to_string(),
            ));
        }
        if message.message.is_none() {
            return Err(Error::InvalidFormat(
                "DashStreamMessage is missing inner message".to_string(),
            ));
        }

        // Enforce max_message_size on the *uncompressed* protobuf payload to ensure
        // consumers can always decompress within their configured limit.
        let uncompressed_len = message.encoded_len();
        if uncompressed_len > self.config.max_message_size {
            return Err(Error::InvalidFormat(format!(
                "Uncompressed message size {} bytes exceeds maximum {} bytes",
                uncompressed_len, self.config.max_message_size
            )));
        }

        // Check rate limit if enabled
        if let Some(ref limiter) = self.rate_limiter {
            let tenant_id = &self.config.tenant_id;

            match limiter.check_rate_limit(tenant_id, 1).await {
                Ok(true) => {
                    // Rate limit OK, continue
                }
                Ok(false) => {
                    // Rate limited - reject
                    return Err(Error::InvalidFormat(format!(
                        "Rate limit exceeded for tenant: {}",
                        tenant_id
                    )));
                }
                Err(e) => {
                    // Rate limiter error - fail CLOSED (reject message)
                    // Security: failing open when rate limiter is broken defeats protection
                    return Err(Error::InvalidFormat(format!(
                        "Rate limiter error for tenant {}: {}. Failing closed for safety.",
                        tenant_id, e
                    )));
                }
            }
        }

        // Encode message with optional compression using configured threshold/level.
        // Framing header is always added; enable_compression only controls whether
        // we attempt Zstd compression.
        let (payload, _is_compressed) = encode_message_with_compression_config(
            &message,
            self.config.enable_compression,
            self.config.compression_threshold,
            self.config.compression_level,
        )?;

        // Check framed size limit before sending (allow 1-byte framing header).
        let framed_max = self.config.max_message_size.saturating_add(1);
        if payload.len() > framed_max {
            return Err(Error::InvalidFormat(format!(
                "Message size {} bytes exceeds maximum {} bytes",
                payload.len(),
                framed_max
            )));
        }

        // Issue #14: Inject trace context into Kafka headers for distributed tracing
        let mut injector = KafkaHeaderInjector::new();
        let propagator = TraceContextPropagator::new();
        let context = opentelemetry::Context::current();
        propagator.inject_context(&context, &mut injector);

        // Retry loop with exponential backoff
        // NOTE: message_id (in Header) is preserved across retries because the payload is
        // encoded ONCE above and reused. See S-7 doc comment for application-level retry scenarios.
        let retry_config = &self.config.retry_config;
        let max_attempts = if retry_config.enabled {
            retry_config.max_attempts.max(1)
        } else {
            1
        };
        let mut last_error = None;

        for attempt in 0..max_attempts {
            // Build record fresh for each attempt (FutureRecord is not Clone)
            let mut record = FutureRecord::to(&self.config.topic)
                .key(thread_id.as_bytes())
                .payload(&payload);

            // Re-attach headers for each attempt
            if !injector.headers.is_empty() {
                let mut headers = rdkafka::message::OwnedHeaders::new();
                for (key, value) in &injector.headers {
                    headers = headers.insert(rdkafka::message::Header {
                        key,
                        value: Some(value.as_bytes()),
                    });
                }
                record = record.headers(headers);
            }

            match self
                .producer
                .send(record, Timeout::After(effective_timeout))
                .await
            {
                Ok(_) => {
                    MESSAGES_SENT_TOTAL.inc();
                    if attempt > 0 {
                        tracing::info!(
                            attempt = attempt + 1,
                            thread_id = %thread_id,
                            "Kafka send succeeded after retry"
                        );
                    }
                    return Ok(());
                }
                Err((err, _)) => {
                    // Don't retry or sleep after the last attempt
                    if attempt + 1 < max_attempts {
                        SEND_RETRIES_TOTAL.inc();

                        // Calculate delay with exponential backoff and jitter
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
                            thread_id = %thread_id,
                            error = %err,
                            "Kafka send failed, retrying"
                        );

                        tokio::time::sleep(Duration::from_millis(total_delay)).await;
                    }
                    // Store error after logging to avoid borrow-after-move
                    last_error = Some(err);
                }
            }
        }

        // All retries exhausted - last_error is guaranteed to be Some because we only
        // exit the loop after at least one Err (max_attempts >= 1)
        #[allow(clippy::expect_used)] // Invariant: last_error is always Some when loop exhausts
        let err = last_error.expect("last_error should be Some after retry loop exhaustion");
        SEND_FAILURES_TOTAL.inc();
        tracing::error!(
            attempts = max_attempts,
            thread_id = %thread_id,
            error = %err,
            "Kafka send failed after all retries"
        );

        // Best-effort DLQ capture for forensic analysis (no recursion).
        // DlqMessage is consumer-oriented; we use sentinel partition/offset for producer failures.
        let dlq_message = DlqMessage::new(
            &payload,
            format!("Kafka send failed after {} attempts: {}", max_attempts, err),
            self.config.topic.clone(),
            -1,
            -1,
            format!("producer:{}", self.config.tenant_id),
            "kafka_send_error",
        )
        .with_thread_id(thread_id.to_string())
        .with_tenant_id(self.config.tenant_id.clone())
        .with_current_trace_context();
        if let Some(ref handler) = self.dlq_handler {
            handler.send_fire_and_forget_with_retry(dlq_message);
        }

        Err(Error::Io(std::io::Error::other(format!(
            "Failed to send Kafka message after {} attempts: {}",
            max_attempts, err
        ))))
    }

    /// Send an Event message
    pub async fn send_event(&self, event: Event) -> Result<()> {
        let thread_id = event
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Send an Event message with a per-call timeout override
    ///
    /// Use this when you need different timeout behavior for specific events,
    /// e.g., shorter timeouts for time-sensitive operations or longer timeouts
    /// when broker latency is expected.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to send
    /// * `timeout` - Per-call timeout override
    pub async fn send_event_with_timeout(&self, event: Event, timeout: Duration) -> Result<()> {
        let thread_id = event
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        self.send_message_with_timeout(message, &thread_id, Some(timeout))
            .await
    }

    /// Send a StateDiff message
    pub async fn send_state_diff(&self, diff: StateDiff) -> Result<()> {
        let thread_id = diff
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::StateDiff(diff)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Send a TokenChunk message
    pub async fn send_token_chunk(&self, chunk: TokenChunk) -> Result<()> {
        let thread_id = chunk
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::TokenChunk(chunk)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Send a ToolExecution message
    pub async fn send_tool_execution(&self, tool: ToolExecution) -> Result<()> {
        let thread_id = tool
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::ToolExecution(tool)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Send a Checkpoint message
    pub async fn send_checkpoint(&self, checkpoint: Checkpoint) -> Result<()> {
        let thread_id = checkpoint
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Checkpoint(checkpoint)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Send a Metrics message
    pub async fn send_metrics(&self, metrics: Metrics) -> Result<()> {
        let thread_id = metrics
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Metrics(metrics)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Send an Error message
    pub async fn send_error(&self, error: crate::Error) -> Result<()> {
        let thread_id = error
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Error(error)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Send an EventBatch message containing multiple events
    ///
    /// Batching events reduces scheduler overhead by sending multiple events
    /// in a single Kafka message. The batch header provides metadata for the
    /// entire batch, while individual events retain their own headers.
    ///
    /// # Arguments
    ///
    /// * `batch` - EventBatch containing a header and multiple events
    pub async fn send_event_batch(&self, batch: crate::EventBatch) -> Result<()> {
        let thread_id = batch
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::EventBatch(batch)),
        };

        self.send_message(message, &thread_id).await
    }

    /// Flush pending messages and wait for acknowledgments
    pub async fn flush(&self, timeout: Duration) -> Result<()> {
        let producer = self.producer.clone();
        tokio::task::spawn_blocking(move || producer.flush(Timeout::After(timeout)))
            .await
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to join flush task: {}",
                    e
                )))
            })?
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to flush Kafka producer: {}",
                    e
                )))
            })
    }

    /// Best-effort shutdown helper for the internal DLQ handler.
    ///
    /// This waits for in-flight fire-and-forget DLQ sends (bounded by semaphore) and then
    /// flushes the underlying Kafka producer.
    pub async fn drain_dlq_and_flush(&self, drain_timeout: Duration, flush_timeout: Duration) {
        if let Some(ref handler) = self.dlq_handler {
            handler.drain_and_flush(drain_timeout, flush_timeout).await;
        }
    }

    /// Get the next sequence number for a thread
    ///
    /// Sequence numbers start at 1 and increment for each message in the thread.
    /// This enables detection of message loss, reordering, and duplicates.
    ///
    /// Uses lock-free atomic operations via DashMap to avoid blocking async tasks.
    /// This is critical for high-throughput telemetry where blocking could stall
    /// the async executor.
    fn next_sequence(&self, thread_id: &str) -> u64 {
        self.maybe_prune_sequence_counters(thread_id);

        let tick = self
            .sequence_counter_clock
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);

        // Get or insert a per-thread counter, then atomically increment.
        let entry = self
            .sequence_counters
            .entry(thread_id.to_string())
            .or_insert_with(|| ThreadSequenceCounter::new(tick));

        entry.last_used_tick.store(tick, Ordering::Relaxed);
        entry.counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Prevent unbounded growth of per-thread sequence counters.
    fn maybe_prune_sequence_counters(&self, current_thread_id: &str) {
        const MAX_SEQUENCE_COUNTERS: usize = 100_000;
        const PRUNE_BATCH: usize = 1000;

        if self.sequence_counters.len() <= MAX_SEQUENCE_COUNTERS {
            return;
        }

        // M-517: Remove least-recently-used counters first, rather than relying on DashMap's
        // nondeterministic iteration order (which can evict active threads and cause frequent
        // sequence resets).
        let mut oldest: BinaryHeap<(u64, String)> = BinaryHeap::with_capacity(PRUNE_BATCH);
        for entry in self.sequence_counters.iter() {
            if entry.key().as_str() == current_thread_id {
                continue;
            }

            let last_used = entry.value().last_used_tick.load(Ordering::Relaxed);
            if oldest.len() < PRUNE_BATCH {
                oldest.push((last_used, entry.key().clone()));
                continue;
            }

            if let Some((largest_tick_in_heap, _)) = oldest.peek() {
                if last_used < *largest_tick_in_heap {
                    oldest.pop();
                    oldest.push((last_used, entry.key().clone()));
                }
            }
        }

        let removed = oldest.len();
        for (_, key) in oldest.into_iter() {
            self.sequence_counters.remove(&key);
        }

        tracing::warn!(
            removed = removed,
            remaining = self.sequence_counters.len(),
            max = MAX_SEQUENCE_COUNTERS,
            "Pruned producer sequence counters to cap memory"
        );
    }

    /// Create a standard header for messages
    ///
    /// Helper method for creating headers with consistent metadata.
    /// Used by quality monitoring and other telemetry components.
    pub fn create_header(
        &self,
        thread_id: &str,
        message_type: crate::MessageType,
    ) -> crate::Header {
        let sequence = self.next_sequence(thread_id);

        crate::Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: self.config.tenant_id.clone(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: message_type as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: crate::CURRENT_SCHEMA_VERSION,
        }
    }

    /// M-617: Check if the producer is healthy and can communicate with Kafka.
    ///
    /// This fetches cluster metadata to verify broker connectivity.
    /// Useful for health endpoints and readiness probes.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for metadata response
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Producer is healthy and can reach the broker
    /// * `Err(Error)` - Producer cannot reach the broker or is in an unhealthy state
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_streaming::producer::DashStreamProducer;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let producer = DashStreamProducer::new("localhost:9092", "topic").await?;
    /// if producer.health_check(Duration::from_secs(5)).is_ok() {
    ///     println!("Producer is healthy");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn health_check(&self, timeout: Duration) -> Result<()> {
        // Fetch cluster metadata - this validates broker connectivity
        self.producer
            .client()
            .fetch_metadata(Some(&self.config.topic), Timeout::After(timeout))
            .map_err(|e| Error::Kafka(format!("Health check failed: {}", e)))?;
        Ok(())
    }
}

/// Default timeout for flushing messages on drop.
///
/// Set to 0ms to keep Drop non-blocking; callers should invoke `flush()`
/// explicitly on graceful shutdown when delivery matters.
const DROP_FLUSH_TIMEOUT: Duration = Duration::from_millis(0);

impl Drop for DashStreamProducer {
    /// Best-effort non-blocking flush on drop.
    ///
    /// `Drop` cannot be async, and a blocking flush can stall runtime threads on shutdown.
    /// We therefore attempt a zero-timeout flush here and rely on explicit `flush()` calls
    /// for graceful shutdown.
    fn drop(&mut self) {
        // Attempt a zero-timeout flush (non-blocking).
        let _ = self
            .producer
            .flush(Timeout::After(DROP_FLUSH_TIMEOUT))
            .map_err(|e| {
                tracing::debug!(
                    error = %e,
                    "Drop flush timed out; call DashStreamProducer::flush() for guaranteed delivery"
                );
            });
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventType, Header, MessageType};

    fn create_test_event() -> Event {
        Event {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: "test-thread".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "start".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_send_event() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        // Start Kafka in Docker (automatically cleaned up when test ends)
        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(3)).await;

        let producer = DashStreamProducer::new(&bootstrap_servers, "test-events")
            .await
            .expect("Failed to create producer");

        let event = create_test_event();
        producer
            .send_event(event)
            .await
            .expect("Failed to send event");

        producer
            .flush(Duration::from_secs(5))
            .await
            .expect("Failed to flush");
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_send_multiple_events() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        // Start Kafka in Docker (automatically cleaned up when test ends)
        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(3)).await;

        let producer = DashStreamProducer::new(&bootstrap_servers, "test-events")
            .await
            .expect("Failed to create producer");

        for i in 0..10 {
            let mut event = create_test_event();
            if let Some(ref mut header) = event.header {
                header.sequence = i;
            }
            producer
                .send_event(event)
                .await
                .expect("Failed to send event");
        }

        producer
            .flush(Duration::from_secs(5))
            .await
            .expect("Failed to flush");
    }

    #[test]
    fn test_producer_config_default() {
        let config = ProducerConfig::default();
        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.topic, "dashstream-events");
        assert!(config.enable_compression);
        assert!(config.enable_idempotence);
        assert_eq!(config.tenant_id, "default");
    }

    // === ProducerConfig Tests ===

    #[test]
    fn test_producer_config_custom() {
        let config = ProducerConfig {
            bootstrap_servers: "kafka1:9092,kafka2:9092".to_string(),
            topic: "custom-topic".to_string(),
            enable_compression: false,
            timeout: Duration::from_secs(60),
            enable_idempotence: false,
            max_in_flight: 10,
            kafka_compression: "gzip".to_string(),
            tenant_id: "custom-tenant".to_string(),
            max_message_size: 1_048_576,
            ..Default::default()
        };

        assert_eq!(config.bootstrap_servers, "kafka1:9092,kafka2:9092");
        assert_eq!(config.topic, "custom-topic");
        assert!(!config.enable_compression);
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(!config.enable_idempotence);
        assert_eq!(config.max_in_flight, 10);
        assert_eq!(config.kafka_compression, "gzip");
        assert_eq!(config.tenant_id, "custom-tenant");
        // Default security config
        assert_eq!(config.security_protocol, "plaintext");
        assert!(config.ssl_ca_location.is_none());
        assert!(config.sasl_mechanism.is_none());
    }

    #[test]
    fn test_producer_config_clone() {
        let config = ProducerConfig::default();
        let cloned = config.clone();

        assert_eq!(config.bootstrap_servers, cloned.bootstrap_servers);
        assert_eq!(config.topic, cloned.topic);
        assert_eq!(config.enable_compression, cloned.enable_compression);
        assert_eq!(config.timeout, cloned.timeout);
    }

    #[test]
    fn test_producer_config_tls_sasl() {
        // Test SSL config
        let ssl_config = ProducerConfig {
            security_protocol: "ssl".to_string(),
            ssl_ca_location: Some("/path/to/ca.pem".to_string()),
            ssl_certificate_location: Some("/path/to/cert.pem".to_string()),
            ssl_key_location: Some("/path/to/key.pem".to_string()),
            ssl_key_password: Some("secret".to_string()),
            ..Default::default()
        };
        assert_eq!(ssl_config.security_protocol, "ssl");
        assert_eq!(
            ssl_config.ssl_ca_location,
            Some("/path/to/ca.pem".to_string())
        );
        assert_eq!(
            ssl_config.ssl_certificate_location,
            Some("/path/to/cert.pem".to_string())
        );
        assert_eq!(
            ssl_config.ssl_key_location,
            Some("/path/to/key.pem".to_string())
        );
        assert_eq!(ssl_config.ssl_key_password, Some("secret".to_string()));

        // Test SASL config
        let sasl_config = ProducerConfig {
            security_protocol: "sasl_ssl".to_string(),
            ssl_ca_location: Some("/path/to/ca.pem".to_string()),
            sasl_mechanism: Some("SCRAM-SHA-256".to_string()),
            sasl_username: Some("user".to_string()),
            sasl_password: Some("password".to_string()),
            ..Default::default()
        };
        assert_eq!(sasl_config.security_protocol, "sasl_ssl");
        assert_eq!(
            sasl_config.sasl_mechanism,
            Some("SCRAM-SHA-256".to_string())
        );
        assert_eq!(sasl_config.sasl_username, Some("user".to_string()));
        assert_eq!(sasl_config.sasl_password, Some("password".to_string()));

        // Test SASL_PLAINTEXT (no SSL)
        let sasl_plain_config = ProducerConfig {
            security_protocol: "sasl_plaintext".to_string(),
            sasl_mechanism: Some("PLAIN".to_string()),
            sasl_username: Some("user".to_string()),
            sasl_password: Some("password".to_string()),
            ..Default::default()
        };
        assert_eq!(sasl_plain_config.security_protocol, "sasl_plaintext");
        assert!(sasl_plain_config.ssl_ca_location.is_none());
    }

    #[test]
    fn test_producer_config_debug() {
        let config = ProducerConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("ProducerConfig"));
        assert!(debug_str.contains("localhost:9092"));
        assert!(debug_str.contains("dashstream-events"));
    }

    #[test]
    fn test_producer_config_timeout_values() {
        let config = ProducerConfig {
            timeout: Duration::from_millis(100),
            ..Default::default()
        };
        assert_eq!(config.timeout.as_millis(), 100);

        let config2 = ProducerConfig {
            timeout: Duration::from_secs(300),
            ..Default::default()
        };
        assert_eq!(config2.timeout.as_secs(), 300);
    }

    #[test]
    fn test_producer_config_max_in_flight_values() {
        let config = ProducerConfig {
            max_in_flight: 1,
            ..Default::default()
        };
        assert_eq!(config.max_in_flight, 1);

        let config2 = ProducerConfig {
            max_in_flight: 100,
            ..Default::default()
        };
        assert_eq!(config2.max_in_flight, 100);
    }

    #[test]
    fn test_producer_config_compression_types() {
        let types = vec!["none", "gzip", "snappy", "lz4", "zstd"];

        for compression_type in types {
            let config = ProducerConfig {
                kafka_compression: compression_type.to_string(),
                ..Default::default()
            };
            assert_eq!(config.kafka_compression, compression_type);
        }
    }

    #[test]
    fn test_producer_config_idempotence_variants() {
        let config_enabled = ProducerConfig {
            enable_idempotence: true,
            ..Default::default()
        };
        assert!(config_enabled.enable_idempotence);

        let config_disabled = ProducerConfig {
            enable_idempotence: false,
            ..Default::default()
        };
        assert!(!config_disabled.enable_idempotence);
    }

    #[test]
    fn test_producer_config_enable_compression_variants() {
        let config_enabled = ProducerConfig {
            enable_compression: true,
            ..Default::default()
        };
        assert!(config_enabled.enable_compression);

        let config_disabled = ProducerConfig {
            enable_compression: false,
            ..Default::default()
        };
        assert!(!config_disabled.enable_compression);
    }

    #[test]
    fn test_producer_config_topic_names() {
        let topics = vec![
            "events",
            "dashstream-events",
            "my.events.topic",
            "events_2025",
            "EVENTS-UPPERCASE",
        ];

        for topic in topics {
            let config = ProducerConfig {
                topic: topic.to_string(),
                ..Default::default()
            };
            assert_eq!(config.topic, topic);
        }
    }

    #[test]
    fn test_producer_config_multiple_bootstrap_servers() {
        let servers = vec![
            "localhost:9092",
            "kafka1:9092,kafka2:9092",
            "kafka1:9092,kafka2:9092,kafka3:9092",
            "broker1.example.com:9092,broker2.example.com:9092",
        ];

        for server in servers {
            let config = ProducerConfig {
                bootstrap_servers: server.to_string(),
                ..Default::default()
            };
            assert_eq!(config.bootstrap_servers, server);
        }
    }

    // === Event Creation Tests ===

    #[test]
    fn test_create_test_event() {
        let event = create_test_event();

        assert!(event.header.is_some());
        let header = event.header.unwrap();
        assert_eq!(header.tenant_id, "test-tenant");
        assert_eq!(header.thread_id, "test-thread");
        assert_eq!(header.sequence, 1);
        assert_eq!(header.r#type, MessageType::Event as i32);
        assert_eq!(header.schema_version, 1);
        assert_eq!(header.compression, 0);
        assert!(header.parent_id.is_empty());
        assert_eq!(header.message_id.len(), 16); // UUID is 16 bytes
    }

    #[test]
    fn test_create_test_event_event_type() {
        let event = create_test_event();
        assert_eq!(event.event_type, EventType::GraphStart as i32);
        assert_eq!(event.node_id, "start");
    }

    #[test]
    fn test_create_test_event_initial_values() {
        let event = create_test_event();
        assert!(event.attributes.is_empty());
        assert_eq!(event.duration_us, 0);
        assert_eq!(event.llm_request_id, "");
    }

    #[test]
    fn test_create_test_event_timestamp() {
        let event = create_test_event();
        let header = event.header.unwrap();

        // Timestamp should be recent (within last second)
        let now = chrono::Utc::now().timestamp_micros();
        let diff = (now - header.timestamp_us).abs();
        assert!(diff < 1_000_000); // Within 1 second
    }

    #[test]
    fn test_create_test_event_unique_message_ids() {
        let event1 = create_test_event();
        let event2 = create_test_event();

        let id1 = event1.header.unwrap().message_id;
        let id2 = event2.header.unwrap().message_id;

        // UUIDs should be unique
        assert_ne!(id1, id2);
    }

    // === Message Type Extraction Tests ===

    #[test]
    fn test_event_thread_id_extraction() {
        let event = create_test_event();
        let thread_id = event
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();
        assert_eq!(thread_id, "test-thread");
    }

    #[test]
    fn test_event_without_header_thread_id() {
        let event = Event {
            header: None,
            event_type: EventType::GraphStart as i32,
            node_id: "start".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        };

        let thread_id = event
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();
        assert_eq!(thread_id, "");
    }

    #[test]
    fn test_state_diff_message_creation() {
        let diff = StateDiff {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: "diff-thread".to_string(),
                sequence: 1,
                r#type: MessageType::StateDiff as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            base_checkpoint_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            operations: vec![],
            state_hash: vec![0; 32],
            full_state: vec![],
        };

        let thread_id = diff
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();
        assert_eq!(thread_id, "diff-thread");
        assert_eq!(diff.base_checkpoint_id.len(), 16);
    }

    #[test]
    fn test_token_chunk_message_creation() {
        let chunk = TokenChunk {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: "token-thread".to_string(),
                sequence: 1,
                r#type: MessageType::TokenChunk as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            request_id: "req-123".to_string(),
            text: "Hello".to_string(),
            token_ids: vec![],
            logprobs: vec![],
            chunk_index: 0,
            is_final: false,
            model: "gpt-4".to_string(),
            finish_reason: 0,
            stats: None,
        };

        let thread_id = chunk
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();
        assert_eq!(thread_id, "token-thread");
        assert_eq!(chunk.text, "Hello");
        assert!(!chunk.is_final);
    }

    #[test]
    fn test_tool_execution_message_creation() {
        use crate::tool_execution::ExecutionStage;
        let tool = ToolExecution {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: "tool-thread".to_string(),
                sequence: 1,
                r#type: MessageType::ToolExecution as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            call_id: "call-123".to_string(),
            tool_name: "calculator".to_string(),
            stage: ExecutionStage::Completed as i32,
            arguments: br#"{"x": 5, "y": 3}"#.to_vec(),
            result: b"8".to_vec(),
            error: "".to_string(),
            error_details: None,
            duration_us: 1000,
            retry_count: 0,
        };

        let thread_id = tool
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();
        assert_eq!(thread_id, "tool-thread");
        assert_eq!(tool.tool_name, "calculator");
        assert_eq!(tool.call_id, "call-123");
    }

    #[test]
    fn test_checkpoint_message_creation() {
        let checkpoint = Checkpoint {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: "checkpoint-thread".to_string(),
                sequence: 1,
                r#type: MessageType::Checkpoint as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            checkpoint_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            state: vec![1, 2, 3, 4],
            state_type: "TestState".to_string(),
            checksum: vec![0; 32], // SHA-256 hash
            storage_uri: "".to_string(),
            compression_info: None,
            metadata: Default::default(),
        };

        let thread_id = checkpoint
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();
        assert_eq!(thread_id, "checkpoint-thread");
        assert_eq!(checkpoint.checkpoint_id.len(), 16); // UUID is 16 bytes
        assert_eq!(checkpoint.state, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_metrics_message_creation() {
        let metrics = Metrics {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: "metrics-thread".to_string(),
                sequence: 1,
                r#type: MessageType::Metrics as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            scope: "graph".to_string(),
            scope_id: "graph-1".to_string(),
            metrics: Default::default(),
            tags: Default::default(),
        };

        let thread_id = metrics
            .header
            .as_ref()
            .map(|h| h.thread_id.clone())
            .unwrap_or_default();
        assert_eq!(thread_id, "metrics-thread");
        assert_eq!(metrics.scope, "graph");
    }

    // === DashStreamMessage Wrapping Tests ===

    #[test]
    fn test_dashstream_message_event_wrapping() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        match message.message {
            Some(crate::dash_stream_message::Message::Event(e)) => {
                assert_eq!(e.node_id, "start");
            }
            _ => panic!("Expected Event message"),
        }
    }

    #[test]
    fn test_dashstream_message_state_diff_wrapping() {
        let diff = StateDiff {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::StateDiff as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            base_checkpoint_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            operations: vec![],
            state_hash: vec![0; 32],
            full_state: vec![],
        };

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::StateDiff(diff)),
        };

        match message.message {
            Some(crate::dash_stream_message::Message::StateDiff(d)) => {
                assert_eq!(d.base_checkpoint_id.len(), 16);
            }
            _ => panic!("Expected StateDiff message"),
        }
    }

    #[test]
    fn test_dashstream_message_token_chunk_wrapping() {
        let chunk = TokenChunk {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::TokenChunk as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            request_id: "req-1".to_string(),
            text: "test".to_string(),
            token_ids: vec![],
            logprobs: vec![],
            chunk_index: 0,
            is_final: false,
            model: "gpt-4".to_string(),
            finish_reason: 0,
            stats: None,
        };

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::TokenChunk(chunk)),
        };

        match message.message {
            Some(crate::dash_stream_message::Message::TokenChunk(c)) => {
                assert_eq!(c.text, "test");
            }
            _ => panic!("Expected TokenChunk message"),
        }
    }

    #[test]
    fn test_dashstream_message_tool_execution_wrapping() {
        use crate::tool_execution::ExecutionStage;
        let tool = ToolExecution {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::ToolExecution as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            call_id: "call-1".to_string(),
            tool_name: "calc".to_string(),
            stage: ExecutionStage::Completed as i32,
            arguments: b"{}".to_vec(),
            result: b"42".to_vec(),
            error: "".to_string(),
            error_details: None,
            duration_us: 100,
            retry_count: 0,
        };

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::ToolExecution(tool)),
        };

        match message.message {
            Some(crate::dash_stream_message::Message::ToolExecution(t)) => {
                assert_eq!(t.tool_name, "calc");
            }
            _ => panic!("Expected ToolExecution message"),
        }
    }

    #[test]
    fn test_dashstream_message_checkpoint_wrapping() {
        let checkpoint = Checkpoint {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::Checkpoint as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            checkpoint_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            state: vec![],
            state_type: "TestState".to_string(),
            checksum: vec![0; 32],
            storage_uri: "".to_string(),
            compression_info: None,
            metadata: Default::default(),
        };

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Checkpoint(checkpoint)),
        };

        match message.message {
            Some(crate::dash_stream_message::Message::Checkpoint(c)) => {
                assert_eq!(c.checkpoint_id.len(), 16);
            }
            _ => panic!("Expected Checkpoint message"),
        }
    }

    #[test]
    fn test_dashstream_message_metrics_wrapping() {
        let metrics = Metrics {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::Metrics as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            scope: "test".to_string(),
            scope_id: "test-1".to_string(),
            metrics: Default::default(),
            tags: Default::default(),
        };

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Metrics(metrics)),
        };

        match message.message {
            Some(crate::dash_stream_message::Message::Metrics(_)) => {
                // Success
            }
            _ => panic!("Expected Metrics message"),
        }
    }

    // === Message Type Coverage Tests ===

    #[test]
    fn test_message_type_variants() {
        // Ensure all MessageType variants are valid
        let types = vec![
            MessageType::Event as i32,
            MessageType::StateDiff as i32,
            MessageType::TokenChunk as i32,
            MessageType::ToolExecution as i32,
            MessageType::Checkpoint as i32,
            MessageType::Metrics as i32,
        ];

        for msg_type in types {
            assert!(msg_type >= 0);
        }
    }

    #[test]
    fn test_event_type_variants() {
        // Ensure all EventType variants are valid
        let types = vec![
            EventType::GraphStart as i32,
            EventType::GraphEnd as i32,
            EventType::NodeStart as i32,
            EventType::NodeEnd as i32,
        ];

        for event_type in types {
            assert!(event_type >= 0);
        }
    }

    // === Header Field Tests ===

    #[test]
    fn test_header_with_parent_id() {
        let parent_uuid = uuid::Uuid::new_v4();
        let event = Event {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 2,
                r#type: MessageType::Event as i32,
                parent_id: parent_uuid.as_bytes().to_vec(),
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::NodeStart as i32,
            node_id: "node1".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        };

        let header = event.header.unwrap();
        assert!(!header.parent_id.is_empty());
        assert_eq!(header.parent_id.len(), 16); // UUID is 16 bytes
    }

    #[test]
    fn test_header_sequence_numbering() {
        for seq in 0..100 {
            let event = Event {
                header: Some(Header {
                    message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                    timestamp_us: chrono::Utc::now().timestamp_micros(),
                    tenant_id: "test".to_string(),
                    thread_id: "test".to_string(),
                    sequence: seq,
                    r#type: MessageType::Event as i32,
                    parent_id: vec![],
                    compression: 0,
                    schema_version: 1,
                }),
                event_type: EventType::NodeStart as i32,
                node_id: format!("node-{}", seq),
                attributes: Default::default(),
                duration_us: 0,
                llm_request_id: "".to_string(),
            };

            let header = event.header.unwrap();
            assert_eq!(header.sequence, seq);
        }
    }

    #[test]
    fn test_header_tenant_ids() {
        let tenant_ids = vec!["tenant-1", "org-abc", "user-123", "test-tenant"];

        for tenant in tenant_ids {
            let event = Event {
                header: Some(Header {
                    message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                    timestamp_us: chrono::Utc::now().timestamp_micros(),
                    tenant_id: tenant.to_string(),
                    thread_id: "test".to_string(),
                    sequence: 1,
                    r#type: MessageType::Event as i32,
                    parent_id: vec![],
                    compression: 0,
                    schema_version: 1,
                }),
                event_type: EventType::GraphStart as i32,
                node_id: "start".to_string(),
                attributes: Default::default(),
                duration_us: 0,
                llm_request_id: "".to_string(),
            };

            let header = event.header.unwrap();
            assert_eq!(header.tenant_id, tenant);
        }
    }

    #[test]
    fn test_header_compression_flag() {
        // Uncompressed
        let event1 = Event {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "start".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        };
        assert_eq!(event1.header.unwrap().compression, 0);

        // Compressed
        let event2 = Event {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 1,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "start".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        };
        assert_eq!(event2.header.unwrap().compression, 1);
    }

    #[test]
    fn test_header_schema_version() {
        let event = Event {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test".to_string(),
                thread_id: "test".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "start".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        };

        assert_eq!(event.header.unwrap().schema_version, 1);
    }

    // === Sequence Number Tracking Tests ===

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_sequence_numbers_increment_per_thread() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        let producer = DashStreamProducer::new(&bootstrap_servers, "test-sequences")
            .await
            .expect("Failed to create producer");

        // Create headers for same thread
        let header1 = producer.create_header("thread-1", MessageType::Event);
        let header2 = producer.create_header("thread-1", MessageType::Event);
        let header3 = producer.create_header("thread-1", MessageType::Event);

        // Sequence numbers should increment: 1, 2, 3
        assert_eq!(header1.sequence, 1);
        assert_eq!(header2.sequence, 2);
        assert_eq!(header3.sequence, 3);
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_sequence_numbers_per_thread_isolation() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        let producer = DashStreamProducer::new(&bootstrap_servers, "test-sequences")
            .await
            .expect("Failed to create producer");

        // Create headers for different threads
        let header_a1 = producer.create_header("thread-a", MessageType::Event);
        let header_b1 = producer.create_header("thread-b", MessageType::Event);
        let header_a2 = producer.create_header("thread-a", MessageType::Event);
        let header_b2 = producer.create_header("thread-b", MessageType::Event);

        // Each thread has independent sequence counters
        assert_eq!(header_a1.sequence, 1);
        assert_eq!(header_b1.sequence, 1); // thread-b starts at 1
        assert_eq!(header_a2.sequence, 2); // thread-a continues from 1
        assert_eq!(header_b2.sequence, 2); // thread-b continues from 1
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_sequence_numbers_multiple_threads() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        let producer = DashStreamProducer::new(&bootstrap_servers, "test-sequences")
            .await
            .expect("Failed to create producer");

        // Simulate 3 threads each sending 5 messages
        for thread_num in 1..=3 {
            let thread_id = format!("thread-{}", thread_num);
            for seq in 1..=5 {
                let header = producer.create_header(&thread_id, MessageType::Event);
                assert_eq!(header.sequence, seq);
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_sequence_numbers_thread_id_format() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        let producer = DashStreamProducer::new(&bootstrap_servers, "test-sequences")
            .await
            .expect("Failed to create producer");

        // Test various thread ID formats
        let thread_ids = vec![
            "simple",
            "with-dashes",
            "with_underscores",
            "with.dots",
            "UUID-a1b2c3d4",
            "session:123:user:456",
        ];

        for thread_id in thread_ids {
            let h1 = producer.create_header(thread_id, MessageType::Event);
            let h2 = producer.create_header(thread_id, MessageType::Event);

            assert_eq!(h1.sequence, 1);
            assert_eq!(h2.sequence, 2);
        }
    }

    // === Tenant ID Configuration Tests ===

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_tenant_id_default() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Using new() should use default tenant
        let producer = DashStreamProducer::new(&bootstrap_servers, "test-tenant")
            .await
            .expect("Failed to create producer");

        let header = producer.create_header("thread-1", MessageType::Event);
        assert_eq!(header.tenant_id, "default");
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_tenant_id_custom() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Using new_with_tenant() should use custom tenant
        let producer =
            DashStreamProducer::new_with_tenant(&bootstrap_servers, "test-tenant", "customer-123")
                .await
                .expect("Failed to create producer");

        let header = producer.create_header("thread-1", MessageType::Event);
        assert_eq!(header.tenant_id, "customer-123");
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_tenant_id_multiple_tenants() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create producers for different tenants
        let producer_a =
            DashStreamProducer::new_with_tenant(&bootstrap_servers, "test-tenant", "tenant-a")
                .await
                .expect("Failed to create producer A");

        let producer_b =
            DashStreamProducer::new_with_tenant(&bootstrap_servers, "test-tenant", "tenant-b")
                .await
                .expect("Failed to create producer B");

        // Verify tenant isolation
        let header_a = producer_a.create_header("thread-1", MessageType::Event);
        let header_b = producer_b.create_header("thread-1", MessageType::Event);

        assert_eq!(header_a.tenant_id, "tenant-a");
        assert_eq!(header_b.tenant_id, "tenant-b");
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_tenant_id_formats() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Test various tenant ID formats
        let tenant_ids = vec![
            "simple",
            "with-dashes",
            "with_underscores",
            "with.dots",
            "org:123:user:456",
            "UUID-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
        ];

        for tenant_id in tenant_ids {
            let producer =
                DashStreamProducer::new_with_tenant(&bootstrap_servers, "test-tenant", tenant_id)
                    .await
                    .expect("Failed to create producer");

            let header = producer.create_header("thread-1", MessageType::Event);
            assert_eq!(header.tenant_id, tenant_id);
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_tenant_id_with_config() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create producer with custom config including tenant ID
        let config = ProducerConfig {
            bootstrap_servers: bootstrap_servers.clone(),
            topic: "test-tenant".to_string(),
            tenant_id: "config-tenant".to_string(),
            ..Default::default()
        };

        let producer = DashStreamProducer::with_config(config)
            .await
            .expect("Failed to create producer");

        let header = producer.create_header("thread-1", MessageType::Event);
        assert_eq!(header.tenant_id, "config-tenant");
    }

    #[test]
    fn test_producer_config_max_message_size() {
        let config = ProducerConfig {
            max_message_size: 2_097_152, // 2 MB
            ..Default::default()
        };
        assert_eq!(config.max_message_size, 2_097_152);

        // Default should be 1 MB
        let default_config = ProducerConfig::default();
        assert_eq!(default_config.max_message_size, 1_048_576);
    }
}
