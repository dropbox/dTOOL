// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Core utility document loaders.
//!
//! This module provides loaders for utility and development file formats:
//! - `LogFileLoader`: System and application log files
//! - `DiffLoader`: Git diffs and patch files
//! - `SQLLoader`: SQL scripts and queries
//! - `ForthLoader`: Forth programming language files
//! - `XAMLLoader`: XAML (eXtensible Application Markup Language) files
//! - `SVGLoader`: Scalable Vector Graphics files
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document};
use crate::core::error::Result;

use crate::core::documents::DocumentLoader;

/// Loads log files with intelligent entry separation.
///
/// The `LogFileLoader` reads log files and can optionally separate individual
/// log entries based on timestamp patterns.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::LogFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = LogFileLoader::new("app.log")
///     .with_separate_entries(true);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LogFileLoader {
    /// Path to the log file
    pub file_path: PathBuf,
    /// Create separate documents per log entry (default: false)
    pub separate_entries: bool,
    /// Pattern to identify start of new log entry (default: timestamp pattern)
    pub entry_pattern: Option<String>,
}

impl LogFileLoader {
    /// Create a new `LogFileLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_entries: false,
            entry_pattern: None,
        }
    }

    /// Create separate documents per log entry.
    #[must_use]
    pub fn with_separate_entries(mut self, separate: bool) -> Self {
        self.separate_entries = separate;
        self
    }

    /// Set custom pattern to identify start of new log entry.
    #[must_use]
    pub fn with_entry_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.entry_pattern = Some(pattern.into());
        self
    }

    fn is_entry_start(line: &str) -> bool {
        // Common log timestamp patterns:
        // [2025-10-30 12:34:56]
        // 2025-10-30 12:34:56
        // [INFO] [2025-10-30]
        // etc.

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Check for timestamp-like patterns
        // [YYYY-MM-DD or YYYY/MM/DD at start
        let starts_with_timestamp =
            trimmed.starts_with('[') || trimmed.chars().next().is_some_and(|c| c.is_ascii_digit());

        starts_with_timestamp
            && (trimmed.contains('-') || trimmed.contains('/'))
            && trimmed.contains(':')
    }
}

#[async_trait]
impl DocumentLoader for LogFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let mut documents = Vec::new();
        let mut all_content = String::new();
        let mut entry_count = 0;
        let mut current_entry = String::new();

        for line in content.lines() {
            let is_new_entry = if let Some(ref pattern) = self.entry_pattern {
                line.contains(pattern.as_str())
            } else {
                Self::is_entry_start(line)
            };

            if is_new_entry && !current_entry.is_empty() {
                // End of previous entry
                entry_count += 1;

                if self.separate_entries {
                    let doc = Document::new(current_entry.clone())
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("entry_index", entry_count - 1)
                        .with_metadata("format", "log");

                    documents.push(doc);
                } else {
                    all_content.push_str(&current_entry);
                    all_content.push('\n');
                }

                current_entry.clear();
            }

            current_entry.push_str(line);
            current_entry.push('\n');
        }

        // Handle last entry
        if !current_entry.is_empty() {
            entry_count += 1;

            if self.separate_entries {
                let doc = Document::new(current_entry)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("entry_index", entry_count - 1)
                    .with_metadata("format", "log");

                documents.push(doc);
            } else {
                all_content.push_str(&current_entry);
            }
        }

        if !self.separate_entries {
            let doc = Document::new(all_content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "log")
                .with_metadata("entry_count", entry_count);

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loads diff/patch files (Git diffs, unified diffs).
///
/// The `DiffLoader` reads diff and patch files, with optional separation
/// per file being modified.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DiffLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DiffLoader::new("changes.diff")
///     .with_separate_files(true);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DiffLoader {
    /// Path to the diff/patch file
    pub file_path: PathBuf,
    /// Separate documents per file diff (default: false)
    pub separate_files: bool,
}

