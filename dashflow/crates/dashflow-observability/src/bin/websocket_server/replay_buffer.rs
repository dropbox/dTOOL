// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Replay buffer for message persistence and recovery.
//!
//! This module provides a hybrid memory + Redis replay buffer that enables
//! clients to recover missed messages after connection interruptions.

use bytes::Bytes;
use prometheus::{HistogramVec, IntCounter};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, Semaphore};

// Import helper function from parent module
use super::parse_env_var_with_warning;

// M-1010: Maximum allowed thread_id length before hashing.
// Thread IDs longer than this are hashed to prevent Redis key size explosion.
const MAX_THREAD_ID_LENGTH: usize = 128;

// M-1010: Maximum safe Redis score value (same as MAX_SAFE_INTEGER for JS compatibility).
// Used for precision warnings when sequences/offsets exceed this threshold.
const MAX_SAFE_REDIS_SCORE: i64 = 9_007_199_254_740_991;

/// M-1010: Sanitize thread_id for use in Redis keys.
/// M-1013: Updated to use base64url encoding instead of hashing for collision-free keys.
///
/// Thread IDs are user-controlled and can contain:
/// - Colons (`:`) which interfere with Redis key parsing
/// - Very long strings which cause key size explosion
/// - Special characters that may cause parsing issues
///
/// This function:
/// 1. Encodes thread_ids that are too long (>MAX_THREAD_ID_LENGTH) or contain unsafe chars
/// 2. Uses URL-safe base64 encoding (RFC 4648 §5) which is:
///    - **Collision-free**: Different inputs always produce different outputs
///    - **Reversible**: Can decode back to original for debugging
///    - **Redis-safe**: Uses only alphanumeric chars plus `-` and `_`
/// 3. Returns safe thread_ids as-is for debugging clarity
///
/// Encoded keys use `b64_` prefix to distinguish from raw keys.
fn sanitize_thread_id_for_redis(thread_id: &str) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    // Check if sanitization is needed:
    // - Length > MAX_THREAD_ID_LENGTH (to bound key size)
    // - Contains `:` (interferes with Redis key segment parsing)
    // - Contains any non-alphanumeric/underscore/hyphen chars (future-proofing)
    let needs_encoding = thread_id.len() > MAX_THREAD_ID_LENGTH
        || thread_id.contains(':')
        || !thread_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

    if needs_encoding {
        // M-1013: Use URL-safe base64 encoding (collision-free, reversible)
        let encoded = URL_SAFE_NO_PAD.encode(thread_id.as_bytes());
        // Prefix with "b64_" to indicate encoding method
        format!("b64_{}", encoded)
    } else {
        // Safe to use as-is (no problematic characters, reasonable length)
        thread_id.to_string()
    }
}

/// M-1013: Compute the legacy hash-based key for backward compatibility reads.
/// This mirrors the pre-M-1013 behavior for fallback lookups.
///
/// **NOTE (M-1032):** This legacy function only checks `:` and length for backward
/// compatibility with pre-M-1013 keys. The current `sanitize_thread_id_for_redis`
/// function correctly handles ALL unsafe characters (whitespace, control chars,
/// unicode, etc.) via the strict character allowlist check. This legacy function
/// is ONLY used for reading old keys, not for creating new ones.
fn legacy_hash_thread_id_for_redis(thread_id: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Legacy behavior: only check `:` and length (pre-M-1013)
    // Do NOT change this - it must match the old key format for backward compat
    let needs_hash = thread_id.len() > MAX_THREAD_ID_LENGTH || thread_id.contains(':');

    if needs_hash {
        let mut hasher = DefaultHasher::new();
        thread_id.hash(&mut hasher);
        let hash = hasher.finish();
        format!("h_{:016x}", hash)
    } else {
        thread_id.to_string()
    }
}

// Types needed for replay buffer
/// Message stored in replay buffer with metadata for sequence tracking
///
/// M-995/M-996: Uses `Bytes` for zero-copy cloning when broadcasting to multiple
/// WebSocket clients. Cloning `Bytes` is O(1) (atomic ref-count bump) vs O(n) for `Vec<u8>`.
#[derive(Clone, Debug)]
pub(crate) struct MessageWithMetadata {
    /// Binary protobuf message data (M-995: Bytes for zero-copy broadcast)
    pub(crate) data: Bytes,
    /// Kafka partition the message was read from
    pub(crate) partition: i32,
    /// Kafka offset the message was read from
    pub(crate) offset: i64,
    /// thread_id from DashStream Header (if available)
    pub(crate) thread_id: Option<String>,
    /// Sequence number from QualityEvent (if available)
    pub(crate) sequence: Option<u64>,
    /// Timestamp when message was received
    #[allow(dead_code)] // Architectural: Reserved for future TTL/eviction logic
    pub(crate) timestamp: Instant,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct KafkaCursor {
    pub(crate) partition: i32,
    pub(crate) offset: i64,
}

/// Outbound message for WebSocket broadcast and replay.
///
/// M-995/M-996: Uses `Bytes` for efficient zero-copy cloning when:
/// - Broadcasting to N WebSocket clients (O(1) clone vs O(message_size) per client)
/// - Storing in replay buffer (single allocation shared between broadcast + replay)
#[derive(Clone, Debug)]
pub(crate) struct OutboundBinaryMessage {
    /// Binary protobuf message data (M-995: Bytes for zero-copy broadcast)
    pub(crate) data: Bytes,
    pub(crate) cursor: KafkaCursor,
}

/// Redis writes are **best-effort**: they are spawned as non-blocking background
/// tasks and may fail silently. This design prioritizes low-latency message
/// processing. For durability guarantees, rely on Kafka's replay capability.
/// See [`add_message()`](Self::add_message) for details.
#[derive(Clone)]
pub(crate) struct ReplayBuffer {
    /// Fast path: Recent messages in memory
    memory: Arc<RwLock<VecDeque<MessageWithMetadata>>>,
    max_memory_size: usize,

    /// Persistent path: Redis for longer retention
    redis: Option<Arc<redis::aio::ConnectionManager>>,
    redis_key_prefix: String,
    /// S-13: Configurable Redis TTL (seconds) via REDIS_MESSAGE_TTL_SECS env var
    redis_message_ttl_secs: u64,
    /// M-728: Configurable ZCARD check cadence via REDIS_ZCARD_CHECK_CADENCE env var
    redis_zcard_check_cadence: u64,
    /// M-767: Configurable max concurrent Redis writes via REDIS_MAX_CONCURRENT_WRITES env var
    redis_max_concurrent_writes: usize,
    /// M-767: Configurable max sequences to retain in Redis via REDIS_MAX_SEQUENCES env var
    redis_max_sequences: usize,

    /// Metrics (internal counters for text export)
    memory_hits: Arc<AtomicU64>,
    redis_hits: Arc<AtomicU64>,
    redis_misses: Arc<AtomicU64>,
    /// M-1022: Track current memory buffer size for operational visibility
    memory_buffer_size: Arc<AtomicU64>,
    /// Bound concurrent Redis write tasks to avoid unbounded `tokio::spawn` under load
    redis_write_semaphore: Arc<Semaphore>,
    redis_write_dropped: Arc<AtomicU64>,
    redis_write_failures: Arc<AtomicU64>,
    /// M-714: Counter for ZCARD cadence optimization
    redis_write_counter: Arc<AtomicU64>,

    /// Prometheus metrics (for /metrics endpoint)
    prom_connection_errors: Option<IntCounter>,
    prom_operation_latency: Option<HistogramVec>,
}

impl ReplayBuffer {
    /// Max number of recent sequences to retain in Redis
    pub(crate) const REDIS_MAX_SEQUENCES: usize = 10_000;
    /// Default TTL for individual message keys stored in Redis (seconds)
    const DEFAULT_REDIS_MESSAGE_TTL_SECS: u64 = 3600; // 1 hour
    /// Max number of concurrent background Redis writes
    const DEFAULT_MAX_CONCURRENT_REDIS_WRITES: usize = 100;
    /// M-698: Per-partition fetch limit for Redis replay queries.
    /// Each partition fetches up to this many messages per page.
    /// The replay loop continues if ANY partition returns exactly this many (may have more).
    const REDIS_PARTITION_PAGE_LIMIT: usize = 1000;
    /// M-714: Only check ZCARD (for trim) every N writes to reduce round-trips.
    /// Set to 0 to disable ZCARD optimization (check every write).
    /// M-728: Configurable via `REDIS_ZCARD_CHECK_CADENCE` env var. Lower values (e.g., 10)
    /// reduce burst spikes but add Redis round-trips. Higher values (e.g., 100) improve
    /// throughput but allow larger ZSET overshoot during bursts.
    const DEFAULT_REDIS_ZCARD_CHECK_CADENCE: u64 = 50;
    /// M-728: Burst threshold - if estimated writes since last ZCARD exceed this,
    /// force an immediate check regardless of cadence. Prevents excessive ZSET growth.
    const REDIS_ZCARD_BURST_THRESHOLD: u64 = 200;
    /// M-724: If Redis writes are saturated, wait briefly to propagate backpressure before dropping.
    const REDIS_WRITE_ACQUIRE_TIMEOUT_MS: u64 = 50;
    /// M-724: Bound how long a single Redis write task may hold a semaphore permit.
    const REDIS_WRITE_TASK_TIMEOUT_MS: u64 = 2_000;

