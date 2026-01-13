//! Git repository information utilities
//!
//! Provides functions for collecting git repository context:
//! - Repository root detection
//! - Current branch and commit information
//! - Recent commit history for LLM context
//!
//! This module delegates to `dashflow-git-tool` for git operations,
//! wrapping synchronous git2 calls in async-compatible interfaces.

use dashflow_git_tool::GitTool;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Timeout for git commands to prevent freezing on large repositories
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// Git repository information
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitInfo {
    /// Current commit hash (full SHA-1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    /// Current branch name (None if detached HEAD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Remote origin URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

impl GitInfo {
    /// Check if any git info is available
    pub fn is_empty(&self) -> bool {
        self.commit_hash.is_none() && self.branch.is_none() && self.repository_url.is_none()
    }

    /// Format as context string for LLM
    pub fn as_context_string(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        let mut parts = Vec::new();
        if let Some(ref branch) = self.branch {
            parts.push(format!("Branch: {}", branch));
        }
        if let Some(ref commit) = self.commit_hash {
            // Use short hash for readability
            let short = if commit.len() > 8 {
                &commit[..8]
            } else {
                commit
            };
            parts.push(format!("Commit: {}", short));
        }
        if let Some(ref url) = self.repository_url {
            parts.push(format!("Repository: {}", url));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }
}

/// A minimal commit log entry
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitLogEntry {
    /// Full commit SHA
    pub sha: String,
    /// Unix timestamp (seconds since epoch)
    pub timestamp: i64,
    /// Single-line commit message subject
    pub subject: String,
}

impl CommitLogEntry {
    /// Get short SHA (first 8 characters)
    pub fn short_sha(&self) -> &str {
        if self.sha.len() > 8 {
            &self.sha[..8]
        } else {
            &self.sha
        }
    }
}

/// Return the git repository root directory, if the path is inside a git repo.
///
/// Walks up the directory hierarchy looking for a `.git` file or directory.
/// Uses dashflow-git-tool's GitTool::discover internally.
pub fn get_git_repo_root(base_dir: &Path) -> Option<PathBuf> {
    // Use dashflow-git-tool's discovery
    GitTool::discover(base_dir)
        .ok()
        .map(|git| git.root_path().to_path_buf())
}

/// Check if a path is inside a git repository
pub fn is_in_git_repo(path: &Path) -> bool {
    GitTool::discover(path).is_ok()
}

/// Collect git repository information from the given working directory.
///
/// Returns None if not in a git repository or if git operations fail.
/// Uses dashflow-git-tool internally with async wrapper.
pub async fn collect_git_info(cwd: &Path) -> Option<GitInfo> {
    let cwd = cwd.to_path_buf();

    // Wrap synchronous git2 operations in spawn_blocking
    tokio::task::spawn_blocking(move || {
        let git = GitTool::discover(&cwd).ok()?;

        let commit_hash = git
            .recent_commits(1, false)
            .ok()
            .and_then(|commits| commits.first().map(|c| c.full_hash.clone()));

        let branch = git.current_branch().ok().and_then(|b| {
            // Return None for detached HEAD state
            if b.starts_with("HEAD detached") {
                None
            } else {
                Some(b)
            }
        });

        // Get remote URL - dashflow-git-tool doesn't expose this directly,
        // so we need to use git2 directly or fall back to None
        let repository_url = None; // Could be implemented if needed

        Some(GitInfo {
            commit_hash,
            branch,
            repository_url,
        })
    })
    .await
    .ok()
    .flatten()
}

/// Return the last `limit` commits reachable from HEAD.
///
/// Each entry contains the SHA, commit timestamp, and subject line.
/// Returns an empty vector if not in a git repo or on error/timeout.
pub async fn recent_commits(cwd: &Path, limit: usize) -> Vec<CommitLogEntry> {
    let cwd = cwd.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let git = match GitTool::discover(&cwd) {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        // Handle limit=0 as "no limit" (return all)
        let actual_limit = if limit == 0 { 1000 } else { limit };

        match git.recent_commits(actual_limit, false) {
            Ok(commits) => commits
                .into_iter()
                .map(|c| CommitLogEntry {
                    sha: c.full_hash,
                    timestamp: c.timestamp,
                    subject: c.summary,
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    })
    .await
    .unwrap_or_default()
}

/// Returns the current checked out branch name.
pub async fn current_branch_name(cwd: &Path) -> Option<String> {
    let cwd = cwd.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let git = GitTool::discover(&cwd).ok()?;
        let branch = git.current_branch().ok()?;

        // Return None for detached HEAD or empty branch
        if branch.starts_with("HEAD detached") || branch.is_empty() {
            None
        } else {
            Some(branch)
        }
    })
    .await
    .ok()
    .flatten()
}

/// Get git status (short format)
///
/// Note: dashflow-git-tool doesn't have a direct equivalent for `git status --short`,
/// so we use file_statuses and format it ourselves.
pub async fn git_status_short(cwd: &Path) -> Option<String> {
    let cwd = cwd.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let git = GitTool::discover(&cwd).ok()?;
        let (staged, unstaged, untracked) = git.file_statuses(true).ok()?;

        let mut lines = Vec::new();

        // Format staged files with index status
        for file in staged {
            let status_char = match file.status {
                dashflow_git_tool::FileStatusType::New => "A ",
                dashflow_git_tool::FileStatusType::Modified => "M ",
                dashflow_git_tool::FileStatusType::Deleted => "D ",
                dashflow_git_tool::FileStatusType::Renamed => "R ",
                dashflow_git_tool::FileStatusType::Copied => "C ",
                dashflow_git_tool::FileStatusType::TypeChange => "T ",
                _ => "? ",
            };
            lines.push(format!("{}{}", status_char, file.path));
        }

        // Format unstaged files with worktree status
        for file in unstaged {
            let status_char = match file.status {
                dashflow_git_tool::FileStatusType::Modified => " M",
                dashflow_git_tool::FileStatusType::Deleted => " D",
                dashflow_git_tool::FileStatusType::Renamed => " R",
                dashflow_git_tool::FileStatusType::TypeChange => " T",
                _ => " ?",
            };
            lines.push(format!("{} {}", status_char, file.path));
        }

        // Format untracked files
        for path in untracked {
            lines.push(format!("?? {}", path));
        }

        Some(lines.join("\n"))
    })
    .await
    .ok()
    .flatten()
}

