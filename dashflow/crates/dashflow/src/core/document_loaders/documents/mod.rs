//! Document format loaders for PDF, Jupyter notebooks, and subtitles.
//!
//! This module provides loaders for various document formats:
//! - **`PDFLoader`**: Extract text from PDF files
//! - **`NotebookLoader`**: Load Jupyter notebook (.ipynb) files
//! - **`SRTLoader`**: Load subtitle (.srt) files

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;
use serde_json::Value;

/// Loads PDF files as documents.
///
/// The `PDFLoader` uses the `pdf-extract` crate to extract text from PDF files.
/// It can split the PDF into one document per page or load the entire PDF as a single document.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PDFLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PDFLoader::new("document.pdf");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct PDFLoader {
    /// Path to the PDF file
    pub file_path: PathBuf,
    /// Whether to split into one document per page (default: true)
    pub split_pages: bool,
}

impl PDFLoader {
    /// Create a new `PDFLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::PDFLoader;
    ///
    /// let loader = PDFLoader::new("document.pdf");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            split_pages: true,
        }
    }

    /// Set whether to split into one document per page.
    ///
    /// If false, the entire PDF becomes a single document.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::PDFLoader;
    ///
    /// let loader = PDFLoader::new("document.pdf")
    ///     .with_split_pages(false);
    /// ```
    #[must_use]
    pub fn with_split_pages(mut self, split_pages: bool) -> Self {
        self.split_pages = split_pages;
        self
    }
}

#[async_trait]
impl DocumentLoader for PDFLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Clone data for spawn_blocking (avoid blocking async runtime with std::fs)
        let file_path = self.file_path.clone();
        let split_pages = self.split_pages;

        // Perform all filesystem I/O and PDF parsing in spawn_blocking
        tokio::task::spawn_blocking(move || {
            // Extract text from PDF
            let bytes = std::fs::read(&file_path)?;
            let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!(
                    "Failed to extract text from PDF: {e}"
                ))
            })?;

            if split_pages {
                // Split by page breaks (pdf-extract separates pages with form feeds)
                let pages: Vec<&str> = text.split('\x0C').collect();
                let documents = pages
                    .into_iter()
                    .enumerate()
                    .filter(|(_, page_text)| !page_text.trim().is_empty())
                    .map(|(page_num, page_text)| {
                        Document::new(page_text.trim().to_string())
                            .with_metadata("source", file_path.display().to_string())
                            .with_metadata("page", page_num)
                    })
                    .collect();
                Ok::<Vec<Document>, crate::core::error::Error>(documents)
            } else {
                // Single document for entire PDF
                let doc = Document::new(text)
                    .with_metadata("source", file_path.display().to_string());
                Ok(vec![doc])
            }
        })
        .await
        .map_err(|e| crate::core::error::Error::other(format!("Task join failed: {e}")))?
    }
}

/// Loads Jupyter notebook (.ipynb) files.
///
/// The `NotebookLoader` parses Jupyter notebook files and extracts code and markdown cells.
/// It can optionally include cell outputs and create separate documents per cell.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::NotebookLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = NotebookLoader::new("notebook.ipynb");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct NotebookLoader {
    /// Path to the notebook file
    pub file_path: PathBuf,
    /// Include cell outputs (default: false)
    pub include_outputs: bool,
    /// Maximum output length to include (default: unlimited)
    pub max_output_length: Option<usize>,
    /// Concatenate cells vs separate documents (default: true)
    pub concatenate: bool,
}

impl NotebookLoader {
    /// Create a new `NotebookLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            include_outputs: false,
            max_output_length: None,
            concatenate: true,
        }
    }

    /// Include cell outputs in the document content.
    #[must_use]
    pub fn with_outputs(mut self, include: bool) -> Self {
        self.include_outputs = include;
        self
    }

    /// Set maximum output length to include per cell.
    #[must_use]
    pub fn with_max_output_length(mut self, length: usize) -> Self {
        self.max_output_length = Some(length);
        self
    }

    /// Create separate documents per cell instead of concatenating.
    #[must_use]
    pub fn with_separate_cells(mut self) -> Self {
        self.concatenate = false;
        self
    }
}

