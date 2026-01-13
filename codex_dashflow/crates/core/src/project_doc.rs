//! Project-level documentation discovery.
//!
//! Project-level documentation is primarily stored in files named `AGENTS.md`.
//! Additional fallback filenames can be configured via `project_doc_fallback_filenames`.
//! We include the concatenation of all files found along the path from the
//! repository root to the current working directory as follows:
//!
//! 1.  Determine the Git repository root by walking upwards from the current
//!     working directory until a `.git` directory or file is found. If no Git
//!     root is found, only the current working directory is considered.
//! 2.  Collect every `AGENTS.md` found from the repository root down to the
//!     current working directory (inclusive) and concatenate their contents in
//!     that order.
//! 3.  We do **not** walk past the Git root.
//!
//! This module uses DashFlow's `discover_to_root()` API for path discovery.

use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tracing::{debug, error, info, warn};

/// Default filename scanned for project-level docs.
pub const DEFAULT_PROJECT_DOC_FILENAME: &str = "AGENTS.md";
/// Preferred local override for project-level docs.
pub const LOCAL_PROJECT_DOC_FILENAME: &str = "AGENTS.override.md";

/// Default maximum bytes to read from project documentation
pub const DEFAULT_PROJECT_DOC_MAX_BYTES: usize = 32 * 1024; // 32KB

/// Options for project documentation discovery
#[derive(Debug, Clone)]
pub struct ProjectDocOptions {
    /// Working directory to start search from
    pub cwd: PathBuf,
    /// Maximum bytes to include from project docs (0 = disabled)
    pub max_bytes: usize,
    /// Additional fallback filenames to search for
    pub fallback_filenames: Vec<String>,
    /// Optional user-provided instructions to prepend
    pub user_instructions: Option<String>,
}

impl Default for ProjectDocOptions {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            max_bytes: DEFAULT_PROJECT_DOC_MAX_BYTES,
            fallback_filenames: Vec::new(),
            user_instructions: None,
        }
    }
}

impl ProjectDocOptions {
    /// Create options for a specific working directory
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            ..Default::default()
        }
    }

    /// Set the maximum bytes to read
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Add a fallback filename
    pub fn with_fallback(mut self, filename: impl Into<String>) -> Self {
        self.fallback_filenames.push(filename.into());
        self
    }

    /// Set user instructions to prepend
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.user_instructions = Some(instructions.into());
        self
    }
}

/// When both user instructions and project doc are present, they will
/// be concatenated with the following separator.
const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

/// Get combined user instructions and project documentation.
///
/// Combines `user_instructions` (if provided) and discovered AGENTS.md files
/// into a single string that can be used as additional context for the LLM.
pub async fn get_user_instructions(options: &ProjectDocOptions) -> Option<String> {
    let project_docs = match read_project_docs(options).await {
        Ok(docs) => docs,
        Err(e) => {
            error!("error trying to find project doc: {e:#}");
            return options.user_instructions.clone();
        }
    };

    let mut parts: Vec<String> = Vec::new();

    if let Some(instructions) = options.user_instructions.clone() {
        parts.push(instructions);
    }

    if let Some(project_doc) = project_docs {
        if !parts.is_empty() {
            parts.push(PROJECT_DOC_SEPARATOR.to_string());
        }
        parts.push(project_doc);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.concat())
    }
}

/// Attempt to locate and load the project documentation.
///
/// On success returns `Ok(Some(contents))` where `contents` is the
/// concatenation of all discovered docs. If no documentation file is found the
/// function returns `Ok(None)`. Unexpected I/O failures bubble up as `Err` so
/// callers can decide how to handle them.
pub async fn read_project_docs(options: &ProjectDocOptions) -> std::io::Result<Option<String>> {
    let max_total = options.max_bytes;

    if max_total == 0 {
        return Ok(None);
    }

    let paths = discover_project_doc_paths(options).await?;
    if paths.is_empty() {
        return Ok(None);
    }

    let mut remaining: u64 = max_total as u64;
    let mut parts: Vec<String> = Vec::new();

    for p in paths {
        if remaining == 0 {
            break;
        }

        let file = match tokio::fs::File::open(&p).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };

        let size = file.metadata().await?.len();
        let mut reader = tokio::io::BufReader::new(file).take(remaining);
        let mut data: Vec<u8> = Vec::new();
        reader.read_to_end(&mut data).await?;

        if size > remaining {
            warn!(
                "Project doc `{}` exceeds remaining budget ({} bytes) - truncating.",
                p.display(),
                remaining,
            );
        }

        let text = String::from_utf8_lossy(&data).to_string();
        if !text.trim().is_empty() {
            parts.push(text);
            remaining = remaining.saturating_sub(data.len() as u64);
        }
    }

    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("\n\n")))
    }
}

