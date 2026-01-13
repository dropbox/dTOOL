// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! WAL segment compaction (NDJSON → Parquet) with retention.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrow::array::{ArrayBuilder, ArrayRef, Int64Builder, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use thiserror::Error;
use tracing::{debug, warn};

use super::index::{IndexError, SqliteIndex};
use super::writer::{WALEvent, WALWriter, WALWriterError};
use crate::core::config_loader::env_vars::{
    env_bool, env_string, env_u64, DASHFLOW_WAL_COMPACTION_BATCH_ROWS,
    DASHFLOW_WAL_COMPACTION_DELETE_WAL, DASHFLOW_WAL_COMPACTION_INTERVAL_SECS,
    DASHFLOW_WAL_COMPACTION_MIN_SEGMENT_AGE_SECS, DASHFLOW_WAL_PARQUET_DIR,
    DASHFLOW_WAL_RETENTION_HOURS,
};

/// Configuration for WAL compaction.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Directory where Parquet files are written.
    pub parquet_dir: PathBuf,
    /// How long to retain data before deletion.
    pub retention: Duration,
    /// How often to run compaction.
    pub interval: Duration,
    /// Minimum age of a segment before compacting.
    pub min_segment_age: Duration,
    /// Whether to delete WAL files after successful compaction.
    pub delete_wal_after_compaction: bool,
    /// Number of rows per Parquet batch.
    pub batch_rows: usize,
}

impl CompactionConfig {
    /// Create configuration from environment variables with defaults.
    #[must_use]
    pub fn from_env(wal_dir: &Path) -> Self {
        let parquet_dir = env_string(DASHFLOW_WAL_PARQUET_DIR)
            .map(PathBuf::from)
            .unwrap_or_else(|| wal_dir.join("parquet"));

        let retention_hours = env_u64(DASHFLOW_WAL_RETENTION_HOURS, 24);
        let interval_secs = env_u64(DASHFLOW_WAL_COMPACTION_INTERVAL_SECS, 60);
        let min_age_secs = env_u64(DASHFLOW_WAL_COMPACTION_MIN_SEGMENT_AGE_SECS, 30);
        let delete_wal_after_compaction =
            env_bool(DASHFLOW_WAL_COMPACTION_DELETE_WAL, true);
        let batch_rows = env_u64(DASHFLOW_WAL_COMPACTION_BATCH_ROWS, 10_000) as usize;

        Self {
            parquet_dir,
            retention: Duration::from_secs(retention_hours * 3600),
            interval: Duration::from_secs(interval_secs),
            min_segment_age: Duration::from_secs(min_age_secs),
            delete_wal_after_compaction,
            batch_rows: batch_rows.max(1),
        }
    }

    /// Create a new config with explicit values (no environment variable lookup).
    ///
    /// This is useful for testing or when environment-based configuration is not desired.
    #[must_use]
    pub fn new(parquet_dir: impl Into<PathBuf>) -> Self {
        Self {
            parquet_dir: parquet_dir.into(),
            retention: Duration::from_secs(24 * 3600), // 24 hours
            interval: Duration::from_secs(60),
            min_segment_age: Duration::from_secs(30),
            delete_wal_after_compaction: true,
            batch_rows: 10_000,
        }
    }

    /// Builder: set data retention duration.
    #[must_use]
    pub fn with_retention(mut self, retention: Duration) -> Self {
        self.retention = retention;
        self
    }

    /// Builder: set compaction interval.
    #[must_use]
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Builder: set minimum segment age before compaction.
    #[must_use]
    pub fn with_min_segment_age(mut self, age: Duration) -> Self {
        self.min_segment_age = age;
        self
    }

    /// Builder: set whether to delete WAL files after compaction.
    #[must_use]
    pub fn with_delete_wal_after_compaction(mut self, delete: bool) -> Self {
        self.delete_wal_after_compaction = delete;
        self
    }

    /// Builder: set the number of rows per Parquet batch.
    #[must_use]
    pub fn with_batch_rows(mut self, rows: usize) -> Self {
        self.batch_rows = rows.max(1); // Ensure at least 1 row per batch
        self
    }

    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or empty if valid.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.interval.is_zero() {
            errors.push("interval cannot be zero".to_string());
        }

