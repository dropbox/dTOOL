//! Developer platform and version control loaders.
//!
//! This module provides loaders for developer tools and platforms:
//! - `GitBook` (documentation platform)
//! - Git (version control repository files)
//! - GitHub Issues (issue tracking and discussions)
//! - Browserless (headless Chrome API for JavaScript-rendered content)
//! - Chromium (local headless browser automation)
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::{Error, Result};

/// Loader for `GitBook` documentation files.
///
/// `GitBook` files are markdown with YAML frontmatter designed for technical
/// documentation. This loader parses the content including metadata.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, GitBookLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = GitBookLoader::new("docs/README.md");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct GitBookLoader {
    file_path: PathBuf,
}

impl GitBookLoader {
    /// Create a new `GitBook` loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for GitBookLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // GitBook files are markdown with YAML frontmatter
        // Format includes title, description, and other metadata at the top
        // The structure is designed for technical documentation

        Ok(vec![Document::new(&content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "gitbook")
            .with_metadata("type", "documentation")])
    }
}

/// Loader for Git repository files.
///
/// Recursively loads all files from a Git repository with optional filtering
/// by file extension. Skips the .git directory.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, GitLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = GitLoader::new("/path/to/repo")
///     .with_file_filter(&[".rs", ".md"])
///     .with_branch("main");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct GitLoader {
    repo_path: PathBuf,
    file_filter: Option<Vec<String>>,
    /// Git branch to load from (defaults to working directory)
    ///
    /// When set, files are read directly from the specified branch using
    /// `git show <branch>:<filepath>` without modifying the working directory.
    /// When None, files are read from the current working directory.
    branch: Option<String>,
}

impl GitLoader {
    /// Create a new Git repository loader for the given path.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            file_filter: None,
            branch: None,
        }
    }

    /// Set file extensions to include (e.g., [".rs", ".md"]).
    #[must_use]
    pub fn with_file_filter(mut self, extensions: &[&str]) -> Self {
        self.file_filter = Some(extensions.iter().map(|s| (*s).to_string()).collect());
        self
    }

    /// Set the branch to load from (defaults to current branch).
    #[must_use]
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }
}

#[async_trait]
impl DocumentLoader for GitLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // If a branch is specified, load files from that branch using git commands
        if let Some(ref branch) = self.branch {
            return self.load_from_branch(branch).await;
        }

        // Otherwise, load from working directory (existing behavior)
        self.load_from_working_directory().await
    }
}

impl GitLoader {
    /// Load documents from a specific git branch using `git show`
    ///
    /// This reads files directly from the branch without modifying the working directory.
    async fn load_from_branch(&self, branch: &str) -> Result<Vec<Document>> {
        // Clone data for spawn_blocking
        let repo_path = self.repo_path.clone();
        let file_filter = self.file_filter.clone();
        let branch = branch.to_string();

        tokio::task::spawn_blocking(move || {
            Self::load_from_branch_sync(&repo_path, &file_filter, &branch)
        })
        .await
        .map_err(|e| Error::Other(format!("spawn_blocking panicked: {}", e)))?
    }

    /// Synchronous implementation of load_from_branch
    fn load_from_branch_sync(
        repo_path: &Path,
        file_filter: &Option<Vec<String>>,
        branch: &str,
    ) -> Result<Vec<Document>> {
        let mut documents = Vec::new();

        // Get list of files in the branch using git ls-tree
        let output = std::process::Command::new("git")
            .args(["ls-tree", "-r", "--name-only", branch])
            .current_dir(repo_path)
            .output()
            .map_err(|e| Error::Other(format!("Failed to run git ls-tree: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Other(format!(
                "git ls-tree failed for branch '{}': {}",
                branch, stderr
            )));
        }

        let file_list = String::from_utf8_lossy(&output.stdout);

        for file_path in file_list.lines() {
            // Skip .git directory (shouldn't be in ls-tree output, but be safe)
            if file_path.starts_with(".git/") {
                continue;
            }

            // Apply file filter if specified
            if let Some(ref filter) = file_filter {
                let has_matching_ext = filter.iter().any(|ext| file_path.ends_with(ext));
                if !has_matching_ext {
                    continue;
                }
            }

            // Read file content from the branch using git show
            let show_output = std::process::Command::new("git")
                .args(["show", &format!("{}:{}", branch, file_path)])
                .current_dir(repo_path)
                .output()
                .map_err(|e| Error::Other(format!("Failed to run git show: {}", e)))?;

            if show_output.status.success() {
                // Try to parse as UTF-8 text (skip binary files)
                if let Ok(content) = String::from_utf8(show_output.stdout) {
                    let doc = Document::new(content)
                        .with_metadata("source", file_path.to_string())
                        .with_metadata("file_path", file_path.to_string())
                        .with_metadata("repo_path", repo_path.to_string_lossy().to_string())
                        .with_metadata("branch", branch.to_string())
                        .with_metadata("format", "git");

                    documents.push(doc);
                }
                // Skip binary files (non-UTF8)
            }
        }

