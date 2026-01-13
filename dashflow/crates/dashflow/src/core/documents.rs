//! Document types for DashFlow
//!
//! This module provides the Document and Blob types for storing text and associated metadata.
//! Documents are the primary unit of content in DashFlow applications, used for
//! RAG pipelines, text splitting, and document processing.
//!
//! # Document Loaders
//!
//! Document loaders provide a way to load data from various sources into Document objects.
//! The loader interface is built around the `DocumentLoader` trait, which supports lazy loading
//! to avoid loading all documents into memory at once.
//!
//! # Blobs
//!
//! Blobs represent raw data by either reference (path) or value (in-memory data).
//! They provide a way to decouple data loading from data parsing, making it easier to
//! reuse parsers across different data sources.
//!
//! # Document Compressors
//!
//! Document compressors are used to post-process retrieved documents. Common use cases:
//! - Filter irrelevant documents based on relevance scores
//! - Extract only relevant portions of documents
//! - Re-rank documents using an LLM
//! - Remove redundant information

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info_span, Instrument};

use crate::core::document_transformers::DocumentTransformer;
use crate::core::error::{Error, Result};
use crate::core::prompts::PromptTemplate;

/// Represents raw data by reference (path) or value (in-memory bytes/string).
///
/// Blobs provide an interface to materialize data in different representations
/// and help decouple data loading from downstream parsing. This is inspired by
/// [Mozilla's Blob API](https://developer.mozilla.org/en-US/docs/Web/API/Blob).
///
/// # Example: Load from memory
///
/// ```
/// use dashflow::core::documents::Blob;
///
/// let blob = Blob::from_data("Hello, world!");
/// assert_eq!(blob.as_string().unwrap(), "Hello, world!");
/// assert_eq!(blob.as_bytes().unwrap(), b"Hello, world!");
/// ```
///
/// # Example: Load from file
///
/// ```no_run
/// use dashflow::core::documents::Blob;
///
/// let blob = Blob::from_path("path/to/file.txt");
/// let content = blob.as_string().unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blob {
    /// Raw data (bytes or string), or None if referencing a file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BlobData>,

    /// MIME type (not to be confused with file extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mimetype: Option<String>,

    /// Encoding to use when decoding bytes to string (default: utf-8)
    #[serde(default = "default_encoding")]
    pub encoding: String,

    /// Path to the file (if loading from filesystem)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,

    /// Metadata associated with the blob
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Optional unique identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Raw data stored in a Blob (either bytes or string)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlobData {
    /// Raw bytes
    Bytes(Vec<u8>),
    /// String data
    String(String),
}

fn default_encoding() -> String {
    "utf-8".to_string()
}

impl Blob {
    /// Create a new Blob from a file path (lazy loading - data not read until needed).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow::core::documents::Blob;
    ///
    /// let blob = Blob::from_path("example.txt");
    /// ```
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path_buf = path.as_ref().to_path_buf();
        let mimetype = mime_guess::from_path(&path_buf)
            .first()
            .map(|m| m.to_string());