    /// S-13: Get Redis message TTL from env var or use default
    fn get_redis_message_ttl_secs() -> u64 {
        let ttl = parse_env_var_with_warning(
            "REDIS_MESSAGE_TTL_SECS",
            Self::DEFAULT_REDIS_MESSAGE_TTL_SECS,
        );
        // M-742: Validate TTL is >= 1 to avoid SETEX/EXPIRE errors
        if ttl == 0 {
            eprintln!(
                "⚠️  Warning: REDIS_MESSAGE_TTL_SECS cannot be 0. Using default {}.",
                Self::DEFAULT_REDIS_MESSAGE_TTL_SECS
            );
            return Self::DEFAULT_REDIS_MESSAGE_TTL_SECS;
        }
        ttl
    }

    /// M-728: Get ZCARD check cadence from env var or use default.
    /// Lower values reduce burst spikes but increase Redis round-trips.
    fn get_redis_zcard_check_cadence() -> u64 {
        parse_env_var_with_warning(
            "REDIS_ZCARD_CHECK_CADENCE",
            Self::DEFAULT_REDIS_ZCARD_CHECK_CADENCE,
        )
    }

    /// M-767: Get max concurrent Redis writes from env var or use default.
    /// Controls backpressure on Redis writes. Lower values provide stronger
    /// backpressure but may cause more drops under load.
    fn get_max_concurrent_redis_writes() -> usize {
        parse_env_var_with_warning(
            "REDIS_MAX_CONCURRENT_WRITES",
            Self::DEFAULT_MAX_CONCURRENT_REDIS_WRITES as u64,
        ) as usize
    }

    /// M-767: Get max sequences to retain in Redis ZSET from env var or use default.
    /// Higher values retain more replay history but consume more memory/storage.
    fn get_redis_max_sequences() -> usize {
        parse_env_var_with_warning(
            "REDIS_MAX_SEQUENCES",
            Self::REDIS_MAX_SEQUENCES as u64,
        ) as usize
    }

    fn redis_trim_stop(sorted_set_size: usize, max_sequences: usize) -> Option<isize> {
        if sorted_set_size > max_sequences {
            Some((sorted_set_size - (max_sequences + 1)) as isize)
        } else {
            None
        }
    }

    fn parse_seq_from_redis_key(key: &str) -> Option<u64> {
        key.rsplit_once(':')
            .and_then(|(_, suffix)| suffix.parse::<u64>().ok())
    }

    fn parse_offset_from_redis_key(key: &str) -> Option<i64> {
        key.rsplit_once(":offset:")
            .and_then(|(_, suffix)| suffix.parse::<i64>().ok())
    }

    /// Create hybrid replay buffer with Redis backing
    pub(crate) async fn new_with_redis(
        max_memory_size: usize,
        redis_url: &str,
        key_prefix: &str,
        prom_connection_errors: Option<IntCounter>,
        prom_operation_latency: Option<HistogramVec>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = redis::Client::open(redis_url)?;
        let conn_manager = redis::aio::ConnectionManager::new(client).await?;

        println!("✅ Redis replay buffer connected: {}", redis_url);

        let redis_message_ttl_secs = Self::get_redis_message_ttl_secs();
        println!("   Redis message TTL: {} seconds", redis_message_ttl_secs);

        let redis_zcard_check_cadence = Self::get_redis_zcard_check_cadence();
        println!(
            "   Redis ZCARD check cadence: {} writes (burst threshold: {})",
            redis_zcard_check_cadence, Self::REDIS_ZCARD_BURST_THRESHOLD
        );

        // M-767: Configurable capacity and concurrency
        let redis_max_concurrent_writes = Self::get_max_concurrent_redis_writes();
        let redis_max_sequences = Self::get_redis_max_sequences();
        println!(
            "   Redis max concurrent writes: {}, max sequences: {}",
            redis_max_concurrent_writes, redis_max_sequences
        );

        Ok(Self {
            memory: Arc::new(RwLock::new(VecDeque::with_capacity(max_memory_size))),
            max_memory_size,
            redis: Some(Arc::new(conn_manager)),
            redis_key_prefix: key_prefix.to_string(),
            redis_message_ttl_secs,
            redis_zcard_check_cadence,
            redis_max_concurrent_writes,
            redis_max_sequences,
            memory_hits: Arc::new(AtomicU64::new(0)),
            redis_hits: Arc::new(AtomicU64::new(0)),
            redis_misses: Arc::new(AtomicU64::new(0)),
            memory_buffer_size: Arc::new(AtomicU64::new(0)), // M-1022
            redis_write_semaphore: Arc::new(Semaphore::new(redis_max_concurrent_writes)),
            redis_write_dropped: Arc::new(AtomicU64::new(0)),
            redis_write_failures: Arc::new(AtomicU64::new(0)),
            redis_write_counter: Arc::new(AtomicU64::new(0)),
            prom_connection_errors,
            prom_operation_latency,
        })
    }

    /// Create memory-only replay buffer (fallback)
    pub(crate) fn new_memory_only(max_memory_size: usize) -> Self {
        println!("⚠️  Redis replay buffer disabled - using memory only");
        // M-767: Still read config values for consistency (used by drain_pending_redis_writes)
        let redis_max_concurrent_writes = Self::get_max_concurrent_redis_writes();
        let redis_max_sequences = Self::get_redis_max_sequences();
        Self {
            memory: Arc::new(RwLock::new(VecDeque::with_capacity(max_memory_size))),
            max_memory_size,
            redis: None,
            redis_key_prefix: "replay".to_string(),
            redis_message_ttl_secs: Self::DEFAULT_REDIS_MESSAGE_TTL_SECS, // Not used without Redis
            redis_zcard_check_cadence: Self::DEFAULT_REDIS_ZCARD_CHECK_CADENCE, // Not used without Redis
            redis_max_concurrent_writes, // M-767: Used by drain_pending_redis_writes
            redis_max_sequences,         // Not used without Redis
            memory_hits: Arc::new(AtomicU64::new(0)),
            redis_hits: Arc::new(AtomicU64::new(0)),
            redis_misses: Arc::new(AtomicU64::new(0)),
            memory_buffer_size: Arc::new(AtomicU64::new(0)), // M-1022
            redis_write_semaphore: Arc::new(Semaphore::new(redis_max_concurrent_writes)),
            redis_write_dropped: Arc::new(AtomicU64::new(0)),
            redis_write_failures: Arc::new(AtomicU64::new(0)),
            redis_write_counter: Arc::new(AtomicU64::new(0)),
            prom_connection_errors: None,
            prom_operation_latency: None,
        }
    }

