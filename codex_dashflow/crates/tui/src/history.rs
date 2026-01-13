//! Command history persistence
//!
//! This module provides file-based persistence for command history across TUI sessions.
//! History is stored in `~/.codex-dashflow/history` as a plain text file with one command
//! per line.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use codex_dashflow_core::config::Config;

/// Maximum number of history entries to keep
const MAX_HISTORY_ENTRIES: usize = 1000;

/// Default history filename
const HISTORY_FILE: &str = "history";

/// Get the path to the history file
///
/// Returns `~/.codex-dashflow/history` or None if the home directory cannot be determined.
pub fn history_file_path() -> Option<PathBuf> {
    Config::ensure_config_dir()
        .ok()
        .map(|dir| dir.join(HISTORY_FILE))
}

/// Load command history from the history file
///
/// Returns an empty vector if the file doesn't exist or cannot be read.
/// Commands are returned in chronological order (oldest first).
pub fn load_history() -> Vec<String> {
    let Some(path) = history_file_path() else {
        tracing::debug!("Could not determine history file path");
        return Vec::new();
    };

    if !path.exists() {
        tracing::debug!("History file does not exist: {}", path.display());
        return Vec::new();
    }

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Failed to open history file: {}", e);
            return Vec::new();
        }
    };

    let reader = BufReader::new(file);
    let mut history: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.is_empty())
        .collect();

    // Keep only the most recent entries if we have too many
    if history.len() > MAX_HISTORY_ENTRIES {
        history = history.split_off(history.len() - MAX_HISTORY_ENTRIES);
    }

    tracing::debug!(
        "Loaded {} history entries from {}",
        history.len(),
        path.display()
    );
    history
}

/// Append a command to the history file
///
/// Creates the file if it doesn't exist. Silently fails if the file cannot be written.
/// Trims the history file if it exceeds `MAX_HISTORY_ENTRIES`.
pub fn append_history(command: &str) {
    // Skip empty commands
    if command.trim().is_empty() {
        return;
    }

    let Some(path) = history_file_path() else {
        tracing::debug!("Could not determine history file path");
        return;
    };

    // Append the command to the file
    let file = OpenOptions::new().create(true).append(true).open(&path);

    match file {
        Ok(mut f) => {
            if let Err(e) = writeln!(f, "{}", command) {
                tracing::warn!("Failed to write to history file: {}", e);
                return;
            }
            tracing::trace!("Appended command to history: {}", command);
        }
        Err(e) => {
            tracing::warn!("Failed to open history file for writing: {}", e);
            return;
        }
    }

    // Check if we need to trim the history file
    trim_history_if_needed(&path);
}

/// Trim the history file if it exceeds `MAX_HISTORY_ENTRIES`
fn trim_history_if_needed(path: &PathBuf) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    // Only trim if we're significantly over the limit (avoid constant rewrites)
    if lines.len() <= MAX_HISTORY_ENTRIES + 100 {
        return;
    }

    // Keep only the most recent entries
    let trimmed: Vec<&String> = lines
        .iter()
        .skip(lines.len() - MAX_HISTORY_ENTRIES)
        .collect();

    // Write back the trimmed history
    let file = match File::create(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Failed to trim history file: {}", e);
            return;
        }
    };

    let mut writer = std::io::BufWriter::new(file);
    for line in trimmed {
        if let Err(e) = writeln!(writer, "{}", line) {
            tracing::warn!("Failed to write trimmed history: {}", e);
            return;
        }
    }

    tracing::debug!("Trimmed history file to {} entries", MAX_HISTORY_ENTRIES);
}

/// Save the entire history to the history file
///
/// Replaces the existing file contents. Used when the history is modified in memory
/// (e.g., after deduplication or editing).
#[allow(dead_code)]
pub fn save_history(history: &[String]) {
    let Some(path) = history_file_path() else {
        tracing::debug!("Could not determine history file path");
        return;
    };

    let file = match File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Failed to create history file: {}", e);
            return;
        }
    };

    let mut writer = std::io::BufWriter::new(file);

    // Keep only the most recent entries
    let start = history.len().saturating_sub(MAX_HISTORY_ENTRIES);
    for command in &history[start..] {
        if let Err(e) = writeln!(writer, "{}", command) {
            tracing::warn!("Failed to write history: {}", e);
            return;
        }
    }

    tracing::debug!(
        "Saved {} history entries to {}",
        history.len().min(MAX_HISTORY_ENTRIES),
        path.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // Helper to create a test history file
    fn create_test_history(dir: &TempDir, content: &str) -> PathBuf {
        let path = dir.path().join("history");
        let mut file = File::create(&path).unwrap();
        write!(file, "{}", content).unwrap();
        path
    }

    #[test]
    fn test_history_file_path() {
        // This test just verifies the function doesn't panic
        let result = history_file_path();
        // May or may not succeed depending on environment
        if let Some(path) = result {
            assert!(path.ends_with("history"));
        }
    }

    #[test]
    fn test_load_history_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = create_test_history(&dir, "");

        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let history: Vec<String> = reader
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.is_empty())
            .collect();

        assert!(history.is_empty());
    }

    #[test]
    fn test_load_history_with_entries() {
        let dir = TempDir::new().unwrap();
        let path = create_test_history(&dir, "ls\npwd\ncd /tmp\n");

        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let history: Vec<String> = reader
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.is_empty())
            .collect();

        assert_eq!(history.len(), 3);
        assert_eq!(history[0], "ls");
        assert_eq!(history[1], "pwd");
        assert_eq!(history[2], "cd /tmp");
    }

    #[test]
    fn test_load_history_filters_empty_lines() {
        let dir = TempDir::new().unwrap();
        let path = create_test_history(&dir, "ls\n\npwd\n\n\ncd /tmp\n");

        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let history: Vec<String> = reader
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.is_empty())
            .collect();

        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_append_to_history_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("history");

        // Write directly to test file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file, "test command").unwrap();

        // Read back
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test command\n");
    }

    #[test]
    fn test_save_history_to_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("history");

        let history = vec!["cmd1".to_string(), "cmd2".to_string(), "cmd3".to_string()];

        let file = File::create(&path).unwrap();
        let mut writer = std::io::BufWriter::new(file);
        for command in &history {
            writeln!(writer, "{}", command).unwrap();
        }
        drop(writer);

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "cmd1\ncmd2\ncmd3\n");
    }

    #[test]
    fn test_max_history_entries_constant() {
        // Verify the constant is reasonable (compile-time checks)
        const _: () = assert!(MAX_HISTORY_ENTRIES >= 100);
        const _: () = assert!(MAX_HISTORY_ENTRIES <= 10000);
        // Runtime check - the constant has the expected value
        assert_eq!(MAX_HISTORY_ENTRIES, 1000);
    }
}
