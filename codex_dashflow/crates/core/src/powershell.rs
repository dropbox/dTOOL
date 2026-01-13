//! PowerShell command extraction utilities
//!
//! This module provides utilities for extracting the script body from PowerShell
//! command invocations, similar to how `bash.rs` handles bash commands.

use std::path::PathBuf;

use crate::shell::{detect_shell_type, ShellType};

/// Known PowerShell flags that can precede -Command
const POWERSHELL_FLAGS: &[&str] = &["-nologo", "-noprofile", "-command", "-c"];

/// Extract the PowerShell script body from an invocation.
///
/// Handles various PowerShell invocation patterns:
/// - `["pwsh", "-NoProfile", "-Command", "Get-ChildItem -Recurse | Select-String foo"]`
/// - `["powershell.exe", "-Command", "Write-Host hi"]`
/// - `["powershell", "-NoLogo", "-NoProfile", "-Command", "...script..."]`
///
/// # Arguments
///
/// * `command` - The command array where the first element is the shell path
///
/// # Returns
///
/// Returns `Some((shell, script))` when:
/// - The first argument is a PowerShell executable
/// - A `-Command` (or `-c`) flag is present followed by a script string
///
/// Returns `None` if the command doesn't match the expected pattern.
///
/// # Examples
///
/// ```no_run
/// use codex_dashflow_core::powershell::extract_powershell_command;
///
/// let cmd = vec![
///     "powershell".to_string(),
///     "-Command".to_string(),
///     "Write-Host hi".to_string(),
/// ];
/// let result = extract_powershell_command(&cmd);
/// assert!(result.is_some());
/// let (shell, script) = result.unwrap();
/// assert_eq!(script, "Write-Host hi");
/// ```
pub fn extract_powershell_command(command: &[String]) -> Option<(&str, &str)> {
    if command.len() < 3 {
        return None;
    }

    let shell = &command[0];
    if detect_shell_type(&PathBuf::from(shell)) != Some(ShellType::PowerShell) {
        return None;
    }

    // Find the first occurrence of -Command (accept common short alias -c as well)
    let mut i = 1usize;
    while i + 1 < command.len() {
        let flag = &command[i];
        // Reject unknown flags
        if !POWERSHELL_FLAGS.contains(&flag.to_ascii_lowercase().as_str()) {
            return None;
        }
        if flag.eq_ignore_ascii_case("-Command") || flag.eq_ignore_ascii_case("-c") {
            let script = &command[i + 1];
            return Some((shell, script.as_str()));
        }
        i += 1;
    }
    None
}

/// Check if a command array appears to be a PowerShell invocation.
///
/// This is a quick check that doesn't validate the full command structure.
pub fn is_powershell_command(command: &[String]) -> bool {
    if command.is_empty() {
        return false;
    }
    detect_shell_type(&PathBuf::from(&command[0])) == Some(ShellType::PowerShell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_basic_powershell_command() {
        let cmd = vec![
            "powershell".to_string(),
            "-Command".to_string(),
            "Write-Host hi".to_string(),
        ];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Write-Host hi");
    }

    #[test]
    fn extracts_lowercase_flags() {
        let cmd = vec![
            "powershell".to_string(),
            "-nologo".to_string(),
            "-command".to_string(),
            "Write-Host hi".to_string(),
        ];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Write-Host hi");
    }

    #[test]
    fn extracts_full_path_powershell_command() {
        let command = if cfg!(windows) {
            "C:\\windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe".to_string()
        } else {
            "/usr/local/bin/powershell.exe".to_string()
        };
        let cmd = vec![command, "-Command".to_string(), "Write-Host hi".to_string()];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Write-Host hi");
    }

    #[test]
    fn extracts_with_noprofile_and_alias() {
        let cmd = vec![
            "pwsh".to_string(),
            "-NoProfile".to_string(),
            "-c".to_string(),
            "Get-ChildItem | Select-String foo".to_string(),
        ];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Get-ChildItem | Select-String foo");
    }

    #[test]
    fn rejects_too_short_command() {
        let cmd = vec!["powershell".to_string(), "-Command".to_string()];
        assert!(extract_powershell_command(&cmd).is_none());
    }

    #[test]
    fn rejects_non_powershell() {
        let cmd = vec![
            "bash".to_string(),
            "-c".to_string(),
            "echo hello".to_string(),
        ];
        assert!(extract_powershell_command(&cmd).is_none());
    }

    #[test]
    fn rejects_unknown_flags() {
        let cmd = vec![
            "powershell".to_string(),
            "-UnknownFlag".to_string(),
            "-Command".to_string(),
            "Write-Host hi".to_string(),
        ];
        assert!(extract_powershell_command(&cmd).is_none());
    }

    #[test]
    fn extracts_pwsh() {
        let cmd = vec![
            "pwsh".to_string(),
            "-Command".to_string(),
            "Get-Process".to_string(),
        ];
        let (shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert!(shell.ends_with("pwsh"));
        assert_eq!(script, "Get-Process");
    }

    #[test]
    fn test_is_powershell_command() {
        assert!(is_powershell_command(&["powershell".to_string()]));
        assert!(is_powershell_command(&["pwsh".to_string()]));
        assert!(is_powershell_command(&["powershell.exe".to_string()]));
        assert!(is_powershell_command(&["pwsh.exe".to_string()]));
        assert!(!is_powershell_command(&["bash".to_string()]));
        assert!(!is_powershell_command(&[]));
    }

    #[test]
    fn extracts_with_multiple_preceding_flags() {
        let cmd = vec![
            "powershell".to_string(),
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Get-Date".to_string(),
        ];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Get-Date");
    }
}
