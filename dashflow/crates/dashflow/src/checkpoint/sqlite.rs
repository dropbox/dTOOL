// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clone_on_ref_ptr: SqliteCheckpointer uses Arc<Mutex<Connection>> for thread-safe sharing
#![allow(clippy::clone_on_ref_ptr)]

//! SQLite-based checkpoint storage
//!
//! Provides lightweight, file-based checkpoint persistence using SQLite.
//! Ideal for single-process deployments and development.
//!
//! # Features
//!
//! - Single file deployment (no external database required)
//! - WAL mode for concurrent access
//! - Automatic schema migrations
//! - In-memory mode for testing
//!
//! # Encryption (requires `encryption` feature)
//!
//! State data can be encrypted using ChaCha20-Poly1305 before storage:
//!
//! ```rust,ignore
//! use dashflow::checkpoint::{SqliteCheckpointer, EncryptionKey, encrypt_bytes, decrypt_bytes};
//!
//! // Create encryption key
//! let key = EncryptionKey::from_bytes(&my_32_byte_key)?;
//!
//! // Manually encrypt state before storing
//! let encrypted_state = encrypt_bytes(&key, &state_bytes)?;
//!
//! // Decrypt after loading
//! let decrypted_state = decrypt_bytes(&key, &encrypted_bytes)?;
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::checkpoint::{SqliteCheckpointer, Checkpointer};
//!
//! // File-based storage
//! let checkpointer = SqliteCheckpointer::new("./checkpoints.db")?;
//!
//! // In-memory for testing
//! let checkpointer = SqliteCheckpointer::in_memory()?;
//!
//! // Save checkpoint
//! checkpointer.save(checkpoint).await?;
//!
//! // Load latest
//! let latest = checkpointer.get_latest("thread-1").await?;
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use rusqlite::{params, Connection, OptionalExtension};

use crate::checkpoint::{Checkpoint, CheckpointMetadata, Checkpointer, ThreadInfo};
use crate::error::{Error, Result};
use crate::state::GraphState;

/// Current schema version for migrations
/// Reserved for future migration support when schema changes are needed.
#[allow(dead_code)] // Architectural: Reserved for schema migration support
const SCHEMA_VERSION: i32 = 1;

/// SQLite-based checkpointer for lightweight persistence
///
/// Uses SQLite for single-file checkpoint storage with support for
/// WAL mode (concurrent reads), automatic migrations, and optional
/// in-memory storage for testing.
pub struct SqliteCheckpointer {
    /// Database connection (wrapped in Mutex for async safety)
    conn: Arc<Mutex<Connection>>,
}

impl SqliteCheckpointer {
    /// Create a new SQLite checkpointer with file-based storage
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SQLite database file
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let checkpointer = SqliteCheckpointer::new("./checkpoints.db")?;
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::ConnectionLost {
                backend: "sqlite".to_string(),
                reason: e.to_string(),
            })
        })?;

        Self::setup_connection(conn)
    }

    /// Create a new SQLite checkpointer with in-memory storage
    ///
    /// Useful for testing and ephemeral workflows.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let checkpointer = SqliteCheckpointer::in_memory()?;
    /// ```
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::ConnectionLost {
                backend: "sqlite-memory".to_string(),
                reason: e.to_string(),
            })
        })?;

        Self::setup_connection(conn)
    }

    /// Setup connection with WAL mode and schema
    fn setup_connection(conn: Connection) -> Result<Self> {
        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                    "Failed to set WAL mode: {}",
                    e
                )))
            })?;

        // Create schema
        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Initialize database schema with migrations
    fn init_schema(conn: &Connection) -> Result<()> {
        // Create version tracking table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::MigrationFailed {
                from: 0,
                to: 0,
                reason: format!("Failed to create schema_version table: {}", e),
            })
        })?;

        // Check current version
        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Apply migrations
        if current_version < 1 {
            Self::migrate_v1(conn)?;
        }

        Ok(())
    }

    /// Migration to version 1: initial schema
    fn migrate_v1(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            -- Main checkpoints table
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                node TEXT NOT NULL,
                state BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                parent_id TEXT,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            -- Index for fast thread lookups
            CREATE INDEX IF NOT EXISTS idx_checkpoints_thread_id
            ON checkpoints(thread_id, timestamp DESC);

            -- Index for parent lookups (history traversal)
            CREATE INDEX IF NOT EXISTS idx_checkpoints_parent_id
            ON checkpoints(parent_id);

            -- Metadata table (key-value pairs per checkpoint)
            CREATE TABLE IF NOT EXISTS checkpoint_metadata (
                checkpoint_id TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                PRIMARY KEY (checkpoint_id, key),
                FOREIGN KEY (checkpoint_id) REFERENCES checkpoints(id) ON DELETE CASCADE
            );

            -- Record migration
            INSERT INTO schema_version (version) VALUES (1);
            ",
        )
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::MigrationFailed {
                from: 0,
                to: 1,
                reason: format!("Failed to apply v1 migration: {}", e),
            })
        })?;

        Ok(())
    }

    /// Serialize checkpoint state to bytes
    fn serialize_state<S: GraphState>(state: &S) -> Result<Vec<u8>> {
        bincode::serialize(state).map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                reason: format!("Failed to serialize state: {}", e),
            })
        })
    }

    /// Deserialize checkpoint state from bytes
    fn deserialize_state<S: GraphState>(data: &[u8]) -> Result<S> {
        bincode::deserialize(data).map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                reason: format!("Failed to deserialize state: {}", e),
            })
        })
    }

    /// Convert SystemTime to Unix timestamp (milliseconds since epoch for better precision)
    fn timestamp_to_unix(timestamp: SystemTime) -> i64 {
        timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    /// Convert Unix timestamp (milliseconds) to SystemTime
    fn unix_to_timestamp(unix: i64) -> SystemTime {
        std::time::UNIX_EPOCH + std::time::Duration::from_millis(unix as u64)
    }
}