        Ok(documents)
    }

    /// Load documents from the working directory (existing behavior)
    async fn load_from_working_directory(&self) -> Result<Vec<Document>> {
        // Clone data for spawn_blocking
        let repo_path = self.repo_path.clone();
        let file_filter = self.file_filter.clone();

        tokio::task::spawn_blocking(move || {
            Self::load_from_working_directory_sync(&repo_path, &file_filter)
        })
        .await
        .map_err(|e| Error::Other(format!("spawn_blocking panicked: {}", e)))?
    }

    /// Synchronous implementation of load_from_working_directory
    fn load_from_working_directory_sync(
        repo_path: &Path,
        file_filter: &Option<Vec<String>>,
    ) -> Result<Vec<Document>> {
        let mut documents = Vec::new();

        // Walk the repository directory
        for entry in walkdir::WalkDir::new(repo_path)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();

            // Skip .git directory
            if path.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }

            // Only process files
            if !path.is_file() {
                continue;
            }

            // Apply file filter if specified
            if let Some(ref filter) = file_filter {
                if let Some(ext) = path.extension() {
                    let ext_str = format!(".{}", ext.to_string_lossy());
                    if !filter.contains(&ext_str) {
                        continue;
                    }
                }
            }

            // Read file content
            if let Ok(content) = fs::read_to_string(path) {
                let relative_path = path
                    .strip_prefix(repo_path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                let doc = Document::new(content)
                    .with_metadata("source", relative_path.clone())
                    .with_metadata("file_path", relative_path)
                    .with_metadata("repo_path", repo_path.to_string_lossy().to_string())
                    .with_metadata("format", "git");

                documents.push(doc);
            }
        }

        Ok(documents)
    }
}

// ============================================================================
// REMOVED: GitHubIssuesLoader (was placeholder returning NotImplemented)
// ============================================================================
//
// Rationale: Removed placeholder loaders that only returned NotImplemented error.
// GitHubIssuesLoader was a 95-line placeholder with no actual implementation.
//
// When implementing GitHub Issues support, recreate with:
// - HTTP client (reqwest) with GitHub API authentication
// - Issues endpoint: GET https://api.github.com/repos/{owner}/{repo}/issues
// - Support for state filtering, pagination, rate limiting
// - Comments loading via /issues/{number}/comments endpoint
// - Document per issue with metadata (number, state, labels, author, etc.)
//
// See: https://docs.github.com/en/rest/issues for GitHub REST API docs
//
// ============================================================================
// REMOVED: BrowserlessLoader (was placeholder returning NotImplemented)
// ============================================================================
//
// Rationale: Removed placeholder loaders that only returned NotImplemented error.
// BrowserlessLoader was a 119-line placeholder with no actual implementation.
//
// When implementing Browserless.io support, recreate with:
// - HTTP client (reqwest) with Browserless API token
// - POST https://chrome.browserless.io/content?token={token}
// - Support for JavaScript rendering, wait conditions, selectors
// - HTML parsing and text extraction (scraper crate)
// - Optional screenshot/PDF generation
//
// See: https://docs.browserless.io/docs/start for Browserless API docs
//
// ============================================================================
// REMOVED: ChromiumLoader (was placeholder returning NotImplemented)
// ============================================================================
//
// Rationale: Removed placeholder loaders that only returned NotImplemented error.
// ChromiumLoader was a 129-line placeholder with no actual implementation.
//
// When implementing local Chromium automation, recreate with:
// - WebDriver client (fantoccini) or CDP client (chromiumoxide)
// - Chrome/Chromium binary detection and launch
// - Headless mode, wait conditions, navigation
// - HTML/text extraction from rendered pages
// - ChromeDriver management (download, start, stop)
//
// See: https://chromedriver.chromium.org/getting-started and fantoccini docs
//
// ============================================================================

#[cfg(test)]
mod tests {
    // Note: Tests for deleted placeholder loaders (GitHubIssuesLoader, BrowserlessLoader,
    // ChromiumLoader) removed in N=302. When implementing these loaders, add proper tests.
}
