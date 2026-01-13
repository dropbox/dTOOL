//! Utilities for file management tools.
//!
//! This module provides path validation and security utilities to ensure
//! file operations are scoped to allowed directories.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error message template for invalid paths
pub const INVALID_PATH_TEMPLATE: &str =
    "Error: Access denied to {arg_name}: {value}. Permission granted exclusively to the current working directory";

/// Error type for file validation failures
#[derive(Debug, Error)]
#[error("Path {path} is outside of the allowed directory {root}")]
pub struct FileValidationError {
    /// The path that failed validation
    pub path: String,
    /// The root directory that was expected
    pub root: String,
}

/// Get a validated relative path, ensuring it's within the root directory.
///
/// This function resolves the path relative to the root and ensures the final
/// path is within the root directory. It prevents directory traversal attacks.
///
/// # Arguments
///
/// * `root` - The root directory to validate against
/// * `user_path` - The user-provided path (relative or absolute)
///
/// # Returns
///
/// The resolved path if it's within the root directory.
///
/// # Errors
///
/// Returns `FileValidationError` if the path is outside the root directory.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use dashflow_file_management::utils::get_validated_relative_path;
///
/// let root = Path::new("/tmp/sandbox");
/// let result = get_validated_relative_path(root, "file.txt");
/// // Returns Ok(/tmp/sandbox/file.txt) if /tmp/sandbox exists
/// ```
pub fn get_validated_relative_path(
    root: &Path,
    user_path: &str,
) -> Result<PathBuf, FileValidationError> {
    // Resolve root to canonical form if it exists
    let root_resolved = if root.exists() {
        root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
    } else {
        root.to_path_buf()
    };

    // Construct the full path
    let full_path = root_resolved.join(user_path);

    // Resolve the full path (this handles .. and . in the path)
    let full_path_resolved = if full_path.exists() {
        full_path
            .canonicalize()
            .unwrap_or_else(|_| full_path.clone())
    } else {
        // For non-existent paths, we need to manually resolve .. and .
        let mut resolved = PathBuf::new();
        for component in full_path.components() {
            match component {
                std::path::Component::ParentDir => {
                    resolved.pop();
                }
                std::path::Component::CurDir => {
                    // Skip
                }
                _ => {
                    resolved.push(component);
                }
            }
        }
        resolved
    };

    // Check if the resolved path starts with the root
    if !full_path_resolved.starts_with(&root_resolved) {
        return Err(FileValidationError {
            path: user_path.to_string(),
            root: root.display().to_string(),
        });
    }

    Ok(full_path_resolved)
}

/// Format an invalid path error message
#[must_use]
pub fn format_invalid_path_error(arg_name: &str, value: &str) -> String {
    INVALID_PATH_TEMPLATE
        .replace("{arg_name}", arg_name)
        .replace("{value}", value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_get_validated_relative_path_valid() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Create a test file
        let test_file = root.join("test.txt");
        fs::write(&test_file, "test").unwrap();

        let result = get_validated_relative_path(root, "test.txt");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("test.txt"));
    }

    #[test]
    fn test_get_validated_relative_path_traversal_attack() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Try to escape using ../
        let result = get_validated_relative_path(root, "../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_validated_relative_path_absolute_outside() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Try to use an absolute path outside root
        let result = get_validated_relative_path(root, "/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_validated_relative_path_nested() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Create nested directory
        let nested = root.join("nested");
        fs::create_dir(&nested).unwrap();

        let result = get_validated_relative_path(root, "nested/file.txt");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("nested/file.txt"));
    }

    #[test]
    fn test_format_invalid_path_error() {
        let error = format_invalid_path_error("file_path", "/etc/passwd");
        assert!(error.contains("file_path"));
        assert!(error.contains("/etc/passwd"));
        assert!(error.contains("Access denied"));
    }

    /// Regression test: filenames containing ".." should NOT be blocked.
    /// Only actual ".." path components (parent directory references) should be blocked,
    /// not filenames that happen to contain ".." as a substring.
    #[test]
    fn test_get_validated_relative_path_allows_dotdot_in_filename() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Create a file with ".." in the filename (valid filename, not traversal)
        let test_file = root.join("report..backup.json");
        fs::write(&test_file, "test").unwrap();

        let result = get_validated_relative_path(root, "report..backup.json");
        assert!(
            result.is_ok(),
            "Filename containing '..' should be allowed: {:?}",
            result.err()
        );
        assert!(result.unwrap().ends_with("report..backup.json"));
    }

    /// Regression test: leading dots in filenames should NOT be blocked.
    #[test]
    fn test_get_validated_relative_path_allows_leading_dots_filename() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Create a file with leading dots (valid filename)
        let test_file = root.join("...hidden");
        fs::write(&test_file, "test").unwrap();

        let result = get_validated_relative_path(root, "...hidden");
        assert!(
            result.is_ok(),
            "Filename with leading dots should be allowed: {:?}",
            result.err()
        );
        assert!(result.unwrap().ends_with("...hidden"));
    }

    /// Regression test: middle traversal should still be blocked.
    #[test]
    fn test_get_validated_relative_path_blocks_middle_traversal() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Try to escape using path traversal in the middle
        let result = get_validated_relative_path(root, "foo/../../../etc/passwd");
        assert!(
            result.is_err(),
            "Middle traversal attack should be blocked"
        );
    }

    /// Regression test: backslash traversal should be blocked on all platforms.
    #[test]
    fn test_get_validated_relative_path_blocks_backslash_traversal() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path();

        // Try Windows-style path traversal
        // Note: On Unix, backslashes are treated as literal characters in filenames,
        // so "..\etc\passwd" becomes a literal filename, not a traversal attack.
        // On Windows, this would be parsed as a traversal and blocked.
        let result = get_validated_relative_path(root, r"..\etc\passwd");

        // On Unix: result is Ok (backslashes are literal filename chars)
        // On Windows: result is Err (blocked as traversal attack)
        // Either way, the function correctly handles the input.
        if result.is_ok() {
            let resolved = result.unwrap();
            // Must compare against canonical root since the function uses canonical paths
            let root_canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
            assert!(
                resolved.starts_with(&root_canonical),
                "Path should stay within root even with backslashes"
            );
        }
        // If Err, the traversal was correctly blocked (Windows behavior)
    }
}
