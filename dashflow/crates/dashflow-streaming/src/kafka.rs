// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Kafka Topic Management Utilities
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

//! # Kafka Topic Management
//!
//! Utilities for managing Kafka topics for DashFlow Streaming.
//!
//! ## Features
//!
//! - **Topic Creation**: Create topics with custom configuration
//! - **Topic Deletion**: Delete topics (use with caution)
//! - **Topic Listing**: List all available topics
//! - **Topic Configuration**: Check and update topic settings
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_streaming::kafka::{create_topic, list_topics, TopicConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a topic
//!     let config = TopicConfig {
//!         num_partitions: 10,
//!         replication_factor: 3,
//!         ..Default::default()
//!     };
//!     create_topic("localhost:9092", "dashstream-events", config).await?;
//!
//!     // List topics
//!     let topics = list_topics("localhost:9092").await?;
//!     println!("Topics: {:?}", topics);
//!
//!     Ok(())
//! }
//! ```

use crate::errors::{Error, Result};
use crate::env_vars;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use std::env;
use std::time::Duration;

// =============================================================================
// Kafka Security Configuration (M-413: Unified across all Kafka clients)
// =============================================================================

/// Valid security protocols for Kafka connections
pub const VALID_SECURITY_PROTOCOLS: &[&str] = &["plaintext", "ssl", "sasl_plaintext", "sasl_ssl"];

/// Valid SASL mechanisms for Kafka authentication
pub const VALID_SASL_MECHANISMS: &[&str] =
    &["PLAIN", "SCRAM-SHA-256", "SCRAM-SHA-512", "GSSAPI", "OAUTHBEARER"];

/// Valid SSL endpoint identification algorithms
pub const VALID_SSL_ENDPOINT_ALGORITHMS: &[&str] = &["https", "none", ""];

/// Valid broker address family values for rdkafka
pub const VALID_BROKER_ADDRESS_FAMILIES: &[&str] = &["any", "v4", "v6"];

// =============================================================================
// Kafka Timeout Constants (M-618: Centralized timeout values)
// =============================================================================

/// Session timeout for metadata-only operations in milliseconds.
///
/// This is used for short-lived BaseConsumer instances that only fetch metadata
/// (topic listing, partition info, etc.) and don't join consumer groups.
///
/// 6 seconds is sufficient for metadata operations while keeping connections
/// short-lived. This is NOT the same as consumer group session timeout, which
/// should use `consumer::DEFAULT_SESSION_TIMEOUT_MS` (30 seconds).
pub const METADATA_SESSION_TIMEOUT_MS: &str = "6000";

/// Default timeout for Kafka admin operations (create/delete topic, etc.) in seconds.
///
/// This is the timeout for admin operations like creating and deleting topics.
/// Configurable via `KAFKA_OPERATION_TIMEOUT_SECS` env var.
pub const DEFAULT_OPERATION_TIMEOUT_SECS: u64 = 30;

/// Default timeout for Kafka metadata operations (fetch topic list, partition info) in seconds.
///
/// This is the timeout for metadata fetch operations like listing topics.
/// Configurable via `KAFKA_METADATA_TIMEOUT_SECS` env var.
pub const DEFAULT_METADATA_TIMEOUT_SECS: u64 = 30;

/// Get the Kafka admin operation timeout from environment or default.
///
/// # Environment Variables
///
/// | Variable | Description | Default |
/// |----------|-------------|---------|
/// | `KAFKA_OPERATION_TIMEOUT_SECS` | Timeout for admin operations (create/delete topic) | 30 |
///
/// # Example
///
/// ```rust
/// use dashflow_streaming::kafka::get_operation_timeout;
///
/// // Returns default (30s) or env var value
/// let timeout = get_operation_timeout();
/// assert!(timeout.as_secs() >= 1);
/// ```
#[must_use]
pub fn get_operation_timeout() -> Duration {
    Duration::from_secs(env_vars::env_u64_or_default(
        env_vars::KAFKA_OPERATION_TIMEOUT_SECS,
        DEFAULT_OPERATION_TIMEOUT_SECS,
    ))
}

/// Get the Kafka metadata fetch timeout from environment or default.
///
/// # Environment Variables
///
/// | Variable | Description | Default |
/// |----------|-------------|---------|
/// | `KAFKA_METADATA_TIMEOUT_SECS` | Timeout for metadata operations (list topics) | 30 |
///
/// # Example
///
/// ```rust
/// use dashflow_streaming::kafka::get_metadata_timeout;
///
/// // Returns default (30s) or env var value
/// let timeout = get_metadata_timeout();
/// assert!(timeout.as_secs() >= 1);
/// ```
#[must_use]
pub fn get_metadata_timeout() -> Duration {
    Duration::from_secs(env_vars::env_u64_or_default(
        env_vars::KAFKA_METADATA_TIMEOUT_SECS,
        DEFAULT_METADATA_TIMEOUT_SECS,
    ))
}

/// Get the broker address family based on environment and bootstrap servers.
///
/// # M-478: Configurable Address Family
///
/// This function determines the appropriate `broker.address.family` setting:
///
/// 1. If `KAFKA_BROKER_ADDRESS_FAMILY` env var is set, use that value (must be "any", "v4", or "v6")
/// 2. For localhost-like addresses (localhost, 127.0.0.1, ::1), default to "v4" to avoid
///    IPv6 resolution issues with Docker-advertised brokers
/// 3. For all other addresses, default to "any" to allow IPv6-only environments
///
/// # Environment Variables
///
/// | Variable | Description | Default |
/// |----------|-------------|---------|
/// | `KAFKA_BROKER_ADDRESS_FAMILY` | Force address family: any, v4, v6 | auto-detect |
///
/// # Examples
///
/// ```rust
/// use dashflow_streaming::kafka::get_broker_address_family;
///
/// // Localhost defaults to v4 for Docker compatibility
/// assert_eq!(get_broker_address_family("localhost:9092"), "v4");
/// assert_eq!(get_broker_address_family("127.0.0.1:9092"), "v4");
///
/// // Remote hosts allow both IPv4 and IPv6
/// assert_eq!(get_broker_address_family("kafka.example.com:9092"), "any");
///
/// // Multiple brokers: if any is localhost, use v4
/// assert_eq!(get_broker_address_family("localhost:9092,kafka.example.com:9092"), "v4");
/// ```
#[must_use]
pub fn get_broker_address_family(bootstrap_servers: &str) -> &'static str {
    // Check for explicit override first
    if let Ok(family) = env::var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY) {
        let family_lower = family.to_lowercase();
        if VALID_BROKER_ADDRESS_FAMILIES.contains(&family_lower.as_str()) {
            // Return static str for valid values
            return match family_lower.as_str() {
                "v4" => "v4",
                "v6" => "v6",
                _ => "any",
            };
        }
        // Invalid value - log warning and fall through to auto-detect
        tracing::warn!(
            value = %family,
            valid = ?VALID_BROKER_ADDRESS_FAMILIES,
            "Invalid KAFKA_BROKER_ADDRESS_FAMILY, using auto-detect"
        );
    }

    // Auto-detect based on bootstrap servers
    // If any broker looks like localhost, use v4 for Docker compatibility
    let is_localhost = bootstrap_servers
        .split(',')
        .any(|server| {
            let host = server.trim().split(':').next().unwrap_or("");
            host.eq_ignore_ascii_case("localhost")
                || host == "127.0.0.1"
                || host == "::1"
                || host.starts_with("127.")
        });

    if is_localhost {
        "v4"
    } else {
        "any"
    }
}

