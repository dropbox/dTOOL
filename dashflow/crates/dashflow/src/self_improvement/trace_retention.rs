// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Trace retention policy for `.dashflow/traces/`.
//!
//! Data retention & cleanup
//!
//! Traces accumulate indefinitely without this module. The `RetentionPolicy` provides
//! configurable limits for:
//! - Maximum number of traces
//! - Maximum age of traces
//! - Maximum total size of traces directory
//!
//! ## Configuration via Environment Variables
//!
//! Following the DASHFLOW_ convention (per DESIGN_INVARIANTS.md):
//! - `DASHFLOW_TRACE_MAX_COUNT`: Maximum number of traces (default: 1000)
//! - `DASHFLOW_TRACE_MAX_AGE_DAYS`: Maximum age in days (default: 30)
//! - `DASHFLOW_TRACE_MAX_SIZE_MB`: Maximum total size in MB (default: 500)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::self_improvement::trace_retention::{RetentionPolicy, TraceRetentionManager};
//!
//! // Use default policy from environment
//! let policy = RetentionPolicy::from_env();
//! let manager = TraceRetentionManager::new(".dashflow/traces", policy);
//!
//! // Run cleanup
//! let stats = manager.cleanup()?;
//! println!("Deleted {} traces, freed {} bytes", stats.deleted_count, stats.freed_bytes);
//! ```

use crate::core::config_loader::env_vars::{
    env_bool, env_u64, env_usize, DASHFLOW_TRACE_COMPRESS_AGE_DAYS, DASHFLOW_TRACE_MAX_AGE_DAYS,
    DASHFLOW_TRACE_MAX_COUNT, DASHFLOW_TRACE_MAX_SIZE_MB, DASHFLOW_TRACE_RETENTION,
};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{debug, info};

/// Default maximum number of traces to retain.
pub const DEFAULT_MAX_TRACES: usize = 1000;

/// Default compression age in days (traces older than this get compressed).
pub const DEFAULT_COMPRESS_AGE_DAYS: u64 = 7;

/// Default maximum age in days.
pub const DEFAULT_MAX_AGE_DAYS: u64 = 30;

/// Default maximum total size in bytes (500 MB).
pub const DEFAULT_MAX_SIZE_BYTES: u64 = 500 * 1024 * 1024;

/// Retention policy configuration.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    /// Maximum number of traces to keep. Oldest traces are deleted first.
    pub max_traces: Option<usize>,
    /// Maximum age of traces. Traces older than this are deleted.
    pub max_age: Option<Duration>,
    /// Maximum total size of traces directory in bytes.
    pub max_size_bytes: Option<u64>,
    /// Age after which traces are compressed.
    /// Traces older than this are gzip-compressed to save space.
    pub compress_age: Option<Duration>,
    /// Whether retention is enabled. If false, no cleanup occurs.
    pub enabled: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_traces: Some(DEFAULT_MAX_TRACES),
            max_age: Some(Duration::from_secs(DEFAULT_MAX_AGE_DAYS * 24 * 60 * 60)),
            max_size_bytes: Some(DEFAULT_MAX_SIZE_BYTES),
            compress_age: Some(Duration::from_secs(
                DEFAULT_COMPRESS_AGE_DAYS * 24 * 60 * 60,
            )),
            enabled: true,
        }
    }
}

