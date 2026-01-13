//! Real-time WebSocket server for streaming DashStream events to browsers
//!
//! Subscribes to Kafka topics and pushes events to connected web clients via WebSocket.
//!
//! # Security Warning
//!
//! **This is a development/example server without authentication or authorization.**
//!
//! By default, it binds to `127.0.0.1` (localhost only) for security. To expose on
//! a network, set `WEBSOCKET_HOST=0.0.0.0`, but be aware:
//!
//! - **No authentication**: Any client can connect to the WebSocket endpoint
//! - **No authorization**: All connected clients receive all streaming events
//! - **Sensitive data exposure**: Events may contain PII, API keys, or internal state
//!
//! For production deployments, place behind a reverse proxy (nginx, Traefik) with:
//! - TLS termination (HTTPS/WSS)
//! - Authentication (OAuth2, JWT, mTLS)
//! - Network segmentation (internal network only)
//!
//! See `docs/OBSERVABILITY_INFRASTRUCTURE.md` for production deployment guidance.
//!
//! # Message Delivery (M-492)
//!
//! Clients subscribe to the broadcast channel **after** WebSocket upgrade completes.
//! Messages broadcast during upgrade may be missed. Use the resume protocol:
//!
//! 1. After connecting, send `{"type": "resume", "lastOffsetsByPartition": {...}, "from": "...", "mode": "..."}`
//! 2. Server replays missed messages from its replay buffer (unless `from: "latest"`)
//! 3. Resume strategies (`from` field, M-703):
//!    - `"latest"`: Start from current position, no replay (ideal for first-time connects)
//!    - `"cursor"`: Resume from provided offsets (default)
//!    - `"earliest"`: Replay from earliest retained offsets
//! 4. Resume modes (`mode` field, M-765):
//!    - `"partition"`: Use partition+offset cursors (explicit)
//!    - `"thread"`: Use thread_id+sequence cursors (explicit, legacy)
//!    - `"auto"` or absent: Implicit selection based on field presence (backwards compatible)
//!
//! # Cursor Reset Protocol (M-706)
//!
//! Clients can request an explicit cursor reset to recover from corrupt state:
//!
//! 1. Send `{"type": "cursor_reset"}`
//! 2. Server responds with `{"type": "cursor_reset_complete", "latestOffsetsByPartition": {...}}`
//! 3. Client can then use these offsets for a clean `"from": "cursor"` resume
//!
//! Use cursor_reset when:
//! - State is known to be corrupt
//! - Topic was recreated with incompatible offsets
//! - Admin action to force clean state
//!
//! # Rate Limiting (M-488)
//!
//! Per-IP connection rate limiting prevents DoS attacks. Configure via:
//! - `WEBSOCKET_MAX_CONNECTIONS_PER_IP`: Max connections per IP (default: 10)
//! - `WEBSOCKET_TRUSTED_PROXY_IPS`: Comma-separated proxy IP allowlist; only these peers may set `x-forwarded-for`

// Allow unwrap/expect in this binary - it's a development/observability server
// that benefits from fail-fast behavior for debugging. Production deployments
// should place this behind a reverse proxy per the security warning above.
// Also allow clone_on_ref_ptr since Arc::clone() style is idiomatic in this codebase.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::clone_on_ref_ptr)]

use axum::{routing::get, Router};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bytes::Bytes; // M-995: Zero-copy broadcast payloads
use chrono::Utc;
use dashflow_streaming::consumer::SequenceValidator; // Issue #11: Sequence validation
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::Event; // Issue #13: For partial decode of DLQ messages
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use prost::Message as ProstMessage; // Issue #13: For Event::decode() in DLQ forensics
use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGaugeVec, Opts, Registry,
};
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::Message as KafkaMessage;
use rdkafka::message::Timestamp as KafkaTimestamp;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256}; // M-1064: SHA256 hash for DLQ payload verification
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock, Semaphore};
use tower_http::services::ServeDir;
use tracing::info_span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// CQ-100: Modules extracted from main.rs for better organization
mod client_ip;
mod config;
mod dashstream;
mod handlers;
mod kafka_util;
mod replay_buffer;
mod state;

// Re-exports from extracted modules
use client_ip::extract_client_ip;
use config::{
    compute_resume_namespace, get_dlq_include_full_payload, get_dlq_send_timeout_ms,
    get_max_concurrent_dlq_sends, get_replay_max_total, get_replay_timeout_secs,
    get_send_timeout_secs, parse_env_var_with_warning, parse_optional_env_var_with_warning,
    DEFAULT_SLOW_CLIENT_DISCONNECT_THRESHOLD, DEFAULT_SLOW_CLIENT_LAG_WINDOW_SECS,
    // Environment variable name constants (M-153)
    EXPECTED_SCHEMAS_PATH, KAFKA_AUTO_OFFSET_RESET, KAFKA_BROKERS, KAFKA_CLUSTER_ID,
    KAFKA_DLQ_TOPIC, KAFKA_GROUP_ID, KAFKA_OLD_DATA_USE_TIMESTAMP, KAFKA_TOPIC,
    OTEL_EXPORTER_OTLP_ENDPOINT, REDIS_URL, WEBSOCKET_HOST, WEBSOCKET_MAX_CONNECTIONS_PER_IP,
};
use dashstream::process_dashstream_header;
use kafka_util::KafkaHeaderExtractor;
use replay_buffer::{KafkaCursor, OutboundBinaryMessage, ReplayBuffer};
use state::{
    ConnectionRateLimiter, DecodeErrorLog, DecodeErrorPolicy, ServerMetrics, ServerMetricsSnapshot,
    ServerState, WebsocketServerMetricsCollector, parse_trusted_proxy_ips_from_env,
};


// CQ-100: extract_client_ip moved to client_ip.rs
// CQ-100: process_dashstream_header moved to dashstream.rs

// =============================================================================
// M-429: DLQ DURABILITY SEMANTICS - EXPLICIT DOCUMENTATION
// =============================================================================
//
// The WebSocket server implements a **FAIL-OPEN** DLQ design. When a message
// fails to decode or validate, it is sent to the DLQ asynchronously via
// `tokio::spawn`. The Kafka consumer offset commit proceeds REGARDLESS of
// whether the DLQ send succeeds.
//
// ## Implications:
//
// 1. **DLQ sends are best-effort**: If the DLQ producer fails (timeout,
//    backpressure, Kafka error), the original message is effectively lost
//    from DLQ forensics.
//
// 2. **Offset commits are independent**: The main Kafka topic offset is
//    committed after the message is processed (broadcast to clients), NOT
//    after DLQ success. This prevents a broken DLQ from blocking the entire
//    pipeline.
//
// 3. **Backpressure drops silently**: If `MAX_CONCURRENT_DLQ_SENDS` is
//    exhausted, new DLQ messages are dropped (logged + metric incremented).
//
// ## Rationale:
//
// This design prioritizes **pipeline availability** over **forensic completeness**.
// A broken DLQ should not block real-time observability. The primary data path
// (main Kafka topic → WebSocket → UI) continues regardless of DLQ health.
//
// ## Monitoring:
//
// - `websocket_dlq_sends_total` - Successful DLQ writes
// - `websocket_dlq_send_failures_total{reason="timeout|kafka_error|backpressure"}` - Failed writes
// - Alert `WebSocketDlqBroken` fires when DLQ failures occur
//
// ## Future Consideration (not implemented):
//
// A "fail-closed" mode where offset commit waits for DLQ success could be
// added as an opt-in feature (`DLQ_FAIL_CLOSED=true`). This would trade
// availability for durability - a DLQ failure would cause consumer lag.
// =============================================================================

// CQ-100: Config constants and functions moved to config.rs
// See config.rs for: DEFAULT_*, parse_env_var_with_warning, compute_resume_namespace, get_* functions

// Replay buffer for reconnection recovery (Issue #3: Data Loss Bugs)
//
// Hybrid architecture: Memory (fast) + Redis (persistent, shared across servers)
// - Memory: Last 1000 messages, 1ms latency, synchronous writes
// - Redis: Last 10,000 messages, configurable TTL (default 1h), 5-10ms latency
//
// # Durability Model (M-494)

// ============================================================================
// Expected Schema Store - Server-side persistence for expected graph schemas
// ============================================================================

/// Expected schema entry stored server-side
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExpectedSchemaEntry {
    /// Content-addressed schema ID (hash)
    pub schema_id: String,
    /// Graph name this schema is for
    pub graph_name: String,
    /// Environment (e.g., "production", "staging", "development")
    pub environment: Option<String>,
    /// When this expectation was set (Unix timestamp ms)
    pub pinned_at: i64,
    /// Who set this expectation (optional, for audit)
    pub pinned_by: Option<String>,
    /// Optional human-readable note about this schema version
    pub note: Option<String>,
}

