// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Write-Ahead Log (WAL) for DashFlow Observability
//!
//! This module provides persistent telemetry storage that survives process restarts.
//! Events are written to append-only segment files, with an SQLite index for fast queries.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │ GraphEvents  │────▶│   WALWriter     │────▶│ ~/.dashflow/wal/│
//! │ (in-memory)  │     │                 │     │   segments/     │
//! └──────────────┘     └────────┬────────┘     └─────────────────┘
//!                               │
//!                               ▼
//!                      ┌─────────────────┐
//!                      │ ~/.dashflow/    │
//!                      │   index.db      │
//!                      └─────────────────┘
//! ```
//!
//! # Segment Files
//!
//! Events are written to segment files in `~/.dashflow/wal/`. Each segment:
//! - Contains events as newline-delimited JSON (NDJSON)
//! - Is named by creation timestamp: `{timestamp_ms}.wal`
//! - Rolls over when size exceeds `max_segment_bytes` (default: 10MB)
//! - Is fsync'd after each write for durability
//!
//! # SQLite Index
//!
//! The index (`~/.dashflow/index.db`) stores:
//! - Execution metadata (ID, graph name, start time, end time, status)
//! - Segment file references (which segment contains which execution)
//! - Quick lookups for recent executions
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow::wal::{WALWriter, WALWriterConfig};
//!
//! // Create writer with default config (~/.dashflow/wal/)
//! let writer = WALWriter::new(WALWriterConfig::default())?;
//!
//! // Write events
//! writer.write_event(&event)?;
//!
//! // Query recent executions
//! let recent = writer.recent_executions(10)?;
//! ```
//!
//! # Configuration
//!
//! - `DASHFLOW_WAL=false` - Disable WAL (default: enabled)
//! - `DASHFLOW_WAL_DIR=/path` - Custom WAL directory
//! - `DASHFLOW_WAL_MAX_SEGMENT_MB=10` - Max segment size in MB
//! - `DASHFLOW_WAL_PARQUET_DIR=/path` - Custom Parquet output directory (default: `$DASHFLOW_WAL_DIR/parquet/`)
//! - `DASHFLOW_WAL_RETENTION_HOURS=24` - Retention window for Parquet + index (default: 24h)
//! - `DASHFLOW_WAL_COMPACTION_INTERVAL_SECS=60` - Background compaction interval (default: 60s)
//! - `DASHFLOW_WAL_COMPACTION_MIN_SEGMENT_AGE_SECS=30` - Minimum segment age before compaction (default: 30s)
//! - `DASHFLOW_WAL_COMPACTION_DELETE_WAL=true` - Delete `.wal` segments after successful compaction (default: true)
//! - `DASHFLOW_WAL_COMPACTION_BATCH_ROWS=10000` - Parquet write batch size in rows (default: 10k)
//!
//! # Background Tasks
//!
//! The WAL system includes background tasks for:
//! - Segment compaction (WAL → Parquet)
//! - Old segment cleanup (default: 7 days)
//! - Index vacuuming

mod callback;
mod index;
mod store;
mod writer;
mod compaction;
mod corpus;

pub use callback::{WALEventCallback, WALTelemetrySink};
pub use index::{ExecutionSummary, IndexError, IndexResult, SqliteIndex};
pub use store::{EventStore, EventStoreConfig, EventStoreError, EventStoreResult};
pub use writer::{SegmentWriter, WALEvent, WALEventType, WALWriter, WALWriterConfig, WALWriterError};
pub use compaction::{compact_once, spawn_compaction_worker, CompactionConfig, CompactionError, CompactionHandle, CompactionStats};
pub use corpus::{DecisionOutcome, DecisionTypeStats, ExecutionWithDecisions, LearningCorpus};

use crate::core::config_loader::env_vars::{env_bool, env_string, DASHFLOW_INDEX_PATH, DASHFLOW_WAL, DASHFLOW_WAL_DIR};