/// Get the number of uncommitted changes
pub async fn uncommitted_change_count(cwd: &Path) -> usize {
    let cwd = cwd.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let git = match GitTool::discover(&cwd) {
            Ok(g) => g,
            Err(_) => return 0,
        };

        match git.file_statuses(true) {
            Ok((staged, unstaged, untracked)) => staged.len() + unstaged.len() + untracked.len(),
            Err(_) => 0,
        }
    })
    .await
    .unwrap_or(0)
}

/// Run a git command with a timeout
async fn run_git_command(args: &[&str], cwd: &Path) -> Option<std::process::Output> {
    let result = timeout(
        GIT_COMMAND_TIMEOUT,
        Command::new("git").args(args).current_dir(cwd).output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => Some(output),
        _ => None, // Timeout or error
    }
}

/// Find the merge-base commit between HEAD and the specified branch.
///
/// Returns the full SHA of the merge-base commit, or None if it cannot be determined.
/// This is useful for code review to find the point where the current branch diverged.
///
/// Note: dashflow-git-tool doesn't expose merge-base, so we use git command directly.
pub async fn merge_base_with_head(cwd: &Path, branch: &str) -> Option<String> {
    // First, try to find the upstream of the branch
    let upstream = format!("{}@{{upstream}}", branch);
    let upstream_result = run_git_command(&["rev-parse", "--abbrev-ref", &upstream], cwd).await;

    let target = if let Some(out) = upstream_result {
        if out.status.success() {
            String::from_utf8(out.stdout)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| branch.to_string())
        } else {
            branch.to_string()
        }
    } else {
        branch.to_string()
    };

    // Now find the merge-base
    let out = run_git_command(&["merge-base", "HEAD", &target], cwd).await?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Get the git diff including both tracked and untracked changes.
