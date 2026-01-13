//! BM25 retriever without Elasticsearch.
//!
//! BM25 (Best Matching 25) is a probabilistic ranking function used by search engines
//! to estimate the relevance of documents to a given search query. This retriever
//! implements BM25 scoring locally without requiring an Elasticsearch instance.
//!
//! # Algorithm
//!
//! BM25 scores documents based on term frequency (TF) and inverse document frequency (IDF):
//!
//! ```text
//! score(D,Q) = Σ IDF(qi) * (f(qi,D) * (k1 + 1)) / (f(qi,D) + k1 * (1 - b + b * |D| / avgdl))
//! ```
//!
//! Where:
//! - `D` is a document
//! - `Q` is a query
//! - `qi` is a query term
//! - `f(qi,D)` is term frequency of qi in D
//! - `|D|` is document length
//! - `avgdl` is average document length
//! - `k1` and `b` are tuning parameters (typically k1=1.5, b=0.75)
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::retrievers::{Retriever, BM25Retriever};
//! use dashflow::core::documents::Document;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let docs = vec![
//!     Document::new("The quick brown fox jumps over the lazy dog"),
//!     Document::new("Machine learning is a subset of artificial intelligence"),
//!     Document::new("Rust is a systems programming language"),
//! ];
//!
//! let retriever = BM25Retriever::from_documents(docs, None)?;
//! let results = retriever._get_relevant_documents("machine learning", None).await?;
//! # Ok(())
//! # }
//! ```

use crate::core::{
    config::RunnableConfig,
    documents::Document,
    error::{Error, Result},
    retrievers::{Retriever, RetrieverInput, RetrieverOutput},
    runnable::Runnable,
};
use async_trait::async_trait;
use std::collections::HashMap;

/// Default preprocessing function that splits text on whitespace.
#[must_use]
pub fn default_preprocessing_func(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}

/// BM25 retriever without Elasticsearch.
///
/// Implements the BM25 ranking algorithm for document retrieval. Documents are
/// scored based on term frequency, document length, and inverse document frequency.
///
/// # BM25 Parameters
///
/// - `k1`: Controls term frequency saturation (default: 1.5). Higher values give more weight to repeated terms.
/// - `b`: Controls length normalization (default: 0.75). 0 = no normalization, 1 = full normalization.
///
/// # Fields
///
/// - `docs`: List of documents to search
/// - `k`: Number of top documents to return (default: 4)
/// - `k1`: BM25 parameter for term frequency saturation
/// - `b`: BM25 parameter for length normalization
/// - `preprocess_func`: Function to tokenize text before scoring
#[derive(Clone)]
pub struct BM25Retriever {
    /// List of documents to search
    docs: Vec<Document>,

    /// Number of documents to return
    k: usize,

    /// BM25 parameter k1 (term frequency saturation)
    k1: f64,

    /// BM25 parameter b (length normalization)
    b: f64,

    /// Preprocessed document tokens
    doc_tokens: Vec<Vec<String>>,

    /// Document lengths
    doc_lengths: Vec<usize>,

    /// Average document length
    avg_doc_length: f64,

    /// IDF scores for terms
    idf_scores: HashMap<String, f64>,
}

impl BM25Retriever {
    /// Create a new `BM25Retriever` from a list of documents.
    ///
    /// # Arguments
    ///
    /// * `docs` - List of documents to index
    /// * `k` - Number of documents to return (default: 4)
    /// * `k1` - BM25 k1 parameter (default: 1.5)
    /// * `b` - BM25 b parameter (default: 0.75)
    ///
    /// # Returns
    ///
    /// A `BM25Retriever` ready for queries
    pub fn new(docs: Vec<Document>, k: usize, k1: f64, b: f64) -> Result<Self> {
        if docs.is_empty() {
            return Err(Error::config(
                "BM25Retriever requires at least one document",
            ));
        }

        // Preprocess all documents
        let doc_tokens: Vec<Vec<String>> = docs
            .iter()
            .map(|doc| default_preprocessing_func(&doc.page_content))
            .collect();

        // Calculate document lengths
        let doc_lengths: Vec<usize> = doc_tokens.iter().map(std::vec::Vec::len).collect();

        // Calculate average document length
        let total_length: usize = doc_lengths.iter().sum();
        let avg_doc_length = total_length as f64 / docs.len() as f64;

        // Calculate IDF scores
        let idf_scores = Self::calculate_idf(&doc_tokens);

        Ok(Self {
            docs,
            k,
            k1,
            b,
            doc_tokens,
            doc_lengths,
            avg_doc_length,
            idf_scores,
        })
    }

