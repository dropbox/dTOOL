// Note: This crate has been audited for unwrap/expect safety.
// Only 1 `.unwrap()` at line 85 (validate_identifier), guarded by an is_empty() check.

//! `PostgreSQL` checkpointer for `DashFlow`
//!
//! Provides persistent checkpoint storage using `PostgreSQL`.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_postgres_checkpointer::PostgresCheckpointer;
//! use dashflow::{StateGraph, GraphState};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MyState {
//!     value: i32,
//! }
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let connection_string = "host=localhost user=postgres password=postgres dbname=dashflow";
//!     let checkpointer = PostgresCheckpointer::new(connection_string).await?;
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
//! - [`dashflow-redis-checkpointer`](https://docs.rs/dashflow-redis-checkpointer) - Alternative: Redis-based checkpointing
//! - [`dashflow-s3-checkpointer`](https://docs.rs/dashflow-s3-checkpointer) - Alternative: S3-based checkpointing for cloud
//! - [PostgreSQL Documentation](https://www.postgresql.org/docs/) - Official PostgreSQL docs

mod error;

use dashflow::{
    checkpointer_helpers::{nanos_to_timestamp, timestamp_to_nanos},
    Checkpoint, CheckpointMetadata, Checkpointer, GraphState, Result as DashFlowResult,
    RetentionPolicy,
};
use std::marker::PhantomData;
use std::time::SystemTime;
use tokio_postgres::{Client, NoTls};
use tracing::{debug, error, info};

#[cfg(feature = "compression")]
use dashflow_compression::{Compression, CompressionType};

/// Validate a PostgreSQL identifier (table name, column name, etc.)
///
/// PostgreSQL identifiers must:
/// - Start with a letter (a-z, A-Z) or underscore
/// - Contain only letters, digits, and underscores
/// - Be at most 63 characters (PostgreSQL limit for unquoted identifiers)
///
/// # Arguments
/// * `name` - The identifier to validate
///
/// # Returns
/// `Ok(())` if valid, `Err(PostgresError::InvalidIdentifier)` if invalid
fn validate_identifier(name: &str) -> Result<(), PostgresError> {
    if name.is_empty() {
        return Err(PostgresError::InvalidIdentifier(
            "identifier cannot be empty".to_string(),
        ));
    }

    if name.len() > 63 {
        return Err(PostgresError::InvalidIdentifier(format!(
            "identifier '{}' exceeds maximum length of 63 characters",
            name
        )));
    }

    let mut chars = name.chars();
    #[allow(clippy::unwrap_used)] // SAFETY: we checked non-empty above
    let first = chars.next().unwrap();

    // First character must be a letter or underscore
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(PostgresError::InvalidIdentifier(format!(
            "identifier '{}' must start with a letter or underscore",
            name
        )));
    }

    // Remaining characters must be letters, digits, or underscores
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(PostgresError::InvalidIdentifier(format!(
                "identifier '{}' contains invalid character '{}'",
                name, c
            )));
        }
    }

    Ok(())
}

/// PostgreSQL-backed checkpointer
///
/// Stores checkpoints in a `PostgreSQL` database table with the following schema:
/// - `checkpoint_id` (TEXT PRIMARY KEY)
/// - `thread_id` (TEXT, indexed)
/// - state (BYTEA, bincode-encoded, optionally compressed)
/// - node (TEXT)
/// - timestamp (BIGINT, Unix timestamp in nanoseconds)
/// - `parent_id` (TEXT, nullable)
/// - metadata (JSONB)
pub struct PostgresCheckpointer<S: GraphState> {
    client: Client,
    table_name: String,
    #[cfg(feature = "compression")]
    compression: Option<Box<dyn Compression>>,
    retention_policy: Option<RetentionPolicy>,
    _phantom: PhantomData<S>,
}

impl<S: GraphState> PostgresCheckpointer<S> {
    /// Create a new `PostgreSQL` checkpointer
    ///
    /// # Arguments
    /// * `connection_string` - `PostgreSQL` connection string (e.g., "host=localhost user=postgres password=postgres dbname=dashflow")
    ///
    /// # Errors
    /// Returns error if connection fails or table creation fails
    pub async fn new(connection_string: &str) -> Result<Self, PostgresError> {
        Self::with_table_name(connection_string, "dashflow_checkpoints").await
    }

