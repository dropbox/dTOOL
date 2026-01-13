//! Ghost commit functionality for undo/restore support
//!
//! Ghost commits capture the complete state of a repository's working tree
//! at a specific point in time, enabling the agent to restore to previous states.
//!
//! This is essential for implementing "undo" functionality in the coding agent.
//!
//! Ported from codex-rs/utils/git/src/ghost_commits.rs

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use thiserror::Error;

/// Default commit message used for ghost commits when none is provided.
const DEFAULT_COMMIT_MESSAGE: &str = "codex snapshot";

/// Default threshold that triggers a warning about large untracked directories.
const LARGE_UNTRACKED_WARNING_THRESHOLD: usize = 200;

/// Directories that should always be ignored when capturing ghost snapshots,
/// even if they are not listed in .gitignore.
const DEFAULT_IGNORED_DIR_NAMES: &[&str] = &[
    "node_modules",
    ".venv",
    "venv",
    "env",
    ".env",
    "dist",
    "build",
    ".pytest_cache",
    ".mypy_cache",
    ".cache",
    ".tox",
    "__pycache__",
    "target", // Rust build directory
];

/// Errors returned while managing git worktree snapshots.
#[derive(Debug, Error)]
pub enum GhostCommitError {
    #[error("git command `{command}` failed with status {status}: {stderr}")]
    GitCommand {
        command: String,
        status: i32,
        stderr: String,
    },
    #[error("git command `{command}` produced non-UTF-8 output")]
    GitOutputUtf8 { command: String },
    #[error("{path:?} is not a git repository")]
    NotAGitRepository { path: PathBuf },
    #[error("path {path:?} must be relative to the repository root")]
    NonRelativePath { path: PathBuf },
    #[error("path {path:?} escapes the repository root")]
    PathEscapesRepository { path: PathBuf },
    #[error(transparent)]
    Io(#[from] io::Error),
}

type CommitID = String;

/// Details of a ghost commit created from a repository state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GhostCommit {
    id: CommitID,
    parent: Option<CommitID>,
    preexisting_untracked_files: Vec<PathBuf>,
    preexisting_untracked_dirs: Vec<PathBuf>,
}

impl GhostCommit {
    /// Create a new ghost commit wrapper from a raw commit ID and optional parent.
    pub fn new(
        id: CommitID,
        parent: Option<CommitID>,
        preexisting_untracked_files: Vec<PathBuf>,
        preexisting_untracked_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            id,
            parent,
            preexisting_untracked_files,
            preexisting_untracked_dirs,
        }
    }

    /// Commit ID for the snapshot.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parent commit ID, if the repository had a `HEAD` at creation time.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// Untracked or ignored files that already existed when the snapshot was captured.
    pub fn preexisting_untracked_files(&self) -> &[PathBuf] {
        &self.preexisting_untracked_files
    }

    /// Untracked or ignored directories that already existed when the snapshot was captured.
    pub fn preexisting_untracked_dirs(&self) -> &[PathBuf] {
        &self.preexisting_untracked_dirs
    }
}

impl fmt::Display for GhostCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Options to control ghost commit creation.
pub struct CreateGhostCommitOptions<'a> {
    /// Path to the repository (or subdirectory within it)
    pub repo_path: &'a Path,
    /// Custom commit message (defaults to "codex snapshot")
    pub message: Option<&'a str>,
    /// Paths to forcibly include even if they are ignored
    pub force_include: Vec<PathBuf>,
}

impl<'a> CreateGhostCommitOptions<'a> {
    /// Creates options scoped to the provided repository path.
    pub fn new(repo_path: &'a Path) -> Self {
        Self {
            repo_path,
            message: None,
            force_include: Vec::new(),
        }
    }

    /// Sets a custom commit message for the ghost commit.
    pub fn message(mut self, message: &'a str) -> Self {
        self.message = Some(message);
        self
    }

    /// Supplies the entire force-include path list at once.
    pub fn force_include<I>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = PathBuf>,
    {
        self.force_include = paths.into_iter().collect();
        self
    }

    /// Adds a single path to the force-include list.
    pub fn push_force_include<P>(mut self, path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.force_include.push(path.into());
        self
    }
}

/// Summary produced alongside a ghost snapshot.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GhostSnapshotReport {
    /// Directories containing a large number of untracked files
    pub large_untracked_dirs: Vec<LargeUntrackedDir>,
}

/// Directory containing a large amount of untracked content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LargeUntrackedDir {
    /// Path to the directory
    pub path: PathBuf,
    /// Number of files in the directory
    pub file_count: usize,
}

impl GhostSnapshotReport {
    /// Format a warning message about large untracked directories.
    pub fn format_large_untracked_warning(&self) -> Option<String> {
        if self.large_untracked_dirs.is_empty() {
            return None;
        }
        const MAX_DIRS: usize = 3;
        let mut parts: Vec<String> = Vec::new();
        for dir in self.large_untracked_dirs.iter().take(MAX_DIRS) {
            parts.push(format!("{} ({} files)", dir.path.display(), dir.file_count));
        }
        if self.large_untracked_dirs.len() > MAX_DIRS {
            let remaining = self.large_untracked_dirs.len() - MAX_DIRS;
            parts.push(format!("{remaining} more"));
        }
        Some(format!(
            "Repository snapshot encountered large untracked directories: {}. \
             This can slow down the agent; consider adding these paths to .gitignore \
             or disabling undo.",
            parts.join(", ")
        ))
    }
}

// ============================================================================
// Git helper functions (synchronous, blocking)
// ============================================================================

fn run_git_for_status(
    dir: &Path,
    args: &[OsString],
    env: Option<&[(OsString, OsString)]>,
) -> Result<(), GhostCommitError> {
    run_git(dir, args, env)?;
    Ok(())
}

fn run_git_for_stdout(
    dir: &Path,
    args: &[OsString],
    env: Option<&[(OsString, OsString)]>,
) -> Result<String, GhostCommitError> {
    let run = run_git(dir, args, env)?;
    String::from_utf8(run.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|_| GhostCommitError::GitOutputUtf8 {
            command: run.command,
        })
}

fn run_git_for_stdout_all(
    dir: &Path,
    args: &[OsString],
    env: Option<&[(OsString, OsString)]>,
) -> Result<String, GhostCommitError> {
    let run = run_git(dir, args, env)?;
    String::from_utf8(run.stdout).map_err(|_| GhostCommitError::GitOutputUtf8 {
        command: run.command,
    })
}

struct GitRun {
    command: String,
    stdout: Vec<u8>,
}

fn run_git(
    dir: &Path,
    args: &[OsString],
    env: Option<&[(OsString, OsString)]>,
) -> Result<GitRun, GhostCommitError> {
    let command_string = build_command_string(args);
    let mut command = Command::new("git");
    command.current_dir(dir);
    if let Some(envs) = env {
        for (key, value) in envs {
            command.env(key, value);
        }
    }
    command.args(args);
    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GhostCommitError::GitCommand {
            command: command_string,
            status: output.status.code().unwrap_or(-1),
            stderr,
        });
    }
    Ok(GitRun {
        command: command_string,
        stdout: output.stdout,
    })
}

fn build_command_string(args: &[OsString]) -> String {
    if args.is_empty() {
        return "git".to_string();
    }
    let joined = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    format!("git {joined}")
}

// ============================================================================
// Git operations
// ============================================================================

