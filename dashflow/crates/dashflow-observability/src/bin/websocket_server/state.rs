//! Server state, metrics, and rate limiting types for the WebSocket server.
//!
//! This module contains the core state management types:
//! - `ServerState` - Main server state with broadcast channel, metrics, and replay buffer
//! - `ServerMetrics` - Atomic counters for server health metrics
//! - `ServerMetricsSnapshot` - Serializable snapshot for health endpoint
//! - `WebsocketServerMetricsCollector` - Prometheus collector for server metrics
//! - `ConnectionRateLimiter` - Per-IP rate limiting to prevent DoS attacks
//! - `DecodeErrorLog` - Structured logging for decode errors

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::Utc;
use prometheus::proto::{
    Counter as ProtoCounter, Gauge as ProtoGauge, LabelPair, Metric as ProtoMetric, MetricFamily,
    MetricType,
};
use prometheus::{
    core::{Collector, Desc},
    HistogramVec, IntCounter, IntCounterVec, Registry,
};
use rdkafka::producer::FutureProducer;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use super::replay_buffer::{OutboundBinaryMessage, ReplayBuffer};
use super::ExpectedSchemaStore;

// =============================================================================
// Connection Rate Limiting (M-488)
// =============================================================================

/// M-488: Connection rate limiter to prevent DoS attacks via unlimited WebSocket connections.
#[derive(Clone)]
pub(crate) struct ConnectionRateLimiter {
    connections_by_ip: Arc<RwLock<HashMap<String, usize>>>,
    max_connections_per_ip: usize,
    rejected_connections: Option<IntCounter>,
}

impl ConnectionRateLimiter {
    pub fn new(max_connections_per_ip: usize, rejected_connections: Option<IntCounter>) -> Self {
        Self {
            connections_by_ip: Arc::new(RwLock::new(HashMap::new())),
            max_connections_per_ip,
            rejected_connections,
        }
    }

    pub async fn try_acquire(&self, ip: &str) -> bool {
        let mut connections = self.connections_by_ip.write().await;
        let count = connections.entry(ip.to_string()).or_insert(0);
        if *count >= self.max_connections_per_ip {
            if let Some(ref metric) = self.rejected_connections {
                metric.inc();
            }
            eprintln!(
                "⚠️  Rate limit: Rejecting connection from {} ({}/{} connections)",
                ip, *count, self.max_connections_per_ip
            );
            false
        } else {
            *count += 1;
            true
        }
    }

    pub async fn release(&self, ip: &str) {
        let mut connections = self.connections_by_ip.write().await;
        if let Some(count) = connections.get_mut(ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(ip);
            }
        }
    }
}

/// Parse trusted proxy IPs from WEBSOCKET_TRUSTED_PROXY_IPS environment variable.
fn parse_trusted_proxy_ips(raw: &str) -> HashSet<IpAddr> {
    let mut ips = HashSet::new();

    for part in raw.split(',') {
        let ip_str = part.trim();
        if ip_str.is_empty() {
            continue;
        }
        match ip_str.parse::<IpAddr>() {
            Ok(ip) => {
                ips.insert(ip);
            }
            Err(e) => {
                eprintln!(
                    "⚠️  Ignoring invalid IP '{}' in WEBSOCKET_TRUSTED_PROXY_IPS: {}",
                    ip_str, e
                );
            }
        }
    }

    ips
}

pub(crate) fn parse_trusted_proxy_ips_from_env() -> HashSet<IpAddr> {
    // M-153: Use constant from config module
    let raw = match std::env::var(super::config::WEBSOCKET_TRUSTED_PROXY_IPS) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };
    parse_trusted_proxy_ips(&raw)
}

// =============================================================================
// Server State
// =============================================================================

/// Server health and metrics state
#[derive(Clone)]
pub(crate) struct ServerState {
    pub(crate) tx: broadcast::Sender<OutboundBinaryMessage>,
    pub(crate) metrics: Arc<ServerMetrics>,
    pub(crate) shutdown_tx: broadcast::Sender<()>,
    pub(crate) prometheus_registry: Arc<Registry>,
    /// Tracks when the server first entered degraded state (Issue #16: auto-recovery)
    pub(crate) degraded_since: Arc<RwLock<Option<Instant>>>,
    /// Kafka producer for dead-letter queue (Issue #3: DLQ publishing)
    #[allow(dead_code)] // Architectural: DLQ producer held for future failed message routing
    pub(crate) dlq_producer: Arc<FutureProducer>,
    /// Client lag monitoring metrics (Issue #6)
    pub(crate) client_lag_events: Option<IntCounterVec>,
    pub(crate) client_lag_messages: Option<IntCounterVec>,
    /// Replay buffer for reconnection recovery (Issue #3 Server-side replay)
    pub(crate) replay_buffer: ReplayBuffer,
    /// Expected schema store (Server-side schema persistence)
    pub(crate) expected_schemas: ExpectedSchemaStore,
    /// M-488: Connection rate limiter
    pub(crate) connection_rate_limiter: ConnectionRateLimiter,
    /// M-702: Only trust x-forwarded-for when the peer is a configured proxy.
    pub(crate) trusted_proxy_ips: Arc<HashSet<IpAddr>>,
    /// M-684: Resume/replay observability metrics
    pub(crate) resume_requests_total: Option<IntCounterVec>,
    pub(crate) replay_messages_total: Option<IntCounterVec>,
    pub(crate) replay_gaps_total: Option<IntCounterVec>,
    /// M-782: Total gap message count (sum of all gap sizes, not just gap events)
    pub(crate) replay_gap_messages_total: Option<IntCounterVec>,
    pub(crate) replay_latency_histogram: Option<HistogramVec>,
    /// M-691: Namespace to prevent resume collisions across Kafka topics/clusters.
    pub(crate) resume_namespace: String,
    pub(crate) kafka_topic: String,
    pub(crate) kafka_group_id: String,
    /// M-682: Slow client disconnect metric
    pub(crate) slow_client_disconnects: Option<IntCounter>,
    /// M-1061: Oversized control frame rejections (DoS prevention)
    pub(crate) control_oversized_total: Option<IntCounter>,
    /// M-1062: Invalid JSON parse failures on control messages
    pub(crate) control_parse_failures_total: Option<IntCounter>,
    /// M-682: Cumulative lag threshold for disconnecting slow clients (backpressure)
    pub(crate) slow_client_disconnect_threshold: u64,
    /// M-773: Sliding window duration (seconds) for lag measurement.
    /// When > 0, lag counter resets after this many seconds (leaky bucket semantics).
    /// When 0, uses lifetime cumulative lag (legacy behavior).
    pub(crate) slow_client_lag_window_secs: u64,
    /// M-1019: Max payload bytes the server accepts. Exposed in /version so UI can detect
    /// config mismatch (e.g., server accepts 20MB but UI can only decode 10MB).
    pub(crate) max_payload_bytes: usize,
    /// M-1020: Policy for handling Kafka decode errors.
    /// `skip` (default) - advance offset and continue; `pause` - stop consuming until restart.
    pub(crate) decode_error_policy: DecodeErrorPolicy,
}