impl RetentionPolicy {
    /// Create a policy with no limits (retain everything).
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            max_traces: None,
            max_age: None,
            max_size_bytes: None,
            compress_age: None,
            enabled: true,
        }
    }

    /// Create a policy from environment variables.
    ///
    /// Reads:
    /// - `DASHFLOW_TRACE_RETENTION`: "true"/"false" to enable/disable (default: true)
    /// - `DASHFLOW_TRACE_MAX_COUNT`: Maximum traces (default: 1000)
    /// - `DASHFLOW_TRACE_MAX_AGE_DAYS`: Maximum age in days (default: 30)
    /// - `DASHFLOW_TRACE_MAX_SIZE_MB`: Maximum size in MB (default: 500)
    /// - `DASHFLOW_TRACE_COMPRESS_AGE_DAYS`: Age for compression in days (default: 7)
    #[must_use]
    pub fn from_env() -> Self {
        let enabled = env_bool(DASHFLOW_TRACE_RETENTION, true);
        let max_traces = Some(env_usize(DASHFLOW_TRACE_MAX_COUNT, DEFAULT_MAX_TRACES));
        let max_age_days = env_u64(DASHFLOW_TRACE_MAX_AGE_DAYS, DEFAULT_MAX_AGE_DAYS);
        let max_size_mb = env_u64(
            DASHFLOW_TRACE_MAX_SIZE_MB,
            DEFAULT_MAX_SIZE_BYTES / (1024 * 1024),
        );
        let compress_age_days = env_u64(DASHFLOW_TRACE_COMPRESS_AGE_DAYS, DEFAULT_COMPRESS_AGE_DAYS);

        Self {
            max_traces,
            max_age: Some(Duration::from_secs(max_age_days * 24 * 60 * 60)),
            max_size_bytes: Some(max_size_mb * 1024 * 1024),
            compress_age: Some(Duration::from_secs(compress_age_days * 24 * 60 * 60)),
            enabled,
        }
    }

    /// Builder: set maximum trace count.
    #[must_use]
    pub fn with_max_traces(mut self, count: usize) -> Self {
        self.max_traces = Some(count);
        self
    }

    /// Builder: set maximum age.
    #[must_use]
    pub fn with_max_age(mut self, age: Duration) -> Self {
        self.max_age = Some(age);
        self
    }

    /// Builder: set maximum age in days.
    #[must_use]
    pub fn with_max_age_days(mut self, days: u64) -> Self {
        self.max_age = Some(Duration::from_secs(days * 24 * 60 * 60));
        self
    }

    /// Builder: set maximum size in bytes.
    #[must_use]
    pub fn with_max_size_bytes(mut self, bytes: u64) -> Self {
        self.max_size_bytes = Some(bytes);
        self
    }

    /// Builder: set maximum size in MB.
    #[must_use]
    pub fn with_max_size_mb(mut self, mb: u64) -> Self {
        self.max_size_bytes = Some(mb * 1024 * 1024);
        self
    }

    /// Builder: disable all limits.
    #[must_use]
    pub fn with_no_limits(mut self) -> Self {
        self.max_traces = None;
        self.max_age = None;
        self.max_size_bytes = None;
        self.compress_age = None;
        self
    }

    /// Builder: set compression age in days.
    #[must_use]
    pub fn with_compress_age_days(mut self, days: u64) -> Self {
        self.compress_age = Some(Duration::from_secs(days * 24 * 60 * 60));
        self
    }

    /// Builder: disable compression.
    #[must_use]
    pub fn without_compression(mut self) -> Self {
        self.compress_age = None;
        self
    }

    /// Builder: enable/disable retention.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Statistics about a cleanup operation.
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    /// Number of traces deleted.
    pub deleted_count: usize,
    /// Total bytes freed.
    pub freed_bytes: u64,
    /// Traces deleted due to count limit.
    pub deleted_for_count: usize,
    /// Traces deleted due to age limit.
    pub deleted_for_age: usize,
    /// Traces deleted due to size limit.
    pub deleted_for_size: usize,
    /// Traces compressed.
    pub compressed_count: usize,
    /// Bytes saved by compression.
    pub compression_saved_bytes: u64,
    /// Any errors encountered (non-fatal).
    pub errors: Vec<String>,
}

/// Statistics about the traces directory.
#[derive(Debug, Clone, Default)]
pub struct TraceDirectoryStats {
    /// Total number of trace files.
    pub trace_count: usize,
    /// Total size of all traces in bytes.
    pub total_size_bytes: u64,
    /// Oldest trace modification time.
    pub oldest_trace: Option<SystemTime>,
    /// Newest trace modification time.
    pub newest_trace: Option<SystemTime>,
    /// Average trace size in bytes.
    pub avg_size_bytes: u64,
}