fn ensure_git_repository(path: &Path) -> Result<(), GhostCommitError> {
    let args = vec![
        OsString::from("rev-parse"),
        OsString::from("--is-inside-work-tree"),
    ];
    match run_git_for_stdout(path, &args, None) {
        Ok(output) if output.trim() == "true" => Ok(()),
        Ok(_) => Err(GhostCommitError::NotAGitRepository {
            path: path.to_path_buf(),
        }),
        Err(GhostCommitError::GitCommand { status: 128, .. }) => {
            Err(GhostCommitError::NotAGitRepository {
                path: path.to_path_buf(),
            })
        }
        Err(err) => Err(err),
    }
}

fn resolve_head(path: &Path) -> Result<Option<String>, GhostCommitError> {
    let args = vec![
        OsString::from("rev-parse"),
        OsString::from("--verify"),
        OsString::from("HEAD"),
    ];
    match run_git_for_stdout(path, &args, None) {
        Ok(sha) => Ok(Some(sha)),
        Err(GhostCommitError::GitCommand { status: 128, .. }) => Ok(None),
        Err(other) => Err(other),
    }
}

fn resolve_repository_root(path: &Path) -> Result<PathBuf, GhostCommitError> {
    let args = vec![
        OsString::from("rev-parse"),
        OsString::from("--show-toplevel"),
    ];
    let root = run_git_for_stdout(path, &args, None)?;
    Ok(PathBuf::from(root))
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, GhostCommitError> {
    let mut result = PathBuf::new();
    let mut saw_component = false;
    for component in path.components() {
        saw_component = true;
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    return Err(GhostCommitError::PathEscapesRepository {
                        path: path.to_path_buf(),
                    });
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(GhostCommitError::NonRelativePath {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    if !saw_component {
        return Err(GhostCommitError::NonRelativePath {
            path: path.to_path_buf(),
        });
    }

    Ok(result)
}

fn repo_subdir(repo_root: &Path, repo_path: &Path) -> Option<PathBuf> {
    if repo_root == repo_path {
        return None;
    }

    repo_path
        .strip_prefix(repo_root)
        .ok()
        .and_then(non_empty_path)
        .or_else(|| {
            let repo_root_canon = repo_root.canonicalize().ok()?;
            let repo_path_canon = repo_path.canonicalize().ok()?;
            repo_path_canon
                .strip_prefix(&repo_root_canon)
                .ok()
                .and_then(non_empty_path)
        })
}

fn non_empty_path(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path.to_path_buf())
    }
}

fn apply_repo_prefix_to_force_include(prefix: Option<&Path>, paths: &[PathBuf]) -> Vec<PathBuf> {
    if paths.is_empty() {
        return Vec::new();
    }

    match prefix {
        Some(prefix) => paths.iter().map(|path| prefix.join(path)).collect(),
        None => paths.to_vec(),
    }
}

// ============================================================================
// Untracked file detection
// ============================================================================

#[derive(Default)]
struct UntrackedSnapshot {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

fn capture_existing_untracked(
    repo_root: &Path,
    repo_prefix: Option<&Path>,
) -> Result<UntrackedSnapshot, GhostCommitError> {
    let mut args = vec![
        OsString::from("status"),
        OsString::from("--porcelain=2"),
        OsString::from("-z"),
        OsString::from("--untracked-files=all"),
    ];
    if let Some(prefix) = repo_prefix {
        args.push(OsString::from("--"));
        args.push(prefix.as_os_str().to_os_string());
    }

    let output = run_git_for_stdout_all(repo_root, &args, None)?;
    if output.is_empty() {
        return Ok(UntrackedSnapshot::default());
    }

    let mut snapshot = UntrackedSnapshot::default();
    for entry in output.split('\0') {
        if entry.is_empty() {
            continue;
        }
        let mut parts = entry.splitn(2, ' ');
        let code = parts.next();
        let path_part = parts.next();
        let (Some(code), Some(path_part)) = (code, path_part) else {
            continue;
        };
        if code != "?" && code != "!" {
            continue;
        }
        if path_part.is_empty() {
            continue;
        }

        let normalized = normalize_relative_path(Path::new(path_part))?;
        if should_ignore_for_snapshot(&normalized) {
            continue;
        }
        let absolute = repo_root.join(&normalized);
        let is_dir = absolute.is_dir();
        if is_dir {
            snapshot.dirs.push(normalized);
        } else {
            snapshot.files.push(normalized);
        }
    }

    Ok(snapshot)
}

fn should_ignore_for_snapshot(path: &Path) -> bool {
    path.components().any(|component| {
        if let Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                return DEFAULT_IGNORED_DIR_NAMES.contains(&name_str);
            }
        }
        false
    })
}

fn detect_large_untracked_dirs(files: &[PathBuf], dirs: &[PathBuf]) -> Vec<LargeUntrackedDir> {
    let mut counts: BTreeMap<PathBuf, usize> = BTreeMap::new();

    let mut sorted_dirs: Vec<&PathBuf> = dirs.iter().collect();
    sorted_dirs.sort_by(|a, b| {
        let a_components = a.components().count();
        let b_components = b.components().count();
        b_components.cmp(&a_components).then_with(|| a.cmp(b))
    });

    for file in files {
        let mut key: Option<PathBuf> = None;
        for dir in &sorted_dirs {
            if file.starts_with(dir.as_path()) {
                key = Some((*dir).clone());
                break;
            }
        }
        let key = key.unwrap_or_else(|| {
            file.parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        });
        let entry = counts.entry(key).or_insert(0);
        *entry += 1;
    }

    let mut result: Vec<LargeUntrackedDir> = counts
        .into_iter()
        .filter(|(_, count)| *count >= LARGE_UNTRACKED_WARNING_THRESHOLD)
        .map(|(path, file_count)| LargeUntrackedDir { path, file_count })
        .collect();
    result.sort_by(|a, b| {
        b.file_count
            .cmp(&a.file_count)
            .then_with(|| a.path.cmp(&b.path))
    });
    result
}

fn to_session_relative_path(path: &Path, repo_prefix: Option<&Path>) -> PathBuf {
    match repo_prefix {
        Some(prefix) => path
            .strip_prefix(prefix)
            .map(PathBuf::from)
            .unwrap_or_else(|_| path.to_path_buf()),
        None => path.to_path_buf(),
    }
}

// ============================================================================
// Ghost commit creation
// ============================================================================

/// Returns the default author and committer identity for ghost commits.
fn default_commit_identity() -> Vec<(OsString, OsString)> {
    vec![
        (
            OsString::from("GIT_AUTHOR_NAME"),
            OsString::from("Codex Snapshot"),
        ),
        (
            OsString::from("GIT_AUTHOR_EMAIL"),
            OsString::from("snapshot@codex.local"),
        ),
        (
            OsString::from("GIT_COMMITTER_NAME"),
            OsString::from("Codex Snapshot"),
        ),
        (
            OsString::from("GIT_COMMITTER_EMAIL"),
            OsString::from("snapshot@codex.local"),
        ),
    ]
}

/// Create a ghost commit capturing the current state of the repository's working tree.
pub fn create_ghost_commit(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<GhostCommit, GhostCommitError> {
    create_ghost_commit_with_report(options).map(|(commit, _)| commit)
}

/// Compute a report describing the working tree for a ghost snapshot without creating a commit.
pub fn capture_ghost_snapshot_report(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<GhostSnapshotReport, GhostCommitError> {
    ensure_git_repository(options.repo_path)?;

    let repo_root = resolve_repository_root(options.repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), options.repo_path);
    let existing_untracked =
        capture_existing_untracked(repo_root.as_path(), repo_prefix.as_deref())?;

    let warning_files = existing_untracked
        .files
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();
    let warning_dirs = existing_untracked
        .dirs
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();

    Ok(GhostSnapshotReport {
        large_untracked_dirs: detect_large_untracked_dirs(&warning_files, &warning_dirs),
    })
}

/// Create a ghost commit capturing the current state along with a report.
pub fn create_ghost_commit_with_report(
    options: &CreateGhostCommitOptions<'_>,
) -> Result<(GhostCommit, GhostSnapshotReport), GhostCommitError> {
    ensure_git_repository(options.repo_path)?;

    let repo_root = resolve_repository_root(options.repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), options.repo_path);
    let parent = resolve_head(repo_root.as_path())?;
    let existing_untracked =
        capture_existing_untracked(repo_root.as_path(), repo_prefix.as_deref())?;

    let warning_files = existing_untracked
        .files
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();
    let warning_dirs = existing_untracked
        .dirs
        .iter()
        .map(|path| to_session_relative_path(path, repo_prefix.as_deref()))
        .collect::<Vec<_>>();
    let large_untracked_dirs = detect_large_untracked_dirs(&warning_files, &warning_dirs);

    let normalized_force = options
        .force_include
        .iter()
        .map(|path| normalize_relative_path(path))
        .collect::<Result<Vec<_>, _>>()?;
    let force_include =
        apply_repo_prefix_to_force_include(repo_prefix.as_deref(), &normalized_force);

    // Create a temporary index file
    let index_tempdir = tempfile::Builder::new()
        .prefix("codex-git-index-")
        .tempdir()?;
    let index_path = index_tempdir.path().join("index");
    let base_env = vec![(
        OsString::from("GIT_INDEX_FILE"),
        OsString::from(index_path.as_os_str()),
    )];

    // Pre-populate the temporary index with HEAD
    if let Some(parent_sha) = parent.as_deref() {
        run_git_for_status(
            repo_root.as_path(),
            &[OsString::from("read-tree"), OsString::from(parent_sha)],
            Some(base_env.as_slice()),
        )?;
    }

    // Add all files
    let mut add_args = vec![OsString::from("add"), OsString::from("--all")];
    if let Some(prefix) = repo_prefix.as_deref() {
        add_args.extend([OsString::from("--"), prefix.as_os_str().to_os_string()]);
    }

    run_git_for_status(repo_root.as_path(), &add_args, Some(base_env.as_slice()))?;

    // Force include specified paths
    if !force_include.is_empty() {
        let mut args = Vec::with_capacity(force_include.len() + 2);
        args.push(OsString::from("add"));
        args.push(OsString::from("--force"));
        args.extend(
            force_include
                .iter()
                .map(|path| OsString::from(path.as_os_str())),
        );
        run_git_for_status(repo_root.as_path(), &args, Some(base_env.as_slice()))?;
    }

    // Write the tree
    let tree_id = run_git_for_stdout(
        repo_root.as_path(),
        &[OsString::from("write-tree")],
        Some(base_env.as_slice()),
    )?;

    // Create the commit
    let mut commit_env = base_env;
    commit_env.extend(default_commit_identity());
    let message = options.message.unwrap_or(DEFAULT_COMMIT_MESSAGE);
    let mut commit_args = vec![OsString::from("commit-tree"), OsString::from(&tree_id)];
    if let Some(parent) = parent.as_deref() {
        commit_args.extend([OsString::from("-p"), OsString::from(parent)]);
    }
    commit_args.extend([OsString::from("-m"), OsString::from(message)]);

    let commit_id = run_git_for_stdout(
        repo_root.as_path(),
        &commit_args,
        Some(commit_env.as_slice()),
    )?;

    let ghost_commit = GhostCommit::new(
        commit_id,
        parent,
        existing_untracked.files,
        existing_untracked.dirs,
    );

    Ok((
        ghost_commit,
        GhostSnapshotReport {
            large_untracked_dirs,
        },
    ))
}

// ============================================================================
// Ghost commit restoration
// ============================================================================

/// Restore the working tree to match the provided ghost commit.
pub fn restore_ghost_commit(
    repo_path: &Path,
    commit: &GhostCommit,
) -> Result<(), GhostCommitError> {
    ensure_git_repository(repo_path)?;

    let repo_root = resolve_repository_root(repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), repo_path);
    let current_untracked =
        capture_existing_untracked(repo_root.as_path(), repo_prefix.as_deref())?;
    restore_to_commit_inner(repo_root.as_path(), repo_prefix.as_deref(), commit.id())?;
    remove_new_untracked(
        repo_root.as_path(),
        commit.preexisting_untracked_files(),
        commit.preexisting_untracked_dirs(),
        current_untracked,
    )
}