        Self {
            data: None,
            mimetype,
            encoding: default_encoding(),
            path: Some(path_buf),
            metadata: HashMap::new(),
            id: None,
        }
    }

    /// Create a new Blob from in-memory data (string).
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::documents::Blob;
    ///
    /// let blob = Blob::from_data("Hello, world!");
    /// assert_eq!(blob.as_string().unwrap(), "Hello, world!");
    /// ```
    pub fn from_data(data: impl Into<String>) -> Self {
        Self {
            data: Some(BlobData::String(data.into())),
            mimetype: Some("text/plain".to_string()),
            encoding: default_encoding(),
            path: None,
            metadata: HashMap::new(),
            id: None,
        }
    }

    /// Create a new Blob from in-memory bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::documents::Blob;
    ///
    /// let blob = Blob::from_bytes(b"Hello, world!".to_vec());
    /// assert_eq!(blob.as_bytes().unwrap(), b"Hello, world!");
    /// ```
    #[must_use]
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self {
            data: Some(BlobData::Bytes(data)),
            mimetype: Some("application/octet-stream".to_string()),
            encoding: default_encoding(),
            path: None,
            metadata: HashMap::new(),
            id: None,
        }
    }

    /// Builder method to set MIME type.
    #[must_use]
    pub fn with_mimetype(mut self, mimetype: impl Into<String>) -> Self {
        self.mimetype = Some(mimetype.into());
        self
    }

    /// Builder method to set encoding.
    #[must_use]
    pub fn with_encoding(mut self, encoding: impl Into<String>) -> Self {
        self.encoding = encoding.into();
        self
    }

    /// Builder method to set path.
    #[must_use]
    pub fn with_path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Builder method to add metadata.
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Builder method to set ID.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Get the source location of the blob.
    ///
    /// Returns the metadata "source" field if present, otherwise the path as a string.
    #[must_use]
    pub fn source(&self) -> Option<String> {
        // Check metadata first
        if let Some(source) = self.metadata.get("source") {
            if let Some(s) = source.as_str() {
                return Some(s.to_string());
            }
        }
        // Fall back to path
        self.path.as_ref().map(|p| p.display().to_string())
    }

    /// Read the blob as a string.
    ///
    /// If the blob references a file, reads the file. If the blob contains in-memory data,
    /// converts it to a string using the blob's encoding.
    pub fn as_string(&self) -> Result<String> {
        match &self.data {
            Some(BlobData::String(s)) => Ok(s.clone()),
            Some(BlobData::Bytes(bytes)) => String::from_utf8(bytes.clone()).map_err(|e| {
                Error::InvalidInput(format!(
                    "Failed to decode bytes as {}: {}",
                    self.encoding, e
                ))
            }),
            None => {
                if let Some(path) = &self.path {
                    std::fs::read_to_string(path).map_err(std::convert::Into::into)
                } else {
                    Err(Error::InvalidInput(
                        "Blob has no data or path to read from".to_string(),
                    ))
                }
            }
        }
    }

    /// Read the blob as bytes.
    ///
    /// If the blob references a file, reads the file. If the blob contains in-memory data,
    /// converts it to bytes.
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        match &self.data {
            Some(BlobData::Bytes(bytes)) => Ok(bytes.clone()),
            Some(BlobData::String(s)) => Ok(s.as_bytes().to_vec()),
            None => {
                if let Some(path) = &self.path {
                    std::fs::read(path).map_err(std::convert::Into::into)
                } else {
                    Err(Error::InvalidInput(
                        "Blob has no data or path to read from".to_string(),
                    ))
                }
            }
        }
    }

    /// Get a reference to the blob content as a string slice, if available in memory.
    ///
    /// Returns `None` if:
    /// - The blob references a file (not loaded into memory)
    /// - The blob contains binary data that isn't valid UTF-8
    ///
    /// Use this method when you only need to inspect the content without ownership.
    /// For guaranteed string access (including file I/O), use `as_string()`.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::documents::Blob;
    ///
    /// let blob = Blob::from_data("Hello, world!");
    /// assert_eq!(blob.as_str_ref(), Some("Hello, world!"));
    ///
    /// let file_blob = Blob::from_path("example.txt");
    /// assert_eq!(file_blob.as_str_ref(), None); // Data not in memory
    /// ```
    #[must_use]
    pub fn as_str_ref(&self) -> Option<&str> {
        match &self.data {
            Some(BlobData::String(s)) => Some(s.as_str()),
            Some(BlobData::Bytes(bytes)) => std::str::from_utf8(bytes).ok(),
            None => None,
        }
    }

    /// Get a reference to the blob content as a byte slice, if available in memory.
    ///
    /// Returns `None` if the blob references a file that hasn't been loaded into memory.
    ///
    /// Use this method when you only need to inspect the content without ownership.
    /// For guaranteed byte access (including file I/O), use `as_bytes()`.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::documents::Blob;
    ///
    /// let blob = Blob::from_data("Hello, world!");
    /// assert_eq!(blob.as_bytes_ref(), Some(b"Hello, world!" as &[u8]));
    ///
    /// let file_blob = Blob::from_path("example.txt");
    /// assert_eq!(file_blob.as_bytes_ref(), None); // Data not in memory
    /// ```
    #[must_use]
    pub fn as_bytes_ref(&self) -> Option<&[u8]> {
        match &self.data {
            Some(BlobData::Bytes(bytes)) => Some(bytes.as_slice()),
            Some(BlobData::String(s)) => Some(s.as_bytes()),
            None => None,
        }
    }
}

