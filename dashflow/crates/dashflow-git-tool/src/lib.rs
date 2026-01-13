// No broad clippy allows - production code uses proper error handling

//! # DashFlow Git Tool
//!
//! Git integration for coding agents. Provides repository detection, commit history,
//! diff generation, and context collection for LLM prompts.
//!
//! ## Features
//!
//! - **Repository detection**: Find .git root from any subdirectory
//! - **Commit history**: Recent commits with filtering
//! - **Changes**: Staged, unstaged, and untracked files
//! - **Diffs**: Between refs, working tree, staged changes
//! - **Branch info**: Current branch, remote tracking
//! - **LLM context**: Format git state for AI consumption
//!
//! ## Example
//!
//! ```no_run
//! use dashflow_git_tool::{GitTool, GitContextOptions};
//!
//! # tokio_test::block_on(async {
//! let git = GitTool::discover(".").unwrap();
//!
//! // Get current branch
//! let branch = git.current_branch().unwrap();
//! println!("On branch: {}", branch);
//!
//! // Collect context for LLM
//! let context = git.collect_context(GitContextOptions::default()).unwrap();
//! println!("{}", context.to_prompt_string());
//! # });
//! ```

use async_trait::async_trait;
use dashflow::core::{
    tools::{Tool, ToolInput},
    Error,
};
use git2::{BranchType, DiffOptions, Repository, StatusOptions};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during git operations
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GitError {
    /// Repository not found
    #[error("Git repository not found: {0}")]
    NotFound(String),

    /// Git operation failed
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    /// Invalid reference
    #[error("Invalid reference: {0}")]
    InvalidRef(String),

    /// Path error
    #[error("Path error: {0}")]
    PathError(String),
}

/// Options for collecting git context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitContextOptions {
    /// Maximum number of recent commits to include
    #[serde(default = "default_max_commits")]
    pub max_commits: usize,

    /// Include diff of uncommitted changes
    #[serde(default = "default_true")]
    pub include_diff: bool,

    /// Include untracked files
    #[serde(default = "default_true")]
    pub include_untracked: bool,

    /// Maximum diff size in bytes
    #[serde(default = "default_max_diff_size")]
    pub max_diff_size: usize,

    /// Include file stats in commits
    #[serde(default)]
    pub include_stats: bool,
}

fn default_max_commits() -> usize {
    10
}
fn default_true() -> bool {
    true
}
fn default_max_diff_size() -> usize {
    50_000
}

impl Default for GitContextOptions {
    fn default() -> Self {
        Self {
            max_commits: 10,
            include_diff: true,
            include_untracked: true,
            max_diff_size: 50_000,
            include_stats: false,
        }
    }
}

/// Information about a commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    /// Commit hash (short)
    pub hash: String,
    /// Full commit hash
    pub full_hash: String,
    /// Commit message (first line)
    pub summary: String,
    /// Full commit message
    pub message: String,
    /// Author name
    pub author: String,
    /// Author email
    pub author_email: String,
    /// Commit timestamp (Unix epoch)
    pub timestamp: i64,
    /// Files changed (if stats enabled)
    pub files_changed: Option<usize>,
    /// Lines added (if stats enabled)
    pub insertions: Option<usize>,
    /// Lines deleted (if stats enabled)
    pub deletions: Option<usize>,
}

/// Information about file status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    /// File path
    pub path: String,
    /// Status type
    pub status: FileStatusType,
}

/// Type of file status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileStatusType {
    /// New file (untracked or staged new)
    New,
    /// Modified
    Modified,
    /// Deleted
    Deleted,
    /// Renamed
    Renamed,
    /// Copied
    Copied,
    /// Type changed
    TypeChange,
    /// Untracked
    Untracked,
    /// Ignored
    Ignored,
    /// Conflicted
    Conflicted,
}

/// Statistics for a single commit: (files_changed, insertions, deletions)
pub type CommitStats = (Option<usize>, Option<usize>, Option<usize>);

/// File status grouped by stage: (staged, unstaged, untracked)
pub type FileStatusGroup = (Vec<FileStatus>, Vec<FileStatus>, Vec<String>);

/// Collected git context for LLM consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitContext {
    /// Repository root path
    pub repo_root: String,
    /// Current branch name
    pub branch: String,
    /// Remote tracking branch (if any)
    pub upstream: Option<String>,
    /// Whether there are uncommitted changes
    pub is_dirty: bool,
    /// Recent commits
    pub commits: Vec<CommitInfo>,
    /// Staged files
    pub staged: Vec<FileStatus>,
    /// Unstaged modified files
    pub unstaged: Vec<FileStatus>,
    /// Untracked files
    pub untracked: Vec<String>,
    /// Diff of uncommitted changes
    pub diff: Option<String>,
    /// Commit count ahead of upstream
    pub ahead: usize,
    /// Commit count behind upstream
    pub behind: usize,
}

