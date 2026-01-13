//! Directory and URL loaders.
//!
//! This module provides core loaders for loading documents from:
//! - Local directories with glob pattern matching
//! - URLs with HTML-to-text conversion
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::constants::{
    DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT, DEFAULT_POOL_IDLE_TIMEOUT,
    DEFAULT_TCP_KEEPALIVE,
};
use crate::core::documents::{Document, DocumentLoader};
use crate::core::error::Result;
use crate::core::http_client::{validate_url_for_ssrf, HttpClientBuilder};

// Import TextLoader from formats::text module
use crate::core::document_loaders::TextLoader;

/// Loads documents from all files in a directory matching a glob pattern.
///
/// The `DirectoryLoader` recursively walks a directory and loads files that match
/// the specified glob pattern using the appropriate document loader.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DirectoryLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DirectoryLoader::new("./docs")
///     .with_glob("**/*.md")
///     .with_recursive(true);
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DirectoryLoader {
    /// Path to the directory
    pub dir_path: PathBuf,
    /// Glob pattern for matching files (default: "**/*.txt")
    pub glob: String,
    /// Whether to recursively search subdirectories (default: true)
    pub recursive: bool,
}

impl DirectoryLoader {
    /// Create a new `DirectoryLoader` for the given directory path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::DirectoryLoader;
    ///
    /// let loader = DirectoryLoader::new("./docs");
    /// ```
    pub fn new(dir_path: impl AsRef<Path>) -> Self {
        Self {
            dir_path: dir_path.as_ref().to_path_buf(),
            glob: "**/*.txt".to_string(),
            recursive: true,
        }
    }

    /// Set the glob pattern for matching files.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::DirectoryLoader;
    ///
    /// let loader = DirectoryLoader::new("./docs")
    ///     .with_glob("**/*.md");
    /// ```
    #[must_use]
    pub fn with_glob(mut self, glob: impl Into<String>) -> Self {
        self.glob = glob.into();
        self
    }

    /// Set whether to recursively search subdirectories.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::DirectoryLoader;
    ///
    /// let loader = DirectoryLoader::new("./docs")
    ///     .with_recursive(false);
    /// ```
    #[must_use]
    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }
}

#[async_trait]
impl DocumentLoader for DirectoryLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // First, collect matching file paths in a blocking task
        // to avoid blocking the async runtime with WalkDir and is_file()
        let dir_path = self.dir_path.clone();
        let recursive = self.recursive;
        let glob = self.glob.clone();

        let matching_paths: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
            let walker = if recursive {
                walkdir::WalkDir::new(&dir_path)
            } else {
                walkdir::WalkDir::new(&dir_path).max_depth(1)
            };

            let mut paths = Vec::new();
            for entry in walker.into_iter().filter_map(std::result::Result::ok) {
                let path = entry.path();

                // Skip directories (is_file is blocking)
                if !path.is_file() {
                    continue;
                }

                // Check if file matches glob pattern (simple implementation)
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy();
                    if glob.contains(&format!("*.{ext_str}")) {
                        paths.push(path.to_path_buf());
                    }
                }
            }
            paths
        })
        .await
        .unwrap_or_default();

        // Now load each file asynchronously
        let mut documents = Vec::new();
        for path in matching_paths {
            let loader = TextLoader::new(&path);
            match loader.load().await {
                Ok(mut docs) => documents.append(&mut docs),
                Err(e) => {
                    // Log error but continue with other files
                    tracing::warn!(path = %path.display(), error = %e, "Error loading file");
                }
            }
        }

        Ok(documents)
    }
}

/// Loads a web page from a URL.
///
/// The `URLLoader` fetches HTML content from a URL and converts it to plain text.
/// This is useful for loading web pages into the document processing pipeline.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::URLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = URLLoader::new("https://example.com");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct URLLoader {
    /// URL to fetch
    pub url: String,
    /// User agent to use for requests
    pub user_agent: Option<String>,
}

impl URLLoader {
    /// Create a new `URLLoader` for the given URL.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::URLLoader;
    ///
    /// let loader = URLLoader::new("https://example.com");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            user_agent: None,
        }
    }

    /// Set a custom user agent for the HTTP request.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::URLLoader;
    ///
    /// let loader = URLLoader::new("https://example.com")
    ///     .with_user_agent("MyBot/1.0");
    /// ```
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }
}

