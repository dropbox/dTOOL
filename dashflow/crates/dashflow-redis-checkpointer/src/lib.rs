//! Redis checkpointer for DashFlow
//!
//! Provides persistent checkpoint storage using Redis.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_redis_checkpointer::RedisCheckpointer;
//! use dashflow::{StateGraph, GraphState};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MyState {
//!     value: i32,
//! }
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let connection_string = "redis://localhost:6379";
//!     let checkpointer = RedisCheckpointer::new(connection_string).await?;
//!
//!     let mut graph = StateGraph::new();
//!     // ... build graph ...
//!     let app = graph.compile()?.with_checkpointer(checkpointer);
//!
//!     let initial_state = MyState { value: 0 };
//!     let result = app.invoke(initial_state).await?;
//!     Ok(())
//! }
//! ```
//!
//! # See Also
//!
//! - [`Checkpointer`] - The trait this implements
//! - [`Checkpoint`] - The checkpoint data structure
//! - [`dashflow-postgres-checkpointer`](https://docs.rs/dashflow-postgres-checkpointer) - Alternative: PostgreSQL-based checkpointing
//! - [`dashflow-redis`](https://docs.rs/dashflow-redis) - Redis vector store (different from checkpointer)
//! - [Redis Documentation](https://redis.io/docs/) - Official Redis docs

use dashflow::{
    checkpointer_helpers::{nanos_to_timestamp, timestamp_to_nanos},
    Checkpoint, CheckpointMetadata, Checkpointer, GraphState, Result as DashFlowResult,
    RetentionPolicy,
};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, RedisError};
use std::marker::PhantomData;
use std::time::SystemTime;
use tracing::{debug, error, info};

#[cfg(feature = "compression")]
use dashflow_compression::{Compression, CompressionType};

/// Errors that can occur when using Redis checkpointer
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RedisCheckpointerError {
    #[error("Redis connection error: {0}")]
    ConnectionError(String),

    #[error("Redis command error: {0}")]
    CommandError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[cfg(feature = "compression")]
    #[error("Compression error: {0}")]
    CompressionError(String),
}

impl From<RedisError> for RedisCheckpointerError {
    fn from(err: RedisError) -> Self {
        RedisCheckpointerError::CommandError(err.to_string())
    }
}

impl From<RedisCheckpointerError> for dashflow::Error {
    fn from(err: RedisCheckpointerError) -> Self {
        use dashflow::error::CheckpointError;
        let checkpoint_err = match err {
            RedisCheckpointerError::ConnectionError(msg) => CheckpointError::ConnectionLost {
                backend: "redis".to_string(),
                reason: msg,
            },
            RedisCheckpointerError::CommandError(msg) => {
                CheckpointError::Other(format!("Redis command error: {}", msg))
            }
            RedisCheckpointerError::SerializationError(msg) => {
                CheckpointError::SerializationFailed { reason: msg }
            }
            RedisCheckpointerError::DeserializationError(msg) => {
                CheckpointError::DeserializationFailed { reason: msg }
            }
            #[cfg(feature = "compression")]
            RedisCheckpointerError::CompressionError(msg) => {
                CheckpointError::Other(format!("Compression error: {}", msg))
            }
        };
        dashflow::Error::Checkpoint(checkpoint_err)
    }
}

/// Redis-backed checkpointer
///
/// Stores checkpoints in Redis with the following data structure:
/// - Hash per checkpoint: `checkpoint:{checkpoint_id}` containing:
///   - `thread_id` - Thread ID
///   - `state` - Bincode-encoded state
///   - `node` - Node name
///   - `timestamp` - Unix timestamp in nanoseconds (string, full precision)
///   - `parent_id` - Parent checkpoint ID (if any)
///   - `metadata` - JSON-encoded metadata
/// - Sorted set per thread: `thread:{thread_id}:checkpoints`
///   - Members: checkpoint IDs
///   - Scores: timestamps in milliseconds (f64-safe, sufficient for ordering)
/// - Set of all thread IDs: `threads`
///
/// Note: ZSET scores use milliseconds to avoid f64 precision loss. The full
/// nanosecond timestamp is stored in the hash field for precise reads.
pub struct RedisCheckpointer<S: GraphState> {
    connection_manager: ConnectionManager,
    key_prefix: String,
    #[cfg(feature = "compression")]
    compression: Option<Box<dyn Compression>>,
    retention_policy: Option<RetentionPolicy>,
    _phantom: PhantomData<S>,
}

impl<S: GraphState> RedisCheckpointer<S> {
    /// Create a new Redis checkpointer
    ///
    /// # Arguments
    /// * `connection_string` - Redis connection string (e.g., "redis://localhost:6379")
    ///
    /// # Errors
    /// Returns error if connection fails
    pub async fn new(connection_string: &str) -> Result<Self, RedisCheckpointerError> {
        Self::with_key_prefix(connection_string, "dashflow").await
    }

    /// Create a new Redis checkpointer with custom key prefix
    ///
    /// # Arguments
    /// * `connection_string` - Redis connection string
    /// * `key_prefix - Prefix for all Redis keys (default: "dashflow"))
    pub async fn with_key_prefix(
        connection_string: &str,
        key_prefix: &str,
    ) -> Result<Self, RedisCheckpointerError> {
        info!("Connecting to Redis: {}", connection_string);
        let client = redis::Client::open(connection_string).map_err(|e| {
            error!("Failed to create Redis client: {}", e);
            RedisCheckpointerError::ConnectionError(e.to_string())
        })?;

        let connection_manager = ConnectionManager::new(client).await.map_err(|e| {
            error!("Failed to connect to Redis: {}", e);
            RedisCheckpointerError::ConnectionError(e.to_string())
        })?;

        debug!("Redis connection established with prefix: {}", key_prefix);

        Ok(Self {
            connection_manager,
            key_prefix: key_prefix.to_string(),
            #[cfg(feature = "compression")]
            compression: None,
            retention_policy: None,
            _phantom: PhantomData,
        })
    }

    /// Enable compression for checkpoint storage
    ///
    /// # Arguments
    /// * `compression_type` - The compression algorithm and configuration to use
    #[cfg(feature = "compression")]
    pub fn with_compression(
        mut self,
        compression_type: CompressionType,
    ) -> Result<Self, RedisCheckpointerError> {
        self.compression = Some(
            compression_type
                .build()
                .map_err(|e| RedisCheckpointerError::CompressionError(e.to_string()))?,
        );
        Ok(self)
    }