/// Unified Kafka security configuration.
///
/// This struct provides consistent security settings across all Kafka clients
/// (producer, consumer, admin client). Use `from_env()` to load from environment
/// variables for consistent configuration in production.
///
/// # Environment Variables
///
/// | Variable | Description | Default |
/// |----------|-------------|---------|
/// | `KAFKA_SECURITY_PROTOCOL` | Protocol: plaintext, ssl, sasl_plaintext, sasl_ssl | plaintext |
/// | `KAFKA_SASL_MECHANISM` | SASL mechanism: PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI, OAUTHBEARER | PLAIN |
/// | `KAFKA_SASL_USERNAME` | Username for SASL authentication | None |
/// | `KAFKA_SASL_PASSWORD` | Password for SASL authentication | None |
/// | `KAFKA_SSL_CA_LOCATION` | Path to CA certificate file | None |
/// | `KAFKA_SSL_CERTIFICATE_LOCATION` | Path to client certificate (mTLS) | None |
/// | `KAFKA_SSL_KEY_LOCATION` | Path to client private key (mTLS) | None |
/// | `KAFKA_SSL_KEY_PASSWORD` | Password for client private key | None |
/// | `KAFKA_SSL_ENDPOINT_ALGORITHM` | Hostname verification: https, none | https |
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::kafka::KafkaSecurityConfig;
///
/// // Load from environment
/// let security = KafkaSecurityConfig::from_env();
///
/// // Or configure manually
/// let security = KafkaSecurityConfig {
///     security_protocol: "sasl_ssl".to_string(),
///     sasl_mechanism: Some("SCRAM-SHA-256".to_string()),
///     sasl_username: Some("kafka-user".to_string()),
///     sasl_password: Some("kafka-password".to_string()),
///     ssl_ca_location: Some("/etc/kafka/ca.pem".to_string()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct KafkaSecurityConfig {
    /// Security protocol: plaintext, ssl, sasl_plaintext, sasl_ssl
    pub security_protocol: String,

    /// SASL mechanism for authentication
    pub sasl_mechanism: Option<String>,

    /// SASL username for PLAIN and SCRAM mechanisms
    pub sasl_username: Option<String>,

    /// SASL password for PLAIN and SCRAM mechanisms
    pub sasl_password: Option<String>,

    /// Path to CA certificate file for SSL/TLS
    pub ssl_ca_location: Option<String>,

    /// Path to client certificate file for mutual TLS (mTLS)
    pub ssl_certificate_location: Option<String>,

    /// Path to client private key file for mutual TLS (mTLS)
    pub ssl_key_location: Option<String>,

    /// Password for the client private key (if encrypted)
    pub ssl_key_password: Option<String>,

    /// SSL endpoint identification algorithm for hostname verification
    pub ssl_endpoint_identification_algorithm: Option<String>,
}

impl Default for KafkaSecurityConfig {
    fn default() -> Self {
        Self {
            security_protocol: "plaintext".to_string(),
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
            ssl_ca_location: None,
            ssl_certificate_location: None,
            ssl_key_location: None,
            ssl_key_password: None,
            ssl_endpoint_identification_algorithm: Some("https".to_string()),
        }
    }
}

impl KafkaSecurityConfig {
    /// Create a new plaintext (insecure) configuration for development.
    #[must_use]
    pub fn plaintext() -> Self {
        Self::default()
    }

    /// Create a TLS-only configuration (no SASL authentication).
    #[must_use]
    pub fn tls(ca_location: impl Into<String>) -> Self {
        Self {
            security_protocol: "ssl".to_string(),
            ssl_ca_location: Some(ca_location.into()),
            ..Default::default()
        }
    }

    /// Create a SASL + TLS configuration (recommended for production).
    #[must_use]
    pub fn sasl_ssl(
        mechanism: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        ca_location: Option<String>,
    ) -> Self {
        Self {
            security_protocol: "sasl_ssl".to_string(),
            sasl_mechanism: Some(mechanism.into()),
            sasl_username: Some(username.into()),
            sasl_password: Some(password.into()),
            ssl_ca_location: ca_location,
            ..Default::default()
        }
    }

    /// Load security configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        let security_protocol = env_vars::env_string_or_default(
            env_vars::KAFKA_SECURITY_PROTOCOL,
            "plaintext",
        );

        let sasl_mechanism = env_vars::env_string(env_vars::KAFKA_SASL_MECHANISM);
        let sasl_username = env_vars::env_string(env_vars::KAFKA_SASL_USERNAME);
        let sasl_password = env_vars::env_string(env_vars::KAFKA_SASL_PASSWORD);

        let ssl_ca_location = env_vars::env_string(env_vars::KAFKA_SSL_CA_LOCATION);
        let ssl_certificate_location =
            env_vars::env_string(env_vars::KAFKA_SSL_CERTIFICATE_LOCATION);
        let ssl_key_location = env_vars::env_string(env_vars::KAFKA_SSL_KEY_LOCATION);
        let ssl_key_password = env_vars::env_string(env_vars::KAFKA_SSL_KEY_PASSWORD);

        let ssl_endpoint_identification_algorithm = env_vars::env_string_or_default(
            env_vars::KAFKA_SSL_ENDPOINT_ALGORITHM,
            "https",
        );

        Self {
            security_protocol,
            sasl_mechanism,
            sasl_username,
            sasl_password,
            ssl_ca_location,
            ssl_certificate_location,
            ssl_key_location,
            ssl_key_password,
            ssl_endpoint_identification_algorithm: Some(ssl_endpoint_identification_algorithm),
        }
    }

    /// Validate the security configuration.
    pub fn validate(&self) -> Result<()> {
        if !VALID_SECURITY_PROTOCOLS.contains(&self.security_protocol.as_str()) {
            return Err(Error::Kafka(format!(
                "Invalid security_protocol '{}'; expected one of: {}",
                self.security_protocol,
                VALID_SECURITY_PROTOCOLS.join(", ")
            )));
        }

        if let Some(ref mechanism) = self.sasl_mechanism {
            if !VALID_SASL_MECHANISMS.contains(&mechanism.as_str()) {
                return Err(Error::Kafka(format!(
                    "Invalid sasl_mechanism '{}'; expected one of: {}",
                    mechanism,
                    VALID_SASL_MECHANISMS.join(", ")
                )));
            }
        }

        if self.sasl_username.is_some() != self.sasl_password.is_some() {
            return Err(Error::Kafka(
                "Both sasl_username and sasl_password must be set together".to_string(),
            ));
        }

        if self.ssl_certificate_location.is_some() != self.ssl_key_location.is_some() {
            return Err(Error::Kafka(
                "Both ssl_certificate_location and ssl_key_location must be set together"
                    .to_string(),
            ));
        }

        if let Some(ref algorithm) = self.ssl_endpoint_identification_algorithm {
            if !VALID_SSL_ENDPOINT_ALGORITHMS.contains(&algorithm.as_str()) {
                return Err(Error::Kafka(format!(
                    "Invalid ssl_endpoint_identification_algorithm '{}'; expected one of: https, none, \"\"",
                    algorithm
                )));
            }
        }

        Ok(())
    }

    /// Apply this security configuration to an rdkafka `ClientConfig`.
    pub fn apply_to_rdkafka(&self, client_config: &mut ClientConfig) {
        client_config.set("security.protocol", &self.security_protocol);

        if let Some(ref ca_location) = self.ssl_ca_location {
            client_config.set("ssl.ca.location", ca_location);
        }
        if let Some(ref cert_location) = self.ssl_certificate_location {
            client_config.set("ssl.certificate.location", cert_location);
        }
        if let Some(ref key_location) = self.ssl_key_location {
            client_config.set("ssl.key.location", key_location);
        }
        if let Some(ref key_password) = self.ssl_key_password {
            client_config.set("ssl.key.password", key_password);
        }

        if self.security_protocol.contains("ssl") {
            if let Some(ref algorithm) = self.ssl_endpoint_identification_algorithm {
                client_config.set("ssl.endpoint.identification.algorithm", algorithm);
            }
        }

        if let Some(ref mechanism) = self.sasl_mechanism {
            client_config.set("sasl.mechanism", mechanism);
        }
        if let Some(ref username) = self.sasl_username {
            client_config.set("sasl.username", username);
        }
        if let Some(ref password) = self.sasl_password {
            client_config.set("sasl.password", password);
        }
    }

    /// Check if TLS/SSL is enabled.
    #[must_use]
    pub fn is_tls_enabled(&self) -> bool {
        self.security_protocol.contains("ssl")
    }

    /// Check if SASL authentication is enabled.
    #[must_use]
    pub fn is_sasl_enabled(&self) -> bool {
        self.security_protocol.contains("sasl")
    }

    /// Check if this is a secure configuration (not plaintext).
    #[must_use]
    pub fn is_secure(&self) -> bool {
        self.security_protocol != "plaintext"
    }

    /// Create a base `ClientConfig` with standard settings and security applied.
    ///
    /// This applies:
    /// - Bootstrap servers
    /// - IPv4 address family (to avoid localhost IPv6 issues with Docker)
    /// - All security settings from this config
    ///
    /// # M-413: Unified Security Config
    ///
    /// Use this method to ensure consistent security settings across all Kafka clients.
    ///
    /// # Note
    ///
    /// This method does NOT validate the configuration. Use [`Self::create_client_config_checked`]
    /// if you want validation errors to surface immediately. Invalid configurations will
    /// cause cryptic errors when the client tries to connect.
    #[must_use]
    pub fn create_client_config(&self, bootstrap_servers: &str) -> ClientConfig {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", bootstrap_servers);
        // M-478: Use configurable address family instead of hardcoded v4
        config.set(
            "broker.address.family",
            get_broker_address_family(bootstrap_servers),
        );
        self.apply_to_rdkafka(&mut config);
        config
    }

    /// Create a base `ClientConfig` with validation.
    ///
    /// Same as [`Self::create_client_config`] but validates the security configuration first,
    /// returning a clear error if any env vars have invalid values.
    ///
    /// # M-474: Validated Config Creation
    ///
    /// Prefer this method over [`Self::create_client_config`] in production code to get
    /// early, actionable error messages instead of cryptic failures at connect time.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Bootstrap servers is empty
    /// - Security protocol is invalid (not one of: plaintext, ssl, sasl_plaintext, sasl_ssl)
    /// - SASL mechanism is invalid (not one of: PLAIN, SCRAM-SHA-256, SCRAM-SHA-512)
    /// - SASL username is set without password (or vice versa)
    /// - SSL certificate is set without key (or vice versa)
    /// - SSL endpoint identification algorithm is invalid
    pub fn create_client_config_checked(&self, bootstrap_servers: &str) -> Result<ClientConfig> {
        // M-474: Validate early for clear error messages
        if bootstrap_servers.trim().is_empty() {
            return Err(Error::Kafka("bootstrap_servers cannot be empty".to_string()));
        }
        self.validate()?;
        Ok(self.create_client_config(bootstrap_servers))
    }
}

