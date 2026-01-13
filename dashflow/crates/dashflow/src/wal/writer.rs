// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! WAL Segment Writer
//!
//! Writes events to append-only segment files with fsync for durability.

use crate::core::config_loader::env_vars::{
    env_string, env_u64, DASHFLOW_WAL_DIR, DASHFLOW_WAL_MAX_SEGMENT_MB,
};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::introspection::ExecutionTrace;

/// Errors that can occur during WAL operations.
#[derive(Debug, Error)]
pub enum WALWriterError {
    /// I/O error during file operations
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Directory creation failed
    #[error("Failed to create directory: {path}")]
    DirectoryCreation {
        /// Path that failed to create
        path: PathBuf,
        /// Underlying error
        #[source]
        source: std::io::Error,
    },

    /// Segment file corrupted
    #[error("Segment file corrupted: {path}")]
    CorruptedSegment {
        /// Path to corrupted segment
        path: PathBuf,
    },

    /// Mutex was poisoned (another thread panicked while holding the lock)
    #[error("Mutex poisoned: {0}")]
    MutexPoisoned(String),
}

/// Configuration for WAL writer.
#[derive(Debug, Clone)]
pub struct WALWriterConfig {
    /// Directory for WAL segment files
    pub wal_dir: PathBuf,

    /// Maximum size of a single segment file in bytes (default: 10MB)
    pub max_segment_bytes: u64,

    /// Whether to fsync after each write (default: true)
    pub fsync_on_write: bool,

    /// Segment file extension (default: ".wal")
    pub segment_extension: String,
}

impl Default for WALWriterConfig {
    fn default() -> Self {
        Self {
            wal_dir: super::wal_directory(),
            max_segment_bytes: 10 * 1024 * 1024, // 10MB
            fsync_on_write: true,
            segment_extension: ".wal".to_string(),
        }
    }
}

impl WALWriterConfig {
    /// Create a new WAL writer configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create config from environment variables.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Some(dir) = env_string(DASHFLOW_WAL_DIR) {
            config.wal_dir = PathBuf::from(dir);
        }

        // Get max segment size in MB from env (default 10 MB)
        let mb = env_u64(DASHFLOW_WAL_MAX_SEGMENT_MB, 10);
        config.max_segment_bytes = mb * 1024 * 1024;

        config
    }

    /// Set the WAL directory path.
    #[must_use]
    pub fn with_wal_dir(mut self, dir: PathBuf) -> Self {
        self.wal_dir = dir;
        self
    }

    /// Set the maximum segment size in bytes.
    #[must_use]
    pub fn with_max_segment_bytes(mut self, bytes: u64) -> Self {
        self.max_segment_bytes = bytes;
        self
    }

    /// Set whether to fsync after each write.
    #[must_use]
    pub fn with_fsync_on_write(mut self, fsync: bool) -> Self {
        self.fsync_on_write = fsync;
        self
    }

    /// Set the segment file extension.
    #[must_use]
    pub fn with_segment_extension(mut self, ext: impl Into<String>) -> Self {
        self.segment_extension = ext.into();
        self
    }
}

/// A single WAL segment file.
#[derive(Debug)]
struct Segment {
    /// Path to the segment file
    path: PathBuf,
    /// Buffered writer for the segment
    writer: BufWriter<File>,
    /// Current size of the segment in bytes
    current_size: u64,
    /// Creation timestamp (milliseconds since epoch)
    #[allow(dead_code)] // Reserved for future: Will be used for segment age/cleanup in OBS-005
    created_at_ms: u64,
}

impl Segment {
    /// Create a new segment file.
    fn create(dir: &Path, extension: &str) -> Result<Self, WALWriterError> {
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let filename = format!("{created_at_ms}{extension}");
        let path = dir.join(filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        Ok(Self {
            path,
            writer: BufWriter::new(file),
            current_size: 0,
            created_at_ms,
        })
    }

    /// Write a line to the segment and return bytes written.
    fn write_line(&mut self, line: &str, fsync: bool) -> Result<u64, WALWriterError> {
        let bytes = line.as_bytes();
        self.writer.write_all(bytes)?;
        self.writer.write_all(b"\n")?;

        if fsync {
            self.writer.flush()?;
            self.writer.get_ref().sync_all()?;
        }

        let written = bytes.len() as u64 + 1; // +1 for newline
        self.current_size += written;
        Ok(written)
    }

    /// Flush any buffered data.
    fn flush(&mut self) -> Result<(), WALWriterError> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok(())
    }
}

