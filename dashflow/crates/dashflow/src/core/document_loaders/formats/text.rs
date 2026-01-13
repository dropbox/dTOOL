//! Text format document loaders.
//!
//! This module provides loaders for text-based file formats including:
//! - Plain text (.txt)
//! - HTML (.html, .htm)
//! - Markdown (.md, .markdown)
//! - Rich Text Format (.rtf)
//! - `AsciiDoc` (.adoc, .asciidoc)
//! - reStructuredText (.rst)
//!
//! All loaders support backward compatibility through re-exports at the top level.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

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
        // Create a blob representing the file
        let blob = Blob::from_path(&self.file_path).with_encoding(&self.encoding);

        // Read the content
        let content = blob.as_string()?;

        // Create a document with metadata
        let doc =
            Document::new(content).with_metadata("source", self.file_path.display().to_string());

        Ok(vec![doc])
    }
}

/// Loads HTML files as documents.
///
/// The `HTMLLoader` reads HTML files and converts them to plain text, preserving
/// the document structure while removing HTML tags.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::HTMLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = HTMLLoader::new("document.html");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HTMLLoader {
    /// Path to the HTML file
    pub file_path: PathBuf,
    /// Width for text wrapping (default: 80)
    pub width: usize,
}

impl HTMLLoader {
    /// Create a new `HTMLLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::HTMLLoader;
    ///
    /// let loader = HTMLLoader::new("document.html");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            width: 80,
        }
    }

    /// Set the width for text wrapping.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::HTMLLoader;
    ///
    /// let loader = HTMLLoader::new("document.html")
    ///     .with_width(100);
    /// ```
    #[must_use]
    pub fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }
}

#[async_trait]
impl DocumentLoader for HTMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let width = self.width;

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Read the HTML file
            let html_content = std::fs::read(&file_path).map_err(crate::core::error::Error::Io)?;

            // Convert HTML to plain text
            let text = html2text::from_read(&html_content[..], width);

            // Create document with metadata
            let doc = Document::new(text)
                .with_metadata("source", file_path.display().to_string())
                .with_metadata("format", "html");

            Ok(vec![doc])
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads Markdown files as documents.
///
/// The `MarkdownLoader` reads Markdown files and can either preserve the Markdown
/// formatting or convert to plain text.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::MarkdownLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = MarkdownLoader::new("document.md");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownLoader {
    /// Path to the Markdown file
    pub file_path: PathBuf,
    /// Whether to convert to plain text (default: false, keeps markdown)
    pub to_plain_text: bool,
}

impl MarkdownLoader {
    /// Create a new `MarkdownLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::MarkdownLoader;
    ///
    /// let loader = MarkdownLoader::new("document.md");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            to_plain_text: false,
        }
    }

    /// Set whether to convert to plain text.
    ///
    /// If true, Markdown formatting is removed and only plain text is returned.
    /// If false (default), Markdown formatting is preserved.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::MarkdownLoader;
    ///
    /// let loader = MarkdownLoader::new("document.md")
    ///     .with_plain_text(true);
    /// ```
    #[must_use]
    pub fn with_plain_text(mut self, to_plain_text: bool) -> Self {
        self.to_plain_text = to_plain_text;
        self
    }
}

