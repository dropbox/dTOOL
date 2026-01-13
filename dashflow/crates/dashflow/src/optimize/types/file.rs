// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! File type for document inputs (PDFs, spreadsheets, text files, etc.)

use super::{LlmContent, ToLlmContent};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported file types for document inputs to LLMs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    /// PDF - Portable Document Format (documents, reports)
    Pdf,
    /// DOCX - Microsoft Word document (Office Open XML)
    Docx,
    /// XLSX - Microsoft Excel spreadsheet (Office Open XML)
    Xlsx,
    /// PPTX - Microsoft PowerPoint presentation (Office Open XML)
    Pptx,
    /// TXT - Plain text file
    Txt,
    /// CSV - Comma-separated values (tabular data)
    Csv,
    /// JSON - JavaScript Object Notation (structured data)
    Json,
    /// XML - Extensible Markup Language (structured data)
    Xml,
    /// HTML - HyperText Markup Language (web pages)
    Html,
    /// MD - Markdown (formatted text)
    Md,
}

impl FileType {
    /// Get MIME type for this file type
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            Self::Txt => "text/plain",
            Self::Csv => "text/csv",
            Self::Json => "application/json",
            Self::Xml => "application/xml",
            Self::Html => "text/html",
            Self::Md => "text/markdown",
        }
    }

    /// Get file extension for this type
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Txt => "txt",
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Md => "md",
        }
    }

    /// Detect file type from extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "docx" => Some(Self::Docx),
            "xlsx" => Some(Self::Xlsx),
            "pptx" => Some(Self::Pptx),
            "txt" => Some(Self::Txt),
            "csv" => Some(Self::Csv),
            "json" => Some(Self::Json),
            "xml" => Some(Self::Xml),
            "html" | "htm" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Md),
            _ => None,
        }
    }

    /// Check if this is a text-based format
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            Self::Txt | Self::Csv | Self::Json | Self::Xml | Self::Html | Self::Md
        )
    }

    /// Check if this is a binary format
    pub fn is_binary(&self) -> bool {
        matches!(self, Self::Pdf | Self::Docx | Self::Xlsx | Self::Pptx)
    }
}

/// File input for document-capable LLMs
///
/// Supports base64-encoded files for use with models that can process
/// documents like PDFs, spreadsheets, and text files.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::types::File;
///
/// // From file
/// let file = File::from_path("document.pdf")?;
///
/// // From bytes
/// let file = File::from_bytes(&bytes, FileType::Pdf)
///     .with_filename("report.pdf");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    /// Base64-encoded file data
    data: String,

    /// File type
    file_type: FileType,

    /// Optional filename
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,

    /// Optional file size in bytes (for metadata)
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<usize>,
}

impl File {
    /// Create file from base64-encoded data
    ///
    /// # Arguments
    /// * `data` - Base64-encoded file data
    /// * `file_type` - File type
    pub fn from_base64(data: impl Into<String>, file_type: FileType) -> Self {
        Self {
            data: data.into(),
            file_type,
            filename: None,
            size_bytes: None,
        }
    }

    /// Create file from raw bytes
    ///
    /// # Arguments
    /// * `bytes` - Raw file bytes
    /// * `file_type` - File type
    pub fn from_bytes(bytes: &[u8], file_type: FileType) -> Self {
        let data = BASE64.encode(bytes);
        let size_bytes = Some(bytes.len());
        Self {
            data,
            file_type,
            filename: None,
            size_bytes,
        }
    }

    /// Create file from text content
    ///
    /// # Arguments
    /// * `content` - Text content
    /// * `file_type` - File type (should be a text-based type)
    pub fn from_text(content: impl Into<String>, file_type: FileType) -> Self {
        let content = content.into();
        Self::from_bytes(content.as_bytes(), file_type)
    }

    /// Load file from path
    ///
    /// Automatically detects file type from extension.
    ///
    /// # Arguments
    /// * `path` - Path to the file
    ///
    /// # Returns
    /// Result with File or error if file cannot be read or type unknown
    pub fn from_path(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        let file_type = FileType::from_extension(ext).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Unknown file type: {}", ext),
            )
        })?;

        let bytes = std::fs::read(path)?;
        let filename = path.file_name().and_then(|n| n.to_str()).map(String::from);

        let mut file = Self::from_bytes(&bytes, file_type);
        file.filename = filename;
        Ok(file)
    }

    /// Set filename
    ///
    /// # Arguments
    /// * `filename` - Filename
    #[must_use]
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Get the base64-encoded file data
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Get the file type
    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    /// Get filename if set
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Get file size in bytes if known
    pub fn size_bytes(&self) -> Option<usize> {
        self.size_bytes
    }

    /// Get MIME type for this file
    pub fn mime_type(&self) -> &'static str {
        self.file_type.mime_type()
    }

    /// Decode the base64 data to raw bytes
    pub fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64.decode(&self.data)
    }

    /// Decode to string (for text-based formats)
    pub fn decode_text(&self) -> Result<String, Box<dyn std::error::Error>> {
        let bytes = self.decode()?;
        Ok(String::from_utf8(bytes)?)
    }
}

