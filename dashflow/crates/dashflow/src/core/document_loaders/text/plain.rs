//! Plain text file loaders.
//!
//! This module provides loaders for plain text files and directories.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::{Error, Result};

/// Loads a single text file as a document.
///
/// The `TextLoader` reads text files and creates a Document with the file content.
/// Metadata includes the source file path.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::TextLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TextLoader::new("example.txt");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TextLoader {
    /// Path to the text file
    pub file_path: PathBuf,
    /// Encoding to use when reading the file (default: utf-8)
    pub encoding: String,
}

impl TextLoader {
    /// Create a new `TextLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::TextLoader;
    ///
    /// let loader = TextLoader::new("example.txt");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            encoding: "utf-8".to_string(),
        }
    }

    /// Set the encoding for reading the file.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::TextLoader;
    ///
    /// let loader = TextLoader::new("example.txt")
    ///     .with_encoding("latin1");
    /// ```
    #[must_use]
    pub fn with_encoding(mut self, encoding: impl Into<String>) -> Self {
        self.encoding = encoding.into();
        self
    }
}

#[async_trait]
impl DocumentLoader for TextLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let encoding = self.encoding.clone();

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        // (Blob::as_string internally calls std::fs::read_to_string)
        tokio::task::spawn_blocking(move || {
            // Create a blob representing the file
            let blob = Blob::from_path(&file_path).with_encoding(&encoding);

            // Read the content
            let content = blob.as_string()?;

            // Create a document with metadata
            let doc =
                Document::new(content).with_metadata("source", file_path.display().to_string());

            Ok(vec![doc])
        })
        .await
        .map_err(|e| Error::Other(format!("spawn_blocking panicked: {e}")))?
    }
}

/// Loads all text files from a directory.
///
/// The `DirectoryLoader` recursively walks a directory and loads all text files
/// matching the specified glob pattern.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DirectoryLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DirectoryLoader::new("./docs")
///     .with_glob("**/*.txt");
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
        // Collect matching file paths in spawn_blocking to avoid blocking async runtime
        // with WalkDir and is_file() calls
        let dir_path = self.dir_path.clone();
        let recursive = self.recursive;
        let glob = self.glob.clone();

        let matching_paths: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
            let walker = if recursive {
                walkdir::WalkDir::new(&dir_path)
            } else {
                walkdir::WalkDir::new(&dir_path).max_depth(1)
            };

            walker
                .into_iter()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.path().is_file())
                .filter(|e| {
                    if let Some(ext) = e.path().extension() {
                        let ext_str = ext.to_string_lossy();
                        glob.contains(&format!("*.{ext_str}"))
                    } else {
                        false
                    }
                })
                .map(|e| e.path().to_path_buf())
                .collect()
        })
        .await
        .map_err(|e| Error::Other(format!("spawn_blocking panicked: {e}")))?;

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

/// Loads a file as unstructured text, handling both text and binary files.
///
/// The `UnstructuredFileLoader` attempts to read files as UTF-8 text. If the file
/// is not valid UTF-8, it reads it as binary data and base64 encodes it.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::UnstructuredFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = UnstructuredFileLoader::new("data.txt");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct UnstructuredFileLoader {
    /// Path to the file
    pub file_path: PathBuf,
}

impl UnstructuredFileLoader {
    /// Create a new `UnstructuredFileLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::UnstructuredFileLoader;
    ///
    /// let loader = UnstructuredFileLoader::new("data.txt");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for UnstructuredFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Try to read as text first
            let blob = Blob::from_path(&file_path);

            let content = if let Ok(text) = blob.as_string() {
                text
            } else {
                // If not valid UTF-8, read as binary and convert to base64
                let bytes = std::fs::read(&file_path).map_err(crate::core::error::Error::Io)?;
                format!(
                    "[Binary file with {} bytes - base64 encoded]\n{}",
                    bytes.len(),
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
                )
            };

            // Detect file extension for metadata
            let extension = file_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            // Create document with metadata
            let doc = Document::new(content)
                .with_metadata("source", file_path.display().to_string())
                .with_metadata("format", "unstructured")
                .with_metadata("file_extension", extension);

            Ok(vec![doc])
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads a binary file as a document with metadata only (no content extraction).
///
/// The `BinaryFileLoader` creates a document containing only metadata about the
/// binary file (size, path, MIME type) without extracting or encoding the content.
/// Useful for indexing binary files or creating references.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::BinaryFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = BinaryFileLoader::new("image.png");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct BinaryFileLoader {
    /// Path to the binary file
    pub file_path: PathBuf,
}

