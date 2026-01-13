// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Event Store API
//!
//! High-level API for querying persisted execution data.
//! Combines WAL segment storage with SQLite index for fast lookups.

use crate::core::config_loader::env_vars::{
    env_bool, env_string, DASHFLOW_WAL_AUTO_COMPACTION, DASHFLOW_WAL_PARQUET_DIR,
};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use arrow::array::{Array, Int64Array, StringArray};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use thiserror::Error;

use super::compaction::{
    compact_once as compact_once_impl, spawn_compaction_worker as spawn_compaction_worker_impl,
    CompactionConfig, CompactionError, CompactionHandle,
};
use super::index::{ExecutionSummary, IndexError, SqliteIndex};
use super::writer::{WALEvent, WALWriter, WALWriterConfig, WALWriterError};
use crate::introspection::ExecutionTrace;

/// Errors that can occur during event store operations.
#[derive(Debug, Error)]
pub enum EventStoreError {
    /// WAL writer error
    #[error("WAL error: {0}")]
    Wal(#[from] WALWriterError),

    /// Index error
    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O error during segment reading
    #[error("Segment I/O error: {0}")]
    SegmentIo(#[from] std::io::Error),

    /// JSON parsing error during segment reading
    #[error("Segment parse error: {0}")]
    SegmentParse(#[from] serde_json::Error),

    /// Parquet error
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    /// Arrow error
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// Compaction error
    #[error("Compaction error: {0}")]
    Compaction(#[from] CompactionError),

    /// Execution not found
    #[error("Execution not found: {0}")]
    ExecutionNotFound(String),

    /// Segment file not found
    #[error("Segment file not found: {0}")]
    SegmentNotFound(PathBuf),
}

/// Result type for event store operations.
pub type EventStoreResult<T> = std::result::Result<T, EventStoreError>;

/// Configuration for the event store.
#[derive(Debug, Clone)]
pub struct EventStoreConfig {
    /// WAL writer configuration
    pub wal: WALWriterConfig,

    /// Path to SQLite index database
    pub index_path: PathBuf,

    /// Whether to automatically start the compaction worker.
    ///
    /// When true (default), a background thread compacts WAL segments to Parquet
    /// and applies retention policies. Set `DASHFLOW_WAL_AUTO_COMPACTION=false`
    /// to disable.
    pub auto_compaction: bool,
}

impl Default for EventStoreConfig {
    fn default() -> Self {
        Self {
            wal: WALWriterConfig::default(),
            index_path: super::index_path(),
            auto_compaction: true,
        }
    }
}

impl EventStoreConfig {
    /// Create config from environment variables.
    pub fn from_env() -> Self {
        let auto_compaction = env_bool(DASHFLOW_WAL_AUTO_COMPACTION, true);

        Self {
            wal: WALWriterConfig::from_env(),
            index_path: super::index_path(),
            auto_compaction,
        }
    }

    /// Create config with compaction disabled.
    ///
    /// Useful for tests to avoid background threads.
    #[must_use]
    pub fn without_compaction(mut self) -> Self {
        self.auto_compaction = false;
        self
    }
}

/// Read events for a specific execution from a segment file.
///
/// Scans the segment file line by line, parsing JSON and filtering
/// by execution ID.
fn read_events_from_segment(
    segment_path: &Path,
    execution_id: &str,
) -> EventStoreResult<Vec<WALEvent>> {
    let file = File::open(segment_path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Try to parse as WALEvent
        let event: WALEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed lines
        };

        // Filter by execution ID
        if let Some(ref event_exec_id) = event.execution_id {
            if event_exec_id == execution_id {
                events.push(event);
            }
        }
    }

    Ok(events)
}

fn parquet_dir_for_wal_dir(wal_dir: &Path) -> PathBuf {
    env_string(DASHFLOW_WAL_PARQUET_DIR)
        .map(PathBuf::from)
        .unwrap_or_else(|| wal_dir.join("parquet"))
}

fn list_parquet_files(parquet_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(parquet_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(e) => return Err(e),
    };

    for entry in entries {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let is_parquet = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("parquet"));
        if is_parquet {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn read_events_from_parquet(
    parquet_path: &Path,
    execution_id: &str,
) -> EventStoreResult<Vec<WALEvent>> {
    let file = File::open(parquet_path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let reader = builder.build()?;

    let mut events = Vec::new();

    for batch in reader {
        let batch = batch?;

        let ts = batch
            .column_by_name("timestamp_ms")
            .and_then(|a| a.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                EventStoreError::Config("Parquet schema missing timestamp_ms".to_string())
            })?;
        let event_type = batch
            .column_by_name("event_type")
            .and_then(|a| a.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                EventStoreError::Config("Parquet schema missing event_type".to_string())
            })?;
        let exec = batch
            .column_by_name("execution_id")
            .and_then(|a| a.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                EventStoreError::Config("Parquet schema missing execution_id".to_string())
            })?;
        let payload = batch
            .column_by_name("payload_json")
            .and_then(|a| a.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                EventStoreError::Config("Parquet schema missing payload_json".to_string())
            })?;

        for row in 0..batch.num_rows() {
            if exec.is_null(row) {
                continue;
            }
            let row_exec = exec.value(row);
            if row_exec != execution_id {
                continue;
            }

            let row_event_type = event_type.value(row);
            let event_type = row_event_type
                .parse()
                .map_err(|_| EventStoreError::Config(format!("Unknown WAL event_type: '{}' (expected one of: ExecutionStart, ExecutionEnd, NodeStart, NodeEnd, EdgeEvaluated, StateChanged, DecisionMade, OutcomeObserved)", row_event_type)))?;

            let payload_json = payload.value(row);
            let payload = serde_json::from_str(payload_json)?;

            events.push(WALEvent {
                timestamp_ms: ts.value(row) as u64,
                event_type,
                execution_id: Some(row_exec.to_string()),
                // Hierarchical IDs not available from legacy Parquet storage
                // These would need schema migration to support
                parent_execution_id: None,
                root_execution_id: None,
                depth: None,
                payload,
            });
        }
    }

    Ok(events)
}

/// Event store for persisted execution data.
///
/// Combines WAL segment storage for durability with SQLite index for fast queries.
/// By default (following Invariant 6), a background compaction worker automatically
/// converts WAL segments to Parquet format and applies retention policies.
///
/// # Architecture
///
/// ```text
/// ┌──────────────┐     ┌─────────────────┐
/// │ write_trace()│────▶│   WALWriter     │────▶ WAL segments
/// └──────────────┘     └────────┬────────┘                 │
///                               │                          ▼
///                               ▼                    ┌─────────────┐
///                      ┌─────────────────┐           │ Compaction  │
///                      │  SqliteIndex    │           │   Worker    │
///                      └─────────────────┘           └──────┬──────┘
///                               │                          │
///                               ▼                          ▼
///                        Query results               Parquet files
/// ```
///
/// # Auto-Compaction
///
/// By default, EventStore starts a background compaction worker that:
/// - Converts closed WAL segments to Parquet format
/// - Updates the index to point to Parquet files
/// - Applies retention policies (default: 24 hours)
/// - Cleans up old data
///
/// Disable with `DASHFLOW_WAL_AUTO_COMPACTION=false` or use
/// [`EventStoreConfig::without_compaction()`].
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::wal::{EventStore, EventStoreConfig};
///
/// // Create event store with default config (auto-compaction enabled)
/// let store = EventStore::new(EventStoreConfig::default())?;
///
/// // Write execution trace
/// store.write_trace(&trace)?;
///
/// // Query recent executions
/// let recent = store.recent_executions(10)?;
/// for exec in recent {
///     println!("{}: {} nodes, {}ms", exec.execution_id, exec.node_count, exec.duration_ms);
/// }
/// ```
pub struct EventStore {
    writer: WALWriter,
    index: SqliteIndex,
    /// Background compaction worker handle (if auto-compaction is enabled).
    compaction_handle: Option<CompactionHandle>,
}

impl EventStore {
    /// Create a new event store with the given configuration.
    ///
    /// If `config.auto_compaction` is true (the default), a background compaction
    /// worker is started automatically. The worker will be stopped when the
    /// EventStore is dropped.
    pub fn new(config: EventStoreConfig) -> EventStoreResult<Self> {
        let writer = WALWriter::new(config.wal)?;
        let index = SqliteIndex::new(&config.index_path)?;

        let compaction_handle = if config.auto_compaction {
            let compaction_config = CompactionConfig::from_env(writer.wal_dir());
            Some(spawn_compaction_worker_impl(
                writer.clone(),
                index.clone(),
                compaction_config,
            ))
        } else {
            None
        };

        Ok(Self {
            writer,
            index,
            compaction_handle,
        })
    }

    /// Create an event store with default configuration from environment.
    pub fn from_env() -> EventStoreResult<Self> {
        Self::new(EventStoreConfig::from_env())
    }

    /// Write an execution trace to the store.
    ///
    /// This writes the trace to the WAL segment and updates the index.
    pub fn write_trace(&self, trace: &ExecutionTrace) -> EventStoreResult<()> {
        // Write to WAL
        self.writer.write_trace(trace)?;

        // Record the actual segment that received the write (post-rotation).
        let segment_path = self
            .writer
            .current_segment_path()?
            .map(|p| p.to_string_lossy().to_string());

        // Update index
        self.index.upsert_execution(trace, segment_path.as_deref())?;

        Ok(())
    }

    /// Compact closed WAL segments to Parquet and apply retention.
    pub fn compact_once(&self, config: &CompactionConfig) -> EventStoreResult<()> {
        compact_once_impl(&self.writer, &self.index, config)?;
        Ok(())
    }

    /// Spawn a background compaction worker thread.
    ///
    /// The worker runs until `CompactionHandle::stop_and_join()` is called.
    pub fn spawn_compaction_worker(&self, config: CompactionConfig) -> CompactionHandle {
        spawn_compaction_worker_impl(self.writer.clone(), self.index.clone(), config)
    }

    /// Query recent executions.
    ///
    /// Returns executions ordered by start time (most recent first).
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of executions to return
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let store = EventStore::from_env()?;
    /// let recent = store.recent_executions(10)?;
    /// for exec in recent {
    ///     println!("{}: completed={}, duration={}ms",
    ///         exec.execution_id, exec.completed, exec.duration_ms);
    /// }
    /// ```
    pub fn recent_executions(&self, limit: usize) -> EventStoreResult<Vec<ExecutionSummary>> {
        Ok(self.index.recent_executions(limit)?)
    }

    /// Get an execution by its ID.
    ///
    /// Returns `None` if no execution with the given ID exists.
    pub fn execution_by_id(&self, execution_id: &str) -> EventStoreResult<Option<ExecutionSummary>> {
        Ok(self.index.execution_by_id(execution_id)?)
    }

    /// Query executions by thread ID.
    ///
    /// Returns executions for the given thread, ordered by start time (most recent first).
    pub fn executions_by_thread(
        &self,
        thread_id: &str,
        limit: usize,
    ) -> EventStoreResult<Vec<ExecutionSummary>> {
        Ok(self.index.executions_by_thread(thread_id, limit)?)
    }

    /// Count total executions in the store.
    pub fn count(&self) -> EventStoreResult<i64> {
        Ok(self.index.count()?)
    }

    /// Get all events for an execution.
    ///
    /// Reads events from the WAL segment file associated with the execution.
    /// Returns events in chronological order (oldest first).
    ///
    /// # Arguments
    ///
    /// * `execution_id` - The execution ID to query
    ///
    /// # Returns
    ///
    /// Vector of WAL events for the execution, or an error if:
    /// - The execution is not found in the index
    /// - The segment file is not found or unreadable
    /// - Events fail to parse
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let store = EventStore::from_env()?;
    /// let events = store.execution_events("exec-123")?;
    /// for event in events {
    ///     println!("{:?}: {:?}", event.event_type, event.payload);
    /// }
    /// ```
    pub fn execution_events(&self, execution_id: &str) -> EventStoreResult<Vec<WALEvent>> {
        // First, look up the execution in the index
        let summary = self
            .index
            .execution_by_id(execution_id)?
            .ok_or_else(|| EventStoreError::ExecutionNotFound(execution_id.to_string()))?;

        let segments = self.writer.list_segments()?;
        let parquet_dir = parquet_dir_for_wal_dir(self.writer.wal_dir());
        let parquet_files = list_parquet_files(&parquet_dir)?;

        let parquet_stems: HashSet<String> = parquet_files
            .iter()
            .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string()))
            .collect();

        let mut sources: Vec<PathBuf> = Vec::new();
        if let Some(ref segment_path_str) = summary.segment_path {
            sources.push(PathBuf::from(segment_path_str));
        }
        sources.extend(segments.into_iter().filter(|segment| {
            let Some(stem) = segment.file_stem().and_then(|s| s.to_str()) else {
                return true;
            };
            !parquet_stems.contains(stem)
        }));
        sources.extend(parquet_files);

        let mut seen = HashSet::<PathBuf>::new();
        let mut events = Vec::new();

        for source in sources {
            if !seen.insert(source.clone()) {
                continue;
            }
            if !source.exists() {
                continue;
            }

            let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("parquet") {
                events.extend(read_events_from_parquet(&source, execution_id)?);
            } else {
                events.extend(read_events_from_segment(&source, execution_id)?);
            }
        }

        // Sort by timestamp to ensure chronological order
        events.sort_by_key(|e| e.timestamp_ms);

        Ok(events)
    }