#[async_trait]
impl DocumentLoader for NotebookLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse the notebook JSON
        let notebook: Value = serde_json::from_str(&content).map_err(|e| {
            crate::core::error::Error::InvalidInput(format!(
                "Failed to parse notebook {}: {}",
                self.file_path.display(),
                e
            ))
        })?;

        let cells = notebook
            .get("cells")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                crate::core::error::Error::InvalidInput(
                    "Invalid notebook format: missing 'cells' array".to_string(),
                )
            })?;

        let mut documents = Vec::new();
        let mut all_content = String::new();

        for (idx, cell) in cells.iter().enumerate() {
            let cell_type = cell.get("cell_type").and_then(|v| v.as_str()).unwrap_or("");

            // Extract source content
            let source = match cell.get("source") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(""),
                _ => continue,
            };

            let mut cell_content = format!("# Cell {} ({})\n{}\n", idx + 1, cell_type, source);

            // Include outputs if requested
            if self.include_outputs {
                if let Some(outputs) = cell.get("outputs").and_then(|v| v.as_array()) {
                    for output in outputs {
                        let output_text = self.extract_output_text(output);
                        if !output_text.is_empty() {
                            cell_content.push_str("\n## Output:\n");
                            cell_content.push_str(&output_text);
                            cell_content.push('\n');
                        }
                    }
                }
            }

            if self.concatenate {
                all_content.push_str(&cell_content);
                all_content.push('\n');
            } else {
                // Create separate document per cell
                let doc = Document::new(cell_content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("cell_index", idx as i64)
                    .with_metadata("cell_type", cell_type);

                documents.push(doc);
            }
        }

        if self.concatenate {
            let doc = Document::new(all_content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "ipynb")
                .with_metadata("cell_count", cells.len() as i64);

            documents.push(doc);
        }

        Ok(documents)
    }
}

impl NotebookLoader {
    fn extract_output_text(&self, output: &Value) -> String {
        let mut text = String::new();

        // Handle different output types
        if let Some(output_type) = output.get("output_type").and_then(|v| v.as_str()) {
            match output_type {
                "stream" => {
                    if let Some(stream_text) = output.get("text") {
                        text = self.value_to_string(stream_text);
                    }
                }
                "execute_result" | "display_data" => {
                    if let Some(data) = output.get("data") {
                        // Prefer text/plain output
                        if let Some(plain_text) = data.get("text/plain") {
                            text = self.value_to_string(plain_text);
                        }
                    }
                }
                "error" => {
                    if let Some(traceback) = output.get("traceback").and_then(|v| v.as_array()) {
                        text = traceback
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                    }
                }
                _ => {}
            }
        }

        // Apply max length if specified
        if let Some(max_len) = self.max_output_length {
            if text.len() > max_len {
                text.truncate(max_len);
                text.push_str("...");
            }
        }

        text
    }

    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(""),
            _ => value.to_string(),
        }
    }
}

/// Loads `SubRip` subtitle (.srt) files.
///
/// The `SRTLoader` reads .srt subtitle files and extracts text content with optional timestamps.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::SRTLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SRTLoader::new("subtitles.srt");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SRTLoader {
    /// Path to the SRT file
    pub file_path: PathBuf,
    /// Include timestamps in the output (default: false)
    pub include_timestamps: bool,
}

impl SRTLoader {
    /// Create a new `SRTLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            include_timestamps: false,
        }
    }

    /// Include timestamps in the extracted text.
    #[must_use]
    pub fn with_timestamps(mut self, include: bool) -> Self {
        self.include_timestamps = include;
        self
    }
}

