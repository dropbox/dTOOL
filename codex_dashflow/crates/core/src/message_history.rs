//! Persistence layer for the global, append-only message history file.
//!
//! The history is stored at `~/.codex/history.jsonl` with one JSON object per
//! line (JSON Lines format) for efficient appending and parsing.
//!
//! Each record has the schema:
//! ```json
//! {"session_id":"<uuid>","ts":<unix_seconds>,"text":"<message>"}
//! ```
//!
//! File locking is used to ensure atomic writes across concurrent processes.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Result, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncReadExt;
use tracing::warn;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Filename for the message history inside the codex home directory.
pub const HISTORY_FILENAME: &str = "history.jsonl";

/// When history exceeds max_bytes, trim to this fraction of max_bytes.
const HISTORY_SOFT_CAP_RATIO: f64 = 0.8;

/// Maximum retries when acquiring file lock.
const MAX_RETRIES: usize = 10;

/// Sleep duration between lock retries.
const RETRY_SLEEP: Duration = Duration::from_millis(100);

/// A single entry in the message history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Session/conversation ID
    pub session_id: String,
    /// Unix timestamp (seconds since epoch)
    pub ts: u64,
    /// Message text
    pub text: String,
}

/// Configuration for history persistence.
#[derive(Debug, Clone, Default)]
pub struct HistoryConfig {
    /// Directory where history file is stored (default: ~/.codex)
    pub codex_home: PathBuf,
    /// Maximum size in bytes (None = unlimited)
    pub max_bytes: Option<usize>,
    /// Whether to persist history
    pub enabled: bool,
}

impl HistoryConfig {
    /// Create a new config with the given home directory.
    pub fn new(codex_home: impl Into<PathBuf>) -> Self {
        Self {
            codex_home: codex_home.into(),
            max_bytes: None,
            enabled: true,
        }
    }

    /// Set the maximum history size in bytes.
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = Some(max_bytes);
        self
    }

    /// Disable history persistence.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Get the path to the history file.
    pub fn history_path(&self) -> PathBuf {
        self.codex_home.join(HISTORY_FILENAME)
    }
}

/// Append a message to the history file.
///
/// Uses advisory file locking to ensure atomic writes across concurrent processes.
pub async fn append_entry(text: &str, session_id: &str, config: &HistoryConfig) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let path = config.history_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Get current timestamp
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| std::io::Error::other(format!("system clock before Unix epoch: {e}")))?
        .as_secs();

    // Build the JSON line
    let entry = HistoryEntry {
        session_id: session_id.to_string(),
        ts,
        text: text.to_string(),
    };
    let mut line = serde_json::to_string(&entry)
        .map_err(|e| std::io::Error::other(format!("failed to serialize history entry: {e}")))?;
    line.push('\n');

    // Open history file
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.append(true);
        options.mode(0o600);
    }

    let mut history_file = options.open(&path)?;

    // Ensure permissions (Unix only)
    ensure_owner_only_permissions(&history_file).await?;

    let max_bytes = config.max_bytes;

    // Write with file locking
    tokio::task::spawn_blocking(move || -> Result<()> {
        for _ in 0..MAX_RETRIES {
            match history_file.try_lock() {
                Ok(()) => {
                    // Seek to end (needed on Windows where append mode isn't set)
                    history_file.seek(SeekFrom::End(0))?;
                    history_file.write_all(line.as_bytes())?;
                    history_file.flush()?;
                    enforce_history_limit(&mut history_file, max_bytes)?;
                    return Ok(());
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(RETRY_SLEEP);
                }
                Err(e) => return Err(e.into()),
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "could not acquire exclusive lock on history file after multiple attempts",
        ))
    })
    .await??;

    Ok(())
}

/// Trim the history file to honor max_bytes, dropping oldest entries first.
fn enforce_history_limit(file: &mut File, max_bytes: Option<usize>) -> Result<()> {
    let Some(max_bytes) = max_bytes else {
        return Ok(());
    };

    if max_bytes == 0 {
        return Ok(());
    }

    let max_bytes = max_bytes as u64;
    let mut current_len = file.metadata()?.len();

    if current_len <= max_bytes {
        return Ok(());
    }

    // Read all line lengths
    let mut reader_file = file.try_clone()?;
    reader_file.seek(SeekFrom::Start(0))?;

    let mut buf_reader = BufReader::new(reader_file);
    let mut line_lengths = Vec::new();
    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        let bytes = buf_reader.read_line(&mut line_buf)?;
        if bytes == 0 {
            break;
        }
        line_lengths.push(bytes as u64);
    }

    if line_lengths.is_empty() {
        return Ok(());
    }

    // Calculate trim target (soft cap)
    let last_index = line_lengths.len() - 1;
    let soft_cap = (max_bytes as f64 * HISTORY_SOFT_CAP_RATIO).floor() as u64;
    let trim_target = soft_cap.max(line_lengths[last_index]);

    // Determine how many bytes to drop from the start
    let mut drop_bytes = 0u64;
    let mut idx = 0usize;

    while current_len > trim_target && idx < last_index {
        current_len = current_len.saturating_sub(line_lengths[idx]);
        drop_bytes += line_lengths[idx];
        idx += 1;
    }

    if drop_bytes == 0 {
        return Ok(());
    }

    // Read the tail (entries to keep)
    let mut reader = buf_reader.into_inner();
    reader.seek(SeekFrom::Start(drop_bytes))?;

    let mut tail = Vec::with_capacity(current_len as usize);
    reader.read_to_end(&mut tail)?;

    // Rewrite the file with only the tail
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&tail)?;
    file.flush()?;

    Ok(())
}

