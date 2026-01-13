//! Media format document loaders.
//!
//! This module provides loaders for media-related file formats:
//! - Jupyter notebooks (.ipynb)
//! - Subtitle formats (SRT, `WebVTT`)

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};

// ============================================================================
// Jupyter Notebook Loader
// ============================================================================

/// Loads Jupyter notebook (.ipynb) files.
///
/// The `NotebookLoader` reads .ipynb files (JSON format) and extracts code and markdown cells.
/// Each notebook is loaded as a single document with cells concatenated by default.
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
        let notebook: Value = serde_json::from_str(&content)?;

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

// ============================================================================
// SRT (SubRip) Loader
// ============================================================================

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

// ============================================================================
// WebVTT Loader
// ============================================================================

/// Loads `WebVTT` subtitle/caption files used for web video.
/// `WebVTT` is a standard format for displaying timed text tracks.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::WebVTTLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = WebVTTLoader::new("video_subtitles.vtt");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct WebVTTLoader {
    file_path: PathBuf,
}

impl WebVTTLoader {
    /// Create a new `WebVTT` loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for WebVTTLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // WebVTT format:
        // WEBVTT
        //
        // 00:00:00.000 --> 00:00:02.000
        // Subtitle text here
        //
        // Extract just the text, removing timestamps and cue identifiers

        let mut text_lines = Vec::new();
        let mut in_cue_text = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip header line
            if trimmed.starts_with("WEBVTT") || trimmed.is_empty() {
                in_cue_text = false;
                continue;
            }

            // Skip timestamp lines (contain -->)
            if trimmed.contains("-->") {
                in_cue_text = true;
                continue;
            }