#[async_trait]
impl DocumentLoader for URLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Validate URL to prevent SSRF attacks (blocks private IPs, metadata endpoints)
        validate_url_for_ssrf(&self.url)?;

        // Build HTTP client with connection pool limits (using centralized constants)
        let client = HttpClientBuilder::new()
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(DEFAULT_POOL_IDLE_TIMEOUT)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .request_timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
            .tcp_keepalive(DEFAULT_TCP_KEEPALIVE)
            .build()?;

        // Fetch the URL
        let response = client
            .get(&self.url)
            .header(
                reqwest::header::USER_AGENT,
                self.user_agent.as_deref().unwrap_or("DashFlow/0.1.0"),
            )
            .send()
            .await
            .map_err(|e| crate::core::error::Error::Http(format!("Failed to fetch URL: {e}")))?;

        // Check status
        if !response.status().is_success() {
            return Err(crate::core::error::Error::Http(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Get content type to determine if it's HTML
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Get the response body
        let body = response.text().await.map_err(|e| {
            crate::core::error::Error::Http(format!("Failed to read response body: {e}"))
        })?;

        // Convert HTML to text if content is HTML
        let content = if content_type.contains("html") {
            html2text::from_read(body.as_bytes(), 80)
        } else {
            body
        };

        // Create document with URL as metadata
        let doc = Document::new(content).with_metadata("source", self.url.clone());

        Ok(vec![doc])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ==========================================================================
    // DirectoryLoader Tests
    // ==========================================================================

    #[test]
    fn test_directory_loader_new() {
        let loader = DirectoryLoader::new("./docs");
        assert_eq!(loader.dir_path, PathBuf::from("./docs"));
        assert_eq!(loader.glob, "**/*.txt");
        assert!(loader.recursive);
    }

    #[test]
    fn test_directory_loader_with_glob() {
        let loader = DirectoryLoader::new("./docs").with_glob("**/*.md");
        assert_eq!(loader.glob, "**/*.md");

        let loader2 = DirectoryLoader::new("./docs").with_glob("*.rs".to_string());
        assert_eq!(loader2.glob, "*.rs");
    }

    #[test]
    fn test_directory_loader_with_recursive() {
        let loader = DirectoryLoader::new("./docs").with_recursive(false);
        assert!(!loader.recursive);

        let loader2 = DirectoryLoader::new("./docs").with_recursive(true);
        assert!(loader2.recursive);
    }

    #[test]
    fn test_directory_loader_clone() {
        let loader = DirectoryLoader::new("./test")
            .with_glob("**/*.rs")
            .with_recursive(false);
        let cloned = loader.clone();
        assert_eq!(cloned.dir_path, loader.dir_path);
        assert_eq!(cloned.glob, loader.glob);
        assert_eq!(cloned.recursive, loader.recursive);
    }

    #[test]
    fn test_directory_loader_debug() {
        let loader = DirectoryLoader::new("./docs");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("DirectoryLoader"));
        assert!(debug_str.contains("docs"));
    }

    #[test]
    fn test_directory_loader_chained_config() {
        let loader = DirectoryLoader::new("/tmp/test")
            .with_glob("*.txt")
            .with_recursive(false);

        assert_eq!(loader.dir_path, PathBuf::from("/tmp/test"));
        assert_eq!(loader.glob, "*.txt");
        assert!(!loader.recursive);
    }

    #[tokio::test]
    async fn test_directory_loader_load_txt_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        let file1_path = temp_dir.path().join("file1.txt");
        std::fs::write(&file1_path, "Content of file 1").unwrap();

        let file2_path = temp_dir.path().join("file2.txt");
        std::fs::write(&file2_path, "Content of file 2").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.txt");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_directory_loader_non_recursive() {
        let temp_dir = TempDir::new().unwrap();

        // Create file in root
        let file1_path = temp_dir.path().join("root.txt");
        std::fs::write(&file1_path, "Root file").unwrap();

        // Create subdirectory with file
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        let file2_path = sub_dir.join("nested.txt");
        std::fs::write(&file2_path, "Nested file").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path())
            .with_glob("**/*.txt")
            .with_recursive(false);
        let docs = loader.load().await.unwrap();

        // Should only find the root file
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_directory_loader_recursive() {
        let temp_dir = TempDir::new().unwrap();

        // Create file in root
        let file1_path = temp_dir.path().join("root.txt");
        std::fs::write(&file1_path, "Root file").unwrap();

        // Create subdirectory with file
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        let file2_path = sub_dir.join("nested.txt");
        std::fs::write(&file2_path, "Nested file").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path())
            .with_glob("**/*.txt")
            .with_recursive(true);
        let docs = loader.load().await.unwrap();

        // Should find both files
        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_directory_loader_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let loader = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.txt");
        let docs = loader.load().await.unwrap();

        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn test_directory_loader_no_matching_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file with non-matching extension
        let file_path = temp_dir.path().join("file.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.txt");
        let docs = loader.load().await.unwrap();

        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn test_directory_loader_nonexistent_directory() {
        let loader = DirectoryLoader::new("/nonexistent/path/to/dir");
        let docs = loader.load().await.unwrap();

        // Should return empty, not error (walkdir handles this gracefully)
        assert!(docs.is_empty());
    }

    // ==========================================================================
    // URLLoader Tests
    // ==========================================================================

    #[test]
    fn test_url_loader_new() {
        let loader = URLLoader::new("https://example.com");
        assert_eq!(loader.url, "https://example.com");
        assert!(loader.user_agent.is_none());
    }

    #[test]
    fn test_url_loader_with_user_agent() {
        let loader = URLLoader::new("https://example.com").with_user_agent("MyBot/1.0");
        assert_eq!(loader.user_agent, Some("MyBot/1.0".to_string()));

        let loader2 =
            URLLoader::new("https://example.com").with_user_agent("CustomAgent".to_string());
        assert_eq!(loader2.user_agent, Some("CustomAgent".to_string()));
    }

    #[test]
    fn test_url_loader_clone() {
        let loader = URLLoader::new("https://example.com").with_user_agent("Test");
        let cloned = loader.clone();
        assert_eq!(cloned.url, loader.url);
        assert_eq!(cloned.user_agent, loader.user_agent);
    }

    #[test]
    fn test_url_loader_debug() {
        let loader = URLLoader::new("https://example.com");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("URLLoader"));
        assert!(debug_str.contains("example.com"));
    }

    #[tokio::test]
    async fn test_url_loader_ssrf_private_ip() {
        let loader = URLLoader::new("http://192.168.1.1/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_url_loader_ssrf_localhost() {
        let loader = URLLoader::new("http://localhost/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_url_loader_ssrf_loopback() {
        let loader = URLLoader::new("http://127.0.0.1/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_url_loader_ssrf_ipv6_loopback() {
        let loader = URLLoader::new("http://[::1]/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_url_loader_ssrf_metadata_endpoint() {
        let loader = URLLoader::new("http://169.254.169.254/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_url_loader_invalid_url() {
        let loader = URLLoader::new("not-a-valid-url");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_url_loader_ssrf_link_local() {
        let loader = URLLoader::new("http://169.254.0.1/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    // ==========================================================================
    // DocumentLoader Trait Tests
    // ==========================================================================

    #[test]
    fn test_loaders_implement_document_loader() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<DirectoryLoader>();
        _assert_document_loader::<URLLoader>();
    }

    // ==========================================================================
    // Path Handling Tests
    // ==========================================================================

    #[test]
    fn test_directory_loader_various_paths() {
        let loader1 = DirectoryLoader::new(".");
        assert_eq!(loader1.dir_path, PathBuf::from("."));

        let loader2 = DirectoryLoader::new("/absolute/path");
        assert_eq!(loader2.dir_path, PathBuf::from("/absolute/path"));

        let loader3 = DirectoryLoader::new("relative/path");
        assert_eq!(loader3.dir_path, PathBuf::from("relative/path"));
    }

    #[test]
    fn test_url_loader_various_urls() {
        let loader1 = URLLoader::new("https://example.com");
        assert_eq!(loader1.url, "https://example.com");

        let loader2 = URLLoader::new("https://example.com/path/to/page?query=value");
        assert_eq!(loader2.url, "https://example.com/path/to/page?query=value");

        let loader3 = URLLoader::new("http://example.com:8080");
        assert_eq!(loader3.url, "http://example.com:8080");
    }

    #[test]
    fn test_directory_loader_glob_patterns() {
        let loader1 = DirectoryLoader::new(".").with_glob("**/*.rs");
        assert_eq!(loader1.glob, "**/*.rs");

        let loader2 = DirectoryLoader::new(".").with_glob("*.md");
        assert_eq!(loader2.glob, "*.md");

        let loader3 = DirectoryLoader::new(".").with_glob("src/**/*.rs");
        assert_eq!(loader3.glob, "src/**/*.rs");
    }
}
