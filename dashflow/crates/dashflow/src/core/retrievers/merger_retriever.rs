// Allow clippy warnings for merger retriever
// - needless_pass_by_value: query String passed to async retriever calls
#![allow(clippy::needless_pass_by_value)]

// Merger Retriever
//
// Python baseline: ~/dashflow/libs/dashflow/dashflow_classic/retrievers/merger_retriever.py
//
// Merges results from multiple retrievers using a round-robin pattern.

use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

use crate::core::config::RunnableConfig;
use crate::core::documents::Document;
use crate::core::error::Error;
use crate::core::retrievers::{Retriever, RetrieverOutput};
use crate::core::runnable::Runnable;

/// Retriever that merges the results of multiple retrievers.
///
/// This retriever runs multiple retrievers in parallel and merges their results
/// using a round-robin pattern. For each position (0, 1, 2, ...), it takes one
/// document from each retriever's results at that position.
///
/// # Algorithm (from Python baseline)
///
/// ```python
/// # From merger_retriever.py:53-84
/// def merge_documents(self, query: str, run_manager) -> list[Document]:
///     # Get the results of all retrievers.
///     retriever_docs = [
///         retriever.invoke(query, config={"callbacks": run_manager.get_child(f"retriever_{i + 1}")})
///         for i, retriever in enumerate(self.retrievers)
///     ]
///
///     # Merge the results using round-robin pattern.
///     merged_documents = []
///     max_docs = max(map(len, retriever_docs), default=0)
///     for i in range(max_docs):
///         for _retriever, doc in zip(self.retrievers, retriever_docs, strict=False):
///             if i < len(doc):
///                 merged_documents.append(doc[i])
///
///     return merged_documents
/// ```
///
/// # Example
///
/// Given 3 retrievers returning:
/// - Retriever 1: [A1, A2, A3]
/// - Retriever 2: [B1, B2]
/// - Retriever 3: [C1, C2, C3, C4]
///
/// Merged result: [A1, B1, C1, A2, B2, C2, A3, C3, C4]
///
/// # Usage
///
/// ```ignore
/// use dashflow::core::retrievers::{MergerRetriever, BM25Retriever, TFIDFRetriever};
/// use std::sync::Arc;
///
/// let bm25 = Arc::new(BM25Retriever::from_texts(texts, None)?);
/// let tfidf = Arc::new(TFIDFRetriever::from_texts(texts, None)?);
///
/// let merger = MergerRetriever::new(vec![bm25, tfidf]);
/// let docs = merger._get_relevant_documents("query").await?;
/// // docs contains results from both retrievers, interleaved
/// ```
#[derive(Clone)]
pub struct MergerRetriever {
    retrievers: Vec<Arc<dyn Retriever + Send + Sync>>,
}

impl fmt::Debug for MergerRetriever {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MergerRetriever")
            .field("num_retrievers", &self.retrievers.len())
            .finish()
    }
}

impl MergerRetriever {
    /// Create a new merger retriever.
    ///
    /// # Arguments
    ///
    /// * `retrievers` - List of retrievers to merge results from
    ///
    /// # Returns
    ///
    /// A new `MergerRetriever` instance
    ///
    /// # Example
    ///
    /// ```ignore
    /// let merger = MergerRetriever::new(vec![retriever1, retriever2]);
    /// ```
    #[must_use]
    pub fn new(retrievers: Vec<Arc<dyn Retriever + Send + Sync>>) -> Self {
        Self { retrievers }
    }

    /// Get the number of retrievers.
    #[must_use]
    pub fn num_retrievers(&self) -> usize {
        self.retrievers.len()
    }

    /// Merge documents using round-robin pattern.
    ///
    /// For each position i (0, 1, 2, ...), take document at position i
    /// from each retriever that has a document at that position.
    ///
    /// # Arguments
    ///
    /// * `retriever_docs` - Vector of document vectors from each retriever
    ///
    /// # Returns
    ///
    /// Merged vector of documents in round-robin order
    fn merge_documents_impl(&self, retriever_docs: Vec<Vec<Document>>) -> Vec<Document> {
        let mut merged_documents = Vec::new();

        // Find max length among all retriever results
        let max_docs = retriever_docs
            .iter()
            .map(std::vec::Vec::len)
            .max()
            .unwrap_or(0);

        // Round-robin merge: for each position i, take doc[i] from each retriever
        for i in 0..max_docs {
            for docs in &retriever_docs {
                if i < docs.len() {
                    merged_documents.push(docs[i].clone());
                }
            }
        }

        merged_documents
    }
}