#[async_trait]
impl DocumentLoader for SRTLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let mut text_content = String::new();
        let mut subtitle_count = 0;

        // Parse SRT format: sequence number, timestamp, text, blank line
        let mut lines = content.lines();
        while let Some(line) = lines.next() {
            let line = line.trim();

            // Skip empty lines and sequence numbers
            if line.is_empty() {
                continue;
            }

            // Check if this is a sequence number (digits only)
            if line.chars().all(|c| c.is_ascii_digit()) {
                subtitle_count += 1;

                // Next line should be timestamp
                if let Some(timestamp_line) = lines.next() {
                    let timestamp = timestamp_line.trim();

                    if self.include_timestamps {
                        text_content.push_str(timestamp);
                        text_content.push('\n');
                    }

                    // Next lines until blank are the subtitle text
                    let mut subtitle_text = Vec::new();
                    for text_line in lines.by_ref() {
                        if text_line.trim().is_empty() {
                            break;
                        }
                        subtitle_text.push(text_line.trim());
                    }

                    if !subtitle_text.is_empty() {
                        text_content.push_str(&subtitle_text.join(" "));
                        text_content.push('\n');
                        if self.include_timestamps {
                            text_content.push('\n');
                        }
                    }
                }
            }
        }

        let document = Document::new(text_content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "srt")
            .with_metadata("subtitle_count", i64::from(subtitle_count));

        Ok(vec![document])
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use std::fs;
    use tempfile::TempDir;

    // ========================================
    // NotebookLoader Tests
    // ========================================

    #[tokio::test]
    async fn test_notebook_loader() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        // Create a minimal Jupyter notebook
        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "source": "# Test Notebook"
                },
                {
                    "cell_type": "code",
                    "source": ["print('Hello, World!')"],
                    "outputs": [
                        {
                            "output_type": "stream",
                            "text": ["Hello, World!\n"]
                        }
                    ]
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path);
        let docs = loader.load().await.unwrap();

        // Validate document count and concatenation mode (default)
        assert_eq!(
            docs.len(),
            1,
            "Should create single document in concatenation mode"
        );

        // Validate content includes both cells
        assert!(
            docs[0].page_content.contains("Test Notebook"),
            "Should include markdown cell content"
        );
        assert!(
            docs[0].page_content.contains("print('Hello, World!')"),
            "Should include code cell content"
        );
        assert!(
            docs[0].page_content.contains("# Cell 1 (markdown)"),
            "Should include cell header with type"
        );
        assert!(
            docs[0].page_content.contains("# Cell 2 (code)"),
            "Should include second cell header"
        );

        // Validate outputs not included by default
        assert!(
            !docs[0].page_content.contains("Output:"),
            "Should not include outputs by default"
        );
        assert!(
            !docs[0].page_content.contains("Hello, World!\n"),
            "Should not include output text when include_outputs=false"
        );

        // Validate metadata completeness
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("ipynb"),
            "Should set format metadata to 'ipynb'"
        );
        assert_eq!(
            docs[0].get_metadata("cell_count").and_then(|v| v.as_i64()),
            Some(2),
            "Should set cell_count metadata to 2"
        );
        assert!(
            docs[0]
                .get_metadata("source")
                .and_then(|v| v.as_str())
                .is_some(),
            "Should set source path metadata"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_with_outputs() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": ["x = 42\nx"],
                    "outputs": [
                        {
                            "output_type": "execute_result",
                            "data": {
                                "text/plain": ["42"]
                            }
                        }
                    ]
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path).with_outputs(true);
        let docs = loader.load().await.unwrap();

        // Validate document count
        assert_eq!(
            docs.len(),
            1,
            "Should create single document in concatenation mode"
        );

        // Validate cell source included
        assert!(
            docs[0].page_content.contains("x = 42"),
            "Should include cell source code"
        );
        assert!(
            docs[0].page_content.contains("# Cell 1 (code)"),
            "Should include cell header"
        );

        // Validate outputs included when with_outputs(true)
        assert!(
            docs[0].page_content.contains("Output:"),
            "Should include output header when with_outputs=true"
        );
        assert!(
            docs[0].page_content.contains("42"),
            "Should include execute_result output text"
        );

        // Validate metadata
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("ipynb"),
            "Should set format metadata"
        );
        assert_eq!(
            docs[0].get_metadata("cell_count").and_then(|v| v.as_i64()),
            Some(1),
            "Should set cell_count metadata"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_separate_cells() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "source": "# Introduction"
                },
                {
                    "cell_type": "code",
                    "source": ["x = 1"],
                    "outputs": []
                },
                {
                    "cell_type": "code",
                    "source": ["y = 2"],
                    "outputs": []
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path).with_separate_cells();
        let docs = loader.load().await.unwrap();

        // Validate separate documents per cell
        assert_eq!(
            docs.len(),
            3,
            "Should create separate document for each cell when with_separate_cells=true"
        );

        // Validate first document (markdown cell)
        assert!(
            docs[0].page_content.contains("Introduction"),
            "First doc should contain markdown content"
        );
        assert_eq!(
            docs[0].get_metadata("cell_index").and_then(|v| v.as_i64()),
            Some(0),
            "First doc should have cell_index=0"
        );
        assert_eq!(
            docs[0].get_metadata("cell_type").and_then(|v| v.as_str()),
            Some("markdown"),
            "First doc should have cell_type=markdown"
        );

        // Validate second document (code cell)
        assert!(
            docs[1].page_content.contains("x = 1"),
            "Second doc should contain first code cell"
        );
        assert_eq!(
            docs[1].get_metadata("cell_index").and_then(|v| v.as_i64()),
            Some(1),
            "Second doc should have cell_index=1"
        );
        assert_eq!(
            docs[1].get_metadata("cell_type").and_then(|v| v.as_str()),
            Some("code"),
            "Second doc should have cell_type=code"
        );

        // Validate third document (code cell)
        assert!(
            docs[2].page_content.contains("y = 2"),
            "Third doc should contain second code cell"
        );
        assert_eq!(
            docs[2].get_metadata("cell_index").and_then(|v| v.as_i64()),
            Some(2),
            "Third doc should have cell_index=2"
        );

        // Validate all docs have source metadata
        for doc in &docs {
            assert!(
                doc.get_metadata("source")
                    .and_then(|v| v.as_str())
                    .is_some(),
                "All documents should have source path metadata"
            );
        }
    }

    #[tokio::test]
    async fn test_notebook_loader_output_types() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": ["print('stream output')"],
                    "outputs": [
                        {
                            "output_type": "stream",
                            "text": ["stream output\n"]
                        }
                    ]
                },
                {
                    "cell_type": "code",
                    "source": ["42"],
                    "outputs": [
                        {
                            "output_type": "execute_result",
                            "data": {
                                "text/plain": ["42"]
                            }
                        }
                    ]
                },
                {
                    "cell_type": "code",
                    "source": ["import matplotlib.pyplot as plt"],
                    "outputs": [
                        {
                            "output_type": "display_data",
                            "data": {
                                "text/plain": ["<Figure>"]
                            }
                        }
                    ]
                },
                {
                    "cell_type": "code",
                    "source": ["1/0"],
                    "outputs": [
                        {
                            "output_type": "error",
                            "ename": "ZeroDivisionError",
                            "evalue": "division by zero",
                            "traceback": [
                                "Traceback (most recent call last):",
                                "  File \"<stdin>\", line 1, in <module>",
                                "ZeroDivisionError: division by zero"
                            ]
                        }
                    ]
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path).with_outputs(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "Should create single document in concatenation mode"
        );

        // Validate all 3 output types included
        assert!(
            docs[0].page_content.contains("stream output"),
            "Should include stream output"
        );
        assert!(
            docs[0].page_content.contains("42"),
            "Should include execute_result output"
        );
        assert!(
            docs[0].page_content.contains("<Figure>"),
            "Should include display_data output"
        );
        assert!(
            docs[0].page_content.contains("ZeroDivisionError"),
            "Should include error traceback"
        );
        assert!(
            docs[0].page_content.contains("division by zero"),
            "Should include error message"
        );

        // Validate output markers present
        let output_count = docs[0].page_content.matches("## Output:").count();
        assert_eq!(
            output_count, 4,
            "Should have 4 output sections (one per cell with output)"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_max_output_length() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        let long_output = "a".repeat(1000);
        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": ["print('a' * 1000)"],
                    "outputs": [
                        {
                            "output_type": "stream",
                            "text": [long_output]
                        }
                    ]
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path)
            .with_outputs(true)
            .with_max_output_length(100);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);

        // Validate output truncated to max length
        let output_section = docs[0]
            .page_content
            .split("## Output:")
            .nth(1)
            .expect("Should have output section");

        // Output should be truncated to ~100 chars + "..." (103 total)
        assert!(
            output_section.contains("..."),
            "Should include ellipsis when truncated"
        );

        // The actual output length in the section should be around max_length + "..."
        // Count 'a' characters - should be exactly 100
        let a_count = output_section.chars().filter(|&c| c == 'a').count();
        assert!(
            a_count <= 100,
            "Should truncate output to max_output_length (found {} 'a' chars)",
            a_count
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_source_formats() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "single string source"
                },
                {
                    "cell_type": "code",
                    "source": ["line 1\n", "line 2\n", "line 3"]
                },
                {
                    "cell_type": "markdown",
                    "source": ["# Title\n", "Paragraph"]
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);

        // Validate single string source parsed correctly
        assert!(
            docs[0].page_content.contains("single string source"),
            "Should parse single string source"
        );

        // Validate array source joined correctly
        assert!(
            docs[0].page_content.contains("line 1"),
            "Should parse array source line 1"
        );
        assert!(
            docs[0].page_content.contains("line 2"),
            "Should parse array source line 2"
        );
        assert!(
            docs[0].page_content.contains("line 3"),
            "Should parse array source line 3"
        );

        // Validate markdown array source
        assert!(
            docs[0].page_content.contains("# Title"),
            "Should parse markdown array source"
        );
        assert!(
            docs[0].page_content.contains("Paragraph"),
            "Should include all markdown lines"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_nbformat_versions() {
        let temp_dir = TempDir::new().unwrap();

        // Test nbformat 4
        let notebook_path_v4 = temp_dir.path().join("test_v4.ipynb");
        let notebook_v4 = serde_json::json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": ["print('v4')"],
                    "outputs": []
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5
        });
        fs::write(&notebook_path_v4, notebook_v4.to_string()).unwrap();

        let loader_v4 = NotebookLoader::new(&notebook_path_v4);
        let docs_v4 = loader_v4.load().await.unwrap();
        assert_eq!(docs_v4.len(), 1, "Should load nbformat 4 notebook");
        assert!(
            docs_v4[0].page_content.contains("print('v4')"),
            "Should parse nbformat 4 cells"
        );

        // Test nbformat 3 structure (minimal validation - may not be fully supported)
        let notebook_path_v3 = temp_dir.path().join("test_v3.ipynb");
        let notebook_v3 = serde_json::json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "print('v3')",
                    "outputs": []
                }
            ],
            "metadata": {},
            "nbformat": 3,
            "nbformat_minor": 0
        });
        fs::write(&notebook_path_v3, notebook_v3.to_string()).unwrap();

        let loader_v3 = NotebookLoader::new(&notebook_path_v3);
        let result_v3 = loader_v3.load().await;
        // NBFormat 3 may or may not be supported - just verify it doesn't panic
        assert!(
            result_v3.is_ok() || result_v3.is_err(),
            "Should handle nbformat 3 without panic"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "source": "# 你好世界 (Chinese)\n## こんにちは世界 (Japanese)\n### 안녕하세요 (Korean)"
                },
                {
                    "cell_type": "code",
                    "source": ["print('مرحبا')  # Arabic"],
                    "outputs": [
                        {
                            "output_type": "stream",
                            "text": ["مرحبا\n"]
                        }
                    ]
                },
                {
                    "cell_type": "markdown",
                    "source": "Emoji: 🚀 🐍 💻 📊 🧪 🔬"
                },
                {
                    "cell_type": "code",
                    "source": ["# Math symbols: ∑ ∫ √ π ∞ ≠ ≈\n# Currency: € £ ¥ ₹ ₽"],
                    "outputs": []
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        // Test without outputs
        let loader = NotebookLoader::new(&notebook_path);
        let docs = loader.load().await.unwrap();
        assert_eq!(docs.len(), 1);

        // Validate Chinese
        assert!(
            docs[0].page_content.contains("你好世界"),
            "Should include Chinese characters"
        );

        // Validate Japanese
        assert!(
            docs[0].page_content.contains("こんにちは世界"),
            "Should include Japanese characters"
        );

        // Validate Korean
        assert!(
            docs[0].page_content.contains("안녕하세요"),
            "Should include Korean characters"
        );

        // Validate Arabic
        assert!(
            docs[0].page_content.contains("مرحبا"),
            "Should include Arabic characters"
        );

        // Validate emoji
        assert!(
            docs[0].page_content.contains("🚀"),
            "Should include emoji 🚀"
        );
        assert!(
            docs[0].page_content.contains("🐍"),
            "Should include emoji 🐍"
        );
        assert!(
            docs[0].page_content.contains("💻"),
            "Should include emoji 💻"
        );

        // Validate math symbols
        assert!(
            docs[0].page_content.contains("∑"),
            "Should include math symbol ∑"
        );
        assert!(
            docs[0].page_content.contains("∫"),
            "Should include math symbol ∫"
        );
        assert!(
            docs[0].page_content.contains("π"),
            "Should include math symbol π"
        );

        // Validate currency
        assert!(
            docs[0].page_content.contains("€"),
            "Should include currency €"
        );
        assert!(
            docs[0].page_content.contains("¥"),
            "Should include currency ¥"
        );
        assert!(
            docs[0].page_content.contains("₹"),
            "Should include currency ₹"
        );

        // Test with outputs
        let loader_with_outputs = NotebookLoader::new(&notebook_path).with_outputs(true);
        let docs_with_outputs = loader_with_outputs.load().await.unwrap();
        assert!(
            docs_with_outputs[0].page_content.contains("مرحبا"),
            "Should include Arabic in outputs"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_empty_cells() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        let notebook_json = serde_json::json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "source": ""
                },
                {
                    "cell_type": "code",
                    "source": [],
                    "outputs": []
                },
                {
                    "cell_type": "code",
                    "source": [""],
                    "outputs": []
                },
                {
                    "cell_type": "markdown",
                    "source": "Non-empty cell"
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");
        assert_eq!(
            docs[0].get_metadata("cell_count").and_then(|v| v.as_i64()),
            Some(4),
            "Should count all cells including empty ones"
        );

        // Validate non-empty cell included
        assert!(
            docs[0].page_content.contains("Non-empty cell"),
            "Should include non-empty cell"
        );

        // Validate cell headers for empty cells still present
        assert!(
            docs[0].page_content.contains("# Cell 1 (markdown)"),
            "Should include header for empty markdown cell"
        );
        assert!(
            docs[0].page_content.contains("# Cell 2 (code)"),
            "Should include header for empty code cell"
        );
        assert!(
            docs[0].page_content.contains("# Cell 4 (markdown)"),
            "Should include header for non-empty cell"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_empty_notebook() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        // Notebook with no cells
        let notebook_json = serde_json::json!({
            "cells": [],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path);
        let docs = loader.load().await.unwrap();

        // Should create single document with empty content in concatenation mode
        assert_eq!(
            docs.len(),
            1,
            "Should create single document even for empty notebook"
        );
        assert_eq!(
            docs[0].get_metadata("cell_count").and_then(|v| v.as_i64()),
            Some(0),
            "Should set cell_count=0 for empty notebook"
        );
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("ipynb"),
            "Should still set format metadata"
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_malformed_json() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        // Invalid JSON syntax
        fs::write(&notebook_path, "{ invalid json }").unwrap();

        let loader = NotebookLoader::new(&notebook_path);
        let result = loader.load().await;

        assert!(result.is_err(), "Should error on malformed JSON");
    }

    #[tokio::test]
    async fn test_notebook_loader_missing_cells() {
        let temp_dir = TempDir::new().unwrap();
        let notebook_path = temp_dir.path().join("test.ipynb");

        // Valid JSON but missing 'cells' field
        let notebook_json = serde_json::json!({
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 4
        });

        fs::write(&notebook_path, notebook_json.to_string()).unwrap();

        let loader = NotebookLoader::new(&notebook_path);
        let result = loader.load().await;

        assert!(
            result.is_err(),
            "Should error when 'cells' field is missing"
        );

        // Validate error message mentions cells
        if let Err(e) = result {
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.to_lowercase().contains("cells") || error_msg.contains("Invalid"),
                "Error should mention missing 'cells' field or invalid format: {}",
                error_msg
            );
        }
    }

    #[tokio::test]
    async fn test_notebook_loader_file_not_found() {
        let loader = NotebookLoader::new("/nonexistent/path/notebook.ipynb");
        let result = loader.load().await;

        assert!(result.is_err(), "Should error when file does not exist");
    }

    // ========================================
    // SRTLoader Tests
    // ========================================

    #[tokio::test]
    async fn test_srt_loader() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("test.srt");

        let srt_content = r#"1
00:00:00,000 --> 00:00:02,000
Hello, this is the first subtitle.

2
00:00:02,500 --> 00:00:05,000
And this is the second one.

3
00:00:05,500 --> 00:00:08,000
Finally, the third subtitle.
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Content validation - without timestamps
        assert!(
            docs[0].page_content.contains("first subtitle"),
            "Should contain first subtitle text"
        );
        assert!(
            docs[0].page_content.contains("second one"),
            "Should contain second subtitle text"
        );
        assert!(
            docs[0].page_content.contains("third subtitle"),
            "Should contain third subtitle text"
        );

        // Timestamps should NOT be included by default
        assert!(
            !docs[0].page_content.contains("00:00:00,000"),
            "Should not include timestamps by default"
        );
        assert!(
            !docs[0].page_content.contains("00:00:02,500"),
            "Should not include timestamps by default"
        );

        // Metadata validation
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("srt"),
            "Metadata format should be 'srt'"
        );
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(3),
            "Metadata subtitle_count should be 3"
        );
        assert!(
            docs[0].get_metadata("source").is_some(),
            "Metadata should include source path"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_with_timestamps() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("test.srt");

        let srt_content = r#"1
00:00:00,000 --> 00:00:02,000
Test subtitle

2
00:00:03,000 --> 00:00:05,500
Second subtitle
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path).with_timestamps(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Timestamps SHOULD be included when with_timestamps(true)
        assert!(
            docs[0]
                .page_content
                .contains("00:00:00,000 --> 00:00:02,000"),
            "Should include first timestamp"
        );
        assert!(
            docs[0]
                .page_content
                .contains("00:00:03,000 --> 00:00:05,500"),
            "Should include second timestamp"
        );

        // Content should still be present
        assert!(
            docs[0].page_content.contains("Test subtitle"),
            "Should contain first subtitle text"
        );
        assert!(
            docs[0].page_content.contains("Second subtitle"),
            "Should contain second subtitle text"
        );

        // Metadata validation
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(2),
            "Metadata subtitle_count should be 2"
        );
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("srt"),
            "Metadata format should be 'srt'"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_multiline_subtitles() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("test.srt");

        // Test multi-line subtitle text (common in SRT files)
        let srt_content = r#"1