/// Restore the working tree to match the given commit ID.
pub fn restore_to_commit(repo_path: &Path, commit_id: &str) -> Result<(), GhostCommitError> {
    ensure_git_repository(repo_path)?;

    let repo_root = resolve_repository_root(repo_path)?;
    let repo_prefix = repo_subdir(repo_root.as_path(), repo_path);
    restore_to_commit_inner(repo_root.as_path(), repo_prefix.as_deref(), commit_id)
}

fn restore_to_commit_inner(
    repo_root: &Path,
    repo_prefix: Option<&Path>,
    commit_id: &str,
) -> Result<(), GhostCommitError> {
    let mut restore_args = vec![
        OsString::from("restore"),
        OsString::from("--source"),
        OsString::from(commit_id),
        OsString::from("--worktree"),
        OsString::from("--staged"),
        OsString::from("--"),
    ];
    if let Some(prefix) = repo_prefix {
        restore_args.push(prefix.as_os_str().to_os_string());
    } else {
        restore_args.push(OsString::from("."));
    }

    run_git_for_status(repo_root, &restore_args, None)?;
    Ok(())
}

fn remove_new_untracked(
    repo_root: &Path,
    preserved_files: &[PathBuf],
    preserved_dirs: &[PathBuf],
    current: UntrackedSnapshot,
) -> Result<(), GhostCommitError> {
    if current.files.is_empty() && current.dirs.is_empty() {
        return Ok(());
    }

    let preserved_file_set: HashSet<PathBuf> = preserved_files.iter().cloned().collect();
    let preserved_dirs_vec: Vec<PathBuf> = preserved_dirs.to_vec();

    for path in current.files {
        if should_preserve(&path, &preserved_file_set, &preserved_dirs_vec) {
            continue;
        }
        remove_path(&repo_root.join(&path))?;
    }

    for dir in current.dirs {
        if should_preserve(&dir, &preserved_file_set, &preserved_dirs_vec) {
            continue;
        }
        remove_path(&repo_root.join(&dir))?;
    }

    Ok(())
}

fn should_preserve(
    path: &Path,
    preserved_files: &HashSet<PathBuf>,
    preserved_dirs: &[PathBuf],
) -> bool {
    if preserved_files.contains(path) {
        return true;
    }

    preserved_dirs
        .iter()
        .any(|dir| path.starts_with(dir.as_path()))
}