    /// Create a `BM25Retriever` from documents with default parameters.
    pub fn from_documents(docs: Vec<Document>, k: Option<usize>) -> Result<Self> {
        Self::new(docs, k.unwrap_or(4), 1.5, 0.75)
    }

    /// Create a `BM25Retriever` from texts with optional metadata.
    pub fn from_texts(
        texts: Vec<String>,
        metadatas: Option<Vec<HashMap<String, serde_json::Value>>>,
        k: Option<usize>,
    ) -> Result<Self> {
        let docs: Vec<Document> = if let Some(metas) = metadatas {
            texts
                .into_iter()
                .zip(metas)
                .map(|(text, meta)| Document {
                    page_content: text,
                    metadata: meta,
                    id: None,
                })
                .collect()
        } else {
            texts.into_iter().map(Document::new).collect()
        };

        Self::from_documents(docs, k)
    }

    /// Set the k1 parameter for BM25 scoring.
    ///
    /// k1 controls term frequency saturation. Higher values give more weight to repeated terms.
    /// Typical range: 1.2 to 2.0, default: 1.5
    pub fn set_k1(&mut self, k1: f64) {
        self.k1 = k1;
    }

    /// Set the b parameter for BM25 scoring.
    ///
    /// b controls length normalization. 0 = no normalization, 1 = full normalization.
    /// Typical range: 0.5 to 0.9, default: 0.75
    pub fn set_b(&mut self, b: f64) {
        self.b = b;
    }

    /// Calculate IDF (Inverse Document Frequency) scores for all terms.
    ///
    /// IDF = log((N - df + 0.5) / (df + 0.5) + 1)
    ///
    /// Where:
    /// - N is the total number of documents
    /// - df is the number of documents containing the term
    fn calculate_idf(doc_tokens: &[Vec<String>]) -> HashMap<String, f64> {
        let n = doc_tokens.len() as f64;
        let mut term_doc_freq: HashMap<String, usize> = HashMap::new();

        // Count document frequency for each term
        for tokens in doc_tokens {
            let mut seen_terms = std::collections::HashSet::new();
            for token in tokens {
                if seen_terms.insert(token.clone()) {
                    *term_doc_freq.entry(token.clone()).or_insert(0) += 1;
                }
            }
        }

        // Calculate IDF for each term
        term_doc_freq
            .into_iter()
            .map(|(term, df)| {
                let df = df as f64;
                let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                (term, idf)
            })
            .collect()
    }

    /// Calculate BM25 score for a document given query tokens.
    fn score_document(&self, query_tokens: &[String], doc_idx: usize) -> f64 {
        let doc_tokens = &self.doc_tokens[doc_idx];
        let doc_length = self.doc_lengths[doc_idx] as f64;

        // Count term frequencies in document
        let mut term_freq: HashMap<&str, usize> = HashMap::new();
        for token in doc_tokens {
            *term_freq.entry(token.as_str()).or_insert(0) += 1;
        }

        // Calculate BM25 score
        let mut score = 0.0;
        for query_token in query_tokens {
            if let Some(&tf) = term_freq.get(query_token.as_str()) {
                if let Some(&idf) = self.idf_scores.get(query_token) {
                    let tf = tf as f64;
                    let norm = 1.0 - self.b + self.b * (doc_length / self.avg_doc_length);
                    score += idf * (tf * (self.k1 + 1.0)) / (tf + self.k1 * norm);
                }
            }
        }

        score
    }