#[async_trait]
impl Retriever for MergerRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>, Error> {
        if self.retrievers.is_empty() {
            return Ok(Vec::new());
        }

        // Run all retrievers in parallel
        let mut handles = Vec::new();
        for retriever in &self.retrievers {
            let retriever = Arc::clone(retriever);
            let query = query.to_string();
            let config = config.cloned();
            handles.push(tokio::spawn(async move {
                retriever
                    ._get_relevant_documents(&query, config.as_ref())
                    .await
            }));
        }

        // Collect results
        let mut retriever_docs = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(docs)) => retriever_docs.push(docs),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(Error::other(format!("Failed to join retriever task: {e}"))),
            }
        }

        // Merge using round-robin pattern
        Ok(self.merge_documents_impl(retriever_docs))
    }
}

#[async_trait]
impl Runnable for MergerRetriever {
    type Input = String;
    type Output = RetrieverOutput;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output, Error> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }
}

#[cfg(test)]
mod tests {
    use crate::core::documents::Document;
    use crate::test_prelude::*;

    // Mock retriever for testing
    #[derive(Clone)]
    struct MockRetriever {
        docs: Vec<Document>,
    }

    #[async_trait]
    impl Retriever for MockRetriever {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<Vec<Document>, Error> {
            Ok(self.docs.clone())
        }
    }

    #[async_trait]
    impl Runnable for MockRetriever {
        type Input = String;
        type Output = RetrieverOutput;

        async fn invoke(
            &self,
            input: Self::Input,
            config: Option<RunnableConfig>,
        ) -> StdResult<Self::Output, Error> {
            self._get_relevant_documents(&input, config.as_ref()).await
        }
    }