/// Discover the list of AGENTS.md files using DashFlow's `discover_to_root()` API.
///
/// The list is ordered from repository root to the current working
/// directory (inclusive). Symlinks are allowed. When `max_bytes` is zero,
/// returns an empty list.
///
/// This function uses DashFlow's `dashflow_project::ProjectContext::discover_to_root()`
/// to walk from the working directory up to the project root, collecting
/// documentation files along the way.
pub async fn discover_project_doc_paths(
    options: &ProjectDocOptions,
) -> std::io::Result<Vec<PathBuf>> {
    if options.max_bytes == 0 {
        return Ok(Vec::new());
    }

    // Canonicalize working directory
    let cwd = options
        .cwd
        .canonicalize()
        .unwrap_or_else(|_| options.cwd.clone());

    // Find git root for project boundary
    let root = find_git_root(&cwd).unwrap_or_else(|| cwd.clone());

    debug!(
        "Using DashFlow project discovery from root: {}",
        root.display()
    );

    // Use DashFlow's project discovery
    let project = match dashflow_project::discover_project(root.clone()).await {
        Ok(p) => p,
        Err(e) => {
            warn!("DashFlow project discovery failed: {e}, falling back to cwd only");
            // Fallback: just check cwd for doc files
            return Ok(discover_in_single_dir(&cwd, options));
        }
    };

    // Build ordered list of candidate filenames (override first, then default, then fallbacks)
    let mut filenames: Vec<&str> = vec![LOCAL_PROJECT_DOC_FILENAME, DEFAULT_PROJECT_DOC_FILENAME];
    for fb in &options.fallback_filenames {
        if !fb.is_empty() && !filenames.contains(&fb.as_str()) {
            filenames.push(fb.as_str());
        }
    }

    // Use DashFlow's discover_to_root to find files
    // We need to collect files while respecting priority (override > default > fallback)
    // and only pick one file per directory
    let mut result: Vec<PathBuf> = Vec::new();
    let mut seen_dirs = std::collections::HashSet::new();

    for filename in &filenames {
        // discover_to_root returns files from start (cwd) to root
        let found = project.discover_to_root(&cwd, filename);

        for path in found {
            if let Some(parent) = path.parent() {
                if !seen_dirs.contains(parent) {
                    seen_dirs.insert(parent.to_path_buf());
                    result.push(path);
                }
            }
        }
    }

    // Sort by path depth (root first, deeper directories later)
    // discover_to_root returns from cwd toward root, we want root-to-cwd order
    result.sort_by_key(|p| p.components().count());

    info!(
        "Found {} project doc files via DashFlow discover_to_root",
        result.len()
    );

    Ok(result)
}