impl std::fmt::Display for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Blob")?;
        if let Some(source) = self.source() {
            write!(f, " {source}")?;
        }
        Ok(())
    }
}

/// A document with text content and metadata.
///
/// Documents are used throughout DashFlow for representing pieces of text
/// that need to be processed, stored, or retrieved. Each document contains:
/// - `page_content`: The text content
/// - `metadata`: Optional metadata as key-value pairs
/// - `id`: Optional unique identifier
///
/// # Example
///
/// ```
/// use dashflow::core::documents::Document;
///
/// let doc = Document::new("Hello, world!")
///     .with_metadata("source", "example.txt".to_string())
///     .with_metadata("page", 1);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// The text content of the document
    pub page_content: String,

    /// Metadata associated with the document
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Optional unique identifier for the document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl Document {
    /// Create a new document with the given text content.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::documents::Document;
    ///
    /// let doc = Document::new("Hello, world!");
    /// assert_eq!(doc.page_content, "Hello, world!");
    /// ```
    pub fn new(page_content: impl Into<String>) -> Self {
        Self {
            page_content: page_content.into(),
            metadata: HashMap::new(),
            id: None,
        }
    }

    /// Add metadata to the document (builder pattern).
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::documents::Document;
    ///
    /// let doc = Document::new("Hello")
    ///     .with_metadata("source", "example.txt".to_string())
    ///     .with_metadata("page", 1);
    /// ```
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set the document ID (builder pattern).
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::documents::Document;
    ///
    /// let doc = Document::new("Hello").with_id("doc-123");
    /// assert_eq!(doc.id, Some("doc-123".to_string()));
    /// ```
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Get metadata value by key.
    #[must_use]
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Set metadata value.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) {
        self.metadata.insert(key.into(), value.into());
    }
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.metadata.is_empty() {
            write!(f, "page_content='{}'", self.page_content)
        } else {
            write!(
                f,
                "page_content='{}' metadata={:?}",
                self.page_content, self.metadata
            )
        }
    }
}

/// Trait for compressing documents given a query context.
///
/// Document compressors are used to post-process retrieved documents before
/// returning them to the user. This can include filtering, extracting relevant
/// passages, re-ranking, or any other transformation that improves result quality.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::{Document, DocumentCompressor};
///
/// struct SimpleCompressor;
///
/// #[async_trait]
/// impl DocumentCompressor for SimpleCompressor {
///     async fn compress_documents(
///         &self,
///         documents: Vec<Document>,
///         query: &str,
///         config: Option<&RunnableConfig>,
///     ) -> Result<Vec<Document>> {
///         // Filter documents based on some criteria
///         Ok(documents.into_iter()
///             .filter(|doc| doc.page_content.contains(query))
///             .collect())
///     }
/// }
/// ```
#[async_trait]
pub trait DocumentCompressor: Send + Sync {
    /// Compress retrieved documents given the query context.
    ///
    /// # Arguments
    ///
    /// * `documents` - The retrieved documents to compress
    /// * `query` - The query context used to retrieve the documents
    /// * `config` - Optional configuration for the compression
    ///
    /// # Returns
    ///
    /// A potentially smaller or modified list of documents
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        config: Option<&crate::core::config::RunnableConfig>,
    ) -> Result<Vec<Document>>;
}

/// Enum to hold either a `DocumentTransformer` or `DocumentCompressor`
///
/// This allows the pipeline to accept both types of document processors.
pub enum DocumentProcessor {
    /// A document transformer that transforms documents without needing a query
    Transformer(Arc<dyn DocumentTransformer>),
    /// A document compressor that compresses documents based on a query
    Compressor(Arc<dyn DocumentCompressor>),
}

