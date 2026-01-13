//! Shell detection and execution utilities.
//!
//! This module provides shell type detection, path resolution, and command
//! argument derivation for cross-platform shell execution.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Supported shell types for command execution.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
}

/// A resolved shell with its type and path.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Shell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
}

impl Shell {
    /// Returns the canonical name for this shell type.
    pub fn name(&self) -> &'static str {
        match self.shell_type {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::PowerShell => "powershell",
            ShellType::Sh => "sh",
            ShellType::Cmd => "cmd",
        }
    }

    /// Takes a command string and returns the full list of command args to
    /// use with `exec()` to run the shell command.
    pub fn derive_exec_args(&self, command: &str, use_login_shell: bool) -> Vec<String> {
        match self.shell_type {
            ShellType::Zsh | ShellType::Bash | ShellType::Sh => {
                let arg = if use_login_shell { "-lc" } else { "-c" };
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    arg.to_string(),
                    command.to_string(),
                ]
            }
            ShellType::PowerShell => {
                let mut args = vec![self.shell_path.to_string_lossy().to_string()];
                if !use_login_shell {
                    args.push("-NoProfile".to_string());
                }
                args.push("-Command".to_string());
                args.push(command.to_string());
                args
            }
            ShellType::Cmd => {
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    "/c".to_string(),
                    command.to_string(),
                ]
            }
        }
    }
}

/// Get the user's default shell path from the system.
#[cfg(unix)]
fn get_user_shell_path() -> Option<PathBuf> {
    use libc::{getpwuid, getuid};
    use std::ffi::CStr;

    unsafe {
        let uid = getuid();
        let pw = getpwuid(uid);

        if !pw.is_null() {
            let shell_path = CStr::from_ptr((*pw).pw_shell)
                .to_string_lossy()
                .into_owned();
            Some(PathBuf::from(shell_path))
        } else {
            None
        }
    }
}

#[cfg(not(unix))]
fn get_user_shell_path() -> Option<PathBuf> {
    None
}

/// Check if a file exists at the given path.
fn file_exists(path: &PathBuf) -> Option<PathBuf> {
    if std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        Some(path.clone())
    } else {
        None
    }
}

/// Resolve a shell path, checking provided path, user default, which, and fallbacks.
fn get_shell_path(
    shell_type: ShellType,
    provided_path: Option<&PathBuf>,
    binary_name: &str,
    fallback_paths: &[&str],
) -> Option<PathBuf> {
    // If exact provided path exists, use it
    if provided_path.and_then(file_exists).is_some() {
        return provided_path.cloned();
    }

    // Check if the shell we are trying to load is user's default shell
    let default_shell_path = get_user_shell_path();
    if let Some(ref default_path) = default_shell_path {
        if detect_shell_type(default_path) == Some(shell_type) {
            return default_shell_path;
        }
    }

    // Try to find via which
    if let Ok(path) = which::which(binary_name) {
        return Some(path);
    }

    // Check fallback paths
    for path in fallback_paths {
        if let Some(path) = file_exists(&PathBuf::from(path)) {
            return Some(path);
        }
    }

    None
}

fn get_zsh_shell(path: Option<&PathBuf>) -> Option<Shell> {
    get_shell_path(ShellType::Zsh, path, "zsh", &["/bin/zsh"]).map(|shell_path| Shell {
        shell_type: ShellType::Zsh,
        shell_path,
    })
}

fn get_bash_shell(path: Option<&PathBuf>) -> Option<Shell> {
    get_shell_path(ShellType::Bash, path, "bash", &["/bin/bash"]).map(|shell_path| Shell {
        shell_type: ShellType::Bash,
        shell_path,
    })
}

fn get_sh_shell(path: Option<&PathBuf>) -> Option<Shell> {
    get_shell_path(ShellType::Sh, path, "sh", &["/bin/sh"]).map(|shell_path| Shell {
        shell_type: ShellType::Sh,
        shell_path,
    })
}

