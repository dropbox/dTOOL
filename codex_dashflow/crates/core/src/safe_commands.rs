//! Known safe command detection.
//!
//! This module complements `safety.rs` (dangerous command detection) by providing
//! a whitelist of known-safe commands that can be auto-approved without user
//! confirmation.
//!
//! Based on codex-rs/core/src/command_safety/is_safe_command.rs

use std::path::Path;

use crate::bash::parse_shell_lc_plain_commands;

/// Check if a command is known to be safe and can be auto-approved.
///
/// This function returns true only for commands that are provably safe:
/// - Read-only operations that don't modify state
/// - Commands with no side effects
/// - Shell scripts composed entirely of safe commands
///
/// Returns false for anything unknown, which defers to normal approval flow.
pub fn is_known_safe_command(command: &[String]) -> bool {
    // Normalize zsh to bash for checking purposes
    let command: Vec<String> = command
        .iter()
        .map(|s| {
            if s == "zsh" {
                "bash".to_string()
            } else {
                s.clone()
            }
        })
        .collect();

    // Check if command itself is safe
    if is_safe_to_call_with_exec(&command) {
        return true;
    }

    // Support `bash -lc "..."` where the script consists solely of one or
    // more "plain" commands (only bare words / quoted strings) combined with
    // safe shell operators (&&, ||, ;, |). If every individual command in
    // the script is itself a known-safe command, then the composite
    // expression is considered safe.
    if let Some(all_commands) = parse_shell_lc_plain_commands(&command) {
        if !all_commands.is_empty()
            && all_commands
                .iter()
                .all(|cmd| is_safe_to_call_with_exec(cmd))
        {
            return true;
        }
    }

    false
}

/// Check if a direct command (not via shell) is safe to execute.
fn is_safe_to_call_with_exec(command: &[String]) -> bool {
    let Some(cmd0) = command.first().map(String::as_str) else {
        return false;
    };

    // Extract just the command name (not the full path)
    match Path::new(cmd0).file_name().and_then(|osstr| osstr.to_str()) {
        // Basic read-only commands
        Some(
            "cat" | "cd" | "echo" | "false" | "grep" | "head" | "ls" | "nl" | "pwd" | "tail"
            | "true" | "wc" | "which",
        ) => true,

        // find is safe ONLY without dangerous options
        Some("find") => is_safe_find_command(command),

        // ripgrep is safe ONLY without dangerous options
        Some("rg") => is_safe_ripgrep_command(command),

        // Git read-only commands only
        Some("git") => matches!(
            command.get(1).map(String::as_str),
            Some("branch" | "status" | "log" | "diff" | "show")
        ),

        // Rust cargo check only (no side effects)
        Some("cargo") if command.get(1).map(String::as_str) == Some("check") => true,

        // Special-case `sed -n {N|M,N}p` (read-only print commands)
        Some("sed")
            if command.len() <= 4
                && command.get(1).map(String::as_str) == Some("-n")
                && is_valid_sed_n_arg(command.get(2).map(String::as_str)) =>
        {
            true
        }

        // File type identification
        Some("file") => true,

        // stat and related
        Some("stat") => true,

        // env without modifications
        Some("env") if !command.iter().any(|arg| arg.contains('=')) => true,

        // df and du (disk usage info)
        Some("df" | "du") => true,

        // date and uptime (system info)
        Some("date" | "uptime") => true,

        // hostname
        Some("hostname") => true,

        // whoami and id
        Some("whoami" | "id") => true,

        // Anything else is not known to be safe
        _ => false,
    }
}

/// Check if a find command is safe (no dangerous options).
fn is_safe_find_command(command: &[String]) -> bool {
    // Options that can execute arbitrary commands or delete files
    const UNSAFE_FIND_OPTIONS: &[&str] = &[
        // Options that execute commands
        "-exec", "-execdir", "-ok", "-okdir",  // Option that deletes files
        "-delete", // Options that write to files
        "-fls", "-fprint", "-fprint0", "-fprintf",
    ];

    !command
        .iter()
        .any(|arg| UNSAFE_FIND_OPTIONS.contains(&arg.as_str()))
}