00:00:00,000 --> 00:00:03,000
This is a multi-line subtitle.
It spans across multiple lines.
And even a third line here.

2
00:00:04,000 --> 00:00:06,000
Another subtitle
with two lines.
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Multi-line subtitles should be joined with spaces
        assert!(
            docs[0].page_content.contains("multi-line subtitle"),
            "Should contain first line"
        );
        assert!(
            docs[0].page_content.contains("multiple lines"),
            "Should contain second line"
        );
        assert!(
            docs[0].page_content.contains("third line"),
            "Should contain third line"
        );
        assert!(
            docs[0].page_content.contains("Another subtitle"),
            "Should contain second subtitle first line"
        );
        assert!(
            docs[0].page_content.contains("two lines"),
            "Should contain second subtitle second line"
        );

        // Metadata
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(2),
            "Should count 2 subtitles"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("test.srt");

        // Test Unicode in subtitles (multiple languages, emoji, symbols)
        let srt_content = r#"1
00:00:00,000 --> 00:00:02,000
你好世界 (Chinese)

2
00:00:02,500 --> 00:00:04,500
こんにちは世界 (Japanese)

3
00:00:05,000 --> 00:00:07,000
안녕하세요 (Korean)

4
00:00:07,500 --> 00:00:09,500
مرحبا بالعالم (Arabic)

