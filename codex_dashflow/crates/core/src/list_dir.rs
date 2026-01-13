//! Hierarchical directory listing tool
//!
//! Provides structured directory listings with depth control, pagination,
//! and formatting optimized for LLM consumption. Supports breadth-first
//! traversal with configurable limits.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::FileType;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Default number of entries to return (1-indexed offset)
pub const DEFAULT_OFFSET: usize = 1;
/// Default number of entries to return
pub const DEFAULT_LIMIT: usize = 25;
/// Default depth of directory traversal
pub const DEFAULT_DEPTH: usize = 2;
/// Maximum length for a single entry name
const MAX_ENTRY_LENGTH: usize = 500;
/// Number of spaces per indentation level
const INDENTATION_SPACES: usize = 2;

/// Result of a directory listing operation
#[derive(Debug)]
pub struct ListDirResult {
    /// The absolute path that was listed
    pub absolute_path: PathBuf,
    /// Formatted directory entries
    pub entries: Vec<String>,
    /// Whether there are more entries beyond the limit
    pub has_more: bool,
    /// Total entries found (before limit)
    pub total_entries: usize,
}

/// Error type for directory listing operations
#[derive(Debug, thiserror::Error)]
pub enum ListDirError {
    #[error("offset must be a 1-indexed entry number (got 0)")]
    ZeroOffset,
    #[error("limit must be greater than zero")]
    ZeroLimit,
    #[error("depth must be greater than zero")]
    ZeroDepth,
    #[error("path must be absolute: {0}")]
    RelativePath(PathBuf),
    #[error("offset {0} exceeds directory entry count {1}")]
    OffsetExceedsEntries(usize, usize),
    #[error("failed to read directory: {0}")]
    ReadDirError(std::io::Error),
    #[error("failed to inspect entry: {0}")]
    EntryInspectError(std::io::Error),
}

/// List a directory with hierarchical output
///
/// # Arguments
/// * `path` - Absolute path to the directory to list
/// * `offset` - 1-indexed starting entry number (default: 1)
/// * `limit` - Maximum number of entries to return (default: 25)
/// * `depth` - Maximum depth of subdirectory traversal (default: 2)
///
/// # Returns
/// A `ListDirResult` containing formatted entries and metadata
pub async fn list_dir(
    path: &Path,
    offset: usize,
    limit: usize,
    depth: usize,
) -> Result<ListDirResult, ListDirError> {
    // Validate parameters
    if offset == 0 {
        return Err(ListDirError::ZeroOffset);
    }
    if limit == 0 {
        return Err(ListDirError::ZeroLimit);
    }
    if depth == 0 {
        return Err(ListDirError::ZeroDepth);
    }
    if !path.is_absolute() {
        return Err(ListDirError::RelativePath(path.to_path_buf()));
    }

    // Collect all entries using BFS
    let mut entries = Vec::new();
    collect_entries(path, Path::new(""), depth, &mut entries).await?;

    let total_entries = entries.len();

    if entries.is_empty() {
        return Ok(ListDirResult {
            absolute_path: path.to_path_buf(),
            entries: Vec::new(),
            has_more: false,
            total_entries: 0,
        });
    }

    // Apply offset (1-indexed)
    let start_index = offset - 1;
    if start_index >= entries.len() {
        return Err(ListDirError::OffsetExceedsEntries(offset, entries.len()));
    }

    // Calculate slice bounds
    let remaining_entries = entries.len() - start_index;
    let capped_limit = limit.min(remaining_entries);
    let end_index = start_index + capped_limit;
    let has_more = end_index < entries.len();

    // Get and sort the selected entries
    let mut selected_entries = entries[start_index..end_index].to_vec();
    selected_entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    // Format entries
    let formatted: Vec<String> = selected_entries.iter().map(format_entry_line).collect();

    Ok(ListDirResult {
        absolute_path: path.to_path_buf(),
        entries: formatted,
        has_more,
        total_entries,
    })
}

/// Format the result as a string suitable for LLM consumption
pub fn format_result(result: &ListDirResult) -> String {
    let mut output = Vec::with_capacity(result.entries.len() + 2);
    output.push(format!("Absolute path: {}", result.absolute_path.display()));
    output.extend(result.entries.iter().cloned());
    if result.has_more {
        output.push(format!("More than {} entries found", result.entries.len()));
    }
    output.join("\n")
}