fn remove_path(path: &Path) -> Result<(), GhostCommitError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(err.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn run_git_in(repo_path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .status()
            .expect("git command");
        assert!(status.success(), "git command failed: {args:?}");
    }

    fn run_git_stdout(repo_path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .expect("git command");
        assert!(output.status.success(), "git command failed: {args:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn init_test_repo(repo: &Path) {
        run_git_in(repo, &["init", "--initial-branch=main"]);
        run_git_in(repo, &["config", "core.autocrlf", "false"]);
        run_git_in(repo, &["config", "user.name", "Test User"]);
        run_git_in(repo, &["config", "user.email", "test@example.com"]);
    }

    #[test]
    fn test_ghost_commit_struct() {
        let commit = GhostCommit::new(
            "abc123".to_string(),
            Some("def456".to_string()),
            vec![PathBuf::from("file1.txt")],
            vec![PathBuf::from("dir1")],
        );

        assert_eq!(commit.id(), "abc123");
        assert_eq!(commit.parent(), Some("def456"));
        assert_eq!(
            commit.preexisting_untracked_files(),
            &[PathBuf::from("file1.txt")]
        );
        assert_eq!(
            commit.preexisting_untracked_dirs(),
            &[PathBuf::from("dir1")]
        );
    }

    #[test]
    fn test_ghost_commit_display() {
        let commit = GhostCommit::new("abc123".to_string(), None, vec![], vec![]);
        assert_eq!(format!("{}", commit), "abc123");
    }

    #[test]
    fn test_ghost_commit_serialization() {
        let commit = GhostCommit::new(
            "abc123".to_string(),
            Some("def456".to_string()),
            vec![PathBuf::from("file.txt")],
            vec![],
        );
        let json = serde_json::to_string(&commit).unwrap();
        let parsed: GhostCommit = serde_json::from_str(&json).unwrap();
        assert_eq!(commit, parsed);
    }

    #[test]
    fn test_large_untracked_dir() {
        let dir = LargeUntrackedDir {
            path: PathBuf::from("models"),
            file_count: 500,
        };
        assert_eq!(dir.path, PathBuf::from("models"));
        assert_eq!(dir.file_count, 500);
    }

    #[test]
    fn test_ghost_snapshot_report_format_warning_empty() {
        let report = GhostSnapshotReport::default();
        assert!(report.format_large_untracked_warning().is_none());
    }

    #[test]
    fn test_ghost_snapshot_report_format_warning_single() {
        let report = GhostSnapshotReport {
            large_untracked_dirs: vec![LargeUntrackedDir {
                path: PathBuf::from("models"),
                file_count: 300,
            }],
        };
        let warning = report.format_large_untracked_warning().unwrap();
        assert!(warning.contains("models"));
        assert!(warning.contains("300 files"));
    }

    #[test]
    fn test_ghost_snapshot_report_format_warning_multiple() {
        let report = GhostSnapshotReport {
            large_untracked_dirs: vec![
                LargeUntrackedDir {
                    path: PathBuf::from("models"),
                    file_count: 500,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("data"),
                    file_count: 300,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("cache"),
                    file_count: 250,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("logs"),
                    file_count: 200,
                },
            ],
        };
        let warning = report.format_large_untracked_warning().unwrap();
        assert!(warning.contains("models"));
        assert!(warning.contains("1 more"));
    }

    #[test]
    fn test_create_ghost_commit_options() {
        let path = PathBuf::from("/tmp/test");
        let options = CreateGhostCommitOptions::new(&path)
            .message("test snapshot")
            .push_force_include("file.txt")
            .force_include(vec![PathBuf::from("dir/")]);

        assert_eq!(options.message, Some("test snapshot"));
        assert_eq!(options.force_include.len(), 1);
    }

    #[test]
    fn test_normalize_relative_path() {
        assert_eq!(
            normalize_relative_path(Path::new("a/b/c")).unwrap(),
            PathBuf::from("a/b/c")
        );
        assert_eq!(
            normalize_relative_path(Path::new("./a/b")).unwrap(),
            PathBuf::from("a/b")
        );
        assert_eq!(
            normalize_relative_path(Path::new("a/b/../c")).unwrap(),
            PathBuf::from("a/c")
        );
    }

    #[test]
    fn test_normalize_relative_path_escapes() {
        let result = normalize_relative_path(Path::new("../outside.txt"));
        assert!(matches!(
            result,
            Err(GhostCommitError::PathEscapesRepository { .. })
        ));
    }

    #[test]
    fn test_should_ignore_for_snapshot() {
        assert!(should_ignore_for_snapshot(Path::new("node_modules/pkg")));
        assert!(should_ignore_for_snapshot(Path::new(".venv/lib/python")));
        assert!(should_ignore_for_snapshot(Path::new("target/debug")));
        assert!(!should_ignore_for_snapshot(Path::new("src/main.rs")));
    }

    #[test]
    fn test_detect_large_untracked_dirs_empty() {
        let files: Vec<PathBuf> = Vec::new();
        let dirs: Vec<PathBuf> = Vec::new();
        let result = detect_large_untracked_dirs(&files, &dirs);
        assert!(result.is_empty());
    }

    #[test]
    fn test_create_and_restore_roundtrip() -> Result<(), GhostCommitError> {
        let temp = TempDir::new()?;
        let repo = temp.path();
        init_test_repo(repo);

        // Create initial commit
        fs::write(repo.join("tracked.txt"), "initial\n")?;
        run_git_in(repo, &["add", "tracked.txt"]);
        run_git_in(repo, &["commit", "-m", "init"]);

        // Modify files
        fs::write(repo.join("tracked.txt"), "modified\n")?;
        fs::write(repo.join("new-file.txt"), "new content\n")?;

        // Create ghost commit
        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;
        assert!(ghost.parent().is_some());

        // Modify more
        fs::write(repo.join("tracked.txt"), "other state\n")?;
        fs::remove_file(repo.join("new-file.txt"))?;
        fs::write(repo.join("ephemeral.txt"), "temp data\n")?;

        // Restore
        restore_ghost_commit(repo, &ghost)?;

        // Verify
        let tracked_after = fs::read_to_string(repo.join("tracked.txt"))?;
        assert_eq!(tracked_after, "modified\n");
        let new_file_after = fs::read_to_string(repo.join("new-file.txt"))?;
        assert_eq!(new_file_after, "new content\n");
        assert!(!repo.join("ephemeral.txt").exists());

        Ok(())
    }

    #[test]
    fn test_create_snapshot_without_head() -> Result<(), GhostCommitError> {
        let temp = TempDir::new()?;
        let repo = temp.path();
        init_test_repo(repo);

        // Create file without committing
        fs::write(repo.join("tracked.txt"), "first contents\n")?;

        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo))?;
        assert!(ghost.parent().is_none());

        let message = run_git_stdout(repo, &["log", "-1", "--format=%s", ghost.id()]);
        assert_eq!(message, DEFAULT_COMMIT_MESSAGE);

        Ok(())
    }

    #[test]
    fn test_create_ghost_commit_custom_message() -> Result<(), GhostCommitError> {
        let temp = TempDir::new()?;
        let repo = temp.path();
        init_test_repo(repo);

        fs::write(repo.join("tracked.txt"), "contents\n")?;
        run_git_in(repo, &["add", "tracked.txt"]);
        run_git_in(repo, &["commit", "-m", "initial"]);

        let message = "custom message";
        let ghost = create_ghost_commit(&CreateGhostCommitOptions::new(repo).message(message))?;
        let commit_message = run_git_stdout(repo, &["log", "-1", "--format=%s", ghost.id()]);
        assert_eq!(commit_message, message);

        Ok(())
    }

    #[test]
    fn test_not_a_git_repository() {
        let temp = TempDir::new().expect("tempdir");
        let result = create_ghost_commit(&CreateGhostCommitOptions::new(temp.path()));
        assert!(matches!(
            result,
            Err(GhostCommitError::NotAGitRepository { .. })
        ));
    }

    #[test]
    fn test_restore_requires_git_repository() {
        let temp = TempDir::new().expect("tempdir");
        let err = restore_to_commit(temp.path(), "deadbeef").unwrap_err();
        assert!(matches!(err, GhostCommitError::NotAGitRepository { .. }));
    }

    #[test]
    fn test_force_include_parent_path_rejected() {
        let temp = TempDir::new().expect("tempdir");
        let repo = temp.path();
        init_test_repo(repo);

        let options = CreateGhostCommitOptions::new(repo)
            .force_include(vec![PathBuf::from("../outside.txt")]);
        let err = create_ghost_commit(&options).unwrap_err();
        assert!(matches!(
            err,
            GhostCommitError::PathEscapesRepository { .. }
        ));
    }

    #[test]
    fn test_ghost_commit_error_display() {
        let err = GhostCommitError::NotAGitRepository {
            path: PathBuf::from("/tmp/test"),
        };
        assert!(err.to_string().contains("not a git repository"));

        let err = GhostCommitError::GitCommand {
            command: "git status".to_string(),
            status: 1,
            stderr: "error".to_string(),
        };
        assert!(err.to_string().contains("git status"));
    }

    #[test]
    fn test_ghost_commit_clone() {
        let commit = GhostCommit::new(
            "abc123".to_string(),
            Some("def456".to_string()),
            vec![PathBuf::from("file.txt")],
            vec![PathBuf::from("dir")],
        );
        let cloned = commit.clone();
        assert_eq!(commit, cloned);
        assert_eq!(commit.id(), cloned.id());
        assert_eq!(commit.parent(), cloned.parent());
    }

    #[test]
    fn test_ghost_commit_error_git_output_utf8() {
        let err = GhostCommitError::GitOutputUtf8 {
            command: "git log".to_string(),
        };
        assert!(err.to_string().contains("non-UTF-8"));
        assert!(err.to_string().contains("git log"));
    }

    #[test]
    fn test_ghost_commit_error_non_relative_path() {
        let err = GhostCommitError::NonRelativePath {
            path: PathBuf::from("/absolute/path"),
        };
        assert!(err.to_string().contains("relative"));
    }

    #[test]
    fn test_ghost_commit_error_path_escapes() {
        let err = GhostCommitError::PathEscapesRepository {
            path: PathBuf::from("../outside"),
        };
        assert!(err.to_string().contains("escapes"));
    }

    #[test]
    fn test_ghost_commit_error_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = GhostCommitError::Io(io_err);
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_ghost_snapshot_report_clone() {
        let report = GhostSnapshotReport {
            large_untracked_dirs: vec![LargeUntrackedDir {
                path: PathBuf::from("test"),
                file_count: 100,
            }],
        };
        let cloned = report.clone();
        assert_eq!(report, cloned);
    }

    #[test]
    fn test_large_untracked_dir_clone() {
        let dir = LargeUntrackedDir {
            path: PathBuf::from("models"),
            file_count: 500,
        };
        let cloned = dir.clone();
        assert_eq!(dir, cloned);
        assert_eq!(dir.path, cloned.path);
        assert_eq!(dir.file_count, cloned.file_count);
    }

    #[test]
    fn test_large_untracked_dir_eq() {
        let dir1 = LargeUntrackedDir {
            path: PathBuf::from("models"),
            file_count: 500,
        };
        let dir2 = LargeUntrackedDir {
            path: PathBuf::from("models"),
            file_count: 500,
        };
        let dir3 = LargeUntrackedDir {
            path: PathBuf::from("different"),
            file_count: 500,
        };
        assert_eq!(dir1, dir2);
        assert_ne!(dir1, dir3);
    }

    #[test]
    fn test_build_command_string() {
        assert_eq!(build_command_string(&[]), "git");

        let args = vec![OsString::from("status"), OsString::from("--porcelain")];
        assert_eq!(build_command_string(&args), "git status --porcelain");
    }

    #[test]
    fn test_normalize_relative_path_absolute() {
        let result = normalize_relative_path(Path::new("/absolute/path"));
        assert!(matches!(
            result,
            Err(GhostCommitError::NonRelativePath { .. })
        ));
    }

    #[test]
    fn test_normalize_relative_path_empty() {
        let result = normalize_relative_path(Path::new(""));
        assert!(matches!(
            result,
            Err(GhostCommitError::NonRelativePath { .. })
        ));
    }

    #[test]
    fn test_normalize_relative_path_curdir_only() {
        // "." alone results in empty path after normalization, which is still an error
        // Actually the current implementation returns Ok(PathBuf::new()) for "."
        // since CurDir is just skipped. Let me check actual behavior.
        let result = normalize_relative_path(Path::new("."));
        // The function requires at least one Normal component
        // "." by itself has saw_component=true but result is empty
        // Since the function errors if !saw_component but "." does set saw_component,
        // it actually returns Ok(empty path)
        assert!(result.is_ok());
        assert!(result.unwrap().as_os_str().is_empty());
    }

    #[test]
    fn test_normalize_relative_path_complex() {
        let result = normalize_relative_path(Path::new("a/./b/../c/./d")).unwrap();
        assert_eq!(result, PathBuf::from("a/c/d"));
    }

    #[test]
    fn test_detect_large_untracked_dirs_below_threshold() {
        // Create files under dirs but below threshold (200)
        let files: Vec<PathBuf> = (0..100)
            .map(|i| PathBuf::from(format!("small/file{}.txt", i)))
            .collect();
        let dirs = vec![PathBuf::from("small")];
        let result = detect_large_untracked_dirs(&files, &dirs);
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_large_untracked_dirs_above_threshold() {
        // Create files over threshold (200)
        let files: Vec<PathBuf> = (0..250)
            .map(|i| PathBuf::from(format!("large/file{}.txt", i)))
            .collect();
        let dirs = vec![PathBuf::from("large")];
        let result = detect_large_untracked_dirs(&files, &dirs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, PathBuf::from("large"));
        assert_eq!(result[0].file_count, 250);
    }

    #[test]
    fn test_detect_large_untracked_dirs_multiple_dirs() {
        // Create files in multiple dirs
        let mut files: Vec<PathBuf> = Vec::new();
        for i in 0..300 {
            files.push(PathBuf::from(format!("big/file{}.txt", i)));
        }
        for i in 0..100 {
            files.push(PathBuf::from(format!("small/file{}.txt", i)));
        }
        let dirs = vec![PathBuf::from("big"), PathBuf::from("small")];
        let result = detect_large_untracked_dirs(&files, &dirs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, PathBuf::from("big"));
    }

    #[test]
    fn test_should_preserve() {
        let preserved_files: HashSet<PathBuf> =
            vec![PathBuf::from("file1.txt"), PathBuf::from("file2.txt")]
                .into_iter()
                .collect();
        let preserved_dirs = vec![PathBuf::from("dir1"), PathBuf::from("dir2")];

        // File in preserved set
        assert!(should_preserve(
            Path::new("file1.txt"),
            &preserved_files,
            &preserved_dirs
        ));

        // File not in preserved set
        assert!(!should_preserve(
            Path::new("other.txt"),
            &preserved_files,
            &preserved_dirs
        ));

        // File under preserved dir
        assert!(should_preserve(
            Path::new("dir1/subfile.txt"),
            &preserved_files,
            &preserved_dirs
        ));

        // File under non-preserved dir
        assert!(!should_preserve(
            Path::new("dir3/file.txt"),
            &preserved_files,
            &preserved_dirs
        ));
    }

    #[test]
    fn test_remove_path_nonexistent() {
        let result = remove_path(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_path_file() -> Result<(), GhostCommitError> {
        let temp = TempDir::new()?;
        let file_path = temp.path().join("test_file.txt");
        fs::write(&file_path, "content")?;
        assert!(file_path.exists());

        remove_path(&file_path)?;
        assert!(!file_path.exists());
        Ok(())
    }

    #[test]
    fn test_remove_path_directory() -> Result<(), GhostCommitError> {
        let temp = TempDir::new()?;
        let dir_path = temp.path().join("test_dir");
        fs::create_dir(&dir_path)?;
        fs::write(dir_path.join("file.txt"), "content")?;
        assert!(dir_path.exists());

        remove_path(&dir_path)?;
        assert!(!dir_path.exists());
        Ok(())
    }

    #[test]
    fn test_create_ghost_commit_options_new_only() {
        let path = PathBuf::from("/tmp/test");
        let options = CreateGhostCommitOptions::new(&path);

        assert_eq!(options.repo_path, Path::new("/tmp/test"));
        assert!(options.message.is_none());
        assert!(options.force_include.is_empty());
    }

    #[test]
    fn test_to_session_relative_path() {
        // Without prefix
        let path = PathBuf::from("src/main.rs");
        let result = to_session_relative_path(&path, None);
        assert_eq!(result, path);

        // With prefix
        let result =
            to_session_relative_path(Path::new("project/src/main.rs"), Some(Path::new("project")));
        assert_eq!(result, PathBuf::from("src/main.rs"));

        // Path not under prefix (shouldn't happen but should handle)
        let result =
            to_session_relative_path(Path::new("other/file.rs"), Some(Path::new("project")));
        assert_eq!(result, PathBuf::from("other/file.rs"));
    }

    #[test]
    fn test_non_empty_path() {
        assert!(non_empty_path(Path::new("")).is_none());
        assert_eq!(
            non_empty_path(Path::new("test")),
            Some(PathBuf::from("test"))
        );
    }

    #[test]
    fn test_apply_repo_prefix_to_force_include() {
        // Empty paths
        let result = apply_repo_prefix_to_force_include(Some(Path::new("prefix")), &[]);
        assert!(result.is_empty());

        // No prefix
        let paths = vec![PathBuf::from("file1"), PathBuf::from("file2")];
        let result = apply_repo_prefix_to_force_include(None, &paths);
        assert_eq!(result, paths);

        // With prefix
        let result = apply_repo_prefix_to_force_include(Some(Path::new("prefix")), &paths);
        assert_eq!(result[0], PathBuf::from("prefix/file1"));
        assert_eq!(result[1], PathBuf::from("prefix/file2"));
    }

    #[test]
    fn test_default_commit_identity() {
        let identity = default_commit_identity();
        assert_eq!(identity.len(), 4);

        let names: Vec<_> = identity.iter().map(|(k, _)| k.to_str().unwrap()).collect();
        assert!(names.contains(&"GIT_AUTHOR_NAME"));
        assert!(names.contains(&"GIT_AUTHOR_EMAIL"));
        assert!(names.contains(&"GIT_COMMITTER_NAME"));
        assert!(names.contains(&"GIT_COMMITTER_EMAIL"));
    }

    #[test]
    fn test_ghost_commit_debug() {
        let commit = GhostCommit::new("abc".to_string(), None, vec![], vec![]);
        let debug_str = format!("{:?}", commit);
        assert!(debug_str.contains("GhostCommit"));
        assert!(debug_str.contains("abc"));
    }

    #[test]
    fn test_ghost_commit_error_debug() {
        let err = GhostCommitError::NotAGitRepository {
            path: PathBuf::from("/test"),
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotAGitRepository"));
    }

    #[test]
    fn test_ghost_snapshot_report_default() {
        let report = GhostSnapshotReport::default();
        assert!(report.large_untracked_dirs.is_empty());
    }

    #[test]
    fn test_large_untracked_dir_debug() {
        let dir = LargeUntrackedDir {
            path: PathBuf::from("test"),
            file_count: 100,
        };
        let debug_str = format!("{:?}", dir);
        assert!(debug_str.contains("LargeUntrackedDir"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_ghost_snapshot_report_debug() {
        let report = GhostSnapshotReport::default();
        let debug_str = format!("{:?}", report);
        assert!(debug_str.contains("GhostSnapshotReport"));
    }

    // ============================================================================
    // Additional test coverage (N=286)
    // ============================================================================

    // GhostCommit comprehensive tests

    #[test]
    fn test_ghost_commit_empty_fields() {
        let commit = GhostCommit::new("".to_string(), None, vec![], vec![]);
        assert_eq!(commit.id(), "");
        assert!(commit.parent().is_none());
        assert!(commit.preexisting_untracked_files().is_empty());
        assert!(commit.preexisting_untracked_dirs().is_empty());
    }

    #[test]
    fn test_ghost_commit_with_many_untracked_files() {
        let files: Vec<PathBuf> = (0..100)
            .map(|i| PathBuf::from(format!("file{}.txt", i)))
            .collect();
        let commit = GhostCommit::new("abc".to_string(), None, files.clone(), vec![]);
        assert_eq!(commit.preexisting_untracked_files().len(), 100);
    }

    #[test]
    fn test_ghost_commit_with_many_untracked_dirs() {
        let dirs: Vec<PathBuf> = (0..50)
            .map(|i| PathBuf::from(format!("dir{}", i)))
            .collect();
        let commit = GhostCommit::new("def".to_string(), None, vec![], dirs.clone());
        assert_eq!(commit.preexisting_untracked_dirs().len(), 50);
    }

    #[test]
    fn test_ghost_commit_parent_some_vs_none() {
        let with_parent = GhostCommit::new(
            "abc".to_string(),
            Some("parent-sha".to_string()),
            vec![],
            vec![],
        );
        let without_parent = GhostCommit::new("abc".to_string(), None, vec![], vec![]);

        assert_eq!(with_parent.parent(), Some("parent-sha"));
        assert!(without_parent.parent().is_none());
    }

    #[test]
    fn test_ghost_commit_serialization_with_all_fields() {
        let commit = GhostCommit::new(
            "sha256abc123".to_string(),
            Some("parent_sha".to_string()),
            vec![
                PathBuf::from("untracked1.txt"),
                PathBuf::from("untracked2.txt"),
            ],
            vec![PathBuf::from("untracked_dir")],
        );
        let json = serde_json::to_string(&commit).unwrap();
        let parsed: GhostCommit = serde_json::from_str(&json).unwrap();
        assert_eq!(commit, parsed);
    }

    #[test]
    fn test_ghost_commit_display_long_id() {
        let long_id = "a".repeat(100);
        let commit = GhostCommit::new(long_id.clone(), None, vec![], vec![]);
        assert_eq!(format!("{}", commit), long_id);
    }

    // LargeUntrackedDir comprehensive tests

    #[test]
    fn test_large_untracked_dir_zero_files() {
        let dir = LargeUntrackedDir {
            path: PathBuf::from("empty"),
            file_count: 0,
        };
        assert_eq!(dir.file_count, 0);
    }

    #[test]
    fn test_large_untracked_dir_max_usize() {
        let dir = LargeUntrackedDir {
            path: PathBuf::from("huge"),
            file_count: usize::MAX,
        };
        assert_eq!(dir.file_count, usize::MAX);
    }

    #[test]
    fn test_large_untracked_dir_special_path_chars() {
        let dir = LargeUntrackedDir {
            path: PathBuf::from("path with spaces/and-dashes/and_underscores"),
            file_count: 100,
        };
        assert!(dir.path.to_string_lossy().contains("spaces"));
    }

    // GhostSnapshotReport format_large_untracked_warning tests

    #[test]
    fn test_ghost_snapshot_report_format_warning_exactly_3_dirs() {
        let report = GhostSnapshotReport {
            large_untracked_dirs: vec![
                LargeUntrackedDir {
                    path: PathBuf::from("dir1"),
                    file_count: 200,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("dir2"),
                    file_count: 250,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("dir3"),
                    file_count: 300,
                },
            ],
        };
        let warning = report.format_large_untracked_warning().unwrap();
        assert!(warning.contains("dir1"));
        assert!(warning.contains("dir2"));
        assert!(warning.contains("dir3"));
        assert!(!warning.contains("more"));
    }

    #[test]
    fn test_ghost_snapshot_report_format_warning_more_than_3_dirs() {
        let report = GhostSnapshotReport {
            large_untracked_dirs: vec![
                LargeUntrackedDir {
                    path: PathBuf::from("d1"),
                    file_count: 500,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("d2"),
                    file_count: 400,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("d3"),
                    file_count: 300,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("d4"),
                    file_count: 200,
                },
                LargeUntrackedDir {
                    path: PathBuf::from("d5"),
                    file_count: 200,
                },
            ],
        };
        let warning = report.format_large_untracked_warning().unwrap();
        assert!(warning.contains("2 more"));
    }

    // CreateGhostCommitOptions tests

    #[test]
    fn test_create_ghost_commit_options_chain_all() {
        let path = PathBuf::from("/some/path");
        let options = CreateGhostCommitOptions::new(&path)
            .message("custom msg")
            .push_force_include("file1.txt")
            .push_force_include("file2.txt")
            .force_include(vec![PathBuf::from("replaced.txt")]);

        assert_eq!(options.message, Some("custom msg"));
        // force_include replaces all previous, so only 1 item
        assert_eq!(options.force_include.len(), 1);
        assert_eq!(options.force_include[0], PathBuf::from("replaced.txt"));
    }

    #[test]
    fn test_create_ghost_commit_options_empty_message() {
        let path = PathBuf::from("/test");
        let options = CreateGhostCommitOptions::new(&path).message("");
        assert_eq!(options.message, Some(""));
    }

    // normalize_relative_path edge cases

    #[test]
    fn test_normalize_relative_path_multiple_parent_dirs() {
        // a/b/c/../../../d -> Error (escapes)
        let result = normalize_relative_path(Path::new("a/b/c/../../../d"));
        // This should be OK: a/b/c -> a/b -> a -> d
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("d"));
    }

    #[test]
    fn test_normalize_relative_path_deep_nesting() {
        let result = normalize_relative_path(Path::new("a/b/c/d/e/f/../../../../../..")).unwrap();
        // a/b/c/d/e/f -> a/b/c/d/e -> a/b/c/d -> a/b/c -> a/b -> a -> (empty)
        assert!(result.as_os_str().is_empty());
    }

    #[test]
    fn test_normalize_relative_path_only_curdir() {
        let result = normalize_relative_path(Path::new("./././."));
        assert!(result.is_ok());
        assert!(result.unwrap().as_os_str().is_empty());
    }

    // should_ignore_for_snapshot tests

    #[test]
    fn test_should_ignore_for_snapshot_all_default_dirs() {
        let ignored_dirs = [
            "node_modules",
            ".venv",
            "venv",
            "env",
            ".env",
            "dist",
            "build",
            ".pytest_cache",
            ".mypy_cache",
            ".cache",
            ".tox",
            "__pycache__",
            "target",
        ];

        for dir in ignored_dirs {
            let path = Path::new(dir);
            assert!(
                should_ignore_for_snapshot(path),
                "Expected {} to be ignored",
                dir
            );
        }
    }

    #[test]
    fn test_should_ignore_for_snapshot_nested_ignored() {
        assert!(should_ignore_for_snapshot(Path::new(
            "src/node_modules/pkg"
        )));
        assert!(should_ignore_for_snapshot(Path::new(
            "project/.venv/lib/python"
        )));
        assert!(should_ignore_for_snapshot(Path::new(
            "rust/target/debug/bin"
        )));
    }

    #[test]
    fn test_should_ignore_for_snapshot_similar_names() {
        // These should NOT be ignored (similar but not exact)
        assert!(!should_ignore_for_snapshot(Path::new("node_module"))); // missing 's'
        assert!(!should_ignore_for_snapshot(Path::new("targets"))); // extra 's'
        assert!(!should_ignore_for_snapshot(Path::new("my_env"))); // not exactly "env"
    }

    // detect_large_untracked_dirs tests

    #[test]
    fn test_detect_large_untracked_dirs_exactly_at_threshold() {
        // LARGE_UNTRACKED_WARNING_THRESHOLD is 200
        let files: Vec<PathBuf> = (0..200)
            .map(|i| PathBuf::from(format!("exact/file{}.txt", i)))
            .collect();
        let dirs = vec![PathBuf::from("exact")];
        let result = detect_large_untracked_dirs(&files, &dirs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_count, 200);
    }

    #[test]
    fn test_detect_large_untracked_dirs_just_below_threshold() {
        let files: Vec<PathBuf> = (0..199)
            .map(|i| PathBuf::from(format!("below/file{}.txt", i)))
            .collect();
        let dirs = vec![PathBuf::from("below")];
        let result = detect_large_untracked_dirs(&files, &dirs);
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_large_untracked_dirs_files_without_dir() {
        // Files not in any tracked directory - should use parent
        let files = vec![PathBuf::from("orphan1.txt"), PathBuf::from("orphan2.txt")];
        let dirs: Vec<PathBuf> = vec![];
        let result = detect_large_untracked_dirs(&files, &dirs);
        // With only 2 files, won't meet threshold
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_large_untracked_dirs_sorting() {
        // Result should be sorted by file_count descending
        let mut files: Vec<PathBuf> = Vec::new();
        for i in 0..300 {
            files.push(PathBuf::from(format!("big/file{}.txt", i)));
        }
        for i in 0..250 {
            files.push(PathBuf::from(format!("medium/file{}.txt", i)));
        }
        for i in 0..200 {
            files.push(PathBuf::from(format!("small/file{}.txt", i)));
        }

        let dirs = vec![
            PathBuf::from("big"),
            PathBuf::from("medium"),
            PathBuf::from("small"),
        ];
        let result = detect_large_untracked_dirs(&files, &dirs);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].file_count, 300);
        assert_eq!(result[1].file_count, 250);
        assert_eq!(result[2].file_count, 200);
    }

    // build_command_string tests

    #[test]
    fn test_build_command_string_single_arg() {
        let args = vec![OsString::from("status")];
        assert_eq!(build_command_string(&args), "git status");
    }

    #[test]
    fn test_build_command_string_with_special_chars() {
        let args = vec![
            OsString::from("commit"),
            OsString::from("-m"),
            OsString::from("fix: handle edge cases"),
        ];
        let result = build_command_string(&args);
        assert!(result.contains("fix: handle edge cases"));
    }

    // to_session_relative_path tests

    #[test]
    fn test_to_session_relative_path_no_prefix() {
        let path = PathBuf::from("src/main.rs");
        let result = to_session_relative_path(&path, None);
        assert_eq!(result, path);
    }

    #[test]
    fn test_to_session_relative_path_with_matching_prefix() {
        let result =
            to_session_relative_path(Path::new("project/src/main.rs"), Some(Path::new("project")));
        assert_eq!(result, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_to_session_relative_path_prefix_not_matching() {
        let result =
            to_session_relative_path(Path::new("other/file.rs"), Some(Path::new("project")));
        // If prefix doesn't match, returns original
        assert_eq!(result, PathBuf::from("other/file.rs"));
    }

    // apply_repo_prefix_to_force_include tests

    #[test]
    fn test_apply_repo_prefix_to_force_include_with_prefix() {
        let paths = vec![PathBuf::from("a.txt"), PathBuf::from("b/c.txt")];
        let result = apply_repo_prefix_to_force_include(Some(Path::new("repo")), &paths);
        assert_eq!(result[0], PathBuf::from("repo/a.txt"));
        assert_eq!(result[1], PathBuf::from("repo/b/c.txt"));
    }

    #[test]
    fn test_apply_repo_prefix_to_force_include_empty_prefix() {
        let paths = vec![PathBuf::from("file.txt")];
        let result = apply_repo_prefix_to_force_include(Some(Path::new("")), &paths);
        // Empty prefix still joins
        assert_eq!(result[0], PathBuf::from("file.txt"));
    }

    // should_preserve tests

    #[test]
    fn test_should_preserve_file_in_set() {
        let preserved_files: HashSet<PathBuf> =
            vec![PathBuf::from("important.txt")].into_iter().collect();
        let preserved_dirs = vec![];

        assert!(should_preserve(
            Path::new("important.txt"),
            &preserved_files,
            &preserved_dirs
        ));
    }

    #[test]
    fn test_should_preserve_file_under_dir() {
        let preserved_files: HashSet<PathBuf> = HashSet::new();
        let preserved_dirs = vec![PathBuf::from("preserved_dir")];

        assert!(should_preserve(
            Path::new("preserved_dir/subdir/file.txt"),
            &preserved_files,
            &preserved_dirs
        ));
    }

    #[test]
    fn test_should_preserve_false_for_unrelated() {
        let preserved_files: HashSet<PathBuf> =
            vec![PathBuf::from("other.txt")].into_iter().collect();
        let preserved_dirs = vec![PathBuf::from("other_dir")];

        assert!(!should_preserve(
            Path::new("unrelated/file.txt"),
            &preserved_files,
            &preserved_dirs
        ));
    }

    // GhostCommitError tests

    #[test]
    fn test_ghost_commit_error_git_command_display() {
        let err = GhostCommitError::GitCommand {
            command: "git status --porcelain".to_string(),
            status: 128,
            stderr: "fatal: not a git repository".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("git status --porcelain"));
        assert!(msg.contains("128"));
        assert!(msg.contains("fatal: not a git repository"));
    }

    #[test]
    fn test_ghost_commit_error_git_output_utf8_display() {
        let err = GhostCommitError::GitOutputUtf8 {
            command: "git log".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("git log"));
        assert!(msg.contains("non-UTF-8"));
    }

    #[test]
    fn test_ghost_commit_error_not_a_git_repository_display() {
        let err = GhostCommitError::NotAGitRepository {
            path: PathBuf::from("/home/user/project"),
        };
        let msg = err.to_string();
        assert!(msg.contains("not a git repository"));
        assert!(msg.contains("/home/user/project"));
    }

    #[test]
    fn test_ghost_commit_error_non_relative_path_display() {
        let err = GhostCommitError::NonRelativePath {
            path: PathBuf::from("/absolute/path"),
        };
        let msg = err.to_string();
        assert!(msg.contains("relative"));
    }

    #[test]
    fn test_ghost_commit_error_path_escapes_display() {
        let err = GhostCommitError::PathEscapesRepository {
            path: PathBuf::from("../../../escape"),
        };
        let msg = err.to_string();
        assert!(msg.contains("escapes"));
    }

    // Default commit identity tests

    #[test]
    fn test_default_commit_identity_values() {
        let identity = default_commit_identity();
        let author_name = identity
            .iter()
            .find(|(k, _)| k == "GIT_AUTHOR_NAME")
            .map(|(_, v)| v.to_str().unwrap());
        assert_eq!(author_name, Some("Codex Snapshot"));

        let author_email = identity
            .iter()
            .find(|(k, _)| k == "GIT_AUTHOR_EMAIL")
            .map(|(_, v)| v.to_str().unwrap());
        assert_eq!(author_email, Some("snapshot@codex.local"));
    }

    // non_empty_path tests

    #[test]
    fn test_non_empty_path_empty_string() {
        assert!(non_empty_path(Path::new("")).is_none());
    }

    #[test]
    fn test_non_empty_path_single_char() {
        assert_eq!(non_empty_path(Path::new("a")), Some(PathBuf::from("a")));
    }

    #[test]
    fn test_non_empty_path_complex_path() {
        assert_eq!(
            non_empty_path(Path::new("a/b/c")),
            Some(PathBuf::from("a/b/c"))
        );
    }

    // DEFAULT_IGNORED_DIR_NAMES coverage

    #[test]
    fn test_default_ignored_dir_names_count() {
        // Verify we have the expected number of ignored directories
        assert_eq!(DEFAULT_IGNORED_DIR_NAMES.len(), 13);
    }

    // GhostCommit equality and ordering

    #[test]
    fn test_ghost_commit_eq_reflexive() {
        let commit = GhostCommit::new(
            "abc".to_string(),
            Some("def".to_string()),
            vec![PathBuf::from("f.txt")],
            vec![],
        );
        assert_eq!(commit, commit.clone());
    }

    #[test]
    fn test_ghost_commit_ne_different_id() {
        let c1 = GhostCommit::new("abc".to_string(), None, vec![], vec![]);
        let c2 = GhostCommit::new("xyz".to_string(), None, vec![], vec![]);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_ghost_commit_ne_different_parent() {
        let c1 = GhostCommit::new("abc".to_string(), Some("p1".to_string()), vec![], vec![]);
        let c2 = GhostCommit::new("abc".to_string(), Some("p2".to_string()), vec![], vec![]);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_ghost_commit_ne_different_files() {
        let c1 = GhostCommit::new(
            "abc".to_string(),
            None,
            vec![PathBuf::from("file1.txt")],
            vec![],
        );
        let c2 = GhostCommit::new(
            "abc".to_string(),
            None,
            vec![PathBuf::from("file2.txt")],
            vec![],
        );
        assert_ne!(c1, c2);
    }

    // UntrackedSnapshot related (via capture functions)

    #[test]
    fn test_untracked_snapshot_default() {
        let snapshot = UntrackedSnapshot::default();
        assert!(snapshot.files.is_empty());
        assert!(snapshot.dirs.is_empty());
    }
}