// =============================================================================
// Decode Error Policy (M-1020)
// =============================================================================

/// M-1020: Policy for handling decode errors on Kafka messages.
///
/// When a Kafka message fails protobuf decoding, the server must decide:
/// - **Skip**: Advance the offset and continue (default, prioritizes availability).
///   Data loss risk: the failed message is permanently skipped.
/// - **Pause**: Stop consuming from Kafka; keep WebSocket connections alive.
///   Blocks further processing until operator restart/fix; alerts loudly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DecodeErrorPolicy {
    /// Advance offset past decode errors (prioritize availability over durability)
    #[default]
    Skip,
    /// Stop consuming on decode error (prioritize durability; requires operator intervention)
    Pause,
}

impl DecodeErrorPolicy {
    /// Parse from environment variable KAFKA_ON_DECODE_ERROR.
    /// Accepts: "skip" (default), "pause".
    pub fn from_env() -> Self {
        // M-153: Use constant from config module
        Self::parse(&std::env::var(super::config::KAFKA_ON_DECODE_ERROR).unwrap_or_default())
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "pause" => {
                println!("⚠️  KAFKA_ON_DECODE_ERROR=pause: Server will STOP consuming on decode failures");
                Self::Pause
            }
            "skip" | "" => Self::Skip,
            other => {
                eprintln!(
                    "⚠️  Unknown KAFKA_ON_DECODE_ERROR value '{}', defaulting to 'skip'",
                    other
                );
                Self::Skip
            }
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Pause => "pause",
        }
    }
}

// =============================================================================
// Server Metrics
// =============================================================================

const MS_PER_SEC: u64 = 1_000;
const DROPPED_MESSAGES_WINDOW_SECS: u64 = 120;
const DROPPED_MESSAGES_WINDOW_MS: u64 = DROPPED_MESSAGES_WINDOW_SECS * MS_PER_SEC;

/// Generic sliding window for counting events within a time range.
/// Used for dropped messages and decode errors (low-frequency events).
#[derive(Debug, Default)]
struct SlidingCountWindow {
    events: VecDeque<(u64, u64)>,
    sum: u64,
}

impl SlidingCountWindow {
    fn prune(&mut self, now_ms_since_start: u64, window_ms: u64) {
        while let Some((event_ms_since_start, count)) = self.events.front().copied() {
            if now_ms_since_start.saturating_sub(event_ms_since_start) <= window_ms {
                break;
            }
            self.events.pop_front();
            self.sum = self.sum.saturating_sub(count);
        }
    }

    fn record(&mut self, now_ms_since_start: u64, count: u64, window_ms: u64) {
        if count == 0 {
            return;
        }
        self.events.push_back((now_ms_since_start, count));
        self.sum = self.sum.saturating_add(count);
        self.prune(now_ms_since_start, window_ms);
    }

    fn sum_recent(&mut self, now_ms_since_start: u64, window_ms: u64) -> u64 {
        self.prune(now_ms_since_start, window_ms);
        self.sum
    }
}

// Type aliases for clarity
type DroppedMessagesWindow = SlidingCountWindow;
type DecodeErrorsWindow = SlidingCountWindow;

// =============================================================================
// M-1105: Lockless Sliding Window for High-Frequency Counters
// =============================================================================

/// Lockless sliding window counter using time-bucketed atomics.
///
/// M-1105: Replaces `Mutex<SlidingCountWindow>` for high-frequency counters
/// (messages_received) to eliminate lock contention on the hot path.
///
/// Design:
/// - Fixed array of buckets, each covering BUCKET_SECS seconds
/// - Each bucket stores (epoch, count) packed in a u64: upper 32 bits = epoch, lower 32 = count
/// - On record: CAS loop to increment current bucket (or reset if epoch changed)
/// - On read: sum buckets where epoch is within the window
///
/// Trade-offs:
/// - Slightly approximate (bucket boundaries, CAS retries can drop rare counts)
/// - Sufficient for health metrics which need directional accuracy, not exact counts
const BUCKET_SECS: u64 = 10;
const NUM_BUCKETS: usize = 12; // 120s window / 10s per bucket
const MAX_COUNT_PER_BUCKET: u64 = u32::MAX as u64; // 4B max per bucket - plenty

/// Lockless sliding window for high-throughput event counting.
/// Uses atomic CAS operations instead of mutex locks.
#[derive(Debug)]
struct LocklessSlidingWindow {
    /// Array of (epoch, count) pairs packed as u64: upper 32 bits = epoch, lower 32 = count
    buckets: [AtomicU64; NUM_BUCKETS],
    start_time: Instant,
}