    /// Flush any buffered data to disk.
    pub fn flush(&self) -> EventStoreResult<()> {
        self.writer.flush()?;
        Ok(())
    }

    /// Get a reference to the underlying WAL writer.
    pub fn writer(&self) -> &WALWriter {
        &self.writer
    }

    /// Get a reference to the underlying index.
    pub fn index(&self) -> &SqliteIndex {
        &self.index
    }
}

impl Clone for EventStore {
    /// Clone the event store.
    ///
    /// The cloned store shares the same WAL writer and SQLite index connection
    /// (both are thread-safe via internal synchronization).
    ///
    /// Note: The cloned store does NOT have a compaction worker. Only the
    /// original store manages the compaction worker lifecycle.
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            index: self.index.clone(), // Arc<Mutex<Connection>> - infallible clone
            compaction_handle: None,   // Cloned stores don't own compaction worker
        }
    }
}

impl Drop for EventStore {
    fn drop(&mut self) {
        // Stop the compaction worker if we own one
        if let Some(handle) = self.compaction_handle.take() {
            handle.stop_and_join();
        }
    }
}

impl std::fmt::Debug for EventStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStore")
            .field("writer", &self.writer)
            .field("index", &self.index)
            .field(
                "compaction_handle",
                &self.compaction_handle.as_ref().map(|_| "Some(CompactionHandle)"),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::Duration;
    use tempfile::TempDir;

    /// Mutex to serialize tests that manipulate DASHFLOW_WAL_COMPACTION_INTERVAL_SECS env var.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn test_config(temp: &TempDir) -> EventStoreConfig {
        EventStoreConfig {
            wal: WALWriterConfig {
                wal_dir: temp.path().join("wal"),
                max_segment_bytes: 1024,
                fsync_on_write: false,
                segment_extension: ".wal".to_string(),
            },
            index_path: temp.path().join("index.db"),
            auto_compaction: false, // Disable for tests to avoid background threads
        }
    }

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
    fn event_store_creates_directories() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        let _store = EventStore::new(config.clone()).unwrap();

        assert!(config.wal.wal_dir.exists());
        assert!(config.index_path.exists());
    }

    #[test]
    fn write_and_query_trace() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        let trace = test_trace("exec-123");
        store.write_trace(&trace).unwrap();
        store.flush().unwrap();

        // Query via recent_executions
        let recent = store.recent_executions(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].execution_id, "exec-123");
        assert!(recent[0].completed);
        assert_eq!(recent[0].duration_ms, 100);
        assert_eq!(recent[0].total_tokens, 50);
    }

    #[test]
    fn query_by_execution_id() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        store.write_trace(&test_trace("exec-456")).unwrap();

        let result = store.execution_by_id("exec-456").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().execution_id, "exec-456");

        let missing = store.execution_by_id("nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn query_by_thread_id() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        let mut trace1 = test_trace("exec-1");
        trace1.thread_id = Some("thread-A".to_string());
        store.write_trace(&trace1).unwrap();

        let mut trace2 = test_trace("exec-2");
        trace2.thread_id = Some("thread-A".to_string());
        store.write_trace(&trace2).unwrap();

        let mut trace3 = test_trace("exec-3");
        trace3.thread_id = Some("thread-B".to_string());
        store.write_trace(&trace3).unwrap();

        let thread_a = store.executions_by_thread("thread-A", 10).unwrap();
        assert_eq!(thread_a.len(), 2);

        let thread_b = store.executions_by_thread("thread-B", 10).unwrap();
        assert_eq!(thread_b.len(), 1);
    }

    #[test]
    fn recent_executions_respects_limit() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        for i in 0..20 {
            let mut trace = test_trace(&format!("exec-{i}"));
            trace.started_at = Some(format!("2025-01-01T00:00:{i:02}Z"));
            store.write_trace(&trace).unwrap();
        }

        let recent_5 = store.recent_executions(5).unwrap();
        assert_eq!(recent_5.len(), 5);

        let recent_100 = store.recent_executions(100).unwrap();
        assert_eq!(recent_100.len(), 20);
    }

    #[test]
    fn count_executions() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        assert_eq!(store.count().unwrap(), 0);

        store.write_trace(&test_trace("exec-1")).unwrap();
        store.write_trace(&test_trace("exec-2")).unwrap();
        store.write_trace(&test_trace("exec-3")).unwrap();

        assert_eq!(store.count().unwrap(), 3);
    }

    #[test]
    fn execution_events_returns_events_for_execution() {
        use super::super::writer::WALEventType;

        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        // Write a trace
        let trace = test_trace("exec-events-test");
        store.write_trace(&trace).unwrap();
        store.flush().unwrap();

        // Query events for this execution
        let events = store.execution_events("exec-events-test").unwrap();

        // Should have at least one event (the execution trace)
        assert!(!events.is_empty());
        assert!(events
            .iter()
            .all(|e| e.execution_id.as_deref() == Some("exec-events-test")));

        // Verify the event type is execution trace
        assert!(events
            .iter()
            .any(|e| e.event_type == WALEventType::ExecutionTrace));
    }

    #[test]
    fn execution_events_not_found() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        // Query events for non-existent execution
        let result = store.execution_events("nonexistent");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EventStoreError::ExecutionNotFound(_)
        ));
    }

    #[test]
    fn execution_events_isolates_by_execution_id() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        // Write multiple traces
        store.write_trace(&test_trace("exec-A")).unwrap();
        store.write_trace(&test_trace("exec-B")).unwrap();
        store.write_trace(&test_trace("exec-C")).unwrap();
        store.flush().unwrap();

        // Query events for exec-B only
        let events_b = store.execution_events("exec-B").unwrap();

        // All events should be for exec-B
        assert!(!events_b.is_empty());
        for event in &events_b {
            assert_eq!(event.execution_id.as_deref(), Some("exec-B"));
        }

        // Query events for exec-A
        let events_a = store.execution_events("exec-A").unwrap();
        assert!(!events_a.is_empty());
        for event in &events_a {
            assert_eq!(event.execution_id.as_deref(), Some("exec-A"));
        }
    }

    #[test]
    fn execution_events_reads_from_parquet_after_compaction() {
        let temp = TempDir::new().unwrap();
        let mut config = test_config(&temp);
        config.wal.max_segment_bytes = 1; // Force rotation on every write

        let store = EventStore::new(config).unwrap();

        store.write_trace(&test_trace("exec-1")).unwrap();
        store.write_trace(&test_trace("exec-2")).unwrap();
        store.flush().unwrap();

        let wal_dir = store.writer().wal_dir().to_path_buf();
        let parquet_dir = wal_dir.join("parquet");

        store
            .compact_once(&CompactionConfig {
                parquet_dir: parquet_dir.clone(),
                retention: Duration::from_secs(24 * 3600),
                interval: Duration::from_secs(1),
                min_segment_age: Duration::from_secs(0),
                delete_wal_after_compaction: true,
                batch_rows: 10,
            })
            .unwrap();

        // Oldest segment should be compacted and deleted; newest stays as active WAL.
        let wal_segments: Vec<_> = std::fs::read_dir(&wal_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("wal"))
            .collect();
        assert_eq!(wal_segments.len(), 1);

        let parquet_files: Vec<_> = std::fs::read_dir(&parquet_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("parquet"))
            .collect();
        assert_eq!(parquet_files.len(), 1);

        let exec_1 = store.execution_by_id("exec-1").unwrap().unwrap();
        assert!(exec_1
            .segment_path
            .as_deref()
            .is_some_and(|p| p.ends_with(".parquet")));

        let exec_2 = store.execution_by_id("exec-2").unwrap().unwrap();
        assert!(exec_2
            .segment_path
            .as_deref()
            .is_some_and(|p| p.ends_with(".wal")));

        let events_1 = store.execution_events("exec-1").unwrap();
        assert!(!events_1.is_empty());
        assert!(events_1.iter().all(|e| e.execution_id.as_deref() == Some("exec-1")));

        let events_2 = store.execution_events("exec-2").unwrap();
        assert!(!events_2.is_empty());
        assert!(events_2.iter().all(|e| e.execution_id.as_deref() == Some("exec-2")));
    }

    #[test]
    fn auto_compaction_disabled_by_default_in_test_config() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Test config has auto_compaction disabled
        assert!(!config.auto_compaction);

        // Store should have no compaction handle
        let store = EventStore::new(config).unwrap();
        assert!(store.compaction_handle.is_none());
    }

    /// Combined test for auto-compaction behavior with env var manipulation.
    /// Uses mutex to prevent race conditions when tests run in parallel.
    #[test]
    fn auto_compaction_and_clone_behavior() {
        let _guard = ENV_MUTEX.lock().unwrap();

        // Save original value
        let original = std::env::var("DASHFLOW_WAL_COMPACTION_INTERVAL_SECS").ok();

        // Use short interval env var to speed up test shutdown
        std::env::set_var("DASHFLOW_WAL_COMPACTION_INTERVAL_SECS", "1");

        // ---- Test: auto_compaction_enabled_creates_worker ----
        {
            let temp = TempDir::new().unwrap();
            let mut config = test_config(&temp);
            config.auto_compaction = true;

            let store = EventStore::new(config).unwrap();

            // Store should have a compaction handle
            assert!(store.compaction_handle.is_some());

            // Drop should cleanly stop the worker (within ~1 second)
            drop(store);
        }

        // ---- Test: clone_does_not_share_compaction_handle ----
        {
            let temp = TempDir::new().unwrap();
            let mut config = test_config(&temp);
            config.auto_compaction = true;

            let store = EventStore::new(config).unwrap();
            assert!(store.compaction_handle.is_some());

            let cloned = store.clone();
            // Cloned store should NOT have compaction handle
            assert!(cloned.compaction_handle.is_none());

            // Both stores can write and read (they share the WAL writer and index connection)
            store.write_trace(&test_trace("exec-orig")).unwrap();
            cloned.write_trace(&test_trace("exec-clone")).unwrap();

            // Both stores see all writes (shared index connection via Arc<Mutex>)
            assert!(store.execution_by_id("exec-orig").unwrap().is_some());
            assert!(cloned.execution_by_id("exec-clone").unwrap().is_some());
            // Cross-visibility: each store can see the other's writes
            assert!(store.execution_by_id("exec-clone").unwrap().is_some());
            assert!(cloned.execution_by_id("exec-orig").unwrap().is_some());
        }

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("DASHFLOW_WAL_COMPACTION_INTERVAL_SECS", val);
        } else {
            std::env::remove_var("DASHFLOW_WAL_COMPACTION_INTERVAL_SECS");
        }
    }

    #[test]
    fn without_compaction_builder_method() {
        let config = EventStoreConfig::default().without_compaction();
        assert!(!config.auto_compaction);
    }
}