impl GitContext {
    /// Format context as a string for LLM prompts
    #[must_use]
    pub fn to_prompt_string(&self) -> String {
        let mut output = String::new();

        // Branch info
        output.push_str(&format!("Branch: {}", self.branch));
        if let Some(ref upstream) = self.upstream {
            output.push_str(&format!(" (tracking {})", upstream));
        }
        output.push('\n');

        // Ahead/behind
        if self.ahead > 0 || self.behind > 0 {
            output.push_str(&format!("Ahead: {}, Behind: {}\n", self.ahead, self.behind));
        }

        // Status
        if self.is_dirty {
            output.push_str("\nUncommitted changes:\n");

            if !self.staged.is_empty() {
                output.push_str("  Staged:\n");
                for file in &self.staged {
                    output.push_str(&format!("    {:?}: {}\n", file.status, file.path));
                }
            }

            if !self.unstaged.is_empty() {
                output.push_str("  Modified:\n");
                for file in &self.unstaged {
                    output.push_str(&format!("    {:?}: {}\n", file.status, file.path));
                }
            }

            if !self.untracked.is_empty() {
                output.push_str("  Untracked:\n");
                for file in &self.untracked {
                    output.push_str(&format!("    {}\n", file));
                }
            }
        }

        // Recent commits
        if !self.commits.is_empty() {
            output.push_str("\nRecent commits:\n");
            for commit in &self.commits {
                output.push_str(&format!("  {} {}\n", commit.hash, commit.summary));
            }
        }

        // Diff
        if let Some(ref diff) = self.diff {
            if !diff.is_empty() {
                output.push_str("\nDiff:\n```diff\n");
                output.push_str(diff);
                if !diff.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str("```\n");
            }
        }

        output
    }

    /// Convert to JSON for structured output
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

/// Git tool for repository operations
pub struct GitTool {
    repo: Repository,
    repo_path: PathBuf,
}

impl std::fmt::Debug for GitTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitTool")
            .field("repo_path", &self.repo_path)
            .finish()
    }
}

impl GitTool {
    /// Discover a git repository from the given path
    ///
    /// Searches upward from the given path to find a .git directory.
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, GitError> {
        let path = path.as_ref();
        let repo = Repository::discover(path)
            .map_err(|e| GitError::NotFound(format!("{}: {}", path.display(), e)))?;
        let repo_path = repo.workdir().unwrap_or_else(|| repo.path()).to_path_buf();

        Ok(Self { repo, repo_path })
    }

    /// Open a git repository at the exact path
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GitError> {
        let path = path.as_ref();
        let repo = Repository::open(path)
            .map_err(|e| GitError::NotFound(format!("{}: {}", path.display(), e)))?;
        let repo_path = repo.workdir().unwrap_or_else(|| repo.path()).to_path_buf();

        Ok(Self { repo, repo_path })
    }

    /// Get the repository root path
    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.repo_path
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> Result<String, GitError> {
        let head = self.repo.head()?;
        if head.is_branch() {
            Ok(head.shorthand().unwrap_or("HEAD").to_string())
        } else {
            // Detached HEAD - return short commit hash
            let commit = head.peel_to_commit()?;
            Ok(format!(
                "HEAD detached at {}",
                &commit.id().to_string()[..7]
            ))
        }
    }

    /// Get the upstream branch name if set
    pub fn upstream_branch(&self) -> Result<Option<String>, GitError> {
        let head = self.repo.head()?;
        if !head.is_branch() {
            return Ok(None);
        }

        let branch_name = head
            .shorthand()
            .ok_or_else(|| GitError::InvalidRef("Could not get branch name".to_string()))?;

        let branch = self.repo.find_branch(branch_name, BranchType::Local)?;
        match branch.upstream() {
            Ok(upstream) => Ok(upstream.name()?.map(String::from)),
            Err(_) => Ok(None),
        }
    }