impl Default for LocklessSlidingWindow {
    fn default() -> Self {
        Self {
            buckets: Default::default(),
            start_time: Instant::now(),
        }
    }
}

impl LocklessSlidingWindow {
    fn new(start_time: Instant) -> Self {
        Self {
            buckets: Default::default(),
            start_time,
        }
    }

    /// Get the current epoch number (increments every BUCKET_SECS seconds)
    #[inline]
    fn current_epoch(&self) -> u32 {
        (self.start_time.elapsed().as_secs() / BUCKET_SECS) as u32
    }

    /// Record `count` events at the current time.
    /// Uses CAS loop to safely increment without locks.
    #[inline]
    pub fn record(&self, count: u64) {
        if count == 0 {
            return;
        }
        let current_epoch = self.current_epoch();
        let bucket_idx = (current_epoch as usize) % NUM_BUCKETS;

        // CAS loop to update bucket atomically
        loop {
            let current = self.buckets[bucket_idx].load(Ordering::Relaxed);
            let stored_epoch = (current >> 32) as u32;
            let stored_count = current & 0xFFFF_FFFF;

            let (new_epoch, new_count) = if stored_epoch == current_epoch {
                // Same epoch - add to existing count (saturating to prevent overflow)
                (current_epoch, (stored_count + count).min(MAX_COUNT_PER_BUCKET))
            } else {
                // Different epoch (bucket is stale) - start fresh with current epoch
                (current_epoch, count.min(MAX_COUNT_PER_BUCKET))
            };

            let new_value = ((new_epoch as u64) << 32) | new_count;

            match self.buckets[bucket_idx].compare_exchange_weak(
                current,
                new_value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(_) => continue, // CAS failed, retry
            }
        }
    }

    /// Sum events within the last `window_secs` seconds.
    /// Returns approximate count (buckets outside window are excluded).
    pub fn sum_recent(&self, window_secs: u64) -> u64 {
        let current_epoch = self.current_epoch();
        let window_epochs = (window_secs / BUCKET_SECS) as u32;
        let oldest_valid_epoch = current_epoch.saturating_sub(window_epochs);

        let mut sum = 0u64;
        for bucket in &self.buckets {
            let value = bucket.load(Ordering::Relaxed);
            let stored_epoch = (value >> 32) as u32;
            let stored_count = value & 0xFFFF_FFFF;

            // Include bucket if its epoch is within the valid window
            if stored_epoch >= oldest_valid_epoch && stored_epoch <= current_epoch {
                sum = sum.saturating_add(stored_count);
            }
        }
        sum
    }
}

/// Server metrics tracked for health endpoint
///
/// **RACE CONDITION FIX (Issue #5)**:
/// Counters use `AtomicU64`/`AtomicUsize` instead of plain integers with `RwLock`.
/// This prevents lost updates when multiple async tasks update metrics concurrently.
///
/// **Design Pattern**:
/// - Counters (`kafka_messages_received`, `kafka_errors`, `dropped_messages`) → `AtomicU64`
/// - Non-counter fields (`connected_clients`) → `AtomicUsize`
/// - Timestamp recency → stored as monotonic milliseconds-since-start atomics (so sync Prometheus
///   collectors can read them without async locks)
///
/// **Performance**: Atomic operations use CPU-level lock-free instructions (much faster than mutex).
#[derive(Debug)]
pub(crate) struct ServerMetrics {
    /// Total Kafka messages processed since startup (atomic counter)
    pub(crate) kafka_messages_received: AtomicU64,
    /// Total Kafka messages successfully processed (monotonic, scrape-safe counter)
    ///
    /// This is used for `websocket_kafka_messages_total{status="success"}`. It is tracked
    /// explicitly (not derived from other atomics) to ensure the exported Prometheus
    /// counter never "goes backwards" due to non-atomic multi-field scrape snapshots.
    pub(crate) kafka_messages_success: AtomicU64,
    /// Total Kafka messages that failed processing (monotonic, scrape-safe counter)
    ///
    /// This is used for `websocket_kafka_messages_total{status="error"}` and includes
    /// only new-data decode errors (old/pre-session decode failures are tracked separately).
    pub(crate) kafka_messages_error: AtomicU64,
    /// Total Kafka errors encountered (atomic counter)
    pub(crate) kafka_errors: AtomicU64,
    /// Infrastructure errors (connection, DNS, network) - Issue #1: Separate from data errors
    pub(crate) infrastructure_errors: AtomicU64,
    /// Number of currently connected WebSocket clients (atomic counter)
    pub(crate) connected_clients: AtomicUsize,
    /// Total messages dropped due to slow clients (atomic counter)
    /// M-1026: This counts TOTAL drops across ALL clients (not unique messages).
    /// With N clients, 1 lag event that drops M messages contributes M×N to this counter.
    /// Use `lag_events` for backpressure decisions instead.
    pub(crate) dropped_messages: AtomicU64,
    /// Total lag events (atomic counter) - M-1027: Use this for backpressure
    /// Each lag event increments this by 1, regardless of how many messages were dropped.
    /// This provides stable backpressure behavior regardless of client count.
    pub(crate) lag_events: AtomicU64,
    /// Total protobuf decode errors (Issue #11: buffer underflow tracking)
    /// This counter tracks messages that fail protobuf decoding, indicating data corruption
    pub(crate) decode_errors: AtomicU64,
    /// Decode errors from old data (pre-session, can be safely skipped)
    /// These are messages from before this session started - expected with auto.offset.reset=earliest
    ///
    /// This is also exported as `websocket_kafka_messages_total{status="old_data_error"}`.
    pub(crate) old_data_decode_errors: AtomicU64,
    /// M-1107: Messages with missing payload (tombstone/corruption/compaction)
    /// Indicates data loss - should be surfaced in /health and alerts
    pub(crate) payload_missing: AtomicU64,
    /// M-1033: Total WebSocket send operations that failed (excluding timeouts)
    pub(crate) send_failed: AtomicU64,
    /// M-1033: Total WebSocket send operations that timed out
    pub(crate) send_timeout: AtomicU64,
    /// Timestamp when server started (immutable after construction)
    pub(crate) start_time: Instant,
    /// Milliseconds since start for last Kafka message (0 = never)
    last_kafka_message_ms_since_start: AtomicU64,
    /// Milliseconds since start for last infrastructure error (0 = never)
    last_infrastructure_error_ms_since_start: AtomicU64,
    /// Milliseconds since start for last client drop event (0 = never)
    last_drop_ms_since_start: AtomicU64,
    /// Sliding window of dropped messages for recency-based health alerts (M-1041)
    dropped_messages_window: Mutex<DroppedMessagesWindow>,
    /// M-1069/M-1070: Sliding window of decode errors for recency-based health/circuit-breaker
    decode_errors_window: Mutex<DecodeErrorsWindow>,
    /// M-1069/M-1105: Lockless sliding window of successfully received messages.
    /// Uses atomic CAS operations instead of mutex to eliminate hot-path contention.
    messages_received_window: LocklessSlidingWindow,
    /// M-1095: Sliding window of send failures for detecting "currently stuck" clients.
    /// Low-frequency events, so mutex-based is fine.
    send_failed_window: Mutex<SlidingCountWindow>,
    /// M-1095: Sliding window of send timeouts for detecting "currently stuck" clients.
    send_timeout_window: Mutex<SlidingCountWindow>,
}