        if self.batch_rows == 0 {
            errors.push("batch_rows must be > 0".to_string());
        }

        if self.retention < self.interval {
            errors.push(format!(
                "retention ({:?}) should be >= interval ({:?})",
                self.retention, self.interval
            ));
        }

        errors
    }
}

impl Default for CompactionConfig {
    fn default() -> Self {
        let wal_dir = super::wal_directory();
        Self::from_env(&wal_dir)
    }
}

/// Statistics from a compaction run.
#[derive(Debug, Clone, Default)]
pub struct CompactionStats {
    /// Number of WAL segments compacted to Parquet.
    pub segments_compacted: usize,
    /// Number of WAL segment files deleted.
    pub wal_segments_deleted: usize,
    /// Number of Parquet files written.
    pub parquet_files_written: usize,
    /// Number of old Parquet files deleted (retention cleanup).
    pub parquet_files_deleted: usize,
    /// Number of index rows deleted (retention cleanup).
    pub index_rows_deleted: usize,
    /// Number of index rows updated with new file paths.
    pub index_rows_rewritten: usize,
}

/// Errors that can occur during WAL compaction.
#[derive(Debug, Error)]
pub enum CompactionError {
    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Error from the WAL writer.
    #[error("WAL error: {0}")]
    Wal(#[from] WALWriterError),

    /// JSON parsing error when reading WAL segments.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Apache Arrow error during columnar conversion.
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// Parquet error when writing compacted files.
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    /// Index error when updating the SQLite index.
    #[error("Index error: {0}")]
    Index(#[from] IndexError),
}

/// Handle to control a background compaction worker.
pub struct CompactionHandle {
    /// Flag to signal the worker to stop.
    stop: Arc<AtomicBool>,
    /// Thread handle for joining.
    join: Option<thread::JoinHandle<()>>,
}

impl CompactionHandle {
    /// Stop the compaction worker and wait for it to finish.
    pub fn stop_and_join(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

/// Spawn a background thread that periodically runs compaction.
///
/// The worker converts WAL segments to Parquet format and cleans up
/// old data according to the retention policy.
pub fn spawn_compaction_worker(
    writer: WALWriter,
    index: SqliteIndex,
    config: CompactionConfig,
) -> CompactionHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);

    let join = thread::spawn(move || {
        while !stop_clone.load(Ordering::Relaxed) {
            if let Err(e) = compact_once(&writer, &index, &config) {
                warn!(error = %e, "WAL compaction failed");
            }
            thread::sleep(config.interval);
        }
    });

    CompactionHandle {
        stop,
        join: Some(join),
    }
}

/// Run one compaction cycle.
///
/// This converts eligible WAL segments to Parquet, updates the index,
/// and cleans up data older than the retention period.
///
/// # Errors
///
/// Returns error if I/O, Parquet writing, or index update fails.
pub fn compact_once(
    writer: &WALWriter,
    index: &SqliteIndex,
    config: &CompactionConfig,
) -> Result<CompactionStats, CompactionError> {
    fs::create_dir_all(&config.parquet_dir)?;

    let now_ms = unix_now_ms();
    let active_segment = writer.current_segment_path()?;

    let mut stats = CompactionStats::default();

    for segment_path in writer.list_segments()? {
        if active_segment
            .as_ref()
            .is_some_and(|active| same_file(active, &segment_path))
        {
            continue;
        }

        let segment_created_ms =
            parse_segment_timestamp_ms(&segment_path).unwrap_or_else(|| {
                file_modified_ms(&segment_path).unwrap_or(now_ms)
            });

        if now_ms.saturating_sub(segment_created_ms) < config.min_segment_age.as_millis() as u64 {
            continue;
        }

        let parquet_path = parquet_path_for_segment(&config.parquet_dir, &segment_path);

        if parquet_path.exists() {
            if config.delete_wal_after_compaction {
                let parquet_len = parquet_path.metadata().map(|m| m.len()).unwrap_or(0);
                if parquet_len > 0 {
                    fs::remove_file(&segment_path)?;
                    stats.wal_segments_deleted += 1;
                }
            }
            continue;
        }

        let rows_written = write_parquet_from_segment(&segment_path, &parquet_path, config)?;
        if rows_written == 0 {
            continue;
        }

        let old_path = segment_path.to_string_lossy();
        let new_path = parquet_path.to_string_lossy();
        stats.index_rows_rewritten += index.rewrite_segment_path(old_path.as_ref(), new_path.as_ref())?;

        stats.segments_compacted += 1;
        stats.parquet_files_written += 1;

        if config.delete_wal_after_compaction {
            fs::remove_file(&segment_path)?;
            stats.wal_segments_deleted += 1;
        }
    }

    stats.parquet_files_deleted = cleanup_old_parquet_files(&config.parquet_dir, config.retention)?;

    let cutoff_ms = now_ms.saturating_sub(config.retention.as_millis() as u64) as i64;
    stats.index_rows_deleted = index.delete_older_than(cutoff_ms)?;

    Ok(stats)
}

fn write_parquet_from_segment(
    segment_path: &Path,
    parquet_path: &Path,
    config: &CompactionConfig,
) -> Result<usize, CompactionError> {
    let file = File::open(segment_path)?;
    let reader = BufReader::new(file);

    let schema = Arc::new(Schema::new(vec![
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("event_type", DataType::Utf8, false),
        Field::new("execution_id", DataType::Utf8, true),
        Field::new("payload_json", DataType::Utf8, false),
    ]));

    let props = WriterProperties::builder().build();

    let out = File::create(parquet_path)?;
    let mut parquet_writer = ArrowWriter::try_new(out, Arc::clone(&schema), Some(props))?;

    let mut timestamp_builder = Int64Builder::new();
    let mut event_type_builder = StringBuilder::new();
    let mut execution_id_builder = StringBuilder::new();
    let mut payload_builder = StringBuilder::new();

    let mut total_rows = 0usize;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let event: WALEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        timestamp_builder.append_value(event.timestamp_ms as i64);
        event_type_builder.append_value(event.event_type.as_str());
        execution_id_builder.append_option(event.execution_id.as_deref());
        payload_builder.append_value(event.payload.to_string());

        total_rows += 1;

        if total_rows % config.batch_rows == 0 {
            flush_batch(
                Arc::clone(&schema),
                &mut parquet_writer,
                &mut timestamp_builder,
                &mut event_type_builder,
                &mut execution_id_builder,
                &mut payload_builder,
            )?;
        }
    }

    if total_rows % config.batch_rows != 0 {
        flush_batch(
            schema,
            &mut parquet_writer,
            &mut timestamp_builder,
            &mut event_type_builder,
            &mut execution_id_builder,
            &mut payload_builder,
        )?;
    }

    parquet_writer.close()?;
    debug!(
        segment = %segment_path.display(),
        parquet = %parquet_path.display(),
        rows = total_rows,
        "Compacted WAL segment to Parquet"
    );

    Ok(total_rows)
}

fn flush_batch(
    schema: Arc<Schema>,
    parquet_writer: &mut ArrowWriter<File>,
    timestamp_builder: &mut Int64Builder,
    event_type_builder: &mut StringBuilder,
    execution_id_builder: &mut StringBuilder,
    payload_builder: &mut StringBuilder,
) -> Result<(), CompactionError> {
    if timestamp_builder.len() == 0 {
        return Ok(());
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(timestamp_builder.finish()),
        Arc::new(event_type_builder.finish()),
        Arc::new(execution_id_builder.finish()),
        Arc::new(payload_builder.finish()),
    ];

    let batch = RecordBatch::try_new(schema, columns)?;
    parquet_writer.write(&batch)?;
    Ok(())
}

fn cleanup_old_parquet_files(parquet_dir: &Path, retention: Duration) -> Result<usize, CompactionError> {
    let now_ms = unix_now_ms();
    let retention_ms = retention.as_millis() as u64;
    let mut deleted = 0usize;

    let entries = match fs::read_dir(parquet_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_parquet = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("parquet"));
        if !is_parquet {
            continue;
        }

        let created_ms = parse_segment_timestamp_ms(&path)
            .or_else(|| file_modified_ms(&path))
            .unwrap_or(now_ms);

        if now_ms.saturating_sub(created_ms) > retention_ms {
            fs::remove_file(&path)?;
            deleted += 1;
        }
    }