/// Document compressor that chains multiple transformers and compressors
///
/// This pipeline applies a sequence of document processors (transformers and compressors)
/// to retrieved documents. Transformers modify documents without context, while compressors
/// can filter or modify based on the query context.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::{DocumentCompressorPipeline, DocumentProcessor};
/// use dashflow::core::document_transformers::LongContextReorder;
/// use std::sync::Arc;
///
/// let reorder = Arc::new(LongContextReorder::new());
/// let pipeline = DocumentCompressorPipeline::new(vec![
///     DocumentProcessor::Transformer(reorder),
/// ]);
///
/// let compressed = pipeline.compress_documents(docs, "query", None).await?;
/// ```
pub struct DocumentCompressorPipeline {
    /// List of document processors that are chained together and run in sequence
    pub processors: Vec<DocumentProcessor>,
}

impl DocumentCompressorPipeline {
    /// Create a new pipeline with the given processors
    #[must_use]
    pub fn new(processors: Vec<DocumentProcessor>) -> Self {
        Self { processors }
    }

    /// Add a transformer to the pipeline
    #[must_use]
    pub fn with_transformer(mut self, transformer: Arc<dyn DocumentTransformer>) -> Self {
        self.processors
            .push(DocumentProcessor::Transformer(transformer));
        self
    }

    /// Add a compressor to the pipeline
    #[must_use]
    pub fn with_compressor(mut self, compressor: Arc<dyn DocumentCompressor>) -> Self {
        self.processors
            .push(DocumentProcessor::Compressor(compressor));
        self
    }
}

#[async_trait]
impl DocumentCompressor for DocumentCompressorPipeline {
    async fn compress_documents(
        &self,
        mut documents: Vec<Document>,
        query: &str,
        config: Option<&crate::core::config::RunnableConfig>,
    ) -> Result<Vec<Document>> {
        for processor in &self.processors {
            documents = match processor {
                DocumentProcessor::Transformer(transformer) => {
                    transformer.transform_documents(documents)?
                }
                DocumentProcessor::Compressor(compressor) => {
                    compressor
                        .compress_documents(documents, query, config)
                        .await?
                }
            };
        }
        Ok(documents)
    }
}

// ============================================================================
// Traced Document Compressor
// ============================================================================

/// A document compressor wrapper that adds automatic tracing and observability.
///
/// This struct wraps any `DocumentCompressor` and instruments all calls with
/// OpenTelemetry spans. This enables:
///
/// - Distributed tracing across service boundaries
/// - Performance monitoring and latency tracking
/// - Document count tracking
/// - Error monitoring and alerting
///
/// # Span Attributes
///
/// Each traced call includes the following span attributes:
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `compressor.operation` | Always "compress_documents" |
/// | `compressor.input_count` | Number of input documents |
/// | `compressor.query_len` | Length of the query string |
/// | `compressor.output_count` | Number of output documents |
/// | `compressor.duration_ms` | Call duration in milliseconds |
/// | `compressor.success` | Whether the call succeeded |
/// | `service.name` | Service name (if configured) |
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::{TracedDocumentCompressor, DocumentCompressorTracedExt};
/// use dashflow_jina::rerank::JinaRerank;
///
/// // Wrap any document compressor with tracing
/// let reranker = JinaRerank::new()?;
/// let traced = reranker.with_tracing();
///
/// // Or with a custom service name
/// let traced = reranker.with_tracing_named("rag-pipeline");
///
/// // All calls are now automatically traced
/// let reranked = traced.compress_documents(docs, "What is Rust?", None).await?;
/// ```
pub struct TracedDocumentCompressor<C: DocumentCompressor> {
    /// The underlying document compressor
    inner: C,
    /// Optional service name for trace attribution
    service_name: Option<String>,
}