impl DiffLoader {
    /// Create a new `DiffLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_files: false,
        }
    }

    /// Create separate documents per file in the diff.
    #[must_use]
    pub fn with_separate_files(mut self, separate: bool) -> Self {
        self.separate_files = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for DiffLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_files {
            // Split by "diff --git" markers
            let mut documents = Vec::new();
            let mut current_diff = String::new();
            let mut file_name = String::new();
            let mut file_index = 0;

            for line in content.lines() {
                if line.starts_with("diff --git") {
                    // Save previous diff
                    if !current_diff.is_empty() {
                        let doc = Document::new(current_diff.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("file_index", file_index)
                            .with_metadata("file_name", file_name.clone())
                            .with_metadata("format", "diff");

                        documents.push(doc);
                        current_diff.clear();
                        file_index += 1;
                    }

                    // Extract file name from diff header
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        file_name = parts[3].trim_start_matches("b/").to_string();
                    }
                } else if line.starts_with("--- ") && file_name.is_empty() {
                    // Fallback for diffs without "diff --git" header
                    if !current_diff.is_empty() {
                        let doc = Document::new(current_diff.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("file_index", file_index)
                            .with_metadata("file_name", file_name.clone())
                            .with_metadata("format", "diff");

                        documents.push(doc);
                        current_diff.clear();
                        file_index += 1;
                    }

                    file_name = line
                        .trim_start_matches("--- ")
                        .trim_start_matches("a/")
                        .to_string();
                }

                current_diff.push_str(line);
                current_diff.push('\n');
            }

            // Add last diff
            if !current_diff.is_empty() {
                let doc = Document::new(current_diff)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("file_index", file_index)
                    .with_metadata("file_name", file_name)
                    .with_metadata("format", "diff");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let mut additions = 0;
            let mut deletions = 0;

            for line in content.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    additions += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    deletions += 1;
                }
            }

            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "diff")
                .with_metadata("additions", additions)
                .with_metadata("deletions", deletions);

            Ok(vec![doc])
        }
    }
}

/// Loads SQL script files.
///
/// The `SQLLoader` reads SQL scripts and can optionally separate by statement
/// (terminated by semicolons).
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::SQLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SQLLoader::new("schema.sql")
///     .with_separate_statements(true);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SQLLoader {
    /// Path to the SQL file
    pub file_path: PathBuf,
    /// Separate documents per SQL statement (default: false)
    pub separate_statements: bool,
}

impl SQLLoader {
    /// Create a new `SQLLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_statements: false,
        }
    }

    /// Create separate documents per SQL statement (terminated by semicolon).
    #[must_use]
    pub fn with_separate_statements(mut self, separate: bool) -> Self {
        self.separate_statements = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for SQLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_statements {
            // Split by semicolons
            let mut documents = Vec::new();
            let mut current_stmt = String::new();
            let mut stmt_index = 0;
            let mut in_string = false;
            let mut string_char = ' ';

            for line in content.lines() {
                // Track string literals to avoid splitting on semicolons inside strings
                for ch in line.chars() {
                    if (ch == '\'' || ch == '"') && !in_string {
                        in_string = true;
                        string_char = ch;
                    } else if ch == string_char && in_string {
                        in_string = false;
                    }
                }

                current_stmt.push_str(line);
                current_stmt.push('\n');

                // Check for statement terminator
                if !in_string && line.trim_end().ends_with(';') {
                    let trimmed = current_stmt.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("--") {
                        // Extract statement type (SELECT, INSERT, CREATE, etc.)
                        let stmt_type = trimmed
                            .split_whitespace()
                            .next()
                            .unwrap_or("UNKNOWN")
                            .to_uppercase();

                        let doc = Document::new(current_stmt.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("statement_index", stmt_index)
                            .with_metadata("statement_type", stmt_type)
                            .with_metadata("format", "sql");

                        documents.push(doc);
                        current_stmt.clear();
                        stmt_index += 1;
                    }
                }
            }

            // Add any remaining content (incomplete statement)
            if !current_stmt.trim().is_empty() {
                let doc = Document::new(current_stmt)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("statement_index", stmt_index)
                    .with_metadata("statement_type", "INCOMPLETE")
                    .with_metadata("format", "sql");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Count statements for metadata
            let stmt_count = content.matches(';').count();

            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "sql")
                .with_metadata("statement_count", stmt_count);

            Ok(vec![doc])
        }
    }
}

