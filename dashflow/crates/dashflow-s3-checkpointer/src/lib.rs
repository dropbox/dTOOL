//! S3 checkpointer for `DashFlow`
//!
//! Provides persistent checkpoint storage using Amazon S3 or S3-compatible object storage.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_s3_checkpointer::S3Checkpointer;
//! use dashflow::{StateGraph, GraphState};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MyState {
//!     value: i32,
//! }
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let bucket = "my-checkpoints";
//!     let checkpointer = S3Checkpointer::new(bucket).await?;
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
//! - [`dashflow-dynamodb-checkpointer`](https://docs.rs/dashflow-dynamodb-checkpointer) - Alternative: DynamoDB-based checkpointing (AWS)
//! - [`dashflow-postgres-checkpointer`](https://docs.rs/dashflow-postgres-checkpointer) - Alternative: PostgreSQL-based checkpointing
//! - [AWS S3 Documentation](https://docs.aws.amazon.com/s3/) - Official S3 docs

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use dashflow::{
    Checkpoint, CheckpointMetadata, Checkpointer, GraphState, Result as DashFlowResult,
    RetentionPolicy,
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use tracing::{debug, error, info, warn};

#[cfg(feature = "compression")]
use dashflow_compression::{Compression, CompressionType};

/// Errors that can occur when using S3 checkpointer
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum S3CheckpointerError {
    #[error("S3 connection error: {0}")]
    ConnectionError(String),

    #[error("S3 operation error: {0}")]
    OperationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[cfg(feature = "compression")]
    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Object not found: {0}")]
    NotFound(String),
}

impl From<S3CheckpointerError> for dashflow::Error {
    fn from(err: S3CheckpointerError) -> Self {
        use dashflow::error::CheckpointError;
        let checkpoint_err = match err {
            S3CheckpointerError::ConnectionError(msg) => CheckpointError::ConnectionLost {
                backend: "s3".to_string(),
                reason: msg,
            },
            S3CheckpointerError::OperationError(msg) => {
                CheckpointError::Other(format!("S3 operation error: {}", msg))
            }
            S3CheckpointerError::SerializationError(msg) => {
                CheckpointError::SerializationFailed { reason: msg }
            }
            S3CheckpointerError::DeserializationError(msg) => {
                CheckpointError::DeserializationFailed { reason: msg }
            }
            S3CheckpointerError::NotFound(id) => CheckpointError::NotFound { checkpoint_id: id },
            #[cfg(feature = "compression")]
            S3CheckpointerError::CompressionError(msg) => {
                CheckpointError::Other(format!("Compression error: {}", msg))
            }
        };
        dashflow::Error::Checkpoint(checkpoint_err)
    }
}

/// Checkpoint data stored in S3
///
/// Contains the checkpoint along with its metadata for efficient listing
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize",
    deserialize = "S: for<'de2> Deserialize<'de2>"
))]
struct StoredCheckpoint<S: GraphState> {
    checkpoint: Checkpoint<S>,
}

/// Thread index stored in S3
///
/// Maintains a list of checkpoint metadata for efficient listing
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ThreadIndex {
    checkpoints: Vec<CheckpointMetadata>,
}

impl ThreadIndex {
    fn new() -> Self {
        Self {
            checkpoints: Vec::new(),
        }
    }

    fn add(&mut self, metadata: CheckpointMetadata) {
        // Add to front (newest first)
        self.checkpoints.insert(0, metadata);
    }

    fn remove(&mut self, checkpoint_id: &str) -> bool {
        let len_before = self.checkpoints.len();
        self.checkpoints.retain(|m| m.id != checkpoint_id);
        len_before != self.checkpoints.len()
    }

    fn get_latest(&self) -> Option<&CheckpointMetadata> {
        self.checkpoints.first()
    }
}

/// S3-backed checkpointer
///
/// Stores checkpoints in Amazon S3 or S3-compatible object storage with the following structure:
/// - Checkpoint objects: `{prefix}/checkpoints/{checkpoint_id}` containing bincode-encoded checkpoint
/// - Thread indexes: `{prefix}/threads/{thread_id}/index.json` containing checkpoint metadata list
///
/// # Performance Considerations
///
/// - Checkpoints are stored as individual S3 objects for parallel access
/// - Thread indexes maintain checkpoint metadata for efficient listing (avoids `ListObjects`)
/// - Bincode serialization for compact checkpoint storage
/// - Supports S3-compatible storage (`MinIO`, `DigitalOcean` Spaces, etc.)
pub struct S3Checkpointer<S: GraphState> {
    client: Client,
    bucket: String,
    prefix: String,
    #[cfg(feature = "compression")]
    compression: Option<Box<dyn Compression>>,
    retention_policy: Option<RetentionPolicy>,
    _phantom: PhantomData<S>,
}