    /// Check if repository has uncommitted changes
    pub fn is_dirty(&self) -> Result<bool, GitError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        Ok(!statuses.is_empty())
    }

    /// Get recent commits
    pub fn recent_commits(
        &self,
        max_count: usize,
        include_stats: bool,
    ) -> Result<Vec<CommitInfo>, GitError> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();

        for (i, oid) in revwalk.enumerate() {
            if i >= max_count {
                break;
            }

            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;

            let hash = oid.to_string();
            let short_hash = hash[..7.min(hash.len())].to_string();

            let message = commit.message().unwrap_or("").to_string();
            let summary = commit.summary().unwrap_or("").to_string();

            let author = commit.author();
            let author_name = author.name().unwrap_or("Unknown").to_string();
            let author_email = author.email().unwrap_or("").to_string();

            let timestamp = commit.time().seconds();

            let (files_changed, insertions, deletions) = if include_stats {
                self.commit_stats(&commit).unwrap_or((None, None, None))
            } else {
                (None, None, None)
            };

            commits.push(CommitInfo {
                hash: short_hash,
                full_hash: hash,
                summary,
                message,
                author: author_name,
                author_email,
                timestamp,
                files_changed,
                insertions,
                deletions,
            });
        }

        Ok(commits)
    }

    /// Get stats for a commit
    fn commit_stats(&self, commit: &git2::Commit) -> Result<CommitStats, GitError> {
        let tree = commit.tree()?;
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = self
            .repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

        let stats = diff.stats()?;
        Ok((
            Some(stats.files_changed()),
            Some(stats.insertions()),
            Some(stats.deletions()),
        ))
    }

    /// Get file statuses (staged, unstaged, untracked)
    pub fn file_statuses(&self, include_untracked: bool) -> Result<FileStatusGroup, GitError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(include_untracked);
        opts.include_ignored(false);

        let statuses = self.repo.statuses(Some(&mut opts))?;

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("").to_string();
            let status = entry.status();

            // Staged changes (index)
            if status.is_index_new() {
                staged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::New,
                });
            } else if status.is_index_modified() {
                staged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::Modified,
                });
            } else if status.is_index_deleted() {
                staged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::Deleted,
                });
            } else if status.is_index_renamed() {
                staged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::Renamed,
                });
            } else if status.is_index_typechange() {
                staged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::TypeChange,
                });
            }

            // Unstaged changes (workdir)
            if status.is_wt_modified() {
                unstaged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::Modified,
                });
            } else if status.is_wt_deleted() {
                unstaged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::Deleted,
                });
            } else if status.is_wt_renamed() {
                unstaged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::Renamed,
                });
            } else if status.is_wt_typechange() {
                unstaged.push(FileStatus {
                    path: path.clone(),
                    status: FileStatusType::TypeChange,
                });
            }

            // Untracked
            if status.is_wt_new() {
                untracked.push(path);
            }
        }

        Ok((staged, unstaged, untracked))
    }

    /// Get diff of uncommitted changes
    pub fn uncommitted_diff(&self, max_size: usize) -> Result<String, GitError> {
        let mut diff_opts = DiffOptions::new();
        diff_opts.include_untracked(false);

        // Get diff between HEAD and index (staged)
        let head_tree = self.repo.head()?.peel_to_tree()?;
        let staged_diff =
            self.repo
                .diff_tree_to_index(Some(&head_tree), None, Some(&mut diff_opts))?;

        // Get diff between index and workdir (unstaged)
        let unstaged_diff = self
            .repo
            .diff_index_to_workdir(None, Some(&mut diff_opts))?;

        let mut output = String::new();

        // Format staged diff
        staged_diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if output.len() < max_size {
                let prefix = match line.origin() {
                    '+' => "+",
                    '-' => "-",
                    ' ' => " ",
                    _ => "",
                };
                if !prefix.is_empty() {
                    output.push_str(prefix);
                }
                if let Ok(content) = std::str::from_utf8(line.content()) {
                    output.push_str(content);
                }
            }
            true
        })?;

        // Format unstaged diff
        unstaged_diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if output.len() < max_size {
                let prefix = match line.origin() {
                    '+' => "+",
                    '-' => "-",
                    ' ' => " ",
                    _ => "",
                };
                if !prefix.is_empty() {
                    output.push_str(prefix);
                }
                if let Ok(content) = std::str::from_utf8(line.content()) {
                    output.push_str(content);
                }
            }
            true
        })?;

        if output.len() >= max_size {
            output.truncate(max_size);
            output.push_str("\n... (truncated)");
        }

        Ok(output)
    }

    /// Get ahead/behind counts relative to upstream
    pub fn ahead_behind(&self) -> Result<(usize, usize), GitError> {
        let head = self.repo.head()?;
        if !head.is_branch() {
            return Ok((0, 0));
        }

        let branch_name = head
            .shorthand()
            .ok_or_else(|| GitError::InvalidRef("Could not get branch name".to_string()))?;

        let local = self.repo.find_branch(branch_name, BranchType::Local)?;
        let upstream = match local.upstream() {
            Ok(u) => u,
            Err(_) => return Ok((0, 0)),
        };

        let local_oid = local
            .get()
            .target()
            .ok_or_else(|| GitError::InvalidRef("Local branch has no target".to_string()))?;

        let upstream_oid = upstream
            .get()
            .target()
            .ok_or_else(|| GitError::InvalidRef("Upstream branch has no target".to_string()))?;

        let (ahead, behind) = self.repo.graph_ahead_behind(local_oid, upstream_oid)?;
        Ok((ahead, behind))
    }

    /// Collect all git context for LLM consumption
    #[allow(clippy::needless_pass_by_value)] // Ergonomic API - options is cheap to clone
    pub fn collect_context(&self, options: GitContextOptions) -> Result<GitContext, GitError> {
        let branch = self.current_branch()?;
        let upstream = self.upstream_branch()?;
        let is_dirty = self.is_dirty()?;
        let commits = self.recent_commits(options.max_commits, options.include_stats)?;
        let (staged, unstaged, untracked) = self.file_statuses(options.include_untracked)?;
        let (ahead, behind) = self.ahead_behind().unwrap_or((0, 0));

        let diff = if options.include_diff && is_dirty {
            Some(self.uncommitted_diff(options.max_diff_size)?)
        } else {
            None
        };

        Ok(GitContext {
            repo_root: self.repo_path.display().to_string(),
            branch,
            upstream,
            is_dirty,
            commits,
            staged,
            unstaged,
            untracked: if options.include_untracked {
                untracked
            } else {
                Vec::new()
            },
            diff,
            ahead,
            behind,
        })
    }

    /// Get diff between two refs
    pub fn diff_refs(&self, from: &str, to: &str, max_size: usize) -> Result<String, GitError> {
        let from_obj = self.repo.revparse_single(from)?;
        let to_obj = self.repo.revparse_single(to)?;

        let from_tree = from_obj.peel_to_tree()?;
        let to_tree = to_obj.peel_to_tree()?;

        let diff = self
            .repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)?;

        let mut output = String::new();

        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if output.len() < max_size {
                let prefix = match line.origin() {
                    '+' => "+",
                    '-' => "-",
                    ' ' => " ",
                    _ => "",
                };
                if !prefix.is_empty() {
                    output.push_str(prefix);
                }
                if let Ok(content) = std::str::from_utf8(line.content()) {
                    output.push_str(content);
                }
            }
            true
        })?;

        if output.len() >= max_size {
            output.truncate(max_size);
            output.push_str("\n... (truncated)");
        }

        Ok(output)
    }
}