fn get_powershell_shell(path: Option<&PathBuf>) -> Option<Shell> {
    get_shell_path(
        ShellType::PowerShell,
        path,
        "pwsh",
        &["/usr/local/bin/pwsh"],
    )
    .or_else(|| get_shell_path(ShellType::PowerShell, path, "powershell", &[]))
    .map(|shell_path| Shell {
        shell_type: ShellType::PowerShell,
        shell_path,
    })
}

fn get_cmd_shell(path: Option<&PathBuf>) -> Option<Shell> {
    get_shell_path(ShellType::Cmd, path, "cmd", &[]).map(|shell_path| Shell {
        shell_type: ShellType::Cmd,
        shell_path,
    })
}

/// Returns the ultimate fallback shell when nothing else works.
fn ultimate_fallback_shell() -> Shell {
    if cfg!(windows) {
        Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
        }
    } else {
        Shell {
            shell_type: ShellType::Sh,
            shell_path: PathBuf::from("/bin/sh"),
        }
    }
}

/// Get a shell by looking up the model-provided path.
pub fn get_shell_by_model_provided_path(shell_path: &PathBuf) -> Shell {
    detect_shell_type(shell_path)
        .and_then(|shell_type| get_shell(shell_type, Some(shell_path)))
        .unwrap_or_else(ultimate_fallback_shell)
}

/// Get a specific shell type, optionally from a specific path.
pub fn get_shell(shell_type: ShellType, path: Option<&PathBuf>) -> Option<Shell> {
    match shell_type {
        ShellType::Zsh => get_zsh_shell(path),
        ShellType::Bash => get_bash_shell(path),
        ShellType::PowerShell => get_powershell_shell(path),
        ShellType::Sh => get_sh_shell(path),
        ShellType::Cmd => get_cmd_shell(path),
    }
}

/// Detect the shell type from a path by examining the filename.
pub fn detect_shell_type(shell_path: &PathBuf) -> Option<ShellType> {
    match shell_path.as_os_str().to_str() {
        Some("zsh") => Some(ShellType::Zsh),
        Some("sh") => Some(ShellType::Sh),
        Some("cmd") => Some(ShellType::Cmd),
        Some("bash") => Some(ShellType::Bash),
        Some("pwsh") => Some(ShellType::PowerShell),
        Some("powershell") => Some(ShellType::PowerShell),
        _ => {
            // Try extracting the file stem (e.g., "zsh" from "/bin/zsh")
            let shell_name = shell_path.file_stem()?;
            let shell_name_path = PathBuf::from(shell_name);
            if shell_name_path != *shell_path {
                detect_shell_type(&shell_name_path)
            } else {
                None
            }
        }
    }
}

/// Get the user's default shell with platform-appropriate fallbacks.
pub fn default_user_shell() -> Shell {
    default_user_shell_from_path(get_user_shell_path())
}