/// Find the git root by walking upwards from the given directory.
///
/// Returns the path to the directory containing `.git`, or None if no git root is found.
fn find_git_root(start: &std::path::Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    if let Ok(canon) = std::fs::canonicalize(&current) {
        current = canon;
    }

    loop {
        let git_marker = current.join(".git");
        if git_marker.exists() {
            return Some(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

/// Fallback discovery for a single directory (when DashFlow discovery fails)
fn discover_in_single_dir(dir: &std::path::Path, options: &ProjectDocOptions) -> Vec<PathBuf> {
    let mut filenames: Vec<&str> = vec![LOCAL_PROJECT_DOC_FILENAME, DEFAULT_PROJECT_DOC_FILENAME];
    for fb in &options.fallback_filenames {
        if !fb.is_empty() && !filenames.contains(&fb.as_str()) {
            filenames.push(fb.as_str());
        }
    }

    for filename in filenames {
        let candidate = dir.join(filename);
        if candidate.exists() && candidate.is_file() {
            return vec![candidate];
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a temp dir with specific files
    fn setup_temp_dir() -> TempDir {
        tempfile::tempdir().expect("create tempdir")
    }

    #[test]
    fn test_project_doc_options_builder() {
        let options = ProjectDocOptions::new(PathBuf::from("/test"))
            .with_max_bytes(1024)
            .with_fallback("README.md")
            .with_instructions("Test instructions");

        assert_eq!(options.cwd, PathBuf::from("/test"));
        assert_eq!(options.max_bytes, 1024);
        assert_eq!(options.fallback_filenames, vec!["README.md".to_string()]);
        assert_eq!(
            options.user_instructions,
            Some("Test instructions".to_string())
        );
    }

    #[test]
    fn test_default_project_doc_filename_constant() {
        assert_eq!(DEFAULT_PROJECT_DOC_FILENAME, "AGENTS.md");
    }

    #[test]
    fn test_local_project_doc_filename_constant() {
        assert_eq!(LOCAL_PROJECT_DOC_FILENAME, "AGENTS.override.md");
    }

    #[test]
    fn test_project_doc_separator_constant() {
        assert!(PROJECT_DOC_SEPARATOR.contains("project-doc"));
    }

    #[test]
    fn test_project_doc_options_debug() {
        let options = ProjectDocOptions::default();
        let debug_str = format!("{:?}", options);
        assert!(debug_str.contains("ProjectDocOptions"));
    }

    #[test]
    fn test_project_doc_options_clone() {
        let options = ProjectDocOptions::new(PathBuf::from("/test"))
            .with_max_bytes(2048)
            .with_fallback("CLAUDE.md");
        let cloned = options.clone();
        assert_eq!(cloned.cwd, options.cwd);
        assert_eq!(cloned.max_bytes, options.max_bytes);
        assert_eq!(cloned.fallback_filenames, options.fallback_filenames);
    }

    #[test]
    fn test_project_doc_options_default_values() {
        let options = ProjectDocOptions::default();
        assert_eq!(options.max_bytes, DEFAULT_PROJECT_DOC_MAX_BYTES);
        assert!(options.fallback_filenames.is_empty());
        assert!(options.user_instructions.is_none());
    }

    #[test]
    fn test_project_doc_options_new_inherits_defaults() {
        let options = ProjectDocOptions::new(PathBuf::from("/custom"));
        assert_eq!(options.cwd, PathBuf::from("/custom"));
        assert_eq!(options.max_bytes, DEFAULT_PROJECT_DOC_MAX_BYTES);
    }

    #[test]
    fn test_project_doc_options_with_max_bytes() {
        let options = ProjectDocOptions::default().with_max_bytes(4096);
        assert_eq!(options.max_bytes, 4096);
    }

    #[test]
    fn test_project_doc_options_with_fallback_multiple() {
        let options = ProjectDocOptions::default()
            .with_fallback("README.md")
            .with_fallback("CLAUDE.md");
        assert_eq!(options.fallback_filenames.len(), 2);
    }

    #[test]
    fn test_project_doc_options_with_instructions_string() {
        let options = ProjectDocOptions::default().with_instructions("My custom instructions");
        assert_eq!(
            options.user_instructions,
            Some("My custom instructions".to_string())
        );
    }

    #[test]
    fn test_find_git_root_with_git_dir() {
        let temp = setup_temp_dir();
        let git_dir = temp.path().join(".git");
        fs::create_dir(&git_dir).expect("create .git dir");

        let result = find_git_root(temp.path());
        assert!(result.is_some());
        // Compare canonical paths
        let expected = temp.path().canonicalize().unwrap();
        let result_path = result.unwrap();
        let actual = result_path.canonicalize().unwrap_or(result_path);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_find_git_root_no_git_dir() {
        let temp = setup_temp_dir();
        // Don't create .git - but the test temp dir might be inside a git repo
        // So we just verify the function runs without panic
        let _ = find_git_root(temp.path());
    }

    #[test]
    fn test_discover_in_single_dir_with_agents_md() {
        let temp = setup_temp_dir();
        let agents_file = temp.path().join("AGENTS.md");
        fs::write(&agents_file, "# Test").expect("write file");

        let options = ProjectDocOptions::new(temp.path().to_path_buf());
        let result = discover_in_single_dir(temp.path(), &options);

        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("AGENTS.md"));
    }

    #[test]
    fn test_discover_in_single_dir_prefers_override() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("AGENTS.md"), "# Default").expect("write default");
        fs::write(temp.path().join("AGENTS.override.md"), "# Override").expect("write override");

        let options = ProjectDocOptions::new(temp.path().to_path_buf());
        let result = discover_in_single_dir(temp.path(), &options);

        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("AGENTS.override.md"));
    }

    #[test]
    fn test_discover_in_single_dir_empty() {
        let temp = setup_temp_dir();
        let options = ProjectDocOptions::new(temp.path().to_path_buf());
        let result = discover_in_single_dir(temp.path(), &options);
        assert!(result.is_empty());
    }

    #[test]
    fn test_discover_in_single_dir_with_fallback() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("CLAUDE.md"), "# Claude").expect("write file");

        let options = ProjectDocOptions::new(temp.path().to_path_buf()).with_fallback("CLAUDE.md");
        let result = discover_in_single_dir(temp.path(), &options);

        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("CLAUDE.md"));
    }

    #[tokio::test]
    async fn test_read_project_docs_returns_none_when_empty() {
        let temp = setup_temp_dir();
        let options = ProjectDocOptions::new(temp.path().to_path_buf());
        let result = read_project_docs(&options).await.expect("should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_read_project_docs_returns_content() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("AGENTS.md"), "Test content").expect("write");
        let options = ProjectDocOptions::new(temp.path().to_path_buf());
        let result = read_project_docs(&options).await.expect("should succeed");
        assert!(result.is_some());
        assert!(result.unwrap().contains("Test content"));
    }

    #[tokio::test]
    async fn test_read_project_docs_respects_max_bytes() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("AGENTS.md"), "A".repeat(1000)).expect("write");
        let options = ProjectDocOptions::new(temp.path().to_path_buf()).with_max_bytes(100);
        let result = read_project_docs(&options).await.expect("should succeed");
        assert!(result.is_some());
        assert!(result.unwrap().len() <= 100);
    }

    #[tokio::test]
    async fn test_read_project_docs_zero_max_bytes_returns_none() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("AGENTS.md"), "Content").expect("write");
        let options = ProjectDocOptions::new(temp.path().to_path_buf()).with_max_bytes(0);
        let result = read_project_docs(&options).await.expect("should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_user_instructions_with_only_user_instructions() {
        let temp = setup_temp_dir();
        let options = ProjectDocOptions::new(temp.path().to_path_buf())
            .with_instructions("User provided instructions");
        let result = get_user_instructions(&options).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "User provided instructions");
    }

    #[tokio::test]
    async fn test_get_user_instructions_with_both() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("AGENTS.md"), "Project docs").expect("write");
        let options = ProjectDocOptions::new(temp.path().to_path_buf())
            .with_instructions("User instructions");
        let result = get_user_instructions(&options).await;
        assert!(result.is_some());
        let content = result.unwrap();
        assert!(content.contains("User instructions"));
        assert!(content.contains("Project docs"));
        assert!(content.contains("project-doc"));
    }

    #[tokio::test]
    async fn test_get_user_instructions_none_when_empty() {
        let temp = setup_temp_dir();
        let options = ProjectDocOptions::new(temp.path().to_path_buf());
        let result = get_user_instructions(&options).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_discover_project_doc_paths_with_zero_max_bytes() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("AGENTS.md"), "Content").expect("write");
        let options = ProjectDocOptions::new(temp.path().to_path_buf()).with_max_bytes(0);
        let result = discover_project_doc_paths(&options)
            .await
            .expect("should succeed");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_discover_project_doc_paths_finds_file() {
        let temp = setup_temp_dir();
        fs::write(temp.path().join("AGENTS.md"), "Content").expect("write");
        let options = ProjectDocOptions::new(temp.path().to_path_buf());
        let result = discover_project_doc_paths(&options)
            .await
            .expect("should succeed");
        assert!(!result.is_empty());
    }
}
