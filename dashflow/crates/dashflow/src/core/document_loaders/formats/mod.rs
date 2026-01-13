//! File format document loaders.
//!
//! This module provides loaders for various file formats including:
//! - Text formats (plain text, markdown, HTML, RTF, etc.)
//! - Structured formats (CSV, JSON, YAML, XML, etc.)
//! - Document formats (PDF, Word, EPUB)
//! - Archive formats (ZIP, TAR, GZIP)
//! - Media formats (SRT, `WebVTT`, Notebook)

// Text format loaders (Extracted from legacy.rs)
pub mod text;

// Structured format loaders (Extracted from legacy.rs)
pub mod structured;

// Document format loaders (Extracted from legacy.rs)
pub mod documents;

// Archive format loaders (Extracted from legacy.rs)
pub mod archives;

// Media format loaders (Extracted from legacy.rs)
pub mod media;

// Re-export text format loaders for convenient access
pub use text::{AsciiDocLoader, HTMLLoader, MarkdownLoader, RSTLoader, RTFLoader, TextLoader};

// Re-export structured format loaders for convenient access
pub use structured::{
    CSVLoader, IniLoader, JSONLoader, TOMLLoader, TSVLoader, XMLLoader, YAMLLoader,
};

// Re-export document format loaders for convenient access
pub use documents::{EpubLoader, PDFLoader, PowerPointLoader, WordDocumentLoader};

// Re-export archive format loaders for convenient access
pub use archives::{GzipFileLoader, TarFileLoader, ZipFileLoader};

// Re-export media format loaders for convenient access
pub use media::{NotebookLoader, SRTLoader, WebVTTLoader};