impl Default for ServerMetrics {
    fn default() -> Self {
        let start_time = Instant::now();
        Self {
            kafka_messages_received: AtomicU64::new(0),
            kafka_messages_success: AtomicU64::new(0),
            kafka_messages_error: AtomicU64::new(0),
            kafka_errors: AtomicU64::new(0),
            infrastructure_errors: AtomicU64::new(0),
            connected_clients: AtomicUsize::new(0),
            dropped_messages: AtomicU64::new(0),
            lag_events: AtomicU64::new(0),
            decode_errors: AtomicU64::new(0),
            old_data_decode_errors: AtomicU64::new(0),
            payload_missing: AtomicU64::new(0),
            send_failed: AtomicU64::new(0),
            send_timeout: AtomicU64::new(0),
            start_time,
            last_kafka_message_ms_since_start: AtomicU64::new(0),
            last_infrastructure_error_ms_since_start: AtomicU64::new(0),
            last_drop_ms_since_start: AtomicU64::new(0),
            dropped_messages_window: Mutex::new(DroppedMessagesWindow::default()),
            decode_errors_window: Mutex::new(DecodeErrorsWindow::default()),
            // M-1105: Use lockless sliding window for high-frequency message counting
            messages_received_window: LocklessSlidingWindow::new(start_time),
            // M-1095: Low-frequency send error windows
            send_failed_window: Mutex::new(SlidingCountWindow::default()),
            send_timeout_window: Mutex::new(SlidingCountWindow::default()),
        }
    }
}