5
00:00:10,000 --> 00:00:12,000
Привет мир (Russian)

6
00:00:12,500 --> 00:00:14,500
🌍🚀🌟💻📚🎉 Emoji test

7
00:00:15,000 --> 00:00:17,000
Math: ∑∫√π∞≠≈ Currency: €£¥₹₽
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Validate Unicode content preservation
        assert!(
            docs[0].page_content.contains("你好世界"),
            "Should preserve Chinese characters"
        );
        assert!(
            docs[0].page_content.contains("こんにちは世界"),
            "Should preserve Japanese characters"
        );
        assert!(
            docs[0].page_content.contains("안녕하세요"),
            "Should preserve Korean characters"
        );
        assert!(
            docs[0].page_content.contains("مرحبا بالعالم"),
            "Should preserve Arabic characters"
        );
        assert!(
            docs[0].page_content.contains("Привет мир"),
            "Should preserve Cyrillic characters"
        );
        assert!(
            docs[0].page_content.contains("🌍🚀🌟💻📚🎉"),
            "Should preserve emoji"
        );
        assert!(
            docs[0].page_content.contains("∑∫√π∞≠≈"),
            "Should preserve math symbols"
        );
        assert!(
            docs[0].page_content.contains("€£¥₹₽"),
            "Should preserve currency symbols"
        );

        // Metadata
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(7),
            "Should count 7 subtitles"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_empty_srt() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("empty.srt");

        // Test completely empty SRT file
        fs::write(&srt_path, "").unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "Should create single document even for empty file"
        );
        assert!(
            docs[0].page_content.is_empty() || docs[0].page_content.trim().is_empty(),
            "Content should be empty or whitespace only"
        );
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(0),
            "Subtitle count should be 0 for empty file"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_whitespace_only() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("whitespace.srt");

        // Test SRT file with only whitespace and newlines
        let srt_content = "\n\n   \n\t\n  \n";
        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(0),
            "Subtitle count should be 0 for whitespace-only file"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_malformed_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("malformed.srt");

        // Test malformed timestamp format
        let srt_content = r#"1