/// Get metadata about the history file: (log_id, entry_count).
///
/// The log_id is stable across appends (inode on Unix, creation time on Windows).
pub async fn history_metadata(config: &HistoryConfig) -> (u64, usize) {
    let path = config.history_path();
    history_metadata_for_file(&path).await
}

async fn history_metadata_for_file(path: &Path) -> (u64, usize) {
    let log_id = match fs::metadata(path).await {
        Ok(metadata) => history_log_id(&metadata).unwrap_or(0),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (0, 0),
        Err(_) => return (0, 0),
    };

    // Count newlines to get entry count
    let mut file = match fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return (log_id, 0),
    };

    let mut buf = [0u8; 8192];
    let mut count = 0usize;
    loop {
        match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                count += buf[..n].iter().filter(|&&b| b == b'\n').count();
            }
            Err(_) => return (log_id, 0),
        }
    }

    (log_id, count)
}

/// Look up a specific history entry by log_id and offset.
///
/// Returns None if the log_id doesn't match or the offset is out of bounds.
pub fn lookup(log_id: u64, offset: usize, config: &HistoryConfig) -> Option<HistoryEntry> {
    let path = config.history_path();
    lookup_history_entry(&path, log_id, offset)
}

fn lookup_history_entry(path: &Path, log_id: u64, offset: usize) -> Option<HistoryEntry> {
    let file = match OpenOptions::new().read(true).open(path) {
        Ok(f) => f,
        Err(e) => {
            warn!(error = %e, "failed to open history file");
            return None;
        }
    };

    let metadata = match file.metadata() {
        Ok(m) => m,
        Err(e) => {
            warn!(error = %e, "failed to stat history file");
            return None;
        }
    };

    let current_log_id = history_log_id(&metadata)?;

    if log_id != 0 && current_log_id != log_id {
        return None;
    }

    // Try to acquire shared lock
    for _ in 0..MAX_RETRIES {
        match file.try_lock_shared() {
            Ok(()) => {
                let reader = BufReader::new(&file);
                for (idx, line_res) in reader.lines().enumerate() {
                    let line = match line_res {
                        Ok(l) => l,
                        Err(e) => {
                            warn!(error = %e, "failed to read line from history file");
                            return None;
                        }
                    };

                    if idx == offset {
                        match serde_json::from_str::<HistoryEntry>(&line) {
                            Ok(entry) => return Some(entry),
                            Err(e) => {
                                warn!(error = %e, "failed to parse history entry");
                                return None;
                            }
                        }
                    }
                }
                return None;
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                std::thread::sleep(RETRY_SLEEP);
            }
            Err(e) => {
                warn!(error = %e, "failed to acquire shared lock on history file");
                return None;
            }
        }
    }

    None
}

/// Ensure file has owner-only permissions (0o600 on Unix).
#[cfg(unix)]
async fn ensure_owner_only_permissions(file: &File) -> Result<()> {
    let metadata = file.metadata()?;
    let current_mode = metadata.permissions().mode() & 0o777;
    if current_mode != 0o600 {
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        let perms_clone = perms.clone();
        let file_clone = file.try_clone()?;
        tokio::task::spawn_blocking(move || file_clone.set_permissions(perms_clone)).await??;
    }
    Ok(())
}

#[cfg(not(unix))]
async fn ensure_owner_only_permissions(_file: &File) -> Result<()> {
    Ok(())
}

/// Get a stable identifier for the history file (inode on Unix, creation time on Windows).
#[cfg(unix)]
fn history_log_id(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino())
}

#[cfg(windows)]
fn history_log_id(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::windows::fs::MetadataExt;
    Some(metadata.creation_time())
}