    /// Add message to both memory and Redis.
    ///
    /// Stores two replay cursors:
    /// - Per-thread `sequence` (legacy; not globally meaningful)
    /// - Kafka `(partition, offset)` (global catch-up cursor)
    ///
    /// # Durability Model (M-494)
    ///
    /// - **Memory writes** are synchronous and guaranteed to succeed before this
    ///   method returns. Memory storage is bounded by `max_memory_size`.
    ///
    /// - **Redis writes** are **best-effort and bounded-latency**. They are spawned
    ///   as background tasks and may fail without notification to the caller:
    ///   - If the Redis write semaphore cannot be acquired quickly, the write is
    ///     dropped and counted in
    ///     `redis_write_dropped` metric
    ///   - If the Redis write fails, the error is logged and counted in
    ///     `redis_write_failures` metric
    ///   - If a Redis write task stalls/hangs, it times out to release its permit
    ///     (also counted in `redis_write_failures`)
    ///   - The caller cannot know if Redis persistence succeeded
    ///
    /// This design prioritizes low-latency message processing over guaranteed
    /// Redis persistence. For durability guarantees, rely on Kafka's replay
    /// capability rather than Redis.
    ///
    /// # Metrics
    ///
    /// Monitor these metrics for Redis write health:
    /// - `replay_buffer_redis_write_dropped_total`: Writes dropped due to semaphore exhaustion
    /// - `replay_buffer_redis_write_failures_total`: Writes that failed during execution
    ///
    /// M-1001: Add `thread_sequences` parameter to support multi-thread EventBatch indexing.
    /// When `thread_sequences` is Some, all thread_id -> max_seq pairs are indexed for replay.
    /// When None, falls back to single `thread_id` + `sequence` for backwards compatibility.
    ///
    /// M-995/M-996: Accepts `Bytes` for zero-copy cloning. The same `Bytes` instance is
    /// shared between the broadcast channel and replay buffer, avoiding duplicate allocations.
    pub(crate) async fn add_message(
        &self,
        data: Bytes,
        thread_id: Option<String>,
        sequence: Option<u64>,
        partition: i32,
        offset: i64,
        thread_sequences: Option<std::collections::HashMap<String, u64>>,
    ) {
        let thread_id_for_redis = thread_id.clone();

        // Always add to memory (fast path)
        // M-995: Bytes::clone() is O(1) (atomic ref-count bump)
        {
            let mut memory = self.memory.write().await;
            let popped = if memory.len() >= self.max_memory_size {
                memory.pop_front();
                true
            } else {
                false
            };
            memory.push_back(MessageWithMetadata {
                data: data.clone(),
                partition,
                offset,
                thread_id,
                sequence,
                timestamp: Instant::now(),
            });
            // M-1022: Update memory buffer size metric
            // Only increment if we didn't pop (otherwise size stays constant)
            if !popped {
                self.memory_buffer_size.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Also add to Redis (persistent path) - best effort.
        if let Some(redis) = &self.redis {
            let permit = match tokio::time::timeout(
                std::time::Duration::from_millis(Self::REDIS_WRITE_ACQUIRE_TIMEOUT_MS),
                self.redis_write_semaphore.clone().acquire_owned(),
            )
            .await
            {
                Ok(Ok(p)) => p,
                Ok(Err(_)) | Err(_) => {
                    self.redis_write_dropped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };

            let offset_key = format!(
                "{}:partition:{}:offset:{}",
                self.redis_key_prefix, partition, offset
            );
            let offsets_set_key =
                format!("{}:partition:{}:offsets", self.redis_key_prefix, partition);

            // M-1001: Build list of thread index entries.
            // If thread_sequences is provided (multi-thread EventBatch), use all of them.
            // Otherwise, use single thread_id + sequence for backwards compatibility.
            // M-1010: Sanitize thread_ids to prevent Redis key hygiene issues.
            let thread_entries: Vec<(String, String, u64)> = if let Some(ref ts) = thread_sequences {
                ts.iter()
                    .map(|(tid, seq)| {
                        let safe_tid = sanitize_thread_id_for_redis(tid);
                        (
                            format!("{}:thread:{}:seq:{}", self.redis_key_prefix, safe_tid, seq),
                            format!("{}:thread:{}:sequences", self.redis_key_prefix, safe_tid),
                            *seq,
                        )
                    })
                    .collect()
            } else if let (Some(tid), Some(seq)) = (thread_id_for_redis.as_deref(), sequence) {
                let safe_tid = sanitize_thread_id_for_redis(tid);
                vec![(
                    format!("{}:thread:{}:seq:{}", self.redis_key_prefix, safe_tid, seq),
                    format!("{}:thread:{}:sequences", self.redis_key_prefix, safe_tid),
                    seq,
                )]
            } else {
                vec![]
            };
            let mut conn = redis.as_ref().clone();
            let redis_write_failures = self.redis_write_failures.clone();
            let redis_write_counter = self.redis_write_counter.clone();
            let prom_connection_errors = self.prom_connection_errors.clone();
            let prom_operation_latency = self.prom_operation_latency.clone();
            let redis_message_ttl_secs = self.redis_message_ttl_secs; // S-13: Capture configurable TTL
            let redis_zcard_check_cadence = self.redis_zcard_check_cadence; // M-728: Capture configurable cadence
            let redis_max_sequences = self.redis_max_sequences; // M-767: Capture configurable max sequences
            let redis_write_timeout_ms = Self::REDIS_WRITE_TASK_TIMEOUT_MS;

            tokio::spawn(async move {
                use redis::AsyncCommands;

                let _permit = permit; // Keep permit alive until write completes
                let start = Instant::now();

                // M-689: Warn if offset exceeds MAX_SAFE_INTEGER (precision loss in f64)
                // This is extremely unlikely (would take ~292,000 years at 1M msg/sec)
                // but we warn for awareness. Full fix would require lexicographic storage.
                if offset > MAX_SAFE_REDIS_SCORE {
                    tracing::warn!(
                        partition = partition,
                        offset = offset,
                        max_safe = MAX_SAFE_REDIS_SCORE,
                        "Offset exceeds MAX_SAFE_INTEGER - Redis f64 score may lose precision"
                    );
                }

                let write_result = tokio::time::timeout(
                    std::time::Duration::from_millis(redis_write_timeout_ms),
                    async {
                        // M-714: Pipeline all mandatory writes (SETEX + ZADD + EXPIRE) into single round-trip.
                        // This reduces 3-6 sequential commands to 1 pipeline execution.
                        let mut pipe = redis::pipe();

                        // Offset-based storage (always)
                        // M-995: Dereference Bytes to &[u8] for Redis (Bytes doesn't impl ToRedisArgs)
                        pipe.cmd("SETEX")
                            .arg(&offset_key)
                            .arg(redis_message_ttl_secs)
                            .arg(&*data);
                        pipe.cmd("ZADD")
                            .arg(&offsets_set_key)
                            .arg(offset as f64)
                            .arg(&offset_key);
                        pipe.cmd("EXPIRE")
                            .arg(&offsets_set_key)
                            .arg(redis_message_ttl_secs as i64);

                        // Thread-based storage (legacy, optional)
                        // M-766: Include partition/offset metadata in thread-mode values so that
                        // replayed messages can include Kafka cursor info (required by M-720 cursor pairing).
                        // Format: [4 bytes partition (i32 LE)][8 bytes offset (i64 LE)][data...]
                        // M-1001: Iterate over all thread entries (supports multi-thread EventBatch)
                        for (thread_key, thread_set_key, seq) in &thread_entries {
                            // M-689: Warn if sequence exceeds MAX_SAFE_INTEGER
                            if *seq > MAX_SAFE_REDIS_SCORE as u64 {
                                tracing::warn!(
                                    thread_key = %thread_key,
                                    sequence = seq,
                                    max_safe = MAX_SAFE_REDIS_SCORE,
                                    "Sequence exceeds MAX_SAFE_INTEGER - Redis f64 score may lose precision"
                                );
                            }

                            // M-766: Pack partition/offset + data into single value
                            let mut packed = Vec::with_capacity(12 + data.len());
                            packed.extend_from_slice(&partition.to_le_bytes());
                            packed.extend_from_slice(&offset.to_le_bytes());
                            packed.extend_from_slice(&data);

                            pipe.cmd("SETEX")
                                .arg(thread_key)
                                .arg(redis_message_ttl_secs)
                                .arg(&packed);
                            pipe.cmd("ZADD")
                                .arg(thread_set_key)
                                .arg(*seq as f64)
                                .arg(thread_key);
                            pipe.cmd("EXPIRE")
                                .arg(thread_set_key)
                                .arg(redis_message_ttl_secs as i64);
                        }

                        // Execute the write pipeline
                        if let Err(e) = pipe.query_async::<()>(&mut conn).await {
                            redis_write_failures.fetch_add(1, Ordering::Relaxed);
                            if let Some(ref errors) = prom_connection_errors {
                                errors.inc();
                            }
                            eprintln!("⚠️  Redis replay pipeline write failed: {}", e);
                            return;
                        }

                        // M-714: Only check ZCARD (for trim) every N writes to reduce round-trips.
                        // Trim is eventually consistent - we don't need to check every single write.
                        // M-728: Also check if burst threshold exceeded to prevent excessive ZSET growth.
                        let write_num = redis_write_counter.fetch_add(1, Ordering::Relaxed);
                        let cadence_check = redis_zcard_check_cadence == 0
                            || write_num % redis_zcard_check_cadence == 0;
                        // M-728: Force check if writes since last reset exceed burst threshold
                        let burst_check = write_num >= Self::REDIS_ZCARD_BURST_THRESHOLD
                            && write_num % Self::REDIS_ZCARD_BURST_THRESHOLD == 0;
                        let should_check_trim = cadence_check || burst_check;

                        if should_check_trim {
                            // Trim offset ZSET if needed
                            // M-767: Use configurable max_sequences
                            match conn.zcard::<_, usize>(&offsets_set_key).await {
                                Ok(size) => {
                                    if let Some(stop) = Self::redis_trim_stop(size, redis_max_sequences) {
                                        if let Err(e) = conn
                                            .zremrangebyrank::<_, i64>(&offsets_set_key, 0isize, stop)
                                            .await
                                        {
                                            // Non-fatal for trim failures
                                            tracing::warn!(
                                                key = %offsets_set_key,
                                                error = %e,
                                                "Redis ZREMRANGEBYRANK failed for offset ZSET"
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        key = %offsets_set_key,
                                        error = %e,
                                        "Redis ZCARD failed for offset ZSET"
                                    );
                                }
                            }

                            // Trim thread ZSETs if present
                            // M-767: Use configurable max_sequences
                            // M-1001: Trim all thread ZSETs for multi-thread batches
                            for (_, thread_set_key, _) in &thread_entries {
                                match conn.zcard::<_, usize>(thread_set_key).await {
                                    Ok(size) => {
                                        if let Some(stop) = Self::redis_trim_stop(size, redis_max_sequences) {
                                            if let Err(e) = conn
                                                .zremrangebyrank::<_, i64>(thread_set_key, 0isize, stop)
                                                .await
                                            {
                                                tracing::warn!(
                                                    key = %thread_set_key,
                                                    error = %e,
                                                    "Redis ZREMRANGEBYRANK failed for thread ZSET"
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            key = %thread_set_key,
                                            error = %e,
                                            "Redis ZCARD failed for thread ZSET"
                                        );
                                    }
                                }
                            }
                        }

                        // Record write latency
                        if let Some(ref latency) = prom_operation_latency {
                            latency
                                .with_label_values(&["write"])
                                .observe(start.elapsed().as_secs_f64() * 1000.0);
                        }
                    },
                )
                .await;

                if write_result.is_err() {
                    redis_write_failures.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        timeout_ms = redis_write_timeout_ms,
                        "Redis replay write task timed out; releasing permit to avoid starvation"
                    );
                }
            });
        }
    }

    /// Legacy replay path for scalar cursors.
    ///
    /// Older clients use a single `lastSequence` which is not globally meaningful because
    /// DashStream sequences are per-thread_id. This method provides best-effort replay by
    /// scanning the in-memory arrival-ordered buffer and replaying everything after the
    /// first message with `sequence > last_sequence`.
    ///
    /// # Note
    /// Kept for backwards compatibility testing; production uses partition-offset resume.
    /// M-995: Returns `Bytes` for zero-copy cloning.
    #[cfg(test)]
    async fn get_messages_after_legacy(&self, last_sequence: u64) -> (Vec<Bytes>, Option<u64>) {
        let memory = self.memory.read().await;
        let mut messages = Vec::new();
        let mut started = false;
        let mut first_seq = None;

        for m in memory.iter() {
            if !started {
                if let Some(seq) = m.sequence {
                    if seq > last_sequence {
                        started = true;
                        first_seq = Some(seq);
                        messages.push(m.data.clone());
                    }
                }
            } else {
                messages.push(m.data.clone());
            }
        }

        if started {
            self.memory_hits.fetch_add(1, Ordering::Relaxed);
        }

        (messages, first_seq)
    }

    /// Get messages after per-thread cursors.
    ///
    /// DashStream `Header.sequence` is scoped to `Header.thread_id`, so replay must be too.
    ///
    /// M-766: Return type changed from `Vec<Vec<u8>>` to `Vec<OutboundBinaryMessage>`
    /// to include Kafka cursor metadata for cursor pairing (M-720).
    pub(crate) async fn get_messages_after_by_thread(
        &self,
        last_sequences_by_thread: &HashMap<String, u64>,
    ) -> (Vec<OutboundBinaryMessage>, Vec<(String, u64)>) {
        let mut thread_ids: Vec<&String> = last_sequences_by_thread.keys().collect();
        thread_ids.sort();

        let mut all_messages = Vec::new();
        let mut gaps = Vec::new();

        for thread_id in thread_ids {
            let last_seq = last_sequences_by_thread
                .get(thread_id)
                .copied()
                .unwrap_or(0);
            let (messages, gap) = self
                .get_messages_after_for_thread(thread_id.as_str(), last_seq)
                .await;
            all_messages.extend(messages);
            if let Some(gap_size) = gap {
                gaps.push((thread_id.to_string(), gap_size));
            }
        }

        (all_messages, gaps)
    }

    /// Get messages after per-partition offsets (global catch-up cursor).
    ///
    /// This supports catching up unknown/new threads that started while the UI was offline.
    ///
    /// M-676 Issue A: Automatically includes partitions the client hasn't seen yet.
    /// Missing partitions are treated as -1 (replay from earliest retained in buffer).
    ///
    /// M-698: Returns (messages, gaps, new_partitions, truncated_partitions) where
    /// truncated_partitions contains partition IDs that hit the per-partition page limit
    /// and may have more data available.
    pub(crate) async fn get_messages_after_by_partition(
        &self,
        last_offsets_by_partition: &HashMap<i32, i64>,
    ) -> (Vec<OutboundBinaryMessage>, Vec<(i32, i64)>, HashSet<i32>, HashSet<i32>) {
        // M-676: Discover all known partitions (client's + server's)
        let known_partitions = self.get_known_partitions().await;

        // Union: client-known partitions + server-known partitions
        let mut partitions: HashSet<i32> = last_offsets_by_partition.keys().copied().collect();
        let new_partitions: HashSet<i32> =
            known_partitions.difference(&partitions).copied().collect();
        partitions.extend(new_partitions.iter().copied());

        let mut sorted_partitions: Vec<i32> = partitions.iter().copied().collect();
        sorted_partitions.sort();

        let mut all_messages = Vec::new();
        let mut gaps = Vec::new();
        let mut truncated_partitions: HashSet<i32> = HashSet::new(); // M-698: Track partitions with more data

        for partition in sorted_partitions {
            // M-676: Partitions not in client request use -1 (replay from earliest)
            let last_offset = last_offsets_by_partition
                .get(&partition)
                .copied()
                .unwrap_or(-1);
            let (messages, gap, truncated) = self
                .get_messages_after_for_partition(partition, last_offset)
                .await;
            all_messages.extend(messages);
            if let Some(gap_size) = gap {
                gaps.push((partition, gap_size));
            }
            // M-698: Track partitions that hit the limit (may have more data)
            if truncated {
                truncated_partitions.insert(partition);
            }
        }

        (all_messages, gaps, new_partitions, truncated_partitions)
    }

    /// M-698: Returns (messages, gap, truncated) where truncated indicates Redis hit the limit
    /// M-995: Uses Bytes for zero-copy data handling.
    async fn get_messages_after_for_partition(
        &self,
        partition: i32,
        last_offset: i64,
    ) -> (Vec<OutboundBinaryMessage>, Option<i64>, bool) {
        // M-995: Use Bytes for zero-copy - Redis returns Vec<u8> which we convert to Bytes,
        // memory already stores Bytes (O(1) clone)
        let mut by_offset: std::collections::BTreeMap<i64, Bytes> =
            std::collections::BTreeMap::new();
        let mut redis_truncated = false; // M-698: Track if Redis hit the per-partition limit

        if let Some(redis) = &self.redis {
            let start = Instant::now();
            match self
                .fetch_from_redis_for_partition(partition, last_offset, redis)
                .await
            {
                Ok((items, hit_limit)) if !items.is_empty() => {
                    self.redis_hits.fetch_add(1, Ordering::Relaxed);
                    if let Some(ref latency) = self.prom_operation_latency {
                        latency
                            .with_label_values(&["read"])
                            .observe(start.elapsed().as_secs_f64() * 1000.0);
                    }
                    redis_truncated = hit_limit; // M-698: Propagate truncation indicator
                    for (offset, data) in items {
                        // M-995: Convert Vec<u8> from Redis to Bytes
                        by_offset.insert(offset, Bytes::from(data));
                    }
                }
                Ok(_) => {
                    self.redis_misses.fetch_add(1, Ordering::Relaxed);
                    if let Some(ref latency) = self.prom_operation_latency {
                        latency
                            .with_label_values(&["read"])
                            .observe(start.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "⚠️  Redis fetch failed for partition={} last_offset={}: {}",
                        partition, last_offset, e
                    );
                    self.redis_misses.fetch_add(1, Ordering::Relaxed);
                    if let Some(ref errors) = self.prom_connection_errors {
                        errors.inc();
                    }
                }
            }
        }

        // Memory is a best-effort supplement (e.g., for messages still in-flight to Redis).
        // We merge by offset and preserve per-partition order.
        {
            let memory = self.memory.read().await;
            let mut any = false;
            for m in memory.iter() {
                if m.partition != partition {
                    continue;
                }
                if m.offset <= last_offset {
                    continue;
                }
                any = true;
                // M-995: Bytes::clone() is O(1) (atomic ref-count bump)
                by_offset.insert(m.offset, m.data.clone());
            }
            if any {
                self.memory_hits.fetch_add(1, Ordering::Relaxed);
            }
        }

        let first_offset = by_offset.keys().next().copied();
        let messages: Vec<OutboundBinaryMessage> = by_offset
            .into_iter()
            .map(|(offset, data)| OutboundBinaryMessage {
                data,
                cursor: KafkaCursor { partition, offset },
            })
            .collect();

        let expected_next = last_offset.saturating_add(1);
        let gap = first_offset
            .and_then(|o| o.checked_sub(expected_next))
            .filter(|gap| *gap > 0);

        (messages, gap, redis_truncated)
    }

    /// M-698: Returns (items, hit_limit) where hit_limit is true if exactly LIMIT items were returned
    async fn fetch_from_redis_for_partition(
        &self,
        partition: i32,
        last_offset: i64,
        redis: &Arc<redis::aio::ConnectionManager>,
    ) -> Result<(Vec<(i64, Vec<u8>)>, bool), Box<dyn std::error::Error>> {
        use redis::AsyncCommands;

        let mut conn = redis.as_ref().clone();
        let offsets_set_key = format!("{}:partition:{}:offsets", self.redis_key_prefix, partition);

        // M-698: Use constant for per-partition limit
        let keys: Vec<String> = conn
            .zrangebyscore_limit(
                &offsets_set_key,
                (last_offset + 1) as f64,
                "+inf",
                0,
                Self::REDIS_PARTITION_PAGE_LIMIT as isize,
            )
            .await?;

        // M-698: Track if we hit the limit (may have more data)
        let hit_limit = keys.len() >= Self::REDIS_PARTITION_PAGE_LIMIT;

        if keys.is_empty() {
            return Ok((Vec::new(), hit_limit));
        }

        // M-700: Use MGET to fetch all values in a single round-trip instead of N+1 GETs.
        // This dramatically reduces latency for replays with many messages.
        let values: Vec<Option<Vec<u8>>> = conn.mget(&keys).await?;

        let mut items = Vec::new();
        let mut missing_keys = Vec::new();

        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            match value {
                Some(data) => {
                    if let Some(offset) = Self::parse_offset_from_redis_key(&key) {
                        items.push((offset, data));
                    }
                }
                None => missing_keys.push(key),
            }
        }

        // Best-effort cleanup: remove expired/missing keys from sorted set
        if !missing_keys.is_empty() {
            let _: () = conn.zrem(&offsets_set_key, missing_keys).await?;
        }

        items.sort_by_key(|(offset, _)| *offset);
        Ok((items, hit_limit))
    }

    /// M-766: Return type changed from `Vec<Vec<u8>>` to `Vec<OutboundBinaryMessage>`
    /// to include Kafka cursor metadata for cursor pairing (M-720).
    async fn get_messages_after_for_thread(
        &self,
        thread_id: &str,
        last_sequence: u64,
    ) -> (Vec<OutboundBinaryMessage>, Option<u64>) {
        // M-766: Use OutboundBinaryMessage to preserve cursor metadata
        let mut by_seq: std::collections::BTreeMap<u64, OutboundBinaryMessage> =
            std::collections::BTreeMap::new();

        if let Some(redis) = &self.redis {
            let start = Instant::now();
            match self
                .fetch_from_redis_for_thread(thread_id, last_sequence, redis)
                .await
            {
                Ok(items) if !items.is_empty() => {
                    self.redis_hits.fetch_add(1, Ordering::Relaxed);
                    if let Some(ref latency) = self.prom_operation_latency {
                        latency
                            .with_label_values(&["read"])
                            .observe(start.elapsed().as_secs_f64() * 1000.0);
                    }
                    // M-766: Items now include cursor metadata
                    for (seq, outbound) in items {
                        by_seq.insert(seq, outbound);
                    }
                }
                Ok(_) => {
                    self.redis_misses.fetch_add(1, Ordering::Relaxed);
                    if let Some(ref latency) = self.prom_operation_latency {
                        latency
                            .with_label_values(&["read"])
                            .observe(start.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "⚠️  Redis fetch failed for thread_id={} last_seq={}: {}",
                        thread_id, last_sequence, e
                    );
                    self.redis_misses.fetch_add(1, Ordering::Relaxed);
                    if let Some(ref errors) = self.prom_connection_errors {
                        errors.inc();
                    }
                }
            }
        }

        // Memory is a best-effort supplement (e.g., for messages still in-flight to Redis).
        // We merge by sequence and preserve per-thread order.
        {
            let memory = self.memory.read().await;
            let mut any = false;
            for m in memory.iter() {
                if m.thread_id.as_deref() != Some(thread_id) {
                    continue;
                }
                let Some(seq) = m.sequence else {
                    continue;
                };
                if seq <= last_sequence {
                    continue;
                }
                any = true;
                // M-766: Memory messages have partition/offset from MessageWithMetadata
                by_seq.insert(
                    seq,
                    OutboundBinaryMessage {
                        data: m.data.clone(),
                        cursor: KafkaCursor {
                            partition: m.partition,
                            offset: m.offset,
                        },
                    },
                );
            }
            if any {
                self.memory_hits.fetch_add(1, Ordering::Relaxed);
            }
        }

        let first_seq = by_seq.keys().next().copied();
        let messages: Vec<OutboundBinaryMessage> = by_seq.into_values().collect();

        let expected_next = last_sequence.saturating_add(1);
        let gap = first_seq
            .and_then(|seq| seq.checked_sub(expected_next))
            .filter(|gap| *gap > 0);

        (messages, gap)
    }

    /// M-766: Return type changed from `Vec<(u64, Vec<u8>)>` to `Vec<(u64, OutboundBinaryMessage)>`
    /// to include Kafka cursor metadata for cursor pairing (M-720).
    ///
    /// Storage format (M-766): [4 bytes partition (i32 LE)][8 bytes offset (i64 LE)][data...]
    /// Legacy format: just data bytes (no header) - treated as partition=0, offset=0
    /// M-995: Returns Bytes for zero-copy handling downstream.
    ///
    /// M-1013: Now tries both new (base64-encoded) and legacy (hash-based) keys for
    /// backward compatibility during migration. New data is written with base64 encoding,
    /// but old data written before M-1013 may use hash-based keys.
    async fn fetch_from_redis_for_thread(
        &self,
        thread_id: &str,
        last_sequence: u64,
        redis: &Arc<redis::aio::ConnectionManager>,
    ) -> Result<Vec<(u64, OutboundBinaryMessage)>, Box<dyn std::error::Error>> {
        use redis::AsyncCommands;

        let mut conn = redis.as_ref().clone();

        // M-1011: Warn if sequence exceeds MAX_SAFE_INTEGER on read path (mirrors write warning).
        // At this magnitude, f64 score math may lose precision for range queries.
        if last_sequence > MAX_SAFE_REDIS_SCORE as u64 {
            tracing::warn!(
                thread_id = %thread_id,
                last_sequence = last_sequence,
                max_safe = MAX_SAFE_REDIS_SCORE,
                "Sequence exceeds MAX_SAFE_INTEGER - Redis f64 range query may be imprecise"
            );
        }

        // M-1013: Try new base64-encoded key first, then fall back to legacy hash-based key.
        // This ensures backward compatibility with data written before M-1013.
        let safe_tid = sanitize_thread_id_for_redis(thread_id);
        let legacy_tid = legacy_hash_thread_id_for_redis(thread_id);
        let use_legacy_fallback = safe_tid != legacy_tid; // Only fallback if they differ

        // Try new key format first
        let set_key = format!("{}:thread:{}:sequences", self.redis_key_prefix, safe_tid);
        let keys: Vec<String> = conn
            .zrangebyscore_limit(&set_key, last_sequence as f64 + 1.0, "+inf", 0, 1000)
            .await?;

        // M-1013: If no results and legacy key differs, try legacy fallback
        let (keys, set_key, used_legacy) = if keys.is_empty() && use_legacy_fallback {
            let legacy_set_key =
                format!("{}:thread:{}:sequences", self.redis_key_prefix, legacy_tid);
            let legacy_keys: Vec<String> = conn
                .zrangebyscore_limit(&legacy_set_key, last_sequence as f64 + 1.0, "+inf", 0, 1000)
                .await?;

            if !legacy_keys.is_empty() {
                tracing::info!(
                    thread_id = %thread_id,
                    legacy_tid = %legacy_tid,
                    new_tid = %safe_tid,
                    count = legacy_keys.len(),
                    "M-1013: Using legacy hash-based Redis key for thread replay (pre-migration data)"
                );
            }
            (legacy_keys, legacy_set_key, true)
        } else {
            (keys, set_key, false)
        };

        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // M-700: Use MGET to fetch all values in a single round-trip instead of N+1 GETs.
        let values: Vec<Option<Vec<u8>>> = conn.mget(&keys).await?;

        let mut items = Vec::new();
        let mut missing_keys = Vec::new();

        for (key, value) in keys.into_iter().zip(values.into_iter()) {
            match value {
                Some(packed) => {
                    if let Some(seq) = Self::parse_seq_from_redis_key(&key) {
                        // M-766: Unpack partition/offset + data from stored value
                        // Format: [4 bytes partition][8 bytes offset][data...]
                        // Legacy format (< 12 bytes): treat as partition=0, offset=0, data=packed
                        // M-995: Convert to Bytes for zero-copy downstream
                        let (partition, offset, data) = if packed.len() >= 12 {
                            let partition =
                                i32::from_le_bytes([packed[0], packed[1], packed[2], packed[3]]);
                            let offset = i64::from_le_bytes([
                                packed[4], packed[5], packed[6], packed[7], packed[8], packed[9],
                                packed[10], packed[11],
                            ]);
                            // M-995: Use Bytes::from to convert slice to owned Bytes
                            (partition, offset, Bytes::copy_from_slice(&packed[12..]))
                        } else {
                            // Legacy format: no cursor metadata (treat as best-effort)
                            (0, 0, Bytes::from(packed))
                        };
                        items.push((
                            seq,
                            OutboundBinaryMessage {
                                data,
                                cursor: KafkaCursor { partition, offset },
                            },
                        ));
                    }
                }
                None => missing_keys.push(key),
            }
        }

        // Best-effort cleanup: remove expired/missing message keys from the sorted set.
        if !missing_keys.is_empty() {
            let _: () = conn.zrem(&set_key, &missing_keys).await?;
        }

        // M-1013: Log summary when legacy data was used
        if used_legacy && !items.is_empty() {
            tracing::debug!(
                thread_id = %thread_id,
                items_count = items.len(),
                "M-1013: Successfully replayed from legacy hash-based Redis keys"
            );
        }

        Ok(items)
    }

    /// M-676: Discover all known partitions from memory and Redis.
    ///
    /// This allows the server to replay partitions that the client has never seen,
    /// enabling catch-up for runs started while the UI was offline.
    pub(crate) async fn get_known_partitions(&self) -> HashSet<i32> {
        let mut partitions = HashSet::new();

        // Scan memory for known partitions
        {
            let memory = self.memory.read().await;
            for m in memory.iter() {
                partitions.insert(m.partition);
            }
        }

        // M-677: Scan Redis for partition keys using SCAN (non-blocking) instead of KEYS (O(N) blocking)
        // Pattern: {prefix}:partition:*:offsets
        if let Some(redis) = &self.redis {
            let mut conn = redis.as_ref().clone();
            let pattern = format!("{}:partition:*:offsets", self.redis_key_prefix);

            // Use SCAN iterator to avoid blocking Redis
            let mut cursor: u64 = 0;
            loop {
                let scan_result: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(&pattern)
                    .arg("COUNT")
                    .arg(100) // Batch size - process 100 keys per iteration
                    .query_async(&mut conn)
                    .await;

                match scan_result {
                    Ok((next_cursor, keys)) => {
                        for key in keys {
                            // Parse partition from key like "dashstream-replay:partition:0:offsets"
                            if let Some(partition) = Self::parse_partition_from_redis_key(&key) {
                                partitions.insert(partition);
                            }
                        }
                        cursor = next_cursor;
                        if cursor == 0 {
                            // SCAN complete when cursor returns to 0
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  Redis SCAN failed in get_known_partitions: {}", e);
                        break;
                    }
                }
            }
        }

        partitions
    }

    /// Parse partition number from Redis key like "{prefix}:partition:{N}:offsets"
    fn parse_partition_from_redis_key(key: &str) -> Option<i32> {
        let parts: Vec<&str> = key.split(':').collect();
        // Expect: [prefix, "partition", N, "offsets"]
        if parts.len() >= 4
            && parts[parts.len() - 3] == "partition"
            && parts[parts.len() - 1] == "offsets"
        {
            parts[parts.len() - 2].parse().ok()
        } else {
            None
        }
    }

    /// M-679: Get the oldest retained offset for a partition.
    /// M-703: Get the latest (highest) offset for a partition.
    ///
    /// Returns the maximum offset currently in our buffer (memory or Redis).
    /// If no data exists for the partition, returns None.
    async fn get_latest_offset_for_partition(&self, partition: i32) -> Option<i64> {
        let mut latest: Option<i64> = None;

        // Check memory
        {
            let memory = self.memory.read().await;
            for m in memory.iter() {
                if m.partition == partition {
                    latest = Some(match latest {
                        Some(o) => o.max(m.offset),
                        None => m.offset,
                    });
                }
            }
        }

        // Check Redis (get maximum score from the sorted set)
        if let Some(redis) = &self.redis {
            let mut conn = redis.as_ref().clone();
            let offsets_set_key = format!("{}:partition:{}:offsets", self.redis_key_prefix, partition);

            // ZREVRANGEBYSCORE with LIMIT 1 gives us the largest offset
            let result: Result<Vec<String>, _> = redis::cmd("ZREVRANGEBYSCORE")
                .arg(&offsets_set_key)
                .arg("+inf")
                .arg("-inf")
                .arg("LIMIT")
                .arg(0)
                .arg(1)
                .query_async(&mut conn)
                .await;

            if let Ok(keys) = result {
                if let Some(key) = keys.first() {
                    if let Some(offset) = Self::parse_offset_from_redis_key(key) {
                        latest = Some(match latest {
                            Some(o) => o.max(offset),
                            None => offset,
                        });
                    }
                }
            }
        }

        latest
    }

    /// M-703: Get latest offsets for all known partitions.
    ///
    /// Used for "from:latest" resume mode where client wants to start from
    /// the current position without replaying historical data.
    pub(crate) async fn get_latest_offsets_for_all_partitions(&self) -> HashMap<i32, i64> {
        let known_partitions = self.get_known_partitions().await;
        let mut latest_offsets = HashMap::new();

        for partition in known_partitions {
            if let Some(offset) = self.get_latest_offset_for_partition(partition).await {
                latest_offsets.insert(partition, offset);
            }
        }

        latest_offsets
    }

    ///
    /// Returns the minimum offset currently in our buffer (memory or Redis).
    /// If no data exists for the partition, returns None.
    async fn get_oldest_offset_for_partition(&self, partition: i32) -> Option<i64> {
        let mut oldest: Option<i64> = None;

        // Check memory
        {
            let memory = self.memory.read().await;
            for m in memory.iter() {
                if m.partition == partition {
                    oldest = Some(match oldest {
                        Some(o) => o.min(m.offset),
                        None => m.offset,
                    });
                }
            }
        }

        // Check Redis (get minimum score from the sorted set)
        if let Some(redis) = &self.redis {
            let mut conn = redis.as_ref().clone();
            let offsets_set_key = format!("{}:partition:{}:offsets", self.redis_key_prefix, partition);

            // ZRANGEBYSCORE with LIMIT 1 gives us the smallest offset
            let result: Result<Vec<String>, _> = redis::cmd("ZRANGEBYSCORE")
                .arg(&offsets_set_key)
                .arg("-inf")
                .arg("+inf")
                .arg("LIMIT")
                .arg(0)
                .arg(1)
                .query_async(&mut conn)
                .await;

            if let Ok(keys) = result {
                if let Some(key) = keys.first() {
                    if let Some(offset) = Self::parse_offset_from_redis_key(key) {
                        oldest = Some(match oldest {
                            Some(o) => o.min(offset),
                            None => offset,
                        });
                    }
                }
            }
        }

        oldest
    }

    /// M-679: Check for stale cursors across partitions.
    ///
    /// Returns a vec of (partition, requested_offset, oldest_retained_offset) for partitions
    /// where the client's cursor is older than our oldest retained data.
    pub(crate) async fn check_for_stale_cursors(
        &self,
        requested_offsets: &HashMap<i32, i64>,
    ) -> Vec<(i32, i64, i64)> {
        let mut stale = Vec::new();

        for (&partition, &requested_offset) in requested_offsets {
            // M-780: Only skip negative offsets (special values like -1 = "from beginning").
            // Offset 0 is a valid Kafka offset (first message) and should be checked.
            if requested_offset < 0 {
                continue;
            }

            if let Some(oldest) = self.get_oldest_offset_for_partition(partition).await {
                // If client's offset is older than our oldest, data was likely evicted
                if requested_offset < oldest {
                    stale.push((partition, requested_offset, oldest));
                }
            }
        }

        stale
    }

    pub(crate) fn snapshot_metrics(&self) -> ReplayBufferMetricsSnapshot {
        ReplayBufferMetricsSnapshot {
            redis_enabled: self.redis.is_some(),
            memory_hits: self.memory_hits.load(Ordering::Relaxed),
            redis_hits: self.redis_hits.load(Ordering::Relaxed),
            redis_misses: self.redis_misses.load(Ordering::Relaxed),
            redis_write_dropped: self.redis_write_dropped.load(Ordering::Relaxed),
            redis_write_failures: self.redis_write_failures.load(Ordering::Relaxed),
            // M-1022: Include operational metrics
            memory_buffer_size: self.memory_buffer_size.load(Ordering::Relaxed),
            max_memory_size: self.max_memory_size,
            redis_message_ttl_secs: self.redis_message_ttl_secs,
            redis_max_sequences: self.redis_max_sequences,
        }
    }

    /// M-746: Clear all replay buffer data (memory + Redis).
    ///
    /// Called during cursor_reset to prevent stale history from reappearing after
    /// the client explicitly resets their cursor. Without this, old messages
    /// stored in Redis could be replayed on the next resume, causing state
    /// inconsistencies.
    ///
    /// Returns the number of Redis keys deleted (0 if Redis not enabled).
    ///
    /// # M-768: Synchronization with in-flight writes
    ///
    /// This function drains all pending Redis write tasks BEFORE clearing keys.
    /// Without this synchronization, in-flight writes could complete AFTER clear()
    /// and repopulate the buffer, causing stale data to reappear.
    pub(crate) async fn clear(&self) -> usize {
        // M-768: Drain pending Redis writes before clearing to prevent repopulation race.
        // Use a short timeout since cursor_reset should be quick; in-flight writes are
        // typically fast. If timeout, warn but continue (better to clear with small
        // repopulation risk than block indefinitely).
        self.drain_pending_redis_writes(std::time::Duration::from_millis(500))
            .await;

        // Clear memory buffer
        {
            let mut mem = self.memory.write().await;
            mem.clear();
            // M-1022: Reset memory buffer size metric
            self.memory_buffer_size.store(0, Ordering::Relaxed);
        }

        // M-769: Clear Redis using SCAN + UNLINK pattern with timeout.
        // UNLINK is async (returns immediately, deletion happens in background).
        // Timeout prevents blocking on slow Redis or large key sets.
        let Some(redis) = &self.redis else {
            return 0;
        };

        let mut conn = redis.as_ref().clone();
        let pattern = format!("{}:*", self.redis_key_prefix);
        let mut deleted_count: usize = 0;

        // M-769: Overall timeout for SCAN+UNLINK loop (5 seconds default)
        // This prevents cursor_reset from blocking indefinitely on slow Redis.
        // M-153: Use constant from config module
        let clear_timeout = std::time::Duration::from_secs(
            std::env::var(super::config::REDIS_CLEAR_TIMEOUT_SECS)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
        );
        let start = Instant::now();

        // Use SCAN to find all keys, then UNLINK in batches (async deletion)
        let mut cursor: u64 = 0;
        loop {
            // M-769: Check timeout before each SCAN iteration
            if start.elapsed() >= clear_timeout {
                eprintln!(
                    "⚠️  Redis clear() timed out after {:?} (deleted {} keys so far)",
                    clear_timeout, deleted_count
                );
                break;
            }

            let scan_result: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await;

            match scan_result {
                Ok((next_cursor, keys)) => {
                    if !keys.is_empty() {
                        // M-769: Use UNLINK instead of DEL for async deletion.
                        // UNLINK returns immediately; actual memory reclaim happens
                        // asynchronously in Redis background thread. This reduces
                        // latency for cursor_reset operations.
                        let unlink_result: Result<usize, _> = redis::cmd("UNLINK")
                            .arg(&keys)
                            .query_async(&mut conn)
                            .await;

                        if let Ok(count) = unlink_result {
                            deleted_count += count;
                        }
                    }
                    cursor = next_cursor;
                    if cursor == 0 {
                        // SCAN complete when cursor returns to 0
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Redis SCAN failed in clear(): {}", e);
                    break;
                }
            }
        }

        deleted_count
    }

    pub(crate) async fn drain_pending_redis_writes(&self, timeout: tokio::time::Duration) {
        // M-767: Use configurable max concurrent writes
        match tokio::time::timeout(
            timeout,
            self.redis_write_semaphore
                .acquire_many(self.redis_max_concurrent_writes as u32),
        )
        .await
        {
            Ok(Ok(permit)) => drop(permit),
            Ok(Err(e)) => eprintln!("⚠️  ReplayBuffer Redis drain failed: {}", e),
            Err(_) => eprintln!(
                "⚠️  Timed out waiting for in-flight ReplayBuffer Redis writes (>{:?}); continuing shutdown",
                timeout
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReplayBufferMetricsSnapshot {
    pub(crate) redis_enabled: bool,
    pub(crate) memory_hits: u64,
    pub(crate) redis_hits: u64,
    pub(crate) redis_misses: u64,
    pub(crate) redis_write_dropped: u64,
    pub(crate) redis_write_failures: u64,
    // M-1022: Operational metrics for predicting cursor_stale/eviction
    pub(crate) memory_buffer_size: u64,      // Current messages in memory buffer
    pub(crate) max_memory_size: usize,       // Configured max memory buffer capacity
    pub(crate) redis_message_ttl_secs: u64,  // Configured Redis TTL (retention)
    pub(crate) redis_max_sequences: usize,   // Max sequences retained per thread in Redis
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replay_gap_is_reported_from_memory() {
        let buffer = ReplayBuffer::new_memory_only(10);
        buffer
            .add_message(Bytes::from_static(&[0x01]), Some("t1".to_string()), Some(9), 0, 9, None)
            .await;
        buffer
            .add_message(Bytes::from_static(&[0x02]), Some("t1".to_string()), Some(13), 0, 13, None)
            .await;
        buffer
            .add_message(Bytes::from_static(&[0x03]), Some("t1".to_string()), Some(14), 0, 14, None)
            .await;

        let (messages, gap) = buffer.get_messages_after_for_thread("t1", 10).await;
        let data: Vec<&[u8]> = messages.iter().map(|m| m.data.as_ref()).collect();
        assert_eq!(data, vec![&[0x02][..], &[0x03][..]]);
        assert_eq!(gap, Some(2)); // missing 11,12
    }

    #[tokio::test]
    async fn replay_contiguous_has_no_gap() {
        let buffer = ReplayBuffer::new_memory_only(10);
        buffer
            .add_message(Bytes::from_static(&[0x01]), Some("t1".to_string()), Some(11), 0, 11, None)
            .await;

        let (messages, gap) = buffer.get_messages_after_for_thread("t1", 10).await;
        let data: Vec<&[u8]> = messages.iter().map(|m| m.data.as_ref()).collect();
        assert_eq!(data, vec![&[0x01][..]]);
        assert_eq!(gap, None);
    }

    #[tokio::test]
    async fn replay_up_to_date_returns_empty_no_gap() {
        let buffer = ReplayBuffer::new_memory_only(10);
        buffer
            .add_message(Bytes::from_static(&[0x01]), Some("t1".to_string()), Some(10), 0, 10, None)
            .await;

        let (messages, gap) = buffer.get_messages_after_for_thread("t1", 10).await;
        assert!(messages.is_empty());
        assert_eq!(gap, None);
    }

    #[tokio::test]
    async fn legacy_replay_returns_arrival_order_after_first_matching_sequence() {
        let buffer = ReplayBuffer::new_memory_only(10);
        buffer
            .add_message(Bytes::from_static(&[0x01]), Some("t1".to_string()), Some(9), 0, 9, None)
            .await;
        buffer
            .add_message(Bytes::from_static(&[0x02]), Some("t2".to_string()), Some(1), 0, 10, None)
            .await;
        buffer
            .add_message(Bytes::from_static(&[0x03]), Some("t1".to_string()), Some(10), 0, 11, None)
            .await;
        buffer
            .add_message(Bytes::from_static(&[0x04]), Some("t2".to_string()), Some(2), 0, 12, None)
            .await;

        let (messages, first_seq) = buffer.get_messages_after_legacy(9).await;
        assert_eq!(first_seq, Some(10));
        let data: Vec<&[u8]> = messages.iter().map(|m| m.as_ref()).collect();
        assert_eq!(data, vec![&[0x03][..], &[0x04][..]]);
    }

    #[tokio::test]
    async fn partition_replay_returns_offsets_after_cursor_and_reports_gap() {
        let buffer = ReplayBuffer::new_memory_only(10);
        buffer
            .add_message(Bytes::from_static(&[0x01]), Some("t1".to_string()), Some(9), 0, 9, None)
            .await;
        buffer
            .add_message(Bytes::from_static(&[0x02]), Some("t1".to_string()), Some(13), 0, 13, None)
            .await;
        buffer
            .add_message(Bytes::from_static(&[0x03]), Some("t1".to_string()), Some(14), 0, 14, None)
            .await;

        let mut offsets = HashMap::new();
        offsets.insert(0, 10);

        let (messages, gaps, new_partitions, truncated_partitions) =
            buffer.get_messages_after_by_partition(&offsets).await;
        assert_eq!(gaps, vec![(0, 2)]); // missing 11,12
        assert!(new_partitions.is_empty()); // partition 0 was already in offsets
        assert!(truncated_partitions.is_empty());
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].cursor.partition, 0);
        assert_eq!(messages[0].cursor.offset, 13);
        assert_eq!(messages[0].data.as_ref(), &[0x02]);
        assert_eq!(messages[1].cursor.offset, 14);
        assert_eq!(messages[1].data.as_ref(), &[0x03]);
    }

    #[tokio::test]
    async fn partition_replay_discovers_unknown_partitions() {
        let buffer = ReplayBuffer::new_memory_only(10);
        // Add messages to partition 0
        buffer
            .add_message(Bytes::from_static(&[0x01]), Some("t1".to_string()), Some(1), 0, 100, None)
            .await;
        // Add messages to partition 1 (client hasn't seen this)
        buffer
            .add_message(Bytes::from_static(&[0x02]), Some("t2".to_string()), Some(1), 1, 200, None)
            .await;

        // Client only knows about partition 0
        let mut offsets = HashMap::new();
        offsets.insert(0, 99);

        let (messages, _gaps, new_partitions, truncated_partitions) =
            buffer.get_messages_after_by_partition(&offsets).await;

        // M-676: Server should discover partition 1 and include it
        assert!(new_partitions.contains(&1));
        assert!(truncated_partitions.is_empty());
        assert_eq!(messages.len(), 2); // one from each partition
    }

    #[test]
    fn redis_trim_stop_is_correct() {
        // M-767: Test with configurable max_sequences parameter
        let max = ReplayBuffer::REDIS_MAX_SEQUENCES;
        assert_eq!(ReplayBuffer::redis_trim_stop(max, max), None);
        assert_eq!(ReplayBuffer::redis_trim_stop(max + 1, max), Some(0));
        assert_eq!(ReplayBuffer::redis_trim_stop(max + 2, max), Some(1));
        assert_eq!(ReplayBuffer::redis_trim_stop(max + 123, max), Some(122));

        // Test with custom max_sequences
        let custom_max = 500;
        assert_eq!(ReplayBuffer::redis_trim_stop(custom_max, custom_max), None);
        assert_eq!(ReplayBuffer::redis_trim_stop(custom_max + 1, custom_max), Some(0));
        assert_eq!(ReplayBuffer::redis_trim_stop(custom_max + 50, custom_max), Some(49));
    }

    #[test]
    fn parse_seq_from_redis_key_extracts_suffix() {
        assert_eq!(
            ReplayBuffer::parse_seq_from_redis_key("dashstream-replay:thread:t1:seq:42"),
            Some(42)
        );
        assert_eq!(
            ReplayBuffer::parse_seq_from_redis_key("dashstream-replay:thread:t1:seq:not-a-number"),
            None
        );
        assert_eq!(ReplayBuffer::parse_seq_from_redis_key(""), None);
    }

    // M-1013: Tests for thread_id sanitization
    #[test]
    fn sanitize_thread_id_safe_ids_unchanged() {
        // Simple alphanumeric IDs should be unchanged
        assert_eq!(sanitize_thread_id_for_redis("thread1"), "thread1");
        assert_eq!(sanitize_thread_id_for_redis("abc123"), "abc123");
        assert_eq!(sanitize_thread_id_for_redis("my_thread"), "my_thread");
        assert_eq!(sanitize_thread_id_for_redis("my-thread"), "my-thread");
        assert_eq!(
            sanitize_thread_id_for_redis("Thread_123-abc"),
            "Thread_123-abc"
        );
    }

    #[test]
    fn sanitize_thread_id_encodes_colons() {
        // IDs with colons must be encoded to avoid Redis key segment confusion
        let encoded = sanitize_thread_id_for_redis("thread:with:colons");
        assert!(
            encoded.starts_with("b64_"),
            "Expected b64_ prefix, got: {}",
            encoded
        );
        assert!(
            !encoded.contains(':'),
            "Encoded ID should not contain colons"
        );

        // Verify different inputs produce different outputs (collision-free)
        let encoded2 = sanitize_thread_id_for_redis("thread:other:colons");
        assert_ne!(
            encoded, encoded2,
            "Different inputs should produce different outputs"
        );
    }

    #[test]
    fn sanitize_thread_id_encodes_long_ids() {
        // IDs longer than MAX_THREAD_ID_LENGTH must be encoded
        let long_id = "a".repeat(MAX_THREAD_ID_LENGTH + 1);
        let encoded = sanitize_thread_id_for_redis(&long_id);
        assert!(
            encoded.starts_with("b64_"),
            "Expected b64_ prefix for long ID"
        );

        // IDs exactly at the limit should be unchanged (if otherwise safe)
        let at_limit = "a".repeat(MAX_THREAD_ID_LENGTH);
        assert_eq!(sanitize_thread_id_for_redis(&at_limit), at_limit);
    }

    #[test]
    fn sanitize_thread_id_encodes_special_chars() {
        // IDs with special characters should be encoded
        let special = "thread with spaces";
        let encoded = sanitize_thread_id_for_redis(special);
        assert!(
            encoded.starts_with("b64_"),
            "Expected b64_ prefix for special chars"
        );

        // Other special characters
        assert!(sanitize_thread_id_for_redis("thread/path").starts_with("b64_"));
        assert!(sanitize_thread_id_for_redis("thread.name").starts_with("b64_"));
        assert!(sanitize_thread_id_for_redis("thread@domain").starts_with("b64_"));
    }

    #[test]
    fn sanitize_thread_id_is_collision_free() {
        // Verify that encoding is collision-free for similar inputs
        let inputs = vec![
            "thread1",
            "thread:1",
            "thread_1",
            "THREAD1",
            "thread:1:2",
            "thread:1:3",
        ];

        let mut seen = std::collections::HashSet::new();
        for input in &inputs {
            let encoded = sanitize_thread_id_for_redis(input);
            assert!(
                seen.insert(encoded.clone()),
                "Collision detected for input '{}': {}",
                input,
                encoded
            );
        }
    }

    #[test]
    fn legacy_hash_matches_pre_m1013_behavior() {
        // Legacy hash should use DefaultHasher for backward compatibility
        // Safe IDs unchanged
        assert_eq!(legacy_hash_thread_id_for_redis("thread1"), "thread1");

        // IDs with colons get hashed (not encoded)
        let hashed = legacy_hash_thread_id_for_redis("thread:with:colons");
        assert!(
            hashed.starts_with("h_"),
            "Expected h_ prefix for legacy hash"
        );
        assert_eq!(hashed.len(), 2 + 16, "Expected 'h_' + 16 hex chars");

        // Long IDs get hashed
        let long_id = "a".repeat(MAX_THREAD_ID_LENGTH + 1);
        let hashed_long = legacy_hash_thread_id_for_redis(&long_id);
        assert!(
            hashed_long.starts_with("h_"),
            "Expected h_ prefix for long ID hash"
        );
    }

    #[test]
    fn new_and_legacy_keys_differ_when_encoding_needed() {
        // When encoding is needed, new and legacy keys should differ
        let thread_id = "thread:with:colons";
        let new_key = sanitize_thread_id_for_redis(thread_id);
        let legacy_key = legacy_hash_thread_id_for_redis(thread_id);

        assert_ne!(
            new_key, legacy_key,
            "New and legacy keys should differ for encoded IDs"
        );
        assert!(new_key.starts_with("b64_"));
        assert!(legacy_key.starts_with("h_"));

        // Safe IDs should be identical
        let safe_id = "simple_thread";
        assert_eq!(
            sanitize_thread_id_for_redis(safe_id),
            legacy_hash_thread_id_for_redis(safe_id)
        );
    }
}