impl<C: DocumentCompressor> TracedDocumentCompressor<C> {
    /// Create a new `TracedDocumentCompressor` wrapping the given compressor.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying document compressor to wrap
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            service_name: None,
        }
    }

    /// Create a new `TracedDocumentCompressor` with a service name.
    ///
    /// The service name is included in trace attributes for easier filtering
    /// and grouping in observability platforms.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying document compressor to wrap
    /// * `service_name` - Name to identify this service in traces
    #[must_use]
    pub fn with_service_name(inner: C, service_name: impl Into<String>) -> Self {
        Self {
            inner,
            service_name: Some(service_name.into()),
        }
    }

    /// Get a reference to the underlying compressor.
    #[must_use]
    pub fn inner(&self) -> &C {
        &self.inner
    }

    /// Get the service name, if configured.
    #[must_use]
    pub fn service_name(&self) -> Option<&str> {
        self.service_name.as_deref()
    }
}

#[async_trait]
impl<C: DocumentCompressor> DocumentCompressor for TracedDocumentCompressor<C> {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        config: Option<&crate::core::config::RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let input_count = documents.len();
        let query_len = query.len();
        let start = Instant::now();

        let span = if let Some(ref service) = self.service_name {
            info_span!(
                "compressor.compress_documents",
                compressor.operation = "compress_documents",
                compressor.input_count = input_count,
                compressor.query_len = query_len,
                service.name = service.as_str(),
                compressor.duration_ms = tracing::field::Empty,
                compressor.success = tracing::field::Empty,
                compressor.output_count = tracing::field::Empty,
            )
        } else {
            info_span!(
                "compressor.compress_documents",
                compressor.operation = "compress_documents",
                compressor.input_count = input_count,
                compressor.query_len = query_len,
                compressor.duration_ms = tracing::field::Empty,
                compressor.success = tracing::field::Empty,
                compressor.output_count = tracing::field::Empty,
            )
        };

        let result = async {
            self.inner
                .compress_documents(documents, query, config)
                .await
        }
        .instrument(span.clone())
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;
        let success = result.is_ok();

        span.record("compressor.duration_ms", duration_ms);
        span.record("compressor.success", success);

        if let Ok(ref docs) = result {
            span.record("compressor.output_count", docs.len() as u64);
            tracing::info!(
                parent: &span,
                duration_ms = duration_ms,
                input_count = input_count,
                output_count = docs.len(),
                "Document compression completed"
            );
        } else {
            tracing::warn!(
                parent: &span,
                duration_ms = duration_ms,
                error = ?result.as_ref().err(),
                "Document compression failed"
            );
        }

        result
    }
}

/// Extension trait adding tracing support to `DocumentCompressor`.
///
/// This trait is automatically implemented for all types that implement `DocumentCompressor`,
/// providing convenient methods to wrap compressors with automatic tracing.
pub trait DocumentCompressorTracedExt: DocumentCompressor + Sized {
    /// Wrap this document compressor with automatic tracing.
    ///
    /// Returns a `TracedDocumentCompressor` that instruments all calls with OpenTelemetry spans.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let traced = reranker.with_tracing();
    /// ```
    fn with_tracing(self) -> TracedDocumentCompressor<Self> {
        TracedDocumentCompressor::new(self)
    }

    /// Wrap this document compressor with automatic tracing and a custom service name.
    ///
    /// The service name is included in span attributes for easier filtering.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let traced = reranker.with_tracing_named("rag-pipeline");
    /// ```
    fn with_tracing_named(self, service_name: impl Into<String>) -> TracedDocumentCompressor<Self> {
        TracedDocumentCompressor::with_service_name(self, service_name)
    }
}

/// Blanket implementation of `DocumentCompressorTracedExt` for all `DocumentCompressor` implementations.
impl<C: DocumentCompressor + Sized> DocumentCompressorTracedExt for C {}