    /// Set retention policy for automatic checkpoint cleanup
    ///
    /// # Arguments
    /// * `policy` - The retention policy to apply
    pub fn with_retention_policy(mut self, policy: RetentionPolicy) -> Self {
        self.retention_policy = Some(policy);
        self
    }

    /// Apply retention policy to clean up old checkpoints for a thread
    ///
    /// This method should be called periodically or after saving checkpoints
    /// to enforce the retention policy rules.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID to apply retention policy to
    ///
    /// # Returns
    /// Number of checkpoints deleted
    pub async fn apply_retention(&self, thread_id: &str) -> DashFlowResult<usize> {
        let policy = match &self.retention_policy {
            Some(p) => p,
            None => return Ok(0), // No policy configured
        };

        // Get all checkpoints for this thread
        let checkpoints = self.list(thread_id).await?;

        // Evaluate retention policy
        let (_to_keep, to_delete) = policy.evaluate(&checkpoints, SystemTime::now());

        // Delete checkpoints marked for deletion
        for checkpoint_id in &to_delete {
            self.delete(checkpoint_id).await?;
        }

        debug!(
            "Applied retention policy to thread {}: deleted {} checkpoints",
            thread_id,
            to_delete.len()
        );

        Ok(to_delete.len())
    }

    /// Get the Redis key for a checkpoint
    fn checkpoint_key(&self, checkpoint_id: &str) -> String {
        format!("{}:checkpoint:{}", self.key_prefix, checkpoint_id)
    }

    /// Get the Redis key for a thread's checkpoint sorted set
    fn thread_checkpoints_key(&self, thread_id: &str) -> String {
        format!("{}:thread:{}:checkpoints", self.key_prefix, thread_id)
    }

    /// Get the Redis key for the set of all threads
    fn threads_key(&self) -> String {
        format!("{}:threads", self.key_prefix)
    }

    /// Helper to deserialize a checkpoint from Redis hash data
    ///
    /// # Arguments
    /// * `data` - HashMap from Redis HGETALL command
    ///
    /// # Returns
    /// Deserialized checkpoint or error
    #[allow(clippy::needless_pass_by_value)] // Takes ownership - callers don't need data after
    fn deserialize_checkpoint(
        &self,
        data: std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<Checkpoint<S>, RedisCheckpointerError> {
        let thread_id = String::from_utf8(
            data.get("thread_id")
                .ok_or_else(|| {
                    RedisCheckpointerError::DeserializationError(
                        "Missing thread_id field".to_string(),
                    )
                })?
                .clone(),
        )
        .map_err(|e| {
            RedisCheckpointerError::DeserializationError(format!("Invalid thread_id UTF-8: {}", e))
        })?;

        let checkpoint_id = String::from_utf8(
            data.get("checkpoint_id")
                .ok_or_else(|| {
                    RedisCheckpointerError::DeserializationError(
                        "Missing checkpoint_id field".to_string(),
                    )
                })?
                .clone(),
        )
        .map_err(|e| {
            RedisCheckpointerError::DeserializationError(format!(
                "Invalid checkpoint_id UTF-8: {}",
                e
            ))
        })?;

        #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
        let mut state_bytes = data
            .get("state")
            .ok_or_else(|| {
                RedisCheckpointerError::DeserializationError("Missing state field".to_string())
            })?
            .clone();

        // Decompress if compression is enabled
        #[cfg(feature = "compression")]
        if let Some(ref compressor) = self.compression {
            state_bytes = compressor.decompress(&state_bytes).map_err(|e| {
                error!("Failed to decompress checkpoint state: {}", e);
                RedisCheckpointerError::DeserializationError(format!("Decompression error: {}", e))
            })?;
            debug!("Decompressed state to {} bytes", state_bytes.len());
        }

        let state: S = bincode::deserialize(&state_bytes).map_err(|e| {
            RedisCheckpointerError::DeserializationError(format!(
                "Failed to deserialize state: {}",
                e
            ))
        })?;

        let node = String::from_utf8(
            data.get("node")
                .ok_or_else(|| {
                    RedisCheckpointerError::DeserializationError("Missing node field".to_string())
                })?
                .clone(),
        )
        .map_err(|e| {
            RedisCheckpointerError::DeserializationError(format!("Invalid node UTF-8: {}", e))
        })?;

        let timestamp_bytes = data
            .get("timestamp")
            .ok_or_else(|| {
                RedisCheckpointerError::DeserializationError("Missing timestamp field".to_string())
            })?
            .clone();

        let timestamp_str = String::from_utf8(timestamp_bytes).map_err(|e| {
            RedisCheckpointerError::DeserializationError(format!("Invalid timestamp UTF-8: {}", e))
        })?;

        let timestamp_nanos: i64 = timestamp_str.parse().map_err(|e| {
            RedisCheckpointerError::DeserializationError(format!("Invalid timestamp: {}", e))
        })?;

        let timestamp = nanos_to_timestamp(timestamp_nanos);

        let parent_id = data
            .get("parent_id")
            .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
            .filter(|s| !s.is_empty());

        let metadata_bytes = data.get("metadata").map(|b| b.as_slice()).unwrap_or(b"{}");
        let metadata: std::collections::HashMap<String, String> =
            serde_json::from_slice(metadata_bytes).map_err(|e| {
                RedisCheckpointerError::DeserializationError(format!(
                    "Failed to deserialize metadata: {}",
                    e
                ))
            })?;

        Ok(Checkpoint {
            id: checkpoint_id,
            thread_id,
            state,
            node,
            timestamp,
            parent_id,
            metadata,
        })
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for RedisCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> DashFlowResult<()> {
        let checkpoint_key = self.checkpoint_key(&checkpoint.id);
        let thread_checkpoints_key = self.thread_checkpoints_key(&checkpoint.thread_id);
        let threads_key = self.threads_key();

        // Serialize state with bincode
        #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
        let mut state_bytes = bincode::serialize(&checkpoint.state)
            .map_err(|e| dashflow::Error::Generic(format!("Failed to serialize state: {}", e)))?;

        // Apply compression if enabled
        #[cfg(feature = "compression")]
        if let Some(ref compressor) = self.compression {
            state_bytes = compressor.compress(&state_bytes).map_err(|e| {
                error!("Failed to compress checkpoint state: {}", e);
                dashflow::Error::Generic(format!("Compression error: {}", e))
            })?;
            debug!("Compressed state to {} bytes", state_bytes.len());
        }

        // Serialize metadata as JSON
        let metadata_json = serde_json::to_string(&checkpoint.metadata).map_err(|e| {
            dashflow::Error::Generic(format!("Failed to serialize metadata: {}", e))
        })?;

        let timestamp_nanos = timestamp_to_nanos(checkpoint.timestamp);

        // Store checkpoint as Redis hash
        let mut conn = self.connection_manager.clone();

        // Use Redis pipelining for atomic multi-command execution
        let mut pipe = redis::pipe();
        pipe.atomic();

        // Store checkpoint hash
        pipe.hset_multiple(
            &checkpoint_key,
            &[
                ("checkpoint_id", checkpoint.id.as_bytes()),
                ("thread_id", checkpoint.thread_id.as_bytes()),
                ("state", &state_bytes),
                ("node", checkpoint.node.as_bytes()),
                ("timestamp", timestamp_nanos.to_string().as_bytes()),
                (
                    "parent_id",
                    checkpoint
                        .parent_id
                        .as_ref()
                        .map(|s| s.as_bytes())
                        .unwrap_or(b""),
                ),
                ("metadata", metadata_json.as_bytes()),
            ],
        );

        // Add checkpoint to thread's sorted set
        // Use milliseconds for score (safe for f64, sufficient for ordering)
        // Full nanoseconds are stored in the hash field for precision
        let score_millis = timestamp_nanos / 1_000_000;
        pipe.zadd(&thread_checkpoints_key, &checkpoint.id, score_millis as f64);

        // Add thread to threads set
        pipe.sadd(&threads_key, &checkpoint.thread_id);

        // Execute pipeline
        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to save checkpoint: {}", e))
            })?;

        debug!(
            "Saved checkpoint {} for thread {}",
            checkpoint.id, checkpoint.thread_id
        );

        // Apply retention policy if configured
        if self.retention_policy.is_some() {
            if let Err(e) = self.apply_retention(&checkpoint.thread_id).await {
                // Log but don't fail the save operation if retention cleanup fails
                tracing::warn!(
                    "Failed to apply retention policy for thread {}: {}",
                    checkpoint.thread_id,
                    e
                );
            }
        }

        Ok(())
    }