impl BinaryFileLoader {
    /// Create a new `BinaryFileLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::BinaryFileLoader;
    ///
    /// let loader = BinaryFileLoader::new("data.bin");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for BinaryFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Get file metadata
            let metadata_std =
                std::fs::metadata(&file_path).map_err(crate::core::error::Error::Io)?;

            let file_size = metadata_std.len();

            // Detect MIME type
            let mime_type = mime_guess::from_path(&file_path)
                .first()
                .map_or_else(|| "application/octet-stream".to_string(), |m| m.to_string());

            // Detect file extension
            let extension = file_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            // Create document with metadata only (no content)
            let content = format!(
                "Binary file: {}\nSize: {} bytes\nType: {}\nExtension: {}",
                file_path.display(),
                file_size,
                mime_type,
                extension
            );

            let doc = Document::new(content)
                .with_metadata("source", file_path.display().to_string())
                .with_metadata("format", "binary")
                .with_metadata("file_size", file_size.to_string())
                .with_metadata("mime_type", mime_type)
                .with_metadata("file_extension", extension);

            Ok(vec![doc])
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_text_loader() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, world!").unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Hello, world!");
        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(
            docs[0].get_metadata("source").and_then(|v| v.as_str()),
            file_path.to_str()
        );
    }

    #[tokio::test]
    async fn test_text_loader_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        fs::write(&file_path, "").unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Empty file should produce one document");
        assert_eq!(docs[0].page_content, "", "Content should be empty string");
        assert!(docs[0].metadata.contains_key("source"));
    }

    #[tokio::test]
    async fn test_text_loader_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("unicode.txt");

        let unicode_content = "Hello 世界 🌍 مرحبا שלום Привет\nMulti-byte: 🚀🎉\nMath: ∑∫√π";
        fs::write(&file_path, unicode_content).unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, unicode_content);
        assert!(docs[0].page_content.contains("世界"));
        assert!(docs[0].page_content.contains("🌍"));
        assert!(docs[0].page_content.contains("مرحبا"));
        assert!(docs[0].page_content.contains("שלום"));
        assert!(docs[0].page_content.contains("Привет"));
    }

    #[tokio::test]
    async fn test_text_loader_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");

        // Create a 1MB file (1000 lines of 1000 chars each)
        let line = "A".repeat(1000) + "\n";
        let content = line.repeat(1000);
        fs::write(&file_path, &content).unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content.len(), content.len());
        assert_eq!(docs[0].page_content.lines().count(), 1000);
    }

    #[tokio::test]
    async fn test_text_loader_single_character() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("single.txt");
        fs::write(&file_path, "X").unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "X");
        assert_eq!(docs[0].page_content.len(), 1);
    }

    #[tokio::test]
    async fn test_text_loader_mixed_line_endings() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("mixed.txt");

        // Mix of \n, \r\n, \r line endings
        let content = "Line 1\nLine 2\r\nLine 3\rLine 4";
        fs::write(&file_path, content).unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, content);
        // Content preserved exactly as written
        assert!(docs[0].page_content.contains("\n"));
        assert!(docs[0].page_content.contains("\r\n"));
    }

    #[tokio::test]
    async fn test_text_loader_whitespace_only() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("whitespace.txt");
        fs::write(&file_path, "   \n\t\t\n   ").unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "   \n\t\t\n   ");
        // Whitespace should be preserved, not stripped
        assert!(docs[0].page_content.contains(" "));
        assert!(docs[0].page_content.contains("\t"));
    }

    #[tokio::test]
    async fn test_text_loader_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.txt");

        let loader = TextLoader::new(&file_path);
        let result = loader.load().await;

        assert!(result.is_err(), "Should error when file doesn't exist");
        // Error message should be helpful
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("nonexistent")
                || err_msg.contains("No such file")
                || err_msg.contains("not found"),
            "Error should mention missing file, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_text_loader_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("special.txt");

        // Test special characters: null bytes, control chars, etc.
        let content = "Line1\x00NullByte\nTab\there\nVertical\x0bTab";
        fs::write(&file_path, content).unwrap();

        let loader = TextLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, content);
        // Special characters preserved
        assert!(docs[0].page_content.contains("\x00"));
        assert!(docs[0].page_content.contains("\x0b"));
    }

    #[tokio::test]
    async fn test_directory_loader() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        fs::write(temp_dir.path().join("file1.txt"), "Content 1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "Content 2").unwrap();
        fs::write(temp_dir.path().join("file3.md"), "Markdown content").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.txt");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should load 2 .txt files");
        assert!(docs.iter().any(|d| d.page_content == "Content 1"));
        assert!(docs.iter().any(|d| d.page_content == "Content 2"));
        // Verify .md file was excluded
        assert!(!docs.iter().any(|d| d.page_content.contains("Markdown")));
        // Verify all docs have source metadata
        assert!(docs.iter().all(|d| d.metadata.contains_key("source")));
    }

    #[tokio::test]
    async fn test_directory_loader_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let loader = DirectoryLoader::new(temp_dir.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            0,
            "Empty directory should produce zero documents"
        );
    }

    #[tokio::test]
    async fn test_directory_loader_nested_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested structure
        let nested_dir = temp_dir.path().join("subdir");
        fs::create_dir(&nested_dir).unwrap();
        let deep_dir = nested_dir.join("deep");
        fs::create_dir(&deep_dir).unwrap();

        fs::write(temp_dir.path().join("root.txt"), "Root file").unwrap();
        fs::write(nested_dir.join("nested.txt"), "Nested file").unwrap();
        fs::write(deep_dir.join("deep.txt"), "Deep file").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.txt");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3, "Should find files in nested directories");
        assert!(docs.iter().any(|d| d.page_content == "Root file"));
        assert!(docs.iter().any(|d| d.page_content == "Nested file"));
        assert!(docs.iter().any(|d| d.page_content == "Deep file"));
    }

    #[tokio::test]
    async fn test_directory_loader_mixed_file_types() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("doc.txt"), "Text").unwrap();
        fs::write(temp_dir.path().join("data.json"), r#"{"key":"value"}"#).unwrap();
        fs::write(temp_dir.path().join("style.css"), "body {}").unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Title").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path());
        let docs = loader.load().await.unwrap();

        // DirectoryLoader may have a default file type filter (e.g., only .txt)
        // Verify at least some files are loaded
        assert!(!docs.is_empty(), "Should load at least one file");
        // If it loads only .txt by default, that's expected behavior
        if docs.len() == 1 {
            assert!(docs.iter().any(|d| d.page_content == "Text"));
        }
        // All loaded documents should have source metadata
        assert!(docs.iter().all(|d| d.metadata.contains_key("source")));
    }

    #[tokio::test]
    async fn test_directory_loader_glob_no_matches() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("file.txt"), "Content").unwrap();
        fs::write(temp_dir.path().join("file.md"), "Markdown").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.pdf");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 0, "No matches should produce zero documents");
    }

    #[tokio::test]
    async fn test_directory_loader_glob_multiple_patterns() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("doc.txt"), "Text").unwrap();
        fs::write(temp_dir.path().join("doc.md"), "Markdown").unwrap();
        fs::write(temp_dir.path().join("data.json"), "JSON").unwrap();

        // Test multiple separate glob patterns - test first .txt, then .md
        let loader_txt = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.txt");
        let docs_txt = loader_txt.load().await.unwrap();
        assert_eq!(docs_txt.len(), 1);
        assert!(docs_txt.iter().any(|d| d.page_content == "Text"));

        let loader_md = DirectoryLoader::new(temp_dir.path()).with_glob("**/*.md");
        let docs_md = loader_md.load().await.unwrap();
        assert_eq!(docs_md.len(), 1);
        assert!(docs_md.iter().any(|d| d.page_content == "Markdown"));
    }

    #[tokio::test]
    async fn test_directory_loader_hidden_files() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join(".hidden"), "Hidden content").unwrap();
        fs::write(temp_dir.path().join("visible.txt"), "Visible content").unwrap();

        let loader = DirectoryLoader::new(temp_dir.path());
        let docs = loader.load().await.unwrap();

        // Behavior may vary: some loaders skip hidden files, some include them
        // Validate that visible file is always loaded
        assert!(docs.iter().any(|d| d.page_content == "Visible content"));
    }

    #[tokio::test]
    async fn test_directory_loader_path_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent_dir");

        let loader = DirectoryLoader::new(&nonexistent);
        let result = loader.load().await;

        // DirectoryLoader may return empty vec or error for nonexistent paths
        // Either behavior is acceptable
        if result.is_ok() {
            assert_eq!(
                result.unwrap().len(),
                0,
                "Nonexistent directory should produce no documents"
            );
        }
    }

    #[tokio::test]
    async fn test_directory_loader_file_instead_of_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "Content").unwrap();

        let loader = DirectoryLoader::new(&file_path);
        let result = loader.load().await;

        // Should either error or handle gracefully
        // If it errors, that's expected; if it succeeds, it should have 0 or 1 docs
        if result.is_ok() {
            let docs = result.unwrap();
            assert!(docs.len() <= 1, "File path should not load as directory");
        }
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_text() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let text_content = "This is unstructured text content.\nWith multiple lines.";
        fs::write(&file_path, text_content).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return exactly one document");
        assert_eq!(
            docs[0].page_content, text_content,
            "Content should match original text"
        );
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("unstructured"),
            "Should have format=unstructured metadata"
        );
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("txt"),
            "Should detect .txt extension"
        );
        let source = docs[0].get_metadata("source").and_then(|v| v.as_str());
        assert!(source.is_some(), "Should have source metadata");
        assert!(
            source.unwrap().contains("test.txt"),
            "Source should contain file name"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_binary() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        // Write some binary data
        let binary_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE];
        fs::write(&file_path, &binary_data).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "Should return exactly one document for binary file"
        );
        // Binary files should be base64 encoded (lines 1575-1581 behavior)
        assert!(
            docs[0].page_content.contains("Binary file with"),
            "Should indicate binary file with byte count"
        );
        assert!(
            docs[0].page_content.contains("5 bytes"),
            "Should show byte count (5 bytes)"
        );
        assert!(
            docs[0].page_content.contains("base64 encoded"),
            "Should mention base64 encoding"
        );
        // Verify base64 content is present (lines 1580-1581 encode bytes to base64)
        assert!(
            docs[0].page_content.len() > 50,
            "Should contain base64 data (longer than headers)"
        );
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("bin"),
            "Should detect .bin extension"
        );
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("unstructured"),
            "Should have format=unstructured metadata"
        );
        let source = docs[0].get_metadata("source").and_then(|v| v.as_str());
        assert!(source.is_some(), "Should have source metadata");
        assert!(
            source.unwrap().contains("test.bin"),
            "Source should contain file name"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");

        fs::write(&file_path, "").unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document for empty file");
        assert_eq!(docs[0].page_content, "", "Content should be empty string");
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("unstructured"),
            "Should have format=unstructured metadata"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("unicode.txt");

        let unicode_content = "Hello 世界! 🌍\nMulti-byte: ñ, é, ü\nEmoji: 🚀🎉";
        fs::write(&file_path, unicode_content).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert_eq!(
            docs[0].page_content, unicode_content,
            "Unicode content should be preserved exactly"
        );
        assert!(
            docs[0].page_content.contains("世界"),
            "Should preserve Chinese characters"
        );
        assert!(docs[0].page_content.contains("🌍"), "Should preserve emoji");
        assert!(
            docs[0].page_content.contains("ñ"),
            "Should preserve accented characters"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("noext");

        let content = "File without extension";
        fs::write(&file_path, content).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert_eq!(docs[0].page_content, content, "Content should match");
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("unknown"),
            "Should use 'unknown' for missing extension (line 1590)"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_various_extensions() {
        let temp_dir = TempDir::new().unwrap();

        // Test .log extension
        let log_path = temp_dir.path().join("app.log");
        fs::write(&log_path, "Log entry 1").unwrap();
        let loader_log = UnstructuredFileLoader::new(&log_path);
        let docs_log = loader_log.load().await.unwrap();
        assert_eq!(
            docs_log[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("log"),
            "Should detect .log extension"
        );

        // Test .md extension
        let md_path = temp_dir.path().join("doc.md");
        fs::write(&md_path, "# Markdown").unwrap();
        let loader_md = UnstructuredFileLoader::new(&md_path);
        let docs_md = loader_md.load().await.unwrap();
        assert_eq!(
            docs_md[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("md"),
            "Should detect .md extension"
        );

        // Test .data extension
        let data_path = temp_dir.path().join("file.data");
        fs::write(&data_path, "Data content").unwrap();
        let loader_data = UnstructuredFileLoader::new(&data_path);
        let docs_data = loader_data.load().await.unwrap();
        assert_eq!(
            docs_data[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("data"),
            "Should detect .data extension"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_large_text() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");

        // Create ~1000 lines of text (~50KB)
        let mut large_content = String::new();
        for i in 0..1000 {
            large_content.push_str(&format!(
                "Line {}: This is a test line with some content.\n",
                i
            ));
        }
        fs::write(&file_path, &large_content).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document for large file");
        assert_eq!(
            docs[0].page_content, large_content,
            "Large content should be preserved exactly"
        );
        assert!(
            docs[0].page_content.len() > 40000,
            "Content should be over 40KB"
        );
        assert!(
            docs[0].page_content.contains("Line 0:"),
            "Should contain first line"
        );
        assert!(
            docs[0].page_content.contains("Line 999:"),
            "Should contain last line"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_large_binary() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.bin");

        // Create 1KB of binary data
        let binary_data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        fs::write(&file_path, &binary_data).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "Should return one document for large binary file"
        );
        assert!(
            docs[0].page_content.contains("Binary file with"),
            "Should indicate binary file"
        );
        assert!(
            docs[0].page_content.contains("1024 bytes"),
            "Should show 1024 bytes"
        );
        assert!(
            docs[0].page_content.contains("base64 encoded"),
            "Should mention base64"
        );
        // Base64 encoding expands ~33%, so 1024 bytes -> ~1366 base64 chars + headers
        assert!(
            docs[0].page_content.len() > 1300,
            "Should contain base64 data"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_whitespace_only() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("whitespace.txt");

        let whitespace_content = "   \n\t\n  \n\t\t\n   ";
        fs::write(&file_path, whitespace_content).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert_eq!(
            docs[0].page_content, whitespace_content,
            "Whitespace should be preserved exactly"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_mixed_newlines() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("mixed.txt");

        // Mix of \n and \r\n newlines
        let mixed_content = "Line 1\nLine 2\r\nLine 3\nLine 4\r\n";
        fs::write(&file_path, mixed_content).unwrap();

        let loader = UnstructuredFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert_eq!(
            docs[0].page_content, mixed_content,
            "Mixed newlines should be preserved"
        );
    }

    #[tokio::test]
    async fn test_unstructured_file_loader_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.txt");

        let loader = UnstructuredFileLoader::new(&file_path);
        let result = loader.load().await;

        assert!(result.is_err(), "Should return error for nonexistent file");
    }

    #[tokio::test]
    async fn test_binary_file_loader() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.dat");

        // Write some binary data
        let binary_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE];
        fs::write(&file_path, &binary_data).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return exactly one document");
        // Should contain file metadata (lines 1666-1672 format content)
        assert!(
            docs[0].page_content.contains("Binary file:"),
            "Content should start with 'Binary file:'"
        );
        assert!(
            docs[0].page_content.contains("Size: 5 bytes"),
            "Should show exact byte size"
        );
        assert!(
            docs[0].page_content.contains("Type:"),
            "Should include MIME type"
        );
        assert!(
            docs[0].page_content.contains("Extension:"),
            "Should include extension"
        );
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("binary"),
            "Should have format=binary metadata"
        );
        assert_eq!(
            docs[0].get_metadata("file_size").and_then(|v| v.as_str()),
            Some("5"),
            "Should have file_size=5 metadata"
        );
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("dat"),
            "Should detect .dat extension"
        );
        let mime = docs[0].get_metadata("mime_type").and_then(|v| v.as_str());
        assert!(mime.is_some(), "Should have mime_type metadata");
        let source = docs[0].get_metadata("source").and_then(|v| v.as_str());
        assert!(source.is_some(), "Should have source metadata");
        assert!(
            source.unwrap().contains("test.dat"),
            "Source should contain file name"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.bin");

        fs::write(&file_path, []).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document for empty file");
        assert!(
            docs[0].page_content.contains("Size: 0 bytes"),
            "Should show 0 bytes"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_image_png() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("image.png");

        // Write PNG header bytes (not a valid PNG, just header signature)
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        fs::write(&file_path, &png_header).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert!(
            docs[0].page_content.contains("Size: 8 bytes"),
            "Should show 8 bytes"
        );
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("png"),
            "Should detect .png extension"
        );
        // MIME type should be image/png (mime_guess from extension)
        let mime = docs[0].get_metadata("mime_type").and_then(|v| v.as_str());
        assert!(mime.is_some(), "Should have MIME type");
        assert!(
            mime.unwrap().contains("image"),
            "PNG should have image MIME type"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_pdf() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("document.pdf");

        // Write PDF header signature (not valid PDF, just header)
        let pdf_header = b"%PDF-1.4\n";
        fs::write(&file_path, pdf_header).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("pdf"),
            "Should detect .pdf extension"
        );
        let mime = docs[0].get_metadata("mime_type").and_then(|v| v.as_str());
        assert!(mime.is_some(), "Should have MIME type");
        assert!(
            mime.unwrap().contains("pdf"),
            "PDF should have pdf MIME type"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("binaryfile");

        let binary_data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        fs::write(&file_path, &binary_data).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("unknown"),
            "Should use 'unknown' for missing extension (line 1663)"
        );
        let mime = docs[0].get_metadata("mime_type").and_then(|v| v.as_str());
        assert!(mime.is_some(), "Should have MIME type");
        assert_eq!(
            mime.unwrap(),
            "application/octet-stream",
            "Unknown extension should default to application/octet-stream (line 1656)"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_zip() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("archive.zip");

        // Write ZIP header signature (PK\x03\x04)
        let zip_header = vec![0x50, 0x4B, 0x03, 0x04];
        fs::write(&file_path, &zip_header).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document");
        assert_eq!(
            docs[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("zip"),
            "Should detect .zip extension"
        );
        let mime = docs[0].get_metadata("mime_type").and_then(|v| v.as_str());
        assert!(mime.is_some(), "Should have MIME type");
        assert!(
            mime.unwrap().contains("zip"),
            "ZIP should have zip MIME type"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_large() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.bin");

        // Create 1MB of binary data
        let large_data: Vec<u8> = (0..1048576).map(|i| (i % 256) as u8).collect();
        fs::write(&file_path, &large_data).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should return one document for large file");
        assert!(
            docs[0].page_content.contains("Size: 1048576 bytes"),
            "Should show 1MB byte size"
        );
        assert_eq!(
            docs[0].get_metadata("file_size").and_then(|v| v.as_str()),
            Some("1048576"),
            "Should have file_size=1048576 metadata"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_various_extensions() {
        let temp_dir = TempDir::new().unwrap();

        // Test .jpg extension
        let jpg_path = temp_dir.path().join("photo.jpg");
        fs::write(&jpg_path, vec![0xFF, 0xD8, 0xFF]).unwrap();
        let loader_jpg = BinaryFileLoader::new(&jpg_path);
        let docs_jpg = loader_jpg.load().await.unwrap();
        assert_eq!(
            docs_jpg[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("jpg"),
            "Should detect .jpg extension"
        );

        // Test .mp4 extension
        let mp4_path = temp_dir.path().join("video.mp4");
        fs::write(&mp4_path, vec![0x00, 0x00, 0x00, 0x18]).unwrap();
        let loader_mp4 = BinaryFileLoader::new(&mp4_path);
        let docs_mp4 = loader_mp4.load().await.unwrap();
        assert_eq!(
            docs_mp4[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("mp4"),
            "Should detect .mp4 extension"
        );

        // Test .exe extension
        let exe_path = temp_dir.path().join("program.exe");
        fs::write(&exe_path, vec![0x4D, 0x5A]).unwrap(); // MZ header
        let loader_exe = BinaryFileLoader::new(&exe_path);
        let docs_exe = loader_exe.load().await.unwrap();
        assert_eq!(
            docs_exe[0]
                .get_metadata("file_extension")
                .and_then(|v| v.as_str()),
            Some("exe"),
            "Should detect .exe extension"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_content_format() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        let binary_data = vec![0xCA, 0xFE, 0xBA, 0xBE];
        fs::write(&file_path, &binary_data).unwrap();

        let loader = BinaryFileLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        // Verify content format follows "Binary file: {path}\nSize: {size} bytes\nType: {mime}\nExtension: {ext}"
        let content = &docs[0].page_content;
        assert!(
            content.starts_with("Binary file:"),
            "Content should start with 'Binary file:'"
        );
        assert!(content.contains("\n"), "Content should have newlines");
        let lines: Vec<&str> = content.lines().collect();
        assert!(
            lines.len() >= 4,
            "Content should have at least 4 lines (file, size, type, extension)"
        );
        assert!(
            lines[0].starts_with("Binary file:"),
            "First line should be file path"
        );
        assert!(lines[1].starts_with("Size:"), "Second line should be size");
        assert!(
            lines[2].starts_with("Type:"),
            "Third line should be MIME type"
        );
        assert!(
            lines[3].starts_with("Extension:"),
            "Fourth line should be extension"
        );
    }

    #[tokio::test]
    async fn test_binary_file_loader_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.bin");

        let loader = BinaryFileLoader::new(&file_path);
        let result = loader.load().await;

        assert!(result.is_err(), "Should return error for nonexistent file");
    }
}