/// Check if a ripgrep command is safe (no dangerous options).
fn is_safe_ripgrep_command(command: &[String]) -> bool {
    // Options that take an argument and can execute external commands
    const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &[
        // Takes an arbitrary command executed for each match
        "--pre",
        // Takes a command to obtain hostname
        "--hostname-bin",
    ];

    // Options without arguments that are unsafe
    const UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS: &[&str] = &[
        // Calls decompression tools
        "--search-zip",
        "-z",
    ];

    !command.iter().any(|arg| {
        UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS.contains(&arg.as_str())
            || UNSAFE_RIPGREP_OPTIONS_WITH_ARGS
                .iter()
                .any(|&opt| arg == opt || arg.starts_with(&format!("{opt}=")))
    })
}

/// Returns true if `arg` matches /^(\d+,)?\d+p$/ for sed -n Np or M,Np
fn is_valid_sed_n_arg(arg: Option<&str>) -> bool {
    let s = match arg {
        Some(s) => s,
        None => return false,
    };

    // Must end with 'p', strip it
    let core = match s.strip_suffix('p') {
        Some(rest) => rest,
        None => return false,
    };

    // Split on ',' and ensure 1 or 2 numeric parts
    let parts: Vec<&str> = core.split(',').collect();
    match parts.as_slice() {
        // Single number, e.g. "10p"
        [num] => !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()),

        // Two numbers, e.g. "1,5p"
        [a, b] => {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }

        // Anything else is invalid
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn test_known_safe_basic_commands() {
        assert!(is_known_safe_command(&vec_str(&["ls"])));
        assert!(is_known_safe_command(&vec_str(&["ls", "-la"])));
        assert!(is_known_safe_command(&vec_str(&["pwd"])));
        assert!(is_known_safe_command(&vec_str(&["cat", "file.txt"])));
        assert!(is_known_safe_command(&vec_str(&["echo", "hello"])));
        assert!(is_known_safe_command(&vec_str(&[
            "grep", "pattern", "file"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "head", "-n", "10", "file"
        ])));
        assert!(is_known_safe_command(&vec_str(&["tail", "-f", "log"])));
        assert!(is_known_safe_command(&vec_str(&["wc", "-l", "file"])));
        assert!(is_known_safe_command(&vec_str(&["which", "bash"])));
    }

    #[test]
    fn test_git_safe_commands() {
        assert!(is_known_safe_command(&vec_str(&["git", "status"])));
        assert!(is_known_safe_command(&vec_str(&["git", "log"])));
        assert!(is_known_safe_command(&vec_str(&["git", "diff"])));
        assert!(is_known_safe_command(&vec_str(&["git", "branch"])));
        assert!(is_known_safe_command(&vec_str(&["git", "show"])));
    }

    #[test]
    fn test_git_unsafe_commands() {
        assert!(!is_known_safe_command(&vec_str(&["git", "reset"])));
        assert!(!is_known_safe_command(&vec_str(&["git", "push"])));
        assert!(!is_known_safe_command(&vec_str(&["git", "commit"])));
        assert!(!is_known_safe_command(&vec_str(&["git", "checkout"])));
        assert!(!is_known_safe_command(&vec_str(&["git", "rm"])));
    }

    #[test]
    fn test_cargo_check_safe() {
        assert!(is_known_safe_command(&vec_str(&["cargo", "check"])));
        assert!(!is_known_safe_command(&vec_str(&["cargo", "build"])));
        assert!(!is_known_safe_command(&vec_str(&["cargo", "run"])));
    }

    #[test]
    fn test_sed_n_print_safe() {
        assert!(is_known_safe_command(&vec_str(&["sed", "-n", "1p"])));
        assert!(is_known_safe_command(&vec_str(&["sed", "-n", "1,5p"])));
        assert!(is_known_safe_command(&vec_str(&[
            "sed", "-n", "1,5p", "file.txt"
        ])));
        // Unsafe sed commands
        assert!(!is_known_safe_command(&vec_str(&["sed", "-i", "s/a/b/g"])));
        assert!(!is_known_safe_command(&vec_str(&["sed", "-n", "xp"])));
    }

    #[test]
    fn test_find_safe_commands() {
        assert!(is_known_safe_command(&vec_str(&[
            "find", ".", "-name", "*.rs"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "find", "/tmp", "-type", "f"
        ])));
    }

    #[test]
    fn test_find_unsafe_commands() {
        assert!(!is_known_safe_command(&vec_str(&[
            "find", ".", "-exec", "rm", "{}", ";"
        ])));
        assert!(!is_known_safe_command(&vec_str(&["find", ".", "-delete"])));
        assert!(!is_known_safe_command(&vec_str(&[
            "find", ".", "-execdir", "echo", "{}", ";"
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "find", ".", "-fprint", "/tmp/out"
        ])));
    }

    #[test]
    fn test_ripgrep_safe() {
        assert!(is_known_safe_command(&vec_str(&["rg", "pattern"])));
        assert!(is_known_safe_command(&vec_str(&["rg", "-n", "pattern"])));
        assert!(is_known_safe_command(&vec_str(&[
            "rg",
            "--color=always",
            "pattern"
        ])));
    }

    #[test]
    fn test_ripgrep_unsafe() {
        assert!(!is_known_safe_command(&vec_str(&[
            "rg", "--pre", "pwned", "pattern"
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "rg",
            "--pre=pwned",
            "pattern"
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "rg",
            "--hostname-bin",
            "evil",
            "pattern"
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "rg",
            "--search-zip",
            "pattern"
        ])));
        assert!(!is_known_safe_command(&vec_str(&["rg", "-z", "pattern"])));
    }

    #[test]
    fn test_bash_lc_safe_commands() {
        assert!(is_known_safe_command(&vec_str(&["bash", "-lc", "ls"])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git status"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls && pwd"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "grep pattern file || true"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls | wc -l"
        ])));
    }

    #[test]
    fn test_bash_lc_unsafe_commands() {
        // Unsafe: contains rm
        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls && rm file"
        ])));
        // Unsafe: redirection
        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls > out.txt"
        ])));
        // Unsafe: subshell
        assert!(!is_known_safe_command(&vec_str(&["bash", "-lc", "(ls)"])));
    }

    #[test]
    fn test_zsh_normalized_to_bash() {
        assert!(is_known_safe_command(&vec_str(&["zsh", "-lc", "ls"])));
        assert!(is_known_safe_command(&vec_str(&[
            "zsh",
            "-lc",
            "git status"
        ])));
    }

    #[test]
    fn test_full_path_commands() {
        assert!(is_known_safe_command(&vec_str(&["/bin/ls"])));
        assert!(is_known_safe_command(&vec_str(&["/usr/bin/git", "status"])));
        assert!(is_known_safe_command(&vec_str(&["/usr/bin/cat", "file"])));
    }

    #[test]
    fn test_unknown_commands_not_safe() {
        assert!(!is_known_safe_command(&vec_str(&["rm", "file"])));
        assert!(!is_known_safe_command(&vec_str(&["wget", "url"])));
        assert!(!is_known_safe_command(&vec_str(&["curl", "url"])));
        assert!(!is_known_safe_command(&vec_str(&["sudo", "ls"])));
        assert!(!is_known_safe_command(&vec_str(&["chmod", "755", "file"])));
    }

    #[test]
    fn test_system_info_commands() {
        assert!(is_known_safe_command(&vec_str(&["whoami"])));
        assert!(is_known_safe_command(&vec_str(&["id"])));
        assert!(is_known_safe_command(&vec_str(&["hostname"])));
        assert!(is_known_safe_command(&vec_str(&["date"])));
        assert!(is_known_safe_command(&vec_str(&["uptime"])));
        assert!(is_known_safe_command(&vec_str(&["df", "-h"])));
        assert!(is_known_safe_command(&vec_str(&["du", "-sh", "."])));
    }

    #[test]
    fn test_file_stat_commands() {
        assert!(is_known_safe_command(&vec_str(&["file", "foo.txt"])));
        assert!(is_known_safe_command(&vec_str(&["stat", "foo.txt"])));
    }

    #[test]
    fn test_env_safe_only_when_no_assignment() {
        assert!(is_known_safe_command(&vec_str(&["env"])));
        assert!(!is_known_safe_command(&vec_str(&["env", "FOO=bar", "cmd"])));
    }

    #[test]
    fn test_valid_sed_n_arg() {
        assert!(is_valid_sed_n_arg(Some("1p")));
        assert!(is_valid_sed_n_arg(Some("10p")));
        assert!(is_valid_sed_n_arg(Some("1,5p")));
        assert!(is_valid_sed_n_arg(Some("100,200p")));

        assert!(!is_valid_sed_n_arg(Some("xp")));
        assert!(!is_valid_sed_n_arg(Some("1,2,3p")));
        assert!(!is_valid_sed_n_arg(Some("1")));
        assert!(!is_valid_sed_n_arg(Some("")));
        assert!(!is_valid_sed_n_arg(None));
    }
}