impl ServerMetrics {
    fn now_ms_since_start(&self) -> u64 {
        u64::try_from(self.start_time.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(crate) fn note_kafka_message(&self) {
        self.last_kafka_message_ms_since_start
            .store(self.now_ms_since_start(), Ordering::Relaxed);
    }

    pub(crate) fn note_infrastructure_error(&self) {
        self.last_infrastructure_error_ms_since_start
            .store(self.now_ms_since_start(), Ordering::Relaxed);
    }

    pub(crate) fn record_drop(&self, dropped: u64) {
        if dropped == 0 {
            return;
        }

        let now_ms_since_start = self.now_ms_since_start();
        self.dropped_messages
            .fetch_add(dropped, Ordering::Relaxed);
        self.last_drop_ms_since_start
            .store(now_ms_since_start, Ordering::Relaxed);

        match self.dropped_messages_window.lock() {
            Ok(mut guard) => guard.record(now_ms_since_start, dropped, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .record(now_ms_since_start, dropped, DROPPED_MESSAGES_WINDOW_MS),
        }
    }

    /// M-1069: Record a decode error in the sliding window.
    /// Call this when a decode error occurs (in addition to incrementing the atomic counter).
    pub(crate) fn record_decode_error(&self) {
        let now_ms_since_start = self.now_ms_since_start();
        match self.decode_errors_window.lock() {
            Ok(mut guard) => guard.record(now_ms_since_start, 1, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .record(now_ms_since_start, 1, DROPPED_MESSAGES_WINDOW_MS),
        }
    }

    /// M-1069/M-1105: Record a message received in the sliding window.
    /// Call this for each Kafka message processed (success or error).
    ///
    /// M-1105: Now uses lockless atomic CAS operations instead of mutex,
    /// eliminating contention on the hot path (called per-message).
    #[inline]
    pub(crate) fn record_message_received(&self) {
        self.messages_received_window.record(1);
    }

    /// M-1095: Record a send failure in the sliding window.
    /// Call this when a WebSocket send operation fails (in addition to incrementing the atomic counter).
    pub(crate) fn record_send_failed(&self) {
        let now_ms_since_start = self.now_ms_since_start();
        match self.send_failed_window.lock() {
            Ok(mut guard) => guard.record(now_ms_since_start, 1, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .record(now_ms_since_start, 1, DROPPED_MESSAGES_WINDOW_MS),
        }
    }

    /// M-1095: Record a send timeout in the sliding window.
    /// Call this when a WebSocket send operation times out (in addition to incrementing the atomic counter).
    pub(crate) fn record_send_timeout(&self) {
        let now_ms_since_start = self.now_ms_since_start();
        match self.send_timeout_window.lock() {
            Ok(mut guard) => guard.record(now_ms_since_start, 1, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .record(now_ms_since_start, 1, DROPPED_MESSAGES_WINDOW_MS),
        }
    }

    /// Create a serializable snapshot of current metrics
    /// This reads all atomic counters and combines with timestamp data
    pub(crate) fn snapshot(&self) -> ServerMetricsSnapshot {
        let now_ms_since_start = self.now_ms_since_start();
        let uptime_seconds = self.start_time.elapsed().as_secs();

        let last_kafka_message_ago_seconds = {
            let last_ms = self
                .last_kafka_message_ms_since_start
                .load(Ordering::Relaxed);
            if last_ms == 0 {
                None
            } else {
                Some(now_ms_since_start.saturating_sub(last_ms) / MS_PER_SEC)
            }
        };

        let last_infrastructure_error_ago_seconds = {
            let last_ms = self
                .last_infrastructure_error_ms_since_start
                .load(Ordering::Relaxed);
            if last_ms == 0 {
                None
            } else {
                Some(now_ms_since_start.saturating_sub(last_ms) / MS_PER_SEC)
            }
        };

        let last_drop_ago_seconds = {
            let last_ms = self.last_drop_ms_since_start.load(Ordering::Relaxed);
            if last_ms == 0 {
                None
            } else {
                Some(now_ms_since_start.saturating_sub(last_ms) / MS_PER_SEC)
            }
        };

        let dropped_messages_last_120s = match self.dropped_messages_window.lock() {
            Ok(mut guard) => guard.sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
        };

        // M-1069: Windowed decode errors and messages for accurate recency-based error rate
        let decode_errors_last_120s = match self.decode_errors_window.lock() {
            Ok(mut guard) => guard.sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
        };

        // M-1105: Use lockless sliding window for message count (no mutex acquisition)
        let messages_last_120s = self.messages_received_window.sum_recent(DROPPED_MESSAGES_WINDOW_SECS);

        // M-1095: Windowed send failures for detecting "currently stuck" clients
        let send_failed_last_120s = match self.send_failed_window.lock() {
            Ok(mut guard) => guard.sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
        };

        let send_timeout_last_120s = match self.send_timeout_window.lock() {
            Ok(mut guard) => guard.sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
            Err(poisoned) => poisoned
                .into_inner()
                .sum_recent(now_ms_since_start, DROPPED_MESSAGES_WINDOW_MS),
        };

        ServerMetricsSnapshot {
            kafka_messages_received: self.kafka_messages_received.load(Ordering::Relaxed),
            kafka_errors: self.kafka_errors.load(Ordering::Relaxed),
            infrastructure_errors: self.infrastructure_errors.load(Ordering::Relaxed),
            connected_clients: self.connected_clients.load(Ordering::Relaxed),
            uptime_seconds,
            last_kafka_message_ago_seconds,
            dropped_messages: self.dropped_messages.load(Ordering::Relaxed),
            dropped_messages_last_120s,
            last_drop_ago_seconds,
            decode_errors: self.decode_errors.load(Ordering::Relaxed),
            old_data_decode_errors: self.old_data_decode_errors.load(Ordering::Relaxed),
            // M-1107: Surface payload_missing in health for data loss visibility
            payload_missing: self.payload_missing.load(Ordering::Relaxed),
            last_infrastructure_error_ago_seconds,
            // M-1069: Windowed metrics for recency-based health/circuit-breaker decisions
            decode_errors_last_120s,
            messages_last_120s,
            // M-1072 FIX: Send failure counters for operator visibility in /health
            send_failed: self.send_failed.load(Ordering::Relaxed),
            send_timeout: self.send_timeout.load(Ordering::Relaxed),
            // M-1095: Windowed send failures for detecting "currently stuck" clients
            send_failed_last_120s,
            send_timeout_last_120s,
        }
    }
}

/// Serializable view of ServerMetrics for health endpoint
/// This is what gets returned in JSON responses
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ServerMetricsSnapshot {
    /// Total Kafka messages processed since startup
    pub(crate) kafka_messages_received: u64,
    /// Total Kafka errors encountered
    pub(crate) kafka_errors: u64,
    /// Infrastructure errors (connection, DNS, network) - Issue #1: Separate from data errors
    pub(crate) infrastructure_errors: u64,
    /// Number of currently connected WebSocket clients
    pub(crate) connected_clients: usize,
    /// Server uptime in seconds
    pub(crate) uptime_seconds: u64,
    /// Last successful Kafka message timestamp
    pub(crate) last_kafka_message_ago_seconds: Option<u64>,
    /// Total messages dropped due to slow clients (lagged receivers)
    /// This counter indicates data loss when clients can't keep up with Kafka throughput
    pub(crate) dropped_messages: u64,
    /// Messages dropped due to slow clients within the last 120 seconds (M-1041)
    pub(crate) dropped_messages_last_120s: u64,
    /// Seconds since the last client drop event (0/None = never)
    pub(crate) last_drop_ago_seconds: Option<u64>,
    /// Total protobuf decode errors (Issue #11: buffer underflow tracking)
    pub(crate) decode_errors: u64,
    /// Decode errors from old data (pre-session, can be safely skipped)
    pub(crate) old_data_decode_errors: u64,
    /// M-1107: Messages with missing payload (tombstone/corruption/compaction)
    /// Indicates data loss - any value > 0 should be investigated
    pub(crate) payload_missing: u64,
    /// Seconds since last infrastructure error (S-25 fix: enables accurate recency checks)
    pub(crate) last_infrastructure_error_ago_seconds: Option<u64>,
    /// M-1069: Decode errors within the last 120 seconds (numerator for windowed error rate)
    pub(crate) decode_errors_last_120s: u64,
    /// M-1069: Kafka messages received within the last 120 seconds (denominator for windowed error rate)
    pub(crate) messages_last_120s: u64,
    /// M-1072 FIX: Total WebSocket send operations that failed (excluding timeouts).
    /// Indicates client disconnections mid-send or network issues.
    pub(crate) send_failed: u64,
    /// M-1072 FIX: Total WebSocket send operations that timed out.
    /// High values indicate wedged/slow clients causing server-side backpressure.
    pub(crate) send_timeout: u64,
    /// M-1095: Send failures within the last 120 seconds.
    /// Use this to detect "currently stuck" clients quickly (vs lifetime counter).
    pub(crate) send_failed_last_120s: u64,
    /// M-1095: Send timeouts within the last 120 seconds.
    /// Use this to detect "currently stuck" clients quickly (vs lifetime counter).
    pub(crate) send_timeout_last_120s: u64,
}

// =============================================================================
// Prometheus Metrics Collector
// =============================================================================

/// Prometheus collector for websocket server metrics backed by atomic counters.
///
/// `/metrics` should only gather from the registry; this collector bridges atomic counters into
/// Prometheus metric families at scrape time.
pub(crate) struct WebsocketServerMetricsCollector {
    metrics: Arc<ServerMetrics>,
    replay_buffer: ReplayBuffer,
    /// M-1035: Broadcast sender for accurate connected_clients count via receiver_count().
    /// This ensures `/metrics` and `/health` report the same value.
    broadcast_tx: broadcast::Sender<OutboundBinaryMessage>,
    descs: Vec<Desc>,
}

impl WebsocketServerMetricsCollector {
    /// M-1035: Now requires broadcast_tx to ensure `/metrics` uses the same
    /// connected_clients source as `/health` (receiver_count instead of atomic).
    pub fn new(
        metrics: Arc<ServerMetrics>,
        replay_buffer: ReplayBuffer,
        broadcast_tx: broadcast::Sender<OutboundBinaryMessage>,
    ) -> Self {
        let descs = vec![
            Desc::new(
                "websocket_kafka_messages_total".to_string(),
                "Total Kafka messages processed (success=decoded, error=new decode failure, old_data_error=skipped old data decode failure)".to_string(),
                vec!["status".to_string()],
                HashMap::new(),
            )
            .expect("valid websocket_kafka_messages_total desc"),
            Desc::new(
                "websocket_connected_clients".to_string(),
                "Number of currently connected WebSocket clients".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_connected_clients desc"),
            // M-1026: Clarified help text - this counts per-client drops, not unique messages
            Desc::new(
                "websocket_dropped_messages_total".to_string(),
                "Total messages dropped across all clients (N clients × M drops = N×M counted). Use websocket_client_lag_events_total for event-based alerting.".to_string(),
                vec!["reason".to_string()],
                HashMap::new(),
            )
            .expect("valid websocket_dropped_messages_total desc"),
            Desc::new(
                "websocket_uptime_seconds".to_string(),
                "Server uptime in seconds".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_uptime_seconds desc"),
            // M-1042: Expose recency gauges used by /health for Prometheus alerting.
            // Sentinel: -1 means "never".
            Desc::new(
                "websocket_last_kafka_message_age_seconds".to_string(),
                "Seconds since last Kafka message received (-1 = never)".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_last_kafka_message_age_seconds desc"),
            Desc::new(
                "websocket_last_infrastructure_error_age_seconds".to_string(),
                "Seconds since last infrastructure error (-1 = never)".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_last_infrastructure_error_age_seconds desc"),
            Desc::new(
                "websocket_infrastructure_errors_total".to_string(),
                "Total infrastructure errors (network, Kafka connection)".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_infrastructure_errors_total desc"),
            Desc::new(
                "websocket_old_data_decode_errors_total".to_string(),
                "Total decode errors from old/pre-cached data".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_old_data_decode_errors_total desc"),
            Desc::new(
                "replay_buffer_memory_hits_total".to_string(),
                "Replay requests served from memory".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid replay_buffer_memory_hits_total desc"),
            Desc::new(
                "replay_buffer_redis_hits_total".to_string(),
                "Replay requests served from Redis".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid replay_buffer_redis_hits_total desc"),
            Desc::new(
                "replay_buffer_redis_misses_total".to_string(),
                "Replay requests not found in Redis".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid replay_buffer_redis_misses_total desc"),
            Desc::new(
                "replay_buffer_redis_write_dropped_total".to_string(),
                "Replay Redis writes dropped due to concurrency limiting".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid replay_buffer_redis_write_dropped_total desc"),
            Desc::new(
                "replay_buffer_redis_write_failures_total".to_string(),
                "Replay Redis write failures".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid replay_buffer_redis_write_failures_total desc"),
            // M-1033: WebSocket send error counters
            Desc::new(
                "websocket_send_failed_total".to_string(),
                "Total WebSocket send operations that failed (excluding timeouts)".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_send_failed_total desc"),
            Desc::new(
                "websocket_send_timeout_total".to_string(),
                "Total WebSocket send operations that timed out".to_string(),
                Vec::new(),
                HashMap::new(),
            )
            .expect("valid websocket_send_timeout_total desc"),
        ];

        Self {
            metrics,
            replay_buffer,
            broadcast_tx,
            descs,
        }
    }

    fn label_pair(name: &'static str, value: &'static str) -> LabelPair {
        let mut lp = LabelPair::default();
        lp.set_name(name.to_string());
        lp.set_value(value.to_string());
        lp
    }

    fn make_counter_metric(value: u64, labels: Vec<LabelPair>) -> ProtoMetric {
        let mut metric = ProtoMetric::default();
        metric.set_label(labels.into());
        let mut counter = ProtoCounter::default();
        counter.set_value(value as f64);
        metric.set_counter(counter);
        metric
    }

    fn make_gauge_metric(value: f64) -> ProtoMetric {
        let mut metric = ProtoMetric::default();
        let mut gauge = ProtoGauge::default();
        gauge.set_value(value);
        metric.set_gauge(gauge);
        metric
    }

    fn single_counter_family(name: &str, help: &str, value: u64) -> MetricFamily {
        let mut family = MetricFamily::default();
        family.set_name(name.to_string());
        family.set_help(help.to_string());
        family.set_field_type(MetricType::COUNTER);
        family
            .mut_metric()
            .push(Self::make_counter_metric(value, Vec::new()));
        family
    }

    fn labeled_counter_family(
        name: &str,
        help: &str,
        series: Vec<(Vec<LabelPair>, u64)>,
    ) -> MetricFamily {
        let mut family = MetricFamily::default();
        family.set_name(name.to_string());
        family.set_help(help.to_string());
        family.set_field_type(MetricType::COUNTER);
        for (labels, value) in series {
            family
                .mut_metric()
                .push(Self::make_counter_metric(value, labels));
        }
        family
    }

    fn single_gauge_family(name: &str, help: &str, value: f64) -> MetricFamily {
        let mut family = MetricFamily::default();
        family.set_name(name.to_string());
        family.set_help(help.to_string());
        family.set_field_type(MetricType::GAUGE);
        family.mut_metric().push(Self::make_gauge_metric(value));
        family
    }
}

impl Collector for WebsocketServerMetricsCollector {
    fn desc(&self) -> Vec<&Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let success = self.metrics.kafka_messages_success.load(Ordering::Relaxed);
        let errors = self.metrics.kafka_messages_error.load(Ordering::Relaxed);

        // M-1035: Use receiver_count() instead of atomic counter to match /health endpoint.
        // This ensures `/metrics` and `/health` always report the same value.
        let connected_clients = self.broadcast_tx.receiver_count() as f64;
        let dropped_messages = self.metrics.dropped_messages.load(Ordering::Relaxed);
        let uptime_seconds = self.metrics.start_time.elapsed().as_secs() as f64;
        let infrastructure_errors = self.metrics.infrastructure_errors.load(Ordering::Relaxed);
        let old_data_decode_errors = self.metrics.old_data_decode_errors.load(Ordering::Relaxed);
        let now_ms_since_start = self.metrics.now_ms_since_start();

        let last_kafka_message_age_seconds = {
            let last_ms = self
                .metrics
                .last_kafka_message_ms_since_start
                .load(Ordering::Relaxed);
            if last_ms == 0 {
                -1.0
            } else {
                (now_ms_since_start.saturating_sub(last_ms) as f64) / (MS_PER_SEC as f64)
            }
        };

        let last_infrastructure_error_age_seconds = {
            let last_ms = self
                .metrics
                .last_infrastructure_error_ms_since_start
                .load(Ordering::Relaxed);
            if last_ms == 0 {
                -1.0
            } else {
                (now_ms_since_start.saturating_sub(last_ms) as f64) / (MS_PER_SEC as f64)
            }
        };

        let replay = self.replay_buffer.snapshot_metrics();

        vec![
            Self::labeled_counter_family(
                "websocket_kafka_messages_total",
                "Total Kafka messages processed (success=decoded, error=new decode failure, old_data_error=skipped old data decode failure)",
                vec![
                    (vec![Self::label_pair("status", "success")], success),
                    (vec![Self::label_pair("status", "error")], errors),
                    (
                        vec![Self::label_pair("status", "old_data_error")],
                        old_data_decode_errors,
                    ),
                ],
            ),
            Self::single_gauge_family(
                "websocket_connected_clients",
                "Number of currently connected WebSocket clients",
                connected_clients,
            ),
            // M-1026: Clarified help text - this counts per-client drops, not unique messages
            Self::labeled_counter_family(
                "websocket_dropped_messages_total",
                "Total messages dropped across all clients (N clients × M drops = N×M counted)",
                vec![(
                    vec![Self::label_pair("reason", "lagged_receiver")],
                    dropped_messages,
                )],
            ),
            Self::single_gauge_family(
                "websocket_uptime_seconds",
                "Server uptime in seconds",
                uptime_seconds,
            ),
            Self::single_gauge_family(
                "websocket_last_kafka_message_age_seconds",
                "Seconds since last Kafka message received (-1 = never)",
                last_kafka_message_age_seconds,
            ),
            Self::single_gauge_family(
                "websocket_last_infrastructure_error_age_seconds",
                "Seconds since last infrastructure error (-1 = never)",
                last_infrastructure_error_age_seconds,
            ),
            Self::single_counter_family(
                "websocket_infrastructure_errors_total",
                "Total infrastructure errors (network, Kafka connection)",
                infrastructure_errors,
            ),
            Self::single_counter_family(
                "websocket_old_data_decode_errors_total",
                "Total decode errors from old/pre-cached data",
                old_data_decode_errors,
            ),
            Self::single_counter_family(
                "replay_buffer_memory_hits_total",
                "Replay requests served from memory",
                replay.memory_hits,
            ),
            Self::single_counter_family(
                "replay_buffer_redis_hits_total",
                "Replay requests served from Redis",
                replay.redis_hits,
            ),
            Self::single_counter_family(
                "replay_buffer_redis_misses_total",
                "Replay requests not found in Redis",
                replay.redis_misses,
            ),
            Self::single_counter_family(
                "replay_buffer_redis_write_dropped_total",
                "Replay Redis writes dropped due to concurrency limiting",
                replay.redis_write_dropped,
            ),
            Self::single_counter_family(
                "replay_buffer_redis_write_failures_total",
                "Replay Redis write failures",
                replay.redis_write_failures,
            ),
            // M-1033: WebSocket send error counters
            Self::single_counter_family(
                "websocket_send_failed_total",
                "Total WebSocket send operations that failed (excluding timeouts)",
                self.metrics.send_failed.load(Ordering::Relaxed),
            ),
            Self::single_counter_family(
                "websocket_send_timeout_total",
                "Total WebSocket send operations that timed out",
                self.metrics.send_timeout.load(Ordering::Relaxed),
            ),
        ]
    }
}

// =============================================================================
// Decode Error Logging
// =============================================================================

/// Structured log entry for decode errors (Issue #17: Root cause analysis)
///
/// This provides machine-readable JSON logs for:
/// - Correlation across distributed systems (trace_id)
/// - Message metadata analysis (size, headers, timestamp)
/// - Pattern detection (error types, timing)
/// - Offline replay and debugging
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DecodeErrorLog {
    /// Log level (always "ERROR" for decode failures)
    pub level: String,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Unique correlation ID for tracing this event across systems
    pub trace_id: String,
    /// Error message
    pub error: String,
    /// Error category for grouping
    pub error_type: String,
    /// Message size in bytes
    pub message_size: usize,
    /// First 16 bytes as hex string (for signature analysis)
    pub first_bytes_hex: String,
    /// Last 16 bytes as hex string (for corruption detection)
    pub last_bytes_hex: String,
    /// Kafka message count (position in stream)
    pub message_count: u64,
    /// Kafka partition (if available)
    pub kafka_partition: Option<i32>,
    /// Kafka offset (if available)
    pub kafka_offset: Option<i64>,
    /// Component name
    pub component: String,
}

impl DecodeErrorLog {
    /// Create a new structured error log entry
    pub fn new(
        error_msg: String,
        error_type: String,
        binary_data: &[u8],
        message_count: u64,
        kafka_partition: Option<i32>,
        kafka_offset: Option<i64>,
    ) -> Self {
        let msg_size = binary_data.len();
        let first_bytes_hex = if msg_size >= 16 {
            format!("{:02x?}", &binary_data[..16])
        } else {
            format!("{:02x?}", binary_data)
        };
        let last_bytes_hex = if msg_size >= 16 {
            format!("{:02x?}", &binary_data[msg_size - 16..])
        } else {
            String::new()
        };

        DecodeErrorLog {
            level: "ERROR".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            trace_id: Uuid::new_v4().to_string(),
            error: error_msg,
            error_type,
            message_size: msg_size,
            first_bytes_hex,
            last_bytes_hex,
            message_count,
            kafka_partition,
            kafka_offset,
            component: "websocket-server".to_string(),
        }
    }

    /// Log as JSON to stderr
    pub fn log_json(&self) {
        if let Ok(json) = serde_json::to_string(&self) {
            eprintln!("{}", json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_proxy_ip_parser_handles_empty_and_invalid_entries() {
        assert!(parse_trusted_proxy_ips("").is_empty());
        assert!(parse_trusted_proxy_ips(" , , ").is_empty());

        let ips = parse_trusted_proxy_ips("127.0.0.1, ::1, not_an_ip, 10.0.0.1");
        assert!(ips.contains(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(ips.contains(&"::1".parse::<IpAddr>().unwrap()));
        assert!(ips.contains(&"10.0.0.1".parse::<IpAddr>().unwrap()));
        assert_eq!(ips.len(), 3);
    }

    #[test]
    fn decode_error_policy_parse_and_as_str() {
        assert_eq!(DecodeErrorPolicy::parse(""), DecodeErrorPolicy::Skip);
        assert_eq!(DecodeErrorPolicy::parse("skip"), DecodeErrorPolicy::Skip);
        assert_eq!(DecodeErrorPolicy::parse(" pause "), DecodeErrorPolicy::Pause);
        assert_eq!(DecodeErrorPolicy::parse("unknown"), DecodeErrorPolicy::Skip);

        assert_eq!(DecodeErrorPolicy::Skip.as_str(), "skip");
        assert_eq!(DecodeErrorPolicy::Pause.as_str(), "pause");
    }

    #[tokio::test]
    async fn connection_rate_limiter_enforces_max_connections_per_ip() {
        let limiter = ConnectionRateLimiter::new(2, None);
        let ip = "192.0.2.1";

        assert!(limiter.try_acquire(ip).await);
        assert!(limiter.try_acquire(ip).await);
        assert!(!limiter.try_acquire(ip).await);

        limiter.release(ip).await;
        assert!(limiter.try_acquire(ip).await);

        limiter.release(ip).await;
        limiter.release(ip).await;
        limiter.release(ip).await;

        let connections = limiter.connections_by_ip.read().await;
        assert!(connections.get(ip).is_none());
    }

    #[test]
    fn sliding_count_window_records_and_prunes_with_boundary_inclusive() {
        let mut window = SlidingCountWindow::default();
        let window_ms = 5_000;

        window.record(0, 1, window_ms);
        window.record(1_000, 2, window_ms);
        window.record(2_000, 0, window_ms);

        assert_eq!(window.sum_recent(2_000, window_ms), 3);
        assert_eq!(window.sum_recent(6_000, window_ms), 2);
        assert_eq!(window.sum_recent(6_001, window_ms), 0);
    }

    #[test]
    fn decode_error_log_sets_size_and_hex_fields() {
        let small = DecodeErrorLog::new(
            "oops".to_string(),
            "decode".to_string(),
            &[0, 1, 2],
            7,
            Some(1),
            Some(42),
        );
        assert_eq!(small.message_size, 3);
        assert!(!small.first_bytes_hex.is_empty());
        assert!(small.last_bytes_hex.is_empty());
        assert_eq!(small.message_count, 7);
        assert_eq!(small.kafka_partition, Some(1));
        assert_eq!(small.kafka_offset, Some(42));

        let bytes: Vec<u8> = (0u8..32u8).collect();
        let large = DecodeErrorLog::new(
            "oops".to_string(),
            "decode".to_string(),
            &bytes,
            8,
            None,
            None,
        );
        assert_eq!(large.message_size, 32);
        assert!(!large.first_bytes_hex.is_empty());
        assert!(!large.last_bytes_hex.is_empty());
    }
}