/// Loads Forth programming language files.
///
/// The `ForthLoader` reads Forth source files and can optionally separate
/// by word definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::ForthLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ForthLoader::new("program.fs")
///     .with_separate_definitions(true);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ForthLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl ForthLoader {
    /// Creates a new Forth source file loader.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the `.fs`, `.fth`, or `.4th` file to load
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// When enabled, creates separate documents for each word definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for ForthLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_definitions {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "forth")]);
        }

        // Parse Forth word definitions
        // Forth syntax: : word-name definition ; (colon definitions)
        //               VARIABLE name (variable definitions)
        //               CONSTANT name (constant definitions)
        //               CREATE name (create definitions)
        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut definition_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Skip empty lines and comments (Forth comments: ( ... ) or \ ... )
            if line.is_empty() || line.starts_with('\\') || line.starts_with("( ") {
                i += 1;
                continue;
            }

            // Check for colon definition: : word-name ...
            if line.starts_with(": ") {
                // Extract word name
                let def_name = line
                    .strip_prefix(": ")
                    .unwrap_or("")
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .trim();

                // Collect definition until semicolon
                let mut definition_lines = vec![lines[i]];
                let mut found_semicolon = line.contains(';');
                i += 1;

                while i < lines.len() && !found_semicolon {
                    definition_lines.push(lines[i]);
                    if lines[i].contains(';') {
                        found_semicolon = true;
                    }
                    i += 1;
                }

                let definition_content = definition_lines.join("\n");
                documents.push(
                    Document::new(&definition_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "forth")
                        .with_metadata("definition_index", definition_index.to_string())
                        .with_metadata("definition_name", def_name.to_string()),
                );
                definition_index += 1;
            } else if line.starts_with("VARIABLE ") || line.starts_with("CREATE ") {
                // VARIABLE name or CREATE name
                let parts: Vec<&str> = line.split_whitespace().collect();
                let def_name = if parts.len() >= 2 {
                    parts[1]
                } else {
                    "unknown"
                };

                documents.push(
                    Document::new(line)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "forth")
                        .with_metadata("definition_index", definition_index.to_string())
                        .with_metadata("definition_name", def_name.to_string()),
                );
                definition_index += 1;
                i += 1;
            } else if line.contains(" CONSTANT ") {
                // value CONSTANT name (Forth syntax: number comes first)
                let parts: Vec<&str> = line.split_whitespace().collect();
                let def_name = if parts.len() >= 3 {
                    parts[2]
                } else {
                    "unknown"
                };

                documents.push(
                    Document::new(line)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "forth")
                        .with_metadata("definition_index", definition_index.to_string())
                        .with_metadata("definition_name", def_name.to_string()),
                );
                definition_index += 1;
                i += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "forth")])
        } else {
            Ok(documents)
        }
    }
}

/// Loader for XAML (eXtensible Application Markup Language) files.
///
/// XAML is Microsoft's declarative XML-based markup language for defining
/// user interfaces in .NET applications (WPF, UWP, Xamarin, .NET MAUI).
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::XAMLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = XAMLLoader::new("MainWindow.xaml");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct XAMLLoader {
    file_path: PathBuf,
}

impl XAMLLoader {
    /// Create a new XAML loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for XAMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        Ok(vec![Document::new(&content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "xaml")])
    }
}