// =============================================================================
// Topic Configuration
// =============================================================================

/// Valid cleanup policies for Kafka topics
pub const VALID_CLEANUP_POLICIES: &[&str] = &["delete", "compact", "compact,delete"];

/// Valid compression types for Kafka topics
pub const VALID_COMPRESSION_TYPES: &[&str] = &["none", "gzip", "snappy", "lz4", "zstd", "producer"];

/// Topic configuration
#[derive(Debug, Clone)]
pub struct TopicConfig {
    /// Number of partitions (must be >= 1)
    pub num_partitions: i32,

    /// Replication factor (must be >= 1)
    pub replication_factor: i32,

    /// Retention time in milliseconds (-1 = forever)
    pub retention_ms: i64,

    /// Segment size in bytes
    pub segment_bytes: i64,

    /// Cleanup policy: "delete", "compact", or "compact,delete"
    pub cleanup_policy: String,

    /// Compression type: "none", "gzip", "snappy", "lz4", "zstd", or "producer"
    pub compression_type: String,

    /// Minimum in-sync replicas (optional, for production use with replication_factor > 1)
    /// When set, producers with acks=all will wait for this many replicas to acknowledge.
    pub min_insync_replicas: Option<i32>,
}

impl Default for TopicConfig {
    fn default() -> Self {
        Self {
            num_partitions: 10,
            replication_factor: 1, // Default for single-node setup
            retention_ms: 7 * 24 * 60 * 60 * 1000, // 7 days
            segment_bytes: 1024 * 1024 * 1024, // 1 GB
            cleanup_policy: "delete".to_string(),
            compression_type: "zstd".to_string(),
            min_insync_replicas: None, // Not set by default
        }
    }
}

/// Validate bootstrap_servers is not empty
fn validate_bootstrap_servers(bootstrap_servers: &str) -> Result<()> {
    if bootstrap_servers.trim().is_empty() {
        return Err(Error::Kafka(
            "bootstrap_servers cannot be empty".to_string(),
        ));
    }
    Ok(())
}

/// Validate TopicConfig fields
fn validate_topic_config(config: &TopicConfig) -> Result<()> {
    // K-1: Validate num_partitions >= 1
    if config.num_partitions < 1 {
        return Err(Error::Kafka(format!(
            "num_partitions must be >= 1, got {}",
            config.num_partitions
        )));
    }

    // K-2: Validate replication_factor >= 1
    if config.replication_factor < 1 {
        return Err(Error::Kafka(format!(
            "replication_factor must be >= 1, got {}",
            config.replication_factor
        )));
    }

    // K-3: Validate cleanup_policy
    if !VALID_CLEANUP_POLICIES.contains(&config.cleanup_policy.as_str()) {
        return Err(Error::Kafka(format!(
            "invalid cleanup_policy '{}', must be one of: {}",
            config.cleanup_policy,
            VALID_CLEANUP_POLICIES.join(", ")
        )));
    }

    // K-4: Validate compression_type
    if !VALID_COMPRESSION_TYPES.contains(&config.compression_type.as_str()) {
        return Err(Error::Kafka(format!(
            "invalid compression_type '{}', must be one of: {}",
            config.compression_type,
            VALID_COMPRESSION_TYPES.join(", ")
        )));
    }

    // K-9: Validate min_insync_replicas if set
    if let Some(min_isr) = config.min_insync_replicas {
        if min_isr < 1 {
            return Err(Error::Kafka(format!(
                "min_insync_replicas must be >= 1, got {}",
                min_isr
            )));
        }
        if min_isr > config.replication_factor {
            return Err(Error::Kafka(format!(
                "min_insync_replicas ({}) cannot exceed replication_factor ({})",
                min_isr, config.replication_factor
            )));
        }
    }

    Ok(())
}

/// Maximum retry attempts for transient failures
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Base delay for exponential backoff (milliseconds)
const RETRY_BASE_DELAY_MS: u64 = 100;