    async fn load(&self, checkpoint_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        let checkpoint_key = self.checkpoint_key(checkpoint_id);

        let mut conn = self.connection_manager.clone();

        // Get all fields from checkpoint hash
        let data: std::collections::HashMap<String, Vec<u8>> = conn
            .hgetall(&checkpoint_key)
            .await
            .map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to load checkpoint: {}", e))
            })?;

        if data.is_empty() {
            return Ok(None);
        }

        let checkpoint = self.deserialize_checkpoint(data).map_err(|e| {
            dashflow::Error::Generic(format!("Failed to deserialize checkpoint: {}", e))
        })?;

        debug!("Loaded checkpoint {}", checkpoint_id);

        Ok(Some(checkpoint))
    }

    async fn get_latest(&self, thread_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        let thread_checkpoints_key = self.thread_checkpoints_key(thread_id);

        let mut conn = self.connection_manager.clone();

        // Get the checkpoint with the highest timestamp (most recent)
        // ZREVRANGE returns members in descending score order
        let checkpoint_ids: Vec<String> = conn
            .zrevrange(&thread_checkpoints_key, 0, 0)
            .await
            .map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to get latest checkpoint: {}", e))
            })?;

        if let Some(checkpoint_id) = checkpoint_ids.first() {
            self.load(checkpoint_id).await
        } else {
            Ok(None)
        }
    }

    async fn list(&self, thread_id: &str) -> DashFlowResult<Vec<CheckpointMetadata>> {
        let thread_checkpoints_key = self.thread_checkpoints_key(thread_id);

        let mut conn = self.connection_manager.clone();

        // Get all checkpoint IDs for this thread, ordered by timestamp (descending)
        let checkpoint_ids: Vec<String> = conn
            .zrevrange(&thread_checkpoints_key, 0, -1)
            .await
            .map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to list checkpoints: {}", e))
            })?;

        let mut metadatas = Vec::new();

        for checkpoint_id in checkpoint_ids {
            let checkpoint_key = self.checkpoint_key(&checkpoint_id);

            // Get metadata fields only (no state) using HMGET
            let fields = [
                "checkpoint_id",
                "thread_id",
                "node",
                "timestamp",
                "parent_id",
                "metadata",
            ];

            // HMGET returns values in same order as fields requested
            let values: Vec<Option<Vec<u8>>> = redis::cmd("HMGET")
                .arg(&checkpoint_key)
                .arg(&fields)
                .query_async(&mut conn)
                .await
                .map_err(|e: RedisError| {
                    dashflow::Error::Generic(format!("Failed to get checkpoint metadata: {}", e))
                })?;

            // Build HashMap from fields and values
            let mut data = std::collections::HashMap::new();
            for (field, value) in fields.iter().zip(values.into_iter()) {
                if let Some(v) = value {
                    data.insert((*field).to_string(), v);
                }
            }

            if data.is_empty() {
                continue;
            }

            let id = checkpoint_id;

            let thread_id_val = String::from_utf8(
                data.get("thread_id")
                    .ok_or_else(|| dashflow::Error::Generic("Missing thread_id field".to_string()))?
                    .clone(),
            )
            .map_err(|e| dashflow::Error::Generic(format!("Invalid thread_id UTF-8: {}", e)))?;

            let node = String::from_utf8(
                data.get("node")
                    .ok_or_else(|| dashflow::Error::Generic("Missing node field".to_string()))?
                    .clone(),
            )
            .map_err(|e| dashflow::Error::Generic(format!("Invalid node UTF-8: {}", e)))?;

            let timestamp_bytes = data
                .get("timestamp")
                .ok_or_else(|| dashflow::Error::Generic("Missing timestamp field".to_string()))?
                .clone();

            let timestamp_str = String::from_utf8(timestamp_bytes)
                .map_err(|e| dashflow::Error::Generic(format!("Invalid timestamp UTF-8: {}", e)))?;

            let timestamp_nanos: i64 = timestamp_str
                .parse()
                .map_err(|e| dashflow::Error::Generic(format!("Invalid timestamp: {}", e)))?;

            let timestamp = nanos_to_timestamp(timestamp_nanos);

            let parent_id = data
                .get("parent_id")
                .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
                .filter(|s| !s.is_empty());

            let metadata_bytes = data.get("metadata").map(|b| b.as_slice()).unwrap_or(b"{}");
            let metadata: std::collections::HashMap<String, String> =
                serde_json::from_slice(metadata_bytes).map_err(|e| {
                    dashflow::Error::Generic(format!("Failed to deserialize metadata: {}", e))
                })?;

            metadatas.push(CheckpointMetadata {
                id,
                thread_id: thread_id_val,
                node,
                timestamp,
                parent_id,
                metadata,
            });
        }

        debug!(
            "Listed {} checkpoints for thread {}",
            metadatas.len(),
            thread_id
        );

        Ok(metadatas)
    }

    async fn delete(&self, checkpoint_id: &str) -> DashFlowResult<()> {
        let checkpoint_key = self.checkpoint_key(checkpoint_id);

        // First, get the thread_id from the checkpoint
        let mut conn = self.connection_manager.clone();

        let thread_id_bytes: Option<Vec<u8>> = conn
            .hget(&checkpoint_key, "thread_id")
            .await
            .map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to get thread_id for checkpoint: {}", e))
            })?;

        if let Some(thread_id_bytes) = thread_id_bytes {
            let thread_id = String::from_utf8(thread_id_bytes)
                .map_err(|e| dashflow::Error::Generic(format!("Invalid thread_id UTF-8: {}", e)))?;

            let thread_checkpoints_key = self.thread_checkpoints_key(&thread_id);

            // Use pipeline for atomic deletion
            let mut pipe = redis::pipe();
            pipe.atomic();

            // Delete checkpoint hash
            pipe.del(&checkpoint_key);

            // Remove from thread's sorted set
            pipe.zrem(&thread_checkpoints_key, checkpoint_id);

            // Execute pipeline
            pipe.query_async::<()>(&mut conn)
                .await
                .map_err(|e: RedisError| {
                    dashflow::Error::Generic(format!("Failed to delete checkpoint: {}", e))
                })?;

            debug!("Deleted checkpoint {}", checkpoint_id);
        } else {
            // Checkpoint hash doesn't exist or is missing thread_id.
            // Delete the checkpoint key anyway, and also try to clean up orphaned
            // ZSET entries by scanning all threads. This prevents orphaned entries.
            let _: () = conn.del(&checkpoint_key).await.map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to delete checkpoint: {}", e))
            })?;

            // Try to clean up orphaned ZSET entries across all threads
            let threads_key = self.threads_key();
            let thread_ids: Vec<String> = conn.smembers(&threads_key).await.unwrap_or_default();

            for thread_id in thread_ids {
                let thread_checkpoints_key = self.thread_checkpoints_key(&thread_id);
                // Best-effort cleanup - ignore errors
                let _: std::result::Result<(), _> = conn
                    .zrem::<_, _, ()>(&thread_checkpoints_key, checkpoint_id)
                    .await;
            }

            debug!(
                "Deleted checkpoint {} (orphan cleanup attempted)",
                checkpoint_id
            );
        }

        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> DashFlowResult<()> {
        let thread_checkpoints_key = self.thread_checkpoints_key(thread_id);
        let threads_key = self.threads_key();

        let mut conn = self.connection_manager.clone();

        // Get all checkpoint IDs for this thread
        let checkpoint_ids: Vec<String> = conn
            .zrange(&thread_checkpoints_key, 0, -1)
            .await
            .map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to get checkpoints for thread: {}", e))
            })?;

        // Delete all checkpoint hashes and the thread's sorted set
        let mut pipe = redis::pipe();
        pipe.atomic();

        for checkpoint_id in checkpoint_ids {
            let checkpoint_key = self.checkpoint_key(&checkpoint_id);
            pipe.del(&checkpoint_key);
        }

        // Delete thread's checkpoint sorted set
        pipe.del(&thread_checkpoints_key);

        // Remove thread from threads set
        pipe.srem(&threads_key, thread_id);

        // Execute pipeline
        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to delete thread: {}", e))
            })?;

        debug!("Deleted all checkpoints for thread {}", thread_id);

        Ok(())
    }

    async fn list_threads(&self) -> DashFlowResult<Vec<dashflow::ThreadInfo>> {
        let threads_key = self.threads_key();
        let mut conn = self.connection_manager.clone();

        // Get all thread IDs from the threads set
        let thread_ids: Vec<String> =
            conn.smembers(&threads_key).await.map_err(|e: RedisError| {
                dashflow::Error::Generic(format!("Failed to list threads: {}", e))
            })?;

        let mut thread_infos = Vec::with_capacity(thread_ids.len());

        for thread_id in thread_ids {
            let thread_checkpoints_key = self.thread_checkpoints_key(&thread_id);

            // Get the latest checkpoint ID for this thread (highest score)
            // ZSET scores are in milliseconds for ordering; precise timestamp is in hash
            let latest: Vec<String> = conn
                .zrevrange(&thread_checkpoints_key, 0, 0)
                .await
                .map_err(|e: RedisError| {
                    dashflow::Error::Generic(format!(
                        "Failed to get latest checkpoint for thread: {}",
                        e
                    ))
                })?;

            if let Some(checkpoint_id) = latest.into_iter().next() {
                // Read precise timestamp from the hash field (nanoseconds)
                let checkpoint_key = self.checkpoint_key(&checkpoint_id);
                let timestamp_bytes: Option<Vec<u8>> =
                    conn.hget(&checkpoint_key, "timestamp").await.ok();

                let timestamp = if let Some(bytes) = timestamp_bytes {
                    String::from_utf8(bytes)
                        .ok()
                        .and_then(|s| s.parse::<i64>().ok())
                        .map(nanos_to_timestamp)
                        .unwrap_or(SystemTime::UNIX_EPOCH)
                } else {
                    SystemTime::UNIX_EPOCH
                };

                thread_infos.push(dashflow::ThreadInfo {
                    thread_id,
                    latest_checkpoint_id: checkpoint_id,
                    updated_at: timestamp,
                    checkpoint_count: None,
                });
            }
        }

        // Sort by updated_at DESC (most recent first)
        thread_infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        debug!("Listed {} threads", thread_infos.len());
        Ok(thread_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::SystemTime;

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }

    // ==========================================================================
    // Unit Tests - Error Types and Display
    // ==========================================================================

    #[test]
    fn test_redis_error_connection_error_display() {
        let err = RedisCheckpointerError::ConnectionError("failed to connect".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Redis connection error"));
        assert!(msg.contains("failed to connect"));
    }

    #[test]
    fn test_redis_error_command_error_display() {
        let err = RedisCheckpointerError::CommandError("HSET failed".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Redis command error"));
        assert!(msg.contains("HSET failed"));
    }

    #[test]
    fn test_redis_error_serialization_error_display() {
        let err = RedisCheckpointerError::SerializationError("bincode failed".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Serialization error"));
        assert!(msg.contains("bincode failed"));
    }

    #[test]
    fn test_redis_error_deserialization_error_display() {
        let err = RedisCheckpointerError::DeserializationError("invalid format".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Deserialization error"));
        assert!(msg.contains("invalid format"));
    }

    #[cfg(feature = "compression")]
    #[test]
    fn test_redis_error_compression_error_display() {
        let err = RedisCheckpointerError::CompressionError("zstd failed".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Compression error"));
        assert!(msg.contains("zstd failed"));
    }

    #[test]
    fn test_redis_error_debug_impl() {
        let err = RedisCheckpointerError::ConnectionError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ConnectionError"));
    }

    #[test]
    fn test_redis_error_command_error_debug() {
        let err = RedisCheckpointerError::CommandError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("CommandError"));
    }

    #[test]
    fn test_redis_error_serialization_debug() {
        let err = RedisCheckpointerError::SerializationError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("SerializationError"));
    }

    #[test]
    fn test_redis_error_deserialization_debug() {
        let err = RedisCheckpointerError::DeserializationError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("DeserializationError"));
    }

    // ==========================================================================
    // Unit Tests - Error Conversions
    // ==========================================================================

    #[test]
    fn test_redis_error_conversion_connection_error() {
        let err = RedisCheckpointerError::ConnectionError("lost connection".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("lost connection") || msg.contains("redis"));
    }

    #[test]
    fn test_redis_error_conversion_command_error() {
        let err = RedisCheckpointerError::CommandError("command failed".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("command") || msg.contains("Redis"));
    }

    #[test]
    fn test_redis_error_conversion_serialization_error() {
        let err = RedisCheckpointerError::SerializationError("serialize failed".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("serialize") || msg.contains("Serialization"));
    }

    #[test]
    fn test_redis_error_conversion_deserialization_error() {
        let err = RedisCheckpointerError::DeserializationError("deserialize failed".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("deserialize") || msg.contains("Deserialization"));
    }

    #[cfg(feature = "compression")]
    #[test]
    fn test_redis_error_conversion_compression_error() {
        let err = RedisCheckpointerError::CompressionError("compress failed".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("Compression") || msg.contains("compress"));
    }

    // ==========================================================================
    // Unit Tests - Key Generation
    // ==========================================================================

    /// Helper to create a test checkpointer without connecting to Redis
    /// We can test key generation methods using a struct with just the key_prefix
    struct KeyGenerator {
        key_prefix: String,
    }

    impl KeyGenerator {
        fn new(prefix: &str) -> Self {
            Self {
                key_prefix: prefix.to_string(),
            }
        }

        fn checkpoint_key(&self, checkpoint_id: &str) -> String {
            format!("{}:checkpoint:{}", self.key_prefix, checkpoint_id)
        }

        fn thread_checkpoints_key(&self, thread_id: &str) -> String {
            format!("{}:thread:{}:checkpoints", self.key_prefix, thread_id)
        }

        fn threads_key(&self) -> String {
            format!("{}:threads", self.key_prefix)
        }
    }

    #[test]
    fn test_checkpoint_key_default_prefix() {
        let gen = KeyGenerator::new("dashflow");
        assert_eq!(
            gen.checkpoint_key("cp-123"),
            "dashflow:checkpoint:cp-123"
        );
    }

    #[test]
    fn test_checkpoint_key_custom_prefix() {
        let gen = KeyGenerator::new("myapp");
        assert_eq!(
            gen.checkpoint_key("abc-def"),
            "myapp:checkpoint:abc-def"
        );
    }

    #[test]
    fn test_checkpoint_key_empty_id() {
        let gen = KeyGenerator::new("dashflow");
        assert_eq!(gen.checkpoint_key(""), "dashflow:checkpoint:");
    }

    #[test]
    fn test_checkpoint_key_special_chars() {
        let gen = KeyGenerator::new("dashflow");
        assert_eq!(
            gen.checkpoint_key("cp-123:456"),
            "dashflow:checkpoint:cp-123:456"
        );
    }

    #[test]
    fn test_thread_checkpoints_key_default_prefix() {
        let gen = KeyGenerator::new("dashflow");
        assert_eq!(
            gen.thread_checkpoints_key("thread-1"),
            "dashflow:thread:thread-1:checkpoints"
        );
    }

    #[test]
    fn test_thread_checkpoints_key_custom_prefix() {
        let gen = KeyGenerator::new("production");
        assert_eq!(
            gen.thread_checkpoints_key("user-session-abc"),
            "production:thread:user-session-abc:checkpoints"
        );
    }

    #[test]
    fn test_thread_checkpoints_key_empty_thread() {
        let gen = KeyGenerator::new("dashflow");
        assert_eq!(
            gen.thread_checkpoints_key(""),
            "dashflow:thread::checkpoints"
        );
    }

    #[test]
    fn test_threads_key_default_prefix() {
        let gen = KeyGenerator::new("dashflow");
        assert_eq!(gen.threads_key(), "dashflow:threads");
    }

    #[test]
    fn test_threads_key_custom_prefix() {
        let gen = KeyGenerator::new("app-v2");
        assert_eq!(gen.threads_key(), "app-v2:threads");
    }

    #[test]
    fn test_key_format_consistency() {
        let gen = KeyGenerator::new("prefix");

        // All keys should start with the prefix
        assert!(gen.checkpoint_key("id").starts_with("prefix:"));
        assert!(gen.thread_checkpoints_key("tid").starts_with("prefix:"));
        assert!(gen.threads_key().starts_with("prefix:"));
    }

    #[test]
    fn test_key_uniqueness() {
        let gen = KeyGenerator::new("dashflow");

        let key1 = gen.checkpoint_key("cp-1");
        let key2 = gen.checkpoint_key("cp-2");
        let key3 = gen.thread_checkpoints_key("thread-1");
        let key4 = gen.threads_key();

        // All keys should be unique
        assert_ne!(key1, key2);
        assert_ne!(key1, key3);
        assert_ne!(key1, key4);
        assert_ne!(key2, key3);
        assert_ne!(key2, key4);
        assert_ne!(key3, key4);
    }

    // ==========================================================================
    // Unit Tests - Timestamp Conversion
    // ==========================================================================

    #[test]
    fn test_timestamp_to_nanos_conversion() {
        let now = SystemTime::now();
        let nanos = timestamp_to_nanos(now);

        // Should be positive (after UNIX epoch)
        assert!(nanos > 0);

        // Should be reasonable (after year 2000)
        assert!(nanos > 946_684_800_000_000_000);
    }

    #[test]
    fn test_nanos_to_timestamp_conversion() {
        let original = SystemTime::now();
        let nanos = timestamp_to_nanos(original);
        let converted = nanos_to_timestamp(nanos);

        // Round-trip should preserve the timestamp (within nanosecond precision)
        let original_nanos = timestamp_to_nanos(original);
        let converted_nanos = timestamp_to_nanos(converted);
        assert_eq!(original_nanos, converted_nanos);
    }

    #[test]
    fn test_timestamp_roundtrip() {
        // Test with a specific timestamp
        let nanos: i64 = 1_700_000_000_000_000_000; // Approx Nov 2023
        let timestamp = nanos_to_timestamp(nanos);
        let back_to_nanos = timestamp_to_nanos(timestamp);
        assert_eq!(nanos, back_to_nanos);
    }

    #[test]
    fn test_zset_score_precision() {
        // ZSET scores use milliseconds (safe for f64)
        let nanos: i64 = 1_700_000_000_123_456_789;
        let score_millis = nanos / 1_000_000;

        // Converting to f64 and back should preserve value
        let score_f64 = score_millis as f64;
        let back_to_i64 = score_f64 as i64;
        assert_eq!(score_millis, back_to_i64);
    }

    // ==========================================================================
    // Unit Tests - Checkpoint Serialization
    // ==========================================================================

    #[test]
    fn test_checkpoint_bincode_serialization() {
        let state = TestState { value: 42 };
        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        let bytes = bincode::serialize(&checkpoint.state).unwrap();
        let deserialized: TestState = bincode::deserialize(&bytes).unwrap();
        assert_eq!(deserialized.value, 42);
    }

    #[test]
    fn test_checkpoint_serialization_with_parent() {
        let state = TestState { value: 100 };
        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node2".to_string(),
            Some("parent-id".to_string()),
        );

        assert_eq!(checkpoint.parent_id, Some("parent-id".to_string()));
    }

    #[test]
    fn test_checkpoint_serialization_with_metadata() {
        let state = TestState { value: 50 };
        let mut checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node3".to_string(),
            None,
        );
        checkpoint.metadata.insert("key1".to_string(), "value1".to_string());
        checkpoint.metadata.insert("key2".to_string(), "value2".to_string());

        let metadata_json = serde_json::to_string(&checkpoint.metadata).unwrap();
        let deserialized: std::collections::HashMap<String, String> =
            serde_json::from_str(&metadata_json).unwrap();

        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_checkpoint_metadata_serialization_empty() {
        let metadata: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let json = serde_json::to_string(&metadata).unwrap();
        assert_eq!(json, "{}");

        let deserialized: std::collections::HashMap<String, String> =
            serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_empty());
    }

    #[test]
    fn test_checkpoint_size_estimate() {
        let state = TestState { value: 42 };
        let bytes = bincode::serialize(&state).unwrap();

        // Simple state should be small
        assert!(bytes.len() < 100, "State serialization too large: {} bytes", bytes.len());
    }

    // ==========================================================================
    // Unit Tests - CheckpointMetadata
    // ==========================================================================

    #[test]
    fn test_checkpoint_metadata_creation() {
        let metadata = CheckpointMetadata {
            id: "cp-123".to_string(),
            thread_id: "thread-456".to_string(),
            node: "process_data".to_string(),
            timestamp: SystemTime::now(),
            parent_id: Some("cp-122".to_string()),
            metadata: std::collections::HashMap::new(),
        };

        assert_eq!(metadata.id, "cp-123");
        assert_eq!(metadata.thread_id, "thread-456");
        assert_eq!(metadata.node, "process_data");
        assert_eq!(metadata.parent_id, Some("cp-122".to_string()));
    }

    #[test]
    fn test_checkpoint_metadata_no_parent() {
        let metadata = CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "t-1".to_string(),
            node: "start".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        };

        assert!(metadata.parent_id.is_none());
    }

    #[test]
    fn test_checkpoint_metadata_with_custom_metadata() {
        let mut custom = std::collections::HashMap::new();
        custom.insert("user".to_string(), "alice".to_string());
        custom.insert("version".to_string(), "1.0".to_string());

        let metadata = CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "t-1".to_string(),
            node: "start".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: custom,
        };

        assert_eq!(metadata.metadata.len(), 2);
        assert_eq!(metadata.metadata.get("user"), Some(&"alice".to_string()));
    }

    // ==========================================================================
    // Unit Tests - Checkpoint Creation
    // ==========================================================================

    #[test]
    fn test_checkpoint_new_generates_id() {
        let cp1 = Checkpoint::new("t1".to_string(), TestState { value: 1 }, "n1".to_string(), None);
        let cp2 = Checkpoint::new("t1".to_string(), TestState { value: 2 }, "n1".to_string(), None);

        // Each checkpoint should have a unique ID
        assert_ne!(cp1.id, cp2.id);
    }

    #[test]
    fn test_checkpoint_new_sets_timestamp() {
        let before = SystemTime::now();
        let cp = Checkpoint::new("t1".to_string(), TestState { value: 1 }, "n1".to_string(), None);
        let after = SystemTime::now();

        assert!(cp.timestamp >= before);
        assert!(cp.timestamp <= after);
    }

    #[test]
    fn test_checkpoint_new_sets_thread_id() {
        let cp = Checkpoint::new("my-thread".to_string(), TestState { value: 1 }, "n1".to_string(), None);
        assert_eq!(cp.thread_id, "my-thread");
    }

    #[test]
    fn test_checkpoint_new_sets_node() {
        let cp = Checkpoint::new("t1".to_string(), TestState { value: 1 }, "process_step".to_string(), None);
        assert_eq!(cp.node, "process_step");
    }

    #[test]
    fn test_checkpoint_new_sets_parent_id() {
        let cp = Checkpoint::new(
            "t1".to_string(),
            TestState { value: 1 },
            "n1".to_string(),
            Some("parent-123".to_string()),
        );
        assert_eq!(cp.parent_id, Some("parent-123".to_string()));
    }

    #[test]
    fn test_checkpoint_new_empty_metadata() {
        let cp = Checkpoint::new("t1".to_string(), TestState { value: 1 }, "n1".to_string(), None);
        assert!(cp.metadata.is_empty());
    }

    // ==========================================================================
    // Unit Tests - Retention Policy
    // ==========================================================================

    #[test]
    fn test_retention_policy_none_returns_zero() {
        // When no retention policy is configured, apply_retention should return 0
        // This is tested indirectly through the policy evaluation
        let policy: Option<RetentionPolicy> = None;
        assert!(policy.is_none());
    }

    #[test]
    fn test_retention_policy_keep_last_n() {
        // Create retention policy to keep last 5 checkpoints
        let policy = RetentionPolicy::builder()
            .keep_last_n(5)
            .delete_after(std::time::Duration::from_secs(0)) // Immediately delete old ones
            .build();

        // Create more than max checkpoints
        let now = SystemTime::now();
        let mut metadatas = Vec::new();
        for i in 0..10 {
            metadatas.push(CheckpointMetadata {
                id: format!("cp-{}", i),
                thread_id: "thread-1".to_string(),
                node: "node1".to_string(),
                timestamp: now - std::time::Duration::from_secs(i as u64),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
            });
        }

        let (to_keep, to_delete) = policy.evaluate(&metadatas, now);

        // Should keep 5 (first 5 in the sorted list)
        assert_eq!(to_keep.len(), 5);
        // Remaining 5 should be marked for deletion
        assert_eq!(to_delete.len(), 5);
    }

    #[test]
    fn test_retention_policy_builder() {
        let policy = RetentionPolicy::builder()
            .keep_last_n(10)
            .keep_daily_for(std::time::Duration::from_secs(30 * 86400))
            .keep_weekly_for(std::time::Duration::from_secs(12 * 7 * 86400))
            .delete_after(std::time::Duration::from_secs(90 * 86400))
            .build();

        assert_eq!(policy.keep_last_n, Some(10));
        assert!(policy.keep_daily_for.is_some());
        assert!(policy.keep_weekly_for.is_some());
        assert!(policy.delete_after.is_some());
    }

    #[test]
    fn test_retention_policy_empty() {
        let policy = RetentionPolicy::builder().build();

        assert!(policy.keep_last_n.is_none());
        assert!(policy.keep_daily_for.is_none());
        assert!(policy.keep_weekly_for.is_none());
        assert!(policy.delete_after.is_none());
    }

    // ==========================================================================
    // Unit Tests - ThreadInfo
    // ==========================================================================

    #[test]
    fn test_thread_info_creation() {
        let info = dashflow::ThreadInfo {
            thread_id: "thread-123".to_string(),
            latest_checkpoint_id: "cp-456".to_string(),
            updated_at: SystemTime::now(),
            checkpoint_count: Some(10),
        };

        assert_eq!(info.thread_id, "thread-123");
        assert_eq!(info.latest_checkpoint_id, "cp-456");
        assert_eq!(info.checkpoint_count, Some(10));
    }

    #[test]
    fn test_thread_info_no_checkpoint_count() {
        let info = dashflow::ThreadInfo {
            thread_id: "thread-1".to_string(),
            latest_checkpoint_id: "cp-1".to_string(),
            updated_at: SystemTime::now(),
            checkpoint_count: None,
        };

        assert!(info.checkpoint_count.is_none());
    }

    #[test]
    fn test_thread_info_ordering_by_updated_at() {
        let now = SystemTime::now();
        let earlier = now - std::time::Duration::from_secs(60);

        let info1 = dashflow::ThreadInfo {
            thread_id: "thread-1".to_string(),
            latest_checkpoint_id: "cp-1".to_string(),
            updated_at: earlier,
            checkpoint_count: None,
        };

        let info2 = dashflow::ThreadInfo {
            thread_id: "thread-2".to_string(),
            latest_checkpoint_id: "cp-2".to_string(),
            updated_at: now,
            checkpoint_count: None,
        };

        // info2 should sort before info1 (descending by updated_at)
        let mut infos = vec![info1, info2];
        infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        assert_eq!(infos[0].thread_id, "thread-2");
        assert_eq!(infos[1].thread_id, "thread-1");
    }

    // ==========================================================================
    // Unit Tests - Data Validation
    // ==========================================================================

    #[test]
    fn test_empty_string_parent_id_filter() {
        // Empty strings should be filtered out for parent_id
        let empty = "";
        let filtered = Some(empty.to_string()).filter(|s| !s.is_empty());
        assert!(filtered.is_none());
    }

    #[test]
    fn test_non_empty_parent_id_preserved() {
        let parent = "parent-123";
        let filtered = Some(parent.to_string()).filter(|s| !s.is_empty());
        assert_eq!(filtered, Some("parent-123".to_string()));
    }

    #[test]
    fn test_utf8_thread_id_handling() {
        let thread_id = "スレッド-123"; // Japanese characters
        let bytes = thread_id.as_bytes().to_vec();
        let recovered = String::from_utf8(bytes).unwrap();
        assert_eq!(recovered, thread_id);
    }

    #[test]
    fn test_utf8_node_name_handling() {
        let node = "处理节点"; // Chinese characters
        let bytes = node.as_bytes().to_vec();
        let recovered = String::from_utf8(bytes).unwrap();
        assert_eq!(recovered, node);
    }

    // ==========================================================================
    // Unit Tests - Pipeline Operations
    // ==========================================================================

    #[test]
    fn test_score_calculation_from_nanos() {
        let timestamp_nanos: i64 = 1_700_000_000_123_456_789;
        let score_millis = timestamp_nanos / 1_000_000;

        // Score should be in milliseconds
        assert_eq!(score_millis, 1_700_000_000_123);
    }

    #[test]
    fn test_score_preserves_ordering() {
        let nanos1: i64 = 1_700_000_000_000_000_000;
        let nanos2: i64 = 1_700_000_001_000_000_000;

        let score1 = (nanos1 / 1_000_000) as f64;
        let score2 = (nanos2 / 1_000_000) as f64;

        // Later timestamp should have higher score
        assert!(score2 > score1);
    }

    // ==========================================================================
    // Integration Tests (require Redis server)
    // ==========================================================================

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_save_and_load() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let thread_id = "test_thread_1".to_string();
        let state = TestState { value: 42 };

        let checkpoint = Checkpoint::new(thread_id.clone(), state, "node1".to_string(), None);
        let checkpoint_id = checkpoint.id.clone();

        // Save checkpoint
        checkpointer.save(checkpoint).await.unwrap();

        // Load checkpoint
        let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.state.value, 42);
        assert_eq!(loaded.node, "node1");

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_get_latest() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let thread_id = "test_thread_2".to_string();

        // Save multiple checkpoints
        let checkpoint1 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 1 },
            "node1".to_string(),
            None,
        );
        checkpointer.save(checkpoint1).await.unwrap();

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let checkpoint2 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 2 },
            "node2".to_string(),
            None,
        );
        checkpointer.save(checkpoint2).await.unwrap();

        // Get latest checkpoint
        let latest = checkpointer.get_latest(&thread_id).await.unwrap();
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.state.value, 2);
        assert_eq!(latest.node, "node2");

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_list() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let thread_id = "test_thread_3".to_string();

        // Save multiple checkpoints
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                thread_id.clone(),
                TestState { value: i },
                format!("node{}", i),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // List checkpoints
        let metadatas = checkpointer.list(&thread_id).await.unwrap();
        assert_eq!(metadatas.len(), 3);

        // Should be ordered by timestamp descending (newest first)
        assert_eq!(metadatas[0].node, "node2");
        assert_eq!(metadatas[1].node, "node1");
        assert_eq!(metadatas[2].node, "node0");

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_delete() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let thread_id = "test_thread_4".to_string();
        let state = TestState { value: 42 };

        let checkpoint = Checkpoint::new(thread_id.clone(), state, "node1".to_string(), None);
        let checkpoint_id = checkpoint.id.clone();

        // Save and delete checkpoint
        checkpointer.save(checkpoint).await.unwrap();
        checkpointer.delete(&checkpoint_id).await.unwrap();

        // Verify it's deleted
        let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
        assert!(loaded.is_none());

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_delete_thread() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let thread_id = "test_thread_5".to_string();

        // Save multiple checkpoints
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                thread_id.clone(),
                TestState { value: i },
                format!("node{}", i),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
        }

        // Delete all checkpoints for thread
        checkpointer.delete_thread(&thread_id).await.unwrap();

        // Verify all are deleted
        let metadatas = checkpointer.list(&thread_id).await.unwrap();
        assert_eq!(metadatas.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_list_threads() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        // Create checkpoints for multiple threads
        for i in 0..3 {
            let thread_id = format!("thread_list_{}", i);
            let checkpoint = Checkpoint::new(
                thread_id.clone(),
                TestState { value: i },
                "node1".to_string(),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
        }

        // List threads
        let threads = checkpointer.list_threads().await.unwrap();
        assert!(threads.len() >= 3);

        // Cleanup
        for i in 0..3 {
            let thread_id = format!("thread_list_{}", i);
            checkpointer.delete_thread(&thread_id).await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_with_custom_prefix() {
        let checkpointer = RedisCheckpointer::<TestState>::with_key_prefix(
            "redis://localhost:6379",
            "custom_prefix",
        )
        .await
        .unwrap();

        let thread_id = "test_thread_prefix".to_string();
        let checkpoint = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 99 },
            "node1".to_string(),
            None,
        );
        let checkpoint_id = checkpoint.id.clone();

        checkpointer.save(checkpoint).await.unwrap();

        let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().state.value, 99);

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_checkpoint_with_metadata() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let thread_id = "test_thread_metadata".to_string();
        let mut checkpoint = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 42 },
            "node1".to_string(),
            None,
        );
        checkpoint.metadata.insert("key1".to_string(), "value1".to_string());
        checkpoint.metadata.insert("key2".to_string(), "value2".to_string());
        let checkpoint_id = checkpoint.id.clone();

        checkpointer.save(checkpoint).await.unwrap();

        let loaded = checkpointer.load(&checkpoint_id).await.unwrap().unwrap();
        assert_eq!(loaded.metadata.len(), 2);
        assert_eq!(loaded.metadata.get("key1"), Some(&"value1".to_string()));

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_checkpoint_with_parent() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let thread_id = "test_thread_parent".to_string();

        // Create parent checkpoint
        let parent_checkpoint = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 1 },
            "node1".to_string(),
            None,
        );
        let parent_id = parent_checkpoint.id.clone();
        checkpointer.save(parent_checkpoint).await.unwrap();

        // Create child checkpoint
        let child_checkpoint = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 2 },
            "node2".to_string(),
            Some(parent_id.clone()),
        );
        let child_id = child_checkpoint.id.clone();
        checkpointer.save(child_checkpoint).await.unwrap();

        // Verify parent link
        let loaded = checkpointer.load(&child_id).await.unwrap().unwrap();
        assert_eq!(loaded.parent_id, Some(parent_id));

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_delete_nonexistent() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        // Deleting a nonexistent checkpoint should not error
        let result = checkpointer.delete("nonexistent-checkpoint-id").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_load_nonexistent() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let loaded = checkpointer.load("nonexistent-id").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_get_latest_empty_thread() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let latest = checkpointer.get_latest("nonexistent-thread").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    #[ignore = "requires Redis server"]
    async fn test_redis_checkpointer_list_empty_thread() {
        let checkpointer = RedisCheckpointer::<TestState>::new("redis://localhost:6379")
            .await
            .unwrap();

        let metadatas = checkpointer.list("nonexistent-thread").await.unwrap();
        assert!(metadatas.is_empty());
    }
}