#[cfg(not(any(unix, windows)))]
fn history_log_id(_metadata: &std::fs::Metadata) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn test_config(dir: &TempDir) -> HistoryConfig {
        HistoryConfig::new(dir.path())
    }

    #[tokio::test]
    async fn test_append_and_read_entry() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("hello world", "session-1", &config)
            .await
            .unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 1);
        assert!(log_id > 0);

        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, "hello world");
        assert_eq!(entry.session_id, "session-1");
    }

    #[tokio::test]
    async fn test_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("first", "s1", &config).await.unwrap();
        append_entry("second", "s2", &config).await.unwrap();
        append_entry("third", "s3", &config).await.unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 3);

        let first = lookup(log_id, 0, &config).unwrap();
        assert_eq!(first.text, "first");

        let second = lookup(log_id, 1, &config).unwrap();
        assert_eq!(second.text, "second");

        let third = lookup(log_id, 2, &config).unwrap();
        assert_eq!(third.text, "third");
    }

    #[tokio::test]
    async fn test_disabled_history() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);
        config.enabled = false;

        append_entry("should not persist", "s1", &config)
            .await
            .unwrap();

        let (_, count) = history_metadata(&config).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_history_entry_serialization() {
        let entry = HistoryEntry {
            session_id: "test-session".to_string(),
            ts: 1234567890,
            text: "test message".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test-session"));
        assert!(json.contains("1234567890"));
        assert!(json.contains("test message"));

        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, entry);
    }

    #[test]
    fn test_history_config_path() {
        let config = HistoryConfig::new("/home/user/.codex");
        assert_eq!(
            config.history_path(),
            PathBuf::from("/home/user/.codex/history.jsonl")
        );
    }

    #[tokio::test]
    async fn test_lookup_reads_history_entries() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join(HISTORY_FILENAME);

        let entries = vec![
            HistoryEntry {
                session_id: "first-session".to_string(),
                ts: 1,
                text: "first".to_string(),
            },
            HistoryEntry {
                session_id: "second-session".to_string(),
                ts: 2,
                text: "second".to_string(),
            },
        ];

        let mut file = File::create(&history_path).unwrap();
        for entry in &entries {
            writeln!(file, "{}", serde_json::to_string(entry).unwrap()).unwrap();
        }

        let (log_id, count) = history_metadata_for_file(&history_path).await;
        assert_eq!(count, entries.len());

        let second_entry = lookup_history_entry(&history_path, log_id, 1).unwrap();
        assert_eq!(second_entry, entries[1]);
    }

    #[tokio::test]
    async fn test_history_trim() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);

        // Write a large entry
        let large_text = "x".repeat(500);
        append_entry(&large_text, "s1", &config).await.unwrap();

        // Get the file size
        let path = config.history_path();
        let initial_size = std::fs::metadata(&path).unwrap().len();

        // Set max_bytes to just above initial size
        config.max_bytes = Some((initial_size + 50) as usize);

        // Write another entry - should trigger trim
        append_entry(&large_text, "s2", &config).await.unwrap();

        // Check that only one entry remains
        let (_, count) = history_metadata(&config).await;
        assert_eq!(count, 1);
    }

    // --- Additional tests for improved coverage ---

    #[test]
    fn test_history_config_with_max_bytes() {
        let config = HistoryConfig::new("/tmp/test").with_max_bytes(1024);
        assert_eq!(config.max_bytes, Some(1024));
        assert!(config.enabled);
    }

    #[test]
    fn test_history_config_disabled() {
        let config = HistoryConfig::disabled();
        assert!(!config.enabled);
        assert!(config.max_bytes.is_none());
    }

    #[test]
    fn test_history_config_default() {
        let config = HistoryConfig::default();
        assert!(!config.enabled);
        assert!(config.max_bytes.is_none());
        assert!(config.codex_home.as_os_str().is_empty());
    }

    #[tokio::test]
    async fn test_lookup_mismatched_log_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("test", "session", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;

        // Try to lookup with wrong log_id (non-zero, non-matching)
        let result = lookup(log_id + 999, 0, &config);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lookup_out_of_bounds_offset() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("only one", "session", &config).await.unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 1);

        // Offset 1 is out of bounds (only entry 0 exists)
        let result = lookup(log_id, 1, &config);
        assert!(result.is_none());

        // Large offset
        let result = lookup(log_id, 100, &config);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lookup_with_zero_log_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("test", "session", &config).await.unwrap();

        // log_id of 0 should allow lookup regardless of current log_id
        let result = lookup(0, 0, &config);
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "test");
    }

    #[tokio::test]
    async fn test_history_metadata_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Don't create any entries - file doesn't exist
        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(log_id, 0);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_history_metadata_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join(HISTORY_FILENAME);

        // Create empty file
        File::create(&history_path).unwrap();

        let (log_id, count) = history_metadata_for_file(&history_path).await;
        // log_id should be set (file exists), count should be 0
        assert!(log_id > 0);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_lookup_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // File doesn't exist
        let result = lookup(1, 0, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_history_entry_equality() {
        let entry1 = HistoryEntry {
            session_id: "s1".to_string(),
            ts: 123,
            text: "hello".to_string(),
        };
        let entry2 = HistoryEntry {
            session_id: "s1".to_string(),
            ts: 123,
            text: "hello".to_string(),
        };
        let entry3 = HistoryEntry {
            session_id: "s2".to_string(),
            ts: 123,
            text: "hello".to_string(),
        };

        assert_eq!(entry1, entry2);
        assert_ne!(entry1, entry3);
    }

    #[test]
    fn test_history_entry_clone() {
        let entry = HistoryEntry {
            session_id: "s1".to_string(),
            ts: 456,
            text: "test".to_string(),
        };
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
    }

    #[test]
    fn test_history_entry_debug() {
        let entry = HistoryEntry {
            session_id: "debug-session".to_string(),
            ts: 789,
            text: "debug test".to_string(),
        };
        let debug_str = format!("{:?}", entry);
        assert!(debug_str.contains("HistoryEntry"));
        assert!(debug_str.contains("debug-session"));
    }

    #[test]
    fn test_history_config_clone() {
        let config = HistoryConfig::new("/test/path").with_max_bytes(2048);
        let cloned = config.clone();
        assert_eq!(config.codex_home, cloned.codex_home);
        assert_eq!(config.max_bytes, cloned.max_bytes);
        assert_eq!(config.enabled, cloned.enabled);
    }

    #[test]
    fn test_history_config_debug() {
        let config = HistoryConfig::new("/debug/test");
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("HistoryConfig"));
        assert!(debug_str.contains("/debug/test"));
    }

    #[tokio::test]
    async fn test_enforce_history_limit_zero_max_bytes() {
        // Verify max_bytes of 0 does not cause issues
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);
        config.max_bytes = Some(0);

        // This should succeed even with max_bytes = 0
        append_entry("test", "session", &config).await.unwrap();

        let (_, count) = history_metadata(&config).await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_append_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("deeply");
        let config = HistoryConfig::new(&nested_path);

        // Parent directory doesn't exist yet
        assert!(!nested_path.exists());

        append_entry("test", "session", &config).await.unwrap();

        // Now it should exist
        assert!(nested_path.exists());
        assert!(config.history_path().exists());
    }

    #[test]
    fn test_history_entry_deserialization_from_json_string() {
        let json = r#"{"session_id":"ses-123","ts":999,"text":"parsed text"}"#;
        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.session_id, "ses-123");
        assert_eq!(entry.ts, 999);
        assert_eq!(entry.text, "parsed text");
    }

    #[tokio::test]
    async fn test_multiple_sessions_in_history() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Simulate multiple sessions writing to the same history
        append_entry("msg from session A", "session-A", &config)
            .await
            .unwrap();
        append_entry("msg from session B", "session-B", &config)
            .await
            .unwrap();
        append_entry("another from A", "session-A", &config)
            .await
            .unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 3);

        // Verify each entry
        let e0 = lookup(log_id, 0, &config).unwrap();
        assert_eq!(e0.session_id, "session-A");
        assert_eq!(e0.text, "msg from session A");

        let e1 = lookup(log_id, 1, &config).unwrap();
        assert_eq!(e1.session_id, "session-B");

        let e2 = lookup(log_id, 2, &config).unwrap();
        assert_eq!(e2.session_id, "session-A");
        assert_eq!(e2.text, "another from A");
    }

    #[tokio::test]
    async fn test_unicode_in_history() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let unicode_text = "Hello 世界! 🌍 Γειά σου κόσμε";
        append_entry(unicode_text, "unicode-session", &config)
            .await
            .unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, unicode_text);
    }

    #[tokio::test]
    async fn test_special_characters_in_text() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Text with JSON special characters that need escaping
        let special_text = "quote: \" backslash: \\ newline: \n tab: \t";
        append_entry(special_text, "special", &config)
            .await
            .unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, special_text);
    }

    #[test]
    fn test_history_filename_constant() {
        assert_eq!(HISTORY_FILENAME, "history.jsonl");
    }

    // --- Additional coverage tests (N=282) ---

    #[test]
    fn test_history_config_new_with_path_buf() {
        let path = PathBuf::from("/home/user/.codex");
        let config = HistoryConfig::new(path);
        assert_eq!(config.codex_home, PathBuf::from("/home/user/.codex"));
        assert!(config.enabled);
        assert!(config.max_bytes.is_none());
    }

    #[test]
    fn test_history_config_builder_chain() {
        let config = HistoryConfig::new("/test")
            .with_max_bytes(1024)
            .with_max_bytes(2048); // Should overwrite

        assert_eq!(config.max_bytes, Some(2048));
    }

    #[test]
    fn test_history_config_disabled_fields() {
        let config = HistoryConfig::disabled();
        assert!(!config.enabled);
        // codex_home should be default (empty)
        assert!(config.codex_home.as_os_str().is_empty());
        // max_bytes should be None
        assert!(config.max_bytes.is_none());
    }

    #[tokio::test]
    async fn test_append_entry_very_long_text() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Very long text
        let long_text = "x".repeat(100_000);
        append_entry(&long_text, "long-session", &config)
            .await
            .unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 1);

        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text.len(), 100_000);
    }

    #[tokio::test]
    async fn test_append_entry_empty_text() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("", "empty-text", &config).await.unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 1);

        let entry = lookup(log_id, 0, &config).unwrap();
        assert!(entry.text.is_empty());
    }

    #[tokio::test]
    async fn test_append_entry_empty_session_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("message", "", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert!(entry.session_id.is_empty());
    }

    #[test]
    fn test_history_entry_serde_roundtrip() {
        let entry = HistoryEntry {
            session_id: "test-id".to_string(),
            ts: 1234567890,
            text: "Test message with special chars: \"quotes\" and \\backslash".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_history_entry_deserialize_extra_fields() {
        // JSON with extra fields should still parse (serde default behavior)
        let json = r#"{"session_id":"s1","ts":123,"text":"msg","extra_field":"ignored"}"#;
        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.session_id, "s1");
        assert_eq!(entry.ts, 123);
        assert_eq!(entry.text, "msg");
    }

    #[tokio::test]
    async fn test_history_trim_preserves_newest() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);

        // Write several entries
        for i in 0..10 {
            append_entry(&format!("entry {}", i), &format!("s{}", i), &config)
                .await
                .unwrap();
        }

        let path = config.history_path();
        let initial_size = std::fs::metadata(&path).unwrap().len();

        // Set max_bytes to trigger trimming
        config.max_bytes = Some((initial_size / 2) as usize);

        // Write one more entry to trigger trim
        append_entry("final entry", "final", &config).await.unwrap();

        // Should have fewer entries now
        let (log_id, count) = history_metadata(&config).await;
        assert!(
            count < 11,
            "Expected fewer entries after trim, got {}",
            count
        );

        // The newest entry should still be accessible
        let last = lookup(log_id, count - 1, &config);
        assert!(last.is_some());
    }

    #[tokio::test]
    async fn test_history_metadata_counts_newlines() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join(HISTORY_FILENAME);

        // Manually write entries to control format
        let mut file = File::create(&history_path).unwrap();
        for i in 0..5 {
            writeln!(
                file,
                r#"{{"session_id":"s{}","ts":{},"text":"msg{}"}}"#,
                i, i, i
            )
            .unwrap();
        }

        let (_, count) = history_metadata_for_file(&history_path).await;
        assert_eq!(count, 5);
    }

    #[test]
    fn test_lookup_malformed_json_line() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join(HISTORY_FILENAME);
        let config = HistoryConfig::new(temp_dir.path());

        // Write malformed JSON
        let mut file = File::create(&history_path).unwrap();
        writeln!(file, "not valid json at all").unwrap();

        let (log_id, _) = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(history_metadata_for_file(&history_path));

        // Lookup should return None for malformed entry
        let result = lookup(log_id, 0, &config);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_timestamp_is_reasonable() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("test", "session", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();

        // Timestamp should be recent (within last minute)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert!(entry.ts > 0);
        assert!(entry.ts <= now);
        assert!(now - entry.ts < 60); // Within last minute
    }

    #[test]
    fn test_history_config_path_with_nested_dirs() {
        let config = HistoryConfig::new("/a/b/c/d/.codex");
        let path = config.history_path();
        assert_eq!(path, PathBuf::from("/a/b/c/d/.codex/history.jsonl"));
    }

    #[tokio::test]
    async fn test_lookup_boundary_offsets() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Add exactly 3 entries
        append_entry("first", "s1", &config).await.unwrap();
        append_entry("second", "s2", &config).await.unwrap();
        append_entry("third", "s3", &config).await.unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 3);

        // Test boundary conditions
        assert!(lookup(log_id, 0, &config).is_some()); // First
        assert!(lookup(log_id, 2, &config).is_some()); // Last
        assert!(lookup(log_id, 3, &config).is_none()); // One past end
        assert!(lookup(log_id, usize::MAX, &config).is_none()); // Way past end
    }

    #[test]
    fn test_history_entry_ts_zero() {
        // ts of 0 is valid (Unix epoch)
        let entry = HistoryEntry {
            session_id: "epoch".to_string(),
            ts: 0,
            text: "at the epoch".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ts, 0);
    }

    #[test]
    fn test_history_entry_ts_max() {
        // Very large ts (far future)
        let entry = HistoryEntry {
            session_id: "future".to_string(),
            ts: u64::MAX,
            text: "far future".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ts, u64::MAX);
    }

    #[tokio::test]
    async fn test_multiple_rapid_appends() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Rapid sequential appends
        for i in 0..100 {
            append_entry(&format!("msg{}", i), &format!("s{}", i), &config)
                .await
                .unwrap();
        }

        let (_, count) = history_metadata(&config).await;
        assert_eq!(count, 100);
    }

    #[test]
    fn test_soft_cap_ratio_constant() {
        // Verify soft cap is 80%
        assert!((HISTORY_SOFT_CAP_RATIO - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_max_retries_constant() {
        assert_eq!(MAX_RETRIES, 10);
    }

    #[test]
    fn test_retry_sleep_constant() {
        assert_eq!(RETRY_SLEEP, Duration::from_millis(100));
    }

    // ============================================================
    // Additional coverage tests (N=285)
    // ============================================================

    #[test]
    fn test_history_entry_fields_accessible() {
        let entry = HistoryEntry {
            session_id: "sess".to_string(),
            ts: 12345,
            text: "hello".to_string(),
        };

        // All fields should be publicly accessible
        assert_eq!(entry.session_id, "sess");
        assert_eq!(entry.ts, 12345);
        assert_eq!(entry.text, "hello");
    }

    #[test]
    fn test_history_entry_whitespace_text() {
        let entry = HistoryEntry {
            session_id: "ws".to_string(),
            ts: 1,
            text: "   \t\n   ".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, "   \t\n   ");
    }

    #[test]
    fn test_history_entry_control_characters() {
        let entry = HistoryEntry {
            session_id: "ctrl".to_string(),
            ts: 2,
            text: "line1\r\nline2\x00null".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, entry.text);
    }

    #[test]
    fn test_history_config_default_is_disabled() {
        let config = HistoryConfig::default();
        // Default should be disabled for safety
        assert!(!config.enabled);
    }

    #[test]
    fn test_history_config_new_enables_by_default() {
        let config = HistoryConfig::new("/some/path");
        assert!(config.enabled);
    }

    #[test]
    fn test_history_config_with_max_bytes_zero() {
        let config = HistoryConfig::new("/test").with_max_bytes(0);
        assert_eq!(config.max_bytes, Some(0));
    }

    #[test]
    fn test_history_config_with_max_bytes_large() {
        let config = HistoryConfig::new("/test").with_max_bytes(usize::MAX);
        assert_eq!(config.max_bytes, Some(usize::MAX));
    }

    #[test]
    fn test_history_config_history_path_empty() {
        let config = HistoryConfig::new("");
        assert_eq!(config.history_path(), PathBuf::from(HISTORY_FILENAME));
    }

    #[test]
    fn test_history_config_history_path_relative() {
        let config = HistoryConfig::new("relative/path");
        assert_eq!(
            config.history_path(),
            PathBuf::from("relative/path/history.jsonl")
        );
    }

    #[tokio::test]
    async fn test_append_entry_whitespace_session_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("msg", "  spaces  ", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.session_id, "  spaces  ");
    }

    #[tokio::test]
    async fn test_append_entry_newlines_in_text() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let text_with_newlines = "line1\nline2\nline3";
        append_entry(text_with_newlines, "nl", &config)
            .await
            .unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, text_with_newlines);
    }

    #[tokio::test]
    async fn test_history_entries_ordered_by_insertion() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Add entries with predictable order
        for i in 0..10 {
            append_entry(&format!("msg{}", i), &format!("s{}", i), &config)
                .await
                .unwrap();
        }

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 10);

        // Verify order
        for i in 0..10 {
            let entry = lookup(log_id, i, &config).unwrap();
            assert_eq!(entry.text, format!("msg{}", i));
            assert_eq!(entry.session_id, format!("s{}", i));
        }
    }

    #[tokio::test]
    async fn test_history_timestamps_increasing() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("first", "s1", &config).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        append_entry("second", "s2", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let first = lookup(log_id, 0, &config).unwrap();
        let second = lookup(log_id, 1, &config).unwrap();

        // Second timestamp should be >= first (might be same if subsecond)
        assert!(second.ts >= first.ts);
    }

    #[test]
    fn test_lookup_empty_path() {
        let config = HistoryConfig::new("");
        // Should handle gracefully (file won't exist)
        let result = lookup(1, 0, &config);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_history_metadata_single_entry() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("single", "s1", &config).await.unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert!(log_id > 0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_history_with_json_in_text() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Text containing valid JSON
        let json_text = r#"{"key": "value", "nested": {"a": 1}}"#;
        append_entry(json_text, "json-test", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, json_text);
    }

    #[test]
    fn test_history_entry_deserialize_missing_fields_fails() {
        // Missing required fields should fail
        let incomplete_json = r#"{"session_id":"s1","ts":123}"#; // missing "text"
        let result: std::result::Result<HistoryEntry, _> = serde_json::from_str(incomplete_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_history_entry_deserialize_wrong_type_fails() {
        // Wrong type for field should fail
        let wrong_type = r#"{"session_id":"s1","ts":"not_a_number","text":"msg"}"#;
        let result: std::result::Result<HistoryEntry, _> = serde_json::from_str(wrong_type);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_appends_sequential() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Simulate sequential appends from same "session"
        for i in 0..50 {
            append_entry(&format!("msg{}", i), "concurrent-session", &config)
                .await
                .unwrap();
        }

        let (_, count) = history_metadata(&config).await;
        assert_eq!(count, 50);
    }

    #[test]
    fn test_history_config_clone_enabled_state() {
        let config = HistoryConfig::new("/path").with_max_bytes(1024);
        let cloned = config.clone();

        assert_eq!(config.enabled, cloned.enabled);
        assert!(cloned.enabled);

        let disabled = HistoryConfig::disabled();
        let cloned_disabled = disabled.clone();
        assert!(!cloned_disabled.enabled);
    }

    #[tokio::test]
    async fn test_lookup_with_matching_log_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("test", "session", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;

        // Exact log_id match should work
        let result = lookup(log_id, 0, &config);
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "test");
    }

    #[test]
    fn test_history_entry_eq_all_fields_matter() {
        let base = HistoryEntry {
            session_id: "s1".to_string(),
            ts: 123,
            text: "msg".to_string(),
        };

        // Different session_id
        let diff_session = HistoryEntry {
            session_id: "s2".to_string(),
            ts: 123,
            text: "msg".to_string(),
        };
        assert_ne!(base, diff_session);

        // Different ts
        let diff_ts = HistoryEntry {
            session_id: "s1".to_string(),
            ts: 456,
            text: "msg".to_string(),
        };
        assert_ne!(base, diff_ts);

        // Different text
        let diff_text = HistoryEntry {
            session_id: "s1".to_string(),
            ts: 123,
            text: "other".to_string(),
        };
        assert_ne!(base, diff_text);
    }

    #[tokio::test]
    async fn test_enforce_history_limit_no_max() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Without max_bytes, should keep all entries
        for i in 0..100 {
            append_entry(&"x".repeat(100), &format!("s{}", i), &config)
                .await
                .unwrap();
        }

        let (_, count) = history_metadata(&config).await;
        assert_eq!(count, 100);
    }

    #[tokio::test]
    async fn test_append_creates_file_with_content() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        assert!(!config.history_path().exists());

        append_entry("test", "session", &config).await.unwrap();

        assert!(config.history_path().exists());

        // File should contain valid JSONL
        let content = std::fs::read_to_string(config.history_path()).unwrap();
        assert!(content.contains("test"));
        assert!(content.contains("session"));
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn test_history_entry_serialization_format() {
        let entry = HistoryEntry {
            session_id: "s1".to_string(),
            ts: 1000,
            text: "hello".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();

        // Should contain all required fields
        assert!(json.contains("\"session_id\":"));
        assert!(json.contains("\"ts\":"));
        assert!(json.contains("\"text\":"));

        // Should be valid JSON
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[tokio::test]
    async fn test_history_entry_with_emoji() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let emoji_text = "Hello 👋 World 🌍!";
        append_entry(emoji_text, "emoji-session 🎉", &config)
            .await
            .unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, emoji_text);
        assert_eq!(entry.session_id, "emoji-session 🎉");
    }

    #[test]
    fn test_history_config_fields_public() {
        // Verify all fields are publicly accessible/modifiable
        let config = HistoryConfig {
            codex_home: PathBuf::from("/new/path"),
            max_bytes: Some(2048),
            enabled: true,
        };

        assert_eq!(config.codex_home, PathBuf::from("/new/path"));
        assert_eq!(config.max_bytes, Some(2048));
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_append_to_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // First append creates file
        append_entry("first", "s1", &config).await.unwrap();
        let (log_id1, count1) = history_metadata(&config).await;

        // Second append to existing file
        append_entry("second", "s2", &config).await.unwrap();
        let (log_id2, count2) = history_metadata(&config).await;

        // Same log_id (same file), count increased
        assert_eq!(log_id1, log_id2);
        assert_eq!(count1, 1);
        assert_eq!(count2, 2);
    }

    #[test]
    fn test_lookup_malformed_json_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join(HISTORY_FILENAME);
        let config = HistoryConfig::new(temp_dir.path());

        // Write lines with some malformed JSON
        let mut file = File::create(&history_path).unwrap();
        writeln!(file, r#"{{"session_id":"s1","ts":1,"text":"valid1"}}"#).unwrap();
        writeln!(file, "not valid json").unwrap();
        writeln!(file, r#"{{"session_id":"s3","ts":3,"text":"valid3"}}"#).unwrap();

        let (log_id, count) = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(history_metadata_for_file(&history_path));
        assert_eq!(count, 3); // 3 newlines

        // First entry valid
        let e0 = lookup(log_id, 0, &config);
        assert!(e0.is_some());
        assert_eq!(e0.unwrap().text, "valid1");

        // Second entry is malformed
        let e1 = lookup(log_id, 1, &config);
        assert!(e1.is_none());

        // Third entry valid
        let e2 = lookup(log_id, 2, &config);
        assert!(e2.is_some());
        assert_eq!(e2.unwrap().text, "valid3");
    }

    #[tokio::test]
    async fn test_history_trim_keeps_at_least_one() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);

        // Add a large entry
        let large = "x".repeat(1000);
        append_entry(&large, "s1", &config).await.unwrap();

        // Set max_bytes very small to trigger aggressive trim
        config.max_bytes = Some(100);

        // Add another entry - should trigger trim but keep at least the newest
        append_entry(&large, "s2", &config).await.unwrap();

        let (_, count) = history_metadata(&config).await;
        assert!(count >= 1, "Should keep at least one entry");
    }

    #[test]
    fn test_history_entry_debug_impl() {
        let entry = HistoryEntry {
            session_id: "debug-test".to_string(),
            ts: 999,
            text: "debug message".to_string(),
        };

        let debug = format!("{:?}", entry);
        assert!(debug.contains("HistoryEntry"));
        assert!(debug.contains("debug-test"));
        assert!(debug.contains("999"));
        assert!(debug.contains("debug message"));
    }

    // ============================================================
    // Additional coverage tests (N=292)
    // ============================================================

    #[tokio::test]
    async fn test_append_entry_long_session_id() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let long_session = "x".repeat(10000);
        append_entry("msg", &long_session, &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;
        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.session_id.len(), 10000);
    }

    #[tokio::test]
    async fn test_history_trim_exact_boundary() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);

        // Write two identical-size entries
        let text = "x".repeat(100);
        append_entry(&text, "s1", &config).await.unwrap();
        append_entry(&text, "s2", &config).await.unwrap();

        let path = config.history_path();
        let size_after_two = std::fs::metadata(&path).unwrap().len();

        // Set max_bytes exactly at the boundary
        config.max_bytes = Some(size_after_two as usize);

        // Third entry should fit without trimming (exact boundary)
        append_entry(&text, "s3", &config).await.unwrap();

        let (_, count) = history_metadata(&config).await;
        // Should still have entries, not trimmed aggressively
        assert!(count >= 1);
    }

    #[tokio::test]
    async fn test_lookup_large_offset() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("only", "s1", &config).await.unwrap();

        let (log_id, _) = history_metadata(&config).await;

        // Very large offset
        assert!(lookup(log_id, usize::MAX, &config).is_none());
        assert!(lookup(log_id, 1_000_000, &config).is_none());
    }

    #[test]
    fn test_history_entry_serde_preserves_all_fields() {
        let entry = HistoryEntry {
            session_id: "session-with-dashes".to_string(),
            ts: u64::MAX - 1,
            text: "text\nwith\nnewlines".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.session_id, entry.session_id);
        assert_eq!(parsed.ts, entry.ts);
        assert_eq!(parsed.text, entry.text);
    }

    #[tokio::test]
    async fn test_history_multiple_trim_cycles() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);

        // Set a very small limit
        config.max_bytes = Some(500);

        // Write many entries that will trigger multiple trims
        for i in 0..20 {
            let text = format!("entry {} with some content", i);
            append_entry(&text, &format!("s{}", i), &config)
                .await
                .unwrap();
        }

        let path = config.history_path();
        let size = std::fs::metadata(&path).unwrap().len();

        // File should be within limits
        assert!(size <= 500, "File size {} exceeds limit 500", size);

        let (_, count) = history_metadata(&config).await;
        assert!(count >= 1, "Should have at least one entry");
    }

    #[test]
    fn test_history_config_path_unix_style() {
        let config = HistoryConfig::new("/home/user/.config/codex");
        let path = config.history_path();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/codex/history.jsonl")
        );
    }

    #[test]
    fn test_history_config_with_max_bytes_one() {
        let config = HistoryConfig::new("/test").with_max_bytes(1);
        assert_eq!(config.max_bytes, Some(1));
    }

    #[tokio::test]
    async fn test_append_preserves_entry_order() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let messages = ["alpha", "beta", "gamma", "delta", "epsilon"];
        for msg in &messages {
            append_entry(msg, "session", &config).await.unwrap();
        }

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 5);

        for (i, expected) in messages.iter().enumerate() {
            let entry = lookup(log_id, i, &config).unwrap();
            assert_eq!(&entry.text, *expected);
        }
    }

    #[test]
    fn test_history_entry_empty_fields() {
        let entry = HistoryEntry {
            session_id: "".to_string(),
            ts: 0,
            text: "".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, entry);
    }

    #[tokio::test]
    async fn test_concurrent_config_instances() {
        let temp_dir = TempDir::new().unwrap();

        // Multiple configs pointing to same location
        let config1 = test_config(&temp_dir);
        let config2 = test_config(&temp_dir);

        append_entry("from config1", "s1", &config1).await.unwrap();
        append_entry("from config2", "s2", &config2).await.unwrap();

        let (_, count1) = history_metadata(&config1).await;
        let (_, count2) = history_metadata(&config2).await;

        assert_eq!(count1, count2);
        assert_eq!(count1, 2);
    }

    #[test]
    fn test_history_entry_json_field_order() {
        let entry = HistoryEntry {
            session_id: "s".to_string(),
            ts: 1,
            text: "t".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();

        // The JSON should contain all fields (order may vary with serde)
        assert!(json.contains("session_id"));
        assert!(json.contains("ts"));
        assert!(json.contains("text"));
    }

    #[tokio::test]
    async fn test_append_after_manual_truncate() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Write some entries
        append_entry("first", "s1", &config).await.unwrap();
        append_entry("second", "s2", &config).await.unwrap();

        // Manually truncate the file
        let path = config.history_path();
        File::create(&path).unwrap(); // Truncates to 0

        // Should still be able to append
        append_entry("after truncate", "s3", &config).await.unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 1);

        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, "after truncate");
    }

    #[test]
    fn test_history_entry_deserialize_null_fields_fails() {
        let json = r#"{"session_id":null,"ts":1,"text":"msg"}"#;
        let result: std::result::Result<HistoryEntry, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_history_file_permissions_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("test", "session", &config).await.unwrap();

        let path = config.history_path();
        let metadata = std::fs::metadata(&path).unwrap();

        // On Unix, file should exist and be readable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode() & 0o777;
            // Should be readable/writable by owner (0o600 or similar)
            assert!(mode & 0o400 != 0, "File should be readable");
            assert!(mode & 0o200 != 0, "File should be writable");
        }
        let _ = metadata;
    }

    #[test]
    fn test_history_config_default_disabled_by_default() {
        // Default config should be disabled to prevent accidental history writes
        let config = HistoryConfig::default();
        assert!(!config.enabled);
    }

    #[tokio::test]
    async fn test_lookup_zero_log_id_always_matches() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("test", "session", &config).await.unwrap();

        // log_id of 0 should match any file
        let result = lookup(0, 0, &config);
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "test");
    }

    #[test]
    fn test_history_entry_binary_content_roundtrip() {
        // While not recommended, binary-ish content should survive JSON encoding
        let entry = HistoryEntry {
            session_id: "binary".to_string(),
            ts: 1,
            text: "has\u{0001}control\u{001F}chars".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, entry.text);
    }

    #[tokio::test]
    async fn test_history_single_byte_content() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        append_entry("x", "s", &config).await.unwrap();

        let (log_id, count) = history_metadata(&config).await;
        assert_eq!(count, 1);

        let entry = lookup(log_id, 0, &config).unwrap();
        assert_eq!(entry.text, "x");
    }

    #[test]
    fn test_history_config_clone_independence() {
        let config = HistoryConfig::new("/original").with_max_bytes(1024);
        let mut cloned = config.clone();

        // Modifying clone should not affect original
        cloned.max_bytes = Some(2048);
        cloned.codex_home = PathBuf::from("/modified");

        assert_eq!(config.max_bytes, Some(1024));
        assert_eq!(config.codex_home, PathBuf::from("/original"));
    }

    #[tokio::test]
    async fn test_history_metadata_on_corrupted_file() {
        let temp_dir = TempDir::new().unwrap();
        let history_path = temp_dir.path().join(HISTORY_FILENAME);

        // Write a file with partial/corrupted content (no newlines)
        let mut file = File::create(&history_path).unwrap();
        file.write_all(b"no newline at all").unwrap();

        let config = HistoryConfig::new(temp_dir.path());
        let (log_id, count) = history_metadata(&config).await;

        // log_id should be set, but count should be 0 (no complete entries)
        assert!(log_id > 0);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_history_entry_eq_reflexive() {
        let entry = HistoryEntry {
            session_id: "s".to_string(),
            ts: 1,
            text: "t".to_string(),
        };
        assert_eq!(entry, entry);
    }

    #[test]
    fn test_history_entry_eq_symmetric() {
        let a = HistoryEntry {
            session_id: "s".to_string(),
            ts: 1,
            text: "t".to_string(),
        };
        let b = HistoryEntry {
            session_id: "s".to_string(),
            ts: 1,
            text: "t".to_string(),
        };
        assert_eq!(a, b);
        assert_eq!(b, a);
    }

    #[tokio::test]
    async fn test_append_creates_file_if_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        assert!(!config.history_path().exists());

        append_entry("new", "session", &config).await.unwrap();

        assert!(config.history_path().exists());
    }
}