/// Threshold for using multipart upload (5 MB)
/// AWS recommends multipart upload for objects larger than 5 MB
const MULTIPART_THRESHOLD: usize = 5 * 1024 * 1024;

/// Multipart upload chunk size (5 MB)
/// AWS requires minimum 5 MB per part (except last part)
const MULTIPART_CHUNK_SIZE: usize = 5 * 1024 * 1024;

impl<S: GraphState> S3Checkpointer<S> {
    /// Create a new S3 checkpointer using default AWS configuration
    ///
    /// # Arguments
    /// * `bucket` - S3 bucket name
    ///
    /// # Errors
    /// Returns error if AWS configuration fails
    pub async fn new(bucket: &str) -> Result<Self, S3CheckpointerError> {
        Self::with_prefix(bucket, "dashflow").await
    }

    /// Create a new S3 checkpointer with custom prefix
    ///
    /// # Arguments
    /// * `bucket` - S3 bucket name
    /// * `prefix` - Prefix for all S3 keys (default: "dashflow")
    pub async fn with_prefix(bucket: &str, prefix: &str) -> Result<Self, S3CheckpointerError> {
        info!(
            "Initializing S3 checkpointer: bucket={}, prefix={}",
            bucket, prefix
        );

        let config = aws_config::load_from_env().await;
        let client = Client::new(&config);

        debug!("S3 client initialized");

        Ok(Self {
            client,
            bucket: bucket.to_string(),
            prefix: prefix.to_string(),
            #[cfg(feature = "compression")]
            compression: None,
            retention_policy: None,
            _phantom: PhantomData,
        })
    }

    /// Create a new S3 checkpointer with custom client and configuration
    ///
    /// Useful for testing with `LocalStack` or custom S3 endpoints
    #[must_use]
    pub fn with_client(client: Client, bucket: &str, prefix: &str) -> Self {
        Self {
            client,
            bucket: bucket.to_string(),
            prefix: prefix.to_string(),
            #[cfg(feature = "compression")]
            compression: None,
            retention_policy: None,
            _phantom: PhantomData,
        }
    }

    /// Enable compression for checkpoint storage
    ///
    /// # Arguments
    /// * `compression_type` - The compression algorithm and configuration to use
    #[cfg(feature = "compression")]
    pub fn with_compression(
        mut self,
        compression_type: CompressionType,
    ) -> Result<Self, S3CheckpointerError> {
        self.compression = Some(
            compression_type
                .build()
                .map_err(|e| S3CheckpointerError::CompressionError(e.to_string()))?,
        );
        Ok(self)
    }

    /// Set retention policy for automatic checkpoint cleanup
    ///
    /// # Arguments
    /// * `policy` - The retention policy to apply
    #[must_use]
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
        use std::time::SystemTime;

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

    /// Get the S3 key for a checkpoint object
    fn checkpoint_key(&self, checkpoint_id: &str) -> String {
        format!("{}/checkpoints/{}", self.prefix, checkpoint_id)
    }

    /// Get the S3 key for a thread index
    fn thread_index_key(&self, thread_id: &str) -> String {
        format!("{}/threads/{}/index.json", self.prefix, thread_id)
    }

