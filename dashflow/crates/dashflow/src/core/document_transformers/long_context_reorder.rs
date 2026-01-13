//! Long context reordering transformer.
//!
//! This transformer reorders documents to mitigate the "lost in the middle" problem,
//! where LLMs have difficulty accessing information in the middle of long contexts.
//! The reordering places less relevant documents in the middle and more relevant
//! documents at the beginning and end.
//!
//! Based on: <https://arxiv.org/abs/2307.03172>

use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::error::Result;
use async_trait::async_trait;

/// Reorder documents for long context windows.
///
/// Performance degrades when models must access relevant information in the middle
/// of long contexts. This transformer reorders documents so that:
/// - More relevant documents are placed at the beginning and end
/// - Less relevant documents are placed in the middle
///
/// The algorithm assumes documents are ordered by relevance (most relevant first)
/// and applies a "lost in the middle" reordering pattern.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::{LongContextReorder, DocumentTransformer};
/// use dashflow::core::documents::Document;
///
/// let reorder = LongContextReorder::new();
/// let docs = vec![
///     Document::new("Most relevant"),
///     Document::new("Second most"),
///     Document::new("Third most"),
///     Document::new("Fourth most"),
/// ];
///
/// let reordered = reorder.transform_documents(docs)?;
/// // Result order: [Fourth most, Second most, Most relevant, Third most]
/// // More relevant docs at beginning/end, less relevant in middle
/// ```
///
/// # Python Baseline
///
/// Python: `dashflow_community/document_transformers/long_context_reorder.py`
pub struct LongContextReorder;

impl LongContextReorder {
    /// Create a new `LongContextReorder` transformer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for LongContextReorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Perform lost-in-the-middle reordering.
///
/// The algorithm:
/// 1. Reverse the input documents
/// 2. Iterate through reversed documents
/// 3. For even indices: insert at the beginning
/// 4. For odd indices: append at the end
///
/// This places less relevant documents (middle of original list) in the middle
/// of the result, and more relevant documents (beginning/end of original list)
/// at the beginning/end of the result.
///
/// # Python Algorithm (from Python baseline)
///
/// ```python
/// def _litm_reordering(documents: List[Document]) -> List[Document]:
///     documents.reverse()
///     reordered_result = []
///     for i, value in enumerate(documents):
///         if i % 2 == 1:
///             reordered_result.append(value)
///         else:
///             reordered_result.insert(0, value)
///     return reordered_result
/// ```
fn litm_reordering(mut documents: Vec<Document>) -> Vec<Document> {
    // Reverse documents (start with least relevant)
    documents.reverse();

    let mut reordered = Vec::new();

    // Iterate through reversed documents
    for (i, doc) in documents.into_iter().enumerate() {
        if i % 2 == 1 {
            // Odd index: append to end
            reordered.push(doc);
        } else {
            // Even index: insert at beginning
            reordered.insert(0, doc);
        }
    }

    reordered
}

#[async_trait]
impl DocumentTransformer for LongContextReorder {
    fn transform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        Ok(litm_reordering(documents))
    }

    async fn atransform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        Ok(litm_reordering(documents))
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_long_context_reorder_basic() {
        let reorder = LongContextReorder::new();
        let docs = vec![
            Document::new("doc1"), // Most relevant
            Document::new("doc2"),
            Document::new("doc3"),
            Document::new("doc4"), // Least relevant
        ];

        let result = reorder.transform_documents(docs).unwrap();

        // After reverse: [doc4, doc3, doc2, doc1]
        // i=0 (doc4): insert at 0 -> [doc4]
        // i=1 (doc3): append -> [doc4, doc3]
        // i=2 (doc2): insert at 0 -> [doc2, doc4, doc3]
        // i=3 (doc1): append -> [doc2, doc4, doc3, doc1]
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].page_content, "doc2");
        assert_eq!(result[1].page_content, "doc4");
        assert_eq!(result[2].page_content, "doc3");
        assert_eq!(result[3].page_content, "doc1");
    }

    #[test]
    fn test_long_context_reorder_empty() {
        let reorder = LongContextReorder::new();
        let docs: Vec<Document> = vec![];
        let result = reorder.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_long_context_reorder_single() {
        let reorder = LongContextReorder::new();
        let docs = vec![Document::new("only")];
        let result = reorder.transform_documents(docs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "only");
    }

    #[test]
    fn test_long_context_reorder_two() {
        let reorder = LongContextReorder::new();
        let docs = vec![Document::new("first"), Document::new("second")];
        let result = reorder.transform_documents(docs).unwrap();

        // After reverse: [second, first]
        // i=0 (second): insert at 0 -> [second]
        // i=1 (first): append -> [second, first]
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].page_content, "second");
        assert_eq!(result[1].page_content, "first");
    }

    #[test]
    fn test_long_context_reorder_preserves_metadata() {
        let reorder = LongContextReorder::new();
        let docs = vec![
            Document::new("doc1").with_metadata("rank", 1),
            Document::new("doc2").with_metadata("rank", 2),
            Document::new("doc3").with_metadata("rank", 3),
        ];

        let result = reorder.transform_documents(docs).unwrap();

        // Verify metadata is preserved
        assert_eq!(result.len(), 3);
        for doc in result {
            assert!(doc.metadata.contains_key("rank"));
        }
    }

    #[tokio::test]
    async fn test_long_context_reorder_async() {
        let reorder = LongContextReorder::new();
        let docs = vec![
            Document::new("doc1"),
            Document::new("doc2"),
            Document::new("doc3"),
        ];

        let result = reorder.atransform_documents(docs).await.unwrap();
        assert_eq!(result.len(), 3);
        // After reordering: [doc1, doc3, doc2]
        assert_eq!(result[0].page_content, "doc1");
        assert_eq!(result[1].page_content, "doc3");
        assert_eq!(result[2].page_content, "doc2");
    }
}