/// Loader for SVG (Scalable Vector Graphics) files.
///
/// SVG is an XML-based vector image format for two-dimensional graphics with support for
/// interactivity and animation.
///
/// # History and Context
///
/// - **Created:** 1999 (SVG 1.0), 2001 (SVG 1.1), 2016 (SVG 2)
/// - **Creator:** W3C (World Wide Web Consortium)
/// - **Purpose:** Scalable vector graphics for the web
/// - **Based on:** XML, with influences from VML (Microsoft) and PGML (Adobe)
///
/// SVG provides:
/// - Resolution-independent graphics (scales without quality loss)
/// - Searchable and indexable text
/// - CSS styling support
/// - JavaScript interactivity
/// - Animation (SMIL or CSS)
/// - Accessibility features
/// - Compression support (SVGZ)
///
/// # Key Features
///
/// SVG supports:
/// - Basic shapes (rect, circle, ellipse, line, polyline, polygon)
/// - Paths (complex curves and shapes)
/// - Text (with full font support)
/// - Gradients and patterns
/// - Filters and effects
/// - Transformations (translate, rotate, scale, skew)
/// - Clipping and masking
/// - Links and scripting
///
/// # File Extensions
///
/// - `.svg` - SVG file (XML text)
/// - `.svgz` - Compressed SVG (gzip)
///
/// # Usage
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::SVGLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SVGLoader::new("image.svg");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
///
/// This loader can optionally extract text content from SVG text elements.
#[derive(Debug, Clone)]
pub struct SVGLoader {
    file_path: PathBuf,
    extract_text_only: bool,
}

impl SVGLoader {
    /// Create a new SVG loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            extract_text_only: false,
        }
    }

    /// Enable text-only extraction mode (extracts content from `<text>` elements).
    #[must_use]
    pub fn with_text_only(mut self) -> Self {
        self.extract_text_only = true;
        self
    }

    /// Extract text from SVG \<text\> elements (simple regex-based extraction)
    fn extract_text_content(svg_content: &str) -> String {
        let mut text_parts = Vec::new();
        let mut in_text_element = false;
        let mut current_text = String::new();

        for line in svg_content.lines() {
            let trimmed = line.trim();

            if trimmed.contains("<text") {
                in_text_element = true;
                current_text.clear();
            }

            if in_text_element {
                // Extract text between > and < (simple approach)
                if let Some(start) = trimmed.find('>') {
                    if let Some(end) = trimmed.rfind("</text>") {
                        let text = &trimmed[start + 1..end].trim();
                        if !text.is_empty() {
                            text_parts.push((*text).to_string());
                        }
                        in_text_element = false;
                    } else if let Some(end) = trimmed.rfind('<') {
                        let text = &trimmed[start + 1..end].trim();
                        if !text.is_empty() {
                            current_text.push_str(text);
                        }
                    }
                }
            }

            if trimmed.contains("</text>") {
                if !current_text.is_empty() {
                    text_parts.push(current_text.clone());
                    current_text.clear();
                }
                in_text_element = false;
            }
        }

        text_parts.join("\n")
    }
}