#[async_trait]
impl DocumentLoader for MarkdownLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let to_plain_text = self.to_plain_text;

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Read the Markdown file
            let markdown_content =
                std::fs::read_to_string(&file_path).map_err(crate::core::error::Error::Io)?;

            let content = if to_plain_text {
                // Convert Markdown to plain text
                use pulldown_cmark::{Event, Parser, Tag, TagEnd};

                let parser = Parser::new(&markdown_content);
                let mut plain_text = String::new();

                for event in parser {
                    match event {
                        Event::Text(text) | Event::Code(text) => {
                            plain_text.push_str(&text);
                        }
                        Event::Start(Tag::Paragraph | Tag::Heading { .. }) => {
                            // Add newlines before blocks
                            if !plain_text.is_empty() && !plain_text.ends_with('\n') {
                                plain_text.push('\n');
                            }
                        }
                        Event::End(TagEnd::Paragraph | TagEnd::Heading(_)) => {
                            plain_text.push('\n');
                        }
                        Event::SoftBreak | Event::HardBreak => {
                            plain_text.push('\n');
                        }
                        _ => {}
                    }
                }

                plain_text.trim().to_string()
            } else {
                // Keep original Markdown
                markdown_content
            };

            // Create document with metadata
            let doc = Document::new(content)
                .with_metadata("source", file_path.display().to_string())
                .with_metadata("format", "markdown");

            Ok(vec![doc])
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads Rich Text Format (.rtf) files.
///
/// The `RTFLoader` performs basic text extraction from RTF files by stripping
/// RTF control sequences. For full RTF parsing, consider using a dedicated RTF library.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::RTFLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RTFLoader::new("document.rtf");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RTFLoader {
    /// Path to the RTF file
    pub file_path: PathBuf,
}

impl RTFLoader {
    /// Create a new `RTFLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for RTFLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Basic RTF text extraction - strip control sequences
        let text = Self::extract_text_from_rtf(&content);

        let doc = Document::new(text)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "rtf");

        Ok(vec![doc])
    }
}

impl RTFLoader {
    fn extract_text_from_rtf(content: &str) -> String {
        let mut text = String::new();
        let mut in_control = false;
        let mut brace_depth = 0;
        let mut skip_next = false;

        for ch in content.chars() {
            if skip_next {
                skip_next = false;
                continue;
            }

            match ch {
                '{' => {
                    brace_depth += 1;
                }
                '}' => {
                    brace_depth = (brace_depth - 1).max(0);
                    in_control = false;
                }
                '\\' => {
                    in_control = true;
                }
                ' ' | '\n' | '\r' if in_control => {
                    in_control = false;
                }
                _ if !in_control && brace_depth > 0 => {
                    // Only add text if we're inside the document (brace_depth > 0)
                    if ch.is_ascii_alphanumeric() || ch.is_whitespace() || ch.is_ascii_punctuation()
                    {
                        text.push(ch);
                    }
                }
                _ => {}
            }
        }

        // Clean up excessive whitespace
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }
}

/// Loader for `AsciiDoc` files (.adoc, .asciidoc).
///
/// `AsciiDoc` is a plain text markup language for writing technical documentation.
/// Created in 2002 by Stuart Rackham, inspired by `DocBook` XML.
/// More powerful than Markdown for complex documentation (books, manuals).
/// Used by O'Reilly, GitHub Docs, and many technical projects.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::AsciiDocLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = AsciiDocLoader::new("document.adoc");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct AsciiDocLoader {
    file_path: PathBuf,
    separate_sections: bool,
}

impl AsciiDocLoader {
    /// Create a new `AsciiDoc` loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_sections: false,
        }
    }

    /// Enable separation by sections (== for level 1, === for level 2, etc).
    #[must_use]
    pub fn with_separate_sections(mut self) -> Self {
        self.separate_sections = true;
        self
    }

    /// Check if line is an `AsciiDoc` section header (starts with ==)
    fn is_section_header(line: &str) -> Option<(usize, String)> {
        let trimmed = line.trim();
        if !trimmed.starts_with("==") {
            return None;
        }

        // Count '=' characters
        let level = trimmed.chars().take_while(|&c| c == '=').count();
        if level < 2 {
            return None;
        }

        // Extract title (after '=' and spaces)
        let title = trimmed[level..].trim().to_string();
        if title.is_empty() {
            return None;
        }

        Some((level, title))
    }
}