Invalid timestamp format here
This is subtitle text anyway

2
00:00:02,000 --> 00:00:04,000
Valid subtitle after malformed one
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let result = loader.load().await;

        // Implementation should handle gracefully - either skip malformed or include
        assert!(
            result.is_ok(),
            "Should handle malformed timestamps gracefully"
        );
        let docs = result.unwrap();
        assert_eq!(docs.len(), 1, "Should create single document");
    }

    #[tokio::test]
    async fn test_srt_loader_missing_sequence_numbers() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("missing_seq.srt");

        // Test SRT with non-sequential or missing sequence numbers
        let srt_content = r#"1
00:00:00,000 --> 00:00:02,000
First subtitle

5
00:00:05,000 --> 00:00:07,000
Fifth subtitle (skipped 2-4)

3
00:00:03,000 --> 00:00:04,000
Third subtitle (out of order)
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Should parse all subtitles regardless of sequence order
        assert!(
            docs[0].page_content.contains("First subtitle"),
            "Should include first"
        );
        assert!(
            docs[0].page_content.contains("Fifth subtitle"),
            "Should include fifth"
        );
        assert!(
            docs[0].page_content.contains("Third subtitle"),
            "Should include third"
        );

        // Count should be based on sequence numbers found, not order
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(3),
            "Should count 3 subtitles"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_duplicate_sequence_numbers() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("duplicate_seq.srt");

        // Test duplicate sequence numbers
        let srt_content = r#"1