/// Create a new Kafka topic with validation and retry logic
pub async fn create_topic(
    bootstrap_servers: &str,
    topic_name: &str,
    config: TopicConfig,
) -> Result<()> {
    // K-7: Validate bootstrap_servers
    validate_bootstrap_servers(bootstrap_servers)?;

    // K-1, K-2, K-3, K-4, K-9: Validate topic config
    validate_topic_config(&config)?;

    // K-10: Retry logic with exponential backoff
    let mut last_error = None;
    for attempt in 0..MAX_RETRY_ATTEMPTS {
        if attempt > 0 {
            // Exponential backoff: 100ms, 200ms, 400ms...
            let delay = RETRY_BASE_DELAY_MS * (1 << (attempt - 1));
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        match create_topic_inner(bootstrap_servers, topic_name, &config).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                // Check if error is retryable (transient)
                let err_str = e.to_string();
                let is_retryable = err_str.contains("timeout")
                    || err_str.contains("Timeout")
                    || err_str.contains("connection")
                    || err_str.contains("Connection")
                    || err_str.contains("temporarily")
                    || err_str.contains("LeaderNotAvailable")
                    || err_str.contains("NotLeaderForPartition");

                if !is_retryable || attempt == MAX_RETRY_ATTEMPTS - 1 {
                    return Err(e);
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| Error::Kafka("create_topic failed after retries".to_string())))
}

/// Internal topic creation (without retry)
async fn create_topic_inner(
    bootstrap_servers: &str,
    topic_name: &str,
    config: &TopicConfig,
) -> Result<()> {
    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let admin_client: AdminClient<DefaultClientContext> = security_config
        .create_client_config(bootstrap_servers)
        .create()
        .map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create admin client: {e}"
            )))
        })?;

    // Create temporary strings to avoid lifetime issues
    let retention_ms = config.retention_ms.to_string();
    let segment_bytes = config.segment_bytes.to_string();

    let mut topic = NewTopic::new(
        topic_name,
        config.num_partitions,
        TopicReplication::Fixed(config.replication_factor),
    );

    // Set topic configuration
    topic = topic
        .set("retention.ms", &retention_ms)
        .set("segment.bytes", &segment_bytes)
        .set("cleanup.policy", &config.cleanup_policy)
        .set("compression.type", &config.compression_type);

    // K-9: Apply min.insync.replicas if set
    let min_isr_str;
    if let Some(min_isr) = config.min_insync_replicas {
        min_isr_str = min_isr.to_string();
        topic = topic.set("min.insync.replicas", &min_isr_str);
    }

    // M-49: Use configurable operation timeout
    let opts = AdminOptions::new().operation_timeout(Some(get_operation_timeout()));

    let results = admin_client
        .create_topics(&[topic], &opts)
        .await
        .map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create topic: {e}"
            )))
        })?;

    for result in results {
        match result {
            Ok(_) => {}
            Err((topic, err)) => {
                // Ignore "already exists" error
                if !err.to_string().contains("already exists") {
                    return Err(Error::Io(std::io::Error::other(format!(
                        "Failed to create topic '{topic}': {err}"
                    ))));
                }
            }
        }
    }

    Ok(())
}

/// Delete a Kafka topic
///
/// Returns Ok(()) if the topic was deleted or if it didn't exist.
pub async fn delete_topic(bootstrap_servers: &str, topic_name: &str) -> Result<()> {
    // K-7: Validate bootstrap_servers
    validate_bootstrap_servers(bootstrap_servers)?;

    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let admin_client: AdminClient<DefaultClientContext> = security_config
        .create_client_config(bootstrap_servers)
        .create()
        .map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create admin client: {e}"
            )))
        })?;

    // M-49: Use configurable operation timeout
    let opts = AdminOptions::new().operation_timeout(Some(get_operation_timeout()));

    let results = admin_client
        .delete_topics(&[topic_name], &opts)
        .await
        .map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to delete topic: {e}"
            )))
        })?;

    for result in results {
        match result {
            Ok(_) => {}
            Err((topic, err)) => {
                let err_str = err.to_string();
                // K-5: Gracefully handle "topic doesn't exist" errors
                // These are not failures - the desired end state (topic gone) is achieved
                if err_str.contains("Unknown topic")
                    || err_str.contains("does not exist")
                    || err_str.contains("UnknownTopicOrPartition")
                {
                    // Topic doesn't exist, which is fine - idempotent delete
                    continue;
                }
                return Err(Error::Io(std::io::Error::other(format!(
                    "Failed to delete topic '{topic}': {err}"
                ))));
            }
        }
    }

    Ok(())
}

/// List all Kafka topics
pub async fn list_topics(bootstrap_servers: &str) -> Result<Vec<String>> {
    // K-7: Validate bootstrap_servers
    validate_bootstrap_servers(bootstrap_servers)?;

    let bootstrap_servers = bootstrap_servers.to_string();
    tokio::task::spawn_blocking(move || {
        // M-413: Apply security config from environment
        let security_config = KafkaSecurityConfig::from_env();
        let mut client_config = security_config.create_client_config(&bootstrap_servers);
        client_config.set("session.timeout.ms", METADATA_SESSION_TIMEOUT_MS);
        let consumer: BaseConsumer = client_config.create().map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create consumer: {e}"
            )))
        })?;

        // M-49: Use configurable metadata timeout
        let metadata = consumer
            .fetch_metadata(None, get_metadata_timeout())
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to fetch metadata: {e}"
                )))
            })?;

        let topics: Vec<String> = metadata
            .topics()
            .iter()
            .map(|t| t.name().to_string())
            .collect();

        Ok(topics)
    })
    .await
    .map_err(|e| {
        Error::Io(std::io::Error::other(format!(
            "list_topics join error: {e}"
        )))
    })?
}

/// Check if a topic exists
///
/// K-6: Optimized to use O(1) metadata lookup instead of fetching all topics.
/// Uses `fetch_metadata(Some(topic_name), ...)` for efficient single-topic check.
pub async fn topic_exists(bootstrap_servers: &str, topic_name: &str) -> Result<bool> {
    // K-7: Validate bootstrap_servers
    validate_bootstrap_servers(bootstrap_servers)?;

    let bootstrap_servers = bootstrap_servers.to_string();
    let topic_name = topic_name.to_string();

    tokio::task::spawn_blocking(move || {
        // M-413: Apply security config from environment
        let security_config = KafkaSecurityConfig::from_env();
        let mut client_config = security_config.create_client_config(&bootstrap_servers);
        client_config.set("session.timeout.ms", METADATA_SESSION_TIMEOUT_MS);
        let consumer: BaseConsumer = client_config.create().map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create consumer: {e}"
            )))
        })?;

        // K-6: Fetch metadata for specific topic only (O(1) instead of O(n))
        // M-49: Use configurable metadata timeout
        let metadata = consumer
            .fetch_metadata(Some(&topic_name), get_metadata_timeout())
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to fetch metadata: {e}"
                )))
            })?;

        // If the topic doesn't exist, Kafka still returns metadata but with error
        // We check if any topic in the result matches our name and has no error
        let exists = metadata.topics().iter().any(|t| {
            t.name() == topic_name && t.error().is_none()
        });

        Ok(exists)
    })
    .await
    .map_err(|e| {
        Error::Io(std::io::Error::other(format!(
            "topic_exists join error: {e}"
        )))
    })?
}

/// Get the number of partitions for a topic
///
/// S-2: Returns the partition count for a given topic, enabling multi-partition consumers
/// to spawn one consumer per partition.
///
/// # Errors
/// Returns an error if the topic doesn't exist or if metadata fetch fails.
pub async fn get_partition_count(bootstrap_servers: &str, topic_name: &str) -> Result<i32> {
    validate_bootstrap_servers(bootstrap_servers)?;

    let bootstrap_servers = bootstrap_servers.to_string();
    let topic_name = topic_name.to_string();

    tokio::task::spawn_blocking(move || {
        // M-413: Apply security config from environment
        let security_config = KafkaSecurityConfig::from_env();
        let mut client_config = security_config.create_client_config(&bootstrap_servers);
        client_config.set("session.timeout.ms", METADATA_SESSION_TIMEOUT_MS);
        let consumer: BaseConsumer = client_config.create().map_err(|e| {
            Error::Io(std::io::Error::other(format!(
                "Failed to create consumer: {e}"
            )))
        })?;

        // M-49: Use configurable metadata timeout
        let metadata = consumer
            .fetch_metadata(Some(&topic_name), get_metadata_timeout())
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!(
                    "Failed to fetch metadata: {e}"
                )))
            })?;

        // Find the topic in metadata
        for topic in metadata.topics() {
            if topic.name() == topic_name {
                if let Some(err) = topic.error() {
                    return Err(Error::Kafka(format!(
                        "Topic '{}' has error: {:?}",
                        topic_name, err
                    )));
                }
                return Ok(topic.partitions().len() as i32);
            }
        }

        Err(Error::Kafka(format!("Topic '{}' not found", topic_name)))
    })
    .await
    .map_err(|e| {
        Error::Io(std::io::Error::other(format!(
            "get_partition_count join error: {e}"
        )))
    })?
}