    /// Load thread index from S3
    async fn load_thread_index(&self, thread_id: &str) -> Result<ThreadIndex, S3CheckpointerError> {
        let key = self.thread_index_key(thread_id);

        match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(output) => {
                let bytes = output.body.collect().await.map_err(|e| {
                    S3CheckpointerError::OperationError(format!("Failed to read index body: {e}"))
                })?;

                let index: ThreadIndex =
                    serde_json::from_slice(&bytes.into_bytes()).map_err(|e| {
                        S3CheckpointerError::DeserializationError(format!(
                            "Failed to deserialize thread index: {e}"
                        ))
                    })?;

                Ok(index)
            }
            Err(e) => {
                // Check if object doesn't exist (404)
                if e.to_string().contains("NoSuchKey") {
                    debug!(
                        "Thread index not found, returning empty index: thread_id={}",
                        thread_id
                    );
                    Ok(ThreadIndex::new())
                } else {
                    Err(S3CheckpointerError::OperationError(format!(
                        "Failed to load thread index: {e}"
                    )))
                }
            }
        }
    }

    /// Save thread index to S3
    async fn save_thread_index(
        &self,
        thread_id: &str,
        index: &ThreadIndex,
    ) -> Result<(), S3CheckpointerError> {
        let key = self.thread_index_key(thread_id);

        let index_json = serde_json::to_vec(index).map_err(|e| {
            S3CheckpointerError::SerializationError(format!(
                "Failed to serialize thread index: {e}"
            ))
        })?;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(index_json))
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| {
                S3CheckpointerError::OperationError(format!("Failed to save thread index: {e}"))
            })?;

        debug!("Saved thread index: thread_id={}", thread_id);
        Ok(())
    }

    /// Upload checkpoint data to S3 using multipart upload for large objects
    ///
    /// AWS recommends multipart upload for objects larger than 5 MB:
    /// - Better performance for large uploads
    /// - Automatic retry on part failure
    /// - Progress tracking for large objects
    ///
    /// # Arguments
    /// * `key` - S3 object key
    /// * `data` - Checkpoint data bytes
    ///
    /// # Returns
    /// Ok(()) if upload succeeds, error otherwise
    async fn upload_checkpoint_data(
        &self,
        key: &str,
        data: Vec<u8>,
    ) -> Result<(), S3CheckpointerError> {
        let size = data.len();

        // Use simple put_object for small checkpoints
        if size < MULTIPART_THRESHOLD {
            debug!("Using simple put_object for {} bytes", size);
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(ByteStream::from(data))
                .content_type("application/octet-stream")
                .send()
                .await
                .map_err(|e| {
                    error!("Failed to upload checkpoint: {}", e);
                    S3CheckpointerError::OperationError(format!("S3 upload failed: {e}"))
                })?;
            return Ok(());
        }

        // Use multipart upload for large checkpoints (>5 MB)
        info!(
            "Using multipart upload for {} bytes ({} MB)",
            size,
            size / (1024 * 1024)
        );

        // Step 1: Initiate multipart upload
        let multipart_upload = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .content_type("application/octet-stream")
            .send()
            .await
            .map_err(|e| {
                error!("Failed to initiate multipart upload: {}", e);
                S3CheckpointerError::OperationError(format!("Multipart init failed: {e}"))
            })?;

        let upload_id = multipart_upload.upload_id().ok_or_else(|| {
            S3CheckpointerError::OperationError("No upload ID returned".to_string())
        })?;

        debug!("Initiated multipart upload: upload_id={}", upload_id);

        // Step 2: Upload parts in chunks
        let mut part_number = 1;
        let mut completed_parts = Vec::new();
        let mut offset = 0;

        while offset < size {
            let end = std::cmp::min(offset + MULTIPART_CHUNK_SIZE, size);
            let chunk = &data[offset..end];

            debug!(
                "Uploading part {}: offset={}, size={} bytes",
                part_number,
                offset,
                chunk.len()
            );

            let upload_part_result = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(key)
                .upload_id(upload_id)
                .part_number(part_number)
                .body(ByteStream::from(chunk.to_vec()))
                .send()
                .await
                .map_err(|e| {
                    error!("Failed to upload part {}: {}", part_number, e);
                    S3CheckpointerError::OperationError(format!(
                        "Part {part_number} upload failed: {e}"
                    ))
                })?;

            // Record completed part
            if let Some(e_tag) = upload_part_result.e_tag() {
                completed_parts.push(
                    aws_sdk_s3::types::CompletedPart::builder()
                        .part_number(part_number)
                        .e_tag(e_tag)
                        .build(),
                );
                debug!("Part {} uploaded successfully: etag={}", part_number, e_tag);
            } else {
                return Err(S3CheckpointerError::OperationError(format!(
                    "No ETag returned for part {part_number}"
                )));
            }

            part_number += 1;
            offset = end;
        }

        // Step 3: Complete multipart upload
        let completed_multipart_upload = aws_sdk_s3::types::CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed_multipart_upload)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to complete multipart upload: {}", e);
                S3CheckpointerError::OperationError(format!("Multipart complete failed: {e}"))
            })?;

        info!(
            "Multipart upload completed: {} parts, {} bytes total",
            part_number - 1,
            size
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for S3Checkpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> DashFlowResult<()> {
        let checkpoint_key = self.checkpoint_key(&checkpoint.id);

        // Serialize checkpoint with bincode for compact storage
        let stored = StoredCheckpoint {
            checkpoint: checkpoint.clone(),
        };

        #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
        let mut checkpoint_bytes = bincode::serialize(&stored).map_err(|e| {
            error!("Failed to serialize checkpoint: {}", e);
            dashflow::Error::Generic(format!("Serialization error: {e}"))
        })?;

        // Apply compression if enabled
        #[cfg(feature = "compression")]
        if let Some(ref compressor) = self.compression {
            checkpoint_bytes = compressor.compress(&checkpoint_bytes).map_err(|e| {
                error!("Failed to compress checkpoint: {}", e);
                dashflow::Error::Generic(format!("Compression error: {}", e))
            })?;
            debug!("Compressed checkpoint to {} bytes", checkpoint_bytes.len());
        }

        // Save checkpoint object (uses multipart upload for large checkpoints)
        self.upload_checkpoint_data(&checkpoint_key, checkpoint_bytes)
            .await?;

        debug!(
            "Saved checkpoint to S3: id={}, key={}",
            checkpoint.id, checkpoint_key
        );

        // Update thread index
        let mut index = self.load_thread_index(&checkpoint.thread_id).await?;

        // Check if checkpoint already exists in index (update case)
        index.remove(&checkpoint.id);

        // Add checkpoint metadata to index
        let metadata = CheckpointMetadata {
            id: checkpoint.id.clone(),
            thread_id: checkpoint.thread_id.clone(),
            node: checkpoint.node.clone(),
            timestamp: checkpoint.timestamp,
            parent_id: checkpoint.parent_id.clone(),
            metadata: checkpoint.metadata.clone(),
        };
        index.add(metadata);

        // Save updated index
        self.save_thread_index(&checkpoint.thread_id, &index)
            .await?;

        Ok(())
    }

    async fn load(&self, checkpoint_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        let checkpoint_key = self.checkpoint_key(checkpoint_id);

        match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&checkpoint_key)
            .send()
            .await
        {
            Ok(output) => {
                let bytes = output.body.collect().await.map_err(|e| {
                    error!("Failed to read checkpoint body: {}", e);
                    dashflow::Error::Generic(format!("S3 read error: {e}"))
                })?;

                #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
                let mut checkpoint_bytes = bytes.into_bytes().to_vec();

                // Decompress if compression is enabled
                #[cfg(feature = "compression")]
                if let Some(ref compressor) = self.compression {
                    checkpoint_bytes = compressor.decompress(&checkpoint_bytes).map_err(|e| {
                        error!("Failed to decompress checkpoint: {}", e);
                        dashflow::Error::Generic(format!("Decompression error: {}", e))
                    })?;
                    debug!(
                        "Decompressed checkpoint to {} bytes",
                        checkpoint_bytes.len()
                    );
                }

                let stored: StoredCheckpoint<S> =
                    bincode::deserialize(&checkpoint_bytes).map_err(|e| {
                        error!("Failed to deserialize checkpoint: {}", e);
                        dashflow::Error::Generic(format!("Deserialization error: {e}"))
                    })?;

                debug!("Loaded checkpoint from S3: id={}", checkpoint_id);
                Ok(Some(stored.checkpoint))
            }
            Err(e) => {
                // Check if object doesn't exist (404)
                if e.to_string().contains("NoSuchKey") {
                    debug!("Checkpoint not found: id={}", checkpoint_id);
                    Ok(None)
                } else {
                    error!("Failed to load checkpoint from S3: {}", e);
                    Err(dashflow::Error::Generic(format!("S3 error: {e}")))
                }
            }
        }
    }

    async fn get_latest(&self, thread_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        let index = self.load_thread_index(thread_id).await?;

        if let Some(metadata) = index.get_latest() {
            self.load(&metadata.id).await
        } else {
            Ok(None)
        }
    }

    async fn list(&self, thread_id: &str) -> DashFlowResult<Vec<CheckpointMetadata>> {
        let index = self.load_thread_index(thread_id).await?;
        debug!(
            "Listed {} checkpoints for thread {}",
            index.checkpoints.len(),
            thread_id
        );
        Ok(index.checkpoints)
    }

    async fn delete(&self, checkpoint_id: &str) -> DashFlowResult<()> {
        let checkpoint_key = self.checkpoint_key(checkpoint_id);

        // First, load the checkpoint to get thread_id
        let checkpoint = self.load(checkpoint_id).await?;

        if let Some(checkpoint) = checkpoint {
            // Delete checkpoint object
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(&checkpoint_key)
                .send()
                .await
                .map_err(|e| {
                    error!("Failed to delete checkpoint from S3: {}", e);
                    dashflow::Error::Generic(format!("S3 error: {e}"))
                })?;

            debug!("Deleted checkpoint from S3: id={}", checkpoint_id);

            // Update thread index
            let mut index = self.load_thread_index(&checkpoint.thread_id).await?;
            index.remove(checkpoint_id);
            self.save_thread_index(&checkpoint.thread_id, &index)
                .await?;
        } else {
            warn!("Checkpoint not found for deletion: id={}", checkpoint_id);
        }

        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> DashFlowResult<()> {
        let index = self.load_thread_index(thread_id).await?;

        // Delete all checkpoint objects
        for metadata in &index.checkpoints {
            let checkpoint_key = self.checkpoint_key(&metadata.id);
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(&checkpoint_key)
                .send()
                .await
                .map_err(|e| {
                    error!("Failed to delete checkpoint from S3: {}", e);
                    dashflow::Error::Generic(format!("S3 error: {e}"))
                })?;
        }

        // Delete thread index
        let index_key = self.thread_index_key(thread_id);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&index_key)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to delete thread index from S3: {}", e);
                dashflow::Error::Generic(format!("S3 error: {e}"))
            })?;

        debug!(
            "Deleted thread and {} checkpoints: thread_id={}",
            index.checkpoints.len(),
            thread_id
        );

        Ok(())
    }

    async fn list_threads(&self) -> DashFlowResult<Vec<dashflow::ThreadInfo>> {
        // List all thread index objects in S3
        let list_prefix = format!("{}/threads/", self.prefix);

        let mut thread_infos = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&list_prefix);

            if let Some(token) = continuation_token {
                req = req.continuation_token(token);
            }

            let result = req.send().await.map_err(|e| {
                error!("Failed to list thread indices from S3: {}", e);
                dashflow::Error::Generic(format!("S3 error: {e}"))
            })?;

            if let Some(contents) = result.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        // Extract thread_id from key: prefix/threads/{thread_id}.index
                        let thread_id = key
                            .strip_prefix(&list_prefix)
                            .and_then(|s| s.strip_suffix(".index"))
                            .map(|s| s.to_string());

                        if let Some(thread_id) = thread_id {
                            // Load thread index to get latest checkpoint info
                            if let Ok(index) = self.load_thread_index(&thread_id).await {
                                if let Some(latest) = index.get_latest() {
                                    thread_infos.push(dashflow::ThreadInfo {
                                        thread_id,
                                        latest_checkpoint_id: latest.id.clone(),
                                        updated_at: latest.timestamp,
                                        checkpoint_count: Some(index.checkpoints.len()),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Check if there are more pages
            if result.is_truncated == Some(true) {
                continuation_token = result.next_continuation_token;
            } else {
                break;
            }
        }

        // Sort by updated_at DESC (most recent first)
        thread_infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        debug!("Listed {} threads from S3", thread_infos.len());
        Ok(thread_infos)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
    // Unit Tests - Error Types
    // ==========================================================================

    #[test]
    fn test_s3_error_connection_error_display() {
        let err = S3CheckpointerError::ConnectionError("failed to connect".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("S3 connection error"));
        assert!(msg.contains("failed to connect"));
    }

    #[test]
    fn test_s3_error_operation_error_display() {
        let err = S3CheckpointerError::OperationError("put object failed".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("S3 operation error"));
        assert!(msg.contains("put object failed"));
    }

    #[test]
    fn test_s3_error_serialization_error_display() {
        let err = S3CheckpointerError::SerializationError("bincode failed".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Serialization error"));
        assert!(msg.contains("bincode failed"));
    }

    #[test]
    fn test_s3_error_deserialization_error_display() {
        let err = S3CheckpointerError::DeserializationError("invalid format".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Deserialization error"));
        assert!(msg.contains("invalid format"));
    }

    #[test]
    fn test_s3_error_not_found_display() {
        let err = S3CheckpointerError::NotFound("checkpoint-123".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Object not found"));
        assert!(msg.contains("checkpoint-123"));
    }

    #[test]
    fn test_s3_error_debug_impl() {
        let err = S3CheckpointerError::ConnectionError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ConnectionError"));
    }

    #[test]
    fn test_s3_error_conversion_connection_error() {
        let err = S3CheckpointerError::ConnectionError("lost connection".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("lost connection") || msg.contains("s3"));
    }

    #[test]
    fn test_s3_error_conversion_operation_error() {
        let err = S3CheckpointerError::OperationError("op failed".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("S3 operation error") || msg.contains("op failed"));
    }

    #[test]
    fn test_s3_error_conversion_serialization_error() {
        let err = S3CheckpointerError::SerializationError("serialize failed".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("serialize"));
    }

    #[test]
    fn test_s3_error_conversion_deserialization_error() {
        let err = S3CheckpointerError::DeserializationError("deserialize failed".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("deserialize"));
    }

    #[test]
    fn test_s3_error_conversion_not_found() {
        let err = S3CheckpointerError::NotFound("cp-456".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = format!("{}", dashflow_err);
        assert!(msg.contains("cp-456") || msg.contains("not found"));
    }

    // ==========================================================================
    // Unit Tests - ThreadIndex
    // ==========================================================================

    #[test]
    fn test_thread_index_new() {
        let index = ThreadIndex::new();
        assert!(index.checkpoints.is_empty());
        assert!(index.get_latest().is_none());
    }

    #[test]
    fn test_thread_index_add_single() {
        let mut index = ThreadIndex::new();
        let metadata = CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            node: "node1".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        };

        index.add(metadata.clone());

        assert_eq!(index.checkpoints.len(), 1);
        assert_eq!(index.get_latest().unwrap().id, "cp-1");
    }

    #[test]
    fn test_thread_index_add_multiple() {
        let mut index = ThreadIndex::new();

        for i in 0..5 {
            let metadata = CheckpointMetadata {
                id: format!("cp-{}", i),
                thread_id: "thread-1".to_string(),
                node: format!("node{}", i),
                timestamp: SystemTime::now(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
            };
            index.add(metadata);
        }

        assert_eq!(index.checkpoints.len(), 5);
        // Latest should be the last added (inserted at front)
        assert_eq!(index.get_latest().unwrap().id, "cp-4");
    }

    #[test]
    fn test_thread_index_add_inserts_at_front() {
        let mut index = ThreadIndex::new();

        index.add(CheckpointMetadata {
            id: "cp-old".to_string(),
            thread_id: "t".to_string(),
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        });

        index.add(CheckpointMetadata {
            id: "cp-new".to_string(),
            thread_id: "t".to_string(),
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        });

        // Newest should be first
        assert_eq!(index.checkpoints[0].id, "cp-new");
        assert_eq!(index.checkpoints[1].id, "cp-old");
    }

    #[test]
    fn test_thread_index_remove_existing() {
        let mut index = ThreadIndex::new();

        index.add(CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "t".to_string(),
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        });

        index.add(CheckpointMetadata {
            id: "cp-2".to_string(),
            thread_id: "t".to_string(),
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        });

        let removed = index.remove("cp-1");
        assert!(removed);
        assert_eq!(index.checkpoints.len(), 1);
        assert_eq!(index.checkpoints[0].id, "cp-2");
    }

    #[test]
    fn test_thread_index_remove_nonexistent() {
        let mut index = ThreadIndex::new();

        index.add(CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "t".to_string(),
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        });

        let removed = index.remove("cp-nonexistent");
        assert!(!removed);
        assert_eq!(index.checkpoints.len(), 1);
    }

    #[test]
    fn test_thread_index_remove_from_empty() {
        let mut index = ThreadIndex::new();
        let removed = index.remove("any-id");
        assert!(!removed);
    }

    #[test]
    fn test_thread_index_get_latest_empty() {
        let index = ThreadIndex::new();
        assert!(index.get_latest().is_none());
    }

    #[test]
    fn test_thread_index_serialization() {
        let mut index = ThreadIndex::new();
        index.add(CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            node: "node1".to_string(),
            timestamp: SystemTime::now(),
            parent_id: Some("parent-1".to_string()),
            metadata: std::collections::HashMap::new(),
        });

        let json = serde_json::to_string(&index).unwrap();
        let deserialized: ThreadIndex = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.checkpoints.len(), 1);
        assert_eq!(deserialized.checkpoints[0].id, "cp-1");
        assert_eq!(deserialized.checkpoints[0].parent_id, Some("parent-1".to_string()));
    }

    #[test]
    fn test_thread_index_serialization_empty() {
        let index = ThreadIndex::new();
        let json = serde_json::to_string(&index).unwrap();
        let deserialized: ThreadIndex = serde_json::from_str(&json).unwrap();
        assert!(deserialized.checkpoints.is_empty());
    }

    #[test]
    fn test_thread_index_serialization_large() {
        let mut index = ThreadIndex::new();
        for i in 0..100 {
            index.add(CheckpointMetadata {
                id: format!("cp-{}", i),
                thread_id: "thread-1".to_string(),
                node: format!("node{}", i),
                timestamp: SystemTime::now(),
                parent_id: if i > 0 { Some(format!("cp-{}", i - 1)) } else { None },
                metadata: std::collections::HashMap::new(),
            });
        }

        let json = serde_json::to_string(&index).unwrap();
        let deserialized: ThreadIndex = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.checkpoints.len(), 100);
    }

    // ==========================================================================
    // Unit Tests - StoredCheckpoint Serialization
    // ==========================================================================

    #[test]
    fn test_stored_checkpoint_serialization_bincode() {
        let state = TestState { value: 42 };
        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();
        let deserialized: StoredCheckpoint<TestState> = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.checkpoint.state.value, 42);
        assert_eq!(deserialized.checkpoint.node, "node1");
        assert_eq!(deserialized.checkpoint.thread_id, "thread-1");
    }

    #[test]
    fn test_stored_checkpoint_serialization_with_parent() {
        let state = TestState { value: 100 };
        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node2".to_string(),
            Some("parent-checkpoint-id".to_string()),
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();
        let deserialized: StoredCheckpoint<TestState> = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.checkpoint.parent_id, Some("parent-checkpoint-id".to_string()));
    }

    #[test]
    fn test_stored_checkpoint_serialization_with_metadata() {
        let state = TestState { value: 50 };
        let mut checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node3".to_string(),
            None,
        );
        checkpoint.metadata.insert("key1".to_string(), "value1".to_string());
        checkpoint.metadata.insert("key2".to_string(), "value2".to_string());

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();
        let deserialized: StoredCheckpoint<TestState> = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.checkpoint.metadata.len(), 2);
        assert_eq!(deserialized.checkpoint.metadata.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_stored_checkpoint_size_estimate() {
        let state = TestState { value: 42 };
        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();

        // Should be reasonably small for a simple checkpoint
        assert!(bytes.len() < 1024, "Checkpoint serialization too large: {} bytes", bytes.len());
    }

    // ==========================================================================
    // Unit Tests - Constants
    // ==========================================================================

    #[test]
    fn test_multipart_threshold_is_5mb() {
        assert_eq!(MULTIPART_THRESHOLD, 5 * 1024 * 1024);
    }

    #[test]
    fn test_multipart_chunk_size_is_5mb() {
        assert_eq!(MULTIPART_CHUNK_SIZE, 5 * 1024 * 1024);
    }

    #[test]
    fn test_multipart_chunk_size_matches_threshold() {
        // AWS requires minimum 5MB per part
        assert_eq!(MULTIPART_CHUNK_SIZE, MULTIPART_THRESHOLD);
    }

    // ==========================================================================
    // Unit Tests - CheckpointMetadata
    // ==========================================================================

    #[test]
    fn test_checkpoint_metadata_serialization() {
        let metadata = CheckpointMetadata {
            id: "cp-123".to_string(),
            thread_id: "thread-456".to_string(),
            node: "process_data".to_string(),
            timestamp: SystemTime::now(),
            parent_id: Some("cp-122".to_string()),
            metadata: std::collections::HashMap::new(),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: CheckpointMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "cp-123");
        assert_eq!(deserialized.thread_id, "thread-456");
        assert_eq!(deserialized.node, "process_data");
        assert_eq!(deserialized.parent_id, Some("cp-122".to_string()));
    }

    #[test]
    fn test_checkpoint_metadata_with_custom_metadata() {
        let mut custom_metadata = std::collections::HashMap::new();
        custom_metadata.insert("user_id".to_string(), "user-123".to_string());
        custom_metadata.insert("session_id".to_string(), "sess-456".to_string());

        let metadata = CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "t-1".to_string(),
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: custom_metadata,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: CheckpointMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.metadata.len(), 2);
        assert_eq!(deserialized.metadata.get("user_id"), Some(&"user-123".to_string()));
    }

    // ==========================================================================
    // Unit Tests - Large State Serialization
    // ==========================================================================

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct LargeState {
        data: Vec<u8>,
        name: String,
    }

    #[test]
    fn test_stored_checkpoint_large_state() {
        let large_data = vec![0u8; 1024 * 1024]; // 1 MB
        let state = LargeState {
            data: large_data,
            name: "large-state".to_string(),
        };

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();

        // Should be around 1 MB
        assert!(bytes.len() > 1024 * 1024 - 1024);
        assert!(bytes.len() < 1024 * 1024 + 10240);

        let deserialized: StoredCheckpoint<LargeState> = bincode::deserialize(&bytes).unwrap();
        assert_eq!(deserialized.checkpoint.state.data.len(), 1024 * 1024);
    }

    #[test]
    fn test_stored_checkpoint_multipart_threshold_determination() {
        // Test that we can determine if a checkpoint needs multipart upload
        let small_data = vec![0u8; 1024]; // 1 KB
        let small_state = LargeState {
            data: small_data,
            name: "small".to_string(),
        };

        let small_checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            small_state,
            "node1".to_string(),
            None,
        );

        let small_stored = StoredCheckpoint { checkpoint: small_checkpoint };
        let small_bytes = bincode::serialize(&small_stored).unwrap();

        assert!(small_bytes.len() < MULTIPART_THRESHOLD, "Small checkpoint should not need multipart");

        // Large state that exceeds threshold
        let large_data = vec![0u8; 6 * 1024 * 1024]; // 6 MB
        let large_state = LargeState {
            data: large_data,
            name: "large".to_string(),
        };

        let large_checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            large_state,
            "node1".to_string(),
            None,
        );

        let large_stored = StoredCheckpoint { checkpoint: large_checkpoint };
        let large_bytes = bincode::serialize(&large_stored).unwrap();

        assert!(large_bytes.len() > MULTIPART_THRESHOLD, "Large checkpoint should need multipart");
    }

    // ==========================================================================
    // Unit Tests - Complex State Types
    // ==========================================================================

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct NestedState {
        inner: InnerState,
        items: Vec<String>,
        count: u64,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct InnerState {
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_stored_checkpoint_nested_state() {
        let state = NestedState {
            inner: InnerState {
                name: "inner-name".to_string(),
                values: vec![1, 2, 3, 4, 5],
            },
            items: vec!["item1".to_string(), "item2".to_string()],
            count: 100,
        };

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();
        let deserialized: StoredCheckpoint<NestedState> = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.checkpoint.state.inner.name, "inner-name");
        assert_eq!(deserialized.checkpoint.state.inner.values, vec![1, 2, 3, 4, 5]);
        assert_eq!(deserialized.checkpoint.state.items.len(), 2);
        assert_eq!(deserialized.checkpoint.state.count, 100);
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct StateWithOptions {
        required: String,
        optional: Option<String>,
        optional_number: Option<i64>,
    }

    #[test]
    fn test_stored_checkpoint_optional_fields() {
        let state = StateWithOptions {
            required: "required-value".to_string(),
            optional: Some("optional-value".to_string()),
            optional_number: None,
        };

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();
        let deserialized: StoredCheckpoint<StateWithOptions> = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.checkpoint.state.required, "required-value");
        assert_eq!(deserialized.checkpoint.state.optional, Some("optional-value".to_string()));
        assert_eq!(deserialized.checkpoint.state.optional_number, None);
    }

    // ==========================================================================
    // Unit Tests - Unicode and Special Characters
    // ==========================================================================

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct UnicodeState {
        text: String,
    }

    #[test]
    fn test_stored_checkpoint_unicode_content() {
        let state = UnicodeState {
            text: "Hello 世界! 🌍 Привет мир!".to_string(),
        };

        let checkpoint = Checkpoint::new(
            "thread-日本語".to_string(),
            state,
            "ノード".to_string(),
            None,
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();
        let deserialized: StoredCheckpoint<UnicodeState> = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.checkpoint.state.text, "Hello 世界! 🌍 Привет мир!");
        assert_eq!(deserialized.checkpoint.thread_id, "thread-日本語");
        assert_eq!(deserialized.checkpoint.node, "ノード");
    }

    #[test]
    fn test_thread_index_unicode_ids() {
        let mut index = ThreadIndex::new();
        index.add(CheckpointMetadata {
            id: "检查点-1".to_string(),
            thread_id: "线程-1".to_string(),
            node: "节点".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        });

        let json = serde_json::to_string(&index).unwrap();
        let deserialized: ThreadIndex = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.checkpoints[0].id, "检查点-1");
        assert_eq!(deserialized.checkpoints[0].thread_id, "线程-1");
    }

    // ==========================================================================
    // Unit Tests - Edge Cases
    // ==========================================================================

    #[test]
    fn test_stored_checkpoint_empty_state() {
        #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
        struct EmptyState {}

        let state = EmptyState {};
        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        let stored = StoredCheckpoint { checkpoint };
        let bytes = bincode::serialize(&stored).unwrap();
        let _deserialized: StoredCheckpoint<EmptyState> = bincode::deserialize(&bytes).unwrap();
    }

    #[test]
    fn test_thread_index_remove_all() {
        let mut index = ThreadIndex::new();

        for i in 0..5 {
            index.add(CheckpointMetadata {
                id: format!("cp-{}", i),
                thread_id: "t".to_string(),
                node: "n".to_string(),
                timestamp: SystemTime::now(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
            });
        }

        for i in 0..5 {
            index.remove(&format!("cp-{}", i));
        }

        assert!(index.checkpoints.is_empty());
        assert!(index.get_latest().is_none());
    }

    #[test]
    fn test_checkpoint_id_format() {
        let state = TestState { value: 1 };
        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state,
            "node1".to_string(),
            None,
        );

        // Checkpoint ID should be a valid UUID format
        assert!(!checkpoint.id.is_empty());
        assert!(checkpoint.id.len() > 30); // UUIDs are typically 36 chars
    }

    // ==========================================================================
    // Integration Tests (require AWS credentials and S3 bucket)
    // ==========================================================================

    // These tests require AWS credentials and an S3 bucket
    // To run: AWS_PROFILE=your-profile cargo test -- --ignored
    // Or set AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION

    #[tokio::test]
    #[ignore = "requires AWS credentials and S3 bucket"]
    async fn test_s3_checkpointer_save_and_load() {
        let bucket = std::env::var("TEST_S3_BUCKET")
            .unwrap_or_else(|_| "dashflow-test-checkpoints".to_string());
        let checkpointer = S3Checkpointer::<TestState>::with_prefix(&bucket, "test")
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
    #[ignore = "requires AWS credentials and S3 bucket"]
    async fn test_s3_checkpointer_get_latest() {
        let bucket = std::env::var("TEST_S3_BUCKET")
            .unwrap_or_else(|_| "dashflow-test-checkpoints".to_string());
        let checkpointer = S3Checkpointer::<TestState>::with_prefix(&bucket, "test")
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
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

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
    #[ignore = "requires AWS credentials and S3 bucket"]
    async fn test_s3_checkpointer_list() {
        let bucket = std::env::var("TEST_S3_BUCKET")
            .unwrap_or_else(|_| "dashflow-test-checkpoints".to_string());
        let checkpointer = S3Checkpointer::<TestState>::with_prefix(&bucket, "test")
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
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
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
    #[ignore = "requires AWS credentials and S3 bucket"]
    async fn test_s3_checkpointer_delete() {
        let bucket = std::env::var("TEST_S3_BUCKET")
            .unwrap_or_else(|_| "dashflow-test-checkpoints".to_string());
        let checkpointer = S3Checkpointer::<TestState>::with_prefix(&bucket, "test")
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
    #[ignore = "requires AWS credentials and S3 bucket"]
    async fn test_s3_checkpointer_delete_thread() {
        let bucket = std::env::var("TEST_S3_BUCKET")
            .unwrap_or_else(|_| "dashflow-test-checkpoints".to_string());
        let checkpointer = S3Checkpointer::<TestState>::with_prefix(&bucket, "test")
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
}