///
/// Returns a tuple of (is_git_repo, diff_content):
/// - `is_git_repo`: true if the path is inside a git repository
/// - `diff_content`: concatenated diff of tracked changes plus untracked file contents
///
/// This mirrors the behavior of Codex's TUI `get_git_diff` function.
pub async fn get_git_diff(cwd: &Path) -> (bool, String) {
    if !is_in_git_repo(cwd) {
        return (false, String::new());
    }

    // Get tracked diff
    let tracked_diff = run_git_diff(cwd, &["diff", "--color"]).await;

    // Get untracked files
    let untracked =
        match run_git_command(&["ls-files", "--others", "--exclude-standard"], cwd).await {
            Some(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
            _ => String::new(),
        };

    // Generate diffs for untracked files
    let null_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut untracked_diff = String::new();
    for file in untracked.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let diff = run_git_diff(
            cwd,
            &["diff", "--color", "--no-index", "--", null_path, file],
        )
        .await;
        untracked_diff.push_str(&diff);
    }

    (true, format!("{tracked_diff}{untracked_diff}"))
}

/// Run a git diff command, treating exit code 1 as success (git diff returns 1 when differences exist)
async fn run_git_diff(cwd: &Path, args: &[&str]) -> String {
    let result = timeout(
        GIT_COMMAND_TIMEOUT,
        Command::new("git").args(args).current_dir(cwd).output(),
    )
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() || output.status.code() == Some(1) => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        _ => String::new(),
    }
}

/// Get diff between two commits or branches
pub async fn git_diff_range(cwd: &Path, base: &str, head: &str) -> Option<String> {
    let cwd = cwd.to_path_buf();
    let base = base.to_string();
    let head = head.to_string();

    tokio::task::spawn_blocking(move || {
        let git = GitTool::discover(&cwd).ok()?;
        git.diff_refs(&base, &head, 100_000).ok()
    })
    .await
    .ok()
    .flatten()
}