/// Format a document into a string based on a prompt template
///
/// This function extracts information from a document and formats it using a prompt template.
/// It pulls information from two sources:
///
/// 1. `page_content`: Assigned to a variable named "`page_content`"
/// 2. `metadata`: Each metadata field is assigned to a variable with the same name
///
/// Those variables are then passed into the prompt template to produce a formatted string.
///
/// # Arguments
///
/// * `doc` - Document whose `page_content` and metadata will be used
/// * `prompt` - Prompt template that will format the document
///
/// # Returns
///
/// Formatted string, or an error if required variables are missing
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::{Document, format_document};
/// use dashflow::core::prompts::PromptTemplate;
///
/// let doc = Document::new("This is a joke")
///     .with_metadata("page", 1);
/// let prompt = PromptTemplate::from_template("Page {page}: {page_content}");
/// let result = format_document(&doc, &prompt)?;
/// assert_eq!(result, "Page 1: This is a joke");
/// ```
pub fn format_document(doc: &Document, prompt: &PromptTemplate) -> Result<String> {
    // Build the input variables dict: page_content + all metadata
    // PromptTemplate expects HashMap<String, String>, so convert metadata values to strings
    let mut variables = HashMap::new();
    variables.insert("page_content".to_string(), doc.page_content.clone());

    for (key, value) in &doc.metadata {
        // Convert JSON value to string representation
        let value_str = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            v => v.to_string(), // For objects/arrays, use JSON representation
        };
        variables.insert(key.clone(), value_str);
    }

    // Check that all required variables are present
    let required_vars = &prompt.input_variables;
    let mut missing_vars = Vec::new();
    for var in required_vars {
        if !variables.contains_key(var as &str) {
            missing_vars.push(var.clone());
        }
    }

    if !missing_vars.is_empty() {
        let required_metadata: Vec<_> = required_vars
            .iter()
            .filter(|v| *v != "page_content")
            .cloned()
            .collect();
        return Err(Error::InvalidInput(format!(
            "Document prompt requires documents to have metadata variables: {required_metadata:?}. \
             Received document with missing metadata: {missing_vars:?}"
        )));
    }

    // Format using the variables
    prompt.format(&variables)
}

/// Trait for loading documents from various sources.
///
/// Implementations should use lazy loading (iterators/streams) to avoid loading all documents
/// into memory at once. The `load()` method is provided for convenience but calls `lazy_load()`
/// internally.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::{Document, DocumentLoader};
/// use async_trait::async_trait;
///
/// struct MyLoader {
///     source: String,
/// }
///
/// #[async_trait]
/// impl DocumentLoader for MyLoader {
///     async fn load(&self) -> Result<Vec<Document>> {
///         // Load and return all documents
///         Ok(vec![Document::new("content")])
///     }
/// }
/// ```
#[async_trait]
pub trait DocumentLoader: Send + Sync {
    /// Load all documents from the source.
    ///
    /// This is a convenience method that loads all documents into memory.
    /// For large datasets, consider implementing lazy loading.
    async fn load(&self) -> Result<Vec<Document>>;

    /// Load documents and split them into chunks.
    ///
    /// Splits document into smaller chunks using a text splitter.
    /// Currently just returns the loaded documents without splitting.
    async fn load_and_split(&self) -> Result<Vec<Document>> {
        // Note: Text splitters are available in dashflow-text-splitters crate
        self.load().await
    }
}

/// Trait for loading blobs from various sources.
///
/// Blob loaders provide raw data that can be parsed by downstream parsers.
/// This decouples data loading from data parsing.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::{Blob, BlobLoader};
/// use async_trait::async_trait;
///
/// struct MyBlobLoader {
///     path: String,
/// }
///
/// #[async_trait]
/// impl BlobLoader for MyBlobLoader {
///     async fn yield_blobs(&self) -> Result<Vec<Blob>> {
///         Ok(vec![Blob::from_path(&self.path)])
///     }
/// }
/// ```
#[async_trait]
pub trait BlobLoader: Send + Sync {
    /// Load blobs from the source.
    ///
    /// Returns a vector of blobs. For lazy loading, implementations could return
    /// an iterator or stream instead.
    async fn yield_blobs(&self) -> Result<Vec<Blob>>;
}

/// Trait for parsing blobs into documents.
///
/// Blob parsers convert raw data (Blob) into structured documents.
/// This allows reusing parsers across different blob sources.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::documents::{Blob, BlobParser, Document};
/// use async_trait::async_trait;
///
/// struct TextParser;
///
/// #[async_trait]
/// impl BlobParser for TextParser {
///     async fn parse(&self, blob: &Blob) -> Result<Vec<Document>> {
///         let content = blob.as_string()?;
///         Ok(vec![Document::new(content)])
///     }
/// }
/// ```
#[async_trait]
pub trait BlobParser: Send + Sync {
    /// Parse a blob into one or more documents.
    async fn parse(&self, blob: &Blob) -> Result<Vec<Document>>;
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_document_creation() {
        let doc = Document::new("Hello, world!");
        assert_eq!(doc.page_content, "Hello, world!");
        assert!(doc.metadata.is_empty());
        assert_eq!(doc.id, None);
    }