/// WAL event wrapper with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WALEvent {
    /// Event timestamp (milliseconds since epoch)
    pub timestamp_ms: u64,

    /// Event type discriminator
    pub event_type: WALEventType,

    /// Execution ID this event belongs to
    pub execution_id: Option<String>,

    /// Parent execution ID (Observability Phase 3)
    ///
    /// For subgraph executions, links back to the parent graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_execution_id: Option<String>,

    /// Root execution ID (Observability Phase 3)
    ///
    /// For nested subgraph executions, links to the top-level graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_execution_id: Option<String>,

    /// Subgraph depth (Observability Phase 3)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,

    /// Event payload (JSON)
    pub payload: serde_json::Value,
}

/// Types of events stored in the WAL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WALEventType {
    /// Graph execution started
    ExecutionStart,
    /// Graph execution ended
    ExecutionEnd,
    /// Node started
    NodeStart,
    /// Node ended
    NodeEnd,
    /// Node error
    NodeError,
    /// Edge traversal
    EdgeTraversal,
    /// Edge condition evaluated (Observability Phase 3)
    EdgeEvaluated,
    /// State changed after node execution (Observability Phase 3)
    StateChanged,
    /// Agent decision made (Observability Phase 4)
    DecisionMade,
    /// Outcome observed for a decision (Observability Phase 4)
    OutcomeObserved,
    /// Full execution trace (for compatibility)
    ExecutionTrace,
    /// LLM API call completed (for learning/optimization)
    LlmCallCompleted,
}

impl WALEventType {
    /// Get the string representation of this event type.
    pub fn as_str(&self) -> &'static str {
        match self {
            WALEventType::ExecutionStart => "execution_start",
            WALEventType::ExecutionEnd => "execution_end",
            WALEventType::NodeStart => "node_start",
            WALEventType::NodeEnd => "node_end",
            WALEventType::NodeError => "node_error",
            WALEventType::EdgeTraversal => "edge_traversal",
            WALEventType::EdgeEvaluated => "edge_evaluated",
            WALEventType::StateChanged => "state_changed",
            WALEventType::DecisionMade => "decision_made",
            WALEventType::OutcomeObserved => "outcome_observed",
            WALEventType::ExecutionTrace => "execution_trace",
            WALEventType::LlmCallCompleted => "llm_call_completed",
        }
    }
}

impl std::str::FromStr for WALEventType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "execution_start" => Ok(WALEventType::ExecutionStart),
            "execution_end" => Ok(WALEventType::ExecutionEnd),
            "node_start" => Ok(WALEventType::NodeStart),
            "node_end" => Ok(WALEventType::NodeEnd),
            "node_error" => Ok(WALEventType::NodeError),
            "edge_traversal" => Ok(WALEventType::EdgeTraversal),
            "edge_evaluated" => Ok(WALEventType::EdgeEvaluated),
            "state_changed" => Ok(WALEventType::StateChanged),
            "decision_made" => Ok(WALEventType::DecisionMade),
            "outcome_observed" => Ok(WALEventType::OutcomeObserved),
            "execution_trace" => Ok(WALEventType::ExecutionTrace),
            "llm_call_completed" => Ok(WALEventType::LlmCallCompleted),
            _ => Err(()),
        }
    }
}

/// Segment writer that handles segment rotation.
#[derive(Debug)]
pub struct SegmentWriter {
    config: WALWriterConfig,
    current_segment: Option<Segment>,
}

impl SegmentWriter {
    /// Create a new segment writer.
    pub fn new(config: WALWriterConfig) -> Result<Self, WALWriterError> {
        // Ensure directory exists
        fs::create_dir_all(&config.wal_dir).map_err(|e| WALWriterError::DirectoryCreation {
            path: config.wal_dir.clone(),
            source: e,
        })?;

        Ok(Self {
            config,
            current_segment: None,
        })
    }

    /// Write an event to the current segment.
    pub fn write_event(&mut self, event: &WALEvent) -> Result<(), WALWriterError> {
        let line = serde_json::to_string(event)?;

        // Check if we need to rotate
        let need_rotation = match &self.current_segment {
            Some(seg) => seg.current_size + line.len() as u64 > self.config.max_segment_bytes,
            None => true,
        };

        if need_rotation {
            // Flush current segment if any
            if let Some(ref mut seg) = self.current_segment {
                seg.flush()?;
            }

            // Create new segment
            self.current_segment = Some(Segment::create(
                &self.config.wal_dir,
                &self.config.segment_extension,
            )?);
        }

        // Write to current segment
        if let Some(ref mut seg) = self.current_segment {
            seg.write_line(&line, self.config.fsync_on_write)?;
        }

        Ok(())
    }

    /// Write an execution trace as a single event.
    pub fn write_trace(&mut self, trace: &ExecutionTrace) -> Result<(), WALWriterError> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let event = WALEvent {
            timestamp_ms,
            event_type: WALEventType::ExecutionTrace,
            execution_id: trace.execution_id.clone(),
            parent_execution_id: trace.parent_execution_id.clone(),
            root_execution_id: trace.root_execution_id.clone(),
            depth: trace.depth,
            payload: serde_json::to_value(trace)?,
        };