    /// Create a new `PostgreSQL` checkpointer with custom table name
    ///
    /// # Arguments
    /// * `connection_string` - `PostgreSQL` connection string
    /// * `table_name` - Name of the table to store checkpoints (must be a valid SQL identifier)
    ///
    /// # Errors
    /// Returns `InvalidIdentifier` error if table_name contains invalid characters
    /// (only letters, digits, and underscores are allowed, must start with letter or underscore)
    pub async fn with_table_name(
        connection_string: &str,
        table_name: &str,
    ) -> Result<Self, PostgresError> {
        // Validate table name to prevent SQL injection
        validate_identifier(table_name)?;

        info!("Connecting to PostgreSQL: {}", connection_string);
        let (client, connection) = tokio_postgres::connect(connection_string, NoTls)
            .await
            .map_err(|e| {
                error!("Failed to connect to PostgreSQL: {}", e);
                PostgresError::ConnectionError(e.to_string())
            })?;

        // Spawn connection task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("PostgreSQL connection error: {}", e);
            }
        });

        let checkpointer = Self {
            client,
            table_name: table_name.to_string(),
            #[cfg(feature = "compression")]
            compression: None,
            retention_policy: None,
            _phantom: PhantomData,
        };

        // Create table if it doesn't exist
        checkpointer.initialize_schema().await?;

        Ok(checkpointer)
    }

    /// Enable compression for checkpoint storage
    ///
    /// # Arguments
    /// * `compression_type` - The compression algorithm and configuration to use
    ///
    /// # Example
    /// ```rust,ignore
    /// use dashflow_compression::CompressionType;
    ///
    /// let checkpointer = PostgresCheckpointer::new(connection_string)
    ///     .await?
    ///     .with_compression(CompressionType::Zstd(3))?;
    /// ```
    #[cfg(feature = "compression")]
    pub fn with_compression(
        mut self,
        compression_type: CompressionType,
    ) -> Result<Self, PostgresError> {
        self.compression = Some(
            compression_type
                .build()
                .map_err(|e| PostgresError::CompressionError(e.to_string()))?,
        );
        Ok(self)
    }

    /// Set retention policy for automatic checkpoint cleanup
    ///
    /// # Arguments
    /// * `policy` - The retention policy to apply
    ///
    /// # Example
    /// ```rust,ignore
    /// use dashflow::RetentionPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetentionPolicy::builder()
    ///     .keep_last_n(10)
    ///     .delete_after(Duration::from_secs(90 * 86400))
    ///     .build();
    ///
    /// let checkpointer = PostgresCheckpointer::new(connection_string)
    ///     .await?
    ///     .with_retention_policy(policy);
    /// ```
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

    /// Initialize the database schema
    async fn initialize_schema(&self) -> Result<(), PostgresError> {
        let create_table_sql = format!(
            r"
            CREATE TABLE IF NOT EXISTS {} (
                checkpoint_id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                state BYTEA NOT NULL,
                node TEXT NOT NULL,
                timestamp BIGINT NOT NULL,
                parent_id TEXT,
                metadata JSONB
            );
            CREATE INDEX IF NOT EXISTS idx_{}_thread_id ON {} (thread_id);
            CREATE INDEX IF NOT EXISTS idx_{}_timestamp ON {} (timestamp);
            ",
            self.table_name, self.table_name, self.table_name, self.table_name, self.table_name
        );

        self.client
            .batch_execute(&create_table_sql)
            .await
            .map_err(|e| {
                error!("Failed to create table: {}", e);
                PostgresError::QueryError(e.to_string())
            })?;

        debug!("PostgreSQL schema initialized: table={}", self.table_name);
        Ok(())
    }

    /// Helper to deserialize a checkpoint from a database row
    ///
    /// # Arguments
    /// * `row` - `PostgreSQL` row containing checkpoint data in standard column order:
    ///   (`checkpoint_id`, `thread_id`, state, node, timestamp, `parent_id`, metadata)
    fn deserialize_checkpoint_from_row(
        &self,
        row: &tokio_postgres::Row,
    ) -> DashFlowResult<Checkpoint<S>> {
        let checkpoint_id: String = row.get(0);
        let thread_id: String = row.get(1);
        #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
        let mut state_bytes: Vec<u8> = row.get(2);
        let node: String = row.get(3);
        let timestamp: i64 = row.get(4);
        let parent_id: Option<String> = row.get(5);
        let metadata_json: serde_json::Value = row.get(6);

        // Decompress if compression is enabled
        #[cfg(feature = "compression")]
        if let Some(ref compressor) = self.compression {
            state_bytes = compressor.decompress(&state_bytes).map_err(|e| {
                error!("Failed to decompress checkpoint state: {}", e);
                dashflow::Error::Generic(format!("Decompression error: {}", e))
            })?;
            debug!("Decompressed state to {} bytes", state_bytes.len());
        }

        let state: S = bincode::deserialize(&state_bytes).map_err(|e| {
            error!("Failed to deserialize checkpoint state: {}", e);
            dashflow::Error::Generic(format!("Deserialization error: {e}"))
        })?;

        let metadata: std::collections::HashMap<String, String> =
            serde_json::from_value(metadata_json).map_err(|e| {
                error!("Failed to deserialize checkpoint metadata: {}", e);
                dashflow::Error::Generic(format!("Metadata deserialization error: {e}"))
            })?;

        Ok(Checkpoint {
            id: checkpoint_id,
            thread_id,
            state,
            node,
            timestamp: nanos_to_timestamp(timestamp),
            parent_id,
            metadata,
        })
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for PostgresCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> DashFlowResult<()> {
        // Serialize state with bincode
        #[cfg_attr(not(feature = "compression"), allow(unused_mut))]
        let mut state_bytes = bincode::serialize(&checkpoint.state).map_err(|e| {
            error!("Failed to serialize checkpoint state: {}", e);
            dashflow::Error::Generic(format!("Serialization error: {e}"))
        })?;
        #[cfg(feature = "compression")]
        let uncompressed_len = state_bytes.len();

        // Apply compression if enabled
        #[cfg(feature = "compression")]
        if let Some(ref compressor) = self.compression {
            state_bytes = compressor.compress(&state_bytes).map_err(|e| {
                error!("Failed to compress checkpoint state: {}", e);
                dashflow::Error::Generic(format!("Compression error: {}", e))
            })?;
            debug!(
                "Compressed state from {} to {} bytes",
                uncompressed_len,
                state_bytes.len()
            );
        }

        // Convert metadata to JSON
        let metadata_json = serde_json::to_value(&checkpoint.metadata).map_err(|e| {
            error!("Failed to serialize checkpoint metadata: {}", e);
            dashflow::Error::Generic(format!("Metadata serialization error: {e}"))
        })?;

        let timestamp = timestamp_to_nanos(checkpoint.timestamp);

        let insert_sql = format!(
            "INSERT INTO {} (checkpoint_id, thread_id, state, node, timestamp, parent_id, metadata)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (checkpoint_id) DO UPDATE SET
                 thread_id = EXCLUDED.thread_id,
                 state = EXCLUDED.state,
                 node = EXCLUDED.node,
                 timestamp = EXCLUDED.timestamp,
                 parent_id = EXCLUDED.parent_id,
                 metadata = EXCLUDED.metadata",
            self.table_name
        );

        self.client
            .execute(
                &insert_sql,
                &[
                    &checkpoint.id,
                    &checkpoint.thread_id,
                    &state_bytes,
                    &checkpoint.node,
                    &timestamp,
                    &checkpoint.parent_id,
                    &metadata_json,
                ],
            )
            .await
            .map_err(|e| {
                error!("Failed to save checkpoint: {}", e);
                dashflow::Error::Generic(format!("Database error: {e}"))
            })?;

        debug!("Saved checkpoint: id={}", checkpoint.id);
        Ok(())
    }

    async fn load(&self, checkpoint_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        let select_sql = format!(
            "SELECT checkpoint_id, thread_id, state, node, timestamp, parent_id, metadata
             FROM {}
             WHERE checkpoint_id = $1",
            self.table_name
        );

        let rows = self
            .client
            .query(&select_sql, &[&checkpoint_id])
            .await
            .map_err(|e| {
                error!("Failed to load checkpoint: {}", e);
                dashflow::Error::Generic(format!("Database error: {e}"))
            })?;

        if rows.is_empty() {
            return Ok(None);
        }

        let checkpoint = self.deserialize_checkpoint_from_row(&rows[0])?;
        Ok(Some(checkpoint))
    }

    async fn get_latest(&self, thread_id: &str) -> DashFlowResult<Option<Checkpoint<S>>> {
        let select_sql = format!(
            "SELECT checkpoint_id, thread_id, state, node, timestamp, parent_id, metadata
             FROM {}
             WHERE thread_id = $1
             ORDER BY timestamp DESC, checkpoint_id DESC
             LIMIT 1",
            self.table_name
        );

        let rows = self
            .client
            .query(&select_sql, &[&thread_id])
            .await
            .map_err(|e| {
                error!("Failed to get latest checkpoint: {}", e);
                dashflow::Error::Generic(format!("Database error: {e}"))
            })?;

        if rows.is_empty() {
            return Ok(None);
        }

        let checkpoint = self.deserialize_checkpoint_from_row(&rows[0])?;
        Ok(Some(checkpoint))
    }

    async fn list(&self, thread_id: &str) -> DashFlowResult<Vec<CheckpointMetadata>> {
        let select_sql = format!(
            "SELECT checkpoint_id, thread_id, node, timestamp, parent_id, metadata
             FROM {}
             WHERE thread_id = $1
             ORDER BY timestamp DESC, checkpoint_id DESC",
            self.table_name
        );

        let rows = self
            .client
            .query(&select_sql, &[&thread_id])
            .await
            .map_err(|e| {
                error!("Failed to list checkpoints: {}", e);
                dashflow::Error::Generic(format!("Database error: {e}"))
            })?;

        let mut checkpoints = Vec::new();
        for row in rows {
            let checkpoint_id: String = row.get(0);
            let thread_id: String = row.get(1);
            let node: String = row.get(2);
            let timestamp: i64 = row.get(3);
            let parent_id: Option<String> = row.get(4);
            let metadata_json: serde_json::Value = row.get(5);

            let metadata: std::collections::HashMap<String, String> =
                serde_json::from_value(metadata_json).map_err(|e| {
                    error!(
                        "Failed to deserialize checkpoint metadata for checkpoint '{}': {}",
                        checkpoint_id, e
                    );
                    dashflow::Error::Generic(format!(
                        "Metadata deserialization error for checkpoint '{checkpoint_id}': {e}"
                    ))
                })?;

            checkpoints.push(CheckpointMetadata {
                id: checkpoint_id,
                thread_id,
                node,
                timestamp: nanos_to_timestamp(timestamp),
                parent_id,
                metadata,
            });
        }

        Ok(checkpoints)
    }

    async fn delete(&self, checkpoint_id: &str) -> DashFlowResult<()> {
        let delete_sql = format!("DELETE FROM {} WHERE checkpoint_id = $1", self.table_name);

        self.client
            .execute(&delete_sql, &[&checkpoint_id])
            .await
            .map_err(|e| {
                error!("Failed to delete checkpoint: {}", e);
                dashflow::Error::Generic(format!("Database error: {e}"))
            })?;

        debug!("Deleted checkpoint: id={}", checkpoint_id);
        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> DashFlowResult<()> {
        let delete_sql = format!("DELETE FROM {} WHERE thread_id = $1", self.table_name);

        let rows_deleted = self
            .client
            .execute(&delete_sql, &[&thread_id])
            .await
            .map_err(|e| {
                error!("Failed to delete thread checkpoints: {}", e);
                dashflow::Error::Generic(format!("Database error: {e}"))
            })?;

        debug!(
            "Deleted thread checkpoints: thread_id={}, count={}",
            thread_id, rows_deleted
        );
        Ok(())
    }

    async fn list_threads(&self) -> DashFlowResult<Vec<dashflow::ThreadInfo>> {
        // Get the latest checkpoint for each thread
        let select_sql = format!(
            r#"
            SELECT DISTINCT ON (thread_id)
                   thread_id, checkpoint_id, timestamp
            FROM {}
            ORDER BY thread_id, timestamp DESC, checkpoint_id DESC
            "#,
            self.table_name
        );

        let rows = self.client.query(&select_sql, &[]).await.map_err(|e| {
            error!("Failed to list threads: {}", e);
            dashflow::Error::Generic(format!("Database error: {e}"))
        })?;

        let mut thread_infos: Vec<dashflow::ThreadInfo> = rows
            .iter()
            .map(|row| {
                let thread_id: String = row.get("thread_id");
                let checkpoint_id: String = row.get("checkpoint_id");
                // Schema uses timestamp BIGINT (nanoseconds), convert to SystemTime
                let timestamp_nanos: i64 = row.get("timestamp");
                dashflow::ThreadInfo {
                    thread_id,
                    latest_checkpoint_id: checkpoint_id,
                    updated_at: nanos_to_timestamp(timestamp_nanos),
                    checkpoint_count: None,
                }
            })
            .collect();

        // Sort by updated_at DESC (most recent first)
        thread_infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(thread_infos)
    }
}

/// Error types for `PostgreSQL` checkpointer
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PostgresError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Invalid identifier: {0}")]
    InvalidIdentifier(String),

    #[cfg(feature = "compression")]
    #[error("Compression error: {0}")]
    CompressionError(String),
}