    #[test]
    fn test_document_with_metadata() {
        let doc = Document::new("Hello")
            .with_metadata("source", "example.txt".to_string())
            .with_metadata("page", 1);

        assert_eq!(doc.page_content, "Hello");
        assert_eq!(doc.metadata.len(), 2);
        assert_eq!(
            doc.get_metadata("source").unwrap().as_str().unwrap(),
            "example.txt"
        );
        assert_eq!(doc.get_metadata("page").unwrap().as_i64().unwrap(), 1);
    }

    #[test]
    fn test_document_with_id() {
        let doc = Document::new("Hello").with_id("doc-123");
        assert_eq!(doc.id, Some("doc-123".to_string()));
    }

    #[test]
    fn test_document_serialization() {
        let doc = Document::new("Hello")
            .with_metadata("source", "test".to_string())
            .with_id("123");

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: Document = serde_json::from_str(&json).unwrap();

        assert_eq!(doc, deserialized);
    }

    #[test]
    fn test_document_display() {
        let doc1 = Document::new("Hello");
        assert_eq!(format!("{}", doc1), "page_content='Hello'");

        let doc2 = Document::new("Hello").with_metadata("key", "value".to_string());
        let display = format!("{}", doc2);
        assert!(display.contains("page_content='Hello'"));
        assert!(display.contains("metadata"));
    }

    #[test]
    fn test_blob_from_data() {
        let blob = Blob::from_data("Hello, world!");
        assert_eq!(blob.as_string().unwrap(), "Hello, world!");
        assert_eq!(blob.as_bytes().unwrap(), b"Hello, world!");
    }

    #[test]
    fn test_blob_from_bytes() {
        let blob = Blob::from_bytes(b"Hello, world!".to_vec());
        assert_eq!(blob.as_bytes().unwrap(), b"Hello, world!");
        assert_eq!(blob.as_string().unwrap(), "Hello, world!");
    }