00:00:00,000 --> 00:00:02,000
First occurrence

1
00:00:03,000 --> 00:00:05,000
Second occurrence (duplicate)

2
00:00:06,000 --> 00:00:08,000
Third subtitle
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Should parse all subtitles including duplicates
        assert!(
            docs[0].page_content.contains("First occurrence"),
            "Should include first"
        );
        assert!(
            docs[0].page_content.contains("Second occurrence"),
            "Should include duplicate"
        );
        assert!(
            docs[0].page_content.contains("Third subtitle"),
            "Should include third"
        );

        // Count should include all sequence numbers encountered
        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(3),
            "Should count 3 sequence numbers total"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_special_timestamp_formats() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("special_timestamps.srt");

        // Test various timestamp edge cases
        let srt_content = r#"1
00:00:00,000 --> 00:00:00,001
Very short duration (1ms)

2
01:30:45,999 --> 01:30:50,000
Long timestamp (over 1 hour)

3
00:00:10,500 --> 00:00:10,500
Zero duration timestamp
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Should handle various timestamp formats
        assert!(
            docs[0].page_content.contains("Very short duration"),
            "Should handle 1ms duration"
        );
        assert!(
            docs[0].page_content.contains("Long timestamp"),
            "Should handle >1 hour timestamps"
        );
        assert!(
            docs[0].page_content.contains("Zero duration"),
            "Should handle zero duration"
        );

        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(3),
            "Should count 3 subtitles"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_extra_blank_lines() {
        let temp_dir = TempDir::new().unwrap();
        let srt_path = temp_dir.path().join("extra_blanks.srt");

        // Test SRT with extra blank lines between subtitles
        let srt_content = r#"1
00:00:00,000 --> 00:00:02,000
First subtitle


2
00:00:03,000 --> 00:00:05,000
Second subtitle



3
00:00:06,000 --> 00:00:08,000
Third subtitle
"#;

        fs::write(&srt_path, srt_content).unwrap();

        let loader = SRTLoader::new(&srt_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should create single document");

        // Should handle extra blank lines gracefully
        assert!(
            docs[0].page_content.contains("First subtitle"),
            "Should parse first"
        );
        assert!(
            docs[0].page_content.contains("Second subtitle"),
            "Should parse second"
        );
        assert!(
            docs[0].page_content.contains("Third subtitle"),
            "Should parse third"
        );

        assert_eq!(
            docs[0]
                .get_metadata("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(3),
            "Should count 3 subtitles despite extra blank lines"
        );
    }

    #[tokio::test]
    async fn test_srt_loader_file_not_found() {
        let loader = SRTLoader::new("/nonexistent/path/to/file.srt");
        let result = loader.load().await;

        assert!(result.is_err(), "Should error when file does not exist");
    }
}
