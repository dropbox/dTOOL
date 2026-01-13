// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! SQLite Hot Index for WAL
//!
//! Provides fast lookups for recent executions stored in the WAL.
//! The index stores execution metadata and segment references for quick queries.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::introspection::ExecutionTrace;

/// Errors that can occur during index operations.
#[derive(Debug, Error)]
pub enum IndexError {
    /// SQLite error
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Index file not found
    #[error("Index file not found: {path}")]
    NotFound {
        /// Path that was not found
        path: PathBuf,
    },

    /// Schema migration failed
    #[error("Schema migration failed: {0}")]
    MigrationFailed(String),

    /// Mutex was poisoned (another thread panicked while holding the lock)
    #[error("Mutex poisoned: {0}")]
    MutexPoisoned(String),
}

/// Result type for index operations.
pub type IndexResult<T> = std::result::Result<T, IndexError>;

/// Summary of an execution for query results.
///
/// This is a lightweight view of an execution suitable for listing and filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Database row ID
    pub id: i64,

    /// Unique execution ID
    pub execution_id: String,

    /// Thread ID (for checkpointed executions)
    pub thread_id: Option<String>,

    /// Graph name (if known)
    pub graph_name: Option<String>,

    /// Start timestamp (milliseconds since epoch)
    pub started_at_ms: Option<i64>,

    /// End timestamp (milliseconds since epoch)
    pub ended_at_ms: Option<i64>,

    /// Whether execution completed successfully
    pub completed: bool,

    /// Total execution duration in milliseconds
    pub duration_ms: i64,

    /// Total tokens used
    pub total_tokens: i64,

    /// Number of errors encountered
    pub error_count: i64,

    /// Number of nodes executed
    pub node_count: i64,

    /// Path to the WAL segment containing this execution's events
    pub segment_path: Option<String>,
}

/// Current schema version for migrations.
const SCHEMA_VERSION: i32 = 1;