    #[test]
    fn test_blob_with_metadata() {
        let blob = Blob::from_data("content")
            .with_metadata("key", "value".to_string())
            .with_id("blob-123");

        assert_eq!(blob.id, Some("blob-123".to_string()));
        assert_eq!(blob.metadata.get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn test_blob_source() {
        // Source from path
        let blob1 = Blob::from_path("test.txt");
        assert!(blob1.source().unwrap().contains("test.txt"));

        // Source from metadata
        let blob2 =
            Blob::from_data("content").with_metadata("source", "https://example.com".to_string());
        assert_eq!(blob2.source().unwrap(), "https://example.com");
    }

    #[test]
    fn test_blob_serialization() {
        let blob = Blob::from_data("Hello")
            .with_metadata("key", "value".to_string())
            .with_id("123");

        let json = serde_json::to_string(&blob).unwrap();
        let deserialized: Blob = serde_json::from_str(&json).unwrap();

        assert_eq!(blob, deserialized);
    }

    #[test]
    fn test_blob_as_str_ref() {
        // Test with string data in memory
        let blob = Blob::from_data("Hello, world!");
        assert_eq!(blob.as_str_ref(), Some("Hello, world!"));

        // Test with byte data in memory (valid UTF-8)
        let blob = Blob::from_bytes(b"Hello, world!".to_vec());
        assert_eq!(blob.as_str_ref(), Some("Hello, world!"));

        // Test with file path (not in memory)
        let blob = Blob::from_path("example.txt");
        assert_eq!(blob.as_str_ref(), None);

        // Test with invalid UTF-8
        let blob = Blob::from_bytes(vec![0xFF, 0xFE, 0xFD]);
        assert_eq!(blob.as_str_ref(), None);
    }

    #[test]
    fn test_blob_as_bytes_ref() {
        // Test with string data in memory
        let blob = Blob::from_data("Hello, world!");
        assert_eq!(blob.as_bytes_ref(), Some(b"Hello, world!" as &[u8]));

        // Test with byte data in memory
        let blob = Blob::from_bytes(b"Hello, world!".to_vec());
        assert_eq!(blob.as_bytes_ref(), Some(b"Hello, world!" as &[u8]));

        // Test with file path (not in memory)
        let blob = Blob::from_path("example.txt");
        assert_eq!(blob.as_bytes_ref(), None);
    }

    // ========================================================================
    // TracedDocumentCompressor Tests
    // ========================================================================

    use super::{DocumentCompressorTracedExt, TracedDocumentCompressor};

    /// Mock DocumentCompressor for testing
    struct MockCompressor {
        /// Whether to simulate an error
        should_fail: bool,
    }

    impl MockCompressor {
        fn new() -> Self {
            Self { should_fail: false }
        }

        fn failing() -> Self {
            Self { should_fail: true }
        }
    }

    #[async_trait::async_trait]
    impl DocumentCompressor for MockCompressor {
        async fn compress_documents(
            &self,
            documents: Vec<Document>,
            _query: &str,
            _config: Option<&crate::core::config::RunnableConfig>,
        ) -> Result<Vec<Document>> {
            if self.should_fail {
                return Err(Error::InvalidInput("Mock error".to_string()));
            }
            // Return only the first 2 documents (simulating compression)
            Ok(documents.into_iter().take(2).collect())
        }
    }

    #[tokio::test]
    async fn test_traced_compressor_creation() {
        let compressor = MockCompressor::new();
        let traced = TracedDocumentCompressor::new(compressor);

        assert!(traced.service_name().is_none());
    }

    #[tokio::test]
    async fn test_traced_compressor_with_service_name() {
        let compressor = MockCompressor::new();
        let traced = TracedDocumentCompressor::with_service_name(compressor, "test-service");

        assert_eq!(traced.service_name(), Some("test-service"));
    }

    #[tokio::test]
    async fn test_traced_compressor_compress_documents() {
        // Initialize tracing for test (ignore errors if already initialized)
        let _ = tracing_subscriber::fmt::try_init();

        let compressor = MockCompressor::new();
        let traced = TracedDocumentCompressor::new(compressor);

        let docs = vec![
            Document::new("Document 1"),
            Document::new("Document 2"),
            Document::new("Document 3"),
        ];

        let result = traced.compress_documents(docs, "test query", None).await;

        assert!(result.is_ok());
        let compressed = result.unwrap();
        assert_eq!(compressed.len(), 2); // MockCompressor returns first 2
        assert_eq!(compressed[0].page_content, "Document 1");
        assert_eq!(compressed[1].page_content, "Document 2");
    }

    #[tokio::test]
    async fn test_traced_compressor_compress_documents_failure() {
        let _ = tracing_subscriber::fmt::try_init();

        let compressor = MockCompressor::failing();
        let traced = TracedDocumentCompressor::new(compressor);

        let docs = vec![Document::new("Document 1")];
        let result = traced.compress_documents(docs, "test query", None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Mock error"));
    }

    #[tokio::test]
    async fn test_traced_compressor_extension_trait() {
        let _ = tracing_subscriber::fmt::try_init();

        let compressor = MockCompressor::new();
        let traced = compressor.with_tracing();

        let docs = vec![Document::new("Test doc")];
        let result = traced.compress_documents(docs, "query", None).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_traced_compressor_extension_trait_named() {
        let _ = tracing_subscriber::fmt::try_init();

        let compressor = MockCompressor::new();
        let traced = compressor.with_tracing_named("named-service");

        assert_eq!(traced.service_name(), Some("named-service"));

        let docs = vec![Document::new("Test doc")];
        let result = traced.compress_documents(docs, "query", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_traced_compressor_inner_access() {
        let compressor = MockCompressor::new();
        let traced = TracedDocumentCompressor::new(compressor);

        // Access inner compressor (verify it exists)
        let _inner = traced.inner();
    }
}