/// Request body for setting expected schema
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SetExpectedSchemaRequest {
    pub schema_id: String,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub pinned_by: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// In-memory store for expected schemas with file persistence
#[derive(Clone)]
pub(crate) struct ExpectedSchemaStore {
    /// In-memory store: graph_name -> ExpectedSchemaEntry
    schemas: Arc<RwLock<HashMap<String, ExpectedSchemaEntry>>>,
    /// Path to persistence file
    persistence_path: Option<std::path::PathBuf>,
}

impl ExpectedSchemaStore {
    /// Create a new store with optional file persistence
    ///
    /// M-484: Restructured to avoid block_on() in async context.
    /// Instead of loading after construction, we load first and construct with data.
    pub fn new(persistence_path: Option<std::path::PathBuf>) -> Self {
        // Load schemas from file BEFORE creating the RwLock (M-484 fix)
        let initial_schemas = if let Some(ref path) = persistence_path {
            if path.exists() {
                match Self::load_schemas_sync(path) {
                    Ok(schemas) => {
                        println!(
                            "📦 Loaded {} expected schemas from {:?}",
                            schemas.len(),
                            path
                        );
                        schemas
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to load expected schemas from {:?}: {}", path, e);
                        HashMap::new()
                    }
                }
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        Self {
            schemas: Arc::new(RwLock::new(initial_schemas)),
            persistence_path,
        }
    }

    /// Create a store without persistence (memory-only)
    ///
    /// Test infrastructure: Reserved for test utilities and future configuration options.
    #[allow(dead_code)]
    pub fn new_memory_only() -> Self {
        Self::new(None)
    }

    /// Load schemas from file synchronously (pure sync, no async primitives)
    ///
    /// M-484: This is a pure sync function that reads and parses the file.
    /// No block_on() or async locks involved - safe to call from any context.
    fn load_schemas_sync(
        path: &std::path::Path,
    ) -> std::io::Result<HashMap<String, ExpectedSchemaEntry>> {
        let content = std::fs::read_to_string(path)?;
        let schemas: HashMap<String, ExpectedSchemaEntry> = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(schemas)
    }

    /// Persist schemas to file
    async fn persist(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.persistence_path {
            let schemas = self.schemas.read().await;
            let content = serde_json::to_string_pretty(&*schemas)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            // Write atomically via temp file
            let temp_path = path.with_extension("tmp");
            tokio::fs::write(&temp_path, content).await?;
            tokio::fs::rename(&temp_path, path).await?;
        }
        Ok(())
    }

    /// Get expected schema for a graph
    pub async fn get(&self, graph_name: &str) -> Option<ExpectedSchemaEntry> {
        let schemas = self.schemas.read().await;
        schemas.get(graph_name).cloned()
    }

    /// Set expected schema for a graph
    pub async fn set(
        &self,
        graph_name: String,
        request: SetExpectedSchemaRequest,
    ) -> ExpectedSchemaEntry {
        let entry = ExpectedSchemaEntry {
            schema_id: request.schema_id,
            graph_name: graph_name.clone(),
            environment: request.environment,
            pinned_at: chrono::Utc::now().timestamp_millis(),
            pinned_by: request.pinned_by,
            note: request.note,
        };

        {
            let mut schemas = self.schemas.write().await;
            schemas.insert(graph_name, entry.clone());
        }

        // Persist synchronously to prevent data loss (M-483 fix)
        // The slight latency is acceptable - this writes a small JSON file
        if let Err(e) = self.persist().await {
            tracing::error!("Failed to persist expected schemas: {}", e);
        }

        entry
    }

    /// Remove expected schema for a graph
    pub async fn remove(&self, graph_name: &str) -> Option<ExpectedSchemaEntry> {
        let entry = {
            let mut schemas = self.schemas.write().await;
            schemas.remove(graph_name)
        };

        // Persist synchronously to prevent data loss (M-483 fix)
        if entry.is_some() {
            if let Err(e) = self.persist().await {
                tracing::error!("Failed to persist expected schemas: {}", e);
            }
        }

        entry
    }

    /// List all expected schemas
    pub async fn list(&self) -> Vec<ExpectedSchemaEntry> {
        let schemas = self.schemas.read().await;
        schemas.values().cloned().collect()
    }
}

// Server state, metrics, and rate limiting types are in state.rs

/// Initialize OpenTelemetry distributed tracing (Issue #14)
///
/// Sets up OTLP exporter to send traces to Jaeger at localhost:4317
fn init_telemetry(service_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use opentelemetry_sdk::Resource;

    // Get Jaeger endpoint from env or use default
    let jaeger_endpoint = std::env::var(OTEL_EXPORTER_OTLP_ENDPOINT)
        .unwrap_or_else(|_| "http://jaeger:4317".to_string());

    println!(
        "🔍 Initializing OpenTelemetry tracing: service={}, endpoint={}",
        service_name, jaeger_endpoint
    );

    // Issue #14: Configure OTLP exporter to send traces to Jaeger
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&jaeger_endpoint)
        .build()?;

    // Create resource with service name using builder_empty (0.31 API)
    let resource = Resource::builder_empty()
        .with_service_name(service_name.to_string())
        .build();

    // Create tracer provider with OTLP exporter (0.31 API: with_batch_exporter takes only exporter)
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    let tracer = tracer_provider.tracer(service_name.to_string());

    // Set up tracing subscriber with OpenTelemetry layer
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with(telemetry_layer)
        .init();

    println!(
        "✅ OpenTelemetry tracing initialized with OTLP export to {}",
        jaeger_endpoint
    );
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("⚠️  Failed to listen for Ctrl+C: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(e) => {
                eprintln!("⚠️  Failed to listen for SIGTERM: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = async {
        std::future::pending::<()>().await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #14: Initialize distributed tracing
    init_telemetry("websocket-server")?;

    // 1. Create shutdown signal channel
    // This allows graceful shutdown when receiving SIGTERM/SIGINT
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    // M-482: Create shutdown flag for lag monitor thread (std::thread can't use async channels)
    let lag_monitor_shutdown = Arc::new(AtomicBool::new(false));
    let lag_monitor_shutdown_clone = lag_monitor_shutdown.clone();

    // 2. Spawn signal handler task
    tokio::spawn(async move {
        shutdown_signal().await;
        println!("\n🛑 Received shutdown signal, initiating graceful shutdown...");
        // M-482: Signal lag monitor thread to stop
        lag_monitor_shutdown_clone.store(true, Ordering::SeqCst);
        // Notify all async tasks to shut down
        let _ = shutdown_tx_clone.send(());
    });

    // 3. Create Kafka consumer for DashStream topic
    let kafka_brokers =
        std::env::var(KAFKA_BROKERS).unwrap_or_else(|_| "127.0.0.1:9092".to_string());
    println!("🔗 Connecting to Kafka brokers: {}", kafka_brokers);

    let kafka_topic =
        std::env::var(KAFKA_TOPIC).unwrap_or_else(|_| "dashstream-quality".to_string());
    let kafka_dlq_topic =
        std::env::var(KAFKA_DLQ_TOPIC).unwrap_or_else(|_| format!("{}-dlq", kafka_topic));
    println!("📨 Kafka topic: {}", kafka_topic);
    println!("🧯 Kafka DLQ topic: {}", kafka_dlq_topic);

    // P1.5: Kafka consumer group ID is now configurable via KAFKA_GROUP_ID env var
    // S-23: Read group_id outside loop so it can be used in log message
    let group_id =
        std::env::var(KAFKA_GROUP_ID).unwrap_or_else(|_| "websocket-server-v4".to_string());

    // M-691: Stable namespace for replay/resume to prevent cross-topic/cluster collisions.
    // M-747: Optional KAFKA_CLUSTER_ID prevents collisions when multiple clusters share broker hostnames.
    let kafka_cluster_id: Option<String> = std::env::var(KAFKA_CLUSTER_ID).ok();
    let resume_namespace = compute_resume_namespace(
        &kafka_brokers,
        &kafka_topic,
        &group_id,
        kafka_cluster_id.as_deref(),
    );
    if let Some(ref cluster_id) = kafka_cluster_id {
        println!("🧭 Resume namespace: {} (cluster_id: {})", resume_namespace, cluster_id);
    } else {
        println!("🧭 Resume namespace: {}", resume_namespace);
    }

    // M-413: Unified Kafka security configuration from environment
    // Supports KAFKA_SECURITY_PROTOCOL, KAFKA_SASL_*, KAFKA_SSL_* env vars
    let kafka_security = KafkaSecurityConfig::from_env();
    if let Err(e) = kafka_security.validate() {
        return Err(format!("Invalid Kafka security config: {}", e).into());
    }
    println!("🔐 Kafka security: {}", kafka_security.security_protocol);

    // Issue #2: Retry with exponential backoff instead of panic
    let (consumer, kafka_consumer_create_retries): (StreamConsumer, u32) = {
        let mut retry_count: u32 = 0;
        let max_retries: u32 = 5;
        loop {
            // M-413/M-478: Use unified client config builder so TLS/SASL and
            // broker.address.family (IPv4/IPv6) stay consistent across binaries.
            let mut config = kafka_security.create_client_config(&kafka_brokers);
            config
                .set("group.id", &group_id)
                .set("enable.auto.commit", "true")
                // Store offsets only after we've processed the message (at-least-once).
                .set("enable.auto.offset.store", "false")
                .set(
                    "auto.offset.reset",
                    // Production default: "earliest" prevents data loss on restart
                    // Testing override: KAFKA_AUTO_OFFSET_RESET=latest skips old messages
                    std::env::var(KAFKA_AUTO_OFFSET_RESET)
                        .unwrap_or_else(|_| "earliest".to_string()),
                );
            match config.create() {
                Ok(c) => break (c, retry_count),
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        return Err(format!(
                            "Failed to create Kafka consumer after {} retries: {}",
                            max_retries, e
                        )
                        .into());
                    }
                    let backoff_ms = 1000 * (1 << retry_count); // 2s, 4s, 8s, 16s
                    eprintln!(
                        "⚠️  Failed to create Kafka consumer (attempt {}/{}): {}",
                        retry_count, max_retries, e
                    );
                    eprintln!("   Retrying in {}ms...", backoff_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    };
    let consumer: Arc<StreamConsumer> = Arc::new(consumer);

    // S-23: Use actual group_id instead of hardcoded string
    println!("✅ Kafka consumer created (group: {})", group_id);

    // Issue #2: Retry subscribe with backoff instead of panic
    let kafka_subscribe_retries: u32 = {
        let mut retry_count: u32 = 0;
        let max_retries: u32 = 5;
        loop {
            match consumer.subscribe(&[&kafka_topic]) {
                Ok(_) => break retry_count,
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        return Err(format!(
                            "Failed to subscribe to '{}' after {} retries: {}",
                            kafka_topic, max_retries, e
                        )
                        .into());
                    }
                    let backoff_ms = 1000 * (1 << retry_count); // 2s, 4s, 8s, 16s
                    eprintln!(
                        "⚠️  Failed to subscribe to Kafka topic (attempt {}/{}): {}",
                        retry_count, max_retries, e
                    );
                    eprintln!("   Retrying in {}ms...", backoff_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    };

    println!("✅ Subscribed to '{}' topic", kafka_topic);

    // M-430: Create a metadata-only consumer for background lag monitoring
    // This decouples watermark fetching from the main message processing loop,
    // preventing blocking on the hot path.
    let metadata_consumer: StreamConsumer = {
        // M-413/M-478: Keep security + address-family config consistent.
        let mut config = kafka_security.create_client_config(&kafka_brokers);
        config
            // Use a different group.id so it doesn't interfere with the main consumer
            .set("group.id", format!("{}-lag-monitor", group_id))
            .set("enable.auto.commit", "false"); // Metadata-only, no commits
        config
            .create()
            .expect("Failed to create metadata consumer for lag monitoring")
    };
    println!("✅ Metadata consumer created for lag monitoring");

    // M-1003: Fetch high watermarks at startup for proper old-data classification.
    // This enables correct catch-up semantics: messages before the session-start head
    // are considered "old data" (expected decode errors from schema changes), while
    // messages at or after the head are "new data" (real failures requiring attention).
    //
    // The fetch_watermarks() call is blocking, so we run it synchronously at startup
    // before the async consumer loop begins. This is acceptable because it only happens
    // once at server start, not in the hot path.
    let session_head_offsets: Arc<std::sync::RwLock<HashMap<i32, i64>>> = {
        let kafka_topic_for_watermarks = kafka_topic.clone();
        println!("📊 Fetching high watermarks for old-data classification...");

        // First, get topic metadata to discover all partitions
        let metadata_result = metadata_consumer.fetch_metadata(
            Some(&kafka_topic_for_watermarks),
            std::time::Duration::from_secs(10),
        );

        let metadata = match metadata_result {
            Ok(m) => Some(m),
            Err(e) => {
                // Non-fatal: fall back to lazy detection (first-seen offset per partition)
                eprintln!(
                    "⚠️  Failed to fetch topic metadata for old-data classification: {}. \
                     Will use first-seen offset as fallback.",
                    e
                );
                None
            }
        };

        let mut head_offsets: HashMap<i32, i64> = HashMap::new();

        // M-1028: Bounded startup watermark phase with overall time budget.
        // Problem: With many partitions, sequential 5s timeouts can block server start O(N × 5s).
        // Solution: Overall budget (15s) + reduced per-partition timeout (2s) + early exit.
        let startup_budget_secs: u64 = 15; // Total time budget for all partition watermark fetches
        let per_partition_timeout_secs: u64 = 2; // Reduced from 5s to allow more partitions
        let watermark_start = std::time::Instant::now();
        let mut partitions_skipped: usize = 0;

        if let Some(ref metadata) = metadata {
            // Find our topic in the metadata
            for topic in metadata.topics() {
                if topic.name() == kafka_topic_for_watermarks {
                    if let Some(err) = topic.error() {
                        eprintln!(
                            "⚠️  Topic '{}' has error in metadata: {:?}. Using first-seen fallback.",
                            kafka_topic_for_watermarks, err
                        );
                        break;
                    }

                    let total_partitions = topic.partitions().len();

                    // Fetch high watermark for each partition (bounded)
                    for partition_meta in topic.partitions() {
                        // M-1028: Check overall budget before starting this partition
                        if watermark_start.elapsed().as_secs() >= startup_budget_secs {
                            let remaining =
                                total_partitions - head_offsets.len() - partitions_skipped;
                            eprintln!(
                                "⚠️  Startup watermark budget ({}s) exhausted; skipping {} remaining partition(s). \
                                 These will use first-seen offset fallback.",
                                startup_budget_secs, remaining
                            );
                            partitions_skipped += remaining;
                            break;
                        }

                        let partition_id = partition_meta.id();
                        match metadata_consumer.fetch_watermarks(
                            &kafka_topic_for_watermarks,
                            partition_id,
                            std::time::Duration::from_secs(per_partition_timeout_secs),
                        ) {
                            Ok((_low, high)) => {
                                head_offsets.insert(partition_id, high);
                                println!(
                                    "   Partition {}: high watermark = {} (messages below this are catch-up)",
                                    partition_id, high
                                );
                            }
                            Err(e) => {
                                // Non-fatal per partition: will use first-seen offset for this partition
                                partitions_skipped += 1;
                                eprintln!(
                                    "⚠️  Failed to fetch watermark for partition {}: {}. \
                                     Will use first-seen offset for this partition.",
                                    partition_id, e
                                );
                            }
                        }
                    }
                    break;
                }
            }
        }

        // M-1028: Log startup watermark fetch summary with timing
        let watermark_elapsed_ms = watermark_start.elapsed().as_millis();
        if head_offsets.is_empty() {
            println!(
                "⚠️  No high watermarks fetched ({}ms); old-data classification will use first-seen offset fallback",
                watermark_elapsed_ms
            );
        } else if partitions_skipped > 0 {
            println!(
                "✅ Fetched high watermarks for {} partitions in {}ms ({} skipped → first-seen fallback)",
                head_offsets.len(),
                watermark_elapsed_ms,
                partitions_skipped
            );
        } else {
            println!(
                "✅ Fetched high watermarks for {} partitions in {}ms (catch-up phase detection enabled)",
                head_offsets.len(),
                watermark_elapsed_ms
            );
        }

        Arc::new(std::sync::RwLock::new(head_offsets))
    };
    let session_head_offsets_for_consumer = Arc::clone(&session_head_offsets);

    // M-431: Shared state for partition offsets (replaces unbounded channel)
    // Using std::sync::RwLock (not tokio) because lag monitor runs in a std::thread
    // HashMap value: (current_offset, last_update_time) for staleness tracking
    type PartitionOffsets = std::sync::RwLock<HashMap<i32, (i64, Instant)>>;
    let partition_offsets: Arc<PartitionOffsets> = Arc::new(std::sync::RwLock::new(HashMap::new()));
    let partition_offsets_for_consumer = Arc::clone(&partition_offsets);
    let partition_offsets_for_lag_monitor = Arc::clone(&partition_offsets);

    // 4. Create broadcast channel for WebSocket clients (now sending binary protobuf)
    // Make buffer size configurable to handle varying throughput scenarios
    let buffer_size = parse_env_var_with_warning("WEBSOCKET_BUFFER_SIZE", 1000usize);

    println!("📊 WebSocket buffer size: {} messages", buffer_size);

    // M-979: Make maximum payload size configurable so operators can tune for large snapshots/diffs.
    // This value is passed into dashflow_streaming::codec::decode_message_compatible, which enforces:
    // - max_size (+1 if framed header present)
    // - decompression limits for zstd frames
    let max_payload_bytes = parse_env_var_with_warning(
        "WEBSOCKET_MAX_PAYLOAD_BYTES",
        dashflow_streaming::codec::DEFAULT_MAX_PAYLOAD_SIZE,
    );
    println!("📦 WebSocket max payload size: {} bytes", max_payload_bytes);

    // M-1020: Parse decode error policy from KAFKA_ON_DECODE_ERROR env var.
    // - "skip" (default): Advance offset past decode errors (prioritize availability)
    // - "pause": Stop consuming on decode error (prioritize durability; requires restart)
    let decode_error_policy = DecodeErrorPolicy::from_env();
    println!(
        "📜 Kafka decode error policy: {} (KAFKA_ON_DECODE_ERROR)",
        decode_error_policy.as_str()
    );

    let (tx, _rx) = broadcast::channel::<OutboundBinaryMessage>(buffer_size);
    let tx_clone = tx.clone();

    // 5. Create shared metrics state (atomic counters, no RwLock needed)
    let metrics = Arc::new(ServerMetrics::default());
    let metrics_clone = metrics.clone();

    // 5b. Create Prometheus registry and metrics (Issue #11)
    let prometheus_registry = Registry::new();

    // M-1103 FIX: Return None on registration failure (consistent with other metrics)
    let decode_errors = match IntCounterVec::new(
        Opts::new(
            "websocket_decode_errors_total",
            "Total protobuf decode errors",
        ),
        &["error_type"], // labels: "buffer_underflow", "invalid_protobuf", etc.
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register decode_errors metric: {}", e);
                eprintln!("   Continuing without Prometheus decode_errors metric");
                None // M-1103: Return None on registration failure
            } else {
                Some(m) // M-1103: Only return Some on successful registration
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create decode_errors counter: {}", e);
            eprintln!("   Continuing without Prometheus decode_errors metric");
            None
        }
    };

    // Issue #6: Client lag monitoring - track lag events and amount
    let client_lag_events = match IntCounterVec::new(
        Opts::new(
            "websocket_client_lag_events_total",
            "Total client lag events (missed messages due to slow consumption)",
        ),
        &["severity"], // labels: "warning" (>10 msgs), "critical" (>100 msgs)
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register client_lag_events metric: {}", e);
                eprintln!("   Continuing without Prometheus client_lag_events metric");
                None // ← FIX: Return None on registration failure
            } else {
                Some(m) // ← FIX: Only return Some on success
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create client_lag_events counter: {}", e);
            eprintln!("   Continuing without Prometheus client_lag_events metric");
            None
        }
    };

    let client_lag_messages = match IntCounterVec::new(
        Opts::new(
            "websocket_client_lag_messages_total",
            "Total messages lagged by slow clients (sum of all lag amounts)",
        ),
        &["severity"], // labels: "warning", "critical"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register client_lag_messages metric: {}", e);
                eprintln!("   Continuing without Prometheus client_lag_messages metric");
                None // ← FIX: Return None on registration failure
            } else {
                Some(m) // ← FIX: Only return Some on success
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create client_lag_messages counter: {}", e);
            eprintln!("   Continuing without Prometheus client_lag_messages metric");
            None
        }
    };

    // M-682: Slow client disconnect counter (backpressure)
    let slow_client_disconnects = match IntCounter::new(
        "websocket_slow_client_disconnects_total",
        "Total clients disconnected due to cumulative lag exceeding threshold (backpressure)",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register slow_client_disconnects metric: {}", e);
                eprintln!("   Continuing without Prometheus slow_client_disconnects metric");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create slow_client_disconnects counter: {}", e);
            eprintln!("   Continuing without Prometheus slow_client_disconnects metric");
            None
        }
    };

    // M-1061: Oversized control frame rejections counter (DoS prevention)
    let control_oversized_total = match IntCounter::new(
        "websocket_control_oversized_total",
        "Total WebSocket control frames rejected for exceeding size limit (DoS prevention)",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register control_oversized_total metric: {}", e);
                eprintln!("   Continuing without Prometheus control_oversized_total metric");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create control_oversized_total counter: {}", e);
            eprintln!("   Continuing without Prometheus control_oversized_total metric");
            None
        }
    };

    // M-1062: Invalid JSON parse failures on control messages
    let control_parse_failures_total = match IntCounter::new(
        "websocket_control_parse_failures_total",
        "Total WebSocket control messages with invalid JSON (protocol drift or malicious input)",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register control_parse_failures_total metric: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create control_parse_failures_total counter: {}", e);
            None
        }
    };

    // M-682: Parse threshold from env var
    let slow_client_disconnect_threshold: u64 =
        parse_env_var_with_warning("SLOW_CLIENT_DISCONNECT_THRESHOLD", DEFAULT_SLOW_CLIENT_DISCONNECT_THRESHOLD);
    // M-773: Parse lag window duration from env var
    let slow_client_lag_window_secs: u64 =
        parse_env_var_with_warning("SLOW_CLIENT_LAG_WINDOW_SECS", DEFAULT_SLOW_CLIENT_LAG_WINDOW_SECS);
    if slow_client_disconnect_threshold > 0 {
        if slow_client_lag_window_secs > 0 {
            println!(
                "📊 Slow client disconnect threshold: {} messages within {}s window (windowed backpressure)",
                slow_client_disconnect_threshold, slow_client_lag_window_secs
            );
        } else {
            println!(
                "📊 Slow client disconnect threshold: {} messages lifetime (cumulative backpressure)",
                slow_client_disconnect_threshold
            );
        }
    } else {
        println!("📊 Slow client disconnect threshold: disabled (backpressure off)");
    }

    // Issue #4: Granular Kafka error tracking with error type labels
    let kafka_errors_by_type = match IntCounterVec::new(
        Opts::new(
            "websocket_kafka_errors_by_type_total",
            "Kafka errors by type (dns_failure, connection_timeout, broker_down, decode_error)",
        ),
        &["error_type"], // labels: "dns_failure", "connection_timeout", "broker_down", "decode_error", "unknown"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register kafka_errors_by_type metric: {}", e);
                eprintln!("   Continuing without Prometheus kafka_errors_by_type metric");
                None // ← FIX: Return None on registration failure
            } else {
                Some(m) // ← FIX: Only return Some on success
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create kafka_errors_by_type counter: {}", e);
            eprintln!("   Continuing without Prometheus kafka_errors_by_type metric");
            None
        }
    };

    // M-1025: Track messages with missing payload (payload=None from Kafka)
    // This makes silent drops visible to operators so they can detect data loss.
    let payload_missing_total = match IntCounter::new(
        "websocket_kafka_payload_missing_total",
        "Total Kafka messages received with no payload (silent drops without this metric)",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register payload_missing_total metric: {}", e);
                eprintln!("   Continuing without payload_missing tracking");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create payload_missing_total counter: {}", e);
            eprintln!("   Continuing without payload_missing tracking");
            None
        }
    };

    // Issue #5: End-to-end latency tracking from quality monitor to UI
    // Buckets: 0.1s, 0.25s, 0.5s, 1s, 2.5s, 5s, 10s, 25s, 50s (in milliseconds)
    let e2e_latency_histogram = match HistogramVec::new(
        HistogramOpts::new(
            "websocket_e2e_latency_ms",
            "End-to-end latency from quality monitor produce to websocket consume (milliseconds)",
        )
        .buckets(vec![
            100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 25000.0, 50000.0,
        ]),
        &["stage"], // labels: "producer_to_consumer" - measures producer-to-consumer latency
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register e2e_latency_histogram metric: {}", e);
                eprintln!("   Continuing without Prometheus e2e_latency_histogram metric");
                None // ← FIX: Return None on registration failure
            } else {
                Some(m) // ← FIX: Only return Some on success
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create e2e_latency_histogram: {}", e);
            eprintln!("   Continuing without Prometheus e2e_latency_histogram metric");
            None
        }
    };

    // M-644: Clock skew detection counter for E2E latency
    // Incremented when latency_us < 0 (negative latency indicates clock skew between producer/consumer)
    // or latency_us > 60s (sanity threshold for outliers)
    let clock_skew_events_total = match IntCounter::new(
        "websocket_clock_skew_events_total",
        "Total clock skew events detected in E2E latency calculation (negative or >60s latency)",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!(
                    "⚠️  Failed to register clock_skew_events_total metric: {}",
                    e
                );
                eprintln!("   Continuing without clock skew tracking");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!(
                "⚠️  Failed to create clock_skew_events_total counter: {}",
                e
            );
            eprintln!("   Continuing without clock skew tracking");
            None
        }
    };

    // Issue #11: Sequence validation metrics for detecting message loss, duplicates, reordering
    // P0.4: Removed thread_id label to prevent unbounded cardinality in Prometheus.
    // Per-thread debugging info is logged to traces instead.
    let sequence_gaps_total = match IntCounter::new(
        "dashstream_sequence_gaps_total",
        "Total sequence gaps detected (message loss)",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register sequence_gaps_total metric: {}", e);
                eprintln!("   Continuing without sequence gap tracking");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create sequence_gaps_total counter: {}", e);
            eprintln!("   Continuing without sequence gap tracking");
            None
        }
    };

    // M-1115: Histogram for gap sizes to distinguish minor blips from mass loss
    // Buckets: 1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 5000, 10000
    let sequence_gap_size_histogram = match Histogram::with_opts(
        HistogramOpts::new(
            "dashstream_sequence_gap_size",
            "Size of sequence gaps (number of missing messages per gap event)",
        )
        .buckets(vec![1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 5000.0, 10000.0]),
    ) {
        Ok(h) => {
            if let Err(e) = prometheus_registry.register(Box::new(h.clone())) {
                eprintln!("⚠️  Failed to register sequence_gap_size histogram: {}", e);
                eprintln!("   Continuing without gap size histogram");
                None
            } else {
                Some(h)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create sequence_gap_size histogram: {}", e);
            eprintln!("   Continuing without gap size histogram");
            None
        }
    };

    // P0.4: Removed thread_id label to prevent unbounded cardinality
    let sequence_duplicates_total = match IntCounter::new(
        "dashstream_sequence_duplicates_total",
        "Total duplicate sequences detected",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!(
                    "⚠️  Failed to register sequence_duplicates_total metric: {}",
                    e
                );
                eprintln!("   Continuing without sequence duplicate tracking");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!(
                "⚠️  Failed to create sequence_duplicates_total counter: {}",
                e
            );
            eprintln!("   Continuing without sequence duplicate tracking");
            None
        }
    };

    // P0.4: Removed thread_id label to prevent unbounded cardinality
    let sequence_reorders_total = match IntCounter::new(
        "dashstream_sequence_reorders_total",
        "Total out-of-order sequences detected",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!(
                    "⚠️  Failed to register sequence_reorders_total metric: {}",
                    e
                );
                eprintln!("   Continuing without sequence reorder tracking");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!(
                "⚠️  Failed to create sequence_reorders_total counter: {}",
                e
            );
            eprintln!("   Continuing without sequence reorder tracking");
            None
        }
    };

    // Issue #13: DLQ metrics for monitoring failed message handling
    // S-3: Renamed from dashstream_dlq_* to websocket_dlq_* to avoid schema conflict
    // with library's plain counters. WebSocket-specific metrics have error_type/reason labels.
    let dlq_sends_total = match IntCounterVec::new(
        Opts::new(
            "websocket_dlq_sends_total",
            "Total messages sent to dead letter queue (websocket server)",
        ),
        &["error_type"], // labels: "decode_error", "decompression_failure", etc.
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register dlq_sends_total metric: {}", e);
                eprintln!("   Continuing without DLQ message tracking");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create dlq_sends_total counter: {}", e);
            eprintln!("   Continuing without DLQ message tracking");
            None
        }
    };

    let dlq_send_failures_total = match IntCounterVec::new(
        Opts::new(
            "websocket_dlq_send_failures_total",
            "Failed attempts to send to DLQ (websocket server)",
        ),
        &["reason"], // labels: "timeout", "kafka_error", etc.
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!(
                    "⚠️  Failed to register dlq_send_failures_total metric: {}",
                    e
                );
                eprintln!("   Continuing without DLQ failure tracking");
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!(
                "⚠️  Failed to create dlq_send_failures_total counter: {}",
                e
            );
            eprintln!("   Continuing without DLQ failure tracking");
            None
        }
    };

    // Retry count histogram for websocket server operations (renamed to avoid collision with quality exporter)
    let retry_count_histogram = match HistogramVec::new(
        HistogramOpts::new(
            dashflow_streaming::metrics_constants::METRIC_WS_RETRY_COUNT,
            "Number of retries for websocket operations",
        )
        .buckets(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 20.0]),
        &["operation"], // labels: "kafka_connect", "dlq_send", etc.
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register retry_count_histogram metric: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create retry_count_histogram: {}", e);
            None
        }
    };

    // M-695: Observe real retry counts for startup operations (0 means no retries).
    if let Some(ref h) = retry_count_histogram {
        h.with_label_values(&["kafka_consumer_create"])
            .observe(kafka_consumer_create_retries as f64);
        h.with_label_values(&["kafka_subscribe"])
            .observe(kafka_subscribe_retries as f64);
    }

    // Redis connection errors counter (M-647: renamed to component-scoped name)
    let redis_connection_errors_total = match IntCounter::new(
        "dashstream_websocket_redis_errors_total",
        "Total Redis connection errors in websocket replay buffer",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!(
                    "⚠️  Failed to register websocket_redis_errors_total metric: {}",
                    e
                );
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_redis_errors_total: {}", e);
            None
        }
    };

    // Redis operation latency histogram (M-647: renamed to component-scoped name)
    let redis_operation_latency = match HistogramVec::new(
        HistogramOpts::new(
            "dashstream_websocket_redis_latency_ms",
            "Redis operation latency in milliseconds for websocket replay",
        )
        .buckets(vec![
            1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
        ]),
        &["operation"], // labels: "read", "write"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!(
                    "⚠️  Failed to register websocket_redis_latency_ms metric: {}",
                    e
                );
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!(
                "⚠️  Failed to create websocket_redis_latency_ms histogram: {}",
                e
            );
            None
        }
    };

    // M-419: Kafka consumer lag gauge - measures offset difference between high watermark and current position
    // This metric is critical for detecting when the consumer is falling behind Kafka producers.
    // Lag = high_watermark - current_offset (where current_offset is the NEXT offset to consume)
    let consumer_lag_gauge = match IntGaugeVec::new(
        Opts::new(
            "websocket_kafka_consumer_lag",
            "Kafka consumer lag (high watermark - current offset) by partition",
        ),
        &["partition"],
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register consumer_lag gauge: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create consumer_lag gauge: {}", e);
            None
        }
    };

    // M-437: Lag monitor health metrics
    // Counter for watermark fetch failures (helps detect Kafka connectivity issues)
    let lag_poll_failures = match IntCounter::new(
        "websocket_kafka_lag_poll_failures_total",
        "Total failures fetching Kafka watermarks for lag calculation",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register lag_poll_failures metric: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create lag_poll_failures metric: {}", e);
            None
        }
    };

    // Histogram for lag poll duration (helps detect slow metadata fetches)
    let lag_poll_duration = match HistogramVec::new(
        HistogramOpts::new(
            "websocket_kafka_lag_poll_duration_seconds",
            "Duration of Kafka watermark fetch operations for lag calculation",
        )
        .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["status"], // labels: "success", "error"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register lag_poll_duration metric: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create lag_poll_duration metric: {}", e);
            None
        }
    };

    // Gauge for offset age (time since last offset update per partition)
    // High age indicates stale partitions (possibly after rebalance)
    let offset_age_gauge = match IntGaugeVec::new(
        Opts::new(
            "websocket_kafka_lag_offset_age_seconds",
            "Seconds since last offset update for each partition (high = stale)",
        ),
        &["partition"],
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register offset_age gauge: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create offset_age gauge: {}", e);
            None
        }
    };

    // M-1021: Gauge for partition catch-up phase status
    // Value: 1 when partition is catching up (offset < session_head), 0 when at head
    // Enables alerting on "partition X has been catching up for >N minutes"
    let catchup_phase_gauge = match IntGaugeVec::new(
        Opts::new(
            "websocket_kafka_catchup_phase",
            "Partition catch-up phase status (1=catching up, 0=at head)",
        ),
        &["partition"],
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register catchup_phase gauge: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create catchup_phase gauge: {}", e);
            None
        }
    };

    let prometheus_registry = Arc::new(prometheus_registry);

    // Pre-create counter series for "healthy runs" so Grafana shows 0 instead of "No data"
    // Counters with inc_by(0) just initialize the time series without affecting data
    // NOTE: Do NOT seed histograms with observe(0.0) - that pollutes percentiles!
    // S-25 fix: These are IntCounter (not IntCounterVec), so call inc_by directly.
    if let Some(ref gaps) = sequence_gaps_total {
        gaps.inc_by(0);
    }
    if let Some(ref dups) = sequence_duplicates_total {
        dups.inc_by(0);
    }
    if let Some(ref reorders) = sequence_reorders_total {
        reorders.inc_by(0);
    }
    if let Some(ref dlq_sends) = dlq_sends_total {
        dlq_sends.with_label_values(&["init"]).inc_by(0);
    }
    if let Some(ref dlq_failures) = dlq_send_failures_total {
        dlq_failures.with_label_values(&["init"]).inc_by(0);
    }
    // Removed fake histogram observations
    // Seeding histograms with observe(0.0) corrupts percentile calculations (p50, p95, p99)
    // Counters are safe to seed (inc_by(0) just initializes the series)
    // Histograms should NOT be seeded - let them show "No data" until real observations arrive
    // See: https://prometheus.io/docs/practices/instrumentation/#avoid-missing-metrics
    println!("📊 Pre-created counter series for healthy run visibility (histograms not seeded to preserve accuracy)");

    // 5b. Create Kafka producer for dead-letter queue (Issue #3)
    println!("🔧 Creating Kafka producer for dead-letter queue...");

    // Issue #2: Retry DLQ producer creation with backoff instead of panic
    let (dlq_producer, dlq_producer_create_retries): (FutureProducer, u32) = {
        let mut retry_count: u32 = 0;
        let max_retries: u32 = 5;
        loop {
            // M-413/M-478: Keep security + address-family config consistent.
            let mut config = kafka_security.create_client_config(&kafka_brokers);
            let dlq_send_timeout_ms = get_dlq_send_timeout_ms();
            let dlq_send_timeout_ms_str = dlq_send_timeout_ms.to_string();
            config.set("message.timeout.ms", &dlq_send_timeout_ms_str);
            match config.create() {
                Ok(p) => break (p, retry_count),
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        return Err(format!(
                            "Failed to create DLQ producer after {} retries: {}",
                            max_retries, e
                        )
                        .into());
                    }
                    let backoff_ms = 1000 * (1 << retry_count); // 2s, 4s, 8s, 16s
                    eprintln!(
                        "⚠️  Failed to create DLQ producer (attempt {}/{}): {}",
                        retry_count, max_retries, e
                    );
                    eprintln!("   Retrying in {}ms...", backoff_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    };
    let dlq_producer = Arc::new(dlq_producer);
    println!("✅ DLQ producer created successfully");
    if let Some(ref h) = retry_count_histogram {
        h.with_label_values(&["dlq_producer_create"])
            .observe(dlq_producer_create_retries as f64);
    }
    // S-19: Configurable DLQ concurrency via MAX_CONCURRENT_DLQ_SENDS env var
    let max_concurrent_dlq_sends = get_max_concurrent_dlq_sends();
    let dlq_send_semaphore = Arc::new(Semaphore::new(max_concurrent_dlq_sends));
    println!("   DLQ concurrency limit: {}", max_concurrent_dlq_sends);

    // Create Redis-backed replay buffer
    let redis_url =
        std::env::var(REDIS_URL).unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!("🔗 Configuring Redis replay buffer: {}", redis_url);

    // M-701: Configurable replay buffer memory size via env var
    let replay_buffer_memory_size: usize =
        parse_env_var_with_warning("REPLAY_BUFFER_MEMORY_SIZE", 1000);
    println!("   Memory buffer size: {} messages", replay_buffer_memory_size);

    let replay_buffer_redis_key_prefix = format!("dashstream-replay:{}", resume_namespace);
    let replay_buffer = match ReplayBuffer::new_with_redis(
        replay_buffer_memory_size,
        &redis_url,
        &replay_buffer_redis_key_prefix,
        redis_connection_errors_total.clone(),
        redis_operation_latency.clone(),
    )
    .await
    {
        Ok(buffer) => {
            println!(
                "✅ Replay buffer: Memory ({} msgs) + Redis ({} msgs, TTL from env)",
                replay_buffer_memory_size,
                ReplayBuffer::REDIS_MAX_SEQUENCES
            );
            buffer
        }
        Err(e) => {
            eprintln!("⚠️  Redis connection failed: {}", e);
            eprintln!("   Falling back to memory-only replay buffer");
            ReplayBuffer::new_memory_only(replay_buffer_memory_size)
        }
    };

    // M-488: Connection rate limiting
    let max_connections_per_ip: usize = std::env::var(WEBSOCKET_MAX_CONNECTIONS_PER_IP)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    println!("🔒 Rate limiting: Max {} WebSocket connections per IP", max_connections_per_ip);

    let rejected_connections_metric = match IntCounter::new(
        "websocket_rejected_connections_total",
        "Total WebSocket connections rejected due to rate limiting",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register websocket_rejected_connections_total: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_rejected_connections_total: {}", e);
            None
        }
    };

    let connection_rate_limiter =
        ConnectionRateLimiter::new(max_connections_per_ip, rejected_connections_metric);

    // M-702: Only trust x-forwarded-for when the TCP peer is a configured proxy.
    let trusted_proxy_ips = Arc::new(parse_trusted_proxy_ips_from_env());
    if trusted_proxy_ips.is_empty() {
        println!(
            "🔒 Client IP: x-forwarded-for NOT trusted (set WEBSOCKET_TRUSTED_PROXY_IPS to trust known proxies)"
        );
    } else {
        println!(
            "🔒 Client IP: trusting x-forwarded-for only from {} proxy IP(s) (WEBSOCKET_TRUSTED_PROXY_IPS)",
            trusted_proxy_ips.len()
        );
    }

    // M-684: Resume/replay observability metrics
    // M-781: "legacy" label was documented but never emitted. Thread mode IS the legacy
    // path (single-thread sequence-based resume). Removed misleading "legacy" label.
    let resume_requests_total = match IntCounterVec::new(
        Opts::new(
            "websocket_resume_requests_total",
            "Total resume requests by mode (partition, thread)",
        ),
        &["mode"], // labels: "partition", "thread"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register websocket_resume_requests_total: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_resume_requests_total: {}", e);
            None
        }
    };

    // M-781: "legacy" label was documented but never emitted. Thread mode IS the legacy
    // path (per-thread sequence-based replay). Removed misleading "legacy" label.
    let replay_messages_total = match IntCounterVec::new(
        Opts::new(
            "websocket_replay_messages_total",
            "Total messages replayed by mode (partition, thread)",
        ),
        &["mode"], // labels: "partition", "thread"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register websocket_replay_messages_total: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_replay_messages_total: {}", e);
            None
        }
    };

    let replay_gaps_total = match IntCounterVec::new(
        Opts::new(
            "websocket_replay_gaps_total",
            "Total replay gaps detected by mode (partition, thread)",
        ),
        &["mode"], // labels: "partition", "thread"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register websocket_replay_gaps_total: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_replay_gaps_total: {}", e);
            None
        }
    };

    // M-782: Metric to track total missing message count across all gaps
    // (replay_gaps_total counts gap events; this counts message quantity)
    let replay_gap_messages_total = match IntCounterVec::new(
        Opts::new(
            "websocket_replay_gap_messages_total",
            "Total missing messages in replay gaps by mode (partition, thread)",
        ),
        &["mode"], // labels: "partition", "thread"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register websocket_replay_gap_messages_total: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_replay_gap_messages_total: {}", e);
            None
        }
    };

    let replay_latency_histogram = match HistogramVec::new(
        HistogramOpts::new(
            "websocket_replay_latency_ms",
            "Replay operation latency in milliseconds",
        )
        .buckets(vec![10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 30000.0]),
        &["mode"], // labels: "partition", "thread"
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register websocket_replay_latency_ms: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_replay_latency_ms: {}", e);
            None
        }
    };

    // M-1001: Metric for detecting EventBatches with multiple thread_ids.
    // These batches can cause partial thread replay indexing if not handled properly.
    let multi_thread_batches_total = match IntCounter::new(
        "websocket_multi_thread_batches_total",
        "Count of EventBatch messages containing events from multiple thread_ids",
    ) {
        Ok(m) => {
            if let Err(e) = prometheus_registry.register(Box::new(m.clone())) {
                eprintln!("⚠️  Failed to register websocket_multi_thread_batches_total: {}", e);
                None
            } else {
                Some(m)
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to create websocket_multi_thread_batches_total: {}", e);
            None
        }
    };

    // M-684: Pre-seed resume/replay counters for Grafana visibility
    if let Some(ref m) = resume_requests_total {
        m.with_label_values(&["partition"]).inc_by(0);
        m.with_label_values(&["thread"]).inc_by(0);
    }
    if let Some(ref m) = replay_messages_total {
        m.with_label_values(&["partition"]).inc_by(0);
        m.with_label_values(&["thread"]).inc_by(0);
    }
    if let Some(ref m) = replay_gaps_total {
        m.with_label_values(&["partition"]).inc_by(0);
        m.with_label_values(&["thread"]).inc_by(0);
    }
    // M-782: Pre-seed gap messages metric
    if let Some(ref m) = replay_gap_messages_total {
        m.with_label_values(&["partition"]).inc_by(0);
        m.with_label_values(&["thread"]).inc_by(0);
    }

    let server_state = ServerState {
        tx: tx.clone(),
        metrics: metrics.clone(),
        shutdown_tx: shutdown_tx.clone(),
        prometheus_registry: prometheus_registry.clone(),
        degraded_since: Arc::new(RwLock::new(None)),
        dlq_producer: dlq_producer.clone(),
        client_lag_events: client_lag_events.clone(),
        client_lag_messages: client_lag_messages.clone(),
        replay_buffer,
        //  Expected schema store with file persistence
        expected_schemas: ExpectedSchemaStore::new(Some(std::path::PathBuf::from(
            std::env::var(EXPECTED_SCHEMAS_PATH)
                .unwrap_or_else(|_| ".dashflow/expected_schemas.json".to_string()),
        ))),
        connection_rate_limiter,
        trusted_proxy_ips,
        // M-684: Resume/replay observability metrics
        resume_requests_total,
        replay_messages_total,
        replay_gaps_total,
        // M-782: Gap message count metric
        replay_gap_messages_total,
        replay_latency_histogram,
        resume_namespace: resume_namespace.clone(),
        kafka_topic: kafka_topic.clone(),
        kafka_group_id: group_id.clone(),
        // M-682: Slow client backpressure
        slow_client_disconnects,
        // M-1061: Oversized control frame rejections
        control_oversized_total,
        // M-1062: Invalid JSON parse failures
        control_parse_failures_total,
        slow_client_disconnect_threshold,
        // M-773: Windowed lag tracking
        slow_client_lag_window_secs,
        // M-1019: Expose max payload config for UI config drift detection
        max_payload_bytes,
        // M-1020: Decode error policy (skip vs pause)
        decode_error_policy,
    };

    // P2: Robust /metrics export: gather everything from one registry.
    // Bridge atomic counters (used for /health) into Prometheus via a collector.
    // M-1035: Pass broadcast_tx so collector uses receiver_count() for connected_clients
    // (same source as /health), ensuring both endpoints report consistent values.
    if let Err(e) =
        server_state
            .prometheus_registry
            .register(Box::new(WebsocketServerMetricsCollector::new(
                metrics.clone(),
                server_state.replay_buffer.clone(),
                server_state.tx.clone(),
            )))
    {
        eprintln!(
            "⚠️  Failed to register websocket atomic metrics collector: {}",
            e
        );
    }

    let replay_buffer_for_shutdown = server_state.replay_buffer.clone();

    // 6. Spawn Kafka consumer task with backpressure and shutdown handling
    let mut shutdown_rx_kafka = shutdown_tx.subscribe();
    let decode_errors_clone = decode_errors; // Move ownership (it's an Option)
    let dlq_producer_clone = dlq_producer.clone();
    let kafka_dlq_topic_clone = kafka_dlq_topic.clone();
    let kafka_errors_by_type_clone = kafka_errors_by_type; // Issue #4: Clone for error tracking
    let e2e_latency_histogram_clone = e2e_latency_histogram; // Issue #5: Clone for latency tracking
    let clock_skew_events_clone = clock_skew_events_total; // M-644: Clock skew detection
    let replay_buffer_clone = server_state.replay_buffer.clone(); // Issue #3 Clone for replay buffer

    // Issue #11: Create SequenceValidator for detecting message loss, duplicates, reordering
    let sequence_validator = Arc::new(RwLock::new(SequenceValidator::new()));
    let sequence_validator_clone = sequence_validator.clone();

    // Issue #11: Clone sequence metrics for Kafka consumer task
    let sequence_gaps_clone = sequence_gaps_total;
    let sequence_gap_size_clone = sequence_gap_size_histogram; // M-1115: gap size histogram
    let sequence_duplicates_clone = sequence_duplicates_total;
    let sequence_reorders_clone = sequence_reorders_total;

    // M-1001: Clone multi-thread batch metric for Kafka consumer task
    let multi_thread_batches_clone = multi_thread_batches_total;

    // Issue #13: Clone DLQ metrics for Kafka consumer task
    let dlq_sends_clone = dlq_sends_total;
    let dlq_send_failures_clone = dlq_send_failures_total;
    let dlq_send_semaphore_clone = dlq_send_semaphore.clone();

    // M-1025: Clone payload-missing metric for Kafka consumer task
    let payload_missing_clone = payload_missing_total;

    // M-1014: Clone Kafka config for lazy watermark fetch in consumer task.
    // When we encounter a partition without a session_head_offset (startup fetch failed
    // or partition was created after startup), we spawn a background task to fetch its
    // watermark. This requires Kafka connection details to create a one-off consumer.
    let kafka_security_for_consumer = kafka_security.clone();
    let kafka_brokers_for_consumer = kafka_brokers.clone();
    let kafka_topic_for_consumer = kafka_topic.clone();

    // M-431: Spawn background lag monitoring thread (replaces M-430 async task)
    // Using a dedicated std::thread instead of tokio::spawn because fetch_watermarks() is blocking.
    // This prevents blocking tokio worker threads and avoids runtime starvation.
    // M-642: Clone the main consumer so the lag monitor can read assignment state and
    // garbage-collect revoked partitions.
    let assignment_consumer_for_lag = consumer.clone();
    let consumer_lag_gauge_for_task = consumer_lag_gauge.clone();
    let lag_poll_failures_for_task = lag_poll_failures.clone();
    let lag_poll_duration_for_task = lag_poll_duration.clone();
    let offset_age_gauge_for_task = offset_age_gauge.clone();
    let kafka_topic_for_lag = kafka_topic.clone();
    let lag_check_interval_secs: u64 =
        parse_env_var_with_warning("KAFKA_LAG_CHECK_INTERVAL_SECS", 10u64);

    // M-431: Stale partition threshold (seconds since last offset update)
    // Partitions without updates for this long are considered stale (possibly after rebalance)
    let stale_partition_secs: u64 =
        parse_env_var_with_warning("KAFKA_LAG_STALE_PARTITION_SECS", 60u64);

    // M-482: Clone shutdown flag for lag monitor thread
    let lag_monitor_shutdown_for_thread = lag_monitor_shutdown.clone();

    let lag_monitor_thread = std::thread::Builder::new()
        .name("lag-monitor".to_string())
        .spawn(move || {
            println!(
                "📊 Starting background lag monitor thread (interval: {}s, stale threshold: {}s)...",
                lag_check_interval_secs, stale_partition_secs
            );

            // M-482: Check shutdown flag each iteration instead of infinite loop
            while !lag_monitor_shutdown_for_thread.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_secs(lag_check_interval_secs));

                // M-482: Check again after sleep to respond quickly to shutdown
                if lag_monitor_shutdown_for_thread.load(Ordering::SeqCst) {
                    break;
                }

                // M-642: Lag monitoring must be assignment-aware to avoid false stale alerts after
                // rebalances. If a partition is revoked, stop tracking it and reset its gauges.
                // We only garbage-collect revoked partitions; staleness for still-assigned
                // partitions is meaningful (see M-481).
                let assigned_partitions: Option<HashSet<i32>> =
                    match assignment_consumer_for_lag.assignment() {
                        Ok(tpl) => Some(
                            tpl.elements_for_topic(&kafka_topic_for_lag)
                                .into_iter()
                                .map(|elem| elem.partition())
                                .collect(),
                        ),
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "Lag monitor: failed to read consumer assignment; keeping all tracked partitions"
                            );
                            None
                        }
                    };

                let mut revoked_partitions: Vec<i32> = Vec::new();

                // M-431: Read snapshot of current offsets from shared state (brief lock).
                // If assignment is available, filter to currently assigned partitions and
                // garbage-collect revoked partitions from the tracking map.
                let offsets_snapshot: Vec<(i32, i64, Instant)> = match assigned_partitions.as_ref() {
                    Some(assigned) => match partition_offsets_for_lag_monitor.write() {
                        Ok(mut guard) => {
                            guard.retain(|partition, _| {
                                let keep = assigned.contains(partition);
                                if !keep {
                                    revoked_partitions.push(*partition);
                                }
                                keep
                            });
                            guard.iter().map(|(&p, &(o, t))| (p, o, t)).collect()
                        }
                        Err(poisoned) => {
                            // SAFETY: Lock poisoning means a thread panicked while holding the lock.
                            // We recover by reading/writing through the poison (data may be inconsistent
                            // but lag metrics are non-critical and we'll get fresh data on next tick).
                            tracing::warn!(
                                "Lag monitor: partition offsets lock was poisoned, recovering"
                            );
                            let mut guard = poisoned.into_inner();
                            guard.retain(|partition, _| {
                                let keep = assigned.contains(partition);
                                if !keep {
                                    revoked_partitions.push(*partition);
                                }
                                keep
                            });
                            guard.iter().map(|(&p, &(o, t))| (p, o, t)).collect()
                        }
                    },
                    None => match partition_offsets_for_lag_monitor.read() {
                        Ok(guard) => guard.iter().map(|(&p, &(o, t))| (p, o, t)).collect(),
                        Err(poisoned) => {
                            // SAFETY: Lock poisoning means a thread panicked while holding the lock.
                            // We recover by reading through the poison (data may be inconsistent but
                            // lag metrics are non-critical and we'll get fresh data on next tick).
                            tracing::warn!(
                                "Lag monitor: partition offsets lock was poisoned, recovering"
                            );
                            poisoned.into_inner().iter().map(|(&p, &(o, t))| (p, o, t)).collect()
                        }
                    },
                };

                if !revoked_partitions.is_empty() {
                    tracing::debug!(
                        partitions = ?revoked_partitions,
                        "Lag monitor: garbage-collecting revoked partitions"
                    );
                    for partition in revoked_partitions {
                        let partition_str = partition.to_string();
                        if let Some(ref gauge) = consumer_lag_gauge_for_task {
                            gauge.with_label_values(&[&partition_str]).set(0);
                        }
                        if let Some(ref gauge) = offset_age_gauge_for_task {
                            gauge.with_label_values(&[&partition_str]).set(0);
                        }
                    }
                }

                if offsets_snapshot.is_empty() {
                    continue; // No offsets yet, skip this cycle
                }

                // M-431/M-481: Process each partition and compute lag

                for (partition, current_offset, last_update) in &offsets_snapshot {
                    let age_secs = last_update.elapsed().as_secs();

                    // M-431: Update offset age metric
                    if let Some(ref gauge) = offset_age_gauge_for_task {
                        gauge.with_label_values(&[&partition.to_string()]).set(age_secs as i64);
                    }

                    // M-481: Log stale partitions but CONTINUE to fetch watermarks
                    // CRITICAL: Do NOT skip watermark fetches or zero lag gauges for stale partitions.
                    // A stale partition (no offset updates) may indicate a stuck consumer while
                    // producers continue, meaning LAG IS INCREASING. Zeroing lag masks real outages.
                    // The offset_age metric indicates staleness; alert on it separately.
                    if age_secs > stale_partition_secs {
                        // Log at warn level for visibility - this could indicate a stuck consumer
                        tracing::warn!(
                            partition = partition,
                            age_secs = age_secs,
                            stale_threshold_secs = stale_partition_secs,
                            "Partition is stale (no offset updates) - consumer may be stuck"
                        );
                        // M-481: Continue processing - do NOT skip watermark fetch
                    }

                    // M-431: Fetch watermarks with timing
                    let start = Instant::now();
                    let result = metadata_consumer.fetch_watermarks(
                        &kafka_topic_for_lag,
                        *partition,
                        Duration::from_secs(1),
                    );
                    let elapsed = start.elapsed();

                    match result {
                        Ok((_low, high)) => {
                            let lag = high.saturating_sub(*current_offset).max(0);

                            if let Some(ref gauge) = consumer_lag_gauge_for_task {
                                gauge.with_label_values(&[&partition.to_string()]).set(lag);
                            }

                            // M-437: Record successful poll duration
                            if let Some(ref histogram) = lag_poll_duration_for_task {
                                histogram.with_label_values(&["success"]).observe(elapsed.as_secs_f64());
                            }

                            // Log lag only if significant (>1000 messages behind)
                            if lag > 1000 {
                                tracing::warn!(
                                    partition = partition,
                                    current_offset = current_offset,
                                    high_watermark = high,
                                    lag = lag,
                                    "Consumer lag is high"
                                );
                            }
                        }
                        Err(e) => {
                            // M-437: Record poll failure
                            if let Some(ref counter) = lag_poll_failures_for_task {
                                counter.inc();
                            }
                            if let Some(ref histogram) = lag_poll_duration_for_task {
                                histogram.with_label_values(&["error"]).observe(elapsed.as_secs_f64());
                            }

                            // Don't spam logs - watermark fetch failures are common during rebalances
                            tracing::debug!(
                                partition = partition,
                                error = %e,
                                "Failed to fetch watermarks for lag calculation"
                            );
                        }
                    }
                }

                // M-481: REMOVED stale partition cleanup block
                // Do NOT remove stale partitions from tracking or zero their gauges.
                // A stale partition may indicate a stuck consumer while producers continue,
                // meaning lag is real and increasing. Alert on:
                //   - max(websocket_kafka_consumer_lag) > threshold (existing lag alert)
                //   - max(websocket_kafka_lag_offset_age_seconds) > threshold (staleness alert)
            }

            // M-482: Log clean exit
            println!("📊 Lag monitor thread shutting down gracefully");
        })
        .expect("Failed to spawn lag monitor thread");

    // M-431: Consume cloned variables to prevent unused warnings
    let _ = consumer_lag_gauge; // Cloned above for the thread
    let _ = lag_poll_failures; // Cloned above for the thread
    let _ = lag_poll_duration; // Cloned above for the thread
    let _ = offset_age_gauge; // Cloned above for the thread

    // M-1021: Clone catchup_phase_gauge for use in kafka consumer task
    let catchup_phase_gauge_for_consumer = catchup_phase_gauge.clone();
    let _ = catchup_phase_gauge; // Consumed - use catchup_phase_gauge_for_consumer in consumer task

    let kafka_consumer_task = tokio::spawn(async move {
        println!("📡 Starting Kafka consumer loop...");
        let mut msg_count = 0;
        // M-1027: Use lag_events for backpressure (not dropped_messages)
        // Each lag event increments counter by 1, regardless of messages dropped.
        // This provides stable backpressure behavior regardless of client count.
        // Threshold: 10 lag events/sec by default - adjust via future env var if needed
        let backpressure_threshold = 10u64;
        let mut last_lag_events = 0u64;
        let mut last_backpressure_check = Instant::now();

        // M-994/M-1003: "Old data" classification for decode errors (OFFSET-BASED, not timestamp-based).
        // If we are replaying history (auto.offset.reset=earliest), decode failures from messages
        // produced long before this server started are expected after schema changes.
        //
        // CRITICAL: We classify old data by OFFSET, not timestamp. Timestamp-based classification
        // is vulnerable to clock skew: producers with old clocks or backfilled data would have
        // their decode errors silently suppressed even though they're NEW messages at the head
        // of the topic. This hides real failures from alerting.
        //
        // M-1003: Improved offset-based approach using session-head watermarks:
        // At startup, we fetched the high watermark (head offset) for each partition.
        // Messages with offset < session_head_offset are in the "catch-up phase" (old data).
        // Messages with offset >= session_head_offset are "new data" at the topic head.
        // This correctly classifies initial catch-up as old data, preventing alert spam.
        //
        // Fallback: For partitions where we couldn't fetch watermarks at startup, we use
        // first-seen offset (original M-994 behavior) as a fallback. This still works for
        // rewind/replay scenarios, just not for initial catch-up.
        //
        // Set KAFKA_OLD_DATA_USE_TIMESTAMP=true to revert to timestamp-based (legacy) behavior.
        let use_timestamp_classification =
            std::env::var(KAFKA_OLD_DATA_USE_TIMESTAMP).map_or(false, |v| v == "true" || v == "1");
        let session_start_ms = Utc::now().timestamp_millis();
        let old_data_grace_ms: i64 =
            parse_env_var_with_warning("KAFKA_OLD_DATA_GRACE_SECONDS", 30i64).saturating_mul(1000);
        // M-1003: Track catch-up completion per partition (for logging when we reach head)
        let mut catch_up_completed: std::collections::HashSet<i32> = std::collections::HashSet::new();
        // M-994: Fallback first-seen offsets for partitions without session-head watermark
        let mut first_seen_offsets: std::collections::HashMap<i32, i64> = std::collections::HashMap::new();
        // M-1014: Track partitions for which we've initiated a lazy watermark fetch.
        // This prevents spamming fetch requests for the same partition.
        let mut lazy_watermark_fetch_initiated: std::collections::HashSet<i32> = std::collections::HashSet::new();

        // M-431: Lag monitoring uses shared HashMap - write offset updates directly (O(1), very fast)

        loop {
            tokio::select! {
                // Listen for shutdown signal
                _ = shutdown_rx_kafka.recv() => {
                    println!("📡 Kafka consumer received shutdown signal, stopping...");
                    if let Err(e) = consumer.commit_consumer_state(CommitMode::Sync) {
                        eprintln!("⚠️  Kafka commit_consumer_state failed during shutdown: {}", e);
                    }
                    break;
                }
                // Process Kafka messages
                kafka_result = consumer.recv() => {
            match kafka_result {
                Ok(msg) => {
                    msg_count += 1;
                    let topic = msg.topic().to_string();
                    let partition = msg.partition();
                    let offset = msg.offset();

                    if let Some(payload) = msg.payload() {
                        // M-994/M-1003: Offset-based old-data classification (default).
                        // M-1003: Use session-head watermarks for proper catch-up detection.
                        // If we have a session_head_offset for this partition (fetched at startup),
                        // use it. Otherwise fall back to first-seen offset (M-994 fallback).
                        let session_head_offset: Option<i64> = session_head_offsets_for_consumer
                            .read()
                            .ok()
                            .and_then(|guard| guard.get(&partition).copied());

                        // M-1014: Lazy watermark fetch for partitions missing session_head_offset.
                        // If we don't have a watermark for this partition (startup fetch failed or
                        // partition was created after startup) AND we haven't started a fetch yet,
                        // spawn a background task to fetch it. This provides proper catch-up
                        // classification even for dynamically discovered partitions.
                        if session_head_offset.is_none()
                            && !lazy_watermark_fetch_initiated.contains(&partition)
                        {
                            lazy_watermark_fetch_initiated.insert(partition);
                            tracing::info!(
                                partition = partition,
                                "M-1014: Initiating lazy watermark fetch for partition without session_head"
                            );

                            // Clone what we need for the background task
                            let session_head_offsets_for_lazy = session_head_offsets_for_consumer.clone();
                            let kafka_security_for_lazy = kafka_security_for_consumer.clone();
                            let kafka_brokers_for_lazy = kafka_brokers_for_consumer.clone();
                            let kafka_topic_for_lazy = kafka_topic_for_consumer.clone();
                            let partition_for_lazy = partition;

                            // Spawn blocking task to fetch watermark without blocking the consumer
                            tokio::task::spawn_blocking(move || {
                                // Create a lightweight consumer just for this fetch
                                let mut config =
                                    kafka_security_for_lazy.create_client_config(&kafka_brokers_for_lazy);
                                config
                                    .set("group.id", format!("lazy-watermark-{}", partition_for_lazy))
                                    .set("enable.auto.commit", "false");

                                let consumer: Result<rdkafka::consumer::BaseConsumer, _> = config.create();
                                match consumer {
                                    Ok(c) => {
                                        match c.fetch_watermarks(
                                            &kafka_topic_for_lazy,
                                            partition_for_lazy,
                                            Duration::from_secs(5),
                                        ) {
                                            Ok((_low, high)) => {
                                                // Store the watermark for future use
                                                if let Ok(mut guard) = session_head_offsets_for_lazy.write() {
                                                    guard.insert(partition_for_lazy, high);
                                                    tracing::info!(
                                                        partition = partition_for_lazy,
                                                        high_watermark = high,
                                                        "M-1014: Lazy watermark fetch succeeded"
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                // Fetch failed - will continue using first-seen fallback
                                                tracing::warn!(
                                                    partition = partition_for_lazy,
                                                    error = %e,
                                                    "M-1014: Lazy watermark fetch failed; using first-seen fallback"
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            partition = partition_for_lazy,
                                            error = %e,
                                            "M-1014: Failed to create consumer for lazy watermark fetch"
                                        );
                                    }
                                }
                            });
                        }

                        // Track first-seen offset as fallback for partitions without session-head
                        let is_first_for_partition = !first_seen_offsets.contains_key(&partition);
                        if is_first_for_partition {
                            first_seen_offsets.insert(partition, offset);
                            // M-1021: Set catch-up gauge to 1 when partition is first seen
                            // (assume catching up until proven otherwise)
                            if let Some(ref gauge) = catchup_phase_gauge_for_consumer {
                                gauge.with_label_values(&[&partition.to_string()]).set(1);
                            }
                        }
                        let first_seen_offset = first_seen_offsets.get(&partition).copied().unwrap_or(offset);

                        // Determine the reference offset for old-data classification:
                        // - If session_head_offset is available: catch-up = offset < session_head
                        // - Otherwise: rewind/replay = offset < first_seen (M-994 fallback)
                        let (reference_offset, using_session_head) = match session_head_offset {
                            Some(head) => (head, true),
                            None => (first_seen_offset, false),
                        };

                        // Timestamp-based classification (legacy, opt-in via KAFKA_OLD_DATA_USE_TIMESTAMP)
                        let is_old_by_timestamp = match msg.timestamp() {
                            KafkaTimestamp::CreateTime(ms) | KafkaTimestamp::LogAppendTime(ms)
                                if ms >= 0 =>
                            {
                                ms < session_start_ms.saturating_sub(old_data_grace_ms)
                            }
                            _ => false,
                        };

                        // M-1003: Offset-based classification using session-head or first-seen fallback
                        let is_old_by_offset = offset < reference_offset;

                        // M-1003: Log when catch-up phase completes for a partition (once per partition)
                        // M-1021: Update catch-up gauge when transitioning to head
                        if using_session_head && !is_old_by_offset && !catch_up_completed.contains(&partition) {
                            catch_up_completed.insert(partition);
                            // M-1021: Set catch-up gauge to 0 (now at head, no longer catching up)
                            if let Some(ref gauge) = catchup_phase_gauge_for_consumer {
                                gauge.with_label_values(&[&partition.to_string()]).set(0);
                            }
                            tracing::info!(
                                partition = partition,
                                offset = offset,
                                session_head = reference_offset,
                                "Partition catch-up complete; now at topic head (decode errors are now 'new data')"
                            );
                        }

                        // M-994: Detect suspicious case where timestamp says "old" but offset says "new"
                        // This indicates clock skew or backfilled data that would have been incorrectly suppressed
                        if is_old_by_timestamp && !is_old_by_offset && !use_timestamp_classification {
                            tracing::warn!(
                                partition = partition,
                                offset = offset,
                                "Message has old timestamp but is at current offset; decode errors will NOT be suppressed (M-994)"
                            );
                        }

                        // Use offset-based by default; timestamp-based only if explicitly enabled
                        let is_old_data = if use_timestamp_classification {
                            is_old_by_timestamp
                        } else {
                            is_old_by_offset
                        };

                        // Issue #14: Extract trace context from Kafka headers
                        let extractor = KafkaHeaderExtractor::from_kafka_message(&msg);
                        let propagator = TraceContextPropagator::new();
                        let parent_context = propagator.extract(&extractor);

                        // Create a span as child of the producer span
                        let span = info_span!(
                            "process_kafka_message",
                            partition = partition,
                            offset = offset,
                        );
                        let _ = span.set_parent(parent_context);
                        let _enter = span.enter();

                        // M-1024: Check payload size BEFORE cloning to avoid allocating oversized payloads.
                        // Previously, payload.to_vec() happened first, then decode_message_compatible
                        // enforced the limit - but the allocation had already occurred. This caused
                        // memory amplification for malicious/corrupted oversized messages.
                        if payload.len() > max_payload_bytes {
                            // Handle oversized payload as decode error without expensive allocations
                            let error_type = "payload_too_large";

                            // M-1085 FIX: Count in windowed messages so decode error rate denominator is correct.
                            // Without this, oversized payloads weren't counted as "messages received" which
                            // made the windowed decode error rate artificially high.
                            metrics_clone.kafka_messages_received.fetch_add(1, Ordering::Relaxed);
                            metrics_clone.note_kafka_message();
                            metrics_clone.record_message_received();

                            if is_old_data {
                                metrics_clone
                                    .old_data_decode_errors
                                    .fetch_add(1, Ordering::Relaxed);
                                tracing::debug!(
                                    partition = partition,
                                    offset = offset,
                                    payload_size = payload.len(),
                                    max_payload_bytes = max_payload_bytes,
                                    "Skipping oversized old-data payload (M-1024)"
                                );
                            } else {
                                // Count as decode error
                                metrics_clone.kafka_errors.fetch_add(1, Ordering::Relaxed);
                                metrics_clone.kafka_messages_error.fetch_add(1, Ordering::Relaxed);
                                metrics_clone.decode_errors.fetch_add(1, Ordering::Relaxed);
                                // M-1069: Track in sliding window for windowed error rate calculation
                                metrics_clone.record_decode_error();

                                // Update Prometheus metric
                                if let Some(ref de) = decode_errors_clone {
                                    de.with_label_values(&[error_type]).inc();
                                }

                                tracing::warn!(
                                    partition = partition,
                                    offset = offset,
                                    payload_size = payload.len(),
                                    max_payload_bytes = max_payload_bytes,
                                    "Rejecting oversized payload before allocation (M-1024)"
                                );

                                // M-1024: Send a truncated DLQ record for forensics WITHOUT base64-encoding
                                // the full oversized payload. Include first/last bytes for debugging.
                                // Use same hex format as DecodeErrorLog (format!("{:02x?}", ...))
                                let first_bytes_hex = if payload.len() >= 16 {
                                    format!("{:02x?}", &payload[..16])
                                } else {
                                    format!("{:02x?}", payload)
                                };
                                let last_bytes_hex = if payload.len() >= 16 {
                                    format!("{:02x?}", &payload[payload.len() - 16..])
                                } else {
                                    String::new()
                                };

                                // M-1064: Compute SHA256 hash for content verification (no allocation needed)
                                let payload_sha256 = {
                                    let mut hasher = Sha256::new();
                                    hasher.update(payload);
                                    format!("{:x}", hasher.finalize())
                                };

                                let trace_id = uuid::Uuid::new_v4().to_string();
                                let dlq_message = serde_json::json!({
                                    "trace_id": trace_id,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                    "error": format!("Payload size {} exceeds maximum {}", payload.len(), max_payload_bytes),
                                    "error_type": error_type,
                                    // M-1064: Include SHA256 hash for content verification
                                    "payload_sha256": payload_sha256,
                                    "original_payload_base64": null,  // M-1024: Omit full payload for oversized messages
                                    "payload_truncation_note": format!(
                                        "Payload too large ({} bytes); not base64-encoded to prevent memory amplification. SHA256: {}. DLQ_INCLUDE_FULL_PAYLOAD does not apply to oversized payloads.",
                                        payload.len(), payload_sha256
                                    ),
                                    "message_size": payload.len(),
                                    "first_bytes_hex": first_bytes_hex,
                                    "last_bytes_hex": last_bytes_hex,
                                    "kafka_partition": partition,
                                    "kafka_offset": offset,
                                    "message_count": msg_count,
                                });

                                let dlq_payload = dlq_message.to_string();
                                let trace_id_for_key = trace_id.clone();

                                let dlq_sends_metric = dlq_sends_clone.clone();
                                let dlq_send_failures_metric = dlq_send_failures_clone.clone();

                                let permit = match dlq_send_semaphore_clone
                                    .clone()
                                    .try_acquire_owned()
                                {
                                    Ok(p) => Some(p),
                                    Err(_) => {
                                        if let Some(ref metric) = dlq_send_failures_metric {
                                            metric.with_label_values(&["backpressure"]).inc();
                                        }
                                        tracing::warn!("DLQ backpressure limit reached for oversized payload");
                                        None
                                    }
                                };

                                if let Some(permit) = permit {
                                    let dlq_producer_for_send = dlq_producer_clone.clone();
                                    let dlq_topic_for_send = kafka_dlq_topic_clone.clone();
                                    let error_type_for_metric = error_type.to_string();
                                    tokio::spawn(async move {
                                        let _permit = permit;
                                        let dlq_record = FutureRecord::to(&dlq_topic_for_send)
                                            .payload(&dlq_payload)
                                            .key(&trace_id_for_key);

                                        match dlq_producer_for_send
                                            .send(dlq_record, tokio::time::Duration::from_secs(5))
                                            .await
                                        {
                                            Ok(_) => {
                                                if let Some(ref metric) = dlq_sends_metric {
                                                    metric.with_label_values(&[&error_type_for_metric]).inc();
                                                }
                                            }
                                            Err((e, _)) => {
                                                // M-1104 FIX: Use failure reason (timeout/kafka_error), not error_type
                                                let failure_reason = if e.to_string().contains("timed out") {
                                                    "timeout"
                                                } else {
                                                    "kafka_error"
                                                };
                                                if let Some(ref metric) = dlq_send_failures_metric {
                                                    metric.with_label_values(&[failure_reason]).inc();
                                                }
                                                tracing::error!("Failed to send oversized payload to DLQ: {}", e);
                                            }
                                        }
                                    });
                                }
                            }

                            // M-1096: Apply decode error policy consistently for oversized payloads.
                            // Previously this path always continued (skip) regardless of policy.
                            if decode_error_policy == DecodeErrorPolicy::Pause {
                                // Pause mode: Stop consuming to prevent data loss.
                                // DO NOT store offset - next restart will retry this message.
                                eprintln!("🛑 KAFKA_ON_DECODE_ERROR=pause: Stopping Kafka consumption due to oversized payload");
                                eprintln!("   Partition: {}, Offset: {}", partition, offset);
                                eprintln!("   Payload size: {} bytes (max: {})", payload.len(), max_payload_bytes);
                                eprintln!("   Offset NOT stored - restart will retry this message");
                                eprintln!("   WebSocket connections remain active; no new data will be streamed");
                                eprintln!("   Fix the issue and restart the server to resume");

                                // Enter paused state: loop until shutdown
                                let mut last_pause_log = Instant::now();
                                loop {
                                    tokio::select! {
                                        _ = shutdown_rx_kafka.recv() => {
                                            println!("📤 Kafka consumer (PAUSED) received shutdown signal");
                                            break;
                                        }
                                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                                            // Log every 30 seconds so operators know we're still paused
                                            if last_pause_log.elapsed() >= Duration::from_secs(30) {
                                                eprintln!(
                                                    "⏸️  Kafka consumer PAUSED at partition {} offset {} due to oversized payload (KAFKA_ON_DECODE_ERROR=pause)",
                                                    partition, offset
                                                );
                                                last_pause_log = Instant::now();
                                            }
                                        }
                                    }
                                }
                                // After shutdown, exit the consumer loop entirely
                                break;
                            }

                            // Skip mode (default): Skip to next message - do NOT call payload.to_vec()
                            continue;
                        }

                        // Forward raw binary protobuf payload to WebSocket clients
                        // React will decode using protobufjs
                        // M-995/M-996: Convert to Bytes once, then share between broadcast and replay buffer.
                        // Bytes::clone() is O(1) (atomic ref-count bump) vs O(n) for Vec<u8>.
                        let binary_data: Bytes = Bytes::from(payload.to_vec());

                        // Update metrics BEFORE decode attempt
                        // This counts "messages received from Kafka" not "messages successfully decoded"
                        // Decode errors are tracked separately in decode_errors metric
                        metrics_clone.kafka_messages_received.fetch_add(1, Ordering::Relaxed);
                        metrics_clone.note_kafka_message();
                        // M-1069: Track in sliding window for windowed error rate calculation
                        metrics_clone.record_message_received();

                        // Verify it's valid protobuf (optional logging)
                        // Prefer strict decoding (framed header required), with a legacy fallback
                        // for older unframed messages that may still exist in Kafka.
                        match dashflow_streaming::codec::decode_message_compatible(
                            payload,
                            max_payload_bytes,
                        ) {
                            Ok(decoded_msg) => {
                                metrics_clone
                                    .kafka_messages_success
                                    .fetch_add(1, Ordering::Relaxed);
                                // M-980: Avoid per-message stdout logging in the hot path; rely on metrics.
                                // Emit a small rate-limited log for debugging/visibility.
                                if msg_count % 1000 == 0 {
                                    tracing::info!(
                                        message_count = msg_count,
                                        payload_bytes = binary_data.len(),
                                        "Forwarding Kafka payload to WebSocket clients"
                                    );
                                }

                                    let mut max_clients_sent: Option<usize> = None;

                                    // M-674: Attach Kafka (partition, offset) cursor to every forwarded message.
                                    // This supports catching up threads that started while the UI was offline.
                                    let cursor = KafkaCursor { partition, offset };

                                    // M-1001: Track thread sequences as HashMap to support multi-thread EventBatches.
                                    // All messages have exactly one (thread_id, sequence) for replay indexing,
                                    // but EventBatch may contain events from multiple threads.
                                    let (thread_id, sequence, thread_sequences) = match decoded_msg.message.as_ref() {
                                        Some(dashflow_streaming::dash_stream_message::Message::EventBatch(batch)) => {
                                            // M-1001: Collect ALL unique thread_ids and their max sequences.
                                            // Previously only captured first thread_id, losing others.
                                            let mut thread_max_sequences: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

                                            for (idx, event) in batch.events.iter().enumerate() {
                                                let Some(header) = event.header.as_ref() else {
                                                    tracing::warn!(
                                                        event_index = idx,
                                                        "EventBatch event missing header; skipping sequence validation"
                                                    );
                                                    continue;
                                                };

                                                // M-736: Warn when inner event has sequence=0 (bypasses gap detection)
                                                // Batch headers intentionally use sequence=0, but inner events should have real sequences
                                                if header.sequence == 0 {
                                                    tracing::warn!(
                                                        event_index = idx,
                                                        thread_id = %header.thread_id,
                                                        "EventBatch inner event has sequence=0; gap detection will be bypassed"
                                                    );
                                                }

                                                let (t, s) = process_dashstream_header(
                                                    header,
                                                    &sequence_validator_clone,
                                                    &sequence_gaps_clone,
                                                    &sequence_gap_size_clone, // M-1115
                                                    &sequence_duplicates_clone,
                                                    &sequence_reorders_clone,
                                                    &e2e_latency_histogram_clone,
                                                    &clock_skew_events_clone,
                                                )
                                                .await;

                                                // M-1001: Track max sequence per thread_id instead of single global max
                                                if let (Some(tid), Some(seq)) = (t, s) {
                                                    thread_max_sequences
                                                        .entry(tid)
                                                        .and_modify(|max| *max = (*max).max(seq))
                                                        .or_insert(seq);
                                                }
                                            }

                                            // M-1001: Detect and count multi-thread batches
                                            if thread_max_sequences.len() > 1 {
                                                if let Some(ref m) = multi_thread_batches_clone {
                                                    m.inc();
                                                }
                                                tracing::warn!(
                                                    thread_count = thread_max_sequences.len(),
                                                    threads = ?thread_max_sequences.keys().collect::<Vec<_>>(),
                                                    partition = partition,
                                                    offset = offset,
                                                    "EventBatch contains multiple thread_ids; all will be indexed for replay"
                                                );
                                            }

                                            // For backwards compatibility, return first thread_id and its max seq
                                            // The full thread_sequences map is used for complete indexing below
                                            let (first_thread, first_seq) = thread_max_sequences
                                                .iter()
                                                .next()
                                                .map(|(t, s)| (Some(t.clone()), Some(*s)))
                                                .unwrap_or((None, None));

                                            (first_thread, first_seq, Some(thread_max_sequences))
                                        }
                                        _ => {
                                            // Non-EventBatch message: extract single header
                                            let header_opt: Option<&dashflow_streaming::Header> =
                                                match decoded_msg.message.as_ref() {
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::Event(
                                                            e,
                                                        ),
                                                    ) => e.header.as_ref(),
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::TokenChunk(
                                                            t,
                                                        ),
                                                    ) => t.header.as_ref(),
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::StateDiff(
                                                            s,
                                                        ),
                                                    ) => s.header.as_ref(),
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::ToolExecution(
                                                            te,
                                                        ),
                                                    ) => te.header.as_ref(),
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::Checkpoint(
                                                            c,
                                                        ),
                                                    ) => c.header.as_ref(),
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::Metrics(
                                                            m,
                                                        ),
                                                    ) => m.header.as_ref(),
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::Error(
                                                            err,
                                                        ),
                                                    ) => err.header.as_ref(),
                                                    Some(
                                                        dashflow_streaming::dash_stream_message::Message::ExecutionTrace(
                                                            trace,
                                                        ),
                                                    ) => trace.header.as_ref(),
                                                    _ => None,
                                                };

                                            // Non-EventBatch: single thread, no multi-thread map
                                            let (tid, seq) = if let Some(header) = header_opt {
                                                process_dashstream_header(
                                                    header,
                                                    &sequence_validator_clone,
                                                    &sequence_gaps_clone,
                                                    &sequence_gap_size_clone, // M-1115
                                                    &sequence_duplicates_clone,
                                                    &sequence_reorders_clone,
                                                    &e2e_latency_histogram_clone,
                                                    &clock_skew_events_clone,
                                                )
                                                .await
                                            } else {
                                                (None, None)
                                            };
                                            (tid, seq, None) // No multi-thread map for non-batch
                                        }
                                    };

                                    if let Ok(n) = tx_clone.send(OutboundBinaryMessage {
                                        data: binary_data.clone(),
                                        cursor,
                                    }) {
                                        max_clients_sent = Some(max_clients_sent.map_or(n, |m| m.max(n)));
                                    }

                                    // M-1001: Pass thread_sequences for multi-thread EventBatch indexing
                                    replay_buffer_clone
                                        .add_message(binary_data.clone(), thread_id, sequence, partition, offset, thread_sequences)
                                        .await;

                                    // M-980: Rate-limit client count logging; stdout per message is a throughput killer.
                                    if msg_count % 1000 == 0 {
                                        match max_clients_sent {
                                            Some(n) => tracing::debug!(
                                                message_count = msg_count,
                                                websocket_clients = n,
                                                "Sent message to WebSocket clients"
                                            ),
                                            None => tracing::debug!(
                                                message_count = msg_count,
                                                "No WebSocket clients connected"
                                            ),
                                        }
                                    }

                                // M-1027: Backpressure mechanism using lag_events (not dropped_messages)
                                // This provides stable behavior regardless of client count - each lag
                                // event contributes 1, not N×M where N=clients and M=messages dropped.
                                if max_clients_sent.is_some() {
                                    let window_elapsed = last_backpressure_check.elapsed();
                                    if window_elapsed >= std::time::Duration::from_secs(1) {
                                        let lag_events_total =
                                            metrics_clone.lag_events.load(Ordering::Relaxed);
                                        let lag_events_window = lag_events_total
                                            .saturating_sub(last_lag_events);

                                        if lag_events_window > backpressure_threshold {
                                            println!(
                                                "   ⏸️  BACKPRESSURE: {} lag events in {:?} (> {} threshold), slowing Kafka consumption...",
                                                lag_events_window, window_elapsed, backpressure_threshold
                                            );
                                            tokio::time::sleep(tokio::time::Duration::from_millis(100))
                                                .await;
                                        }

                                        last_lag_events = lag_events_total;
                                        last_backpressure_check = Instant::now();
                                    }
                                }
                            }
                            Err(e) => {
                                // Graceful decode error handling: separate old data from new data
                                // Old data errors (offset < session_start_offset) are expected when
                                // auto.offset.reset=earliest reads messages from before schema changes.
                                // These should be counted separately and not trigger alerts.

                                if is_old_data {
                                    // Old data decode error - expected, just count and skip
                                    let old_data_count = metrics_clone
                                        .old_data_decode_errors
                                        .fetch_add(1, Ordering::Relaxed) + 1;
                                    // M-1093 FIX: Rate-limit logging to prevent spam during catch-up
                                    // Log first 3 errors, then every 100th error to avoid log flood
                                    if old_data_count <= 3 || old_data_count % 100 == 0 {
                                        println!(
                                            "⏭️  Old data decode error #{} at offset {} (message predates session start): {}",
                                            old_data_count, offset, e
                                        );
                                    }
                                } else {
                                    // NEW data decode error - this is a real problem that needs attention
                                    // Issue #17: Structured JSON logging for root cause analysis

                                // Classify error type for Prometheus labels and pattern analysis
                                let error_string = e.to_string();
                                let error_type = if error_string.contains("exceeds maximum") {
                                    // M-979: Make payload-too-large visible as a first-class metric label.
                                    "payload_too_large"
                                } else if error_string.contains("buffer underflow") {
                                    "buffer_underflow"
                                } else if error_string.contains("schema") || error_string.contains("version") {
                                    "schema_version_mismatch"
                                } else if error_string.contains("invalid") {
                                    "invalid_protobuf"
                                } else {
                                    "unknown_decode_error"
                                };

                                // Extract Kafka metadata for correlation
                                let kafka_partition = partition;
                                let kafka_offset = offset;

                                // Create structured log entry with correlation ID
                                let log_entry = DecodeErrorLog::new(
                                    error_string.clone(),
                                    error_type.to_string(),
                                    &binary_data,
                                    msg_count,
                                    Some(kafka_partition),
                                    Some(kafka_offset),
                                );

                                // Log as JSON (machine-readable for log aggregation)
                                log_entry.log_json();

                                // Also log human-readable version for console debugging
                                eprintln!("❌ Decode error #{} (trace_id: {}): {} [type: {}, partition: {}, offset: {}]",
                                    msg_count, log_entry.trace_id, e, error_type, kafka_partition, kafka_offset);

                                // Update error metrics (atomic) - only for NEW data errors
                                metrics_clone.kafka_errors.fetch_add(1, Ordering::Relaxed);
                                metrics_clone
                                    .kafka_messages_error
                                    .fetch_add(1, Ordering::Relaxed);
                                metrics_clone.decode_errors.fetch_add(1, Ordering::Relaxed);
                                // M-1069: Track in sliding window for windowed error rate calculation
                                metrics_clone.record_decode_error();

                                // Update Prometheus metric with error type label (Issue #15)
                                // Issue #2: Only update if metric was successfully created
                                if let Some(ref de) = decode_errors_clone {
                                    de.with_label_values(&[error_type]).inc();
                                }

                                // Issue #3: Write to dead-letter queue for forensic analysis
                                // M-1064: By default, omit full base64 to prevent secret leakage
                                // and avoid exceeding Kafka message size limits. Include SHA256
                                // hash for content verification and first/last bytes for forensics.
                                let payload_sha256 = {
                                    let mut hasher = Sha256::new();
                                    hasher.update(&binary_data);
                                    format!("{:x}", hasher.finalize())
                                };

                                // M-1064: Only include full base64 if explicitly enabled
                                let include_full_payload = get_dlq_include_full_payload();
                                let original_payload_base64: Option<String> = if include_full_payload {
                                    Some(BASE64.encode(&binary_data))
                                } else {
                                    None
                                };

                                // Issue #13: Attempt to extract thread_id/tenant_id even from malformed protobuf
                                // Try partial decode of Event header for forensics (best-effort)
                                let (extracted_thread_id, extracted_tenant_id) =
                                    match <Event as ProstMessage>::decode(&binary_data[..]) {
                                        Ok(event) => {
                                            if let Some(header) = event.header {
                                                (Some(header.thread_id), Some(header.tenant_id))
                                            } else {
                                                (None, None)
                                            }
                                        }
                                        Err(_) => {
                                            // Even header decode failed - no forensic thread_id/tenant_id available
                                            (None, None)
                                        }
                                    };

                                // M-1064: Create truncation note only when payload is omitted
                                let payload_truncation_note: Option<String> = if include_full_payload {
                                    None
                                } else {
                                    Some(format!(
                                        "Full payload omitted for security. SHA256: {}. Set DLQ_INCLUDE_FULL_PAYLOAD=true to include.",
                                        payload_sha256
                                    ))
                                };

                                let dlq_message = json!({
                                    "trace_id": log_entry.trace_id,
                                    "timestamp": log_entry.timestamp,
                                    "error": e.to_string(),
                                    "error_type": error_type,
                                    // M-1064: Include SHA256 hash for content verification
                                    "payload_sha256": payload_sha256,
                                    // M-1064: Only include full base64 if DLQ_INCLUDE_FULL_PAYLOAD=true
                                    "original_payload_base64": original_payload_base64,
                                    // M-1064: Include warning when full payload is omitted
                                    "payload_truncation_note": payload_truncation_note,
                                    "message_size": log_entry.message_size,
                                    "first_bytes_hex": log_entry.first_bytes_hex,
                                    "last_bytes_hex": log_entry.last_bytes_hex,
                                    "kafka_partition": kafka_partition,
                                    "kafka_offset": kafka_offset,
                                    "message_count": msg_count,
                                    "thread_id": extracted_thread_id,  // Issue #13: Best-effort extraction
                                    "tenant_id": extracted_tenant_id,  // Issue #13: Best-effort extraction
                                });

                                let dlq_payload = dlq_message.to_string();
                                let trace_id_for_key = log_entry.trace_id.clone();

                                    // Issue #13: Clone DLQ metrics for spawn task
                                    let dlq_sends_metric = dlq_sends_clone.clone();
                                    let dlq_send_failures_metric = dlq_send_failures_clone.clone();
                                    let error_type_for_metric = error_type.to_string();

                                    let permit = match dlq_send_semaphore_clone
                                        .clone()
                                        .try_acquire_owned()
                                    {
                                        Ok(p) => Some(p),
                                        Err(_) => {
                                            if let Some(ref metric) = dlq_send_failures_metric {
                                                metric.with_label_values(&["backpressure"]).inc();
                                            }
                                            eprintln!(
                                                "⚠️  DLQ backpressure limit reached, dropping DLQ message"
                                            );
                                            None
                                        }
                                    };

                                    if let Some(permit) = permit {
                                        let dlq_producer_for_send = dlq_producer_clone.clone();
                                        let dlq_topic_for_send = kafka_dlq_topic_clone.clone();
                                        tokio::spawn(async move {
                                            let _permit = permit; // Keep permit alive until send completes

                                            let dlq_record = FutureRecord::to(&dlq_topic_for_send)
                                                .payload(&dlq_payload)
                                                .key(&trace_id_for_key);

                                            match dlq_producer_for_send
                                                .send(dlq_record, tokio::time::Duration::from_secs(5))
                                                .await
                                            {
                                                Ok(_) => {
                                                    // Issue #13: Increment DLQ success metric
                                                    if let Some(ref metric) = dlq_sends_metric {
                                                        metric.with_label_values(&[&error_type_for_metric]).inc();
                                                    }
                                                }
                                                Err((e, _)) => {
                                                    eprintln!("⚠️  Failed to publish to DLQ: {}", e);
                                                    // Issue #13: Increment DLQ failure metric
                                                    let failure_reason = if e.to_string().contains("timeout") {
                                                        "timeout"
                                                    } else {
                                                        "kafka_error"
                                                    };
                                                    if let Some(ref metric) = dlq_send_failures_metric {
                                                        metric.with_label_values(&[failure_reason]).inc();
                                                    }
                                                }
                                            }
                                        });
                                    }

                                // M-1020: Check decode error policy - pause or skip?
                                if decode_error_policy == DecodeErrorPolicy::Pause {
                                    // Pause mode: Stop consuming to prevent data loss.
                                    // DO NOT store offset - next restart will retry this message.
                                    eprintln!("🛑 KAFKA_ON_DECODE_ERROR=pause: Stopping Kafka consumption due to decode error");
                                    eprintln!("   Partition: {}, Offset: {}", partition, offset);
                                    eprintln!("   Offset NOT stored - restart will retry this message");
                                    eprintln!("   WebSocket connections remain active; no new data will be streamed");
                                    eprintln!("   Fix the issue and restart the server to resume");

                                    // Enter paused state: loop until shutdown
                                    let mut last_pause_log = Instant::now();
                                    loop {
                                        tokio::select! {
                                            _ = shutdown_rx_kafka.recv() => {
                                                println!("📤 Kafka consumer (PAUSED) received shutdown signal");
                                                break;
                                            }
                                            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                                                // Log every 30 seconds so operators know we're still paused
                                                if last_pause_log.elapsed() >= Duration::from_secs(30) {
                                                    eprintln!(
                                                        "⏸️  Kafka consumer PAUSED at partition {} offset {} due to decode error (KAFKA_ON_DECODE_ERROR=pause)",
                                                        partition, offset
                                                    );
                                                    last_pause_log = Instant::now();
                                                }
                                            }
                                        }
                                    }
                                    // After shutdown, exit the consumer loop entirely
                                    break;
                                }
                                // Skip mode (default): Continue below to store offset and process next message
                                }
                            }
                        }
                    } else {
                        // M-1025: Kafka message has no payload (payload=None).
                        // This is unusual and indicates either:
                        // 1. Producer bug (sending tombstone/null payload)
                        // 2. Kafka compaction deleted the value
                        // 3. Message corruption
                        //
                        // Track this explicitly so operators can detect data loss.
                        if let Some(ref metric) = payload_missing_clone {
                            metric.inc();
                        }
                        // M-1107 FIX: Track in ServerMetrics for /health endpoint visibility
                        metrics_clone.payload_missing.fetch_add(1, Ordering::Relaxed);

                        // M-1085 FIX: Count in windowed messages so decode error rate denominator is correct.
                        // Without this, missing-payload messages weren't counted which made staleness
                        // detection unreliable (fewer messages than Kafka actually delivered).
                        metrics_clone.kafka_messages_received.fetch_add(1, Ordering::Relaxed);
                        metrics_clone.note_kafka_message();
                        metrics_clone.record_message_received();

                        // M-1096: Apply decode error policy consistently for payload-missing messages.
                        // Previously this path always stored offset (skip) regardless of policy.
                        if decode_error_policy == DecodeErrorPolicy::Pause {
                            // Pause mode: Stop consuming to prevent data loss.
                            // DO NOT store offset - next restart will retry this message.
                            eprintln!("🛑 KAFKA_ON_DECODE_ERROR=pause: Stopping Kafka consumption due to missing payload");
                            eprintln!("   Partition: {}, Offset: {}", partition, offset);
                            eprintln!("   Message has no payload (tombstone/corruption/compaction)");
                            eprintln!("   Offset NOT stored - restart will retry this message");
                            eprintln!("   WebSocket connections remain active; no new data will be streamed");
                            eprintln!("   Fix the issue and restart the server to resume");

                            // Enter paused state: loop until shutdown
                            let mut last_pause_log = Instant::now();
                            loop {
                                tokio::select! {
                                    _ = shutdown_rx_kafka.recv() => {
                                        println!("📤 Kafka consumer (PAUSED) received shutdown signal");
                                        break;
                                    }
                                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                                        // Log every 30 seconds so operators know we're still paused
                                        if last_pause_log.elapsed() >= Duration::from_secs(30) {
                                            eprintln!(
                                                "⏸️  Kafka consumer PAUSED at partition {} offset {} due to missing payload (KAFKA_ON_DECODE_ERROR=pause)",
                                                partition, offset
                                            );
                                            last_pause_log = Instant::now();
                                        }
                                    }
                                }
                            }
                            // After shutdown, exit the consumer loop entirely
                            break;
                        }

                        // Skip mode (default): Log and continue to store offset
                        tracing::warn!(
                            partition = partition,
                            offset = offset,
                            "Kafka message has no payload (M-1025); skipping and advancing offset"
                        );
                    }

                    // Store offset only after we've processed the message (at-least-once).
                    // M-414 FIX: Use store_offset_from_message which stores offset+1 (the NEXT
                    // record to read). Kafka semantics: committed offset = next record to fetch
                    // on restart. The previous code stored the current offset, causing
                    // re-processing of the last message on restart.
                    // M-1025/M-1096: We store offset for payload-missing messages only in skip mode.
                    // In pause mode, we break out of the loop before reaching this point.
                    if let Err(e) = consumer.store_offset_from_message(&msg) {
                        eprintln!(
                            "⚠️  Failed to store Kafka offset for {}/{}@{}: {}",
                            topic, partition, offset, e
                        );
                    }

                    // M-431: Update offset in shared state for lag monitor
                    // offset+1 is the NEXT offset to consume
                    // Using std::sync::RwLock write lock (very brief, O(1) insert)
                    match partition_offsets_for_consumer.write() {
                        Ok(mut guard) => {
                            guard.insert(partition, (offset + 1, Instant::now()));
                        }
                        Err(poisoned) => {
                            // SAFETY: Recover from poison - lag metrics are non-critical
                            let mut guard = poisoned.into_inner();
                            guard.insert(partition, (offset + 1, Instant::now()));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Kafka error: {}", e);
                    // S-25 fix: Infrastructure errors are separate from message errors.
                    // Do NOT increment kafka_errors here - that counter is for message processing
                    // errors only (decode failures). This ensures success = total - kafka_errors
                    // remains semantically correct (infra errors don't increment kafka_messages_received).
                    metrics_clone.infrastructure_errors.fetch_add(1, Ordering::Relaxed);

                    // S-25 fix: Track timestamp for accurate recency checks in health endpoint
                    metrics_clone.note_infrastructure_error();

                    // Issue #4: Classify error type for granular monitoring
                    let error_msg = e.to_string().to_lowercase();
                    let error_type = if error_msg.contains("dns") || error_msg.contains("resolve") || error_msg.contains("name") {
                        "dns_failure"
                    } else if error_msg.contains("timeout") || error_msg.contains("timed out") {
                        "connection_timeout"
                    } else if error_msg.contains("broker") || error_msg.contains("connection refused") || error_msg.contains("reset by peer") {
                        "broker_down"
                    } else if error_msg.contains("decode") || error_msg.contains("parse") || error_msg.contains("invalid") {
                        "decode_error"
                    } else {
                        "unknown"
                    };

                    // Update Prometheus metric if available
                    if let Some(ref kafka_errors_by_type) = kafka_errors_by_type_clone {
                        kafka_errors_by_type.with_label_values(&[error_type]).inc();
                    }

                    // Prevent busy-looping on persistent broker/network errors.
                    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                }
            }
                }
            }
        }
        println!("✅ Kafka consumer loop terminated gracefully");
    });

    // 6b. Spawn circuit breaker monitor task (Issue #16: auto-recovery)
    // This task periodically checks health status and triggers shutdown if degraded > 10 minutes
    let server_state_monitor = server_state.clone();
    let mut shutdown_rx_monitor = shutdown_tx.subscribe();
    let circuit_breaker_task = tokio::spawn(async move {
        println!("🔄 Starting adaptive circuit breaker monitor...");
        let check_interval = tokio::time::Duration::from_secs(30); // Check every 30 seconds

        // M-489: Apply jitter to thresholds to prevent thundering herd restarts.
        // Multiple instances entering degraded state simultaneously would restart together
        // without jitter. We use ±10% jitter based on startup time nanoseconds.
        let jitter_factor = {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as f64;
            // Map seed to [-0.1, +0.1] range
            ((seed % 2000.0) / 10000.0) - 0.1
        };

        // Adaptive thresholds (Issue #3: Adaptive circuit breaker)
        // M-489: Base thresholds with jitter applied
        let apply_jitter = |base_secs: u64| -> tokio::time::Duration {
            let jittered = (base_secs as f64) * (1.0 + jitter_factor);
            tokio::time::Duration::from_secs_f64(jittered.max(1.0))
        };
        let quick_restart_threshold = apply_jitter(30); // ~27-33 seconds
        let recovery_threshold = apply_jitter(300); // ~270-330 seconds
        let stuck_threshold = apply_jitter(600); // ~540-660 seconds
        println!(
            "🔄 Circuit breaker thresholds (with jitter): quick={:?}, recovery={:?}, stuck={:?}",
            quick_restart_threshold, recovery_threshold, stuck_threshold
        );

        // Track previous error count to detect improvement
        let mut previous_error_count: Option<u64> = None;

        loop {
            tokio::select! {
                _ = shutdown_rx_monitor.recv() => {
                    println!("🔄 Circuit breaker monitor received shutdown signal, stopping...");
                    break;
                }
                _ = tokio::time::sleep(check_interval) => {
                    // Check current health status
                    let snapshot = server_state_monitor.metrics.snapshot();

                    // M-1070: Use windowed decode error rate (last 120s) instead of lifetime rate.
                    // This ensures the circuit breaker reflects current reality, not past history.
                    let decode_error_rate_120s = if snapshot.messages_last_120s > 0 {
                        (snapshot.decode_errors_last_120s as f64) / (snapshot.messages_last_120s as f64)
                    } else {
                        0.0
                    };

                    // M-1070: Use windowed rate for degraded check (matches /health behavior per M-1069)
                    let is_degraded = snapshot.kafka_messages_received == 0
                        || snapshot.last_kafka_message_ago_seconds.unwrap_or(999) > 60
                        || snapshot.kafka_errors > snapshot.kafka_messages_received / 10
                        || decode_error_rate_120s > 0.01;

                    // Issue #3: Track if error rate is improving
                    // S-25 fix: Use decode_errors + infrastructure_errors to avoid double-counting.
                    // Previously this used kafka_errors + decode_errors, but kafka_errors already
                    // included decode_errors, causing double-counting of decode errors.
                    let current_error_count = snapshot.decode_errors + snapshot.infrastructure_errors;
                    let error_rate_improving = if let Some(prev_count) = previous_error_count {
                        current_error_count <= prev_count // Not increasing (stable or improving)
                    } else {
                        false // First check, no baseline yet
                    };
                    previous_error_count = Some(current_error_count);

                    let mut degraded_since = server_state_monitor.degraded_since.write().await;

                    if is_degraded {
                        // Track when we first became degraded
                        if degraded_since.is_none() {
                            let now = Instant::now();
                            *degraded_since = Some(now);
                            println!("⚠️  Circuit breaker: Server entered degraded state at {:?}", now);
                        } else if let Some(degraded_start) = *degraded_since {
                            // Issue #3: Adaptive circuit breaker timeout
                            let duration = degraded_start.elapsed();

                            // Determine adaptive timeout based on duration and error trend
                            let restart_threshold = if duration < quick_restart_threshold {
                                // Very short degradation - likely transient (Kafka restart)
                                quick_restart_threshold
                            } else if duration < recovery_threshold && error_rate_improving {
                                // System recovering, give it more time
                                recovery_threshold
                            } else {
                                // Either long degradation or not improving - restart now
                                stuck_threshold
                            };

                            let time_until_restart = if duration < restart_threshold {
                                restart_threshold.saturating_sub(duration)
                            } else {
                                tokio::time::Duration::ZERO
                            };

                            if error_rate_improving {
                                println!("⚠️  Circuit breaker: Server degraded for {:?}, error rate improving, restart in {:?}",
                                    duration, time_until_restart);
                            } else {
                                println!("⚠️  Circuit breaker: Server degraded for {:?}, error rate NOT improving, restart in {:?}",
                                    duration, time_until_restart);
                            }

                            if duration >= restart_threshold {
                                eprintln!("🚨 CIRCUIT BREAKER OPEN: Server degraded for {:?} (threshold: {:?})",
                                    duration, restart_threshold);
                                eprintln!("   Error rate improving: {}", error_rate_improving);
                                eprintln!("   Triggering graceful shutdown for auto-restart...");
                                eprintln!("   Docker restart policy will automatically restart the container.");

                                // Trigger shutdown
                                let _ = server_state_monitor.shutdown_tx.send(());
                                break;
                            }
                        }
                    } else {
                        // Server is healthy, reset degraded tracking
                        if degraded_since.is_some() {
                            println!("✅ Circuit breaker: Server recovered to healthy state");
                            *degraded_since = None;
                            previous_error_count = None; // Reset error tracking
                        }
                    }
                }
            }
        }
        println!("✅ Circuit breaker monitor terminated gracefully");
    });

    // 7. Create Axum router with enhanced health endpoint
    let app = Router::new()
        .route("/ws", get(handlers::websocket_handler))
        .route("/health", get(handlers::health_handler))
        .route("/version", get(handlers::version_handler))
        .route("/metrics", get(handlers::metrics_handler))
        //  Expected schema API endpoints
        .route("/api/expected-schema", get(handlers::list_expected_schemas))
        .route(
            "/api/expected-schema/:graph_name",
            get(handlers::get_expected_schema)
                .put(handlers::set_expected_schema)
                .delete(handlers::delete_expected_schema),
        )
        // Use fallback_service instead of nest_service to ensure API routes take precedence
        .fallback_service(ServeDir::new("observability-ui/dist"))
        .with_state(server_state);

    // 8. Start server with graceful shutdown
    // Read host/port from environment (used by Docker containers)
    // M-232: Default to localhost for security - network binding requires explicit opt-in
    let host = std::env::var(WEBSOCKET_HOST).unwrap_or_else(|_| "127.0.0.1".to_string());

    // M-232: Security warning when binding to non-localhost
    if host != "127.0.0.1" && host != "localhost" && host != "::1" {
        eprintln!(
            "⚠️  SECURITY WARNING: Binding to {} exposes this server on the network.",
            host
        );
        eprintln!(
            "   This server has NO authentication - any client can connect and receive events."
        );
        eprintln!("   For production, use a reverse proxy with TLS and authentication.");
        eprintln!("   See module docs or docs/OBSERVABILITY_INFRASTRUCTURE.md for guidance.");
    }
    let explicit_port: Option<u16> = parse_optional_env_var_with_warning("WEBSOCKET_PORT");

    let (listener, bound_port) = if let Some(port) = explicit_port {
        // If WEBSOCKET_PORT is explicitly set (e.g., in Docker), fail fast on bind error
        // This prevents silent fallback that breaks container port mapping
        let addr = format!("{}:{}", host, port);
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => (l, port),
            Err(e) => {
                return Err(format!(
                    "Failed to bind to explicitly configured port {} (WEBSOCKET_PORT): {}",
                    port, e
                )
                .into());
            }
        }
    } else {
        // Development mode: try fallback ports
        let ports = [3002, 3003, 3004, 3005];
        let mut last_error = None;
        let mut bound_listener = None;

        for port in ports.iter() {
            let addr = format!("{}:{}", host, port);
            match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => {
                    bound_listener = Some((l, *port));
                    break;
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to bind to port {}: {}", port, e);
                    last_error = Some(e);
                }
            }
        }

        match bound_listener {
            Some(listener) => listener,
            None => {
                let error_msg = if let Some(err) = last_error {
                    err.to_string()
                } else {
                    "unknown error".to_string()
                };
                return Err(
                    format!("Failed to bind to any port in {:?}: {}", ports, error_msg).into(),
                );
            }
        }
    };

    println!(
        "🚀 WebSocket server starting on http://localhost:{}",
        bound_port
    );
    println!("📊 Serving React UI from observability-ui/dist");
    println!("🔌 WebSocket endpoint: ws://localhost:{}/ws", bound_port);
    println!("🏥 Health endpoint: http://localhost:{}/health", bound_port);
    println!(
        "📦 Version endpoint: http://localhost:{}/version",
        bound_port
    );

    // Use graceful shutdown with signal handling
    let mut shutdown_rx_server = shutdown_tx.subscribe();

    // Issue #2: Propagate error instead of unwrap
    // M-488: Use into_make_service_with_connect_info to enable IP-based rate limiting
    let serve_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        // Wait for shutdown signal
        let _ = shutdown_rx_server.recv().await;
        println!("🛑 Server received shutdown signal, closing connections...");
    })
    .await;

    if let Err(e) = serve_result {
        let _ = shutdown_tx.send(());
        return Err(format!("Server error: {}", e).into());
    }

    let join_timeout = tokio::time::Duration::from_secs(5);
    {
        let mut handle = kafka_consumer_task;
        tokio::select! {
            res = &mut handle => {
                if let Err(e) = res {
                    eprintln!("⚠️  Kafka consumer task terminated with error: {}", e);
                }
            }
            _ = tokio::time::sleep(join_timeout) => {
                eprintln!("⚠️  Kafka consumer task did not shut down within {:?}, aborting...", join_timeout);
                handle.abort();
                let _ = handle.await;
            }
        }
    }

    {
        let mut handle = circuit_breaker_task;
        tokio::select! {
            res = &mut handle => {
                if let Err(e) = res {
                    eprintln!("⚠️  Circuit breaker monitor task terminated with error: {}", e);
                }
            }
            _ = tokio::time::sleep(join_timeout) => {
                eprintln!("⚠️  Circuit breaker monitor task did not shut down within {:?}, aborting...", join_timeout);
                handle.abort();
                let _ = handle.await;
            }
        }
    }

    // Wait for in-flight DLQ send tasks (best-effort).
    // S-19: Use same configurable value as at startup
    let dlq_drain_timeout = tokio::time::Duration::from_secs(5);
    match tokio::time::timeout(
        dlq_drain_timeout,
        dlq_send_semaphore.acquire_many(get_max_concurrent_dlq_sends() as u32),
    )
    .await
    {
        Ok(Ok(permit)) => drop(permit),
        Ok(Err(e)) => eprintln!("⚠️  DLQ drain failed: {}", e),
        Err(_) => eprintln!(
            "⚠️  Timed out waiting for in-flight DLQ sends (>{:?}); continuing shutdown",
            dlq_drain_timeout
        ),
    }

    // Flush DLQ producer (best-effort) so queued delivery reports get a chance to complete.
    let dlq_producer_for_flush = dlq_producer.clone();
    let flush_timeout = std::time::Duration::from_secs(5);
    match tokio::task::spawn_blocking(move || {
        dlq_producer_for_flush.flush(Timeout::After(flush_timeout))
    })
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("⚠️  DLQ producer flush failed: {}", e),
        Err(e) => eprintln!("⚠️  DLQ producer flush task join error: {}", e),
    }

    // Wait for in-flight ReplayBuffer Redis writes (best-effort).
    replay_buffer_for_shutdown
        .drain_pending_redis_writes(tokio::time::Duration::from_secs(5))
        .await;

    // M-482: Wait for lag monitor thread to finish (it checks shutdown flag)
    // The thread wakes up every lag_check_interval_secs, so wait a bit longer
    match tokio::task::spawn_blocking(move || lag_monitor_thread.join()).await {
        Ok(Ok(())) => println!("📊 Lag monitor thread joined successfully"),
        Ok(Err(_)) => eprintln!("⚠️  Lag monitor thread panicked"),
        Err(e) => eprintln!("⚠️  Failed to join lag monitor thread: {}", e),
    }

    println!("✅ WebSocket server shutdown complete");
    Ok(())
}