impl ToLlmContent for File {
    fn to_content(&self) -> LlmContent {
        LlmContent::File {
            data: self.data.clone(),
            media_type: self.file_type.mime_type().to_string(),
            filename: self.filename.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_from_base64() {
        let file = File::from_base64("dGVzdA==", FileType::Pdf);
        assert_eq!(file.data(), "dGVzdA==");
        assert_eq!(file.file_type(), FileType::Pdf);
    }

    #[test]
    fn test_file_from_bytes() {
        let bytes = b"fake pdf data";
        let file = File::from_bytes(bytes, FileType::Pdf);
        assert_eq!(file.file_type(), FileType::Pdf);
        assert_eq!(file.size_bytes(), Some(13));
        // Verify it decodes back
        let decoded = file.decode().unwrap();
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn test_file_from_text() {
        let file = File::from_text("Hello, world!", FileType::Txt);
        assert_eq!(file.file_type(), FileType::Txt);
        assert_eq!(file.decode_text().unwrap(), "Hello, world!");
    }

    #[test]
    fn test_file_with_filename() {
        let file = File::from_base64("dGVzdA==", FileType::Pdf).with_filename("report.pdf");
        assert_eq!(file.filename(), Some("report.pdf"));
    }

    #[test]
    fn test_file_type_detection() {
        assert_eq!(FileType::from_extension("pdf"), Some(FileType::Pdf));
        assert_eq!(FileType::from_extension("docx"), Some(FileType::Docx));
        assert_eq!(FileType::from_extension("xlsx"), Some(FileType::Xlsx));
        assert_eq!(FileType::from_extension("pptx"), Some(FileType::Pptx));
        assert_eq!(FileType::from_extension("txt"), Some(FileType::Txt));
        assert_eq!(FileType::from_extension("csv"), Some(FileType::Csv));
        assert_eq!(FileType::from_extension("json"), Some(FileType::Json));
        assert_eq!(FileType::from_extension("xml"), Some(FileType::Xml));
        assert_eq!(FileType::from_extension("html"), Some(FileType::Html));
        assert_eq!(FileType::from_extension("md"), Some(FileType::Md));
        assert_eq!(FileType::from_extension("exe"), None);
    }

    #[test]
    fn test_file_type_classification() {
        assert!(FileType::Txt.is_text());
        assert!(FileType::Csv.is_text());
        assert!(FileType::Json.is_text());
        assert!(!FileType::Pdf.is_text());

        assert!(FileType::Pdf.is_binary());
        assert!(FileType::Docx.is_binary());
        assert!(!FileType::Txt.is_binary());
    }

    #[test]
    fn test_file_type_mime() {
        assert_eq!(FileType::Pdf.mime_type(), "application/pdf");
        assert_eq!(FileType::Txt.mime_type(), "text/plain");
        assert_eq!(FileType::Csv.mime_type(), "text/csv");
        assert_eq!(FileType::Json.mime_type(), "application/json");
    }

    #[test]
    fn test_to_llm_content() {
        let file = File::from_base64("dGVzdA==", FileType::Pdf).with_filename("doc.pdf");
        let content = file.to_content();
        match content {
            LlmContent::File {
                data,
                media_type,
                filename,
            } => {
                assert_eq!(data, "dGVzdA==");
                assert_eq!(media_type, "application/pdf");
                assert_eq!(filename, Some("doc.pdf".to_string()));
            }
            _ => panic!("Expected File variant"),
        }
    }

    #[test]
    fn test_serialization() {
        let file = File::from_base64("dGVzdA==", FileType::Csv).with_filename("data.csv");

        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("dGVzdA=="));
        assert!(json.contains("csv"));
        assert!(json.contains("data.csv"));

        let deserialized: File = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data(), "dGVzdA==");
        assert_eq!(deserialized.file_type(), FileType::Csv);
        assert_eq!(deserialized.filename(), Some("data.csv"));
    }
}
