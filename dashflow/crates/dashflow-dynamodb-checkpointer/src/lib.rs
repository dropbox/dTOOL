// NOTE: needless_pass_by_value was removed - no unnecessary pass-by-value occurs

//! `DynamoDB` checkpointer for `DashFlow`
//!
//! Provides persistent checkpoint storage using AWS `DynamoDB`.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_dynamodb_checkpointer::DynamoDBCheckpointer;
//! use dashflow::{StateGraph, GraphState};
//! use serde::{Deserialize, Serialize};
//! use aws_config::BehaviorVersion;
//!
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MyState {
//!     value: i32,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure AWS SDK
//!     let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
//!     let client = aws_sdk_dynamodb::Client::new(&config);
//!
//!     // Create DynamoDB checkpointer
//!     let checkpointer = DynamoDBCheckpointer::new()
//!         .with_table_name("dashflow-checkpoints")
//!         .with_dynamodb_client(client);
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
//! - [`dashflow-s3-checkpointer`](https://docs.rs/dashflow-s3-checkpointer) - Alternative: S3-based checkpointing (AWS)
//! - [`dashflow-postgres-checkpointer`](https://docs.rs/dashflow-postgres-checkpointer) - Alternative: PostgreSQL-based checkpointing
//! - [AWS DynamoDB Documentation](https://docs.aws.amazon.com/dynamodb/) - Official DynamoDB docs

use aws_sdk_dynamodb::{
    types::{AttributeValue, WriteRequest},
    Client as DynamoDBClient,
};
use dashflow::{
    checkpointer_helpers::{nanos_to_timestamp, timestamp_to_nanos},
    Checkpoint, CheckpointMetadata, Checkpointer, GraphState, Result as DashFlowResult,
    RetentionPolicy,
};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::SystemTime;
use tracing::{debug, error};

#[cfg(feature = "compression")]
use dashflow_compression::CompressionType;

/// Errors that can occur when using `DynamoDB` checkpointer
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DynamoDBCheckpointerError {
    #[error("DynamoDB error: {0}")]
    DynamoDBError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[cfg(feature = "compression")]
    #[error("Compression error: {0}")]
    CompressionError(String),
}

impl From<DynamoDBCheckpointerError> for dashflow::Error {
    fn from(err: DynamoDBCheckpointerError) -> Self {
        use dashflow::error::CheckpointError;
        let checkpoint_err = match err {
            DynamoDBCheckpointerError::DynamoDBError(msg) => CheckpointError::ConnectionLost {
                backend: "dynamodb".to_string(),
                reason: msg,
            },
            DynamoDBCheckpointerError::SerializationError(msg) => {
                CheckpointError::SerializationFailed { reason: msg }
            }
            DynamoDBCheckpointerError::DeserializationError(msg) => {
                CheckpointError::DeserializationFailed { reason: msg }
            }
            DynamoDBCheckpointerError::ConfigurationError(msg) => {
                CheckpointError::Other(format!("Configuration error: {}", msg))
            }
            #[cfg(feature = "compression")]
            DynamoDBCheckpointerError::CompressionError(msg) => {
                CheckpointError::Other(format!("Compression error: {}", msg))
            }
        };
        dashflow::Error::Checkpoint(checkpoint_err)
    }
}

/// DynamoDB-backed checkpointer
///
/// Stores checkpoints in `DynamoDB` with the following table schema:
/// - Partition Key: `thread_id` (String)
/// - Sort Key: `checkpoint_id` (String)
/// - Attributes:
///   - `state` (Binary) - Bincode-encoded state
///   - `node` (String) - Node name
///   - `timestamp` (Number) - Unix timestamp in nanoseconds
///   - `parent_id` (String) - Parent checkpoint ID (optional)
///   - `metadata` (Map) - Custom metadata
///   - `ttl` (Number) - TTL expiration timestamp (optional)
///
/// # Table Creation
///
/// Before using this checkpointer, create a `DynamoDB` table with:
/// ```bash
/// aws dynamodb create-table \
///   --table-name dashflow-checkpoints \
///   --attribute-definitions \
///     AttributeName=thread_id,AttributeType=S \
///     AttributeName=checkpoint_id,AttributeType=S \
///   --key-schema \
///     AttributeName=thread_id,KeyType=HASH \
///     AttributeName=checkpoint_id,KeyType=RANGE \
///   --billing-mode PAY_PER_REQUEST
/// ```
#[derive(Clone)]
pub struct DynamoDBCheckpointer<S: GraphState> {
    client: DynamoDBClient,
    table_name: String,
    #[cfg(feature = "compression")]
    compression: Option<CompressionType>,
    retention_policy: Option<RetentionPolicy>,
    _phantom: PhantomData<S>,
}

impl<S: GraphState> DynamoDBCheckpointer<S> {
    /// Create a new `DynamoDB` checkpointer builder
    ///
    /// Must call `with_table_name()` and `with_dynamodb_client()` before use.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: DynamoDBClient::from_conf(aws_sdk_dynamodb::Config::builder().build()),
            table_name: String::new(),
            #[cfg(feature = "compression")]
            compression: None,
            retention_policy: None,
            _phantom: PhantomData,
        }
    }

    /// Set the `DynamoDB` table name
    ///
    /// # Arguments
    /// * `table_name` - Name of the `DynamoDB` table for checkpoints
    pub fn with_table_name(mut self, table_name: impl Into<String>) -> Self {
        self.table_name = table_name.into();
        self
    }

    /// Set the `DynamoDB` client
    ///
    /// # Arguments
    /// * `client` - AWS SDK `DynamoDB` client
    #[must_use]
    pub fn with_dynamodb_client(mut self, client: DynamoDBClient) -> Self {
        self.client = client;
        self
    }

    /// Enable compression for checkpoint storage
    ///
    /// # Arguments
    /// * `compression_type` - The compression algorithm and configuration to use
    #[cfg(feature = "compression")]
    pub fn with_compression(mut self, compression_type: CompressionType) -> Self {
        self.compression = Some(compression_type);
        self
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

    /// Validate configuration before use
    fn validate_config(&self) -> Result<(), DynamoDBCheckpointerError> {
        if self.table_name.is_empty() {
            return Err(DynamoDBCheckpointerError::ConfigurationError(
                "Table name must be set with with_table_name()".to_string(),
            ));
        }
        Ok(())
    }

    /// Helper to deserialize a checkpoint from `DynamoDB` item
    ///
    /// # Arguments
    /// * `item` - `DynamoDB` item (`HashMap` of `AttributeValue`)
    ///
    /// # Returns
    /// Deserialized checkpoint or error
    fn deserialize_checkpoint(
        &self,
        item: &HashMap<String, AttributeValue>,
    ) -> Result<Checkpoint<S>, DynamoDBCheckpointerError> {
        let checkpoint_id = item
            .get("checkpoint_id")
            .and_then(|v| v.as_s().ok())
            .ok_or_else(|| {
                DynamoDBCheckpointerError::DeserializationError(
                    "Missing checkpoint_id field".to_string(),
                )
            })?
            .clone();

        let thread_id = item
            .get("thread_id")
            .and_then(|v| v.as_s().ok())
            .ok_or_else(|| {
                DynamoDBCheckpointerError::DeserializationError(
                    "Missing thread_id field".to_string(),
                )
            })?
            .clone();

        #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
        let mut state_bytes = item
            .get("state")
            .and_then(|v| v.as_b().ok())
            .ok_or_else(|| {
                DynamoDBCheckpointerError::DeserializationError("Missing state field".to_string())
            })?
            .as_ref()
            .to_vec();

        // Decompress if compression is enabled
        #[cfg(feature = "compression")]
        if let Some(compression_type) = self.compression {
            let compressor = compression_type.build().map_err(|e| {
                error!("Failed to build compressor: {}", e);
                DynamoDBCheckpointerError::DeserializationError(format!("Compression error: {}", e))
            })?;
            state_bytes = compressor.decompress(&state_bytes).map_err(|e| {
                error!("Failed to decompress checkpoint state: {}", e);
                DynamoDBCheckpointerError::DeserializationError(format!(
                    "Decompression error: {}",
                    e
                ))
            })?;
            debug!("Decompressed state to {} bytes", state_bytes.len());
        }

        let state: S = bincode::deserialize(&state_bytes).map_err(|e| {
            DynamoDBCheckpointerError::DeserializationError(format!(
                "Failed to deserialize state: {e}"
            ))
        })?;

        let node = item
            .get("node")
            .and_then(|v| v.as_s().ok())
            .ok_or_else(|| {
                DynamoDBCheckpointerError::DeserializationError("Missing node field".to_string())
            })?
            .clone();

        let timestamp_nanos = item
            .get("timestamp")
            .and_then(|v| v.as_n().ok())
            .ok_or_else(|| {
                DynamoDBCheckpointerError::DeserializationError(
                    "Missing timestamp field".to_string(),
                )
            })?
            .parse::<i64>()
            .map_err(|e| {
                DynamoDBCheckpointerError::DeserializationError(format!("Invalid timestamp: {e}"))
            })?;

        let timestamp = nanos_to_timestamp(timestamp_nanos);

        let parent_id = item
            .get("parent_id")
            .and_then(|v| v.as_s().ok())
            .filter(|s| !s.is_empty())
            .cloned();

        let metadata = item
            .get("metadata")
            .and_then(|v| v.as_m().ok())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_s().ok().map(|s| (k.clone(), s.clone())))
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

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

impl<S: GraphState> Default for DynamoDBCheckpointer<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for DynamoDBCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> DashFlowResult<()> {
        self.validate_config()?;

        // Serialize state with bincode
        #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
        let mut state_bytes = bincode::serialize(&checkpoint.state)
            .map_err(|e| dashflow::Error::Generic(format!("Failed to serialize state: {e}")))?;

        // Apply compression if enabled
        #[cfg(feature = "compression")]
        if let Some(compression_type) = self.compression {
            let compressor = compression_type.build().map_err(|e| {
                error!("Failed to build compressor: {}", e);
                dashflow::Error::Generic(format!("Compression error: {}", e))
            })?;
            state_bytes = compressor.compress(&state_bytes).map_err(|e| {
                error!("Failed to compress checkpoint state: {}", e);
                dashflow::Error::Generic(format!("Compression error: {}", e))
            })?;
            debug!("Compressed state to {} bytes", state_bytes.len());
        }

        let timestamp_nanos = timestamp_to_nanos(checkpoint.timestamp);

        // Build DynamoDB item
        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S(checkpoint.id.clone()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S(checkpoint.thread_id.clone()),
        );
        item.insert("state".to_string(), AttributeValue::B(state_bytes.into()));
        item.insert(
            "node".to_string(),
            AttributeValue::S(checkpoint.node.clone()),
        );
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N(timestamp_nanos.to_string()),
        );

        if let Some(parent_id) = &checkpoint.parent_id {
            item.insert(
                "parent_id".to_string(),
                AttributeValue::S(parent_id.clone()),
            );
        }

        if !checkpoint.metadata.is_empty() {
            let metadata_map: HashMap<String, AttributeValue> = checkpoint
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), AttributeValue::S(v.clone())))
                .collect();
            item.insert("metadata".to_string(), AttributeValue::M(metadata_map));
        }

        // Put item to DynamoDB
        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to save checkpoint to DynamoDB: {}", e);
                dashflow::Error::Generic(format!("Failed to save checkpoint: {e}"))
            })?;

        debug!(
            "Saved checkpoint {} for thread {}",
            checkpoint.id, checkpoint.thread_id
        );

        Ok(())
    }

    async fn load(&self, checkpoint_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        self.validate_config()?;

        // We need to scan for checkpoint_id since we don't have the thread_id
        // This is less efficient but necessary for the load() API
        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .filter_expression("checkpoint_id = :checkpoint_id")
            .expression_attribute_values(
                ":checkpoint_id",
                AttributeValue::S(checkpoint_id.to_string()),
            )
            .send()
            .await
            .map_err(|e| {
                error!("Failed to load checkpoint from DynamoDB: {}", e);
                dashflow::Error::Generic(format!("Failed to load checkpoint: {e}"))
            })?;

        if let Some(items) = result.items {
            if let Some(item) = items.into_iter().next() {
                let checkpoint = self.deserialize_checkpoint(&item).map_err(|e| {
                    dashflow::Error::Generic(format!("Failed to deserialize checkpoint: {e}"))
                })?;

                debug!("Loaded checkpoint {}", checkpoint_id);
                return Ok(Some(checkpoint));
            }
        }

        Ok(None)
    }

    async fn get_latest(&self, thread_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        self.validate_config()?;

        // Query with partition key and sort descending by sort key to get latest
        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("thread_id = :thread_id")
            .expression_attribute_values(":thread_id", AttributeValue::S(thread_id.to_string()))
            .scan_index_forward(false) // Descending order (newest first)
            .limit(1)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to get latest checkpoint from DynamoDB: {}", e);
                dashflow::Error::Generic(format!("Failed to get latest checkpoint: {e}"))
            })?;

        if let Some(items) = result.items {
            if let Some(item) = items.into_iter().next() {
                let checkpoint = self.deserialize_checkpoint(&item).map_err(|e| {
                    dashflow::Error::Generic(format!("Failed to deserialize checkpoint: {e}"))
                })?;

                debug!("Got latest checkpoint for thread {}", thread_id);
                return Ok(Some(checkpoint));
            }
        }

        Ok(None)
    }

    async fn list(&self, thread_id: &str) -> DashFlowResult<Vec<CheckpointMetadata>> {
        self.validate_config()?;

        // Query all checkpoints for this thread
        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("thread_id = :thread_id")
            .expression_attribute_values(":thread_id", AttributeValue::S(thread_id.to_string()))
            .scan_index_forward(false) // Descending order (newest first)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to list checkpoints from DynamoDB: {}", e);
                dashflow::Error::Generic(format!("Failed to list checkpoints: {e}"))
            })?;

        let mut metadatas = Vec::new();

        if let Some(items) = result.items {
            for item in items {
                let checkpoint_id = item
                    .get("checkpoint_id")
                    .and_then(|v| v.as_s().ok())
                    .ok_or_else(|| dashflow::Error::Generic("Missing checkpoint_id".to_string()))?
                    .clone();

                let thread_id_val = item
                    .get("thread_id")
                    .and_then(|v| v.as_s().ok())
                    .ok_or_else(|| dashflow::Error::Generic("Missing thread_id".to_string()))?
                    .clone();

                let node = item
                    .get("node")
                    .and_then(|v| v.as_s().ok())
                    .ok_or_else(|| dashflow::Error::Generic("Missing node".to_string()))?
                    .clone();

                let timestamp_nanos = item
                    .get("timestamp")
                    .and_then(|v| v.as_n().ok())
                    .ok_or_else(|| dashflow::Error::Generic("Missing timestamp".to_string()))?
                    .parse::<i64>()
                    .map_err(|e| dashflow::Error::Generic(format!("Invalid timestamp: {e}")))?;

                let timestamp = nanos_to_timestamp(timestamp_nanos);

                let parent_id = item
                    .get("parent_id")
                    .and_then(|v| v.as_s().ok())
                    .filter(|s| !s.is_empty())
                    .cloned();

                let metadata = item
                    .get("metadata")
                    .and_then(|v| v.as_m().ok())
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| v.as_s().ok().map(|s| (k.clone(), s.clone())))
                            .collect::<HashMap<String, String>>()
                    })
                    .unwrap_or_default();

                metadatas.push(CheckpointMetadata {
                    id: checkpoint_id,
                    thread_id: thread_id_val,
                    node,
                    timestamp,
                    parent_id,
                    metadata,
                });
            }
        }

        debug!(
            "Listed {} checkpoints for thread {}",
            metadatas.len(),
            thread_id
        );

        Ok(metadatas)
    }

    async fn delete(&self, checkpoint_id: &str) -> DashFlowResult<()> {
        self.validate_config()?;

        // First, scan to find the thread_id for this checkpoint
        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .filter_expression("checkpoint_id = :checkpoint_id")
            .expression_attribute_values(
                ":checkpoint_id",
                AttributeValue::S(checkpoint_id.to_string()),
            )
            .send()
            .await
            .map_err(|e| {
                error!("Failed to find checkpoint for deletion: {}", e);
                dashflow::Error::Generic(format!("Failed to find checkpoint: {e}"))
            })?;

        if let Some(items) = result.items {
            if let Some(item) = items.into_iter().next() {
                let thread_id = item
                    .get("thread_id")
                    .and_then(|v| v.as_s().ok())
                    .ok_or_else(|| dashflow::Error::Generic("Missing thread_id".to_string()))?;

                // Delete the item using partition key + sort key
                self.client
                    .delete_item()
                    .table_name(&self.table_name)
                    .key("thread_id", AttributeValue::S(thread_id.clone()))
                    .key(
                        "checkpoint_id",
                        AttributeValue::S(checkpoint_id.to_string()),
                    )
                    .send()
                    .await
                    .map_err(|e| {
                        error!("Failed to delete checkpoint from DynamoDB: {}", e);
                        dashflow::Error::Generic(format!("Failed to delete checkpoint: {e}"))
                    })?;

                debug!("Deleted checkpoint {}", checkpoint_id);
            }
        }

        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> DashFlowResult<()> {
        self.validate_config()?;

        // Query all checkpoints for this thread
        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("thread_id = :thread_id")
            .expression_attribute_values(":thread_id", AttributeValue::S(thread_id.to_string()))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to query checkpoints for deletion: {}", e);
                dashflow::Error::Generic(format!("Failed to query checkpoints: {e}"))
            })?;

        if let Some(items) = result.items {
            // DynamoDB BatchWriteItem can handle up to 25 items per request
            for chunk in items.chunks(25) {
                let write_requests: Vec<WriteRequest> = chunk
                    .iter()
                    .filter_map(|item| {
                        let checkpoint_id = item.get("checkpoint_id")?.as_s().ok()?.clone();
                        Some(
                            WriteRequest::builder()
                                .delete_request(
                                    aws_sdk_dynamodb::types::DeleteRequest::builder()
                                        .key("thread_id", AttributeValue::S(thread_id.to_string()))
                                        .key("checkpoint_id", AttributeValue::S(checkpoint_id))
                                        .build()
                                        .ok()?,
                                )
                                .build(),
                        )
                    })
                    .collect();

                if !write_requests.is_empty() {
                    self.client
                        .batch_write_item()
                        .request_items(&self.table_name, write_requests)
                        .send()
                        .await
                        .map_err(|e| {
                            error!("Failed to batch delete checkpoints: {}", e);
                            dashflow::Error::Generic(format!("Failed to delete checkpoints: {e}"))
                        })?;
                }
            }
        }

        debug!("Deleted all checkpoints for thread {}", thread_id);

        Ok(())
    }

    async fn list_threads(&self) -> DashFlowResult<Vec<dashflow::ThreadInfo>> {
        self.validate_config()?;

        // Scan all items and group by thread_id to find latest checkpoint per thread
        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .projection_expression("thread_id, checkpoint_id, #ts")
            .expression_attribute_names("#ts", "timestamp")
            .send()
            .await
            .map_err(|e| {
                error!("Failed to scan checkpoints from DynamoDB: {}", e);
                dashflow::Error::Generic(format!("Failed to scan checkpoints: {e}"))
            })?;

        // Group by thread_id and find latest checkpoint for each
        let mut thread_map: HashMap<String, (String, std::time::SystemTime)> = HashMap::new();

        if let Some(items) = result.items {
            for item in items {
                let thread_id = match item.get("thread_id").and_then(|v| v.as_s().ok()) {
                    Some(id) => id.clone(),
                    None => continue,
                };
                let checkpoint_id = match item.get("checkpoint_id").and_then(|v| v.as_s().ok()) {
                    Some(id) => id.clone(),
                    None => continue,
                };
                let timestamp_nanos = match item
                    .get("timestamp")
                    .and_then(|v| v.as_n().ok())
                    .and_then(|s| s.parse::<i64>().ok())
                {
                    Some(ts) => ts,
                    None => continue,
                };
                let timestamp = nanos_to_timestamp(timestamp_nanos);

                // Keep the most recent checkpoint for each thread
                thread_map
                    .entry(thread_id)
                    .and_modify(|(existing_id, existing_ts)| {
                        if timestamp > *existing_ts {
                            *existing_id = checkpoint_id.clone();
                            *existing_ts = timestamp;
                        }
                    })
                    .or_insert((checkpoint_id, timestamp));
            }
        }

        let mut thread_infos: Vec<dashflow::ThreadInfo> = thread_map
            .into_iter()
            .map(
                |(thread_id, (checkpoint_id, timestamp))| dashflow::ThreadInfo {
                    thread_id,
                    latest_checkpoint_id: checkpoint_id,
                    updated_at: timestamp,
                    checkpoint_count: None,
                },
            )
            .collect();

        // Sort by updated_at DESC (most recent first)
        thread_infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        debug!("Listed {} threads", thread_infos.len());
        Ok(thread_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_dynamodb::primitives::Blob;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }

    // ========================================================================
    // Error Type Tests
    // ========================================================================

    #[test]
    fn test_error_dynamodb_error_display() {
        let err = DynamoDBCheckpointerError::DynamoDBError("Connection failed".to_string());
        assert_eq!(err.to_string(), "DynamoDB error: Connection failed");
    }

    #[test]
    fn test_error_serialization_error_display() {
        let err = DynamoDBCheckpointerError::SerializationError("Invalid bytes".to_string());
        assert_eq!(err.to_string(), "Serialization error: Invalid bytes");
    }

    #[test]
    fn test_error_deserialization_error_display() {
        let err = DynamoDBCheckpointerError::DeserializationError("Corrupt data".to_string());
        assert_eq!(err.to_string(), "Deserialization error: Corrupt data");
    }

    #[test]
    fn test_error_configuration_error_display() {
        let err = DynamoDBCheckpointerError::ConfigurationError("Missing table name".to_string());
        assert_eq!(err.to_string(), "Configuration error: Missing table name");
    }

    #[test]
    fn test_error_dynamodb_to_dashflow_error() {
        let err = DynamoDBCheckpointerError::DynamoDBError("Network timeout".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let err_str = dashflow_err.to_string();
        assert!(err_str.contains("dynamodb") || err_str.contains("Network timeout"));
    }

    #[test]
    fn test_error_serialization_to_dashflow_error() {
        let err = DynamoDBCheckpointerError::SerializationError("Failed to encode".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let err_str = dashflow_err.to_string();
        assert!(err_str.contains("Failed to encode") || err_str.contains("Serialization"));
    }

    #[test]
    fn test_error_deserialization_to_dashflow_error() {
        let err = DynamoDBCheckpointerError::DeserializationError("Invalid format".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let err_str = dashflow_err.to_string();
        assert!(err_str.contains("Invalid format") || err_str.contains("Deserialization"));
    }

    #[test]
    fn test_error_configuration_to_dashflow_error() {
        let err = DynamoDBCheckpointerError::ConfigurationError("Bad config".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let err_str = dashflow_err.to_string();
        assert!(err_str.contains("Bad config") || err_str.contains("Configuration"));
    }

    // ========================================================================
    // Builder Pattern Tests
    // ========================================================================

    #[test]
    fn test_builder_new_creates_default_state() {
        let checkpointer = DynamoDBCheckpointer::<TestState>::new();
        assert!(checkpointer.table_name.is_empty());
        assert!(checkpointer.retention_policy.is_none());
    }

    #[test]
    fn test_builder_default_same_as_new() {
        let from_new = DynamoDBCheckpointer::<TestState>::new();
        let from_default = DynamoDBCheckpointer::<TestState>::default();
        assert_eq!(from_new.table_name, from_default.table_name);
        assert!(from_new.retention_policy.is_none());
        assert!(from_default.retention_policy.is_none());
    }

    #[test]
    fn test_builder_with_table_name() {
        let checkpointer = DynamoDBCheckpointer::<TestState>::new().with_table_name("my-table");
        assert_eq!(checkpointer.table_name, "my-table");
    }

    #[test]
    fn test_builder_with_table_name_string() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name(String::from("another-table"));
        assert_eq!(checkpointer.table_name, "another-table");
    }

    #[test]
    fn test_builder_with_retention_policy() {
        use dashflow::RetentionPolicy;
        let policy = RetentionPolicy::builder().keep_last_n(5).build();
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_retention_policy(policy);
        assert!(checkpointer.retention_policy.is_some());
    }

    #[test]
    fn test_builder_chaining() {
        use dashflow::RetentionPolicy;
        let policy = RetentionPolicy::builder().keep_last_n(10).build();
        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("chained-table")
            .with_retention_policy(policy);
        assert_eq!(checkpointer.table_name, "chained-table");
        assert!(checkpointer.retention_policy.is_some());
    }

    // ========================================================================
    // Configuration Validation Tests
    // ========================================================================

    #[test]
    fn test_validate_config_empty_table_name_fails() {
        let checkpointer = DynamoDBCheckpointer::<TestState>::new();
        let result = checkpointer.validate_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            DynamoDBCheckpointerError::ConfigurationError(_)
        ));
        assert!(err.to_string().contains("Table name"));
    }

    #[test]
    fn test_validate_config_with_table_name_succeeds() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("valid-table");
        let result = checkpointer.validate_config();
        assert!(result.is_ok());
    }

    // ========================================================================
    // Deserialization Tests (with mock AttributeValues)
    // ========================================================================

    fn create_mock_checkpoint_item(
        checkpoint_id: &str,
        thread_id: &str,
        state: &TestState,
        node: &str,
        timestamp_nanos: i64,
    ) -> HashMap<String, AttributeValue> {
        let state_bytes = bincode::serialize(state).unwrap();
        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S(checkpoint_id.to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S(thread_id.to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert("node".to_string(), AttributeValue::S(node.to_string()));
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N(timestamp_nanos.to_string()),
        );
        item
    }

    #[test]
    fn test_deserialize_checkpoint_basic() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 42 };
        let item = create_mock_checkpoint_item(
            "cp-123",
            "thread-1",
            &state,
            "node-a",
            1_704_067_200_000_000_000, // 2024-01-01 00:00:00 UTC in nanos
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.id, "cp-123");
        assert_eq!(checkpoint.thread_id, "thread-1");
        assert_eq!(checkpoint.state.value, 42);
        assert_eq!(checkpoint.node, "node-a");
        assert!(checkpoint.parent_id.is_none());
        assert!(checkpoint.metadata.is_empty());
    }

    #[test]
    fn test_deserialize_checkpoint_with_parent_id() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 100 };
        let mut item = create_mock_checkpoint_item(
            "cp-456",
            "thread-2",
            &state,
            "node-b",
            1_704_153_600_000_000_000,
        );
        item.insert(
            "parent_id".to_string(),
            AttributeValue::S("cp-parent".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.parent_id, Some("cp-parent".to_string()));
    }

    #[test]
    fn test_deserialize_checkpoint_with_empty_parent_id_is_none() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 200 };
        let mut item = create_mock_checkpoint_item(
            "cp-789",
            "thread-3",
            &state,
            "node-c",
            1_704_240_000_000_000_000,
        );
        // Empty parent_id should be treated as None
        item.insert("parent_id".to_string(), AttributeValue::S(String::new()));

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert!(checkpoint.parent_id.is_none());
    }

    #[test]
    fn test_deserialize_checkpoint_with_metadata() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 300 };
        let mut item = create_mock_checkpoint_item(
            "cp-meta",
            "thread-4",
            &state,
            "node-d",
            1_704_326_400_000_000_000,
        );

        let mut metadata_map = HashMap::new();
        metadata_map.insert("key1".to_string(), AttributeValue::S("value1".to_string()));
        metadata_map.insert("key2".to_string(), AttributeValue::S("value2".to_string()));
        item.insert("metadata".to_string(), AttributeValue::M(metadata_map));

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.metadata.len(), 2);
        assert_eq!(checkpoint.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(checkpoint.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_deserialize_checkpoint_missing_checkpoint_id_fails() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 1 };
        let state_bytes = bincode::serialize(&state).unwrap();
        let mut item = HashMap::new();
        // Missing checkpoint_id
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-x".to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert("node".to_string(), AttributeValue::S("node-x".to_string()));
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            DynamoDBCheckpointerError::DeserializationError(_)
        ));
        assert!(err.to_string().contains("checkpoint_id"));
    }

    #[test]
    fn test_deserialize_checkpoint_missing_thread_id_fails() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 2 };
        let state_bytes = bincode::serialize(&state).unwrap();
        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-x".to_string()),
        );
        // Missing thread_id
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert("node".to_string(), AttributeValue::S("node-x".to_string()));
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("thread_id"));
    }

    #[test]
    fn test_deserialize_checkpoint_missing_state_fails() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-y".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-y".to_string()),
        );
        // Missing state
        item.insert("node".to_string(), AttributeValue::S("node-y".to_string()));
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("state"));
    }

    #[test]
    fn test_deserialize_checkpoint_missing_node_fails() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 3 };
        let state_bytes = bincode::serialize(&state).unwrap();
        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-z".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-z".to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        // Missing node
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("node"));
    }

    #[test]
    fn test_deserialize_checkpoint_missing_timestamp_fails() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 4 };
        let state_bytes = bincode::serialize(&state).unwrap();
        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-w".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-w".to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert("node".to_string(), AttributeValue::S("node-w".to_string()));
        // Missing timestamp

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("timestamp"));
    }

    #[test]
    fn test_deserialize_checkpoint_invalid_timestamp_fails() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 5 };
        let state_bytes = bincode::serialize(&state).unwrap();
        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-inv".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-inv".to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert(
            "node".to_string(),
            AttributeValue::S("node-inv".to_string()),
        );
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("not-a-number".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("timestamp") || err.to_string().contains("Invalid"));
    }

    #[test]
    fn test_deserialize_checkpoint_invalid_state_bytes_fails() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-bad".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-bad".to_string()),
        );
        // Invalid state bytes (not bincode-encoded TestState)
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(vec![0xFF, 0xFE, 0xFD])),
        );
        item.insert(
            "node".to_string(),
            AttributeValue::S("node-bad".to_string()),
        );
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("deserialize") || err.to_string().contains("state"));
    }

    #[test]
    fn test_deserialize_checkpoint_wrong_attribute_type_for_checkpoint_id() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 6 };
        let state_bytes = bincode::serialize(&state).unwrap();
        let mut item = HashMap::new();
        // checkpoint_id as Number instead of String
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::N("123".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-type".to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert(
            "node".to_string(),
            AttributeValue::S("node-type".to_string()),
        );
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_err());
    }

    // ========================================================================
    // Clone Tests
    // ========================================================================

    #[test]
    fn test_checkpointer_is_clone() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("clone-test");
        let cloned = checkpointer.clone();
        assert_eq!(cloned.table_name, "clone-test");
    }

    // ========================================================================
    // State Type Tests (various serializable types)
    // ========================================================================

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct ComplexState {
        name: String,
        count: u64,
        values: Vec<i32>,
        optional: Option<String>,
    }

    #[test]
    fn test_deserialize_checkpoint_complex_state() {
        let checkpointer =
            DynamoDBCheckpointer::<ComplexState>::new().with_table_name("test-table");

        let state = ComplexState {
            name: "test".to_string(),
            count: 1000,
            values: vec![1, 2, 3, 4, 5],
            optional: Some("present".to_string()),
        };
        let state_bytes = bincode::serialize(&state).unwrap();

        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-complex".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-complex".to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert(
            "node".to_string(),
            AttributeValue::S("node-complex".to_string()),
        );
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1704412800000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.state.name, "test");
        assert_eq!(checkpoint.state.count, 1000);
        assert_eq!(checkpoint.state.values, vec![1, 2, 3, 4, 5]);
        assert_eq!(checkpoint.state.optional, Some("present".to_string()));
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct NestedState {
        inner: TestState,
        level: u8,
    }

    #[test]
    fn test_deserialize_checkpoint_nested_state() {
        let checkpointer =
            DynamoDBCheckpointer::<NestedState>::new().with_table_name("test-table");

        let state = NestedState {
            inner: TestState { value: 999 },
            level: 3,
        };
        let state_bytes = bincode::serialize(&state).unwrap();

        let mut item = HashMap::new();
        item.insert(
            "checkpoint_id".to_string(),
            AttributeValue::S("cp-nested".to_string()),
        );
        item.insert(
            "thread_id".to_string(),
            AttributeValue::S("thread-nested".to_string()),
        );
        item.insert(
            "state".to_string(),
            AttributeValue::B(Blob::new(state_bytes)),
        );
        item.insert(
            "node".to_string(),
            AttributeValue::S("node-nested".to_string()),
        );
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N("1704499200000000000".to_string()),
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.state.inner.value, 999);
        assert_eq!(checkpoint.state.level, 3);
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_deserialize_checkpoint_empty_metadata_map() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 50 };
        let mut item = create_mock_checkpoint_item(
            "cp-empty-meta",
            "thread-empty",
            &state,
            "node-empty",
            1_704_585_600_000_000_000,
        );
        // Add empty metadata map
        item.insert("metadata".to_string(), AttributeValue::M(HashMap::new()));

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert!(checkpoint.metadata.is_empty());
    }

    #[test]
    fn test_deserialize_checkpoint_unicode_strings() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 888 };
        let item = create_mock_checkpoint_item(
            "cp-\u{65E5}\u{672C}\u{8A9E}", // cp-日本語
            "thread-\u{4E2D}\u{6587}",     // thread-中文
            &state,
            "node-\u{00E9}moji-\u{1F389}", // node-émoji-🎉
            1_704_672_000_000_000_000,
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.id, "cp-\u{65E5}\u{672C}\u{8A9E}");
        assert_eq!(checkpoint.thread_id, "thread-\u{4E2D}\u{6587}");
        assert_eq!(checkpoint.node, "node-\u{00E9}moji-\u{1F389}");
    }

    #[test]
    fn test_deserialize_checkpoint_very_large_timestamp() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 77 };
        // Use a large timestamp (year 2100+)
        let item = create_mock_checkpoint_item(
            "cp-future",
            "thread-future",
            &state,
            "node-future",
            4_102_444_800_000_000_000, // 2100-01-01 00:00:00 UTC
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deserialize_checkpoint_zero_timestamp() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: 0 };
        let item = create_mock_checkpoint_item(
            "cp-zero",
            "thread-zero",
            &state,
            "node-zero",
            0, // Unix epoch
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deserialize_checkpoint_negative_state_value() {
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("test-table");

        let state = TestState { value: -9999 };
        let item = create_mock_checkpoint_item(
            "cp-neg",
            "thread-neg",
            &state,
            "node-neg",
            1_704_758_400_000_000_000,
        );

        let result = checkpointer.deserialize_checkpoint(&item);
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.state.value, -9999);
    }

    #[test]
    fn test_builder_empty_table_name() {
        // Should be allowed at build time, but fail on validate
        let checkpointer = DynamoDBCheckpointer::<TestState>::new().with_table_name("");
        assert!(checkpointer.table_name.is_empty());
        assert!(checkpointer.validate_config().is_err());
    }

    #[test]
    fn test_builder_special_characters_in_table_name() {
        // DynamoDB allows alphanumeric, underscore, hyphen, dot
        let checkpointer =
            DynamoDBCheckpointer::<TestState>::new().with_table_name("my-table_name.v1");
        assert_eq!(checkpointer.table_name, "my-table_name.v1");
        assert!(checkpointer.validate_config().is_ok());
    }

    // ========================================================================
    // Retention Policy Tests
    // ========================================================================

    #[test]
    fn test_retention_policy_keep_last() {
        use dashflow::RetentionPolicy;
        let policy = RetentionPolicy::builder().keep_last_n(3).build();
        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test")
            .with_retention_policy(policy);
        assert!(checkpointer.retention_policy.is_some());
    }

    #[test]
    fn test_retention_policy_keep_for_duration() {
        use dashflow::RetentionPolicy;
        use std::time::Duration;
        let policy = RetentionPolicy::builder()
            .keep_daily_for(Duration::from_secs(86400))
            .build();
        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test")
            .with_retention_policy(policy);
        assert!(checkpointer.retention_policy.is_some());
    }

    // ========================================================================
    // Integration tests require DynamoDB (LocalStack or real AWS)
    // Run with: cargo test --features test -- --ignored
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_checkpointer_save_and_load() {
        // This test requires LocalStack or real DynamoDB
        // Setup:
        // docker run -d -p 4566:4566 localstack/localstack
        // aws dynamodb create-table --endpoint-url http://localhost:4566 \
        //   --table-name test-checkpoints \
        //   --attribute-definitions AttributeName=thread_id,AttributeType=S \
        //     AttributeName=checkpoint_id,AttributeType=S \
        //   --key-schema AttributeName=thread_id,KeyType=HASH \
        //     AttributeName=checkpoint_id,KeyType=RANGE \
        //   --billing-mode PAY_PER_REQUEST

        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = DynamoDBClient::new(&config);

        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test-checkpoints")
            .with_dynamodb_client(client);

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
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_checkpointer_get_latest() {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = DynamoDBClient::new(&config);

        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test-checkpoints")
            .with_dynamodb_client(client);

        let thread_id = "test_thread_latest".to_string();

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

        // Get latest should return most recent
        let latest = checkpointer.get_latest(&thread_id).await.unwrap();
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.state.value, 2);
        assert_eq!(latest.node, "node2");

        // Cleanup
        checkpointer.delete_thread(&thread_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_checkpointer_list() {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = DynamoDBClient::new(&config);

        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test-checkpoints")
            .with_dynamodb_client(client);

        let thread_id = "test_thread_list".to_string();

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
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_checkpointer_delete() {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = DynamoDBClient::new(&config);

        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test-checkpoints")
            .with_dynamodb_client(client);

        let thread_id = "test_thread_delete".to_string();
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
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_checkpointer_delete_thread() {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = DynamoDBClient::new(&config);

        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test-checkpoints")
            .with_dynamodb_client(client);

        let thread_id = "test_thread_delete_all".to_string();

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
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_checkpointer_list_threads() {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = DynamoDBClient::new(&config);

        let checkpointer = DynamoDBCheckpointer::<TestState>::new()
            .with_table_name("test-checkpoints")
            .with_dynamodb_client(client);

        // Create checkpoints for multiple threads
        let thread_ids = vec!["thread_a", "thread_b", "thread_c"];
        for thread_id in &thread_ids {
            let checkpoint = Checkpoint::new(
                (*thread_id).to_string(),
                TestState { value: 1 },
                "node1".to_string(),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
        }

        // List threads
        let threads = checkpointer.list_threads().await.unwrap();
        assert!(threads.len() >= 3);

        // Cleanup
        for thread_id in &thread_ids {
            checkpointer.delete_thread(thread_id).await.unwrap();
        }
    }
}