    Ok(deleted)
}

fn parquet_path_for_segment(parquet_dir: &Path, segment_path: &Path) -> PathBuf {
    let stem = segment_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("segment");
    parquet_dir.join(format!("{stem}.parquet"))
}

fn parse_segment_timestamp_ms(path: &Path) -> Option<u64> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.parse::<u64>().ok())
}

fn file_modified_ms(path: &Path) -> Option<u64> {
    let modified = path.metadata().ok()?.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || (a.canonicalize().ok().is_some_and(|ca| {
            b.canonicalize().ok().is_some_and(|cb| ca == cb)
        }))
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_segment(dir: &Path, name: &str, events: &[WALEvent]) -> PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).unwrap();
        for event in events {
            let line = serde_json::to_string(event).unwrap();
            writeln!(file, "{}", line).unwrap();
        }
        path
    }

    fn test_event(execution_id: &str, timestamp_ms: u64) -> WALEvent {
        WALEvent {
            timestamp_ms,
            event_type: super::super::writer::WALEventType::ExecutionStart,
            execution_id: Some(execution_id.to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            payload: serde_json::json!({"test": true}),
        }
    }

    #[test]
    fn parse_segment_timestamp_ms_extracts_timestamp() {
        let path = Path::new("/some/path/1735123456789.wal");
        assert_eq!(parse_segment_timestamp_ms(path), Some(1735123456789));
    }

    #[test]
    fn parse_segment_timestamp_ms_returns_none_for_invalid() {
        let path = Path::new("/some/path/invalid.wal");
        assert_eq!(parse_segment_timestamp_ms(path), None);
    }

    #[test]
    fn parquet_path_for_segment_changes_extension() {
        let parquet_dir = Path::new("/parquet");
        let segment = Path::new("/wal/1735123456789.wal");
        let result = parquet_path_for_segment(parquet_dir, segment);
        assert_eq!(result, PathBuf::from("/parquet/1735123456789.parquet"));
    }

    #[test]
    fn same_file_detects_identical_paths() {
        let path = Path::new("/some/path/file.txt");
        assert!(same_file(path, path));
    }

    #[test]
    fn write_parquet_from_segment_converts_events() {
        let temp = TempDir::new().unwrap();
        let wal_dir = temp.path().join("wal");
        let parquet_dir = temp.path().join("parquet");
        fs::create_dir_all(&wal_dir).unwrap();
        fs::create_dir_all(&parquet_dir).unwrap();

        let events = vec![
            test_event("exec-1", 1000),
            test_event("exec-1", 2000),
            test_event("exec-2", 3000),
        ];
        let segment_path = create_test_segment(&wal_dir, "1000.wal", &events);
        let parquet_path = parquet_dir.join("1000.parquet");

        let config = CompactionConfig {
            parquet_dir: parquet_dir.clone(),
            retention: Duration::from_secs(3600),
            interval: Duration::from_secs(60),
            min_segment_age: Duration::from_secs(0),
            delete_wal_after_compaction: false,
            batch_rows: 10,
        };

        let rows_written = write_parquet_from_segment(&segment_path, &parquet_path, &config).unwrap();
        assert_eq!(rows_written, 3);
        assert!(parquet_path.exists());
    }

    #[test]
    fn cleanup_old_parquet_files_removes_expired() {
        let temp = TempDir::new().unwrap();
        let parquet_dir = temp.path().join("parquet");
        fs::create_dir_all(&parquet_dir).unwrap();

        // Create an "old" parquet file with a very old timestamp in the name
        let old_path = parquet_dir.join("1000000000000.parquet"); // Jan 2001
        File::create(&old_path).unwrap();

        // Create a "new" parquet file with current timestamp
        let now_ms = unix_now_ms();
        let new_path = parquet_dir.join(format!("{}.parquet", now_ms));
        File::create(&new_path).unwrap();

        let retention = Duration::from_secs(3600); // 1 hour
        let deleted = cleanup_old_parquet_files(&parquet_dir, retention).unwrap();

        assert_eq!(deleted, 1);
        assert!(!old_path.exists());
        assert!(new_path.exists());
    }

    #[test]
    fn compact_once_skips_active_segment() {
        let temp = TempDir::new().unwrap();
        let wal_dir = temp.path().join("wal");
        let parquet_dir = temp.path().join("parquet");
        fs::create_dir_all(&wal_dir).unwrap();

        let writer_config = super::super::writer::WALWriterConfig {
            wal_dir: wal_dir.clone(),
            max_segment_bytes: 10 * 1024 * 1024,
            fsync_on_write: false,
            segment_extension: ".wal".to_string(),
        };
        let writer = WALWriter::new(writer_config).unwrap();
        let index = SqliteIndex::in_memory().unwrap();

        // Write an event to create an active segment
        let event = test_event("exec-1", unix_now_ms());
        writer.write_event(&event).unwrap();
        writer.flush().unwrap();

        let config = CompactionConfig {
            parquet_dir: parquet_dir.clone(),
            retention: Duration::from_secs(3600),
            interval: Duration::from_secs(60),
            min_segment_age: Duration::from_secs(0),
            delete_wal_after_compaction: true,
            batch_rows: 10,
        };

        let stats = compact_once(&writer, &index, &config).unwrap();

        // Active segment should NOT be compacted
        assert_eq!(stats.segments_compacted, 0);
        assert_eq!(stats.wal_segments_deleted, 0);
    }

    #[test]
    fn compaction_config_default_values() {
        let config = CompactionConfig::default();
        assert_eq!(config.retention.as_secs(), 24 * 3600);
        assert_eq!(config.interval.as_secs(), 60);
        assert_eq!(config.min_segment_age.as_secs(), 30);
        assert!(config.delete_wal_after_compaction);
        assert_eq!(config.batch_rows, 10_000);
    }

    #[test]
    fn compaction_stats_default() {
        let stats = CompactionStats::default();
        assert_eq!(stats.segments_compacted, 0);
        assert_eq!(stats.wal_segments_deleted, 0);
        assert_eq!(stats.parquet_files_written, 0);
        assert_eq!(stats.parquet_files_deleted, 0);
        assert_eq!(stats.index_rows_deleted, 0);
        assert_eq!(stats.index_rows_rewritten, 0);
    }

    // Tests for CompactionConfig builder pattern

    #[test]
    fn compaction_config_new() {
        let config = CompactionConfig::new("/tmp/parquet");
        assert_eq!(config.parquet_dir, PathBuf::from("/tmp/parquet"));
        assert_eq!(config.retention.as_secs(), 24 * 3600);
        assert_eq!(config.interval.as_secs(), 60);
        assert_eq!(config.min_segment_age.as_secs(), 30);
        assert!(config.delete_wal_after_compaction);
        assert_eq!(config.batch_rows, 10_000);
    }

    #[test]
    fn compaction_config_builder_chain() {
        let config = CompactionConfig::new("/tmp/parquet")
            .with_retention(Duration::from_secs(3600))
            .with_interval(Duration::from_secs(120))
            .with_min_segment_age(Duration::from_secs(60))
            .with_delete_wal_after_compaction(false)
            .with_batch_rows(5000);

        assert_eq!(config.parquet_dir, PathBuf::from("/tmp/parquet"));
        assert_eq!(config.retention.as_secs(), 3600);
        assert_eq!(config.interval.as_secs(), 120);
        assert_eq!(config.min_segment_age.as_secs(), 60);
        assert!(!config.delete_wal_after_compaction);
        assert_eq!(config.batch_rows, 5000);
    }

    #[test]
    fn compaction_config_batch_rows_minimum() {
        // batch_rows should be at least 1
        let config = CompactionConfig::new("/tmp/parquet").with_batch_rows(0);
        assert_eq!(config.batch_rows, 1);
    }

    #[test]
    fn compaction_config_validate_valid() {
        let config = CompactionConfig::new("/tmp/parquet");
        let errors = config.validate();
        assert!(errors.is_empty(), "Default config should be valid");
    }

    #[test]
    fn compaction_config_validate_zero_interval() {
        let config = CompactionConfig::new("/tmp/parquet").with_interval(Duration::ZERO);
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("interval")));
    }

    #[test]
    fn compaction_config_validate_retention_less_than_interval() {
        let config = CompactionConfig::new("/tmp/parquet")
            .with_retention(Duration::from_secs(30))
            .with_interval(Duration::from_secs(60));
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("retention")));
    }
}