#[async_trait]
impl DocumentLoader for AsciiDocLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_sections {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "asciidoc")]);
        }

        // Separate by sections (== Section, === Subsection, etc)
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut current_section = Vec::new();
        let mut current_title = String::new();
        let mut current_level = 0;
        let mut found_first_section = false;

        for line in lines {
            if let Some((level, title)) = Self::is_section_header(line) {
                // Save previous section (only if we've found at least one section)
                if found_first_section && !current_section.is_empty() {
                    let section_content = current_section.join("\n");
                    documents.push(
                        Document::new(&section_content)
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "asciidoc")
                            .with_metadata("section_index", documents.len().to_string())
                            .with_metadata("section_title", current_title.clone())
                            .with_metadata("section_level", current_level.to_string()),
                    );
                    current_section.clear();
                }

                // Start new section
                found_first_section = true;
                current_title = title;
                current_level = level;
                current_section.push(line);
            } else {
                // Only accumulate content after first section is found
                if found_first_section {
                    current_section.push(line);
                }
            }
        }

        // Save last section
        if found_first_section && !current_section.is_empty() {
            let section_content = current_section.join("\n");
            documents.push(
                Document::new(&section_content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "asciidoc")
                    .with_metadata("section_index", documents.len().to_string())
                    .with_metadata("section_title", current_title)
                    .with_metadata("section_level", current_level.to_string()),
            );
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "asciidoc")])
        } else {
            Ok(documents)
        }
    }
}

/// Loads reStructuredText (.rst) files as documents.
///
/// The `RSTLoader` reads .rst files commonly used in Python documentation.
/// Can parse sections and directives.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::RSTLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RSTLoader::new("README.rst");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RSTLoader {
    /// Path to the RST file
    pub file_path: PathBuf,
    /// Separate documents per section (default: false)
    pub separate_sections: bool,
}