/// Metadata for a single trace file.
#[derive(Debug, Clone)]
struct TraceFileInfo {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

/// Manages trace retention and cleanup.
pub struct TraceRetentionManager {
    traces_dir: PathBuf,
    policy: RetentionPolicy,
}

impl TraceRetentionManager {
    /// Create a new retention manager.
    #[must_use]
    pub fn new(traces_dir: impl AsRef<Path>, policy: RetentionPolicy) -> Self {
        Self {
            traces_dir: traces_dir.as_ref().to_path_buf(),
            policy,
        }
    }

    /// Create with default policy from environment.
    #[must_use]
    pub fn with_default_policy(traces_dir: impl AsRef<Path>) -> Self {
        Self::new(traces_dir, RetentionPolicy::from_env())
    }

    /// Get the current policy.
    #[must_use]
    pub fn policy(&self) -> &RetentionPolicy {
        &self.policy
    }

    /// Update the retention policy.
    pub fn set_policy(&mut self, policy: RetentionPolicy) {
        self.policy = policy;
    }

    /// Get statistics about the traces directory.
    ///
    /// # Errors
    ///
    /// Returns error if directory cannot be read.
    pub fn stats(&self) -> io::Result<TraceDirectoryStats> {
        let traces = self.list_traces()?;

        if traces.is_empty() {
            return Ok(TraceDirectoryStats::default());
        }

        let total_size_bytes: u64 = traces.iter().map(|t| t.size).sum();
        let oldest_trace = traces.iter().map(|t| t.modified).min();
        let newest_trace = traces.iter().map(|t| t.modified).max();
        let avg_size_bytes = total_size_bytes / traces.len() as u64;

        Ok(TraceDirectoryStats {
            trace_count: traces.len(),
            total_size_bytes,
            oldest_trace,
            newest_trace,
            avg_size_bytes,
        })
    }

