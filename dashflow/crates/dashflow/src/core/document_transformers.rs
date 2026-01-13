//! Document transformers for processing and filtering documents.
//!
//! Document transformers take a sequence of documents and return a transformed
//! sequence. Common use cases include:
//! - Filtering redundant documents based on embedding similarity
//! - Clustering documents and selecting representatives
//! - Reordering documents for long context windows
//! - Extracting content from HTML
//! - Converting HTML to markdown/text
//! - Extracting structured metadata using LLMs

use crate::core::documents::Document;
use crate::core::error::Result;
use async_trait::async_trait;

/// Trait for transforming documents.
///
/// A document transformation takes a sequence of documents and returns a
/// sequence of transformed documents. Transformations can include filtering,
/// reordering, content extraction, or any other document-level operation.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::DocumentTransformer;
/// use dashflow::core::documents::Document;
///
/// struct MyTransformer;
///
/// #[async_trait]
/// impl DocumentTransformer for MyTransformer {
///     fn transform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
///         // Transform documents
///         Ok(documents)
///     }
/// }
/// ```
#[async_trait]
pub trait DocumentTransformer: Send + Sync {
    /// Transform a list of documents synchronously.
    ///
    /// # Arguments
    ///
    /// * `documents` - The documents to transform
    ///
    /// # Returns
    ///
    /// A vector of transformed documents
    fn transform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>>;

    /// Transform a list of documents asynchronously.
    ///
    /// The default implementation calls `transform_documents` in a blocking fashion.
    /// Override this if you have an async implementation.
    ///
    /// # Arguments
    ///
    /// * `documents` - The documents to transform
    ///
    /// # Returns
    ///
    /// A vector of transformed documents
    async fn atransform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        self.transform_documents(documents)
    }
}

/// Extracts content from HTML documents using BeautifulSoup-style parsing.
///
/// This module provides [`BeautifulSoupTransformer`] for extracting text and
/// metadata from HTML documents with configurable tag selection.
pub mod beautiful_soup_transformer;

/// Filters redundant documents using embedding-based clustering.
///
/// This module provides [`EmbeddingsClusteringFilter`] which clusters documents
/// by embedding similarity and returns representative documents from each cluster.
pub mod embeddings_clustering_filter;

/// Filters redundant documents based on embedding similarity.
///
/// This module provides [`EmbeddingsRedundantFilter`] which removes documents
/// that are too similar to already-seen documents based on embedding distance.
pub mod embeddings_redundant_filter;

/// Translates document content using Google Translate API.
///
/// This module provides [`GoogleTranslateTransformer`] for translating documents
/// between languages using the Google Cloud Translation API.
pub mod google_translate_transformer;

/// Converts HTML documents to plain text.
///
/// This module provides [`Html2TextTransformer`] which strips HTML tags and
/// converts content to clean plaintext while preserving structure.
pub mod html2text_transformer;

/// Reorders documents for optimal long context window utilization.
///
/// This module provides [`LongContextReorder`] which reorders documents to place
/// the most relevant ones at the beginning and end of the context, mitigating
/// the "lost in the middle" phenomenon in LLMs.
pub mod long_context_reorder;

/// Transforms HTML content to Markdown format.
///
/// This module provides [`MarkdownifyTransformer`] which converts HTML documents
/// to clean Markdown, preserving structure while making content more readable
/// and suitable for LLM processing.
pub mod markdownify_transformer;

/// Extracts and tags document metadata using LLMs.
///
/// This module provides [`MetadataTagger`] which uses language models to
/// extract structured metadata from document content, adding key information
/// to document metadata fields.
pub mod metadata_tagger;

/// Transforms text using Nuclia's text processing API.
///
/// This module provides [`NucliaTextTransformer`] for advanced text processing
/// including entity extraction, summarization, and classification.
pub mod nuclia_text_transformer;

pub use beautiful_soup_transformer::BeautifulSoupTransformer;
pub use embeddings_clustering_filter::EmbeddingsClusteringFilter;
pub use embeddings_redundant_filter::EmbeddingsRedundantFilter;
pub use google_translate_transformer::{
    GoogleTranslateConfig, GoogleTranslateTransformer, TranslationParams,
};
pub use html2text_transformer::Html2TextTransformer;
pub use long_context_reorder::LongContextReorder;
pub use markdownify_transformer::{HeadingStyle, MarkdownifyTransformer};
pub use metadata_tagger::MetadataTagger;
pub use nuclia_text_transformer::NucliaTextTransformer;

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    /// Simple test transformer that adds a prefix to document content
    struct PrefixTransformer {
        prefix: String,
    }

    #[async_trait]
    impl DocumentTransformer for PrefixTransformer {
        fn transform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
            Ok(documents
                .into_iter()
                .map(|mut doc| {
                    doc.page_content = format!("{}{}", self.prefix, doc.page_content);
                    doc
                })
                .collect())
        }
    }

    #[test]
    fn test_transform_documents_sync() {
        let transformer = PrefixTransformer {
            prefix: "PREFIX: ".to_string(),
        };
        let docs = vec![
            Document::new("doc1"),
            Document::new("doc2"),
            Document::new("doc3"),
        ];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].page_content, "PREFIX: doc1");
        assert_eq!(result[1].page_content, "PREFIX: doc2");
        assert_eq!(result[2].page_content, "PREFIX: doc3");
    }

    #[tokio::test]
    async fn test_atransform_documents_default_impl() {
        let transformer = PrefixTransformer {
            prefix: "ASYNC: ".to_string(),
        };
        let docs = vec![Document::new("async doc1"), Document::new("async doc2")];

        let result = transformer.atransform_documents(docs).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].page_content, "ASYNC: async doc1");
        assert_eq!(result[1].page_content, "ASYNC: async doc2");
    }

    #[test]
    fn test_transform_empty_documents() {
        let transformer = PrefixTransformer {
            prefix: "PREFIX: ".to_string(),
        };
        let docs = vec![];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_transform_documents_with_metadata() {
        let transformer = PrefixTransformer {
            prefix: "TEST: ".to_string(),
        };
        let mut doc = Document::new("content");
        doc.metadata.insert("key".to_string(), "value".into());
        let docs = vec![doc];

        let result = transformer.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "TEST: content");
        assert_eq!(result[0].metadata.get("key"), Some(&"value".into()));
    }

    /// Test transformer that returns an error
    struct ErrorTransformer;

    #[async_trait]
    impl DocumentTransformer for ErrorTransformer {
        fn transform_documents(&self, _documents: Vec<Document>) -> Result<Vec<Document>> {
            Err(crate::core::error::Error::InvalidInput(
                "Intentional error for testing".to_string(),
            ))
        }
    }

    #[test]
    fn test_transform_documents_error_handling() {
        let transformer = ErrorTransformer;
        let docs = vec![Document::new("doc")];

        let result = transformer.transform_documents(docs);
        assert!(result.is_err());
        match result {
            Err(crate::core::error::Error::InvalidInput(msg)) => {
                assert_eq!(msg, "Intentional error for testing");
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_atransform_documents_error_handling() {
        let transformer = ErrorTransformer;
        let docs = vec![Document::new("doc")];

        let result = transformer.atransform_documents(docs).await;
        assert!(result.is_err());
    }
}