impl RSTLoader {
    /// Create a new `RSTLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_sections: false,
        }
    }

    /// Create separate documents per RST section.
    #[must_use]
    pub fn with_separate_sections(mut self, separate: bool) -> Self {
        self.separate_sections = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for RSTLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_sections {
            // Split by RST section headers (underlined with =, -, ~, etc.)
            let mut documents = Vec::new();
            let mut current_section = String::new();
            let mut section_title = String::new();
            let mut section_index = 0;
            let lines: Vec<&str> = content.lines().collect();

            let mut i = 0;
            while i < lines.len() {
                let line = lines[i];

                // Check if next line is a section underline
                if i + 1 < lines.len() {
                    let next_line = lines[i + 1];
                    let is_underline = next_line.chars().all(|c| {
                        c == '=' || c == '-' || c == '~' || c == '^' || c == '#' || c == '*'
                    }) && next_line.len() >= line.trim().len();

                    if is_underline && !line.trim().is_empty() {
                        // Save previous section
                        if !current_section.is_empty() {
                            let doc = Document::new(current_section.clone())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("section_index", section_index)
                                .with_metadata("section_title", section_title.clone())
                                .with_metadata("format", "rst");

                            documents.push(doc);
                            current_section.clear();
                            section_index += 1;
                        }

                        // Start new section
                        section_title = line.trim().to_string();
                        current_section.push_str(line);
                        current_section.push('\n');
                        current_section.push_str(next_line);
                        current_section.push('\n');
                        i += 2;
                        continue;
                    }
                }

                current_section.push_str(line);
                current_section.push('\n');
                i += 1;
            }

            // Add last section
            if !current_section.is_empty() {
                let doc = Document::new(current_section)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("section_index", section_index)
                    .with_metadata("section_title", section_title)
                    .with_metadata("format", "rst");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "rst");

            Ok(vec![doc])
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_rst_loader() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("README.rst");

        let rst_content = r#"Main Title
==========

This is the introduction.

Section One
-----------

Content of section one.

Section Two
-----------

Content of section two.
"#;

        fs::write(&rst_path, rst_content).unwrap();

        let loader = RSTLoader::new(&rst_path);
        let docs = loader.load().await.unwrap();

        // Document count validation
        assert_eq!(
            docs.len(),
            1,
            "Default mode should produce exactly 1 document"
        );

        // Content validation - all sections preserved
        assert!(
            docs[0].page_content.contains("Main Title"),
            "Document should contain main title"
        );
        assert!(
            docs[0].page_content.contains("=========="),
            "Document should contain title underline"
        );
        assert!(
            docs[0].page_content.contains("This is the introduction."),
            "Document should contain introduction text"
        );
        assert!(
            docs[0].page_content.contains("Section One"),
            "Document should contain Section One"
        );
        assert!(
            docs[0].page_content.contains("-----------"),
            "Document should contain section underline"
        );
        assert!(
            docs[0].page_content.contains("Content of section one."),
            "Document should contain section one content"
        );
        assert!(
            docs[0].page_content.contains("Section Two"),
            "Document should contain Section Two"
        );
        assert!(
            docs[0].page_content.contains("Content of section two."),
            "Document should contain section two content"
        );

        // Metadata validation
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("rst"),
            "Document should have format='rst' metadata"
        );
        assert!(
            docs[0]
                .get_metadata("source")
                .and_then(|v| v.as_str())
                .unwrap()
                .contains("README.rst"),
            "Document should have source metadata with file path"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_separate_sections() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("doc.rst");

        let rst_content = r#"Title
=====

Introduction text.

First Section
-------------

First section content.

Second Section
--------------

Second section content.
"#;

        fs::write(&rst_path, rst_content).unwrap();

        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        // Document count validation
        assert_eq!(
            docs.len(),
            3,
            "Separate sections mode should produce 3 documents for 3 sections"
        );

        // Content validation - section 0 (Title)
        assert!(
            docs[0].page_content.contains("Title"),
            "First document should contain 'Title'"
        );
        assert!(
            docs[0].page_content.contains("====="),
            "First document should contain title underline"
        );
        assert!(
            docs[0].page_content.contains("Introduction text."),
            "First document should contain introduction content"
        );

        // Content validation - section 1 (First Section)
        assert!(
            docs[1].page_content.contains("First Section"),
            "Second document should contain 'First Section' title"
        );
        assert!(
            docs[1].page_content.contains("-------------"),
            "Second document should contain section underline"
        );
        assert!(
            docs[1].page_content.contains("First section content."),
            "Second document should contain first section content"
        );

        // Content validation - section 2 (Second Section)
        assert!(
            docs[2].page_content.contains("Second Section"),
            "Third document should contain 'Second Section' title"
        );
        assert!(
            docs[2].page_content.contains("--------------"),
            "Third document should contain section underline (longer than first)"
        );
        assert!(
            docs[2].page_content.contains("Second section content."),
            "Third document should contain second section content"
        );

        // Metadata validation - section_title
        assert_eq!(
            docs[0]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Title"),
            "First document should have section_title='Title'"
        );
        assert_eq!(
            docs[1]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("First Section"),
            "Second document should have section_title='First Section'"
        );
        assert_eq!(
            docs[2]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Second Section"),
            "Third document should have section_title='Second Section'"
        );

        // Metadata validation - section_index (0-based)
        assert_eq!(
            docs[0]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(0),
            "First document should have section_index=0"
        );
        assert_eq!(
            docs[1]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(1),
            "Second document should have section_index=1"
        );
        assert_eq!(
            docs[2]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(2),
            "Third document should have section_index=2"
        );

        // Metadata validation - format and source
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("rst"),
            "All documents should have format='rst'"
        );
        assert!(
            docs[0]
                .get_metadata("source")
                .and_then(|v| v.as_str())
                .unwrap()
                .contains("doc.rst"),
            "All documents should have source metadata with file path"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("empty.rst");

        // Empty file (0 bytes)
        fs::write(&rst_path, "").unwrap();

        let loader = RSTLoader::new(&rst_path);
        let docs = loader.load().await.unwrap();

        // Document count validation
        assert_eq!(
            docs.len(),
            1,
            "Empty file should produce exactly 1 document"
        );

        // Content validation
        assert_eq!(
            docs[0].page_content, "",
            "Empty file document should have empty content"
        );

        // Metadata validation
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("rst"),
            "Empty file document should have format='rst'"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_single_section() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("single.rst");

        let rst_content = r#"Single Section Title
====================

This is the only section in the document.
It has some content here.
"#;

        fs::write(&rst_path, rst_content).unwrap();

        // Test default mode
        let loader = RSTLoader::new(&rst_path);
        let docs = loader.load().await.unwrap();
        assert_eq!(
            docs.len(),
            1,
            "Default mode with single section should produce 1 document"
        );

        // Test separate sections mode
        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();
        assert_eq!(
            docs.len(),
            1,
            "Separate sections mode with single section should produce 1 document"
        );
        assert_eq!(
            docs[0]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Single Section Title"),
            "Section title should be extracted"
        );
        assert_eq!(
            docs[0]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(0),
            "Single section should have section_index=0"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_many_sections() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("many.rst");

        // Generate 10 sections with different underline characters
        let mut rst_content =
            String::from("Document Title\n==============\n\nDocument introduction.\n\n");
        let underlines = ['=', '-', '~', '^', '#', '*', '=', '-', '~', '^'];
        for (i, &underline_char) in underlines.iter().enumerate() {
            rst_content.push_str(&format!("Section {}\n", i));
            rst_content.push_str(&underline_char.to_string().repeat(10));
            rst_content.push('\n');
            rst_content.push_str(&format!("\nContent of section {}.\n\n", i));
        }

        fs::write(&rst_path, &rst_content).unwrap();

        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        // Document count validation (1 title + 10 sections = 11 documents)
        assert_eq!(
            docs.len(),
            11,
            "Should produce 11 documents for title + 10 sections"
        );

        // Validate first section (title)
        assert!(
            docs[0].page_content.contains("Document Title"),
            "First document should contain document title"
        );
        assert_eq!(
            docs[0]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Document Title"),
            "First document should have section_title='Document Title'"
        );
        assert_eq!(
            docs[0]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(0),
            "First document should have section_index=0"
        );

        // Validate last section (Section 9)
        assert!(
            docs[10].page_content.contains("Section 9"),
            "Last document should contain 'Section 9'"
        );
        assert!(
            docs[10].page_content.contains("Content of section 9."),
            "Last document should contain section 9 content"
        );
        assert_eq!(
            docs[10]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Section 9"),
            "Last document should have section_title='Section 9'"
        );
        assert_eq!(
            docs[10]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(10),
            "Last document should have section_index=10 (0-based, 11th section)"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_directives() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("directives.rst");

        let rst_content = r#"RST Directives
==============

.. note::
   This is a note directive.

.. code-block:: python

   def hello():
       print("Hello, World!")

.. warning::
   This is a warning.

Normal Section
--------------

Some regular content here.
"#;

        fs::write(&rst_path, rst_content).unwrap();

        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        // Document count validation
        assert_eq!(
            docs.len(),
            2,
            "Should produce 2 documents (main section with directives + Normal Section)"
        );

        // Content validation - directives preserved in first section
        assert!(
            docs[0].page_content.contains(".. note::"),
            "First document should contain note directive"
        );
        assert!(
            docs[0].page_content.contains(".. code-block:: python"),
            "First document should contain code-block directive"
        );
        assert!(
            docs[0].page_content.contains(".. warning::"),
            "First document should contain warning directive"
        );
        assert!(
            docs[0].page_content.contains("def hello():"),
            "First document should contain code block content"
        );

        // Content validation - second section
        assert!(
            docs[1].page_content.contains("Normal Section"),
            "Second document should contain 'Normal Section'"
        );
        assert!(
            docs[1].page_content.contains("Some regular content here."),
            "Second document should contain section content"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_nested_sections() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("nested.rst");

        // RST uses different underline characters for section hierarchy
        let rst_content = r#"Main Document Title
===================

Introduction to the document.

Chapter 1: First Chapter
=========================

Chapter introduction.

Section 1.1: First Section
---------------------------

Content of section 1.1.

Subsection 1.1.1: First Subsection
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Content of subsection 1.1.1.

Section 1.2: Second Section
----------------------------

Content of section 1.2.
"#;

        fs::write(&rst_path, rst_content).unwrap();

        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        // Document count validation (all sections detected regardless of hierarchy)
        assert_eq!(
            docs.len(),
            5,
            "Should produce 5 documents for all sections (title, chapter, 2 sections, 1 subsection)"
        );

        // Validate section titles extracted
        assert_eq!(
            docs[0]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Main Document Title"),
            "First section should be 'Main Document Title'"
        );
        assert_eq!(
            docs[1]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Chapter 1: First Chapter"),
            "Second section should be 'Chapter 1: First Chapter'"
        );
        assert_eq!(
            docs[2]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Section 1.1: First Section"),
            "Third section should be 'Section 1.1: First Section'"
        );
        assert_eq!(
            docs[3]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Subsection 1.1.1: First Subsection"),
            "Fourth section should be 'Subsection 1.1.1: First Subsection'"
        );
        assert_eq!(
            docs[4]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Section 1.2: Second Section"),
            "Fifth section should be 'Section 1.2: Second Section'"
        );

        // Validate section indices (0-based, sequential)
        for (i, doc) in docs.iter().enumerate() {
            assert_eq!(
                doc.get_metadata("section_index").and_then(|v| v.as_i64()),
                Some(i as i64),
                "Document {} should have section_index={}",
                i,
                i
            );
        }
    }

    #[tokio::test]
    async fn test_rst_loader_emphasis_and_links() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("markup.rst");

        let rst_content = r#"RST Markup Examples
===================

This document has *emphasis*, **strong emphasis**, and ``code``.

Here is a link: `Python <https://www.python.org/>`_.

And an internal reference link: `Section Two`_.

Section Two
-----------

Content with more markup:
- Bullet list item 1
- Bullet list item 2

1. Numbered list item 1
2. Numbered list item 2
"#;

        fs::write(&rst_path, rst_content).unwrap();

        // Test default mode preserves all markup
        let loader = RSTLoader::new(&rst_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Default mode should produce 1 document");
        assert!(
            docs[0].page_content.contains("*emphasis*"),
            "Document should contain emphasis markup"
        );
        assert!(
            docs[0].page_content.contains("**strong emphasis**"),
            "Document should contain strong emphasis markup"
        );
        assert!(
            docs[0].page_content.contains("``code``"),
            "Document should contain inline code markup"
        );
        assert!(
            docs[0]
                .page_content
                .contains("`Python <https://www.python.org/>`_"),
            "Document should contain external link markup"
        );
        assert!(
            docs[0].page_content.contains("`Section Two`_"),
            "Document should contain internal reference markup"
        );
        assert!(
            docs[0].page_content.contains("- Bullet list"),
            "Document should contain bullet list markup"
        );
        assert!(
            docs[0].page_content.contains("1. Numbered list"),
            "Document should contain numbered list markup"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_very_long_file() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("long.rst");

        // Generate 100 sections (each ~10 lines = ~1000 total lines)
        let mut rst_content = String::from("Long Document\n=============\n\n");
        for i in 0..100 {
            rst_content.push_str(&format!("Section {}\n", i));
            rst_content.push_str(&"-".repeat(20));
            rst_content.push('\n');
            rst_content.push('\n');
            rst_content.push_str(&format!("This is section number {}.\n", i));
            rst_content.push_str("It has some content here.\n");
            rst_content.push_str("And some more lines.\n");
            rst_content.push_str("To make it longer.\n");
            rst_content.push('\n');
        }

        fs::write(&rst_path, &rst_content).unwrap();

        // Test default mode
        let loader = RSTLoader::new(&rst_path);
        let docs = loader.load().await.unwrap();
        assert_eq!(
            docs.len(),
            1,
            "Default mode should produce 1 document for very long file"
        );
        assert!(
            docs[0].page_content.contains("Section 0"),
            "Document should contain first section"
        );
        assert!(
            docs[0].page_content.contains("Section 99"),
            "Document should contain last section"
        );

        // Test separate sections mode
        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();
        assert_eq!(
            docs.len(),
            101,
            "Separate sections mode should produce 101 documents (1 title + 100 sections)"
        );

        // Validate first section (title)
        assert_eq!(
            docs[0]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(0),
            "First section should have section_index=0"
        );

        // Validate last section
        assert_eq!(
            docs[100]
                .get_metadata("section_index")
                .and_then(|v| v.as_i64()),
            Some(100),
            "Last section should have section_index=100"
        );
        assert_eq!(
            docs[100]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Section 99"),
            "Last section should have section_title='Section 99'"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_underline_variations() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("underlines.rst");

        // Test all valid RST underline characters: = - ~ ^ # *
        let rst_content = r#"Title with Equals
=================

Content 1.

Title with Dashes
------------------

Content 2.

Title with Tildes
~~~~~~~~~~~~~~~~~

Content 3.

Title with Carets
^^^^^^^^^^^^^^^^^

Content 4.

Title with Hashes
#################

Content 5.

Title with Asterisks
********************

Content 6.
"#;

        fs::write(&rst_path, rst_content).unwrap();

        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        // Document count validation (6 sections)
        assert_eq!(
            docs.len(),
            6,
            "Should produce 6 documents for 6 different underline styles"
        );

        // Validate each section title
        let expected_titles = [
            "Title with Equals",
            "Title with Dashes",
            "Title with Tildes",
            "Title with Carets",
            "Title with Hashes",
            "Title with Asterisks",
        ];

        for (i, expected_title) in expected_titles.iter().enumerate() {
            assert_eq!(
                docs[i]
                    .get_metadata("section_title")
                    .and_then(|v| v.as_str()),
                Some(*expected_title),
                "Section {} should have title '{}'",
                i,
                expected_title
            );
            assert!(
                docs[i].page_content.contains(expected_title),
                "Section {} content should contain title",
                i
            );
            assert!(
                docs[i]
                    .page_content
                    .contains(&format!("Content {}.", i + 1)),
                "Section {} content should contain 'Content {}.'",
                i,
                i + 1
            );
        }
    }

    #[tokio::test]
    async fn test_rst_loader_malformed_underline() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("malformed.rst");

        // Test cases: underline too short, mixed underline chars (actually valid per implementation)
        // Implementation (line 3600-3602): checks all chars in {=,-,~,^,#,*} and len >= title.trim().len()
        // BUT does NOT check if all chars are the SAME underline character
        let rst_content = r#"Valid Title
===========

Some content here.

Too Short Title
---

More content (this won't be detected as a section because underline too short: 3 < 15).

Mixed Chars
=-=-=-=-=-=

This IS detected as a section (implementation allows mixed underline chars).

Valid Again
-----------

Final content.
"#;

        fs::write(&rst_path, rst_content).unwrap();

        let loader = RSTLoader::new(&rst_path).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        // Document count validation (3 valid sections detected)
        // Implementation requires underline length >= title.trim().len() (line 3602)
        // and all chars must be in {=,-,~,^,#,*} (line 3600-3601)
        // BUT does NOT require all chars to be the same (parser limitation)
        assert_eq!(
            docs.len(),
            3,
            "Should produce 3 documents (Valid Title, Mixed Chars, Valid Again)"
        );

        assert_eq!(
            docs[0]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Valid Title"),
            "First valid section: 'Valid Title'"
        );

        assert_eq!(
            docs[1]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Mixed Chars"),
            "Second section: 'Mixed Chars' (mixed underline chars accepted by parser)"
        );

        assert_eq!(
            docs[2]
                .get_metadata("section_title")
                .and_then(|v| v.as_str()),
            Some("Valid Again"),
            "Third valid section: 'Valid Again'"
        );

        // Verify "Too Short Title" not detected (underline too short)
        assert!(
            docs[0].page_content.contains("Too Short Title"),
            "Too Short Title should be included in previous section (not detected as separate section)"
        );
    }

    #[tokio::test]
    async fn test_rst_loader_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let rst_path = temp_dir.path().join("nonexistent.rst");

        let loader = RSTLoader::new(&rst_path);
        let result = loader.load().await;

        assert!(
            result.is_err(),
            "Loading nonexistent file should return error"
        );
    }
}