    #[tokio::test]
    async fn test_merger_retriever_basic() {
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![
                Document::new("A1"),
                Document::new("A2"),
                Document::new("A3"),
            ],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new("B1"), Document::new("B2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever3 = Arc::new(MockRetriever {
            docs: vec![
                Document::new("C1"),
                Document::new("C2"),
                Document::new("C3"),
                Document::new("C4"),
            ],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2, retriever3]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // Expected: [A1, B1, C1, A2, B2, C2, A3, C3, C4]
        assert_eq!(docs.len(), 9);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[1].page_content, "B1");
        assert_eq!(docs[2].page_content, "C1");
        assert_eq!(docs[3].page_content, "A2");
        assert_eq!(docs[4].page_content, "B2");
        assert_eq!(docs[5].page_content, "C2");
        assert_eq!(docs[6].page_content, "A3");
        assert_eq!(docs[7].page_content, "C3");
        assert_eq!(docs[8].page_content, "C4");
    }

    #[tokio::test]
    async fn test_merger_retriever_empty() {
        let merger = MergerRetriever::new(vec![]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();
        assert_eq!(docs.len(), 0);
    }

    #[tokio::test]
    async fn test_merger_retriever_single() {
        let retriever = Arc::new(MockRetriever {
            docs: vec![Document::new("A1"), Document::new("A2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[1].page_content, "A2");
    }

    #[tokio::test]
    async fn test_merger_retriever_all_empty() {
        let retriever1 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;
        let retriever2 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();
        assert_eq!(docs.len(), 0);
    }

    #[tokio::test]
    async fn test_merger_retriever_equal_length() {
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1"), Document::new("A2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new("B1"), Document::new("B2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // Expected: [A1, B1, A2, B2]
        assert_eq!(docs.len(), 4);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[1].page_content, "B1");
        assert_eq!(docs[2].page_content, "A2");
        assert_eq!(docs[3].page_content, "B2");
    }

    #[tokio::test]
    async fn test_runnable_invoke() {
        let retriever = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever]);
        let docs = merger.invoke("query".to_string(), None).await.unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "A1");
    }

    #[test]
    fn test_num_retrievers() {
        let retriever1 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;
        let retriever2 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        assert_eq!(merger.num_retrievers(), 2);
    }

    // Additional comprehensive tests

    #[tokio::test]
    async fn test_merger_retriever_uneven_lengths() {
        // Test with retrievers returning very uneven result counts
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![
                Document::new("B1"),
                Document::new("B2"),
                Document::new("B3"),
                Document::new("B4"),
                Document::new("B5"),
            ],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever3 = Arc::new(MockRetriever {
            docs: vec![Document::new("C1"), Document::new("C2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2, retriever3]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // Expected: [A1, B1, C1, B2, C2, B3, B4, B5]
        assert_eq!(docs.len(), 8);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[1].page_content, "B1");
        assert_eq!(docs[2].page_content, "C1");
        assert_eq!(docs[3].page_content, "B2");
        assert_eq!(docs[4].page_content, "C2");
        assert_eq!(docs[5].page_content, "B3");
        assert_eq!(docs[6].page_content, "B4");
        assert_eq!(docs[7].page_content, "B5");
    }

    #[tokio::test]
    async fn test_merger_retriever_with_metadata() {
        use serde_json::json;

        // Test that documents with metadata are properly merged
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1").with_metadata("source", "retriever1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new("B1").with_metadata("source", "retriever2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[0].metadata.get("source"), Some(&json!("retriever1")));
        assert_eq!(docs[1].page_content, "B1");
        assert_eq!(docs[1].metadata.get("source"), Some(&json!("retriever2")));
    }

    #[tokio::test]
    async fn test_merger_retriever_many_retrievers() {
        // Test with many retrievers (10+)
        let mut retrievers: Vec<Arc<dyn Retriever + Send + Sync>> = Vec::new();

        for i in 0..15 {
            retrievers.push(Arc::new(MockRetriever {
                docs: vec![Document::new(format!("Doc{}", i))],
            }) as Arc<dyn Retriever + Send + Sync>);
        }

        let merger = MergerRetriever::new(retrievers);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        assert_eq!(docs.len(), 15);
        for (i, doc) in docs.iter().enumerate() {
            assert_eq!(doc.page_content, format!("Doc{}", i));
        }
    }

    #[tokio::test]
    async fn test_merger_retriever_large_documents() {
        // Test with large document content (10KB each)
        let large_content = "x".repeat(10000);

        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new(&large_content)],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new(&large_content)],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content.len(), 10000);
        assert_eq!(docs[1].page_content.len(), 10000);
    }

    #[tokio::test]
    async fn test_merger_retriever_one_empty() {
        // Test when one retriever returns empty results
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1"), Document::new("A2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;

        let retriever3 = Arc::new(MockRetriever {
            docs: vec![Document::new("C1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2, retriever3]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // Expected: [A1, C1, A2]
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[1].page_content, "C1");
        assert_eq!(docs[2].page_content, "A2");
    }

    #[tokio::test]
    async fn test_merger_retriever_special_characters() {
        // Test with special characters in document content
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![
                Document::new("Hello\nWorld"),
                Document::new("Tab\tSeparated"),
            ],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new("Quote\"Test"), Document::new("Emoji 😀")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        assert_eq!(docs.len(), 4);
        assert_eq!(docs[0].page_content, "Hello\nWorld");
        assert_eq!(docs[1].page_content, "Quote\"Test");
        assert_eq!(docs[2].page_content, "Tab\tSeparated");
        assert_eq!(docs[3].page_content, "Emoji 😀");
    }

    #[tokio::test]
    async fn test_merger_retriever_clone() {
        // Test that cloning the merger works correctly
        let retriever = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever]);
        let merger_clone = merger.clone();

        let docs = merger_clone
            ._get_relevant_documents("query", None)
            .await
            .unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "A1");
    }

    #[tokio::test]
    async fn test_merger_retriever_debug() {
        // Test Debug trait implementation
        let retriever1 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;
        let retriever2 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let debug_str = format!("{:?}", merger);

        assert!(debug_str.contains("MergerRetriever"));
        assert!(debug_str.contains("num_retrievers"));
        assert!(debug_str.contains("2"));
    }

    #[tokio::test]
    async fn test_merger_retriever_with_config() {
        // Test that RunnableConfig is properly passed through
        let retriever = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever]);

        let config = RunnableConfig::default();
        let docs = merger
            ._get_relevant_documents("query", Some(&config))
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "A1");
    }

    // Mock retriever that returns an error
    #[derive(Clone)]
    struct ErrorRetriever;

    #[async_trait]
    impl Retriever for ErrorRetriever {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<Vec<Document>, Error> {
            Err(Error::other("Retriever error"))
        }
    }

    #[async_trait]
    impl Runnable for ErrorRetriever {
        type Input = String;
        type Output = RetrieverOutput;

        async fn invoke(
            &self,
            input: Self::Input,
            config: Option<RunnableConfig>,
        ) -> StdResult<Self::Output, Error> {
            self._get_relevant_documents(&input, config.as_ref()).await
        }
    }

    #[tokio::test]
    async fn test_merger_retriever_error_handling() {
        // Test that errors from retrievers are properly propagated
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(ErrorRetriever) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let result = merger._get_relevant_documents("query", None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Retriever error"));
    }

    #[tokio::test]
    async fn test_merger_retriever_empty_query() {
        // Test with empty query string
        let retriever = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever]);
        let docs = merger._get_relevant_documents("", None).await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "A1");
    }

    #[tokio::test]
    async fn test_merger_retriever_long_query() {
        // Test with very long query string
        let long_query = "x".repeat(10000);

        let retriever = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever]);
        let docs = merger
            ._get_relevant_documents(&long_query, None)
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "A1");
    }

    #[tokio::test]
    async fn test_merger_retriever_many_documents() {
        // Test with many documents from each retriever (100+)
        let mut docs1 = Vec::new();
        let mut docs2 = Vec::new();
        let mut docs3 = Vec::new();

        for i in 0..100 {
            docs1.push(Document::new(format!("A{}", i)));
            docs2.push(Document::new(format!("B{}", i)));
            docs3.push(Document::new(format!("C{}", i)));
        }

        let retriever1 =
            Arc::new(MockRetriever { docs: docs1 }) as Arc<dyn Retriever + Send + Sync>;
        let retriever2 =
            Arc::new(MockRetriever { docs: docs2 }) as Arc<dyn Retriever + Send + Sync>;
        let retriever3 =
            Arc::new(MockRetriever { docs: docs3 }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2, retriever3]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // Should have 300 documents total, interleaved
        assert_eq!(docs.len(), 300);

        // Check first few for correct round-robin ordering
        assert_eq!(docs[0].page_content, "A0");
        assert_eq!(docs[1].page_content, "B0");
        assert_eq!(docs[2].page_content, "C0");
        assert_eq!(docs[3].page_content, "A1");
        assert_eq!(docs[4].page_content, "B1");
        assert_eq!(docs[5].page_content, "C1");
    }

    #[test]
    fn test_merger_retriever_num_retrievers_zero() {
        let merger = MergerRetriever::new(vec![]);
        assert_eq!(merger.num_retrievers(), 0);
    }

    #[tokio::test]
    async fn test_merger_retriever_duplicate_documents() {
        // Test that duplicate documents are not deduplicated (preserving Python behavior)
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1"), Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // All 3 documents should be returned, even though they're duplicates
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[1].page_content, "A1");
        assert_eq!(docs[2].page_content, "A1");
    }

    #[tokio::test]
    async fn test_merger_retriever_runnable_with_config() {
        // Test Runnable trait with config
        let retriever = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever]);

        let config = RunnableConfig::default();
        let docs = merger
            .invoke("query".to_string(), Some(config))
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "A1");
    }

    #[tokio::test]
    async fn test_merger_retriever_first_empty_others_not() {
        // Test when first retriever is empty but others have results
        let retriever1 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new("B1"), Document::new("B2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever3 = Arc::new(MockRetriever {
            docs: vec![Document::new("C1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2, retriever3]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // Expected: [B1, C1, B2]
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "B1");
        assert_eq!(docs[1].page_content, "C1");
        assert_eq!(docs[2].page_content, "B2");
    }

    #[tokio::test]
    async fn test_merger_retriever_last_empty_others_not() {
        // Test when last retriever is empty but others have results
        let retriever1 = Arc::new(MockRetriever {
            docs: vec![Document::new("A1")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever2 = Arc::new(MockRetriever {
            docs: vec![Document::new("B1"), Document::new("B2")],
        }) as Arc<dyn Retriever + Send + Sync>;

        let retriever3 =
            Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever + Send + Sync>;

        let merger = MergerRetriever::new(vec![retriever1, retriever2, retriever3]);
        let docs = merger._get_relevant_documents("query", None).await.unwrap();

        // Expected: [A1, B1, B2]
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "A1");
        assert_eq!(docs[1].page_content, "B1");
        assert_eq!(docs[2].page_content, "B2");
    }
}