    /// Check if cleanup is needed based on current policy.
    ///
    /// # Errors
    ///
    /// Returns error if directory cannot be read.
    pub fn needs_cleanup(&self) -> io::Result<bool> {
        if !self.policy.enabled {
            return Ok(false);
        }

        let stats = self.stats()?;
        let now = SystemTime::now();

        // Check count limit
        if let Some(max_traces) = self.policy.max_traces {
            if stats.trace_count > max_traces {
                return Ok(true);
            }
        }

        // Check size limit
        if let Some(max_size) = self.policy.max_size_bytes {
            if stats.total_size_bytes > max_size {
                return Ok(true);
            }
        }

        // Check age limit
        if let (Some(max_age), Some(oldest)) = (self.policy.max_age, stats.oldest_trace) {
            if now.duration_since(oldest).is_ok_and(|age| age > max_age) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Run cleanup according to policy.
    ///
    /// # Errors
    ///
    /// Returns error if directory cannot be read. Individual file deletion
    /// errors are recorded in `CleanupStats::errors` but don't fail the operation.
    pub fn cleanup(&self) -> io::Result<CleanupStats> {
        let mut stats = CleanupStats::default();

        if !self.policy.enabled {
            return Ok(stats);
        }

        let mut traces = self.list_traces()?;

        // Sort by modification time (oldest first) for deletion order
        traces.sort_by_key(|t| t.modified);

        let now = SystemTime::now();

        // Compress old traces(before deletion)
        if let Some(compress_age) = self.policy.compress_age {
            let compress_cutoff = now
                .checked_sub(compress_age)
                .unwrap_or(SystemTime::UNIX_EPOCH);

            for trace in &mut traces {
                // Skip already compressed files or files not old enough
                if trace.path.extension().map_or(true, |ext| ext == "gz") {
                    continue;
                }
                if trace.modified >= compress_cutoff {
                    continue;
                }

                // Compress the file
                match self.compress_file(&trace.path) {
                    Ok((new_path, saved)) => {
                        stats.compressed_count += 1;
                        stats.compression_saved_bytes += saved;
                        // Update the trace info to point to compressed file
                        trace.path = new_path;
                        trace.size = trace.size.saturating_sub(saved);
                    }
                    Err(e) => {
                        stats.errors.push(format!(
                            "Failed to compress {}: {}",
                            trace.path.display(),
                            e
                        ));
                    }
                }
            }
        }

        // Delete traces exceeding age limit
        if let Some(max_age) = self.policy.max_age {
            let cutoff = now.checked_sub(max_age).unwrap_or(SystemTime::UNIX_EPOCH);

            traces.retain(|trace| {
                if trace.modified < cutoff {
                    match fs::remove_file(&trace.path) {
                        Ok(()) => {
                            stats.deleted_count += 1;
                            stats.deleted_for_age += 1;
                            stats.freed_bytes += trace.size;
                            false // Remove from list
                        }
                        Err(e) => {
                            stats.errors.push(format!(
                                "Failed to delete {}: {}",
                                trace.path.display(),
                                e
                            ));
                            true // Keep in list
                        }
                    }
                } else {
                    true // Keep in list
                }
            });
        }

        // Delete traces exceeding size limit (oldest first)
        if let Some(max_size) = self.policy.max_size_bytes {
            let mut current_size: u64 = traces.iter().map(|t| t.size).sum();

            while current_size > max_size && !traces.is_empty() {
                let trace = traces.remove(0); // Remove oldest
                match fs::remove_file(&trace.path) {
                    Ok(()) => {
                        stats.deleted_count += 1;
                        stats.deleted_for_size += 1;
                        stats.freed_bytes += trace.size;
                        current_size -= trace.size;
                    }
                    Err(e) => {
                        stats.errors.push(format!(
                            "Failed to delete {}: {}",
                            trace.path.display(),
                            e
                        ));
                    }
                }
            }
        }

        // Delete traces exceeding count limit (oldest first)
        if let Some(max_traces) = self.policy.max_traces {
            while traces.len() > max_traces {
                let trace = traces.remove(0); // Remove oldest
                match fs::remove_file(&trace.path) {
                    Ok(()) => {
                        stats.deleted_count += 1;
                        stats.deleted_for_count += 1;
                        stats.freed_bytes += trace.size;
                    }
                    Err(e) => {
                        stats.errors.push(format!(
                            "Failed to delete {}: {}",
                            trace.path.display(),
                            e
                        ));
                    }
                }
            }
        }

        // Log summary if any work was done
        if stats.deleted_count > 0 || stats.compressed_count > 0 {
            info!(
                deleted = stats.deleted_count,
                freed_bytes = stats.freed_bytes,
                compressed = stats.compressed_count,
                compression_saved_bytes = stats.compression_saved_bytes,
                deleted_for_age = stats.deleted_for_age,
                deleted_for_size = stats.deleted_for_size,
                deleted_for_count = stats.deleted_for_count,
                remaining = traces.len(),
                errors = stats.errors.len(),
                "Trace retention cleanup completed"
            );
        }

        Ok(stats)
    }

    /// List all trace files with metadata (includes .json and .json.gz).
    fn list_traces(&self) -> io::Result<Vec<TraceFileInfo>> {
        let mut traces = Vec::new();

        if !self.traces_dir.exists() {
            return Ok(traces);
        }

        for entry in fs::read_dir(&self.traces_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Process .json and .json.gz files using proper OsStr extension checking
            let is_json = path.extension() == Some(OsStr::new("json"));
            let is_compressed = path.extension() == Some(OsStr::new("gz"))
                && path.file_stem().and_then(|s| Path::new(s).extension())
                    == Some(OsStr::new("json"));

            if is_json || is_compressed {
                match entry.metadata() {
                    Ok(metadata) => match metadata.modified() {
                        Ok(modified) => {
                            traces.push(TraceFileInfo {
                                path,
                                size: metadata.len(),
                                modified,
                            });
                        }
                        Err(e) => {
                            debug!(path = %path.display(), error = %e, "Skipping trace file: cannot read modification time");
                        }
                    },
                    Err(e) => {
                        debug!(path = %path.display(), error = %e, "Skipping trace file: cannot read metadata");
                    }
                }
            }
        }

        Ok(traces)
    }

    /// Compress a JSON file to .json.gz format.
    ///
    /// Returns the new path and bytes saved.
    fn compress_file(&self, path: &Path) -> io::Result<(PathBuf, u64)> {
        // Read original file
        let original_content = fs::read(path)?;
        let original_size = original_content.len() as u64;

        // Create compressed file
        let compressed_path = PathBuf::from(format!("{}.gz", path.display()));
        let compressed_file = File::create(&compressed_path)?;
        let mut encoder = GzEncoder::new(compressed_file, Compression::default());
        encoder.write_all(&original_content)?;
        encoder.finish()?;

        // Get compressed size
        let compressed_size = fs::metadata(&compressed_path)?.len();

        // Remove original file
        fs::remove_file(path)?;

        // Calculate savings (can be negative if file is small)
        let saved = original_size.saturating_sub(compressed_size);

        Ok((compressed_path, saved))
    }
}

/// Maximum allowed decompressed trace size: 100 MB (protection against gzip bombs)
pub const MAX_DECOMPRESSED_TRACE_SIZE: u64 = 100 * 1024 * 1024;

/// Decompress a gzipped trace file.
///
/// # Security
///
/// This function limits decompressed output to `MAX_DECOMPRESSED_TRACE_SIZE`
/// to protect against gzip bomb attacks.
///
/// # Errors
///
/// Returns error if decompression fails or size exceeds limit.
pub fn decompress_trace(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let decoder = GzDecoder::new(file);
    // Limit decompressed size to prevent gzip bombs
    let mut limited = decoder.take(MAX_DECOMPRESSED_TRACE_SIZE + 1);
    let mut content = String::new();
    limited.read_to_string(&mut content)?;

    if content.len() as u64 > MAX_DECOMPRESSED_TRACE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Decompressed trace size exceeds limit of {} bytes",
                MAX_DECOMPRESSED_TRACE_SIZE
            ),
        ));
    }
    Ok(content)
}