/// Internal entry representation
#[derive(Clone)]
struct DirEntry {
    /// Sortable name (full relative path)
    name: String,
    /// Display name (just the filename component)
    display_name: String,
    /// Depth in the directory tree
    depth: usize,
    /// Type of entry
    kind: DirEntryKind,
}

/// Type of directory entry
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<&FileType> for DirEntryKind {
    fn from(file_type: &FileType) -> Self {
        if file_type.is_symlink() {
            DirEntryKind::Symlink
        } else if file_type.is_dir() {
            DirEntryKind::Directory
        } else if file_type.is_file() {
            DirEntryKind::File
        } else {
            DirEntryKind::Other
        }
    }
}

/// Collect entries using breadth-first traversal
async fn collect_entries(
    dir_path: &Path,
    relative_prefix: &Path,
    depth: usize,
    entries: &mut Vec<DirEntry>,
) -> Result<(), ListDirError> {
    let mut queue = VecDeque::new();
    queue.push_back((dir_path.to_path_buf(), relative_prefix.to_path_buf(), depth));

    while let Some((current_dir, prefix, remaining_depth)) = queue.pop_front() {
        let mut read_dir = fs::read_dir(&current_dir)
            .await
            .map_err(ListDirError::ReadDirError)?;

        let mut dir_entries = Vec::new();

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(ListDirError::ReadDirError)?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(ListDirError::EntryInspectError)?;

            let file_name = entry.file_name();
            let relative_path = if prefix.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                prefix.join(&file_name)
            };

            let display_name = format_entry_component(&file_name);
            let display_depth = prefix.components().count();
            let sort_key = format_entry_name(&relative_path);
            let kind = DirEntryKind::from(&file_type);

            dir_entries.push((
                entry.path(),
                relative_path,
                kind,
                DirEntry {
                    name: sort_key,
                    display_name,
                    depth: display_depth,
                    kind,
                },
            ));
        }

        // Sort entries at this level for deterministic ordering
        dir_entries.sort_unstable_by(|a, b| a.3.name.cmp(&b.3.name));

        for (entry_path, relative_path, kind, dir_entry) in dir_entries {
            // Queue subdirectories for traversal if depth allows
            if kind == DirEntryKind::Directory && remaining_depth > 1 {
                queue.push_back((entry_path, relative_path, remaining_depth - 1));
            }
            entries.push(dir_entry);
        }
    }

    Ok(())
}

/// Format entry name, truncating if too long
fn format_entry_name(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.len() > MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized
    }
}

/// Format a single entry component (filename), truncating if too long
fn format_entry_component(name: &OsStr) -> String {
    let normalized = name.to_string_lossy();
    if normalized.len() > MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized.to_string()
    }
}

/// Format an entry line with proper indentation and type suffix
fn format_entry_line(entry: &DirEntry) -> String {
    let indent = " ".repeat(entry.depth * INDENTATION_SPACES);
    let mut name = entry.display_name.clone();
    match entry.kind {
        DirEntryKind::Directory => name.push('/'),
        DirEntryKind::Symlink => name.push('@'),
        DirEntryKind::Other => name.push('?'),
        DirEntryKind::File => {}
    }
    format!("{indent}{name}")
}