// Note: rusqlite::Connection is not Send, so we wrap in Mutex and use spawn_blocking
// for async compatibility

#[async_trait::async_trait]
impl<S> Checkpointer<S> for SqliteCheckpointer
where
    S: GraphState,
{
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        let state_bytes = Self::serialize_state(&checkpoint.state)?;
        let timestamp_unix = Self::timestamp_to_unix(checkpoint.timestamp);
        let conn = self.conn.clone();

        let id = checkpoint.id.clone();
        let thread_id = checkpoint.thread_id.clone();
        let node = checkpoint.node.clone();
        let parent_id = checkpoint.parent_id.clone();
        let metadata = checkpoint.metadata.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                path: "sqlite-connection".to_string(),
                reason: format!("Lock poisoned: {e}"),
            }))?;

            // Insert checkpoint
            conn.execute(
                "INSERT OR REPLACE INTO checkpoints (id, thread_id, node, state, timestamp, parent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, thread_id, node, state_bytes, timestamp_unix, parent_id],
            )
            .map_err(|e| Error::Checkpoint(crate::error::CheckpointError::Other(format!("Failed to save checkpoint: {}", e))))?;

            // Insert metadata
            for (key, value) in metadata {
                conn.execute(
                    "INSERT OR REPLACE INTO checkpoint_metadata (checkpoint_id, key, value)
                     VALUES (?1, ?2, ?3)",
                    params![id, key, value],
                )
                .map_err(|e| Error::Checkpoint(crate::error::CheckpointError::Other(format!("Failed to save metadata: {}", e))))?;
            }

            Ok(())
        })
        .await
        .map_err(|e| Error::Checkpoint(crate::error::CheckpointError::Other(format!("Task join error: {}", e))))?
    }

    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        let conn = self.conn.clone();
        let checkpoint_id = checkpoint_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                    path: "sqlite-connection".to_string(),
                    reason: format!("Lock poisoned: {e}"),
                })
            })?;

            // Load checkpoint
            let row: Option<(String, String, String, Vec<u8>, i64, Option<String>)> = conn
                .query_row(
                    "SELECT id, thread_id, node, state, timestamp, parent_id
                     FROM checkpoints WHERE id = ?1",
                    params![checkpoint_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .optional()
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to load checkpoint: {}",
                        e
                    )))
                })?;

            let Some((id, thread_id, node, state_bytes, timestamp_unix, parent_id)) = row else {
                return Ok(None);
            };

            // Load metadata
            let mut metadata = HashMap::new();
            let mut stmt = conn
                .prepare("SELECT key, value FROM checkpoint_metadata WHERE checkpoint_id = ?1")
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to prepare metadata query: {}",
                        e
                    )))
                })?;

            let rows = stmt
                .query_map(params![id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to query metadata: {}",
                        e
                    )))
                })?;

            for row in rows {
                let (key, value) = row.map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Metadata row error: {}",
                        e
                    )))
                })?;
                metadata.insert(key, value);
            }

            let state = Self::deserialize_state(&state_bytes)?;

            Ok(Some(Checkpoint {
                id,
                thread_id,
                state,
                node,
                timestamp: Self::unix_to_timestamp(timestamp_unix),
                parent_id,
                metadata,
            }))
        })
        .await
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error: {}",
                e
            )))
        })?
    }

    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        let conn = self.conn.clone();
        let thread_id = thread_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                    path: "sqlite-connection".to_string(),
                    reason: format!("Lock poisoned: {e}"),
                })
            })?;

            // Get latest checkpoint ID for thread
            let latest_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM checkpoints WHERE thread_id = ?1
                     ORDER BY timestamp DESC LIMIT 1",
                    params![thread_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to get latest checkpoint: {}",
                        e
                    )))
                })?;

            let Some(checkpoint_id) = latest_id else {
                return Ok(None);
            };

            // Reuse load logic (but we're inside spawn_blocking, so direct query)
            let row: Option<(String, String, String, Vec<u8>, i64, Option<String>)> = conn
                .query_row(
                    "SELECT id, thread_id, node, state, timestamp, parent_id
                     FROM checkpoints WHERE id = ?1",
                    params![checkpoint_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .optional()
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to load checkpoint: {}",
                        e
                    )))
                })?;

            let Some((id, thread_id, node, state_bytes, timestamp_unix, parent_id)) = row else {
                return Ok(None);
            };

            // Load metadata
            let mut metadata = HashMap::new();
            let mut stmt = conn
                .prepare("SELECT key, value FROM checkpoint_metadata WHERE checkpoint_id = ?1")
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to prepare metadata query: {}",
                        e
                    )))
                })?;

            let rows = stmt
                .query_map(params![id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to query metadata: {}",
                        e
                    )))
                })?;

            for row in rows {
                let (key, value) = row.map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Metadata row error: {}",
                        e
                    )))
                })?;
                metadata.insert(key, value);
            }

            let state = SqliteCheckpointer::deserialize_state(&state_bytes)?;

            Ok(Some(Checkpoint {
                id,
                thread_id,
                state,
                node,
                timestamp: SqliteCheckpointer::unix_to_timestamp(timestamp_unix),
                parent_id,
                metadata,
            }))
        })
        .await
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error: {}",
                e
            )))
        })?
    }

    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>> {
        let conn = self.conn.clone();
        let thread_id = thread_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                    path: "sqlite-connection".to_string(),
                    reason: format!("Lock poisoned: {e}"),
                })
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT c.id, c.thread_id, c.node, c.timestamp, c.parent_id
                     FROM checkpoints c
                     WHERE c.thread_id = ?1
                     ORDER BY c.timestamp DESC",
                )
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to prepare list query: {}",
                        e
                    )))
                })?;

            let rows = stmt
                .query_map(params![thread_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                })
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to list checkpoints: {}",
                        e
                    )))
                })?;

            let mut results = Vec::new();
            for row in rows {
                let (id, thread_id, node, timestamp_unix, parent_id) = row.map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Row error: {}",
                        e
                    )))
                })?;

                // Load metadata for each checkpoint
                let mut metadata = HashMap::new();
                let mut meta_stmt = conn
                    .prepare("SELECT key, value FROM checkpoint_metadata WHERE checkpoint_id = ?1")
                    .map_err(|e| {
                        Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                            "Failed to prepare metadata query: {}",
                            e
                        )))
                    })?;

                let meta_rows = meta_stmt
                    .query_map(params![id], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .map_err(|e| {
                        Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                            "Failed to query metadata: {}",
                            e
                        )))
                    })?;

                for meta_row in meta_rows {
                    let (key, value) = meta_row.map_err(|e| {
                        Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                            "Metadata row error: {}",
                            e
                        )))
                    })?;
                    metadata.insert(key, value);
                }

                results.push(CheckpointMetadata {
                    id,
                    thread_id,
                    node,
                    timestamp: SqliteCheckpointer::unix_to_timestamp(timestamp_unix),
                    parent_id,
                    metadata,
                });
            }

            Ok(results)
        })
        .await
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error: {}",
                e
            )))
        })?
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        let conn = self.conn.clone();
        let checkpoint_id = checkpoint_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                    path: "sqlite-connection".to_string(),
                    reason: format!("Lock poisoned: {e}"),
                })
            })?;

            // Metadata is deleted automatically via CASCADE
            conn.execute(
                "DELETE FROM checkpoints WHERE id = ?1",
                params![checkpoint_id],
            )
            .map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                    "Failed to delete checkpoint: {}",
                    e
                )))
            })?;

            Ok(())
        })
        .await
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error: {}",
                e
            )))
        })?
    }

    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        let conn = self.conn.clone();
        let thread_id = thread_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                    path: "sqlite-connection".to_string(),
                    reason: format!("Lock poisoned: {e}"),
                })
            })?;

            // Metadata is deleted automatically via CASCADE
            conn.execute(
                "DELETE FROM checkpoints WHERE thread_id = ?1",
                params![thread_id],
            )
            .map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                    "Failed to delete thread checkpoints: {}",
                    e
                )))
            })?;

            Ok(())
        })
        .await
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error: {}",
                e
            )))
        })?
    }

    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                    path: "sqlite-connection".to_string(),
                    reason: format!("Lock poisoned: {e}"),
                })
            })?;

            // Get latest checkpoint for each thread, ordered by timestamp DESC
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT thread_id, id, timestamp_secs, timestamp_nanos
                    FROM checkpoints c1
                    WHERE NOT EXISTS (
                        SELECT 1 FROM checkpoints c2
                        WHERE c2.thread_id = c1.thread_id
                        AND (c2.timestamp_secs > c1.timestamp_secs
                             OR (c2.timestamp_secs = c1.timestamp_secs AND c2.id > c1.id))
                    )
                    ORDER BY timestamp_secs DESC, timestamp_nanos DESC
                    "#,
                )
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to prepare list_threads statement: {}",
                        e
                    )))
                })?;

            let rows = stmt
                .query_map([], |row| {
                    let thread_id: String = row.get(0)?;
                    let checkpoint_id: String = row.get(1)?;
                    let secs: u64 = row.get(2)?;
                    let nanos: u32 = row.get(3)?;
                    Ok((thread_id, checkpoint_id, secs, nanos))
                })
                .map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to query threads: {}",
                        e
                    )))
                })?;

            let mut thread_infos = Vec::new();
            for row in rows {
                let (thread_id, checkpoint_id, secs, nanos) = row.map_err(|e| {
                    Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Failed to read thread row: {}",
                        e
                    )))
                })?;

                let timestamp = std::time::UNIX_EPOCH + std::time::Duration::new(secs, nanos);

                thread_infos.push(ThreadInfo {
                    thread_id,
                    latest_checkpoint_id: checkpoint_id,
                    updated_at: timestamp,
                    checkpoint_count: None, // Could add a COUNT subquery but it's expensive
                });
            }

            Ok(thread_infos)
        })
        .await
        .map_err(|e| {
            Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error: {}",
                e
            )))
        })?
    }
}