            // Skip cue identifiers (lines before timestamps that don't contain text)
            // If we're not in a cue and the line doesn't look like a timestamp, it might be text
            if in_cue_text && !trimmed.is_empty() {
                text_lines.push(trimmed.to_string());
            }
        }

        let text_content = text_lines.join(" ");

        Ok(vec![Document::new(text_content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "webvtt")
            .with_metadata("type", "subtitles")])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ==========================================================================
    // NotebookLoader Tests
    // ==========================================================================

    #[test]
    fn test_notebook_loader_new() {
        let loader = NotebookLoader::new("notebook.ipynb");
        assert_eq!(loader.file_path, PathBuf::from("notebook.ipynb"));
        assert!(!loader.include_outputs);
        assert!(loader.max_output_length.is_none());
        assert!(loader.concatenate);
    }

    #[test]
    fn test_notebook_loader_new_from_pathbuf() {
        let path = PathBuf::from("/data/analysis.ipynb");
        let loader = NotebookLoader::new(&path);
        assert_eq!(loader.file_path, path);
    }

    #[test]
    fn test_notebook_loader_with_outputs() {
        let loader = NotebookLoader::new("nb.ipynb").with_outputs(true);
        assert!(loader.include_outputs);
    }

    #[test]
    fn test_notebook_loader_with_outputs_false() {
        let loader = NotebookLoader::new("nb.ipynb").with_outputs(false);
        assert!(!loader.include_outputs);
    }

    #[test]
    fn test_notebook_loader_with_max_output_length() {
        let loader = NotebookLoader::new("nb.ipynb").with_max_output_length(1000);
        assert_eq!(loader.max_output_length, Some(1000));
    }

    #[test]
    fn test_notebook_loader_with_separate_cells() {
        let loader = NotebookLoader::new("nb.ipynb").with_separate_cells();
        assert!(!loader.concatenate);
    }

    #[test]
    fn test_notebook_loader_builder_chain() {
        let loader = NotebookLoader::new("nb.ipynb")
            .with_outputs(true)
            .with_max_output_length(500)
            .with_separate_cells();

        assert!(loader.include_outputs);
        assert_eq!(loader.max_output_length, Some(500));
        assert!(!loader.concatenate);
    }

    #[test]
    fn test_notebook_loader_debug_clone() {
        let loader = NotebookLoader::new("test.ipynb");
        let cloned = loader.clone();
        assert_eq!(loader.file_path, cloned.file_path);
        assert_eq!(loader.include_outputs, cloned.include_outputs);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("NotebookLoader"));
        assert!(debug_str.contains("test.ipynb"));
    }

    fn create_simple_notebook(cells: &[(&str, &str)]) -> String {
        let cells_json: Vec<String> = cells
            .iter()
            .map(|(cell_type, source)| {
                format!(
                    r#"{{"cell_type": "{}", "source": {}, "metadata": {{}}, "outputs": []}}"#,
                    cell_type,
                    serde_json::json!(source)
                )
            })
            .collect();

        format!(
            r#"{{"nbformat": 4, "nbformat_minor": 2, "metadata": {{}}, "cells": [{}]}}"#,
            cells_json.join(",")
        )
    }

    #[tokio::test]
    async fn test_notebook_loader_load_basic() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        let notebook = create_simple_notebook(&[
            ("code", "print('hello')"),
            ("markdown", "# Header"),
        ]);
        temp_file.write_all(notebook.as_bytes()).unwrap();

        let loader = NotebookLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("print('hello')"));
        assert!(docs[0].page_content.contains("# Header"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("ipynb")
        );
        assert_eq!(
            docs[0].metadata.get("cell_count").and_then(|v| v.as_i64()),
            Some(2)
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_separate_cells() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        let notebook = create_simple_notebook(&[
            ("code", "x = 1"),
            ("code", "y = 2"),
            ("markdown", "## Notes"),
        ]);
        temp_file.write_all(notebook.as_bytes()).unwrap();

        let loader = NotebookLoader::new(temp_file.path()).with_separate_cells();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(
            docs[0].metadata.get("cell_type").and_then(|v| v.as_str()),
            Some("code")
        );
        assert_eq!(
            docs[0].metadata.get("cell_index").and_then(|v| v.as_i64()),
            Some(0)
        );
        assert_eq!(
            docs[2].metadata.get("cell_type").and_then(|v| v.as_str()),
            Some("markdown")
        );
    }

    #[tokio::test]
    async fn test_notebook_loader_load_with_outputs_enabled() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        let notebook = r#"{
            "nbformat": 4,
            "nbformat_minor": 2,
            "metadata": {},
            "cells": [{
                "cell_type": "code",
                "source": "print('hello')",
                "metadata": {},
                "outputs": [{
                    "output_type": "stream",
                    "name": "stdout",
                    "text": "hello\n"
                }]
            }]
        }"#;
        temp_file.write_all(notebook.as_bytes()).unwrap();

        let loader = NotebookLoader::new(temp_file.path()).with_outputs(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("## Output:"));
        assert!(docs[0].page_content.contains("hello"));
    }

    #[tokio::test]
    async fn test_notebook_loader_execute_result_output() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        let notebook = r#"{
            "nbformat": 4,
            "nbformat_minor": 2,
            "metadata": {},
            "cells": [{
                "cell_type": "code",
                "source": "42",
                "metadata": {},
                "outputs": [{
                    "output_type": "execute_result",
                    "data": {"text/plain": "42"},
                    "execution_count": 1
                }]
            }]
        }"#;
        temp_file.write_all(notebook.as_bytes()).unwrap();

        let loader = NotebookLoader::new(temp_file.path()).with_outputs(true);
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("42"));
    }

    #[tokio::test]
    async fn test_notebook_loader_error_output() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        let notebook = r#"{
            "nbformat": 4,
            "nbformat_minor": 2,
            "metadata": {},
            "cells": [{
                "cell_type": "code",
                "source": "raise ValueError()",
                "metadata": {},
                "outputs": [{
                    "output_type": "error",
                    "ename": "ValueError",
                    "evalue": "",
                    "traceback": ["ValueError", "at line 1"]
                }]
            }]
        }"#;
        temp_file.write_all(notebook.as_bytes()).unwrap();

        let loader = NotebookLoader::new(temp_file.path()).with_outputs(true);
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("ValueError"));
    }

    #[tokio::test]
    async fn test_notebook_loader_max_output_length() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        let long_output = "x".repeat(100);
        let notebook = format!(
            r#"{{
            "nbformat": 4,
            "nbformat_minor": 2,
            "metadata": {{}},
            "cells": [{{
                "cell_type": "code",
                "source": "print('x' * 100)",
                "metadata": {{}},
                "outputs": [{{
                    "output_type": "stream",
                    "name": "stdout",
                    "text": "{}"
                }}]
            }}]
        }}"#,
            long_output
        );
        temp_file.write_all(notebook.as_bytes()).unwrap();

        let loader = NotebookLoader::new(temp_file.path())
            .with_outputs(true)
            .with_max_output_length(20);
        let docs = loader.load().await.unwrap();

        // Should be truncated with "..."
        assert!(docs[0].page_content.contains("..."));
        // Should not contain full 100 x's
        assert!(!docs[0].page_content.contains(&"x".repeat(100)));
    }

    #[tokio::test]
    async fn test_notebook_loader_source_as_array() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        let notebook = r#"{
            "nbformat": 4,
            "nbformat_minor": 2,
            "metadata": {},
            "cells": [{
                "cell_type": "code",
                "source": ["line1\n", "line2\n", "line3"],
                "metadata": {},
                "outputs": []
            }]
        }"#;
        temp_file.write_all(notebook.as_bytes()).unwrap();

        let loader = NotebookLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("line1"));
        assert!(docs[0].page_content.contains("line2"));
        assert!(docs[0].page_content.contains("line3"));
    }

    #[tokio::test]
    async fn test_notebook_loader_invalid_json() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        temp_file.write_all(b"not valid json").unwrap();

        let loader = NotebookLoader::new(temp_file.path());
        let result = loader.load().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_notebook_loader_missing_cells() {
        let mut temp_file = NamedTempFile::with_suffix(".ipynb").unwrap();
        temp_file
            .write_all(br#"{"nbformat": 4, "metadata": {}}"#)
            .unwrap();

        let loader = NotebookLoader::new(temp_file.path());
        let result = loader.load().await;

        assert!(result.is_err());
    }

    // ==========================================================================
    // SRTLoader Tests
    // ==========================================================================

    #[test]
    fn test_srt_loader_new() {
        let loader = SRTLoader::new("subtitles.srt");
        assert_eq!(loader.file_path, PathBuf::from("subtitles.srt"));
        assert!(!loader.include_timestamps);
    }

    #[test]
    fn test_srt_loader_new_from_pathbuf() {
        let path = PathBuf::from("/video/movie.srt");
        let loader = SRTLoader::new(&path);
        assert_eq!(loader.file_path, path);
    }

    #[test]
    fn test_srt_loader_with_timestamps_true() {
        let loader = SRTLoader::new("sub.srt").with_timestamps(true);
        assert!(loader.include_timestamps);
    }

    #[test]
    fn test_srt_loader_with_timestamps_false() {
        let loader = SRTLoader::new("sub.srt").with_timestamps(false);
        assert!(!loader.include_timestamps);
    }

    #[test]
    fn test_srt_loader_debug_clone() {
        let loader = SRTLoader::new("test.srt");
        let cloned = loader.clone();
        assert_eq!(loader.file_path, cloned.file_path);
        assert_eq!(loader.include_timestamps, cloned.include_timestamps);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("SRTLoader"));
        assert!(debug_str.contains("test.srt"));
    }

    #[tokio::test]
    async fn test_srt_loader_load_basic() {
        let mut temp_file = NamedTempFile::with_suffix(".srt").unwrap();
        let srt_content = r#"1
00:00:01,000 --> 00:00:04,000
Hello, world!

2
00:00:05,000 --> 00:00:08,000
This is a test.

"#;
        temp_file.write_all(srt_content.as_bytes()).unwrap();

        let loader = SRTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Hello, world!"));
        assert!(docs[0].page_content.contains("This is a test."));
        // Without timestamps option
        assert!(!docs[0].page_content.contains("00:00:01,000"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("srt")
        );
        assert_eq!(
            docs[0]
                .metadata
                .get("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(2)
        );
    }

    #[tokio::test]
    async fn test_srt_loader_with_timestamps() {
        let mut temp_file = NamedTempFile::with_suffix(".srt").unwrap();
        let srt_content = r#"1
00:00:01,500 --> 00:00:04,000
First subtitle

2
00:00:05,000 --> 00:00:08,500
Second subtitle

"#;
        temp_file.write_all(srt_content.as_bytes()).unwrap();

        let loader = SRTLoader::new(temp_file.path()).with_timestamps(true);
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("00:00:01,500"));
        assert!(docs[0].page_content.contains("First subtitle"));
    }

    #[tokio::test]
    async fn test_srt_loader_multiline_subtitle() {
        let mut temp_file = NamedTempFile::with_suffix(".srt").unwrap();
        let srt_content = r#"1
00:00:00,000 --> 00:00:05,000
Line one
Line two
Line three

"#;
        temp_file.write_all(srt_content.as_bytes()).unwrap();

        let loader = SRTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        // Lines should be joined with spaces
        assert!(docs[0].page_content.contains("Line one Line two Line three"));
    }

    #[tokio::test]
    async fn test_srt_loader_empty_file() {
        let temp_file = NamedTempFile::with_suffix(".srt").unwrap();

        let loader = SRTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0]
                .metadata
                .get("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(0)
        );
    }

    #[tokio::test]
    async fn test_srt_loader_single_subtitle() {
        let mut temp_file = NamedTempFile::with_suffix(".srt").unwrap();
        let srt_content = r#"1
00:00:00,000 --> 00:00:02,000
Only one subtitle here"#;
        temp_file.write_all(srt_content.as_bytes()).unwrap();

        let loader = SRTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("Only one subtitle"));
        assert_eq!(
            docs[0]
                .metadata
                .get("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(1)
        );
    }

    // ==========================================================================
    // WebVTTLoader Tests
    // ==========================================================================

    #[test]
    fn test_webvtt_loader_new() {
        let loader = WebVTTLoader::new("captions.vtt");
        assert_eq!(loader.file_path, PathBuf::from("captions.vtt"));
    }

    #[test]
    fn test_webvtt_loader_new_from_pathbuf() {
        let path = PathBuf::from("/video/subtitles.vtt");
        let loader = WebVTTLoader::new(&path);
        assert_eq!(loader.file_path, path);
    }

    #[tokio::test]
    async fn test_webvtt_loader_load_basic() {
        let mut temp_file = NamedTempFile::with_suffix(".vtt").unwrap();
        let vtt_content = r#"WEBVTT

00:00:00.000 --> 00:00:02.000
Hello from WebVTT!

00:00:03.000 --> 00:00:05.000
This is caption two.
"#;
        temp_file.write_all(vtt_content.as_bytes()).unwrap();

        let loader = WebVTTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Hello from WebVTT!"));
        assert!(docs[0].page_content.contains("This is caption two"));
        // Timestamps should not be included
        assert!(!docs[0].page_content.contains("00:00:00.000"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("webvtt")
        );
        assert_eq!(
            docs[0].metadata.get("type").and_then(|v| v.as_str()),
            Some("subtitles")
        );
    }

    #[tokio::test]
    async fn test_webvtt_loader_with_header_metadata() {
        let mut temp_file = NamedTempFile::with_suffix(".vtt").unwrap();
        let vtt_content = r#"WEBVTT - This is a title

00:00:01.000 --> 00:00:04.000
First caption
"#;
        temp_file.write_all(vtt_content.as_bytes()).unwrap();

        let loader = WebVTTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].page_content.contains("First caption"));
        // Header line should be skipped
        assert!(!docs[0].page_content.contains("This is a title"));
    }

    #[tokio::test]
    async fn test_webvtt_loader_multiline_cue() {
        let mut temp_file = NamedTempFile::with_suffix(".vtt").unwrap();
        let vtt_content = r#"WEBVTT

00:00:00.000 --> 00:00:05.000
Line one
Line two
"#;
        temp_file.write_all(vtt_content.as_bytes()).unwrap();

        let loader = WebVTTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        // Lines should be joined with spaces
        assert!(docs[0].page_content.contains("Line one"));
        assert!(docs[0].page_content.contains("Line two"));
    }

    #[tokio::test]
    async fn test_webvtt_loader_with_cue_identifiers() {
        let mut temp_file = NamedTempFile::with_suffix(".vtt").unwrap();
        let vtt_content = r#"WEBVTT

cue-1
00:00:00.000 --> 00:00:02.000
Caption with identifier

cue-2
00:00:03.000 --> 00:00:05.000
Another caption
"#;
        temp_file.write_all(vtt_content.as_bytes()).unwrap();

        let loader = WebVTTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        // Cue identifiers should not be in output
        assert!(!docs[0].page_content.contains("cue-1"));
        assert!(!docs[0].page_content.contains("cue-2"));
        assert!(docs[0].page_content.contains("Caption with identifier"));
        assert!(docs[0].page_content.contains("Another caption"));
    }

    #[tokio::test]
    async fn test_webvtt_loader_empty_file() {
        let mut temp_file = NamedTempFile::with_suffix(".vtt").unwrap();
        temp_file.write_all(b"WEBVTT\n\n").unwrap();

        let loader = WebVTTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_webvtt_loader_source_metadata() {
        let mut temp_file = NamedTempFile::with_suffix(".vtt").unwrap();
        temp_file
            .write_all(b"WEBVTT\n\n00:00:00.000 --> 00:00:01.000\nTest")
            .unwrap();

        let loader = WebVTTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.get("source").is_some());
    }

    // ==========================================================================
    // Integration Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_notebook_value_to_string_variants() {
        let loader = NotebookLoader::new("test.ipynb");

        // Test with String value
        let string_val = Value::String("hello".to_string());
        assert_eq!(loader.value_to_string(&string_val), "hello");

        // Test with Array value
        let array_val = Value::Array(vec![
            Value::String("line1\n".to_string()),
            Value::String("line2".to_string()),
        ]);
        assert_eq!(loader.value_to_string(&array_val), "line1\nline2");

        // Test with other value types
        let number_val = Value::Number(42.into());
        assert_eq!(loader.value_to_string(&number_val), "42");
    }

    #[tokio::test]
    async fn test_srt_realistic_movie_subtitles() {
        let mut temp_file = NamedTempFile::with_suffix(".srt").unwrap();
        let srt_content = r#"1
00:00:10,500 --> 00:00:13,000
<i>In a world...</i>

2
00:00:15,000 --> 00:00:17,500
Where everything changed.

3
00:00:20,000 --> 00:00:22,000
<b>One man</b> will rise.

4
00:00:25,500 --> 00:00:28,000
Coming soon to theaters.

"#;
        temp_file.write_all(srt_content.as_bytes()).unwrap();

        let loader = SRTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs[0]
                .metadata
                .get("subtitle_count")
                .and_then(|v| v.as_i64()),
            Some(4)
        );
        // HTML-like tags are preserved in the text
        assert!(docs[0].page_content.contains("In a world"));
    }

    #[tokio::test]
    async fn test_webvtt_with_styling() {
        let mut temp_file = NamedTempFile::with_suffix(".vtt").unwrap();
        // WebVTT can include styling directives
        let vtt_content = r#"WEBVTT

STYLE
::cue {
  color: white;
}

00:00:00.000 --> 00:00:02.000
Styled caption text

"#;
        temp_file.write_all(vtt_content.as_bytes()).unwrap();

        let loader = WebVTTLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        // The caption text should be extracted
        assert!(docs[0].page_content.contains("Styled caption text"));
    }
}