fn default_user_shell_from_path(user_shell_path: Option<PathBuf>) -> Shell {
    if cfg!(windows) {
        get_shell(ShellType::PowerShell, None).unwrap_or_else(ultimate_fallback_shell)
    } else {
        let user_default_shell = user_shell_path
            .and_then(|shell| detect_shell_type(&shell))
            .and_then(|shell_type| get_shell(shell_type, None));

        let shell_with_fallback = if cfg!(target_os = "macos") {
            user_default_shell
                .or_else(|| get_shell(ShellType::Zsh, None))
                .or_else(|| get_shell(ShellType::Bash, None))
        } else {
            user_default_shell
                .or_else(|| get_shell(ShellType::Bash, None))
                .or_else(|| get_shell(ShellType::Zsh, None))
        };

        shell_with_fallback.unwrap_or_else(ultimate_fallback_shell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_type_direct_names() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(detect_shell_type(&PathBuf::from("sh")), Some(ShellType::Sh));
        assert_eq!(
            detect_shell_type(&PathBuf::from("cmd")),
            Some(ShellType::Cmd)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("powershell")),
            Some(ShellType::PowerShell)
        );
    }

    #[test]
    fn test_detect_shell_type_full_paths() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/sh")),
            Some(ShellType::Sh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/usr/local/bin/pwsh")),
            Some(ShellType::PowerShell)
        );
    }

    #[test]
    fn test_detect_shell_type_windows_paths() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("cmd.exe")),
            Some(ShellType::Cmd)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("pwsh.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("powershell.exe")),
            Some(ShellType::PowerShell)
        );
    }

    #[test]
    fn test_detect_shell_type_unknown() {
        assert_eq!(detect_shell_type(&PathBuf::from("fish")), None);
        assert_eq!(detect_shell_type(&PathBuf::from("other")), None);
        assert_eq!(detect_shell_type(&PathBuf::from("/usr/bin/fish")), None);
    }

    #[test]
    fn test_shell_name() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };
        assert_eq!(shell.name(), "bash");

        let shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
        };
        assert_eq!(shell.name(), "zsh");
    }

    #[test]
    fn test_derive_exec_args_unix_shells() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };

        let args = shell.derive_exec_args("echo hello", false);
        assert_eq!(args, vec!["/bin/bash", "-c", "echo hello"]);

        let args = shell.derive_exec_args("echo hello", true);
        assert_eq!(args, vec!["/bin/bash", "-lc", "echo hello"]);
    }

    #[test]
    fn test_derive_exec_args_powershell() {
        let shell = Shell {
            shell_type: ShellType::PowerShell,
            shell_path: PathBuf::from("pwsh"),
        };

        let args = shell.derive_exec_args("Write-Host 'hello'", false);
        assert_eq!(
            args,
            vec!["pwsh", "-NoProfile", "-Command", "Write-Host 'hello'"]
        );

        let args = shell.derive_exec_args("Write-Host 'hello'", true);
        assert_eq!(args, vec!["pwsh", "-Command", "Write-Host 'hello'"]);
    }

    #[test]
    fn test_derive_exec_args_cmd() {
        let shell = Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
        };

        let args = shell.derive_exec_args("echo hello", false);
        assert_eq!(args, vec!["cmd.exe", "/c", "echo hello"]);
    }

    #[test]
    fn test_ultimate_fallback_shell() {
        let shell = ultimate_fallback_shell();
        if cfg!(windows) {
            assert_eq!(shell.shell_type, ShellType::Cmd);
        } else {
            assert_eq!(shell.shell_type, ShellType::Sh);
            assert_eq!(shell.shell_path, PathBuf::from("/bin/sh"));
        }
    }

    #[test]
    fn test_get_shell_by_model_provided_path() {
        let shell = get_shell_by_model_provided_path(&PathBuf::from("/unknown/shell"));
        // Should fall back to ultimate fallback
        if cfg!(windows) {
            assert_eq!(shell.shell_type, ShellType::Cmd);
        } else {
            assert_eq!(shell.shell_type, ShellType::Sh);
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_get_shell_bash() {
        if let Some(shell) = get_shell(ShellType::Bash, None) {
            assert_eq!(shell.shell_type, ShellType::Bash);
            assert!(
                shell.shell_path.to_string_lossy().contains("bash"),
                "Expected bash in path: {:?}",
                shell.shell_path
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_get_shell_sh() {
        let shell = get_shell(ShellType::Sh, None).expect("sh should exist on Unix");
        assert_eq!(shell.shell_type, ShellType::Sh);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_get_shell_zsh_macos() {
        let shell = get_shell(ShellType::Zsh, None).expect("zsh should exist on macOS");
        assert_eq!(shell.shell_type, ShellType::Zsh);
        assert_eq!(shell.shell_path, PathBuf::from("/bin/zsh"));
    }

    #[test]
    fn test_default_user_shell() {
        let shell = default_user_shell();
        // Should always return something
        assert!(!shell.shell_path.as_os_str().is_empty());
    }

    // === Additional tests for comprehensive coverage ===

    // ShellType trait tests
    #[test]
    fn test_shell_type_debug() {
        assert_eq!(format!("{:?}", ShellType::Zsh), "Zsh");
        assert_eq!(format!("{:?}", ShellType::Bash), "Bash");
        assert_eq!(format!("{:?}", ShellType::PowerShell), "PowerShell");
        assert_eq!(format!("{:?}", ShellType::Sh), "Sh");
        assert_eq!(format!("{:?}", ShellType::Cmd), "Cmd");
    }

    #[test]
    fn test_shell_type_clone() {
        let shell = ShellType::Bash;
        let cloned = shell;
        assert_eq!(shell, cloned);
    }

    #[test]
    fn test_shell_type_copy() {
        let shell = ShellType::Zsh;
        let copied = shell; // Copy, not move
        assert_eq!(shell, copied); // Both still valid
    }

    #[test]
    fn test_shell_type_eq() {
        assert_eq!(ShellType::Bash, ShellType::Bash);
        assert_ne!(ShellType::Bash, ShellType::Zsh);
        assert_ne!(ShellType::PowerShell, ShellType::Cmd);
    }

    #[test]
    fn test_shell_type_serde_roundtrip() {
        for shell_type in [
            ShellType::Zsh,
            ShellType::Bash,
            ShellType::PowerShell,
            ShellType::Sh,
            ShellType::Cmd,
        ] {
            let json = serde_json::to_string(&shell_type).unwrap();
            let parsed: ShellType = serde_json::from_str(&json).unwrap();
            assert_eq!(shell_type, parsed);
        }
    }

    #[test]
    fn test_shell_type_serialize() {
        assert_eq!(serde_json::to_string(&ShellType::Zsh).unwrap(), "\"Zsh\"");
        assert_eq!(serde_json::to_string(&ShellType::Bash).unwrap(), "\"Bash\"");
        assert_eq!(
            serde_json::to_string(&ShellType::PowerShell).unwrap(),
            "\"PowerShell\""
        );
        assert_eq!(serde_json::to_string(&ShellType::Sh).unwrap(), "\"Sh\"");
        assert_eq!(serde_json::to_string(&ShellType::Cmd).unwrap(), "\"Cmd\"");
    }

    // Shell struct tests
    #[test]
    fn test_shell_debug() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };
        let debug = format!("{:?}", shell);
        assert!(debug.contains("Shell"));
        assert!(debug.contains("Bash"));
        assert!(debug.contains("/bin/bash"));
    }

    #[test]
    fn test_shell_clone() {
        let shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
        };
        let cloned = shell.clone();
        assert_eq!(shell.shell_type, cloned.shell_type);
        assert_eq!(shell.shell_path, cloned.shell_path);
    }

    #[test]
    fn test_shell_eq() {
        let shell1 = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };
        let shell2 = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };
        let shell3 = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/usr/bin/bash"),
        };
        assert_eq!(shell1, shell2);
        assert_ne!(shell1, shell3);
    }

    #[test]
    fn test_shell_serde_roundtrip() {
        let shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
        };
        let json = serde_json::to_string(&shell).unwrap();
        let parsed: Shell = serde_json::from_str(&json).unwrap();
        assert_eq!(shell, parsed);
    }

    // Shell::name() tests for all variants
    #[test]
    fn test_shell_name_all_variants() {
        let variants = [
            (ShellType::Zsh, "zsh"),
            (ShellType::Bash, "bash"),
            (ShellType::PowerShell, "powershell"),
            (ShellType::Sh, "sh"),
            (ShellType::Cmd, "cmd"),
        ];
        for (shell_type, expected_name) in variants {
            let shell = Shell {
                shell_type,
                shell_path: PathBuf::from("any"),
            };
            assert_eq!(shell.name(), expected_name);
        }
    }

    // derive_exec_args tests for remaining shells
    #[test]
    fn test_derive_exec_args_zsh() {
        let shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
        };

        let args = shell.derive_exec_args("ls -la", false);
        assert_eq!(args, vec!["/bin/zsh", "-c", "ls -la"]);

        let args = shell.derive_exec_args("ls -la", true);
        assert_eq!(args, vec!["/bin/zsh", "-lc", "ls -la"]);
    }

    #[test]
    fn test_derive_exec_args_sh() {
        let shell = Shell {
            shell_type: ShellType::Sh,
            shell_path: PathBuf::from("/bin/sh"),
        };

        let args = shell.derive_exec_args("echo test", false);
        assert_eq!(args, vec!["/bin/sh", "-c", "echo test"]);

        let args = shell.derive_exec_args("echo test", true);
        assert_eq!(args, vec!["/bin/sh", "-lc", "echo test"]);
    }

    #[test]
    fn test_derive_exec_args_cmd_login_flag_ignored() {
        let shell = Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
        };

        // cmd doesn't support login shells, so the flag should be ignored
        let args_no_login = shell.derive_exec_args("dir", false);
        let args_login = shell.derive_exec_args("dir", true);
        assert_eq!(args_no_login, args_login);
        assert_eq!(args_no_login, vec!["cmd.exe", "/c", "dir"]);
    }

    #[test]
    fn test_derive_exec_args_empty_command() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };

        let args = shell.derive_exec_args("", false);
        assert_eq!(args, vec!["/bin/bash", "-c", ""]);
    }

    #[test]
    fn test_derive_exec_args_special_characters() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };

        let args = shell.derive_exec_args("echo \"hello world\" && ls | grep foo", false);
        assert_eq!(
            args,
            vec!["/bin/bash", "-c", "echo \"hello world\" && ls | grep foo"]
        );
    }

    // detect_shell_type edge cases
    #[test]
    fn test_detect_shell_type_empty_path() {
        assert_eq!(detect_shell_type(&PathBuf::from("")), None);
    }

    #[test]
    fn test_detect_shell_type_with_extension() {
        // Windows-style paths with .exe extension
        assert_eq!(
            detect_shell_type(&PathBuf::from("bash.exe")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("zsh.exe")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("sh.exe")),
            Some(ShellType::Sh)
        );
    }

    #[test]
    fn test_detect_shell_type_deep_path() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("/usr/local/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/opt/homebrew/bin/zsh")),
            Some(ShellType::Zsh)
        );
    }

    #[test]
    fn test_detect_shell_type_case_sensitive() {
        // Shell detection is case-sensitive
        assert_eq!(detect_shell_type(&PathBuf::from("BASH")), None);
        assert_eq!(detect_shell_type(&PathBuf::from("ZSH")), None);
        assert_eq!(detect_shell_type(&PathBuf::from("Bash")), None);
    }

    // file_exists tests
    #[test]
    fn test_file_exists_nonexistent() {
        let result = file_exists(&PathBuf::from("/nonexistent/path/to/shell"));
        assert!(result.is_none());
    }

    #[test]
    fn test_file_exists_directory() {
        // Directories should not pass file_exists
        let result = file_exists(&PathBuf::from("/tmp"));
        assert!(result.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_file_exists_existing_file() {
        // /bin/sh should exist on Unix
        let result = file_exists(&PathBuf::from("/bin/sh"));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), PathBuf::from("/bin/sh"));
    }

    // default_user_shell_from_path tests
    #[test]
    fn test_default_user_shell_from_path_none() {
        let shell = default_user_shell_from_path(None);
        // Should fall back to platform default
        if cfg!(windows) {
            assert!(
                shell.shell_type == ShellType::PowerShell || shell.shell_type == ShellType::Cmd
            );
        } else {
            // On Unix, should be zsh, bash, or sh
            assert!(matches!(
                shell.shell_type,
                ShellType::Zsh | ShellType::Bash | ShellType::Sh
            ));
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_default_user_shell_from_path_bash() {
        let shell = default_user_shell_from_path(Some(PathBuf::from("/bin/bash")));
        // Should detect bash from path
        assert_eq!(shell.shell_type, ShellType::Bash);
    }

    #[test]
    #[cfg(unix)]
    fn test_default_user_shell_from_path_zsh() {
        let shell = default_user_shell_from_path(Some(PathBuf::from("/bin/zsh")));
        // Should detect zsh from path
        assert_eq!(shell.shell_type, ShellType::Zsh);
    }

    #[test]
    fn test_default_user_shell_from_path_unknown() {
        let shell = default_user_shell_from_path(Some(PathBuf::from("/bin/fish")));
        // fish is unknown, should fall back
        if cfg!(windows) {
            assert!(
                shell.shell_type == ShellType::PowerShell || shell.shell_type == ShellType::Cmd
            );
        } else {
            // Should fall back to platform default (zsh on macOS, bash on Linux, or sh)
            assert!(matches!(
                shell.shell_type,
                ShellType::Zsh | ShellType::Bash | ShellType::Sh
            ));
        }
    }

    // get_shell tests
    #[test]
    fn test_get_shell_with_explicit_path() {
        // When given an explicit path that exists, it should use it
        let path = PathBuf::from("/bin/sh");
        if path.exists() {
            let shell = get_shell(ShellType::Sh, Some(&path));
            assert!(shell.is_some());
            assert_eq!(shell.unwrap().shell_path, path);
        }
    }

    #[test]
    fn test_get_shell_nonexistent_explicit_path() {
        // When given a nonexistent explicit path, it should fall back
        let path = PathBuf::from("/nonexistent/shell");
        let shell = get_shell(ShellType::Sh, Some(&path));
        // Should either find sh elsewhere or return None
        if let Some(s) = shell {
            assert_ne!(s.shell_path, path);
        }
    }

    // get_shell_by_model_provided_path tests
    #[test]
    fn test_get_shell_by_model_provided_path_known_shell() {
        // Even with nonexistent path, should detect type and use fallback
        let shell = get_shell_by_model_provided_path(&PathBuf::from("/nonexistent/bash"));
        // Should fall back to ultimate fallback since path doesn't exist
        assert!(!shell.shell_path.as_os_str().is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn test_get_shell_by_model_provided_path_sh_exists() {
        let shell = get_shell_by_model_provided_path(&PathBuf::from("/bin/sh"));
        assert_eq!(shell.shell_type, ShellType::Sh);
        assert_eq!(shell.shell_path, PathBuf::from("/bin/sh"));
    }

    // Additional edge cases
    #[test]
    fn test_derive_exec_args_multiline_command() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };

        let args = shell.derive_exec_args("echo line1\necho line2", false);
        assert_eq!(args.len(), 3);
        assert!(args[2].contains('\n'));
    }

    #[test]
    fn test_shell_path_with_spaces() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/path with spaces/bash"),
        };

        let args = shell.derive_exec_args("ls", false);
        assert_eq!(args[0], "/path with spaces/bash");
    }

    #[test]
    fn test_derive_exec_args_unicode_command() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };

        let args = shell.derive_exec_args("echo 'こんにちは'", false);
        assert_eq!(args[2], "echo 'こんにちは'");
    }

    #[test]
    fn test_shell_serde_deserialize() {
        let json = r#"{"shell_type":"Bash","shell_path":"/bin/bash"}"#;
        let shell: Shell = serde_json::from_str(json).unwrap();
        assert_eq!(shell.shell_type, ShellType::Bash);
        assert_eq!(shell.shell_path, PathBuf::from("/bin/bash"));
    }

    #[test]
    fn test_shell_type_deserialize() {
        let json = "\"Zsh\"";
        let shell_type: ShellType = serde_json::from_str(json).unwrap();
        assert_eq!(shell_type, ShellType::Zsh);
    }
}