/// SQLite hot index for recent executions.
///
/// Stores execution metadata for fast lookups. The full execution data
/// is stored in WAL segments; this index provides quick access to recent
/// executions without scanning segment files.
///
/// # Thread Safety
///
/// The index uses `Arc<Mutex<Connection>>` for thread-safe access.
/// Operations acquire the lock for the duration of the database operation.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::wal::{SqliteIndex, ExecutionSummary};
///
/// // Create or open index
/// let index = SqliteIndex::new("~/.dashflow/index.db")?;
///
/// // Query recent executions
/// let recent = index.recent_executions(10)?;
/// for exec in recent {
///     println!("{}: {} nodes, {}ms", exec.execution_id, exec.node_count, exec.duration_ms);
/// }
/// ```
#[derive(Clone)]
pub struct SqliteIndex {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteIndex {
    /// Create or open an index at the given path.
    ///
    /// Creates the parent directories and database file if they don't exist.
    /// Applies schema migrations if needed.
    pub fn new<P: AsRef<Path>>(path: P) -> IndexResult<Self> {
        let path = path.as_ref();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                IndexError::MigrationFailed(format!("Failed to create directory: {e}"))
            })?;
        }

        let conn = Connection::open(path)?;
        Self::setup_connection(conn)
    }

    /// Create an in-memory index for testing.
    pub fn in_memory() -> IndexResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::setup_connection(conn)
    }

    /// Setup connection with WAL mode and schema.
    fn setup_connection(conn: Connection) -> IndexResult<Self> {
        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        // Initialize schema
        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Initialize database schema.
    fn init_schema(conn: &Connection) -> IndexResult<()> {
        // Create schema version table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )?;

        // Check current version
        let current_version: Option<i32> = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                row.get(0)
            })
            .optional()?
            .flatten();

        // Apply migrations if needed
        if current_version.unwrap_or(0) < SCHEMA_VERSION {
            Self::migrate_v1(conn)?;
        }

        Ok(())
    }

    /// Migration to schema version 1.
    fn migrate_v1(conn: &Connection) -> IndexResult<()> {
        // Create executions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS executions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT UNIQUE NOT NULL,
                thread_id TEXT,
                graph_name TEXT,
                started_at_ms INTEGER,
                ended_at_ms INTEGER,
                completed INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                error_count INTEGER NOT NULL DEFAULT 0,
                node_count INTEGER NOT NULL DEFAULT 0,
                segment_path TEXT,
                created_at_ms INTEGER NOT NULL
            )",
            [],
        )?;

        // Create indexes for common queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_executions_started_at ON executions(started_at_ms DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_executions_created_at ON executions(created_at_ms DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_executions_thread_id ON executions(thread_id)",
            [],
        )?;

        // Record migration
        conn.execute(
            "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
            params![SCHEMA_VERSION],
        )?;

        Ok(())
    }

    /// Insert or update an execution in the index.
    ///
    /// If an execution with the same ID already exists, it will be updated.
    pub fn upsert_execution(
        &self,
        trace: &ExecutionTrace,
        segment_path: Option<&str>,
    ) -> IndexResult<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;

        let execution_id = trace
            .execution_id
            .as_deref()
            .unwrap_or("unknown");

        let started_at_ms = parse_iso_timestamp(&trace.started_at);
        let ended_at_ms = parse_iso_timestamp(&trace.ended_at);

        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        conn.execute(
            "INSERT INTO executions (
                execution_id, thread_id, graph_name, started_at_ms, ended_at_ms,
                completed, duration_ms, total_tokens, error_count, node_count,
                segment_path, created_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(execution_id) DO UPDATE SET
                thread_id = excluded.thread_id,
                graph_name = excluded.graph_name,
                started_at_ms = excluded.started_at_ms,
                ended_at_ms = excluded.ended_at_ms,
                completed = excluded.completed,
                duration_ms = excluded.duration_ms,
                total_tokens = excluded.total_tokens,
                error_count = excluded.error_count,
                node_count = excluded.node_count,
                segment_path = excluded.segment_path",
            params![
                execution_id,
                trace.thread_id,
                trace.metadata.get("graph_name").and_then(|v| v.as_str()),
                started_at_ms,
                ended_at_ms,
                trace.completed,
                trace.total_duration_ms as i64,
                trace.total_tokens as i64,
                trace.errors.len() as i64,
                trace.nodes_executed.len() as i64,
                segment_path,
                created_at_ms,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Query recent executions.
    ///
    /// Returns executions ordered by start time (most recent first).
    pub fn recent_executions(&self, limit: usize) -> IndexResult<Vec<ExecutionSummary>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, execution_id, thread_id, graph_name, started_at_ms, ended_at_ms,
                    completed, duration_ms, total_tokens, error_count, node_count, segment_path
             FROM executions
             ORDER BY started_at_ms DESC NULLS LAST, created_at_ms DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ExecutionSummary {
                id: row.get(0)?,
                execution_id: row.get(1)?,
                thread_id: row.get(2)?,
                graph_name: row.get(3)?,
                started_at_ms: row.get(4)?,
                ended_at_ms: row.get(5)?,
                completed: row.get::<_, i32>(6)? != 0,
                duration_ms: row.get(7)?,
                total_tokens: row.get(8)?,
                error_count: row.get(9)?,
                node_count: row.get(10)?,
                segment_path: row.get(11)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(IndexError::from)
    }

    /// Get an execution by its ID.
    pub fn execution_by_id(&self, execution_id: &str) -> IndexResult<Option<ExecutionSummary>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;

        conn.query_row(
            "SELECT id, execution_id, thread_id, graph_name, started_at_ms, ended_at_ms,
                    completed, duration_ms, total_tokens, error_count, node_count, segment_path
             FROM executions
             WHERE execution_id = ?1",
            params![execution_id],
            |row| {
                Ok(ExecutionSummary {
                    id: row.get(0)?,
                    execution_id: row.get(1)?,
                    thread_id: row.get(2)?,
                    graph_name: row.get(3)?,
                    started_at_ms: row.get(4)?,
                    ended_at_ms: row.get(5)?,
                    completed: row.get::<_, i32>(6)? != 0,
                    duration_ms: row.get(7)?,
                    total_tokens: row.get(8)?,
                    error_count: row.get(9)?,
                    node_count: row.get(10)?,
                    segment_path: row.get(11)?,
                })
            },
        )
        .optional()
        .map_err(IndexError::from)
    }

    /// Query executions by thread ID.
    pub fn executions_by_thread(
        &self,
        thread_id: &str,
        limit: usize,
    ) -> IndexResult<Vec<ExecutionSummary>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, execution_id, thread_id, graph_name, started_at_ms, ended_at_ms,
                    completed, duration_ms, total_tokens, error_count, node_count, segment_path
             FROM executions
             WHERE thread_id = ?1
             ORDER BY started_at_ms DESC NULLS LAST, created_at_ms DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![thread_id, limit as i64], |row| {
            Ok(ExecutionSummary {
                id: row.get(0)?,
                execution_id: row.get(1)?,
                thread_id: row.get(2)?,
                graph_name: row.get(3)?,
                started_at_ms: row.get(4)?,
                ended_at_ms: row.get(5)?,
                completed: row.get::<_, i32>(6)? != 0,
                duration_ms: row.get(7)?,
                total_tokens: row.get(8)?,
                error_count: row.get(9)?,
                node_count: row.get(10)?,
                segment_path: row.get(11)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(IndexError::from)
    }

    /// Count total executions in the index.
    pub fn count(&self) -> IndexResult<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;
        conn.query_row("SELECT COUNT(*) FROM executions", [], |row| row.get(0))
            .map_err(IndexError::from)
    }

    /// Rewrite a segment path to a new path for all executions that reference it.
    ///
    /// Returns the number of updated rows.
    pub fn rewrite_segment_path(&self, old_path: &str, new_path: &str) -> IndexResult<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;
        let updated = conn.execute(
            "UPDATE executions SET segment_path = ?1 WHERE segment_path = ?2",
            params![new_path, old_path],
        )?;
        Ok(updated)
    }

    /// Delete executions older than the given timestamp.
    ///
    /// Returns the number of deleted rows.
    pub fn delete_older_than(&self, timestamp_ms: i64) -> IndexResult<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;
        let deleted = conn.execute(
            "DELETE FROM executions WHERE created_at_ms < ?1",
            params![timestamp_ms],
        )?;
        Ok(deleted)
    }

    /// Vacuum the database to reclaim space.
    pub fn vacuum(&self) -> IndexResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| IndexError::MutexPoisoned(e.to_string()))?;
        conn.execute("VACUUM", [])?;
        Ok(())
    }
}