/// Git tool implementation for DashFlow Tool trait
#[derive(Debug, Clone)]
pub struct GitInfoTool {
    working_dir: PathBuf,
}

impl GitInfoTool {
    /// Create a new git info tool
    #[must_use]
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
        }
    }
}

#[async_trait]
impl Tool for GitInfoTool {
    fn name(&self) -> &'static str {
        "git_info"
    }

    fn description(&self) -> &'static str {
        "Get information about the current git repository including branch, status, recent commits, and diffs."
    }

    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "commits", "diff", "context"],
                    "description": "The git action to perform"
                },
                "max_commits": {
                    "type": "integer",
                    "description": "Maximum commits to return (for commits/context actions)",
                    "default": 10
                },
                "include_diff": {
                    "type": "boolean",
                    "description": "Include diff in context output",
                    "default": true
                }
            },
            "required": ["action"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let params: serde_json::Value = match input {
            ToolInput::String(s) => serde_json::json!({"action": s}),
            ToolInput::Structured(obj) => obj,
        };

        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("status")
            .to_string();

        let max_commits = params
            .get("max_commits")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let include_diff = params
            .get("include_diff")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Clone working_dir for move into spawn_blocking
        let working_dir = self.working_dir.clone();

        // All git2 operations are blocking I/O - wrap in spawn_blocking
        tokio::task::spawn_blocking(move || {
            call_git_sync(&working_dir, &action, max_commits, include_diff)
        })
        .await
        .map_err(|e| Error::tool_error(format!("Git task panicked: {e}")))?
    }
}

/// Synchronous implementation of git tool operations.
///
/// All blocking git2 operations are contained here to be run via
/// `spawn_blocking` in the async wrapper.
fn call_git_sync(
    working_dir: &Path,
    action: &str,
    max_commits: usize,
    include_diff: bool,
) -> Result<String, Error> {
    let git = GitTool::discover(working_dir)
        .map_err(|e| Error::tool_error(format!("Git error: {e}")))?;

    match action {
        "status" => {
            let branch = git
                .current_branch()
                .map_err(|e| Error::tool_error(e.to_string()))?;
            let is_dirty = git
                .is_dirty()
                .map_err(|e| Error::tool_error(e.to_string()))?;
            let (staged, unstaged, untracked) = git
                .file_statuses(true)
                .map_err(|e| Error::tool_error(e.to_string()))?;

            let mut output = format!("Branch: {}\n", branch);
            output.push_str(&format!("Dirty: {}\n", is_dirty));

            if !staged.is_empty() {
                output.push_str("\nStaged:\n");
                for f in &staged {
                    output.push_str(&format!("  {:?}: {}\n", f.status, f.path));
                }
            }

            if !unstaged.is_empty() {
                output.push_str("\nUnstaged:\n");
                for f in &unstaged {
                    output.push_str(&format!("  {:?}: {}\n", f.status, f.path));
                }
            }

            if !untracked.is_empty() {
                output.push_str("\nUntracked:\n");
                for f in &untracked {
                    output.push_str(&format!("  {}\n", f));
                }
            }

            Ok(output)
        }
        "commits" => {
            let commits = git
                .recent_commits(max_commits, false)
                .map_err(|e| Error::tool_error(e.to_string()))?;

            let mut output = String::new();
            for c in commits {
                output.push_str(&format!("{} {} <{}>\n", c.hash, c.author, c.author_email));
                output.push_str(&format!("    {}\n", c.summary));
            }

            Ok(output)
        }
        "diff" => {
            let diff = git
                .uncommitted_diff(50_000)
                .map_err(|e| Error::tool_error(e.to_string()))?;

            if diff.is_empty() {
                Ok("No uncommitted changes".to_string())
            } else {
                Ok(diff)
            }
        }
        "context" => {
            let options = GitContextOptions {
                max_commits,
                include_diff,
                ..Default::default()
            };

            let context = git
                .collect_context(options)
                .map_err(|e| Error::tool_error(e.to_string()))?;

            Ok(context.to_prompt_string())
        }
        _ => Err(Error::tool_error(format!("Unknown action: {}", action))),
    }
}