        self.write_event(&event)
    }

    /// Flush and close the current segment.
    pub fn flush(&mut self) -> Result<(), WALWriterError> {
        if let Some(ref mut seg) = self.current_segment {
            seg.flush()?;
        }
        Ok(())
    }

    /// Get the current segment path (if any).
    pub fn current_segment_path(&self) -> Option<&Path> {
        self.current_segment.as_ref().map(|s| s.path.as_path())
    }

    /// List all segment files in the WAL directory.
    pub fn list_segments(&self) -> Result<Vec<PathBuf>, WALWriterError> {
        let mut segments = Vec::new();

        for entry in fs::read_dir(&self.config.wal_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let matches_extension = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.ends_with(&self.config.segment_extension))
                    .unwrap_or(false);

                if matches_extension {
                    segments.push(path);
                }
            }
        }

        // Sort by filename (which is timestamp)
        segments.sort();
        Ok(segments)
    }
}

/// Thread-safe WAL writer.
///
/// This is the main entry point for writing events to the WAL.
/// It wraps `SegmentWriter` with a mutex for thread safety.
#[derive(Debug)]
pub struct WALWriter {
    inner: Arc<Mutex<SegmentWriter>>,
    config: WALWriterConfig,
}

impl WALWriter {
    /// Create a new WAL writer with the given configuration.
    pub fn new(config: WALWriterConfig) -> Result<Self, WALWriterError> {
        let writer = SegmentWriter::new(config.clone())?;
        Ok(Self {
            inner: Arc::new(Mutex::new(writer)),
            config,
        })
    }

    /// Create a WAL writer with default configuration from environment.
    pub fn from_env() -> Result<Self, WALWriterError> {
        Self::new(WALWriterConfig::from_env())
    }

    /// Write an event to the WAL.
    pub fn write_event(&self, event: &WALEvent) -> Result<(), WALWriterError> {
        let mut writer = self
            .inner
            .lock()
            .map_err(|e| WALWriterError::MutexPoisoned(e.to_string()))?;
        writer.write_event(event)
    }

    /// Write an execution trace to the WAL.
    pub fn write_trace(&self, trace: &ExecutionTrace) -> Result<(), WALWriterError> {
        let mut writer = self
            .inner
            .lock()
            .map_err(|e| WALWriterError::MutexPoisoned(e.to_string()))?;
        writer.write_trace(trace)
    }

    /// Flush any buffered data.
    pub fn flush(&self) -> Result<(), WALWriterError> {
        let mut writer = self
            .inner
            .lock()
            .map_err(|e| WALWriterError::MutexPoisoned(e.to_string()))?;
        writer.flush()
    }

    /// Get the WAL directory.
    pub fn wal_dir(&self) -> &Path {
        &self.config.wal_dir
    }

    /// List all segment files.
    pub fn list_segments(&self) -> Result<Vec<PathBuf>, WALWriterError> {
        let writer = self
            .inner
            .lock()
            .map_err(|e| WALWriterError::MutexPoisoned(e.to_string()))?;
        writer.list_segments()
    }

    /// Get the currently-open segment path (if any).
    ///
    /// This is useful for background compaction/cleanup logic to avoid racing
    /// with active writes.
    pub fn current_segment_path(&self) -> Result<Option<PathBuf>, WALWriterError> {
        let writer = self
            .inner
            .lock()
            .map_err(|e| WALWriterError::MutexPoisoned(e.to_string()))?;
        Ok(writer.current_segment_path().map(|p| p.to_path_buf()))
    }

    /// Clone the writer (Arc clone).
    pub fn clone_ref(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            config: self.config.clone(),
        }
    }
}