/// Take up to `n` bytes from string, respecting UTF-8 character boundaries
fn take_bytes_at_char_boundary(s: &str, n: usize) -> &str {
    if n >= s.len() {
        return s;
    }
    // Find the largest byte index <= n that is a char boundary
    let mut end = n;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_list_empty_directory() {
        let temp = tempdir().expect("create tempdir");
        let result = list_dir(temp.path(), 1, 10, 2).await;

        // Empty directory should return Ok with empty entries
        let result = result.expect("should succeed");
        assert!(result.entries.is_empty());
        assert!(!result.has_more);
        assert_eq!(result.total_entries, 0);
    }

    #[tokio::test]
    async fn test_list_single_file() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join("test.txt"), b"content")
            .await
            .expect("write file");

        let result = list_dir(temp.path(), 1, 10, 2)
            .await
            .expect("list directory");

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0], "test.txt");
        assert!(!result.has_more);
        assert_eq!(result.total_entries, 1);
    }

    #[tokio::test]
    async fn test_list_with_subdirectory() {
        let temp = tempdir().expect("create tempdir");
        let sub_dir = temp.path().join("nested");
        tokio::fs::create_dir(&sub_dir)
            .await
            .expect("create subdir");
        tokio::fs::write(temp.path().join("root.txt"), b"root")
            .await
            .expect("write root file");
        tokio::fs::write(sub_dir.join("child.txt"), b"child")
            .await
            .expect("write child file");

        let result = list_dir(temp.path(), 1, 10, 2)
            .await
            .expect("list directory");

        // Should have: nested/, nested/child.txt (indented), root.txt
        assert!(result.entries.iter().any(|e| e == "nested/"));
        assert!(result.entries.iter().any(|e| e.trim() == "child.txt"));
        assert!(result.entries.iter().any(|e| e == "root.txt"));
    }

    #[tokio::test]
    async fn test_depth_limit() {
        let temp = tempdir().expect("create tempdir");
        let nested = temp.path().join("nested");
        let deeper = nested.join("deeper");
        tokio::fs::create_dir(&nested).await.expect("create nested");
        tokio::fs::create_dir(&deeper).await.expect("create deeper");
        tokio::fs::write(temp.path().join("root.txt"), b"root")
            .await
            .expect("write root");
        tokio::fs::write(nested.join("child.txt"), b"child")
            .await
            .expect("write child");
        tokio::fs::write(deeper.join("grandchild.txt"), b"deep")
            .await
            .expect("write grandchild");

        // Depth 1: should only see immediate children
        let result_d1 = list_dir(temp.path(), 1, 10, 1).await.expect("depth 1");
        assert_eq!(result_d1.entries.len(), 2); // nested/, root.txt
        assert!(!result_d1.entries.iter().any(|e| e.contains("child")));

        // Depth 2: should see nested contents but not deeper
        let result_d2 = list_dir(temp.path(), 1, 20, 2).await.expect("depth 2");
        assert!(result_d2.entries.iter().any(|e| e.trim() == "child.txt"));
        assert!(result_d2.entries.iter().any(|e| e.trim() == "deeper/"));
        assert!(!result_d2.entries.iter().any(|e| e.contains("grandchild")));

        // Depth 3: should see everything
        let result_d3 = list_dir(temp.path(), 1, 30, 3).await.expect("depth 3");
        assert!(result_d3
            .entries
            .iter()
            .any(|e| e.trim() == "grandchild.txt"));
    }

    #[tokio::test]
    async fn test_offset_pagination() {
        let temp = tempdir().expect("create tempdir");
        for i in 0..5 {
            let file = temp.path().join(format!("file_{}.txt", i));
            tokio::fs::write(file, b"content")
                .await
                .expect("write file");
        }

        // Get entries 2-3 (offset 2, limit 2)
        let result = list_dir(temp.path(), 2, 2, 1)
            .await
            .expect("paginated list");

        assert_eq!(result.entries.len(), 2);
        assert!(result.has_more); // More entries exist
        assert_eq!(result.total_entries, 5);
    }

    #[tokio::test]
    async fn test_offset_exceeds_entries() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join("only.txt"), b"only")
            .await
            .expect("write file");

        let result = list_dir(temp.path(), 10, 5, 1).await;
        assert!(matches!(
            result,
            Err(ListDirError::OffsetExceedsEntries(10, 1))
        ));
    }

    #[tokio::test]
    async fn test_zero_offset_error() {
        let temp = tempdir().expect("create tempdir");
        let result = list_dir(temp.path(), 0, 10, 2).await;
        assert!(matches!(result, Err(ListDirError::ZeroOffset)));
    }

    #[tokio::test]
    async fn test_zero_limit_error() {
        let temp = tempdir().expect("create tempdir");
        let result = list_dir(temp.path(), 1, 0, 2).await;
        assert!(matches!(result, Err(ListDirError::ZeroLimit)));
    }

    #[tokio::test]
    async fn test_zero_depth_error() {
        let temp = tempdir().expect("create tempdir");
        let result = list_dir(temp.path(), 1, 10, 0).await;
        assert!(matches!(result, Err(ListDirError::ZeroDepth)));
    }

    #[tokio::test]
    async fn test_relative_path_error() {
        let result = list_dir(Path::new("relative/path"), 1, 10, 2).await;
        assert!(matches!(result, Err(ListDirError::RelativePath(_))));
    }

    #[tokio::test]
    async fn test_format_result() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join("file.txt"), b"content")
            .await
            .expect("write file");

        let result = list_dir(temp.path(), 1, 10, 2)
            .await
            .expect("list directory");
        let formatted = format_result(&result);

        assert!(formatted.starts_with("Absolute path:"));
        assert!(formatted.contains("file.txt"));
    }

    #[tokio::test]
    async fn test_format_result_with_truncation() {
        let temp = tempdir().expect("create tempdir");
        for i in 0..10 {
            let file = temp.path().join(format!("file_{}.txt", i));
            tokio::fs::write(file, b"content")
                .await
                .expect("write file");
        }

        let result = list_dir(temp.path(), 1, 5, 1)
            .await
            .expect("list directory");
        let formatted = format_result(&result);

        assert!(formatted.contains("More than 5 entries found"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_symlink_suffix() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("create tempdir");
        let target = temp.path().join("target.txt");
        let link = temp.path().join("link");

        tokio::fs::write(&target, b"target content")
            .await
            .expect("write target");
        symlink(&target, &link).expect("create symlink");

        let result = list_dir(temp.path(), 1, 10, 1)
            .await
            .expect("list directory");

        assert!(result.entries.iter().any(|e| e == "link@"));
        assert!(result.entries.iter().any(|e| e == "target.txt"));
    }

    #[test]
    fn test_take_bytes_at_char_boundary() {
        // ASCII - straightforward
        assert_eq!(take_bytes_at_char_boundary("hello", 3), "hel");
        assert_eq!(take_bytes_at_char_boundary("hello", 10), "hello");

        // Multi-byte UTF-8 characters
        let s = "héllo"; // é is 2 bytes
        assert_eq!(take_bytes_at_char_boundary(s, 1), "h");
        // Taking 2 bytes would cut into é, so we should get just "h"
        assert_eq!(take_bytes_at_char_boundary(s, 2), "h");
        // Taking 3 bytes gets us through é
        assert_eq!(take_bytes_at_char_boundary(s, 3), "hé");
    }

    #[test]
    fn test_take_bytes_empty_string() {
        assert_eq!(take_bytes_at_char_boundary("", 5), "");
    }

    #[tokio::test]
    async fn test_large_limit_no_overflow() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join("a.txt"), b"a")
            .await
            .expect("write a");
        tokio::fs::write(temp.path().join("b.txt"), b"b")
            .await
            .expect("write b");

        // Very large limit shouldn't cause issues
        let result = list_dir(temp.path(), 1, usize::MAX, 1)
            .await
            .expect("list with large limit");

        assert_eq!(result.entries.len(), 2);
        assert!(!result.has_more);
    }

    // === Constants tests ===

    #[test]
    fn test_default_offset_constant() {
        assert_eq!(DEFAULT_OFFSET, 1);
    }

    #[test]
    fn test_default_limit_constant() {
        assert_eq!(DEFAULT_LIMIT, 25);
    }

    #[test]
    fn test_default_depth_constant() {
        assert_eq!(DEFAULT_DEPTH, 2);
    }

    #[test]
    fn test_max_entry_length_constant() {
        assert_eq!(MAX_ENTRY_LENGTH, 500);
    }

    #[test]
    fn test_indentation_spaces_constant() {
        assert_eq!(INDENTATION_SPACES, 2);
    }

    // === ListDirResult tests ===

    #[test]
    fn test_list_dir_result_debug() {
        let result = ListDirResult {
            absolute_path: PathBuf::from("/test"),
            entries: vec!["file.txt".to_string()],
            has_more: false,
            total_entries: 1,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("ListDirResult"));
        assert!(debug_str.contains("/test"));
    }

    // === ListDirError tests ===

    #[test]
    fn test_list_dir_error_zero_offset_display() {
        let err = ListDirError::ZeroOffset;
        assert_eq!(
            err.to_string(),
            "offset must be a 1-indexed entry number (got 0)"
        );
    }

    #[test]
    fn test_list_dir_error_zero_limit_display() {
        let err = ListDirError::ZeroLimit;
        assert_eq!(err.to_string(), "limit must be greater than zero");
    }

    #[test]
    fn test_list_dir_error_zero_depth_display() {
        let err = ListDirError::ZeroDepth;
        assert_eq!(err.to_string(), "depth must be greater than zero");
    }

    #[test]
    fn test_list_dir_error_relative_path_display() {
        let err = ListDirError::RelativePath(PathBuf::from("foo/bar"));
        assert_eq!(err.to_string(), "path must be absolute: foo/bar");
    }

    #[test]
    fn test_list_dir_error_offset_exceeds_display() {
        let err = ListDirError::OffsetExceedsEntries(10, 5);
        assert_eq!(err.to_string(), "offset 10 exceeds directory entry count 5");
    }

    #[test]
    fn test_list_dir_error_read_dir_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let err = ListDirError::ReadDirError(io_err);
        assert!(err.to_string().starts_with("failed to read directory:"));
    }

    #[test]
    fn test_list_dir_error_entry_inspect_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
        let err = ListDirError::EntryInspectError(io_err);
        assert!(err.to_string().starts_with("failed to inspect entry:"));
    }

    #[test]
    fn test_list_dir_error_debug() {
        let err = ListDirError::ZeroOffset;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("ZeroOffset"));
    }

    // === DirEntryKind tests ===

    #[test]
    fn test_dir_entry_kind_clone() {
        let kind = DirEntryKind::Directory;
        let cloned = kind;
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_dir_entry_kind_copy() {
        let kind = DirEntryKind::File;
        let copied: DirEntryKind = kind; // Copy
        assert_eq!(kind, copied);
    }

    #[test]
    fn test_dir_entry_kind_partial_eq() {
        assert_eq!(DirEntryKind::Directory, DirEntryKind::Directory);
        assert_ne!(DirEntryKind::Directory, DirEntryKind::File);
        assert_ne!(DirEntryKind::File, DirEntryKind::Symlink);
        assert_ne!(DirEntryKind::Symlink, DirEntryKind::Other);
    }

    #[test]
    fn test_dir_entry_kind_from_file_type_dir() {
        // Test the conversion logic (we can't easily create FileType, so test via format_entry_line)
        let entry = DirEntry {
            name: "dir".to_string(),
            display_name: "dir".to_string(),
            depth: 0,
            kind: DirEntryKind::Directory,
        };
        let line = format_entry_line(&entry);
        assert!(line.ends_with('/'));
    }

    #[test]
    fn test_dir_entry_kind_symlink_format() {
        let entry = DirEntry {
            name: "link".to_string(),
            display_name: "link".to_string(),
            depth: 0,
            kind: DirEntryKind::Symlink,
        };
        let line = format_entry_line(&entry);
        assert!(line.ends_with('@'));
    }

    #[test]
    fn test_dir_entry_kind_other_format() {
        let entry = DirEntry {
            name: "other".to_string(),
            display_name: "other".to_string(),
            depth: 0,
            kind: DirEntryKind::Other,
        };
        let line = format_entry_line(&entry);
        assert!(line.ends_with('?'));
    }

    #[test]
    fn test_dir_entry_kind_file_format_no_suffix() {
        let entry = DirEntry {
            name: "file.txt".to_string(),
            display_name: "file.txt".to_string(),
            depth: 0,
            kind: DirEntryKind::File,
        };
        let line = format_entry_line(&entry);
        assert_eq!(line, "file.txt");
    }

    // === DirEntry tests ===

    #[test]
    fn test_dir_entry_clone() {
        let entry = DirEntry {
            name: "test".to_string(),
            display_name: "test".to_string(),
            depth: 1,
            kind: DirEntryKind::File,
        };
        let cloned = entry.clone();
        assert_eq!(cloned.name, "test");
        assert_eq!(cloned.depth, 1);
    }

    // === format_entry_line tests ===

    #[test]
    fn test_format_entry_line_depth_zero() {
        let entry = DirEntry {
            name: "file".to_string(),
            display_name: "file".to_string(),
            depth: 0,
            kind: DirEntryKind::File,
        };
        let line = format_entry_line(&entry);
        assert_eq!(line, "file");
    }

    #[test]
    fn test_format_entry_line_depth_one() {
        let entry = DirEntry {
            name: "nested/file".to_string(),
            display_name: "file".to_string(),
            depth: 1,
            kind: DirEntryKind::File,
        };
        let line = format_entry_line(&entry);
        assert_eq!(line, "  file"); // 2 spaces indent
    }

    #[test]
    fn test_format_entry_line_depth_two() {
        let entry = DirEntry {
            name: "a/b/file".to_string(),
            display_name: "file".to_string(),
            depth: 2,
            kind: DirEntryKind::File,
        };
        let line = format_entry_line(&entry);
        assert_eq!(line, "    file"); // 4 spaces indent
    }

    #[test]
    fn test_format_entry_line_directory_with_indent() {
        let entry = DirEntry {
            name: "parent/subdir".to_string(),
            display_name: "subdir".to_string(),
            depth: 1,
            kind: DirEntryKind::Directory,
        };
        let line = format_entry_line(&entry);
        assert_eq!(line, "  subdir/");
    }

    // === format_entry_name tests ===

    #[test]
    fn test_format_entry_name_simple() {
        let path = Path::new("file.txt");
        let name = format_entry_name(path);
        assert_eq!(name, "file.txt");
    }

    #[test]
    fn test_format_entry_name_with_path() {
        let path = Path::new("dir/subdir/file.txt");
        let name = format_entry_name(path);
        assert_eq!(name, "dir/subdir/file.txt");
    }

    #[cfg(windows)]
    #[test]
    fn test_format_entry_name_backslash_replacement() {
        let path = Path::new("dir\\subdir\\file.txt");
        let name = format_entry_name(path);
        assert_eq!(name, "dir/subdir/file.txt");
    }

    #[test]
    fn test_format_entry_name_truncation() {
        let long_name = "a".repeat(600);
        let path = PathBuf::from(long_name);
        let name = format_entry_name(&path);
        assert!(name.len() <= MAX_ENTRY_LENGTH);
    }

    // === format_entry_component tests ===

    #[test]
    fn test_format_entry_component_simple() {
        let name = std::ffi::OsStr::new("file.txt");
        let formatted = format_entry_component(name);
        assert_eq!(formatted, "file.txt");
    }

    #[test]
    fn test_format_entry_component_truncation() {
        let long_name = "b".repeat(600);
        let name = std::ffi::OsString::from(long_name);
        let formatted = format_entry_component(&name);
        assert!(formatted.len() <= MAX_ENTRY_LENGTH);
    }

    // === take_bytes_at_char_boundary edge cases ===

    #[test]
    fn test_take_bytes_zero_len() {
        assert_eq!(take_bytes_at_char_boundary("hello", 0), "");
    }

    #[test]
    fn test_take_bytes_emoji() {
        let s = "👋hello";
        // Emoji is 4 bytes
        assert_eq!(take_bytes_at_char_boundary(s, 1), "");
        assert_eq!(take_bytes_at_char_boundary(s, 2), "");
        assert_eq!(take_bytes_at_char_boundary(s, 3), "");
        assert_eq!(take_bytes_at_char_boundary(s, 4), "👋");
        assert_eq!(take_bytes_at_char_boundary(s, 5), "👋h");
    }

    #[test]
    fn test_take_bytes_cjk_characters() {
        let s = "日本語test"; // Each CJK char is 3 bytes
        assert_eq!(take_bytes_at_char_boundary(s, 3), "日");
        assert_eq!(take_bytes_at_char_boundary(s, 6), "日本");
        assert_eq!(take_bytes_at_char_boundary(s, 9), "日本語");
        assert_eq!(take_bytes_at_char_boundary(s, 10), "日本語t");
    }

    #[test]
    fn test_take_bytes_exact_len() {
        assert_eq!(take_bytes_at_char_boundary("abc", 3), "abc");
    }

    // === format_result edge cases ===

    #[test]
    fn test_format_result_empty_entries() {
        let result = ListDirResult {
            absolute_path: PathBuf::from("/empty"),
            entries: Vec::new(),
            has_more: false,
            total_entries: 0,
        };
        let formatted = format_result(&result);
        assert_eq!(formatted, "Absolute path: /empty");
    }

    #[test]
    fn test_format_result_multiple_entries() {
        let result = ListDirResult {
            absolute_path: PathBuf::from("/test"),
            entries: vec![
                "a.txt".to_string(),
                "b.txt".to_string(),
                "c.txt".to_string(),
            ],
            has_more: false,
            total_entries: 3,
        };
        let formatted = format_result(&result);
        let lines: Vec<&str> = formatted.lines().collect();
        assert_eq!(lines.len(), 4); // path + 3 entries
        assert!(lines[0].starts_with("Absolute path:"));
        assert_eq!(lines[1], "a.txt");
        assert_eq!(lines[2], "b.txt");
        assert_eq!(lines[3], "c.txt");
    }

    #[test]
    fn test_format_result_has_more_message() {
        let result = ListDirResult {
            absolute_path: PathBuf::from("/test"),
            entries: vec!["a.txt".to_string(), "b.txt".to_string()],
            has_more: true,
            total_entries: 10,
        };
        let formatted = format_result(&result);
        assert!(formatted.contains("More than 2 entries found"));
    }

    // === Additional list_dir edge cases ===

    #[tokio::test]
    async fn test_list_dir_hidden_files() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join(".hidden"), b"hidden")
            .await
            .expect("write hidden");
        tokio::fs::write(temp.path().join("visible.txt"), b"visible")
            .await
            .expect("write visible");

        let result = list_dir(temp.path(), 1, 10, 1)
            .await
            .expect("list directory");

        // Both hidden and visible files should be included
        assert_eq!(result.total_entries, 2);
        assert!(result.entries.iter().any(|e| e == ".hidden"));
        assert!(result.entries.iter().any(|e| e == "visible.txt"));
    }

    #[tokio::test]
    async fn test_list_dir_unicode_filenames() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join("日本語.txt"), b"japanese")
            .await
            .expect("write unicode file");
        tokio::fs::write(temp.path().join("émoji_🎉.txt"), b"emoji")
            .await
            .expect("write emoji file");

        let result = list_dir(temp.path(), 1, 10, 1)
            .await
            .expect("list directory");

        assert_eq!(result.total_entries, 2);
        assert!(result.entries.iter().any(|e| e.contains("日本語")));
        assert!(result.entries.iter().any(|e| e.contains("🎉")));
    }

    #[tokio::test]
    async fn test_list_dir_exact_limit() {
        let temp = tempdir().expect("create tempdir");
        for i in 0..5 {
            tokio::fs::write(temp.path().join(format!("file_{}.txt", i)), b"content")
                .await
                .expect("write file");
        }

        // Limit exactly matches entry count
        let result = list_dir(temp.path(), 1, 5, 1)
            .await
            .expect("list directory");

        assert_eq!(result.entries.len(), 5);
        assert!(!result.has_more);
        assert_eq!(result.total_entries, 5);
    }

    #[tokio::test]
    async fn test_list_dir_offset_at_end() {
        let temp = tempdir().expect("create tempdir");
        for i in 0..3 {
            tokio::fs::write(temp.path().join(format!("file_{}.txt", i)), b"content")
                .await
                .expect("write file");
        }

        // Offset to last entry
        let result = list_dir(temp.path(), 3, 10, 1)
            .await
            .expect("list directory");

        assert_eq!(result.entries.len(), 1);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_dir_multiple_subdirs_same_level() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::create_dir(temp.path().join("alpha"))
            .await
            .expect("create alpha");
        tokio::fs::create_dir(temp.path().join("beta"))
            .await
            .expect("create beta");
        tokio::fs::create_dir(temp.path().join("gamma"))
            .await
            .expect("create gamma");

        let result = list_dir(temp.path(), 1, 10, 1)
            .await
            .expect("list directory");

        assert_eq!(result.total_entries, 3);
        // Entries should be sorted alphabetically
        assert!(result.entries[0].contains("alpha"));
        assert!(result.entries[1].contains("beta"));
        assert!(result.entries[2].contains("gamma"));
    }

    #[tokio::test]
    async fn test_list_dir_nested_empty_subdirs() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::create_dir(temp.path().join("empty_dir"))
            .await
            .expect("create empty dir");
        tokio::fs::create_dir(temp.path().join("empty_dir/also_empty"))
            .await
            .expect("create nested empty dir");

        let result = list_dir(temp.path(), 1, 10, 2)
            .await
            .expect("list directory");

        // Should see both empty directories
        assert!(result.entries.iter().any(|e| e == "empty_dir/"));
        assert!(result.entries.iter().any(|e| e.trim() == "also_empty/"));
    }

    #[tokio::test]
    async fn test_list_dir_nonexistent_path() {
        let result = list_dir(Path::new("/nonexistent/path/that/does/not/exist"), 1, 10, 1).await;
        assert!(matches!(result, Err(ListDirError::ReadDirError(_))));
    }

    #[tokio::test]
    async fn test_list_dir_file_not_directory() {
        let temp = tempdir().expect("create tempdir");
        let file_path = temp.path().join("file.txt");
        tokio::fs::write(&file_path, b"content")
            .await
            .expect("write file");

        // Trying to list a file as a directory should fail
        let result = list_dir(&file_path, 1, 10, 1).await;
        assert!(matches!(result, Err(ListDirError::ReadDirError(_))));
    }

    #[tokio::test]
    async fn test_list_dir_deeply_nested() {
        let temp = tempdir().expect("create tempdir");
        let deep = temp.path().join("a/b/c/d/e");
        tokio::fs::create_dir_all(&deep)
            .await
            .expect("create deep path");
        tokio::fs::write(deep.join("deep_file.txt"), b"deep")
            .await
            .expect("write deep file");

        // Depth 6 needed: a(1)/b(2)/c(3)/d(4)/e(5)/deep_file.txt(6)
        let result = list_dir(temp.path(), 1, 50, 6)
            .await
            .expect("list directory");

        assert!(result.entries.iter().any(|e| e.trim() == "deep_file.txt"));
    }

    #[tokio::test]
    async fn test_list_dir_result_absolute_path_is_correct() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join("file.txt"), b"content")
            .await
            .expect("write file");

        let result = list_dir(temp.path(), 1, 10, 1)
            .await
            .expect("list directory");

        assert_eq!(result.absolute_path, temp.path());
    }

    #[tokio::test]
    async fn test_list_dir_mixed_files_and_dirs() {
        let temp = tempdir().expect("create tempdir");
        tokio::fs::write(temp.path().join("aaa_file.txt"), b"file")
            .await
            .expect("write file");
        tokio::fs::create_dir(temp.path().join("bbb_dir"))
            .await
            .expect("create dir");
        tokio::fs::write(temp.path().join("ccc_file.txt"), b"file")
            .await
            .expect("write file");

        let result = list_dir(temp.path(), 1, 10, 1)
            .await
            .expect("list directory");

        assert_eq!(result.total_entries, 3);
        // Check correct suffixes
        assert!(result.entries.iter().any(|e| e == "aaa_file.txt"));
        assert!(result.entries.iter().any(|e| e == "bbb_dir/"));
        assert!(result.entries.iter().any(|e| e == "ccc_file.txt"));
    }

    #[tokio::test]
    async fn test_list_dir_offset_and_limit_combined() {
        let temp = tempdir().expect("create tempdir");
        for i in 0..10 {
            tokio::fs::write(temp.path().join(format!("file_{:02}.txt", i)), b"content")
                .await
                .expect("write file");
        }

        // Get middle section: offset 4, limit 3 (should get files 3,4,5 after sorting)
        let result = list_dir(temp.path(), 4, 3, 1)
            .await
            .expect("list directory");

        assert_eq!(result.entries.len(), 3);
        assert!(result.has_more);
        assert_eq!(result.total_entries, 10);
    }
}