impl std::fmt::Debug for SqliteIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteIndex").finish_non_exhaustive()
    }
}

/// Parse an ISO 8601 timestamp string to milliseconds since epoch.
fn parse_iso_timestamp(ts: &Option<String>) -> Option<i64> {
    ts.as_ref().and_then(|s| {
        // Try parsing common ISO 8601 formats
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.timestamp_millis())
            .ok()
            .or_else(|| {
                // Try without timezone
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                    .map(|dt| dt.and_utc().timestamp_millis())
                    .ok()
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_trace(execution_id: &str) -> ExecutionTrace {
        ExecutionTrace {
            thread_id: Some("test-thread".to_string()),
            execution_id: Some(execution_id.to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            nodes_executed: vec![],
            total_duration_ms: 100,
            total_tokens: 50,
            errors: vec![],
            completed: true,
            started_at: Some("2025-01-01T00:00:00Z".to_string()),
            ended_at: Some("2025-01-01T00:00:01Z".to_string()),
            final_state: None,
            metadata: HashMap::new(),
            execution_metrics: None,
            performance_metrics: None,
        }
    }

    #[test]
    fn in_memory_index_creates_schema() {
        let index = SqliteIndex::in_memory().unwrap();
        assert_eq!(index.count().unwrap(), 0);
    }

    #[test]
    fn insert_and_query_execution() {
        let index = SqliteIndex::in_memory().unwrap();
        let trace = test_trace("exec-123");

        let id = index.upsert_execution(&trace, Some("/path/to/segment.wal")).unwrap();
        assert!(id > 0);

        let recent = index.recent_executions(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].execution_id, "exec-123");
        assert_eq!(recent[0].thread_id, Some("test-thread".to_string()));
        assert!(recent[0].completed);
        assert_eq!(recent[0].duration_ms, 100);
        assert_eq!(recent[0].total_tokens, 50);
        assert_eq!(recent[0].segment_path, Some("/path/to/segment.wal".to_string()));
    }

    #[test]
    fn upsert_updates_existing() {
        let index = SqliteIndex::in_memory().unwrap();

        let mut trace = test_trace("exec-456");
        trace.completed = false;
        trace.total_duration_ms = 50;
        index.upsert_execution(&trace, None).unwrap();

        // Update same execution
        trace.completed = true;
        trace.total_duration_ms = 200;
        index.upsert_execution(&trace, Some("/new/path.wal")).unwrap();

        let result = index.execution_by_id("exec-456").unwrap().unwrap();
        assert!(result.completed);
        assert_eq!(result.duration_ms, 200);
        assert_eq!(result.segment_path, Some("/new/path.wal".to_string()));

        // Should still have only one execution
        assert_eq!(index.count().unwrap(), 1);
    }

    #[test]
    fn query_by_execution_id() {
        let index = SqliteIndex::in_memory().unwrap();
        index.upsert_execution(&test_trace("exec-789"), None).unwrap();

        let result = index.execution_by_id("exec-789").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().execution_id, "exec-789");

        let missing = index.execution_by_id("nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn query_by_thread_id() {
        let index = SqliteIndex::in_memory().unwrap();

        let mut trace1 = test_trace("exec-1");
        trace1.thread_id = Some("thread-A".to_string());
        index.upsert_execution(&trace1, None).unwrap();

        let mut trace2 = test_trace("exec-2");
        trace2.thread_id = Some("thread-A".to_string());
        index.upsert_execution(&trace2, None).unwrap();

        let mut trace3 = test_trace("exec-3");
        trace3.thread_id = Some("thread-B".to_string());
        index.upsert_execution(&trace3, None).unwrap();

        let thread_a = index.executions_by_thread("thread-A", 10).unwrap();
        assert_eq!(thread_a.len(), 2);

        let thread_b = index.executions_by_thread("thread-B", 10).unwrap();
        assert_eq!(thread_b.len(), 1);
    }

    #[test]
    fn recent_executions_respects_limit() {
        let index = SqliteIndex::in_memory().unwrap();

        for i in 0..20 {
            let mut trace = test_trace(&format!("exec-{i}"));
            // Set different start times to ensure ordering
            trace.started_at = Some(format!("2025-01-01T00:00:{i:02}Z"));
            index.upsert_execution(&trace, None).unwrap();
        }

        let recent_5 = index.recent_executions(5).unwrap();
        assert_eq!(recent_5.len(), 5);

        let recent_100 = index.recent_executions(100).unwrap();
        assert_eq!(recent_100.len(), 20);
    }

    #[test]
    fn delete_older_than_removes_old_entries() {
        let index = SqliteIndex::in_memory().unwrap();

        // Insert some executions
        for i in 0..5 {
            index.upsert_execution(&test_trace(&format!("exec-{i}")), None).unwrap();
        }

        assert_eq!(index.count().unwrap(), 5);

        // Delete all (using far future timestamp)
        let future_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + 1_000_000;

        let deleted = index.delete_older_than(future_ts).unwrap();
        assert_eq!(deleted, 5);
        assert_eq!(index.count().unwrap(), 0);
    }

    #[test]
    fn handles_missing_timestamps() {
        let index = SqliteIndex::in_memory().unwrap();

        let trace = ExecutionTrace {
            thread_id: None,
            execution_id: Some("no-times".to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            nodes_executed: vec![],
            total_duration_ms: 0,
            total_tokens: 0,
            errors: vec![],
            completed: false,
            started_at: None,
            ended_at: None,
            final_state: None,
            metadata: HashMap::new(),
            execution_metrics: None,
            performance_metrics: None,
        };

        index.upsert_execution(&trace, None).unwrap();

        let result = index.execution_by_id("no-times").unwrap().unwrap();
        assert!(result.started_at_ms.is_none());
        assert!(result.ended_at_ms.is_none());
    }

    #[test]
    fn file_based_index_creates_directories() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("subdir").join("index.db");

        let index = SqliteIndex::new(&db_path).unwrap();
        assert_eq!(index.count().unwrap(), 0);
        assert!(db_path.exists());
    }
}