impl Clone for WALWriter {
    fn clone(&self) -> Self {
        self.clone_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(dir: &Path) -> WALWriterConfig {
        WALWriterConfig {
            wal_dir: dir.to_path_buf(),
            max_segment_bytes: 1024, // Small for testing
            fsync_on_write: false,   // Faster tests
            segment_extension: ".wal".to_string(),
        }
    }

    #[test]
    fn segment_writer_creates_directory() {
        let temp = TempDir::new().unwrap();
        let wal_dir = temp.path().join("wal");

        let config = WALWriterConfig {
            wal_dir: wal_dir.clone(),
            ..Default::default()
        };

        let _writer = SegmentWriter::new(config).unwrap();
        assert!(wal_dir.exists());
    }

    #[test]
    fn segment_writer_creates_segment_on_first_write() {
        let temp = TempDir::new().unwrap();
        let config = test_config(temp.path());

        let mut writer = SegmentWriter::new(config).unwrap();

        let event = WALEvent {
            timestamp_ms: 1234567890,
            event_type: WALEventType::ExecutionStart,
            execution_id: Some("test-123".to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            payload: serde_json::json!({"test": "data"}),
        };

        writer.write_event(&event).unwrap();
        writer.flush().unwrap();

        let segments = writer.list_segments().unwrap();
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn segment_writer_rotates_on_size() {
        let temp = TempDir::new().unwrap();
        let config = WALWriterConfig {
            wal_dir: temp.path().to_path_buf(),
            max_segment_bytes: 100, // Very small to force rotation
            fsync_on_write: false,
            segment_extension: ".wal".to_string(),
        };

        let mut writer = SegmentWriter::new(config).unwrap();

        // Write multiple events to force rotation
        for i in 0..10 {
            let event = WALEvent {
                timestamp_ms: 1234567890 + i,
                event_type: WALEventType::ExecutionStart,
                execution_id: Some(format!("test-{i}")),
                parent_execution_id: None,
                root_execution_id: None,
                depth: Some(0),
                payload: serde_json::json!({"iteration": i, "data": "some longer data to fill segment"}),
            };
            writer.write_event(&event).unwrap();

            // Small delay to ensure different timestamps for segment names
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        writer.flush().unwrap();

        let segments = writer.list_segments().unwrap();
        assert!(segments.len() > 1, "Expected multiple segments after rotation");
    }

    #[test]
    fn wal_writer_is_thread_safe() {
        let temp = TempDir::new().unwrap();
        let config = test_config(temp.path());

        let writer = WALWriter::new(config).unwrap();
        let writer2 = writer.clone_ref();

        // Write from two "threads" (sequential for simplicity)
        let event1 = WALEvent {
            timestamp_ms: 1,
            event_type: WALEventType::ExecutionStart,
            execution_id: Some("thread1".to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            payload: serde_json::json!({}),
        };
        let event2 = WALEvent {
            timestamp_ms: 2,
            event_type: WALEventType::ExecutionStart,
            execution_id: Some("thread2".to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            payload: serde_json::json!({}),
        };

        writer.write_event(&event1).unwrap();
        writer2.write_event(&event2).unwrap();
        writer.flush().unwrap();

        let segments = writer.list_segments().unwrap();
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn wal_writer_write_trace() {
        let temp = TempDir::new().unwrap();
        let config = test_config(temp.path());

        let writer = WALWriter::new(config).unwrap();

        let trace = ExecutionTrace {
            thread_id: Some("test-thread".to_string()),
            execution_id: Some("exec-123".to_string()),
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
            metadata: std::collections::HashMap::new(),
            execution_metrics: None,
            performance_metrics: None,
        };

        writer.write_trace(&trace).unwrap();
        writer.flush().unwrap();

        let segments = writer.list_segments().unwrap();
        assert_eq!(segments.len(), 1);

        // Verify content
        let content = std::fs::read_to_string(&segments[0]).unwrap();
        assert!(content.contains("exec-123"));
        assert!(content.contains("execution_trace"));
    }

    #[test]
    fn wal_writer_config_new() {
        let config = WALWriterConfig::new();
        assert_eq!(config.max_segment_bytes, 10 * 1024 * 1024);
        assert!(config.fsync_on_write);
        assert_eq!(config.segment_extension, ".wal");
    }

    #[test]
    fn wal_writer_config_builder_wal_dir() {
        let config = WALWriterConfig::new().with_wal_dir(PathBuf::from("/tmp/custom"));
        assert_eq!(config.wal_dir, PathBuf::from("/tmp/custom"));
    }

    #[test]
    fn wal_writer_config_builder_max_segment_bytes() {
        let config = WALWriterConfig::new().with_max_segment_bytes(5 * 1024 * 1024);
        assert_eq!(config.max_segment_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn wal_writer_config_builder_fsync() {
        let config = WALWriterConfig::new().with_fsync_on_write(false);
        assert!(!config.fsync_on_write);
    }

    #[test]
    fn wal_writer_config_builder_segment_extension() {
        let config = WALWriterConfig::new().with_segment_extension(".log");
        assert_eq!(config.segment_extension, ".log");
    }

    #[test]
    fn wal_writer_config_builder_chaining() {
        let temp = TempDir::new().unwrap();
        let config = WALWriterConfig::new()
            .with_wal_dir(temp.path().to_path_buf())
            .with_max_segment_bytes(2048)
            .with_fsync_on_write(false)
            .with_segment_extension(".journal");

        assert_eq!(config.wal_dir, temp.path().to_path_buf());
        assert_eq!(config.max_segment_bytes, 2048);
        assert!(!config.fsync_on_write);
        assert_eq!(config.segment_extension, ".journal");
    }
}