/// Check if WAL persistence is enabled.
///
/// WAL is ON by default (per DESIGN_INVARIANTS.md Invariant 6).
/// Opt-out by setting `DASHFLOW_WAL=false`.
pub fn is_wal_enabled() -> bool {
    env_bool(DASHFLOW_WAL, true)
}

/// Get the WAL directory path.
///
/// Default: `~/.dashflow/wal/`
/// Override with `DASHFLOW_WAL_DIR` environment variable.
pub fn wal_directory() -> std::path::PathBuf {
    if let Some(dir) = env_string(DASHFLOW_WAL_DIR) {
        return std::path::PathBuf::from(dir);
    }

    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".dashflow")
        .join("wal")
}

/// Get the SQLite index path.
///
/// Default: `~/.dashflow/index.db`
/// Override with `DASHFLOW_INDEX_PATH` environment variable.
pub fn index_path() -> std::path::PathBuf {
    if let Some(path) = env_string(DASHFLOW_INDEX_PATH) {
        return std::path::PathBuf::from(path);
    }

    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".dashflow")
        .join("index.db")
}

use std::sync::OnceLock;

/// Global EventStore singleton for trace persistence.
///
/// PERF-002 FIX: Creating a new EventStore per trace write was causing ~100ms overhead
/// per graph invocation due to SQLite connection setup. This singleton ensures we only
/// pay the initialization cost once.
///
/// The EventStore is lazily initialized on first access with compaction disabled
/// (compaction runs via CLI or explicit API calls).
static GLOBAL_EVENT_STORE: OnceLock<Result<EventStore, EventStoreError>> = OnceLock::new();

/// Get or initialize the global EventStore for trace persistence.
///
/// Returns `None` if WAL is disabled or initialization fails.
/// Uses `OnceLock` to ensure thread-safe lazy initialization.
///
/// # Performance
///
/// First call pays ~100ms initialization cost (SQLite setup).
/// Subsequent calls return immediately (~10ns).
pub fn global_event_store() -> Option<&'static EventStore> {
    if !is_wal_enabled() {
        return None;
    }

    GLOBAL_EVENT_STORE
        .get_or_init(|| {
            let config = EventStoreConfig::from_env().without_compaction();
            EventStore::new(config).map_err(|e| {
                tracing::warn!(
                    error = %e,
                    "Failed to initialize global EventStore for trace persistence"
                );
                e
            })
        })
        .as_ref()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize env-var-dependent tests (parallel execution causes races)
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn wal_enabled_by_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("DASHFLOW_WAL");
        assert!(is_wal_enabled());
    }

    #[test]
    fn wal_disabled_with_false() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_WAL", "false");
        let result = is_wal_enabled();
        std::env::remove_var("DASHFLOW_WAL");
        assert!(!result);
    }

    #[test]
    fn wal_disabled_with_zero() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_WAL", "0");
        let result = is_wal_enabled();
        std::env::remove_var("DASHFLOW_WAL");
        assert!(!result);
    }

    #[test]
    fn wal_directory_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("DASHFLOW_WAL_DIR");
        let dir = wal_directory();
        assert!(dir.to_string_lossy().contains(".dashflow"));
        assert!(dir.to_string_lossy().ends_with("wal"));
    }

    #[test]
    fn wal_directory_custom() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_WAL_DIR", "/custom/path");
        let dir = wal_directory();
        std::env::remove_var("DASHFLOW_WAL_DIR");
        assert_eq!(dir, std::path::PathBuf::from("/custom/path"));
    }

    #[test]
    fn index_path_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("DASHFLOW_INDEX_PATH");
        let path = index_path();
        assert!(path.to_string_lossy().contains(".dashflow"));
        assert!(path.to_string_lossy().ends_with("index.db"));
    }

    #[test]
    fn index_path_custom() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DASHFLOW_INDEX_PATH", "/custom/index.db");
        let path = index_path();
        std::env::remove_var("DASHFLOW_INDEX_PATH");
        assert_eq!(path, std::path::PathBuf::from("/custom/index.db"));
    }
}