#[cfg(test)]
// SAFETY: Tests use unwrap() to panic on unexpected errors, clearly indicating test failure.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, GitTool) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();

        // Configure git
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(path)
            .output()
            .unwrap();

        let git = GitTool::discover(path).unwrap();
        (temp_dir, git)
    }

    // ========================================================================
    // CORE GITTOOL TESTS
    // ========================================================================

    #[test]
    fn test_discover_repo() {
        let (_temp_dir, git) = setup_test_repo();
        assert!(git.root_path().exists());
    }

    #[test]
    fn test_discover_from_subdirectory() {
        let (temp_dir, _git) = setup_test_repo();

        // Create a subdirectory
        let subdir = temp_dir.path().join("src").join("lib");
        std::fs::create_dir_all(&subdir).unwrap();

        // Should discover repo from subdirectory
        let git = GitTool::discover(&subdir).unwrap();
        // Use canonicalize to handle /private/var vs /var symlink on macOS
        let expected = temp_dir.path().canonicalize().unwrap();
        let actual = git.root_path().canonicalize().unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_discover_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = GitTool::discover(temp_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, GitError::NotFound(_)));
    }

    #[test]
    fn test_open_repo() {
        let (temp_dir, _) = setup_test_repo();
        let git = GitTool::open(temp_dir.path()).unwrap();
        assert!(git.root_path().exists());
    }

    #[test]
    fn test_open_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = GitTool::open(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_current_branch() {
        let (_temp_dir, git) = setup_test_repo();
        let branch = git.current_branch().unwrap();
        // Default branch could be "master" or "main"
        assert!(branch == "master" || branch == "main");
    }

    #[test]
    fn test_is_dirty_clean() {
        let (_temp_dir, git) = setup_test_repo();
        assert!(!git.is_dirty().unwrap());
    }

    #[test]
    fn test_is_dirty_with_changes() {
        let (temp_dir, git) = setup_test_repo();

        // Make a change
        std::fs::write(temp_dir.path().join("new_file.txt"), "content").unwrap();

        assert!(git.is_dirty().unwrap());
    }

    #[test]
    fn test_recent_commits() {
        let (_temp_dir, git) = setup_test_repo();
        let commits = git.recent_commits(10, false).unwrap();

        assert!(!commits.is_empty());
        assert_eq!(commits[0].summary, "Initial commit");
        assert_eq!(commits[0].author, "Test User");
    }

    #[test]
    fn test_recent_commits_with_stats() {
        let (_temp_dir, git) = setup_test_repo();
        let commits = git.recent_commits(10, true).unwrap();

        assert!(!commits.is_empty());
        // Stats should be populated
        assert!(commits[0].files_changed.is_some());
    }

    #[test]
    fn test_recent_commits_limit() {
        let (temp_dir, git) = setup_test_repo();

        // Create more commits
        for i in 0..5 {
            std::fs::write(temp_dir.path().join(format!("file{}.txt", i)), "content").unwrap();
            Command::new("git")
                .args(["add", "."])
                .current_dir(temp_dir.path())
                .output()
                .unwrap();
            Command::new("git")
                .args(["commit", "-m", &format!("Commit {}", i)])
                .current_dir(temp_dir.path())
                .output()
                .unwrap();
        }

        // Request only 3 commits
        let commits = git.recent_commits(3, false).unwrap();
        assert_eq!(commits.len(), 3);
    }

    #[test]
    fn test_file_statuses() {
        let (temp_dir, git) = setup_test_repo();

        // Create untracked file
        std::fs::write(temp_dir.path().join("untracked.txt"), "content").unwrap();

        let (staged, unstaged, untracked) = git.file_statuses(true).unwrap();

        assert!(staged.is_empty());
        assert!(unstaged.is_empty());
        assert!(untracked.contains(&"untracked.txt".to_string()));
    }

    #[test]
    fn test_file_statuses_exclude_untracked() {
        let (temp_dir, git) = setup_test_repo();

        // Create untracked file
        std::fs::write(temp_dir.path().join("untracked.txt"), "content").unwrap();

        let (staged, unstaged, untracked) = git.file_statuses(false).unwrap();

        assert!(staged.is_empty());
        assert!(unstaged.is_empty());
        assert!(untracked.is_empty()); // Not included when include_untracked=false
    }

    #[test]
    fn test_staged_changes() {
        let (temp_dir, git) = setup_test_repo();

        // Create and stage a file
        std::fs::write(temp_dir.path().join("staged.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let (staged, _unstaged, _untracked) = git.file_statuses(true).unwrap();

        assert!(!staged.is_empty());
        assert_eq!(staged[0].path, "staged.txt");
        assert_eq!(staged[0].status, FileStatusType::New);
    }

    #[test]
    fn test_unstaged_modified() {
        let (temp_dir, git) = setup_test_repo();

        // Modify an existing tracked file
        std::fs::write(temp_dir.path().join("README.md"), "# Modified").unwrap();

        let (staged, unstaged, _untracked) = git.file_statuses(true).unwrap();

        assert!(staged.is_empty());
        assert!(!unstaged.is_empty());
        assert_eq!(unstaged[0].path, "README.md");
        assert_eq!(unstaged[0].status, FileStatusType::Modified);
    }

    #[test]
    fn test_uncommitted_diff_empty() {
        let (_temp_dir, git) = setup_test_repo();

        let diff = git.uncommitted_diff(50_000).unwrap();
        assert!(diff.is_empty());
    }

    #[test]
    fn test_uncommitted_diff_with_changes() {
        let (temp_dir, git) = setup_test_repo();

        // Modify file
        std::fs::write(temp_dir.path().join("README.md"), "# Modified Content").unwrap();

        let diff = git.uncommitted_diff(50_000).unwrap();
        assert!(diff.contains("Modified"));
    }

    #[test]
    fn test_uncommitted_diff_truncation() {
        let (temp_dir, git) = setup_test_repo();

        // Create large change
        let large_content = "A".repeat(10_000);
        std::fs::write(temp_dir.path().join("README.md"), &large_content).unwrap();

        // Request small max size
        let diff = git.uncommitted_diff(100).unwrap();
        assert!(diff.contains("truncated") || diff.len() <= 200);
    }

    #[test]
    fn test_diff_refs() {
        let (temp_dir, git) = setup_test_repo();

        // Create another commit
        std::fs::write(temp_dir.path().join("file2.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Diff between HEAD~1 and HEAD
        let diff = git.diff_refs("HEAD~1", "HEAD", 50_000).unwrap();
        assert!(diff.contains("file2"));
    }

    #[test]
    fn test_diff_refs_invalid_ref() {
        let (_temp_dir, git) = setup_test_repo();

        let result = git.diff_refs("invalid_ref", "HEAD", 50_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_upstream_branch_none() {
        let (_temp_dir, git) = setup_test_repo();

        // Local repo has no upstream
        let upstream = git.upstream_branch().unwrap();
        assert!(upstream.is_none());
    }

    #[test]
    fn test_ahead_behind_no_upstream() {
        let (_temp_dir, git) = setup_test_repo();

        let (ahead, behind) = git.ahead_behind().unwrap();
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    // ========================================================================
    // GITCONTEXTOPTIONS TESTS
    // ========================================================================

    #[test]
    fn test_git_context_options_default() {
        let options = GitContextOptions::default();

        assert_eq!(options.max_commits, 10);
        assert!(options.include_diff);
        assert!(options.include_untracked);
        assert_eq!(options.max_diff_size, 50_000);
        assert!(!options.include_stats);
    }

    #[test]
    fn test_git_context_options_serde() {
        let options = GitContextOptions {
            max_commits: 5,
            include_diff: false,
            include_untracked: false,
            max_diff_size: 10_000,
            include_stats: true,
        };

        let json = serde_json::to_string(&options).unwrap();
        let parsed: GitContextOptions = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.max_commits, 5);
        assert!(!parsed.include_diff);
        assert!(!parsed.include_untracked);
        assert_eq!(parsed.max_diff_size, 10_000);
        assert!(parsed.include_stats);
    }

    #[test]
    fn test_git_context_options_partial_deserialize() {
        // Missing fields should use defaults
        let json = r#"{"max_commits": 20}"#;
        let options: GitContextOptions = serde_json::from_str(json).unwrap();

        assert_eq!(options.max_commits, 20);
        assert!(options.include_diff); // default
        assert!(options.include_untracked); // default
    }

    // ========================================================================
    // GITCONTEXT TESTS
    // ========================================================================

    #[test]
    fn test_collect_context() {
        let (_temp_dir, git) = setup_test_repo();

        let context = git.collect_context(GitContextOptions::default()).unwrap();

        assert!(!context.branch.is_empty());
        assert!(!context.commits.is_empty());
        assert!(!context.is_dirty);
    }

    #[test]
    fn test_collect_context_dirty() {
        let (temp_dir, git) = setup_test_repo();

        // Make repo dirty
        std::fs::write(temp_dir.path().join("dirty.txt"), "content").unwrap();

        let context = git.collect_context(GitContextOptions::default()).unwrap();

        assert!(context.is_dirty);
        assert!(!context.untracked.is_empty());
    }

    #[test]
    fn test_context_to_prompt_string() {
        let (_temp_dir, git) = setup_test_repo();

        let context = git.collect_context(GitContextOptions::default()).unwrap();
        let prompt = context.to_prompt_string();

        assert!(prompt.contains("Branch:"));
        assert!(prompt.contains("Recent commits:"));
    }

    #[test]
    fn test_context_to_prompt_string_dirty() {
        let (temp_dir, git) = setup_test_repo();

        // Create dirty state
        std::fs::write(temp_dir.path().join("new.txt"), "content").unwrap();

        let context = git.collect_context(GitContextOptions::default()).unwrap();
        let prompt = context.to_prompt_string();

        assert!(prompt.contains("Uncommitted changes:"));
        assert!(prompt.contains("Untracked:"));
    }

    #[test]
    fn test_context_to_prompt_string_with_diff() {
        let (temp_dir, git) = setup_test_repo();

        // Modify file
        std::fs::write(temp_dir.path().join("README.md"), "# Changed").unwrap();

        let context = git.collect_context(GitContextOptions::default()).unwrap();
        let prompt = context.to_prompt_string();

        assert!(prompt.contains("Diff:"));
        assert!(prompt.contains("```diff"));
    }

    #[test]
    fn test_context_to_json() {
        let (_temp_dir, git) = setup_test_repo();

        let context = git.collect_context(GitContextOptions::default()).unwrap();
        let json = context.to_json();

        assert!(json.is_object());
        assert!(json["branch"].is_string());
        assert!(json["commits"].is_array());
    }

    #[test]
    fn test_context_no_diff_option() {
        let (temp_dir, git) = setup_test_repo();

        // Make dirty
        std::fs::write(temp_dir.path().join("README.md"), "# Changed").unwrap();

        let options = GitContextOptions {
            include_diff: false,
            ..Default::default()
        };
        let context = git.collect_context(options).unwrap();

        assert!(context.diff.is_none());
    }

    // ========================================================================
    // COMMITINFO TESTS
    // ========================================================================

    #[test]
    fn test_commit_info_serde() {
        let commit = CommitInfo {
            hash: "abc1234".to_string(),
            full_hash: "abc1234567890".to_string(),
            summary: "Test commit".to_string(),
            message: "Test commit\n\nDetails".to_string(),
            author: "Test Author".to_string(),
            author_email: "test@test.com".to_string(),
            timestamp: 1234567890,
            files_changed: Some(3),
            insertions: Some(10),
            deletions: Some(5),
        };

        let json = serde_json::to_string(&commit).unwrap();
        let parsed: CommitInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.hash, "abc1234");
        assert_eq!(parsed.author, "Test Author");
        assert_eq!(parsed.files_changed, Some(3));
    }

    #[test]
    fn test_commit_info_clone() {
        let commit = CommitInfo {
            hash: "abc".to_string(),
            full_hash: "abc123".to_string(),
            summary: "Summary".to_string(),
            message: "Message".to_string(),
            author: "Author".to_string(),
            author_email: "a@b.com".to_string(),
            timestamp: 0,
            files_changed: None,
            insertions: None,
            deletions: None,
        };

        let cloned = commit.clone();
        assert_eq!(cloned.hash, commit.hash);
    }

    // ========================================================================
    // FILESTATUSTYPE TESTS
    // ========================================================================

    #[test]
    fn test_file_status_type_eq() {
        assert_eq!(FileStatusType::New, FileStatusType::New);
        assert_ne!(FileStatusType::New, FileStatusType::Modified);
    }

    #[test]
    fn test_file_status_type_copy() {
        let status = FileStatusType::Modified;
        let copied = status;
        assert_eq!(copied, FileStatusType::Modified);
    }

    #[test]
    fn test_file_status_type_serde() {
        let statuses = vec![
            FileStatusType::New,
            FileStatusType::Modified,
            FileStatusType::Deleted,
            FileStatusType::Renamed,
            FileStatusType::Copied,
            FileStatusType::TypeChange,
            FileStatusType::Untracked,
            FileStatusType::Ignored,
            FileStatusType::Conflicted,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: FileStatusType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_file_status_type_debug() {
        let status = FileStatusType::Modified;
        let debug = format!("{:?}", status);
        assert!(debug.contains("Modified"));
    }

    // ========================================================================
    // FILESTATUS TESTS
    // ========================================================================

    #[test]
    fn test_file_status_serde() {
        let file_status = FileStatus {
            path: "src/main.rs".to_string(),
            status: FileStatusType::Modified,
        };

        let json = serde_json::to_string(&file_status).unwrap();
        let parsed: FileStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.path, "src/main.rs");
        assert_eq!(parsed.status, FileStatusType::Modified);
    }

    #[test]
    fn test_file_status_clone() {
        let file_status = FileStatus {
            path: "test.txt".to_string(),
            status: FileStatusType::New,
        };

        let cloned = file_status.clone();
        assert_eq!(cloned.path, file_status.path);
        assert_eq!(cloned.status, file_status.status);
    }

    // ========================================================================
    // GITERROR TESTS
    // ========================================================================

    #[test]
    fn test_git_error_not_found_display() {
        let err = GitError::NotFound("test/path".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Git repository not found"));
        assert!(msg.contains("test/path"));
    }

    #[test]
    fn test_git_error_invalid_ref_display() {
        let err = GitError::InvalidRef("bad_ref".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Invalid reference"));
        assert!(msg.contains("bad_ref"));
    }

    #[test]
    fn test_git_error_path_display() {
        let err = GitError::PathError("some/path".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Path error"));
    }

    #[test]
    fn test_git_error_debug() {
        let err = GitError::NotFound("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
    }

    // ========================================================================
    // GITINFOTOOL TESTS
    // ========================================================================

    #[test]
    fn test_git_info_tool() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        assert_eq!(tool.name(), "git_info");
        assert!(tool.description().contains("git"));
    }

    #[test]
    fn test_git_info_tool_args_schema() {
        let tool = GitInfoTool::new("/tmp");
        let schema = tool.args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["action"].is_object());
        assert!(schema["properties"]["max_commits"].is_object());
        assert!(schema["properties"]["include_diff"].is_object());
    }

    #[test]
    fn test_git_info_tool_debug() {
        let tool = GitInfoTool::new("/some/path");
        let debug = format!("{:?}", tool);
        assert!(debug.contains("GitInfoTool"));
        assert!(debug.contains("/some/path"));
    }

    #[test]
    fn test_git_info_tool_clone() {
        let tool = GitInfoTool::new("/tmp");
        let cloned = tool.clone();
        assert_eq!(cloned.working_dir, tool.working_dir);
    }

    #[tokio::test]
    async fn test_git_info_tool_status() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "status"});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("Branch:"));
    }

    #[tokio::test]
    async fn test_git_info_tool_commits() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "commits", "max_commits": 5});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("Initial commit"));
    }

    #[tokio::test]
    async fn test_git_info_tool_context() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "context"});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("Branch:"));
        assert!(result.contains("Recent commits:"));
    }

    #[tokio::test]
    async fn test_git_info_tool_diff() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "diff"});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        // Clean repo
        assert!(result.contains("No uncommitted changes"));
    }

    #[tokio::test]
    async fn test_git_info_tool_diff_with_changes() {
        let (temp_dir, _git) = setup_test_repo();

        // Make a change
        std::fs::write(temp_dir.path().join("README.md"), "# Changed").unwrap();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "diff"});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("Changed") || result.contains("-") || result.contains("+"));
    }

    #[tokio::test]
    async fn test_git_info_tool_unknown_action() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "unknown_action"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_git_info_tool_string_input() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        // String input uses the string as the action
        let result = tool._call(ToolInput::String("status".to_string())).await.unwrap();

        assert!(result.contains("Branch:"));
    }

    #[tokio::test]
    async fn test_git_info_tool_default_action() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        // Empty object should use default action "status"
        let input = serde_json::json!({});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        assert!(result.contains("Branch:"));
    }

    #[tokio::test]
    async fn test_git_info_tool_not_a_repo() {
        let temp_dir = TempDir::new().unwrap();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "status"});
        let result = tool._call(ToolInput::Structured(input)).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Git error"));
    }

    #[tokio::test]
    async fn test_git_info_tool_include_diff_false() {
        let (temp_dir, _git) = setup_test_repo();

        let tool = GitInfoTool::new(temp_dir.path());
        let input = serde_json::json!({"action": "context", "include_diff": false});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();

        // Should still work but without diff section
        assert!(result.contains("Branch:"));
    }

    // ========================================================================
    // GITTOOL DEBUG TESTS
    // ========================================================================

    #[test]
    fn test_git_tool_debug() {
        let (_temp_dir, git) = setup_test_repo();
        let debug = format!("{:?}", git);
        assert!(debug.contains("GitTool"));
        assert!(debug.contains("repo_path"));
    }

    // ========================================================================
    // EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_multiple_commits() {
        let (temp_dir, git) = setup_test_repo();

        // Create multiple commits
        for i in 0..10 {
            std::fs::write(
                temp_dir.path().join(format!("file{}.txt", i)),
                format!("content {}", i),
            )
            .unwrap();
            Command::new("git")
                .args(["add", "."])
                .current_dir(temp_dir.path())
                .output()
                .unwrap();
            Command::new("git")
                .args(["commit", "-m", &format!("Commit {}", i)])
                .current_dir(temp_dir.path())
                .output()
                .unwrap();
        }

        let commits = git.recent_commits(100, false).unwrap();
        assert_eq!(commits.len(), 11); // Initial + 10 new
    }

    #[test]
    fn test_special_characters_in_filenames() {
        let (temp_dir, git) = setup_test_repo();

        // Create file with special characters
        std::fs::write(temp_dir.path().join("file with spaces.txt"), "content").unwrap();

        let (_, _, untracked) = git.file_statuses(true).unwrap();
        assert!(untracked.contains(&"file with spaces.txt".to_string()));
    }

    #[test]
    fn test_staged_and_unstaged_same_file() {
        let (temp_dir, git) = setup_test_repo();

        // Stage a change
        std::fs::write(temp_dir.path().join("README.md"), "# Staged change").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Make another change to same file (unstaged)
        std::fs::write(temp_dir.path().join("README.md"), "# Unstaged change").unwrap();

        let (staged, unstaged, _) = git.file_statuses(true).unwrap();

        assert!(!staged.is_empty());
        assert!(!unstaged.is_empty());
        assert_eq!(staged[0].path, "README.md");
        assert_eq!(unstaged[0].path, "README.md");
    }

    #[test]
    fn test_deleted_file() {
        let (temp_dir, git) = setup_test_repo();

        // Delete tracked file
        std::fs::remove_file(temp_dir.path().join("README.md")).unwrap();

        let (_, unstaged, _) = git.file_statuses(true).unwrap();

        assert!(!unstaged.is_empty());
        assert_eq!(unstaged[0].status, FileStatusType::Deleted);
    }

    #[test]
    fn test_staged_deletion() {
        let (temp_dir, git) = setup_test_repo();

        // Stage deletion
        std::fs::remove_file(temp_dir.path().join("README.md")).unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let (staged, _, _) = git.file_statuses(true).unwrap();

        assert!(!staged.is_empty());
        assert_eq!(staged[0].status, FileStatusType::Deleted);
    }

    #[test]
    fn test_context_ahead_behind_values() {
        let (_temp_dir, git) = setup_test_repo();

        let context = git.collect_context(GitContextOptions::default()).unwrap();

        // No upstream, so should be 0
        assert_eq!(context.ahead, 0);
        assert_eq!(context.behind, 0);
    }

    #[test]
    fn test_prompt_string_ahead_behind() {
        // Manually create a context with ahead/behind
        let context = GitContext {
            repo_root: "/test".to_string(),
            branch: "main".to_string(),
            upstream: Some("origin/main".to_string()),
            is_dirty: false,
            commits: vec![],
            staged: vec![],
            unstaged: vec![],
            untracked: vec![],
            diff: None,
            ahead: 3,
            behind: 2,
        };

        let prompt = context.to_prompt_string();
        assert!(prompt.contains("Ahead: 3"));
        assert!(prompt.contains("Behind: 2"));
    }

    #[test]
    fn test_prompt_string_staged_files() {
        let context = GitContext {
            repo_root: "/test".to_string(),
            branch: "main".to_string(),
            upstream: None,
            is_dirty: true,
            commits: vec![],
            staged: vec![FileStatus {
                path: "staged.txt".to_string(),
                status: FileStatusType::New,
            }],
            unstaged: vec![],
            untracked: vec![],
            diff: None,
            ahead: 0,
            behind: 0,
        };

        let prompt = context.to_prompt_string();
        assert!(prompt.contains("Staged:"));
        assert!(prompt.contains("staged.txt"));
    }

    #[test]
    fn test_diff_ends_without_newline() {
        let context = GitContext {
            repo_root: "/test".to_string(),
            branch: "main".to_string(),
            upstream: None,
            is_dirty: true,
            commits: vec![],
            staged: vec![],
            unstaged: vec![],
            untracked: vec![],
            diff: Some("no trailing newline".to_string()),
            ahead: 0,
            behind: 0,
        };

        let prompt = context.to_prompt_string();
        // Should add newline before closing fence
        assert!(prompt.contains("no trailing newline\n```"));
    }

    #[test]
    fn test_diff_ends_with_newline() {
        let context = GitContext {
            repo_root: "/test".to_string(),
            branch: "main".to_string(),
            upstream: None,
            is_dirty: true,
            commits: vec![],
            staged: vec![],
            unstaged: vec![],
            untracked: vec![],
            diff: Some("has trailing newline\n".to_string()),
            ahead: 0,
            behind: 0,
        };

        let prompt = context.to_prompt_string();
        // Should not double the newline
        assert!(prompt.contains("has trailing newline\n```"));
    }
}