#[async_trait]
impl DocumentLoader for SVGLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let page_content = if self.extract_text_only {
            Self::extract_text_content(&content)
        } else {
            content.clone()
        };

        Ok(vec![Document::new(&page_content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "svg")])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ============================================================================
    // LogFileLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_log_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Application started\nProcessing request\nCompleted").unwrap();

        let loader = LogFileLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Application started"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "log");
    }

    #[tokio::test]
    async fn test_log_loader_separate_entries() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "[2025-01-01 10:00:00] First entry\n[2025-01-01 10:01:00] Second entry"
        )
        .unwrap();

        let loader = LogFileLoader::new(file.path()).with_separate_entries(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("First entry"));
        assert!(docs[1].page_content.contains("Second entry"));
    }

    #[tokio::test]
    async fn test_log_loader_custom_pattern() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "ERROR: First error\nERROR: Second error").unwrap();

        let loader = LogFileLoader::new(file.path())
            .with_separate_entries(true)
            .with_entry_pattern("ERROR:");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_log_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "test log").unwrap();

        let loader = LogFileLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "log");
    }

    #[tokio::test]
    async fn test_log_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = LogFileLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    // ============================================================================
    // DiffLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_diff_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "--- a/old.txt\n+++ b/new.txt\n@@ -1,3 +1,3 @@\n-old line\n+new line"
        )
        .unwrap();

        let loader = DiffLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("old line"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "diff");
    }

    #[tokio::test]
    async fn test_diff_loader_separate_files() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "diff --git a/file1.txt b/file1.txt\n--- a/file1.txt\n+++ b/file1.txt\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/file2.txt b/file2.txt\n--- a/file2.txt\n+++ b/file2.txt\n@@ -1 +1 @@\n-foo\n+bar"
        )
        .unwrap();

        let loader = DiffLoader::new(file.path()).with_separate_files(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("file_name").unwrap(), "file1.txt");
        assert_eq!(docs[1].metadata.get("file_name").unwrap(), "file2.txt");
    }

    #[tokio::test]
    async fn test_diff_loader_counts_additions_deletions() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "--- a/test.txt\n+++ b/test.txt\n@@ -1,2 +1,2 @@\n-deleted line\n+added line"
        )
        .unwrap();

        let loader = DiffLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs[0].metadata.get("additions").unwrap(), 1);
        assert_eq!(docs[0].metadata.get("deletions").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_diff_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "--- a/test.txt\n+++ b/test.txt").unwrap();

        let loader = DiffLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "diff");
    }

    #[tokio::test]
    async fn test_diff_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = DiffLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    // ============================================================================
    // SQLLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_sql_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "SELECT * FROM users;").unwrap();

        let loader = SQLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("SELECT"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "sql");
    }

    #[tokio::test]
    async fn test_sql_loader_separate_statements() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "SELECT * FROM users;\nINSERT INTO users VALUES (1, 'John');\nDELETE FROM users WHERE id = 1;"
        )
        .unwrap();

        let loader = SQLLoader::new(file.path()).with_separate_statements(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].metadata.get("statement_type").unwrap(), "SELECT");
        assert_eq!(docs[1].metadata.get("statement_type").unwrap(), "INSERT");
        assert_eq!(docs[2].metadata.get("statement_type").unwrap(), "DELETE");
    }

    #[tokio::test]
    async fn test_sql_loader_statement_count() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "SELECT 1; SELECT 2; SELECT 3;").unwrap();

        let loader = SQLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs[0].metadata.get("statement_count").unwrap(), 3);
    }

    #[tokio::test]
    async fn test_sql_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "SELECT 1;").unwrap();

        let loader = SQLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "sql");
    }

    #[tokio::test]
    async fn test_sql_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = SQLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    // ============================================================================
    // ForthLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_forth_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, ": square dup * ;").unwrap();

        let loader = ForthLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("square"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "forth");
    }

    #[tokio::test]
    async fn test_forth_loader_separate_definitions() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            ": square dup * ;\n: cube dup dup * * ;\nVARIABLE counter"
        )
        .unwrap();

        let loader = ForthLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "square");
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "cube");
        assert_eq!(docs[2].metadata.get("definition_name").unwrap(), "counter");
    }

    #[tokio::test]
    async fn test_forth_loader_constant() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "100 CONSTANT max-value").unwrap();

        let loader = ForthLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("definition_name").unwrap(),
            "max-value"
        );
    }

    #[tokio::test]
    async fn test_forth_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, ": test ;").unwrap();

        let loader = ForthLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "forth");
    }

    #[tokio::test]
    async fn test_forth_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = ForthLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    // ============================================================================
    // XAMLLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_xaml_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "<Window>\n  <Button Content=\"Click Me\"/>\n</Window>"
        )
        .unwrap();

        let loader = XAMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Button"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "xaml");
    }

    #[tokio::test]
    async fn test_xaml_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<Grid/>").unwrap();

        let loader = XAMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "xaml");
    }

    #[tokio::test]
    async fn test_xaml_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = XAMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    // ============================================================================
    // SVGLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_svg_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<svg><circle cx=\"50\" cy=\"50\" r=\"40\"/></svg>").unwrap();

        let loader = SVGLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("circle"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "svg");
    }

    #[tokio::test]
    async fn test_svg_loader_text_extraction() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "<svg><text x=\"10\" y=\"20\">Hello World</text></svg>"
        )
        .unwrap();

        let loader = SVGLoader::new(file.path()).with_text_only();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Hello World"));
    }

    #[tokio::test]
    async fn test_svg_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<svg></svg>").unwrap();

        let loader = SVGLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "svg");
    }

    #[tokio::test]
    async fn test_svg_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = SVGLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }
}