impl Clone for SqliteCheckpointer {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}

impl std::fmt::Debug for SqliteCheckpointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteCheckpointer").finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
        name: String,
    }

    // GraphState is auto-implemented via blanket impl
    impl crate::state::MergeableState for TestState {
        fn merge(&mut self, _other: &Self) {}
    }

    #[tokio::test]
    async fn test_sqlite_checkpointer_save_load() {
        let checkpointer = SqliteCheckpointer::in_memory().unwrap();

        let state = TestState {
            value: 42,
            name: "test".to_string(),
        };

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            state.clone(),
            "node-1".to_string(),
            None,
        );
        let checkpoint_id = checkpoint.id.clone();

        Checkpointer::<TestState>::save(&checkpointer, checkpoint)
            .await
            .unwrap();

        let loaded = Checkpointer::<TestState>::load(&checkpointer, &checkpoint_id)
            .await
            .unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.state, state);
        assert_eq!(loaded.thread_id, "thread-1");
        assert_eq!(loaded.node, "node-1");
    }

    #[tokio::test]
    async fn test_sqlite_checkpointer_get_latest() {
        let checkpointer = SqliteCheckpointer::in_memory().unwrap();

        let state1 = TestState {
            value: 1,
            name: "first".to_string(),
        };
        let state2 = TestState {
            value: 2,
            name: "second".to_string(),
        };

        let checkpoint1 =
            Checkpoint::new("thread-1".to_string(), state1, "node-1".to_string(), None);
        Checkpointer::<TestState>::save(&checkpointer, checkpoint1.clone())
            .await
            .unwrap();

        // Small delay BEFORE creating second checkpoint to ensure different timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let checkpoint2 = Checkpoint::new(
            "thread-1".to_string(),
            state2.clone(),
            "node-2".to_string(),
            Some(checkpoint1.id.clone()),
        );
        Checkpointer::<TestState>::save(&checkpointer, checkpoint2)
            .await
            .unwrap();

        let latest = Checkpointer::<TestState>::get_latest(&checkpointer, "thread-1")
            .await
            .unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().state, state2);
    }

    #[tokio::test]
    async fn test_sqlite_checkpointer_list() {
        let checkpointer = SqliteCheckpointer::in_memory().unwrap();

        let state1 = TestState {
            value: 1,
            name: "first".to_string(),
        };
        let state2 = TestState {
            value: 2,
            name: "second".to_string(),
        };

        let checkpoint1 =
            Checkpoint::new("thread-1".to_string(), state1, "node-1".to_string(), None);
        Checkpointer::<TestState>::save(&checkpointer, checkpoint1)
            .await
            .unwrap();

        // Small delay BEFORE creating second checkpoint to ensure different timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let checkpoint2 =
            Checkpoint::new("thread-1".to_string(), state2, "node-2".to_string(), None);
        Checkpointer::<TestState>::save(&checkpointer, checkpoint2)
            .await
            .unwrap();

        let list = Checkpointer::<TestState>::list(&checkpointer, "thread-1")
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
        // Should be ordered by timestamp DESC (newest first)
        assert_eq!(list[0].node, "node-2");
        assert_eq!(list[1].node, "node-1");
    }

    #[tokio::test]
    async fn test_sqlite_checkpointer_delete() {
        let checkpointer = SqliteCheckpointer::in_memory().unwrap();

        let state = TestState {
            value: 42,
            name: "test".to_string(),
        };

        let checkpoint = Checkpoint::new("thread-1".to_string(), state, "node-1".to_string(), None);
        let checkpoint_id = checkpoint.id.clone();

        Checkpointer::<TestState>::save(&checkpointer, checkpoint)
            .await
            .unwrap();
        Checkpointer::<TestState>::delete(&checkpointer, &checkpoint_id)
            .await
            .unwrap();

        let loaded = Checkpointer::<TestState>::load(&checkpointer, &checkpoint_id)
            .await
            .unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_checkpointer_delete_thread() {
        let checkpointer = SqliteCheckpointer::in_memory().unwrap();

        let state1 = TestState {
            value: 1,
            name: "first".to_string(),
        };
        let state2 = TestState {
            value: 2,
            name: "second".to_string(),
        };

        let checkpoint1 =
            Checkpoint::new("thread-1".to_string(), state1, "node-1".to_string(), None);
        let checkpoint2 =
            Checkpoint::new("thread-1".to_string(), state2, "node-2".to_string(), None);

        Checkpointer::<TestState>::save(&checkpointer, checkpoint1)
            .await
            .unwrap();
        Checkpointer::<TestState>::save(&checkpointer, checkpoint2)
            .await
            .unwrap();

        Checkpointer::<TestState>::delete_thread(&checkpointer, "thread-1")
            .await
            .unwrap();

        let list = Checkpointer::<TestState>::list(&checkpointer, "thread-1")
            .await
            .unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_sqlite_checkpointer_with_metadata() {
        let checkpointer = SqliteCheckpointer::in_memory().unwrap();

        let state = TestState {
            value: 42,
            name: "test".to_string(),
        };

        let checkpoint = Checkpoint::new("thread-1".to_string(), state, "node-1".to_string(), None)
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");
        let checkpoint_id = checkpoint.id.clone();

        Checkpointer::<TestState>::save(&checkpointer, checkpoint)
            .await
            .unwrap();

        let loaded = Checkpointer::<TestState>::load(&checkpointer, &checkpoint_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(loaded.metadata.get("key2"), Some(&"value2".to_string()));
    }
}
