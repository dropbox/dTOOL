//! WSL (Windows Subsystem for Linux) path utilities
//!
//! Provides helpers for detecting WSL environments and converting
//! between Windows and WSL path formats.

use std::ffi::OsStr;

/// Check if the current environment is running under WSL.
///
/// Detects WSL by checking:
/// 1. The `WSL_DISTRO_NAME` environment variable
/// 2. The contents of `/proc/version` for "microsoft"
///
/// # Returns
/// `true` if running under WSL, `false` otherwise.
pub fn is_wsl() -> bool {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WSL_DISTRO_NAME").is_some() {
            return true;
        }
        match std::fs::read_to_string("/proc/version") {
            Ok(version) => version.to_lowercase().contains("microsoft"),
            Err(_) => false,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Convert a Windows absolute path to a WSL mount path.
///
/// Transforms paths like `C:\foo\bar` or `C:/foo/bar` to `/mnt/c/foo/bar`.
///
/// # Arguments
/// * `path` - A Windows-style path string
///
/// # Returns
/// * `Some(wsl_path)` if the input is a valid Windows drive path
/// * `None` if the input doesn't look like a Windows drive path
///
/// # Examples
/// ```no_run
/// use codex_dashflow_core::wsl_paths::win_path_to_wsl;
///
/// assert_eq!(win_path_to_wsl(r"C:\Temp\file.txt"), Some("/mnt/c/Temp/file.txt".to_string()));
/// assert_eq!(win_path_to_wsl("D:/Work/project"), Some("/mnt/d/Work/project".to_string()));
/// assert_eq!(win_path_to_wsl("/home/user"), None);
/// ```
pub fn win_path_to_wsl(path: &str) -> Option<String> {
    let bytes = path.as_bytes();

    // Check minimum length and Windows path format: X:\... or X:/...
    if bytes.len() < 3
        || bytes[1] != b':'
        || !(bytes[2] == b'\\' || bytes[2] == b'/')
        || !bytes[0].is_ascii_alphabetic()
    {
        return None;
    }

    let drive = (bytes[0] as char).to_ascii_lowercase();
    let tail = path[3..].replace('\\', "/");

    if tail.is_empty() {
        return Some(format!("/mnt/{drive}"));
    }
    Some(format!("/mnt/{drive}/{tail}"))
}

/// Normalize a path for WSL if running under WSL.
///
/// If the current environment is WSL and the given path is a Windows-style
/// path, converts it to the equivalent WSL mount path. Otherwise returns
/// the input unchanged.
///
/// # Arguments
/// * `path` - Any path-like value
///
/// # Returns
/// The normalized path as a String.
///
/// # Examples
/// ```no_run
/// use codex_dashflow_core::wsl_paths::normalize_for_wsl;
///
/// // On non-WSL systems, returns input unchanged
/// let path = normalize_for_wsl("/home/user/file.txt");
/// // path is "/home/user/file.txt"
/// ```
pub fn normalize_for_wsl<P: AsRef<OsStr>>(path: P) -> String {
    let value = path.as_ref().to_string_lossy().to_string();

    if !is_wsl() {
        return value;
    }

    if let Some(mapped) = win_path_to_wsl(&value) {
        return mapped;
    }

    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_win_to_wsl_backslash() {
        assert_eq!(
            win_path_to_wsl(r"C:\Temp\codex.zip").as_deref(),
            Some("/mnt/c/Temp/codex.zip")
        );
    }

    #[test]
    fn test_win_to_wsl_forward_slash() {
        assert_eq!(
            win_path_to_wsl("D:/Work/codex.tgz").as_deref(),
            Some("/mnt/d/Work/codex.tgz")
        );
    }

    #[test]
    fn test_win_to_wsl_unix_path_returns_none() {
        assert!(win_path_to_wsl("/home/user/codex").is_none());
    }

    #[test]
    fn test_win_to_wsl_drive_only() {
        assert_eq!(win_path_to_wsl(r"C:\").as_deref(), Some("/mnt/c"));
        assert_eq!(win_path_to_wsl("C:/").as_deref(), Some("/mnt/c"));
    }

    #[test]
    fn test_win_to_wsl_preserves_case_in_path() {
        assert_eq!(
            win_path_to_wsl(r"C:\Users\Admin\Documents").as_deref(),
            Some("/mnt/c/Users/Admin/Documents")
        );
    }

    #[test]
    fn test_win_to_wsl_lowercase_drive() {
        // Drive letter should be lowercased
        assert_eq!(win_path_to_wsl(r"D:\data").as_deref(), Some("/mnt/d/data"));
        assert_eq!(win_path_to_wsl(r"d:\data").as_deref(), Some("/mnt/d/data"));
    }

    #[test]
    fn test_win_to_wsl_invalid_paths() {
        // Too short
        assert!(win_path_to_wsl("C:").is_none());
        assert!(win_path_to_wsl("C").is_none());

        // No drive letter
        assert!(win_path_to_wsl(r":\foo").is_none());

        // Non-alphabetic drive
        assert!(win_path_to_wsl(r"1:\foo").is_none());

        // Wrong separator
        assert!(win_path_to_wsl("C:foo").is_none());
    }

    #[test]
    fn test_normalize_is_noop_on_unix_paths() {
        assert_eq!(normalize_for_wsl("/home/u/x"), "/home/u/x");
    }

    #[test]
    fn test_normalize_handles_osstr() {
        let path = std::path::Path::new("/tmp/test");
        let result = normalize_for_wsl(path);
        assert_eq!(result, "/tmp/test");
    }

    #[test]
    fn test_is_wsl_returns_false_on_non_linux() {
        // On macOS and Windows (non-WSL), this should return false
        #[cfg(not(target_os = "linux"))]
        {
            assert!(!is_wsl());
        }
    }
}