/// Read a trace file, handling both compressed and uncompressed formats.
///
/// # Errors
///
/// Returns error if read fails.
pub fn read_trace_file(path: &Path) -> io::Result<String> {
    // Check for .json.gz using proper OsStr comparison
    let is_compressed = path.extension() == Some(OsStr::new("gz"))
        && path.file_stem().and_then(|s| Path::new(s).extension()) == Some(OsStr::new("json"));

    if is_compressed {
        decompress_trace(path)
    } else {
        fs::read_to_string(path)
    }
}

/// Run cleanup with default policy and traces directory.
///
/// Convenience function for one-off cleanup.
///
/// # Errors
///
/// Returns error if cleanup fails.
pub fn cleanup_traces(traces_dir: impl AsRef<Path>) -> io::Result<CleanupStats> {
    let manager = TraceRetentionManager::with_default_policy(traces_dir);
    manager.cleanup()
}

/// Run cleanup on the default traces directory (`.dashflow/traces`).
///
/// # Errors
///
/// Returns error if cleanup fails.
pub fn cleanup_default_traces() -> io::Result<CleanupStats> {
    cleanup_traces(".dashflow/traces")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_test_trace(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(format!("{}.json", name));
        let mut file = File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_default_policy() {
        let policy = RetentionPolicy::default();
        assert!(policy.enabled);
        assert_eq!(policy.max_traces, Some(DEFAULT_MAX_TRACES));
        assert!(policy.max_age.is_some());
        assert!(policy.max_size_bytes.is_some());
    }

    #[test]
    fn test_unlimited_policy() {
        let policy = RetentionPolicy::unlimited();
        assert!(policy.enabled);
        assert!(policy.max_traces.is_none());
        assert!(policy.max_age.is_none());
        assert!(policy.max_size_bytes.is_none());
    }

    #[test]
    fn test_policy_builders() {
        let policy = RetentionPolicy::default()
            .with_max_traces(50)
            .with_max_age_days(7)
            .with_max_size_mb(100);

        assert_eq!(policy.max_traces, Some(50));
        assert_eq!(policy.max_age, Some(Duration::from_secs(7 * 24 * 60 * 60)));
        assert_eq!(policy.max_size_bytes, Some(100 * 1024 * 1024));
    }

    #[test]
    fn test_empty_directory_stats() {
        let dir = tempdir().unwrap();
        let manager = TraceRetentionManager::new(dir.path(), RetentionPolicy::default());

        let stats = manager.stats().unwrap();
        assert_eq!(stats.trace_count, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert!(stats.oldest_trace.is_none());
    }

    #[test]
    fn test_directory_stats() {
        let dir = tempdir().unwrap();
        create_test_trace(dir.path(), "trace1", r#"{"id": "1"}"#);
        create_test_trace(dir.path(), "trace2", r#"{"id": "2"}"#);
        create_test_trace(dir.path(), "trace3", r#"{"id": "3"}"#);

        let manager = TraceRetentionManager::new(dir.path(), RetentionPolicy::default());
        let stats = manager.stats().unwrap();

        assert_eq!(stats.trace_count, 3);
        assert!(stats.total_size_bytes > 0);
        assert!(stats.oldest_trace.is_some());
        assert!(stats.newest_trace.is_some());
    }

    #[test]
    fn test_cleanup_by_count() {
        let dir = tempdir().unwrap();

        // Create 5 traces
        for i in 0..5 {
            create_test_trace(dir.path(), &format!("trace{}", i), r#"{"id": "x"}"#);
            // Small delay to ensure different modification times
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Policy: keep only 2 traces
        let policy = RetentionPolicy::default()
            .with_max_traces(2)
            .with_no_limits()
            .with_max_traces(2);

        let manager = TraceRetentionManager::new(dir.path(), policy);
        let cleanup_stats = manager.cleanup().unwrap();

        assert_eq!(cleanup_stats.deleted_count, 3);
        assert_eq!(cleanup_stats.deleted_for_count, 3);

        // Verify 2 remain
        let stats = manager.stats().unwrap();
        assert_eq!(stats.trace_count, 2);
    }

    #[test]
    fn test_cleanup_by_size() {
        let dir = tempdir().unwrap();

        // Create traces with known sizes (roughly 100 bytes each)
        let content = r#"{"id": "x", "data": "padding to make this roughly 100 bytes long for testing purposes ok"}"#;
        for i in 0..5 {
            create_test_trace(dir.path(), &format!("trace{}", i), content);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Policy: keep only 200 bytes (should keep ~2 traces)
        let policy = RetentionPolicy::unlimited().with_max_size_bytes(200);

        let manager = TraceRetentionManager::new(dir.path(), policy);
        let cleanup_stats = manager.cleanup().unwrap();

        assert!(cleanup_stats.deleted_count > 0);
        assert!(cleanup_stats.deleted_for_size > 0);
        assert!(cleanup_stats.freed_bytes > 0);

        // Verify remaining size is under limit
        let stats = manager.stats().unwrap();
        assert!(stats.total_size_bytes <= 200);
    }

    #[test]
    fn test_cleanup_disabled() {
        let dir = tempdir().unwrap();

        for i in 0..5 {
            create_test_trace(dir.path(), &format!("trace{}", i), r#"{"id": "x"}"#);
        }

        let policy = RetentionPolicy::default().with_max_traces(2).enabled(false);

        let manager = TraceRetentionManager::new(dir.path(), policy);
        let cleanup_stats = manager.cleanup().unwrap();

        assert_eq!(cleanup_stats.deleted_count, 0);

        // All 5 should remain
        let stats = manager.stats().unwrap();
        assert_eq!(stats.trace_count, 5);
    }

    #[test]
    fn test_needs_cleanup() {
        let dir = tempdir().unwrap();

        for i in 0..5 {
            create_test_trace(dir.path(), &format!("trace{}", i), r#"{"id": "x"}"#);
        }

        // Under limit - should not need cleanup
        let policy = RetentionPolicy::unlimited().with_max_traces(10);
        let manager = TraceRetentionManager::new(dir.path(), policy);
        assert!(!manager.needs_cleanup().unwrap());

        // Over limit - should need cleanup
        let policy = RetentionPolicy::unlimited().with_max_traces(2);
        let manager = TraceRetentionManager::new(dir.path(), policy);
        assert!(manager.needs_cleanup().unwrap());
    }

    #[test]
    fn test_nonexistent_directory() {
        let manager = TraceRetentionManager::new("/nonexistent/path", RetentionPolicy::default());

        let stats = manager.stats().unwrap();
        assert_eq!(stats.trace_count, 0);

        // Cleanup should succeed (nothing to clean)
        let cleanup_stats = manager.cleanup().unwrap();
        assert_eq!(cleanup_stats.deleted_count, 0);
    }
}