/// Get recommended topic configuration for DashFlow Streaming
#[must_use]
pub fn recommended_config() -> TopicConfig {
    TopicConfig {
        num_partitions: 10,
        replication_factor: 3,                 // For production with 3+ brokers
        retention_ms: 7 * 24 * 60 * 60 * 1000, // 7 days
        segment_bytes: 1024 * 1024 * 1024,     // 1 GB
        cleanup_policy: "delete".to_string(),
        compression_type: "zstd".to_string(),
        min_insync_replicas: Some(2),          // K-9: Require 2 replicas for durability
    }
}

/// Get development topic configuration for DashFlow Streaming
#[must_use]
pub fn dev_config() -> TopicConfig {
    TopicConfig {
        num_partitions: 3,
        replication_factor: 1,             // Single broker
        retention_ms: 24 * 60 * 60 * 1000, // 1 day
        segment_bytes: 256 * 1024 * 1024,  // 256 MB
        cleanup_policy: "delete".to_string(),
        compression_type: "producer".to_string(), // Use producer's compression setting (none)
        min_insync_replicas: None,         // Not needed for single broker
    }
}

/// Get topic configuration for Dead Letter Queue (DLQ) topics
///
/// DLQ topics have different requirements than main topics:
/// - Longer retention (30 days) for forensic analysis of failed messages
/// - Fewer partitions (3) since DLQ volume is typically much lower
/// - zstd compression for space efficiency
///
/// # M-410: Topic Provisioning
///
/// Use this with [`ensure_topics_with_dlq`] to provision main + DLQ topics together.
#[must_use]
pub fn dlq_config() -> TopicConfig {
    TopicConfig {
        num_partitions: 3,
        replication_factor: 1,                  // Override for production
        retention_ms: 30 * 24 * 60 * 60 * 1000, // 30 days for forensic analysis
        segment_bytes: 256 * 1024 * 1024,       // 256 MB (lower volume)
        cleanup_policy: "delete".to_string(),
        compression_type: "zstd".to_string(),   // Space efficiency for long retention
        min_insync_replicas: None,
    }
}

/// Ensure a topic exists, creating it if necessary
///
/// This is an idempotent operation - calling it multiple times is safe.
///
/// # Arguments
///
/// * `bootstrap_servers` - Kafka broker addresses
/// * `topic_name` - Name of the topic to ensure exists
/// * `config` - Configuration to use if creating the topic
///
/// # Returns
///
/// * `Ok(true)` - Topic was created
/// * `Ok(false)` - Topic already existed
/// * `Err(_)` - Failed to check or create topic
///
/// # M-410: Topic Provisioning
///
/// Use this instead of relying on Kafka auto-create, which may create topics
/// with incorrect partition counts and retention settings.
pub async fn ensure_topic_exists(
    bootstrap_servers: &str,
    topic_name: &str,
    config: TopicConfig,
) -> Result<bool> {
    // Check if topic already exists
    if topic_exists(bootstrap_servers, topic_name).await? {
        return Ok(false);
    }

    // Topic doesn't exist, create it
    create_topic(bootstrap_servers, topic_name, config).await?;
    Ok(true)
}