/// Convert `PostgresError` to `dashflow::Error` for use with `?` operator
impl From<PostgresError> for dashflow::Error {
    fn from(err: PostgresError) -> Self {
        use dashflow::error::CheckpointError;
        let checkpoint_err = match err {
            PostgresError::ConnectionError(msg) => CheckpointError::ConnectionLost {
                backend: "postgres".to_string(),
                reason: msg,
            },
            PostgresError::QueryError(msg) => CheckpointError::Other(format!("Query error: {}", msg)),
            PostgresError::InvalidIdentifier(msg) => {
                CheckpointError::Other(format!("Invalid identifier: {}", msg))
            }
            #[cfg(feature = "compression")]
            PostgresError::CompressionError(msg) => {
                CheckpointError::SerializationFailed { reason: msg }
            }
        };
        dashflow::Error::Checkpoint(checkpoint_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== PostgresError Display Tests ==========

    #[test]
    fn test_connection_error_display() {
        let err = PostgresError::ConnectionError("refused".to_string());
        assert_eq!(err.to_string(), "Connection error: refused");
    }

    #[test]
    fn test_query_error_display() {
        let err = PostgresError::QueryError("syntax error".to_string());
        assert_eq!(err.to_string(), "Query error: syntax error");
    }

    #[test]
    fn test_invalid_identifier_display() {
        let err = PostgresError::InvalidIdentifier("bad name".to_string());
        assert_eq!(err.to_string(), "Invalid identifier: bad name");
    }

    #[test]
    fn test_connection_error_display_empty() {
        let err = PostgresError::ConnectionError(String::new());
        assert_eq!(err.to_string(), "Connection error: ");
    }

    #[test]
    fn test_query_error_display_empty() {
        let err = PostgresError::QueryError(String::new());
        assert_eq!(err.to_string(), "Query error: ");
    }

    #[test]
    fn test_invalid_identifier_display_empty() {
        let err = PostgresError::InvalidIdentifier(String::new());
        assert_eq!(err.to_string(), "Invalid identifier: ");
    }

    #[test]
    fn test_connection_error_display_multiline() {
        let err = PostgresError::ConnectionError("line1\nline2".to_string());
        let msg = err.to_string();
        assert!(msg.contains("line1"));
        assert!(msg.contains("line2"));
    }

    #[test]
    fn test_query_error_display_special_chars() {
        let err = PostgresError::QueryError("error: column \"id\" does not exist".to_string());
        let msg = err.to_string();
        assert!(msg.contains("column"));
        assert!(msg.contains("does not exist"));
    }

    // ========== PostgresError Debug Tests ==========

    #[test]
    fn test_postgres_error_debug_connection() {
        let err = PostgresError::ConnectionError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ConnectionError"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_postgres_error_debug_query() {
        let err = PostgresError::QueryError("syntax".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("QueryError"));
        assert!(debug.contains("syntax"));
    }

    #[test]
    fn test_postgres_error_debug_invalid_identifier() {
        let err = PostgresError::InvalidIdentifier("bad".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidIdentifier"));
        assert!(debug.contains("bad"));
    }

    // ========== PostgresError to dashflow::Error Conversion Tests ==========

    #[test]
    fn test_postgres_error_conversion_connection() {
        let err = PostgresError::ConnectionError("connection refused".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();
        assert!(
            msg.contains("connection refused") || msg.contains("postgres"),
            "Expected connection error info, got: {}",
            msg
        );
    }

    #[test]
    fn test_postgres_error_conversion_query() {
        let err = PostgresError::QueryError("invalid syntax".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();
        assert!(
            msg.contains("Query error") || msg.contains("invalid syntax"),
            "Expected query error info, got: {}",
            msg
        );
    }

    #[test]
    fn test_postgres_error_conversion_invalid_identifier() {
        let err = PostgresError::InvalidIdentifier("1badname".to_string());
        let dashflow_err: dashflow::Error = err.into();
        let msg = dashflow_err.to_string();
        assert!(
            msg.contains("Invalid identifier") || msg.contains("1badname"),
            "Expected invalid identifier info, got: {}",
            msg
        );
    }

    #[test]
    fn test_postgres_error_conversion_preserves_message() {
        let original_msg = "unique_error_message_12345";
        let err = PostgresError::ConnectionError(original_msg.to_string());
        let dashflow_err: dashflow::Error = err.into();
        let converted_msg = dashflow_err.to_string();
        assert!(
            converted_msg.contains(original_msg),
            "Original message should be preserved, got: {}",
            converted_msg
        );
    }

    // ========== validate_identifier Valid Cases ==========

    #[test]
    fn test_validate_identifier_valid() {
        assert!(validate_identifier("checkpoints").is_ok());
        assert!(validate_identifier("_private").is_ok());
        assert!(validate_identifier("table123").is_ok());
        assert!(validate_identifier("my_table_name").is_ok());
    }

    #[test]
    fn test_validate_identifier_single_letter() {
        assert!(validate_identifier("a").is_ok());
        assert!(validate_identifier("Z").is_ok());
    }

    #[test]
    fn test_validate_identifier_single_underscore() {
        assert!(validate_identifier("_").is_ok());
    }

    #[test]
    fn test_validate_identifier_all_uppercase() {
        assert!(validate_identifier("CHECKPOINTS").is_ok());
        assert!(validate_identifier("MY_TABLE").is_ok());
    }

    #[test]
    fn test_validate_identifier_all_lowercase() {
        assert!(validate_identifier("checkpoints").is_ok());
        assert!(validate_identifier("my_table").is_ok());
    }

    #[test]
    fn test_validate_identifier_mixed_case() {
        assert!(validate_identifier("MyTable").is_ok());
        assert!(validate_identifier("myTABLE123").is_ok());
    }

    #[test]
    fn test_validate_identifier_underscore_prefix() {
        assert!(validate_identifier("_table").is_ok());
        assert!(validate_identifier("__double").is_ok());
        assert!(validate_identifier("___triple").is_ok());
    }

    #[test]
    fn test_validate_identifier_underscore_suffix() {
        assert!(validate_identifier("table_").is_ok());
        assert!(validate_identifier("table__").is_ok());
    }

    #[test]
    fn test_validate_identifier_underscore_in_middle() {
        assert!(validate_identifier("my_table_name").is_ok());
        assert!(validate_identifier("a_b_c_d_e").is_ok());
    }

    #[test]
    fn test_validate_identifier_consecutive_underscores() {
        assert!(validate_identifier("table__name").is_ok());
        assert!(validate_identifier("a___b").is_ok());
    }

    #[test]
    fn test_validate_identifier_max_length_boundary() {
        // Exactly 63 characters should be valid
        let exactly_63 = "a".repeat(63);
        assert!(validate_identifier(&exactly_63).is_ok());
    }

    #[test]
    fn test_validate_identifier_one_under_max() {
        // 62 characters should be valid
        let under_max = "a".repeat(62);
        assert!(validate_identifier(&under_max).is_ok());
    }

    #[test]
    fn test_validate_identifier_numbers_after_letter() {
        assert!(validate_identifier("a1").is_ok());
        assert!(validate_identifier("table1234567890").is_ok());
        assert!(validate_identifier("t123abc456").is_ok());
    }

    #[test]
    fn test_validate_identifier_numbers_after_underscore() {
        assert!(validate_identifier("_123").is_ok());
        assert!(validate_identifier("_1").is_ok());
    }

    #[test]
    fn test_validate_identifier_all_numbers_after_start() {
        assert!(validate_identifier("t0123456789").is_ok());
        assert!(validate_identifier("_0123456789").is_ok());
    }

    // ========== validate_identifier Invalid Cases ==========

    #[test]
    fn test_validate_identifier_empty() {
        let result = validate_identifier("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_validate_identifier_too_long() {
        let long_name = "a".repeat(64);
        let result = validate_identifier(&long_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("63"));
    }

    #[test]
    fn test_validate_identifier_way_too_long() {
        let very_long = "a".repeat(1000);
        let result = validate_identifier(&very_long);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("63"));
    }

    #[test]
    fn test_validate_identifier_starts_with_digit() {
        let result = validate_identifier("1table");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("start with"));
    }

    #[test]
    fn test_validate_identifier_all_digits() {
        assert!(validate_identifier("123").is_err());
        assert!(validate_identifier("0").is_err());
    }

    #[test]
    fn test_validate_identifier_invalid_char() {
        let result = validate_identifier("my-table");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid character"));
    }

    #[test]
    fn test_validate_identifier_hyphen() {
        let result = validate_identifier("my-table");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid character"));
        assert!(err_msg.contains("'-'"));
    }

    #[test]
    fn test_validate_identifier_dot() {
        let result = validate_identifier("schema.table");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid character"));
        assert!(err_msg.contains("'.'"));
    }

    #[test]
    fn test_validate_identifier_special_chars() {
        assert!(validate_identifier("drop;table").is_err());
        assert!(validate_identifier("select*").is_err());
        assert!(validate_identifier("name space").is_err());
    }

    #[test]
    fn test_validate_identifier_space_in_middle() {
        let result = validate_identifier("my table");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid character"));
        assert!(err_msg.contains("' '"));
    }

    #[test]
    fn test_validate_identifier_leading_space() {
        let result = validate_identifier(" table");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("start with"));
    }

    #[test]
    fn test_validate_identifier_trailing_space() {
        let result = validate_identifier("table ");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid character"));
    }

    #[test]
    fn test_validate_identifier_tab_character() {
        let result = validate_identifier("table\tname");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_newline() {
        let result = validate_identifier("table\nname");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_at_sign() {
        let result = validate_identifier("user@host");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid character"));
        assert!(err_msg.contains("'@'"));
    }

    #[test]
    fn test_validate_identifier_dollar_sign() {
        let result = validate_identifier("price$");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid character"));
        assert!(err_msg.contains("'$'"));
    }

    #[test]
    fn test_validate_identifier_parentheses() {
        assert!(validate_identifier("func()").is_err());
        assert!(validate_identifier("(table)").is_err());
    }

    #[test]
    fn test_validate_identifier_brackets() {
        assert!(validate_identifier("array[]").is_err());
        assert!(validate_identifier("[index]").is_err());
    }

    #[test]
    fn test_validate_identifier_quotes() {
        assert!(validate_identifier("\"quoted\"").is_err());
        assert!(validate_identifier("'quoted'").is_err());
        assert!(validate_identifier("`backtick`").is_err());
    }

    #[test]
    fn test_validate_identifier_slash() {
        assert!(validate_identifier("path/to").is_err());
        assert!(validate_identifier("path\\to").is_err());
    }

    #[test]
    fn test_validate_identifier_colon() {
        let result = validate_identifier("schema:table");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid character"));
        assert!(err_msg.contains("':'"));
    }

    #[test]
    fn test_validate_identifier_percent() {
        let result = validate_identifier("100%");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_ampersand() {
        let result = validate_identifier("a&b");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_equals() {
        let result = validate_identifier("a=b");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_plus() {
        let result = validate_identifier("a+b");
        assert!(result.is_err());
    }

    // ========== SQL Injection Prevention Tests ==========

    #[test]
    fn test_validate_identifier_sql_injection_drop() {
        // Classic SQL injection attempt
        let result = validate_identifier("x; DROP TABLE users; --");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection_union() {
        let result = validate_identifier("x UNION SELECT * FROM passwords");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection_comment() {
        let result = validate_identifier("table--comment");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection_quote() {
        let result = validate_identifier("table'");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection_semicolon() {
        let result = validate_identifier("table;");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection_or() {
        let result = validate_identifier("1 OR 1=1");
        assert!(result.is_err());
    }

    // ========== Unicode and Extended ASCII Tests ==========

    #[test]
    fn test_validate_identifier_unicode_letter() {
        // Unicode letters should fail (PostgreSQL unquoted identifiers are ASCII only)
        assert!(validate_identifier("tëst").is_err());
        assert!(validate_identifier("tàble").is_err());
    }

    #[test]
    fn test_validate_identifier_unicode_emoji() {
        assert!(validate_identifier("table🎉").is_err());
        assert!(validate_identifier("🎉table").is_err());
    }

    #[test]
    fn test_validate_identifier_unicode_chinese() {
        assert!(validate_identifier("表格").is_err());
    }

    #[test]
    fn test_validate_identifier_unicode_cyrillic() {
        // Cyrillic 'а' looks like Latin 'a' but should fail
        assert!(validate_identifier("tаble").is_err()); // Uses Cyrillic 'а'
    }

    #[test]
    fn test_validate_identifier_extended_ascii() {
        assert!(validate_identifier("café").is_err());
        assert!(validate_identifier("naïve").is_err());
    }

    // ========== PostgresError Variant Coverage Tests ==========

    #[test]
    fn test_postgres_error_all_variants_constructible() {
        // Verify we can construct all variants
        let errors: Vec<PostgresError> = vec![
            PostgresError::ConnectionError("conn".to_string()),
            PostgresError::QueryError("query".to_string()),
            PostgresError::InvalidIdentifier("ident".to_string()),
        ];
        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn test_postgres_error_connection_with_details() {
        let err = PostgresError::ConnectionError(
            "connection refused: host=localhost port=5432".to_string(),
        );
        let msg = err.to_string();
        assert!(msg.contains("localhost"));
        assert!(msg.contains("5432"));
    }

    #[test]
    fn test_postgres_error_query_with_sql() {
        let err = PostgresError::QueryError(
            "relation \"dashflow_checkpoints\" does not exist".to_string(),
        );
        let msg = err.to_string();
        assert!(msg.contains("dashflow_checkpoints"));
        assert!(msg.contains("does not exist"));
    }

    #[test]
    fn test_postgres_error_invalid_identifier_with_name() {
        let err = PostgresError::InvalidIdentifier(
            "identifier 'my-table' contains invalid character '-'".to_string(),
        );
        let msg = err.to_string();
        assert!(msg.contains("my-table"));
        assert!(msg.contains("-"));
    }

    // ========== PostgresError is non_exhaustive ==========

    #[test]
    fn test_postgres_error_non_exhaustive() {
        // Document that the enum is marked #[non_exhaustive]
        // This allows adding new variants in the future without breaking changes
        match PostgresError::ConnectionError("test".to_string()) {
            PostgresError::ConnectionError(_) => {}
            PostgresError::QueryError(_) => panic!("unexpected variant"),
            PostgresError::InvalidIdentifier(_) => panic!("unexpected variant"),
            #[cfg(feature = "compression")]
            PostgresError::CompressionError(_) => panic!("unexpected variant"),
            // non_exhaustive allows future variants
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    // ========== Common PostgreSQL Table Names ==========

    #[test]
    fn test_validate_identifier_common_names() {
        // Common table names that should all be valid
        assert!(validate_identifier("users").is_ok());
        assert!(validate_identifier("orders").is_ok());
        assert!(validate_identifier("products").is_ok());
        assert!(validate_identifier("transactions").is_ok());
        assert!(validate_identifier("checkpoints").is_ok());
        assert!(validate_identifier("sessions").is_ok());
    }

    #[test]
    fn test_validate_identifier_dashflow_names() {
        // DashFlow-specific names
        assert!(validate_identifier("dashflow_checkpoints").is_ok());
        assert!(validate_identifier("dashflow_states").is_ok());
        assert!(validate_identifier("dashflow_threads").is_ok());
        assert!(validate_identifier("checkpoint_v2").is_ok());
    }

    #[test]
    fn test_validate_identifier_system_prefix() {
        // System-like prefixes (should be valid syntactically)
        assert!(validate_identifier("pg_tables").is_ok());
        assert!(validate_identifier("sys_config").is_ok());
        assert!(validate_identifier("_pg_internal").is_ok());
    }

    // ========== RetentionPolicy Builder Pattern Tests ==========

    #[test]
    fn test_retention_policy_builder_basic() {
        let policy = RetentionPolicy::builder().build();
        // Builder with no settings should produce empty policy
        assert!(policy.keep_last_n.is_none());
        assert!(policy.delete_after.is_none());
        assert!(policy.keep_daily_for.is_none());
        assert!(policy.keep_weekly_for.is_none());
    }

    #[test]
    fn test_retention_policy_keep_last_n() {
        let policy = RetentionPolicy::builder().keep_last_n(10).build();
        assert_eq!(policy.keep_last_n, Some(10));
    }

    #[test]
    fn test_retention_policy_delete_after() {
        use std::time::Duration;
        let policy = RetentionPolicy::builder()
            .delete_after(Duration::from_secs(86400))
            .build();
        assert_eq!(policy.delete_after, Some(Duration::from_secs(86400)));
    }

    #[test]
    fn test_retention_policy_keep_daily_for() {
        use std::time::Duration;
        let policy = RetentionPolicy::builder()
            .keep_daily_for(Duration::from_secs(7 * 86400))
            .build();
        assert_eq!(policy.keep_daily_for, Some(Duration::from_secs(7 * 86400)));
    }

    #[test]
    fn test_retention_policy_keep_weekly_for() {
        use std::time::Duration;
        let policy = RetentionPolicy::builder()
            .keep_weekly_for(Duration::from_secs(30 * 86400))
            .build();
        assert_eq!(
            policy.keep_weekly_for,
            Some(Duration::from_secs(30 * 86400))
        );
    }

    #[test]
    fn test_retention_policy_all_settings() {
        use std::time::Duration;
        let policy = RetentionPolicy::builder()
            .keep_last_n(5)
            .keep_daily_for(Duration::from_secs(7 * 86400))
            .keep_weekly_for(Duration::from_secs(30 * 86400))
            .delete_after(Duration::from_secs(90 * 86400))
            .build();
        assert_eq!(policy.keep_last_n, Some(5));
        assert_eq!(policy.keep_daily_for, Some(Duration::from_secs(7 * 86400)));
        assert_eq!(
            policy.keep_weekly_for,
            Some(Duration::from_secs(30 * 86400))
        );
        assert_eq!(policy.delete_after, Some(Duration::from_secs(90 * 86400)));
    }

    #[test]
    fn test_retention_policy_builder_chaining() {
        use std::time::Duration;
        // Test that builder methods can be chained
        let policy = RetentionPolicy::builder()
            .keep_last_n(3)
            .delete_after(Duration::from_secs(3600))
            .build();
        assert_eq!(policy.keep_last_n, Some(3));
        assert_eq!(policy.delete_after, Some(Duration::from_secs(3600)));
    }

    // ========== Checkpoint Type Annotations ==========

    #[test]
    fn test_checkpoint_metadata_fields() {
        // Test that CheckpointMetadata has expected fields
        let metadata = CheckpointMetadata {
            id: "cp-123".to_string(),
            thread_id: "thread-456".to_string(),
            node: "node-789".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            parent_id: Some("cp-122".to_string()),
            metadata: std::collections::HashMap::new(),
        };
        assert_eq!(metadata.id, "cp-123");
        assert_eq!(metadata.thread_id, "thread-456");
        assert_eq!(metadata.node, "node-789");
        assert_eq!(metadata.parent_id, Some("cp-122".to_string()));
    }

    #[test]
    fn test_checkpoint_metadata_without_parent() {
        let metadata = CheckpointMetadata {
            id: "cp-root".to_string(),
            thread_id: "thread-1".to_string(),
            node: "start".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            parent_id: None,
            metadata: std::collections::HashMap::new(),
        };
        assert!(metadata.parent_id.is_none());
    }

    #[test]
    fn test_checkpoint_metadata_with_custom_metadata() {
        let mut custom = std::collections::HashMap::new();
        custom.insert("key1".to_string(), "value1".to_string());
        custom.insert("key2".to_string(), "value2".to_string());

        let metadata = CheckpointMetadata {
            id: "cp-1".to_string(),
            thread_id: "t-1".to_string(),
            node: "n".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            parent_id: None,
            metadata: custom,
        };
        assert_eq!(metadata.metadata.len(), 2);
        assert_eq!(metadata.metadata.get("key1"), Some(&"value1".to_string()));
    }

    // ========== Timestamp Conversion Tests ==========

    #[test]
    fn test_timestamp_to_nanos_epoch() {
        let epoch = SystemTime::UNIX_EPOCH;
        let nanos = timestamp_to_nanos(epoch);
        assert_eq!(nanos, 0);
    }

    #[test]
    fn test_nanos_to_timestamp_epoch() {
        let timestamp = nanos_to_timestamp(0);
        assert_eq!(timestamp, SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn test_timestamp_roundtrip() {
        let original = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let nanos = timestamp_to_nanos(original);
        let recovered = nanos_to_timestamp(nanos);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_timestamp_conversion_positive() {
        // 1 second after epoch
        let timestamp = nanos_to_timestamp(1_000_000_000);
        let expected = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1);
        assert_eq!(timestamp, expected);
    }

    #[test]
    fn test_timestamp_conversion_large() {
        // Large timestamp (year 2020 approximately)
        let large_nanos: i64 = 1_577_836_800_000_000_000; // 2020-01-01
        let timestamp = nanos_to_timestamp(large_nanos);
        let back_to_nanos = timestamp_to_nanos(timestamp);
        assert_eq!(large_nanos, back_to_nanos);
    }

    // ========== Error Message Quality Tests ==========

    #[test]
    fn test_error_messages_are_informative() {
        // Connection error should mention what went wrong
        let conn_err = PostgresError::ConnectionError("ECONNREFUSED".to_string());
        assert!(conn_err.to_string().len() > 10);

        // Query error should include context
        let query_err = PostgresError::QueryError("permission denied".to_string());
        assert!(query_err.to_string().contains("permission denied"));

        // Invalid identifier should include the bad name
        let ident_err = PostgresError::InvalidIdentifier("my-bad-name".to_string());
        assert!(ident_err.to_string().contains("my-bad-name"));
    }

    #[test]
    fn test_validate_identifier_error_includes_context() {
        // Error for empty should mention empty
        let empty_err = validate_identifier("").unwrap_err();
        assert!(empty_err.to_string().to_lowercase().contains("empty"));

        // Error for too long should mention the limit
        let long_err = validate_identifier(&"x".repeat(100)).unwrap_err();
        assert!(long_err.to_string().contains("63"));

        // Error for invalid char should mention the character
        let char_err = validate_identifier("bad-name").unwrap_err();
        assert!(char_err.to_string().contains("-"));

        // Error for invalid start should mention the requirement
        let start_err = validate_identifier("1name").unwrap_err();
        assert!(start_err.to_string().contains("start"));
    }
}