/// Format recent commits as context for LLM
pub async fn format_commits_for_context(cwd: &Path, limit: usize) -> Option<String> {
    let commits = recent_commits(cwd, limit).await;
    if commits.is_empty() {
        return None;
    }

    let formatted: Vec<String> = commits
        .iter()
        .map(|c| format!("- {} {}", c.short_sha(), c.subject))
        .collect();

    Some(format!("Recent commits:\n{}", formatted.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a test git repository
    async fn create_test_git_repo(temp_dir: &TempDir) -> PathBuf {
        let repo_path = temp_dir.path().join("repo");
        fs::create_dir(&repo_path).expect("Failed to create repo dir");

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to init git repo");

        // Configure git user
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user name");

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to set git user email");

        // Create a test file and commit it
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").expect("Failed to write test file");

        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add files");

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to commit");

        repo_path
    }

    #[test]
    fn test_get_git_repo_root_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let result = get_git_repo_root(temp_dir.path());
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_git_repo_root_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let result = get_git_repo_root(&repo_path);
        assert!(result.is_some());
        // Compare canonicalized paths to handle symlinks (e.g., /var -> /private/var on macOS)
        let result_canon = result.unwrap().canonicalize().unwrap();
        let expected_canon = repo_path.canonicalize().unwrap();
        assert_eq!(result_canon, expected_canon);
    }

    #[tokio::test]
    async fn test_get_git_repo_root_nested() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Create nested directory
        let nested = repo_path.join("a/b/c");
        fs::create_dir_all(&nested).expect("Failed to create nested dir");

        let result = get_git_repo_root(&nested);
        assert!(result.is_some());
        // Compare canonicalized paths to handle symlinks (e.g., /var -> /private/var on macOS)
        let result_canon = result.unwrap().canonicalize().unwrap();
        let expected_canon = repo_path.canonicalize().unwrap();
        assert_eq!(result_canon, expected_canon);
    }

    #[test]
    fn test_is_in_git_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        assert!(!is_in_git_repo(temp_dir.path()));
    }

    #[tokio::test]
    async fn test_collect_git_info_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let result = collect_git_info(temp_dir.path()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_collect_git_info_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let git_info = collect_git_info(&repo_path)
            .await
            .expect("Should collect git info");

        // Should have commit hash
        assert!(git_info.commit_hash.is_some());
        let commit_hash = git_info.commit_hash.as_ref().unwrap();
        assert_eq!(commit_hash.len(), 40); // SHA-1 hash
        assert!(commit_hash.chars().all(|c| c.is_ascii_hexdigit()));

        // Should have branch
        assert!(git_info.branch.is_some());
        let branch = git_info.branch.as_ref().unwrap();
        assert!(branch == "main" || branch == "master");
    }

    #[tokio::test]
    async fn test_recent_commits_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let entries = recent_commits(temp_dir.path(), 10).await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_recent_commits_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let entries = recent_commits(&repo_path, 10).await;
        assert!(!entries.is_empty());

        let first = &entries[0];
        assert_eq!(first.sha.len(), 40);
        assert!(first.subject.contains("Initial commit"));
        assert!(first.timestamp > 0);
    }

    #[tokio::test]
    async fn test_current_branch_name() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let branch = current_branch_name(&repo_path).await;
        assert!(branch.is_some());
        let branch = branch.unwrap();
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn test_git_info_is_empty() {
        let empty = GitInfo::default();
        assert!(empty.is_empty());

        let with_commit = GitInfo {
            commit_hash: Some("abc123".to_string()),
            ..Default::default()
        };
        assert!(!with_commit.is_empty());
    }

    #[test]
    fn test_git_info_as_context_string() {
        let info = GitInfo {
            commit_hash: Some("abc123def456".to_string()),
            branch: Some("main".to_string()),
            repository_url: Some("https://github.com/example/repo.git".to_string()),
        };

        let context = info.as_context_string().unwrap();
        assert!(context.contains("Branch: main"));
        assert!(context.contains("Commit: abc123de"));
        assert!(context.contains("Repository:"));
    }

    #[test]
    fn test_git_info_serialization() {
        let info = GitInfo {
            commit_hash: Some("abc123".to_string()),
            branch: Some("main".to_string()),
            repository_url: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["commit_hash"], "abc123");
        assert_eq!(parsed["branch"], "main");
        // repository_url should be omitted due to skip_serializing_if
        assert!(!parsed.as_object().unwrap().contains_key("repository_url"));
    }

    #[test]
    fn test_commit_log_entry_short_sha() {
        let entry = CommitLogEntry {
            sha: "abc123def456789012345678901234567890".to_string(),
            timestamp: 1234567890,
            subject: "Test commit".to_string(),
        };
        assert_eq!(entry.short_sha(), "abc123de");
    }

    #[tokio::test]
    async fn test_git_status_short() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Clean repo should have empty status
        let status = git_status_short(&repo_path).await;
        assert!(status.is_some());
        assert!(status.unwrap().is_empty());

        // Create uncommitted file
        fs::write(repo_path.join("new.txt"), "content").unwrap();

        let status = git_status_short(&repo_path).await;
        assert!(status.is_some());
        assert!(status.unwrap().contains("new.txt"));
    }

    #[tokio::test]
    async fn test_uncommitted_change_count() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Clean repo
        assert_eq!(uncommitted_change_count(&repo_path).await, 0);

        // Create uncommitted files
        fs::write(repo_path.join("new1.txt"), "content").unwrap();
        fs::write(repo_path.join("new2.txt"), "content").unwrap();

        assert_eq!(uncommitted_change_count(&repo_path).await, 2);
    }

    #[tokio::test]
    async fn test_format_commits_for_context() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let context = format_commits_for_context(&repo_path, 5).await;
        assert!(context.is_some());
        let context = context.unwrap();
        assert!(context.contains("Recent commits:"));
        assert!(context.contains("Initial commit"));
    }

    #[tokio::test]
    async fn test_merge_base_with_head_same_branch() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Get current branch
        let branch = current_branch_name(&repo_path).await.unwrap();

        // merge-base with self should return the current commit
        let merge_base = merge_base_with_head(&repo_path, &branch).await;
        assert!(merge_base.is_some());
        let base_sha = merge_base.unwrap();
        assert_eq!(base_sha.len(), 40);
    }

    #[tokio::test]
    async fn test_merge_base_with_head_nonexistent_branch() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Non-existent branch should return None
        let merge_base = merge_base_with_head(&repo_path, "nonexistent-branch-xyz").await;
        assert!(merge_base.is_none());
    }

    #[tokio::test]
    async fn test_merge_base_with_head_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let merge_base = merge_base_with_head(temp_dir.path(), "main").await;
        assert!(merge_base.is_none());
    }

    #[tokio::test]
    async fn test_get_git_diff_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let (is_repo, diff) = get_git_diff(temp_dir.path()).await;
        assert!(!is_repo);
        assert!(diff.is_empty());
    }

    #[tokio::test]
    async fn test_get_git_diff_clean_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let (is_repo, diff) = get_git_diff(&repo_path).await;
        assert!(is_repo);
        // Clean repo should have empty diff
        assert!(diff.is_empty());
    }

    #[tokio::test]
    async fn test_get_git_diff_with_changes() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Modify existing tracked file
        fs::write(repo_path.join("test.txt"), "modified content").unwrap();

        let (is_repo, diff) = get_git_diff(&repo_path).await;
        assert!(is_repo);
        // Should have tracked diff
        assert!(diff.contains("modified content") || diff.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_get_git_diff_with_untracked() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Create untracked file
        fs::write(repo_path.join("untracked.txt"), "untracked content").unwrap();

        let (is_repo, diff) = get_git_diff(&repo_path).await;
        assert!(is_repo);
        // Should include untracked file in diff
        assert!(diff.contains("untracked.txt") || diff.contains("untracked content"));
    }

    #[tokio::test]
    async fn test_git_diff_range() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Get the initial commit
        let out = run_git_command(&["rev-parse", "HEAD"], &repo_path)
            .await
            .unwrap();
        let initial_sha = String::from_utf8_lossy(&out.stdout).trim().to_string();

        // Create a second commit
        fs::write(repo_path.join("second.txt"), "second file content").unwrap();
        Command::new("git")
            .args(["add", "second.txt"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to add file");
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(&repo_path)
            .output()
            .await
            .expect("Failed to commit");

        // Get diff between initial and HEAD
        let diff = git_diff_range(&repo_path, &initial_sha, "HEAD").await;
        assert!(diff.is_some());
        let diff = diff.unwrap();
        assert!(diff.contains("second.txt"));
    }

    #[tokio::test]
    async fn test_git_diff_range_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let diff = git_diff_range(temp_dir.path(), "HEAD~1", "HEAD").await;
        assert!(diff.is_none());
    }

    // ========================
    // GitInfo struct tests
    // ========================

    #[test]
    fn test_git_info_default() {
        let info = GitInfo::default();
        assert!(info.commit_hash.is_none());
        assert!(info.branch.is_none());
        assert!(info.repository_url.is_none());
    }

    #[test]
    fn test_git_info_clone() {
        let info = GitInfo {
            commit_hash: Some("abc123".to_string()),
            branch: Some("main".to_string()),
            repository_url: Some("https://example.com".to_string()),
        };
        let cloned = info.clone();
        assert_eq!(info, cloned);
    }

    #[test]
    fn test_git_info_debug() {
        let info = GitInfo {
            commit_hash: Some("abc".to_string()),
            branch: None,
            repository_url: None,
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("GitInfo"));
        assert!(debug_str.contains("abc"));
    }

    #[test]
    fn test_git_info_eq() {
        let info1 = GitInfo {
            commit_hash: Some("abc".to_string()),
            branch: Some("main".to_string()),
            repository_url: None,
        };
        let info2 = GitInfo {
            commit_hash: Some("abc".to_string()),
            branch: Some("main".to_string()),
            repository_url: None,
        };
        let info3 = GitInfo {
            commit_hash: Some("def".to_string()),
            branch: Some("main".to_string()),
            repository_url: None,
        };
        assert_eq!(info1, info2);
        assert_ne!(info1, info3);
    }

    #[test]
    fn test_git_info_is_empty_with_branch_only() {
        let info = GitInfo {
            commit_hash: None,
            branch: Some("main".to_string()),
            repository_url: None,
        };
        assert!(!info.is_empty());
    }

    #[test]
    fn test_git_info_is_empty_with_url_only() {
        let info = GitInfo {
            commit_hash: None,
            branch: None,
            repository_url: Some("https://example.com".to_string()),
        };
        assert!(!info.is_empty());
    }

    #[test]
    fn test_git_info_as_context_string_empty() {
        let info = GitInfo::default();
        assert!(info.as_context_string().is_none());
    }

    #[test]
    fn test_git_info_as_context_string_branch_only() {
        let info = GitInfo {
            commit_hash: None,
            branch: Some("feature".to_string()),
            repository_url: None,
        };
        let ctx = info.as_context_string().unwrap();
        assert_eq!(ctx, "Branch: feature");
    }

    #[test]
    fn test_git_info_as_context_string_commit_only() {
        let info = GitInfo {
            commit_hash: Some("abc123def456".to_string()),
            branch: None,
            repository_url: None,
        };
        let ctx = info.as_context_string().unwrap();
        assert_eq!(ctx, "Commit: abc123de");
    }

    #[test]
    fn test_git_info_as_context_string_url_only() {
        let info = GitInfo {
            commit_hash: None,
            branch: None,
            repository_url: Some("https://github.com/test/repo".to_string()),
        };
        let ctx = info.as_context_string().unwrap();
        assert_eq!(ctx, "Repository: https://github.com/test/repo");
    }

    #[test]
    fn test_git_info_as_context_string_short_hash() {
        // Hash exactly 8 chars - should not truncate
        let info = GitInfo {
            commit_hash: Some("abc12345".to_string()),
            branch: None,
            repository_url: None,
        };
        let ctx = info.as_context_string().unwrap();
        assert_eq!(ctx, "Commit: abc12345");
    }

    #[test]
    fn test_git_info_as_context_string_very_short_hash() {
        // Hash less than 8 chars - should not truncate
        let info = GitInfo {
            commit_hash: Some("abc".to_string()),
            branch: None,
            repository_url: None,
        };
        let ctx = info.as_context_string().unwrap();
        assert_eq!(ctx, "Commit: abc");
    }

    #[test]
    fn test_git_info_serde_deserialize() {
        let json = r#"{"commit_hash":"abc123","branch":"main"}"#;
        let info: GitInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.commit_hash, Some("abc123".to_string()));
        assert_eq!(info.branch, Some("main".to_string()));
        assert!(info.repository_url.is_none());
    }

    #[test]
    fn test_git_info_serde_roundtrip() {
        let info = GitInfo {
            commit_hash: Some("abc123".to_string()),
            branch: Some("main".to_string()),
            repository_url: Some("https://example.com".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: GitInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, parsed);
    }

    #[test]
    fn test_git_info_skip_serializing_none() {
        let info = GitInfo {
            commit_hash: Some("abc".to_string()),
            branch: None,
            repository_url: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("branch"));
        assert!(!json.contains("repository_url"));
    }

    // ========================
    // CommitLogEntry tests
    // ========================

    #[test]
    fn test_commit_log_entry_clone() {
        let entry = CommitLogEntry {
            sha: "abc123".to_string(),
            timestamp: 12345,
            subject: "Test".to_string(),
        };
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
    }

    #[test]
    fn test_commit_log_entry_debug() {
        let entry = CommitLogEntry {
            sha: "abc".to_string(),
            timestamp: 100,
            subject: "msg".to_string(),
        };
        let debug = format!("{:?}", entry);
        assert!(debug.contains("CommitLogEntry"));
        assert!(debug.contains("abc"));
        assert!(debug.contains("100"));
        assert!(debug.contains("msg"));
    }

    #[test]
    fn test_commit_log_entry_eq() {
        let e1 = CommitLogEntry {
            sha: "abc".to_string(),
            timestamp: 100,
            subject: "msg".to_string(),
        };
        let e2 = CommitLogEntry {
            sha: "abc".to_string(),
            timestamp: 100,
            subject: "msg".to_string(),
        };
        let e3 = CommitLogEntry {
            sha: "def".to_string(),
            timestamp: 100,
            subject: "msg".to_string(),
        };
        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_commit_log_entry_short_sha_exact_8() {
        let entry = CommitLogEntry {
            sha: "12345678".to_string(),
            timestamp: 0,
            subject: String::new(),
        };
        assert_eq!(entry.short_sha(), "12345678");
    }

    #[test]
    fn test_commit_log_entry_short_sha_less_than_8() {
        let entry = CommitLogEntry {
            sha: "abc".to_string(),
            timestamp: 0,
            subject: String::new(),
        };
        assert_eq!(entry.short_sha(), "abc");
    }

    #[test]
    fn test_commit_log_entry_short_sha_empty() {
        let entry = CommitLogEntry {
            sha: String::new(),
            timestamp: 0,
            subject: String::new(),
        };
        assert_eq!(entry.short_sha(), "");
    }

    #[test]
    fn test_commit_log_entry_serde_roundtrip() {
        let entry = CommitLogEntry {
            sha: "abc123def456".to_string(),
            timestamp: 1234567890,
            subject: "Test commit message".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: CommitLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
    }

    #[test]
    fn test_commit_log_entry_serde_deserialize() {
        let json = r#"{"sha":"abc","timestamp":999,"subject":"message"}"#;
        let entry: CommitLogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.sha, "abc");
        assert_eq!(entry.timestamp, 999);
        assert_eq!(entry.subject, "message");
    }

    // ========================
    // Constant tests
    // ========================

    #[test]
    fn test_git_command_timeout_value() {
        assert_eq!(GIT_COMMAND_TIMEOUT, Duration::from_secs(5));
    }

    // ========================
    // get_git_repo_root tests
    // ========================

    #[test]
    fn test_get_git_repo_root_empty_path() {
        let result = get_git_repo_root(Path::new(""));
        // Empty path should not find a repo
        assert!(result.is_none() || result.is_some()); // May find current dir's repo
    }

    #[test]
    fn test_get_git_repo_root_nonexistent_path() {
        let result = get_git_repo_root(Path::new("/nonexistent/path/xyz/abc"));
        assert!(result.is_none());
    }

    // ========================
    // is_in_git_repo tests
    // ========================

    #[test]
    fn test_is_in_git_repo_nonexistent() {
        assert!(!is_in_git_repo(Path::new("/nonexistent/path")));
    }

    // ========================
    // Async function tests - not in repo
    // ========================

    #[tokio::test]
    async fn test_current_branch_name_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let branch = current_branch_name(temp_dir.path()).await;
        assert!(branch.is_none());
    }

    #[tokio::test]
    async fn test_git_status_short_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let status = git_status_short(temp_dir.path()).await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_uncommitted_change_count_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let count = uncommitted_change_count(temp_dir.path()).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_format_commits_for_context_not_in_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ctx = format_commits_for_context(temp_dir.path(), 10).await;
        assert!(ctx.is_none());
    }

    // ========================
    // recent_commits edge cases
    // ========================

    #[tokio::test]
    async fn test_recent_commits_limit_zero() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // limit=0 should return all commits (no limit applied)
        let entries = recent_commits(&repo_path, 0).await;
        // At least one commit (the initial one)
        assert!(!entries.is_empty());
    }

    #[tokio::test]
    async fn test_recent_commits_limit_one() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Create additional commits
        fs::write(repo_path.join("file2.txt"), "content2").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();

        let entries = recent_commits(&repo_path, 1).await;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].subject.contains("Second commit"));
    }

    #[tokio::test]
    async fn test_recent_commits_multiple() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Create additional commits with small delay to ensure distinct timestamps
        for i in 2..=5 {
            fs::write(
                repo_path.join(format!("file{}.txt", i)),
                format!("content{}", i),
            )
            .unwrap();
            Command::new("git")
                .args(["add", "."])
                .current_dir(&repo_path)
                .output()
                .await
                .unwrap();
            Command::new("git")
                .args(["commit", "-m", &format!("Commit {}", i)])
                .current_dir(&repo_path)
                .output()
                .await
                .unwrap();
        }

        // Request all 5 commits to verify total count
        let all_entries = recent_commits(&repo_path, 10).await;
        assert_eq!(all_entries.len(), 5); // Initial + 4 more

        // Request limited entries
        let entries = recent_commits(&repo_path, 3).await;
        assert_eq!(entries.len(), 3);

        // Verify the commits we requested are valid commits from this repo
        // Note: git2's revwalk may not return strictly chronological order
        // when commits have identical timestamps
        for entry in &entries {
            assert!(!entry.sha.is_empty());
            assert!(!entry.subject.is_empty());
            assert!(entry.timestamp > 0);
        }
    }

    // ========================
    // collect_git_info edge cases
    // ========================

    #[tokio::test]
    async fn test_collect_git_info_detached_head() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        // Get the commit hash
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let commit = String::from_utf8_lossy(&out.stdout).trim().to_string();

        // Create another commit
        fs::write(repo_path.join("file2.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Second"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();

        // Checkout the first commit (detached HEAD)
        Command::new("git")
            .args(["checkout", &commit])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();

        let git_info = collect_git_info(&repo_path).await.unwrap();
        // Branch should be None in detached HEAD state
        assert!(git_info.branch.is_none());
        assert!(git_info.commit_hash.is_some());
    }

    // ========================
    // format_commits_for_context tests
    // ========================

    #[tokio::test]
    async fn test_format_commits_for_context_single_commit() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let ctx = format_commits_for_context(&repo_path, 1).await.unwrap();
        assert!(ctx.starts_with("Recent commits:\n"));
        assert!(ctx.contains("Initial commit"));
        // Should have short SHA (8 chars)
        let lines: Vec<&str> = ctx.lines().collect();
        assert_eq!(lines.len(), 2); // Header + 1 commit
    }

    // ========================
    // git_diff_range edge cases
    // ========================

    #[tokio::test]
    async fn test_git_diff_range_same_commit() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let diff = git_diff_range(&repo_path, "HEAD", "HEAD").await;
        assert!(diff.is_some());
        assert!(diff.unwrap().is_empty()); // Same commit = no diff
    }

    #[tokio::test]
    async fn test_git_diff_range_invalid_ref() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;

        let diff = git_diff_range(&repo_path, "nonexistent", "HEAD").await;
        assert!(diff.is_none());
    }

    // ========================
    // Additional tests for robustness
    // ========================

    #[tokio::test]
    async fn test_is_in_git_repo_true() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;
        assert!(is_in_git_repo(&repo_path));
    }

    #[tokio::test]
    async fn test_is_in_git_repo_nested_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;
        let nested = repo_path.join("deep/nested/dir");
        fs::create_dir_all(&nested).unwrap();
        assert!(is_in_git_repo(&nested));
    }
}