/// Ensure both a main topic and its DLQ topic exist
///
/// Creates `{topic_name}` and `{topic_name}-dlq` with appropriate configurations.
/// This is idempotent - safe to call multiple times.
///
/// # Arguments
///
/// * `bootstrap_servers` - Kafka broker addresses
/// * `topic_name` - Base name for the topic (DLQ will be `{topic_name}-dlq`)
/// * `main_config` - Configuration for the main topic
/// * `dlq_config` - Configuration for the DLQ topic
///
/// # Returns
///
/// * `Ok((main_created, dlq_created))` - Tuple of bools indicating if each was created
/// * `Err(_)` - Failed to check or create topics
///
/// # Example
///
/// ```no_run
/// use dashflow_streaming::kafka::{ensure_topics_with_dlq, recommended_config, dlq_config};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let (main_created, dlq_created) = ensure_topics_with_dlq(
///     "localhost:9092",
///     "my-events",
///     recommended_config(),
///     dlq_config(),
/// ).await?;
///
/// if main_created {
///     println!("Created my-events topic");
/// }
/// if dlq_created {
///     println!("Created my-events-dlq topic");
/// }
/// # Ok(())
/// # }
/// ```
///
/// # M-410: Topic Provisioning
///
/// This ensures production deployments have topics with correct settings,
/// rather than relying on Kafka auto-create which may use defaults.
pub async fn ensure_topics_with_dlq(
    bootstrap_servers: &str,
    topic_name: &str,
    main_config: TopicConfig,
    dlq_config: TopicConfig,
) -> Result<(bool, bool)> {
    let dlq_topic_name = format!("{topic_name}-dlq");

    let main_created = ensure_topic_exists(bootstrap_servers, topic_name, main_config).await?;
    let dlq_created = ensure_topic_exists(bootstrap_servers, &dlq_topic_name, dlq_config).await?;

    Ok((main_created, dlq_created))
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // TopicConfig Tests
    // ============================================================================

    #[test]
    fn test_topic_config_default() {
        let config = TopicConfig::default();
        assert_eq!(config.num_partitions, 10);
        assert_eq!(config.replication_factor, 1);
        assert_eq!(config.retention_ms, 7 * 24 * 60 * 60 * 1000); // 7 days
        assert_eq!(config.segment_bytes, 1024 * 1024 * 1024); // 1 GB
        assert_eq!(config.cleanup_policy, "delete");
        assert_eq!(config.compression_type, "zstd");
        assert_eq!(config.min_insync_replicas, None);
    }

    #[test]
    fn test_topic_config_custom() {
        let config = TopicConfig {
            num_partitions: 20,
            replication_factor: 5,
            retention_ms: 30 * 24 * 60 * 60 * 1000, // 30 days
            segment_bytes: 2 * 1024 * 1024 * 1024,  // 2 GB
            cleanup_policy: "compact".to_string(),
            compression_type: "lz4".to_string(),
            min_insync_replicas: Some(3),
        };
        assert_eq!(config.num_partitions, 20);
        assert_eq!(config.replication_factor, 5);
        assert_eq!(config.retention_ms, 30 * 24 * 60 * 60 * 1000);
        assert_eq!(config.segment_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(config.cleanup_policy, "compact");
        assert_eq!(config.compression_type, "lz4");
        assert_eq!(config.min_insync_replicas, Some(3));
    }

    #[test]
    fn test_topic_config_clone() {
        let config1 = TopicConfig::default();
        let config2 = config1.clone();
        assert_eq!(config1.num_partitions, config2.num_partitions);
        assert_eq!(config1.replication_factor, config2.replication_factor);
        assert_eq!(config1.retention_ms, config2.retention_ms);
        assert_eq!(config1.segment_bytes, config2.segment_bytes);
        assert_eq!(config1.cleanup_policy, config2.cleanup_policy);
        assert_eq!(config1.compression_type, config2.compression_type);
        assert_eq!(config1.min_insync_replicas, config2.min_insync_replicas);
    }

    #[test]
    fn test_topic_config_debug() {
        let config = TopicConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("TopicConfig"));
        assert!(debug_str.contains("num_partitions"));
        assert!(debug_str.contains("10"));
    }

    #[test]
    fn test_topic_config_partition_variants() {
        let single = TopicConfig {
            num_partitions: 1,
            ..Default::default()
        };
        assert_eq!(single.num_partitions, 1);

        let small = TopicConfig {
            num_partitions: 3,
            ..Default::default()
        };
        assert_eq!(small.num_partitions, 3);

        let medium = TopicConfig {
            num_partitions: 10,
            ..Default::default()
        };
        assert_eq!(medium.num_partitions, 10);

        let large = TopicConfig {
            num_partitions: 100,
            ..Default::default()
        };
        assert_eq!(large.num_partitions, 100);
    }

    #[test]
    fn test_topic_config_replication_variants() {
        let none = TopicConfig {
            replication_factor: 1,
            ..Default::default()
        };
        assert_eq!(none.replication_factor, 1);

        let two = TopicConfig {
            replication_factor: 2,
            ..Default::default()
        };
        assert_eq!(two.replication_factor, 2);

        let three = TopicConfig {
            replication_factor: 3,
            ..Default::default()
        };
        assert_eq!(three.replication_factor, 3);

        let five = TopicConfig {
            replication_factor: 5,
            ..Default::default()
        };
        assert_eq!(five.replication_factor, 5);
    }

    #[test]
    fn test_topic_config_retention_variants() {
        // 1 hour
        let one_hour = TopicConfig {
            retention_ms: 60 * 60 * 1000,
            ..Default::default()
        };
        assert_eq!(one_hour.retention_ms, 60 * 60 * 1000);

        // 1 day
        let one_day = TopicConfig {
            retention_ms: 24 * 60 * 60 * 1000,
            ..Default::default()
        };
        assert_eq!(one_day.retention_ms, 24 * 60 * 60 * 1000);

        // 7 days (default)
        let seven_days = TopicConfig {
            retention_ms: 7 * 24 * 60 * 60 * 1000,
            ..Default::default()
        };
        assert_eq!(seven_days.retention_ms, 7 * 24 * 60 * 60 * 1000);

        // 30 days
        let thirty_days = TopicConfig {
            retention_ms: 30 * 24 * 60 * 60 * 1000,
            ..Default::default()
        };
        assert_eq!(thirty_days.retention_ms, 30 * 24 * 60 * 60 * 1000);

        // Forever
        let forever = TopicConfig {
            retention_ms: -1,
            ..Default::default()
        };
        assert_eq!(forever.retention_ms, -1);
    }

    #[test]
    fn test_topic_config_segment_size_variants() {
        // 100 MB
        let small = TopicConfig {
            segment_bytes: 100 * 1024 * 1024,
            ..Default::default()
        };
        assert_eq!(small.segment_bytes, 100 * 1024 * 1024);

        // 256 MB
        let medium = TopicConfig {
            segment_bytes: 256 * 1024 * 1024,
            ..Default::default()
        };
        assert_eq!(medium.segment_bytes, 256 * 1024 * 1024);

        // 1 GB (default)
        let large = TopicConfig {
            segment_bytes: 1024 * 1024 * 1024,
            ..Default::default()
        };
        assert_eq!(large.segment_bytes, 1024 * 1024 * 1024);

        // 2 GB
        let xlarge = TopicConfig {
            segment_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        };
        assert_eq!(xlarge.segment_bytes, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_topic_config_cleanup_policy_variants() {
        // Delete policy (default)
        let delete = TopicConfig {
            cleanup_policy: "delete".to_string(),
            ..Default::default()
        };
        assert_eq!(delete.cleanup_policy, "delete");

        // Compact policy
        let compact = TopicConfig {
            cleanup_policy: "compact".to_string(),
            ..Default::default()
        };
        assert_eq!(compact.cleanup_policy, "compact");

        // Delete + compact
        let both = TopicConfig {
            cleanup_policy: "compact,delete".to_string(),
            ..Default::default()
        };
        assert_eq!(both.cleanup_policy, "compact,delete");
    }

    #[test]
    fn test_topic_config_compression_type_variants() {
        let compression_types = vec!["none", "gzip", "snappy", "lz4", "zstd"];

        for comp_type in compression_types {
            let config = TopicConfig {
                compression_type: comp_type.to_string(),
                ..Default::default()
            };
            assert_eq!(config.compression_type, comp_type);
        }
    }

    // ============================================================================
    // Recommended Configuration Tests
    // ============================================================================

    #[test]
    fn test_recommended_config() {
        let config = recommended_config();
        assert_eq!(config.num_partitions, 10);
        assert_eq!(config.replication_factor, 3);
        assert_eq!(config.retention_ms, 7 * 24 * 60 * 60 * 1000); // 7 days
        assert_eq!(config.segment_bytes, 1024 * 1024 * 1024); // 1 GB
        assert_eq!(config.cleanup_policy, "delete");
        assert_eq!(config.compression_type, "zstd");
        assert_eq!(config.min_insync_replicas, Some(2)); // K-9: Production durability
    }

    #[test]
    fn test_recommended_config_for_production() {
        let config = recommended_config();
        // Production config should have replication for fault tolerance
        assert!(config.replication_factor >= 3);
        // Should have sufficient partitions for parallelism
        assert!(config.num_partitions >= 10);
        // Should use efficient compression
        assert_eq!(config.compression_type, "zstd");
    }

    #[test]
    fn test_dev_config() {
        let config = dev_config();
        assert_eq!(config.num_partitions, 3);
        assert_eq!(config.replication_factor, 1);
        assert_eq!(config.retention_ms, 24 * 60 * 60 * 1000); // 1 day
        assert_eq!(config.segment_bytes, 256 * 1024 * 1024); // 256 MB
        assert_eq!(config.cleanup_policy, "delete");
        assert_eq!(config.compression_type, "producer"); // Use producer's compression
        assert_eq!(config.min_insync_replicas, None); // Not needed for single broker
    }

    #[test]
    fn test_dev_config_for_single_broker() {
        let config = dev_config();
        // Dev config should work with single broker
        assert_eq!(config.replication_factor, 1);
        // Fewer partitions for simplicity
        assert!(config.num_partitions < 10);
        // Shorter retention for development
        assert!(config.retention_ms < 7 * 24 * 60 * 60 * 1000);
        // Smaller segments
        assert!(config.segment_bytes < 1024 * 1024 * 1024);
    }

    #[test]
    fn test_dev_vs_recommended_config() {
        let dev = dev_config();
        let prod = recommended_config();

        // Production should have more partitions
        assert!(prod.num_partitions > dev.num_partitions);

        // Production should have higher replication
        assert!(prod.replication_factor > dev.replication_factor);

        // Production should have longer retention
        assert!(prod.retention_ms > dev.retention_ms);

        // Production should have larger segments
        assert!(prod.segment_bytes > dev.segment_bytes);
    }

    // ============================================================================
    // Configuration Builder Pattern Tests
    // ============================================================================

    #[test]
    fn test_topic_config_builder_pattern() {
        let config = TopicConfig {
            num_partitions: 20,
            replication_factor: 5,
            compression_type: "lz4".to_string(),
            ..Default::default()
        };

        assert_eq!(config.num_partitions, 20);
        assert_eq!(config.replication_factor, 5);
        assert_eq!(config.compression_type, "lz4");
    }

    #[test]
    fn test_topic_config_partial_override() {
        let config = TopicConfig {
            num_partitions: 50,
            compression_type: "snappy".to_string(),
            ..Default::default()
        };

        // Overridden fields
        assert_eq!(config.num_partitions, 50);
        assert_eq!(config.compression_type, "snappy");

        // Default fields preserved
        assert_eq!(config.replication_factor, 1);
        assert_eq!(config.retention_ms, 7 * 24 * 60 * 60 * 1000);
        assert_eq!(config.cleanup_policy, "delete");
    }

    #[test]
    fn test_topic_config_high_throughput() {
        // Configuration for high-throughput scenario
        let config = TopicConfig {
            num_partitions: 100,
            replication_factor: 3,
            segment_bytes: 2 * 1024 * 1024 * 1024, // 2 GB
            compression_type: "lz4".to_string(),   // Fast compression
            ..Default::default()
        };

        assert_eq!(config.num_partitions, 100);
        assert_eq!(config.compression_type, "lz4");
        assert!(config.segment_bytes > 1024 * 1024 * 1024);
    }

    #[test]
    fn test_topic_config_log_compaction() {
        // Configuration for log compaction (changelog topic)
        let config = TopicConfig {
            cleanup_policy: "compact".to_string(),
            segment_bytes: 512 * 1024 * 1024, // Smaller segments for more frequent compaction
            retention_ms: -1,                 // Keep forever (rely on compaction)
            ..Default::default()
        };

        assert_eq!(config.cleanup_policy, "compact");
        assert_eq!(config.retention_ms, -1);
    }

    #[test]
    fn test_topic_config_time_series_data() {
        // Configuration for time-series data
        let config = TopicConfig {
            retention_ms: 90 * 24 * 60 * 60 * 1000, // 90 days
            cleanup_policy: "delete".to_string(),
            compression_type: "zstd".to_string(), // Best compression for historical data
            ..Default::default()
        };

        assert_eq!(config.retention_ms, 90 * 24 * 60 * 60 * 1000);
        assert_eq!(config.compression_type, "zstd");
    }

    // ============================================================================
    // Validation Tests (K-1 through K-9)
    // ============================================================================

    #[test]
    fn test_validate_bootstrap_servers_empty() {
        // K-7: Empty bootstrap_servers should fail
        let result = validate_bootstrap_servers("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_bootstrap_servers_whitespace() {
        // K-7: Whitespace-only bootstrap_servers should fail
        let result = validate_bootstrap_servers("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_bootstrap_servers_valid() {
        // K-7: Valid bootstrap_servers should pass
        assert!(validate_bootstrap_servers("localhost:9092").is_ok());
        assert!(validate_bootstrap_servers("broker1:9092,broker2:9092").is_ok());
    }

    #[test]
    fn test_validate_topic_config_num_partitions_zero() {
        // K-1: num_partitions = 0 should fail
        let config = TopicConfig {
            num_partitions: 0,
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("num_partitions"));
    }

    #[test]
    fn test_validate_topic_config_num_partitions_negative() {
        // K-1: negative num_partitions should fail
        let config = TopicConfig {
            num_partitions: -5,
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_topic_config_num_partitions_valid() {
        // K-1: positive num_partitions should pass
        let config = TopicConfig {
            num_partitions: 1,
            ..Default::default()
        };
        assert!(validate_topic_config(&config).is_ok());
    }

    #[test]
    fn test_validate_topic_config_replication_factor_zero() {
        // K-2: replication_factor = 0 should fail
        let config = TopicConfig {
            replication_factor: 0,
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("replication_factor"));
    }

    #[test]
    fn test_validate_topic_config_replication_factor_negative() {
        // K-2: negative replication_factor should fail
        let config = TopicConfig {
            replication_factor: -1,
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_topic_config_cleanup_policy_invalid() {
        // K-3: invalid cleanup_policy should fail
        let config = TopicConfig {
            cleanup_policy: "invalid".to_string(),
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cleanup_policy"));
    }

    #[test]
    fn test_validate_topic_config_cleanup_policy_valid() {
        // K-3: all valid cleanup policies should pass
        for policy in VALID_CLEANUP_POLICIES {
            let config = TopicConfig {
                cleanup_policy: policy.to_string(),
                ..Default::default()
            };
            assert!(validate_topic_config(&config).is_ok(), "Policy '{}' should be valid", policy);
        }
    }

    #[test]
    fn test_validate_topic_config_compression_type_invalid() {
        // K-4: invalid compression_type should fail
        let config = TopicConfig {
            compression_type: "brotli".to_string(), // Not supported by Kafka
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("compression_type"));
    }

    #[test]
    fn test_validate_topic_config_compression_type_valid() {
        // K-4: all valid compression types should pass
        for comp_type in VALID_COMPRESSION_TYPES {
            let config = TopicConfig {
                compression_type: comp_type.to_string(),
                ..Default::default()
            };
            assert!(validate_topic_config(&config).is_ok(), "Compression type '{}' should be valid", comp_type);
        }
    }

    #[test]
    fn test_validate_topic_config_min_insync_replicas_zero() {
        // K-9: min_insync_replicas = 0 should fail
        let config = TopicConfig {
            replication_factor: 3,
            min_insync_replicas: Some(0),
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("min_insync_replicas"));
    }

    #[test]
    fn test_validate_topic_config_min_insync_replicas_exceeds_replication() {
        // K-9: min_insync_replicas > replication_factor should fail
        let config = TopicConfig {
            replication_factor: 2,
            min_insync_replicas: Some(3),
            ..Default::default()
        };
        let result = validate_topic_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot exceed"));
    }

    #[test]
    fn test_validate_topic_config_min_insync_replicas_valid() {
        // K-9: valid min_insync_replicas should pass
        let config = TopicConfig {
            replication_factor: 3,
            min_insync_replicas: Some(2),
            ..Default::default()
        };
        assert!(validate_topic_config(&config).is_ok());
    }

    #[test]
    fn test_validate_topic_config_min_insync_replicas_none() {
        // K-9: None min_insync_replicas should pass
        let config = TopicConfig::default();
        assert!(validate_topic_config(&config).is_ok());
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_valid_constants() {
        // Verify constants are non-empty
        assert!(!VALID_CLEANUP_POLICIES.is_empty());
        assert!(!VALID_COMPRESSION_TYPES.is_empty());
        // Verify expected values are present
        assert!(VALID_CLEANUP_POLICIES.contains(&"delete"));
        assert!(VALID_CLEANUP_POLICIES.contains(&"compact"));
        assert!(VALID_COMPRESSION_TYPES.contains(&"zstd"));
        assert!(VALID_COMPRESSION_TYPES.contains(&"producer"));
    }

    // ============================================================================
    // Integration Tests (Require Kafka)
    // ============================================================================

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_create_topic() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        let config = dev_config();
        let result = create_topic(&bootstrap_servers, "test-topic-create", config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_list_topics() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create a topic first so list_topics has something to return
        let config = dev_config();
        create_topic(&bootstrap_servers, "test-list-topics", config)
            .await
            .expect("Failed to create topic");

        let topics = list_topics(&bootstrap_servers)
            .await
            .expect("Failed to list topics");
        println!("Topics: {:?}", topics);
        assert!(!topics.is_empty());
        assert!(topics.contains(&"test-list-topics".to_string()));
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_topic_exists() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        let config = dev_config();
        create_topic(&bootstrap_servers, "test-topic-exists", config)
            .await
            .expect("Failed to create topic");

        let exists = topic_exists(&bootstrap_servers, "test-topic-exists")
            .await
            .expect("Failed to check topic");
        assert!(exists);

        let not_exists = topic_exists(&bootstrap_servers, "nonexistent-topic")
            .await
            .expect("Failed to check topic");
        assert!(!not_exists);
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_delete_topic() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        let config = dev_config();
        create_topic(&bootstrap_servers, "test-topic-delete", config)
            .await
            .expect("Failed to create topic");

        let result = delete_topic(&bootstrap_servers, "test-topic-delete").await;
        assert!(result.is_ok());

        // Verify deletion
        tokio::time::sleep(Duration::from_secs(2)).await;
        let exists = topic_exists(&bootstrap_servers, "test-topic-delete")
            .await
            .expect("Failed to check topic");
        assert!(!exists);
    }

    // ============================================================================
    // Topic Provisioning Tests (M-410)
    // ============================================================================

    #[test]
    fn test_dlq_config() {
        // M-410: DLQ config should have appropriate defaults for forensic analysis
        let config = dlq_config();

        // DLQ topics need longer retention for forensic analysis
        assert_eq!(config.retention_ms, 30 * 24 * 60 * 60 * 1000); // 30 days
        // DLQ topics should use zstd compression for space efficiency
        assert_eq!(config.compression_type, "zstd");
        // DLQ topics typically have fewer partitions (lower volume)
        assert_eq!(config.num_partitions, 3);
        // DLQ topics use delete cleanup policy
        assert_eq!(config.cleanup_policy, "delete");
    }

    #[test]
    fn test_dlq_config_validates() {
        // M-410: DLQ config should pass validation
        let config = dlq_config();
        assert!(validate_topic_config(&config).is_ok());
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_ensure_topic_exists_creates_new() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Topic shouldn't exist yet
        let exists_before = topic_exists(&bootstrap_servers, "test-ensure-new")
            .await
            .expect("Failed to check topic");
        assert!(!exists_before);

        // ensure_topic_exists should create it and return true
        let created = ensure_topic_exists(&bootstrap_servers, "test-ensure-new", dev_config())
            .await
            .expect("Failed to ensure topic");
        assert!(created, "Should return true when topic is created");

        // Topic should now exist
        let exists_after = topic_exists(&bootstrap_servers, "test-ensure-new")
            .await
            .expect("Failed to check topic");
        assert!(exists_after);
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_ensure_topic_exists_already_exists() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Create topic first
        create_topic(&bootstrap_servers, "test-ensure-existing", dev_config())
            .await
            .expect("Failed to create topic");

        // ensure_topic_exists should return false (didn't create)
        let created = ensure_topic_exists(&bootstrap_servers, "test-ensure-existing", dev_config())
            .await
            .expect("Failed to ensure topic");
        assert!(!created, "Should return false when topic already exists");
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_ensure_topics_with_dlq() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Neither topic should exist yet
        let main_exists = topic_exists(&bootstrap_servers, "test-with-dlq")
            .await
            .expect("Failed to check topic");
        let dlq_exists = topic_exists(&bootstrap_servers, "test-with-dlq-dlq")
            .await
            .expect("Failed to check topic");
        assert!(!main_exists);
        assert!(!dlq_exists);

        // Create both topics
        let (main_created, dlq_created) = ensure_topics_with_dlq(
            &bootstrap_servers,
            "test-with-dlq",
            recommended_config(),
            dlq_config(),
        )
        .await
        .expect("Failed to ensure topics");

        assert!(main_created, "Main topic should be created");
        assert!(dlq_created, "DLQ topic should be created");

        // Both topics should now exist
        let main_exists = topic_exists(&bootstrap_servers, "test-with-dlq")
            .await
            .expect("Failed to check topic");
        let dlq_exists = topic_exists(&bootstrap_servers, "test-with-dlq-dlq")
            .await
            .expect("Failed to check topic");
        assert!(main_exists, "Main topic should exist");
        assert!(dlq_exists, "DLQ topic should exist");
    }

    #[tokio::test]
    #[ignore = "requires Docker for testcontainers"]
    async fn test_ensure_topics_with_dlq_idempotent() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::kafka::apache;

        let kafka = apache::Kafka::default().start().await.unwrap();
        let bootstrap_servers = format!(
            "127.0.0.1:{}",
            kafka.get_host_port_ipv4(apache::KAFKA_PORT).await.unwrap()
        );
        tokio::time::sleep(Duration::from_secs(3)).await;

        // First call creates both
        let (main_created1, dlq_created1) = ensure_topics_with_dlq(
            &bootstrap_servers,
            "test-idempotent",
            recommended_config(),
            dlq_config(),
        )
        .await
        .expect("Failed to ensure topics");
        assert!(main_created1);
        assert!(dlq_created1);

        // Second call should be idempotent (return false for both)
        let (main_created2, dlq_created2) = ensure_topics_with_dlq(
            &bootstrap_servers,
            "test-idempotent",
            recommended_config(),
            dlq_config(),
        )
        .await
        .expect("Failed to ensure topics");
        assert!(!main_created2, "Second call should not create main topic");
        assert!(!dlq_created2, "Second call should not create DLQ topic");
    }

    // ============================================================================
    // M-474: create_client_config_checked Tests
    // ============================================================================

    #[test]
    fn test_create_client_config_checked_valid() {
        // M-474: Valid config should succeed
        let security = KafkaSecurityConfig::default();
        let result = security.create_client_config_checked("localhost:9092");
        assert!(result.is_ok(), "Valid config should succeed");

        // Just verify we can get the config - the actual settings are applied by rdkafka
        let _config = result.unwrap();
    }

    #[test]
    fn test_create_client_config_checked_empty_bootstrap() {
        // M-474: Empty bootstrap servers should fail with clear error
        let security = KafkaSecurityConfig::default();

        let result = security.create_client_config_checked("");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("empty"),
            "Error should mention empty bootstrap servers: {}",
            err
        );

        let result2 = security.create_client_config_checked("   ");
        assert!(result2.is_err());
    }

    #[test]
    fn test_create_client_config_checked_invalid_protocol() {
        // M-474: Invalid security protocol should fail with clear error
        let security = KafkaSecurityConfig {
            security_protocol: "invalid_protocol".to_string(),
            ..Default::default()
        };

        let result = security.create_client_config_checked("localhost:9092");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("security_protocol") || err.contains("invalid_protocol"),
            "Error should mention invalid protocol: {}",
            err
        );
    }

    #[test]
    fn test_create_client_config_checked_mismatched_sasl() {
        // M-474: SASL username without password should fail
        let security = KafkaSecurityConfig {
            security_protocol: "sasl_plaintext".to_string(),
            sasl_mechanism: Some("PLAIN".to_string()),
            sasl_username: Some("user".to_string()),
            sasl_password: None, // Missing password
            ..Default::default()
        };

        let result = security.create_client_config_checked("localhost:9092");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("sasl_username") || err.contains("sasl_password"),
            "Error should mention mismatched SASL credentials: {}",
            err
        );
    }

    // ============================================================================
    // M-478: get_broker_address_family Tests
    // Combined into single test to avoid env var race conditions (fixes #2094)
    // ============================================================================

    /// Combined test for broker address family to avoid env var race conditions
    /// when tests run in parallel. Uses mutex to serialize access to the shared
    /// KAFKA_BROKER_ADDRESS_FAMILY env var.
    #[test]
    fn test_get_broker_address_family_all_cases() {
        use std::sync::Mutex;
        static BROKER_ADDR_TEST_MUTEX: Mutex<()> = Mutex::new(());
        let _guard = BROKER_ADDR_TEST_MUTEX.lock().unwrap();

        // Save original value
        let original = std::env::var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY).ok();

        // ---- Localhost tests (M-478) ----
        // Localhost addresses should default to v4 for Docker compatibility
        std::env::remove_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY);
        assert_eq!(
            get_broker_address_family("localhost:9092"),
            "v4",
            "localhost should use v4"
        );
        assert_eq!(
            get_broker_address_family("127.0.0.1:9092"),
            "v4",
            "127.0.0.1 should use v4"
        );
        assert_eq!(
            get_broker_address_family("127.0.0.2:9092"),
            "v4",
            "127.x.x.x should use v4"
        );
        assert_eq!(
            get_broker_address_family("LOCALHOST:9092"),
            "v4",
            "LOCALHOST (uppercase) should use v4"
        );

        // ---- Remote address tests (M-478) ----
        // Remote addresses should allow any address family for IPv6 support
        std::env::remove_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY);
        assert_eq!(
            get_broker_address_family("kafka.example.com:9092"),
            "any",
            "Remote host should use any"
        );
        assert_eq!(
            get_broker_address_family("10.0.0.1:9092"),
            "any",
            "Private IPv4 should use any"
        );
        assert_eq!(
            get_broker_address_family("192.168.1.1:9092"),
            "any",
            "Private IPv4 should use any"
        );

        // ---- Multiple brokers tests (M-478) ----
        // If any broker is localhost, use v4 for consistency
        std::env::remove_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY);
        assert_eq!(
            get_broker_address_family("localhost:9092,kafka.example.com:9093"),
            "v4",
            "Mixed localhost+remote should use v4"
        );
        assert_eq!(
            get_broker_address_family("kafka1.example.com:9092,kafka2.example.com:9093"),
            "any",
            "All remote should use any"
        );

        // ---- Env override tests (M-478) ----
        // Env var should override auto-detection
        std::env::set_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY, "v6");
        assert_eq!(
            get_broker_address_family("localhost:9092"),
            "v6",
            "Env var should override to v6"
        );

        std::env::set_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY, "any");
        assert_eq!(
            get_broker_address_family("localhost:9092"),
            "any",
            "Env var should override to any"
        );

        std::env::set_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY, "v4");
        assert_eq!(
            get_broker_address_family("kafka.example.com:9092"),
            "v4",
            "Env var should override to v4"
        );

        // Restore original
        if let Some(val) = original {
            std::env::set_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY, val);
        } else {
            std::env::remove_var(env_vars::KAFKA_BROKER_ADDRESS_FAMILY);
        }
    }
}