    /// Get top k documents for a query.
    fn get_top_n(&self, query: &str) -> Vec<Document> {
        let query_tokens = default_preprocessing_func(query);

        // Score all documents
        let mut scores: Vec<(usize, f64)> = (0..self.docs.len())
            .map(|idx| (idx, self.score_document(&query_tokens, idx)))
            .collect();

        // Sort by score (descending)
        scores.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        // Return top k documents
        scores
            .into_iter()
            .take(self.k)
            .map(|(idx, _)| self.docs[idx].clone())
            .collect()
    }
}

#[async_trait]
impl Retriever for BM25Retriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        Ok(self.get_top_n(query))
    }

    fn name(&self) -> String {
        "BM25Retriever".to_string()
    }
}

#[async_trait]
impl Runnable for BM25Retriever {
    type Input = RetrieverInput;
    type Output = RetrieverOutput;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }

    fn name(&self) -> String {
        "BM25Retriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::default_preprocessing_func;
    use crate::test_prelude::*;

    #[tokio::test]
    async fn test_bm25_basic() {
        let docs = vec![
            Document::new("The quick brown fox jumps over the lazy dog"),
            Document::new("Machine learning is a subset of artificial intelligence"),
            Document::new("Rust is a systems programming language"),
            Document::new("Deep learning uses neural networks"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("machine learning", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // The document about machine learning should be first
        assert!(results[0]
            .page_content
            .contains("Machine learning is a subset"));
    }

    #[tokio::test]
    async fn test_bm25_from_texts() {
        let texts = vec![
            "apple orange banana".to_string(),
            "apple grape".to_string(),
            "banana kiwi".to_string(),
        ];

        let retriever = BM25Retriever::from_texts(texts, None, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // Both documents containing "apple" should be returned
        assert!(results[0].page_content.contains("apple"));
        assert!(results[1].page_content.contains("apple"));
    }

    #[tokio::test]
    async fn test_bm25_no_match() {
        let docs = vec![
            Document::new("apple orange banana"),
            Document::new("grape kiwi mango"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("rust programming", None)
            .await
            .unwrap();

        // Should still return documents even with no matches (scored as 0.0)
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_bm25_empty_docs_error() {
        let docs: Vec<Document> = vec![];
        let result = BM25Retriever::from_documents(docs, Some(4));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bm25_as_runnable() {
        let docs = vec![
            Document::new("first document about rust"),
            Document::new("second document about python"),
            Document::new("third document about rust programming"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever.invoke("rust".to_string(), None).await.unwrap();

        assert_eq!(results.len(), 2);
        // Documents with "rust" should be ranked higher
        assert!(results[0].page_content.contains("rust"));
    }

    #[test]
    fn test_preprocessing_func() {
        let tokens = default_preprocessing_func("The Quick BROWN fox");
        assert_eq!(tokens, vec!["the", "quick", "brown", "fox"]);
    }

    #[test]
    fn test_idf_calculation() {
        let doc_tokens = vec![
            vec!["apple".to_string(), "banana".to_string()],
            vec!["apple".to_string(), "cherry".to_string()],
            vec!["banana".to_string(), "cherry".to_string()],
        ];

        let idf = BM25Retriever::calculate_idf(&doc_tokens);

        // All terms appear in 2/3 documents, so they should have similar IDF
        assert!(idf.contains_key("apple"));
        assert!(idf.contains_key("banana"));
        assert!(idf.contains_key("cherry"));

        // IDF should be positive
        assert!(idf["apple"] > 0.0);
    }

    // === Edge Cases ===

    #[tokio::test]
    async fn test_empty_query() {
        let docs = vec![Document::new("apple orange"), Document::new("banana kiwi")];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever._get_relevant_documents("", None).await.unwrap();

        // Empty query should still return k documents (all with score 0.0)
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_query_with_unknown_terms() {
        let docs = vec![Document::new("apple orange"), Document::new("banana kiwi")];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("zzz xxx yyy", None)
            .await
            .unwrap();

        // Query with no matching terms should return documents with 0 score
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_query_with_special_characters() {
        let docs = vec![
            Document::new("hello world!"),
            Document::new("rust-lang programming"),
            Document::new("test@example.com"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("rust-lang", None)
            .await
            .unwrap();

        // Special characters are treated as word boundaries
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_very_long_query() {
        let docs = vec![
            Document::new("short doc"),
            Document::new("another short doc"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let long_query = "word ".repeat(1000);
        let results = retriever
            ._get_relevant_documents(&long_query, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_case_insensitivity() {
        let docs = vec![
            Document::new("Rust Programming"),
            Document::new("rust is great"),
            Document::new("RUST LANGUAGE"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(3)).unwrap();
        let results_upper = retriever
            ._get_relevant_documents("RUST", None)
            .await
            .unwrap();
        let results_lower = retriever
            ._get_relevant_documents("rust", None)
            .await
            .unwrap();

        // Case should not affect results
        assert_eq!(results_upper.len(), results_lower.len());
        for (upper, lower) in results_upper.iter().zip(results_lower.iter()) {
            assert_eq!(upper.page_content, lower.page_content);
        }
    }

    // === Parameter Tuning Tests ===

    #[tokio::test]
    async fn test_k1_parameter_effect() {
        let docs = vec![
            Document::new("rust rust rust programming"),
            Document::new("rust programming"),
        ];

        // Lower k1: less weight to term frequency
        let mut retriever_low = BM25Retriever::from_documents(docs.clone(), Some(2)).unwrap();
        retriever_low.set_k1(0.5);

        // Higher k1: more weight to term frequency
        let mut retriever_high = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        retriever_high.set_k1(2.5);

        let results_low = retriever_low
            ._get_relevant_documents("rust", None)
            .await
            .unwrap();
        let results_high = retriever_high
            ._get_relevant_documents("rust", None)
            .await
            .unwrap();

        // Both should return same documents but potentially different ordering
        assert_eq!(results_low.len(), 2);
        assert_eq!(results_high.len(), 2);
    }

    #[tokio::test]
    async fn test_b_parameter_effect() {
        let docs = vec![
            Document::new("short"),
            Document::new("this is a much longer document with many more words"),
        ];

        // b=0: no length normalization
        let mut retriever_no_norm = BM25Retriever::from_documents(docs.clone(), Some(2)).unwrap();
        retriever_no_norm.set_b(0.0);

        // b=1: full length normalization
        let mut retriever_full_norm = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        retriever_full_norm.set_b(1.0);

        let results_no_norm = retriever_no_norm
            ._get_relevant_documents("document", None)
            .await
            .unwrap();
        let results_full_norm = retriever_full_norm
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        assert_eq!(results_no_norm.len(), 2);
        assert_eq!(results_full_norm.len(), 2);
    }

    #[tokio::test]
    async fn test_custom_k_value() {
        let docs = vec![
            Document::new("doc1"),
            Document::new("doc2"),
            Document::new("doc3"),
            Document::new("doc4"),
            Document::new("doc5"),
        ];

        let retriever_k2 = BM25Retriever::from_documents(docs.clone(), Some(2)).unwrap();
        let retriever_k5 = BM25Retriever::from_documents(docs, Some(5)).unwrap();

        let results_k2 = retriever_k2
            ._get_relevant_documents("doc", None)
            .await
            .unwrap();
        let results_k5 = retriever_k5
            ._get_relevant_documents("doc", None)
            .await
            .unwrap();

        assert_eq!(results_k2.len(), 2);
        assert_eq!(results_k5.len(), 5);
    }

    // === Scoring Behavior Tests ===

    #[tokio::test]
    async fn test_repeated_terms_score_higher() {
        let docs = vec![
            Document::new("apple apple apple"),
            Document::new("apple orange banana"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        // Document with repeated "apple" should rank first
        assert!(results[0].page_content.contains("apple apple apple"));
    }

    #[tokio::test]
    async fn test_document_length_penalty() {
        let docs = vec![
            Document::new("rust programming"),
            Document::new("rust programming with lots of extra words to make this document much longer than the first one"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("rust programming", None)
            .await
            .unwrap();

        // Shorter document should generally rank higher (with default b=0.75)
        assert_eq!(results[0].page_content, "rust programming");
    }

    #[tokio::test]
    async fn test_multi_term_query_scoring() {
        let docs = vec![
            Document::new("machine learning"),
            Document::new("machine"),
            Document::new("learning"),
            Document::new("deep learning and machine intelligence"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(4)).unwrap();
        let results = retriever
            ._get_relevant_documents("machine learning", None)
            .await
            .unwrap();

        // Document with both terms should rank highest
        assert_eq!(results[0].page_content, "machine learning");
    }

    #[tokio::test]
    async fn test_rare_term_scores_higher() {
        let docs = vec![
            Document::new("common common common"),
            Document::new("common common rare"),
            Document::new("common rare rare"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(3)).unwrap();
        let results = retriever
            ._get_relevant_documents("rare", None)
            .await
            .unwrap();

        // Documents with "rare" (less common term) should rank higher
        let top_doc = &results[0].page_content;
        assert!(top_doc.contains("rare"));
    }

    // === Metadata Handling ===

    #[tokio::test]
    async fn test_from_texts_with_metadata() {
        let texts = vec![
            "first document".to_string(),
            "second document".to_string(),
            "third document".to_string(),
        ];

        let mut meta1 = HashMap::new();
        meta1.insert("id".to_string(), serde_json::json!(1));
        meta1.insert("author".to_string(), serde_json::json!("Alice"));

        let mut meta2 = HashMap::new();
        meta2.insert("id".to_string(), serde_json::json!(2));
        meta2.insert("author".to_string(), serde_json::json!("Bob"));

        let mut meta3 = HashMap::new();
        meta3.insert("id".to_string(), serde_json::json!(3));
        meta3.insert("author".to_string(), serde_json::json!("Charlie"));

        let metadatas = vec![meta1, meta2, meta3];

        let retriever = BM25Retriever::from_texts(texts, Some(metadatas), Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].metadata.contains_key("id"));
        assert!(results[0].metadata.contains_key("author"));
    }

    #[tokio::test]
    async fn test_metadata_preserved_after_retrieval() {
        let mut meta = HashMap::new();
        meta.insert("source".to_string(), serde_json::json!("test.txt"));
        meta.insert("page".to_string(), serde_json::json!(42));

        let doc = Document {
            page_content: "test document with metadata".to_string(),
            metadata: meta,
            id: Some("doc-123".to_string()),
        };

        let retriever = BM25Retriever::from_documents(vec![doc], Some(1)).unwrap();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata["source"], "test.txt");
        assert_eq!(results[0].metadata["page"], 42);
        assert_eq!(results[0].id.as_ref().unwrap(), "doc-123");
    }

    // === IDF Calculation Tests ===

    #[test]
    fn test_idf_single_document() {
        let doc_tokens = vec![vec!["apple".to_string(), "banana".to_string()]];

        let idf = BM25Retriever::calculate_idf(&doc_tokens);

        // Terms appearing in all documents should have lowest IDF
        assert!(idf.contains_key("apple"));
        assert!(idf.contains_key("banana"));
        assert!(idf["apple"] > 0.0); // Still positive due to smoothing
        assert!(idf["banana"] > 0.0);
    }

    #[test]
    fn test_idf_all_unique_terms() {
        let doc_tokens = vec![
            vec!["apple".to_string()],
            vec!["banana".to_string()],
            vec!["cherry".to_string()],
        ];

        let idf = BM25Retriever::calculate_idf(&doc_tokens);

        // All terms appear in only 1/3 documents, so they should have same (high) IDF
        let idf_values: Vec<f64> = idf.values().copied().collect();
        assert_eq!(idf_values.len(), 3);
        // All should be equal
        assert!((idf_values[0] - idf_values[1]).abs() < 1e-10);
        assert!((idf_values[1] - idf_values[2]).abs() < 1e-10);
    }

    #[test]
    fn test_idf_repeated_term_in_document() {
        let doc_tokens = vec![
            vec!["apple".to_string(), "apple".to_string()],
            vec!["banana".to_string()],
        ];

        let idf = BM25Retriever::calculate_idf(&doc_tokens);

        // "apple" appears in 1 document (counted once per doc, not per occurrence)
        assert!(idf.contains_key("apple"));
        assert!(idf.contains_key("banana"));
        // Both should have same IDF since both appear in 1/2 documents
        assert!((idf["apple"] - idf["banana"]).abs() < 1e-10);
    }

    // === Concurrent Access Tests ===

    #[tokio::test]
    async fn test_concurrent_queries() {
        let docs = vec![
            Document::new("machine learning artificial intelligence"),
            Document::new("deep learning neural networks"),
            Document::new("rust programming systems"),
            Document::new("python data science"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();

        // Run multiple queries concurrently
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let r = retriever.clone();
                tokio::spawn(async move {
                    let query = if i % 2 == 0 {
                        "learning"
                    } else {
                        "programming"
                    };
                    r._get_relevant_documents(query, None).await
                })
            })
            .collect();

        for handle in handles {
            let results = handle.await.unwrap().unwrap();
            assert_eq!(results.len(), 2);
        }
    }

    #[tokio::test]
    async fn test_clone_retriever() {
        let docs = vec![
            Document::new("original document"),
            Document::new("another document"),
        ];

        let retriever1 = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let retriever2 = retriever1.clone();

        let results1 = retriever1
            ._get_relevant_documents("document", None)
            .await
            .unwrap();
        let results2 = retriever2
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        assert_eq!(results1.len(), results2.len());
        assert_eq!(results1[0].page_content, results2[0].page_content);
    }

    // === Runnable Trait Tests ===

    #[tokio::test]
    async fn test_runnable_invoke_with_config() {
        let docs = vec![
            Document::new("rust language"),
            Document::new("python language"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(2)).unwrap();
        let config = RunnableConfig::default();
        let results = retriever
            .invoke("rust".to_string(), Some(config))
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].page_content.contains("rust"));
    }

    #[tokio::test]
    async fn test_runnable_name() {
        let docs = vec![Document::new("test")];
        let retriever = BM25Retriever::from_documents(docs, Some(1)).unwrap();
        assert_eq!(Retriever::name(&retriever), "BM25Retriever");
        assert_eq!(Runnable::name(&retriever), "BM25Retriever");
    }

    // === Stress Tests ===

    #[tokio::test]
    async fn test_large_document_corpus() {
        let docs: Vec<Document> = (0..1000)
            .map(|i| Document::new(format!("document number {}", i)))
            .collect();

        let retriever = BM25Retriever::from_documents(docs, Some(10)).unwrap();
        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_single_document_corpus() {
        let docs = vec![Document::new("single document")];

        let retriever = BM25Retriever::from_documents(docs, Some(5)).unwrap();
        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        // Should return 1 document even though k=5
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_k_larger_than_corpus() {
        let docs = vec![
            Document::new("doc1"),
            Document::new("doc2"),
            Document::new("doc3"),
        ];

        let retriever = BM25Retriever::from_documents(docs, Some(10)).unwrap();
        let results = retriever._get_relevant_documents("doc", None).await.unwrap();

        // Should return only 3 documents even though k=10
        assert_eq!(results.len(), 3);
    }

    // === Preprocessing Tests ===

    #[test]
    fn test_preprocessing_whitespace_handling() {
        let tokens = default_preprocessing_func("  multiple   spaces   ");
        assert_eq!(tokens, vec!["multiple", "spaces"]);
    }

    #[test]
    fn test_preprocessing_empty_string() {
        let tokens = default_preprocessing_func("");
        assert_eq!(tokens, Vec::<String>::new());
    }

    #[test]
    fn test_preprocessing_mixed_case() {
        let tokens = default_preprocessing_func("MiXeD CaSe TeXt");
        assert_eq!(tokens, vec!["mixed", "case", "text"]);
    }

    #[test]
    fn test_preprocessing_unicode() {
        let tokens = default_preprocessing_func("hello 世界 مرحبا");
        assert_eq!(tokens, vec!["hello", "世界", "مرحبا"]);
    }
}
