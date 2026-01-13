//! TF-IDF retriever for document retrieval.
//!
//! TF-IDF (Term Frequency-Inverse Document Frequency) is a numerical statistic
//! that reflects how important a word is to a document in a collection. This
//! retriever uses TF-IDF vectors and cosine similarity to find relevant documents.
//!
//! # Algorithm
//!
//! 1. **TF (Term Frequency)**: Measures how frequently a term occurs in a document
//! 2. **IDF (Inverse Document Frequency)**: Measures how important a term is across documents
//! 3. **TF-IDF**: TF * IDF - gives weight to terms that are frequent in a document but rare across the corpus
//! 4. **Cosine Similarity**: Measures similarity between query and document vectors
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::retrievers::{Retriever, TFIDFRetriever};
//! use dashflow::core::documents::Document;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let docs = vec![
//!     Document::new("The quick brown fox jumps over the lazy dog"),
//!     Document::new("Machine learning is a subset of artificial intelligence"),
//!     Document::new("Rust is a systems programming language"),
//! ];
//!
//! let retriever = TFIDFRetriever::from_documents(docs, None)?;
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

/// TF-IDF retriever.
///
/// Implements TF-IDF (Term Frequency-Inverse Document Frequency) scoring for
/// document retrieval. Documents are represented as TF-IDF weighted vectors,
/// and retrieval is performed using cosine similarity.
///
/// # Fields
///
/// - `docs`: List of documents to search
/// - `k`: Number of top documents to return (default: 4)
/// - `tfidf_matrix`: TF-IDF vectors for all documents
/// - `vocabulary`: Mapping from terms to indices in the TF-IDF vectors
#[derive(Clone)]
pub struct TFIDFRetriever {
    /// List of documents to search
    docs: Vec<Document>,

    /// Number of documents to return
    k: usize,

    /// TF-IDF matrix (one vector per document)
    tfidf_matrix: Vec<Vec<f64>>,

    /// Vocabulary mapping terms to indices
    vocabulary: HashMap<String, usize>,

    /// Document frequencies for each term
    doc_frequencies: Vec<usize>,
}

impl TFIDFRetriever {
    /// Create a new `TFIDFRetriever` from a list of documents.
    ///
    /// # Arguments
    ///
    /// * `docs` - List of documents to index
    /// * `k` - Number of documents to return (default: 4)
    ///
    /// # Returns
    ///
    /// A `TFIDFRetriever` ready for queries
    pub fn new(docs: Vec<Document>, k: usize) -> Result<Self> {
        if docs.is_empty() {
            return Err(Error::config(
                "TFIDFRetriever requires at least one document",
            ));
        }

        // Tokenize all documents
        let tokenized_docs: Vec<Vec<String>> = docs
            .iter()
            .map(|doc| Self::tokenize(&doc.page_content))
            .collect();

        // Build vocabulary
        let vocabulary = Self::build_vocabulary(&tokenized_docs);

        // Calculate document frequencies
        let doc_frequencies = Self::calculate_doc_frequencies(&tokenized_docs, &vocabulary);

        // Build TF-IDF matrix
        let tfidf_matrix =
            Self::build_tfidf_matrix(&tokenized_docs, &vocabulary, &doc_frequencies, docs.len());

        Ok(Self {
            docs,
            k,
            tfidf_matrix,
            vocabulary,
            doc_frequencies,
        })
    }

    /// Create a `TFIDFRetriever` from documents with default parameters.
    pub fn from_documents(docs: Vec<Document>, k: Option<usize>) -> Result<Self> {
        Self::new(docs, k.unwrap_or(4))
    }

    /// Create a `TFIDFRetriever` from texts with optional metadata.
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

    /// Tokenize text into lowercase words.
    fn tokenize(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(str::to_lowercase)
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Build vocabulary from tokenized documents.
    fn build_vocabulary(tokenized_docs: &[Vec<String>]) -> HashMap<String, usize> {
        let mut vocab = HashMap::new();
        let mut idx = 0;

        for doc in tokenized_docs {
            for token in doc {
                if !vocab.contains_key(token) {
                    vocab.insert(token.clone(), idx);
                    idx += 1;
                }
            }
        }

        vocab
    }

    /// Calculate document frequencies for each term.
    fn calculate_doc_frequencies(
        tokenized_docs: &[Vec<String>],
        vocabulary: &HashMap<String, usize>,
    ) -> Vec<usize> {
        let mut doc_freqs = vec![0; vocabulary.len()];

        for doc in tokenized_docs {
            let mut seen = vec![false; vocabulary.len()];
            for token in doc {
                if let Some(&idx) = vocabulary.get(token) {
                    if !seen[idx] {
                        doc_freqs[idx] += 1;
                        seen[idx] = true;
                    }
                }
            }
        }

        doc_freqs
    }

    /// Build TF-IDF matrix for all documents.
    fn build_tfidf_matrix(
        tokenized_docs: &[Vec<String>],
        vocabulary: &HashMap<String, usize>,
        doc_frequencies: &[usize],
        num_docs: usize,
    ) -> Vec<Vec<f64>> {
        tokenized_docs
            .iter()
            .map(|doc| Self::compute_tfidf_vector(doc, vocabulary, doc_frequencies, num_docs))
            .collect()
    }

    /// Compute TF-IDF vector for a single document.
    fn compute_tfidf_vector(
        tokens: &[String],
        vocabulary: &HashMap<String, usize>,
        doc_frequencies: &[usize],
        num_docs: usize,
    ) -> Vec<f64> {
        let mut vector = vec![0.0; vocabulary.len()];

        // Calculate term frequencies
        let mut term_freq: HashMap<&str, usize> = HashMap::new();
        for token in tokens {
            *term_freq.entry(token.as_str()).or_insert(0) += 1;
        }

        // Calculate TF-IDF
        let doc_length = tokens.len() as f64;
        for (term, &freq) in &term_freq {
            if let Some(&idx) = vocabulary.get(*term) {
                let tf = freq as f64 / doc_length.max(1.0);
                let idf = ((num_docs as f64) / (doc_frequencies[idx] as f64 + 1.0)).ln() + 1.0;
                vector[idx] = tf * idf;
            }
        }

        vector
    }

    /// Calculate cosine similarity between two vectors.
    fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
        let dot_product: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Get top k documents for a query.
    fn get_top_n(&self, query: &str) -> Vec<Document> {
        // Tokenize query
        let query_tokens = Self::tokenize(query);

        // Compute TF-IDF vector for query
        let query_vector = Self::compute_tfidf_vector(
            &query_tokens,
            &self.vocabulary,
            &self.doc_frequencies,
            self.docs.len(),
        );

        // Calculate similarities with all documents
        let mut scores: Vec<(usize, f64)> = self
            .tfidf_matrix
            .iter()
            .enumerate()
            .map(|(idx, doc_vector)| (idx, Self::cosine_similarity(&query_vector, doc_vector)))
            .collect();

        // Sort by similarity (descending)
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
impl Retriever for TFIDFRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        Ok(self.get_top_n(query))
    }

    fn name(&self) -> String {
        "TFIDFRetriever".to_string()
    }
}

#[async_trait]
impl Runnable for TFIDFRetriever {
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
        "TFIDFRetriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[tokio::test]
    async fn test_tfidf_basic() {
        let docs = vec![
            Document::new("The quick brown fox jumps over the lazy dog"),
            Document::new("Machine learning is a subset of artificial intelligence"),
            Document::new("Rust is a systems programming language"),
            Document::new("Deep learning uses neural networks"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(2)).unwrap();
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
    async fn test_tfidf_from_texts() {
        let texts = vec![
            "apple orange banana".to_string(),
            "apple grape".to_string(),
            "banana kiwi".to_string(),
        ];

        let retriever = TFIDFRetriever::from_texts(texts, None, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // Documents containing "apple" should be returned
        assert!(
            results[0].page_content.contains("apple") || results[1].page_content.contains("apple")
        );
    }

    #[tokio::test]
    async fn test_tfidf_no_match() {
        let docs = vec![
            Document::new("apple orange banana"),
            Document::new("grape kiwi mango"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("rust programming", None)
            .await
            .unwrap();

        // Should still return documents even with no matches (scored as 0.0)
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_tfidf_empty_docs_error() {
        let docs: Vec<Document> = vec![];
        let result = TFIDFRetriever::from_documents(docs, Some(4));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tfidf_as_runnable() {
        let docs = vec![
            Document::new("first document about rust"),
            Document::new("second document about python"),
            Document::new("third document about rust programming"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever.invoke("rust".to_string(), None).await.unwrap();

        assert_eq!(results.len(), 2);
        // Documents with "rust" should be ranked higher
        assert!(
            results[0].page_content.contains("rust") || results[1].page_content.contains("rust")
        );
    }

    #[test]
    fn test_tokenize() {
        let tokens = TFIDFRetriever::tokenize("The Quick BROWN fox");
        assert_eq!(tokens, vec!["the", "quick", "brown", "fox"]);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(TFIDFRetriever::cosine_similarity(&a, &b), 1.0);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert_eq!(TFIDFRetriever::cosine_similarity(&c, &d), 0.0);
    }

    #[test]
    fn test_vocabulary_building() {
        let docs = vec![
            vec!["apple".to_string(), "banana".to_string()],
            vec!["apple".to_string(), "cherry".to_string()],
        ];

        let vocab = TFIDFRetriever::build_vocabulary(&docs);
        assert_eq!(vocab.len(), 3);
        assert!(vocab.contains_key("apple"));
        assert!(vocab.contains_key("banana"));
        assert!(vocab.contains_key("cherry"));
    }

    // ========== Edge Cases ==========

    #[tokio::test]
    async fn test_empty_query() {
        let docs = vec![
            Document::new("first document"),
            Document::new("second document"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever._get_relevant_documents("", None).await.unwrap();

        // Empty query should return documents (zero similarity)
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_single_document() {
        let docs = vec![Document::new("single document about rust")];

        let retriever = TFIDFRetriever::from_documents(docs, Some(4)).unwrap();
        let results = retriever
            ._get_relevant_documents("rust", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_content, "single document about rust");
    }

    #[tokio::test]
    async fn test_k_greater_than_corpus() {
        let docs = vec![Document::new("first"), Document::new("second")];

        let retriever = TFIDFRetriever::from_documents(docs, Some(10)).unwrap();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        // Should return all available documents even if k > corpus size
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_query_with_unknown_terms() {
        let docs = vec![Document::new("apple orange"), Document::new("banana grape")];

        let retriever = TFIDFRetriever::from_documents(docs, Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("unknown totally new words", None)
            .await
            .unwrap();

        // Should still return documents (with zero similarity)
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_long_query() {
        let docs = vec![
            Document::new("machine learning artificial intelligence"),
            Document::new("rust programming systems"),
        ];

        let long_query = "machine learning artificial intelligence neural networks deep learning natural language processing computer vision reinforcement learning supervised unsupervised";
        let retriever = TFIDFRetriever::from_documents(docs, Some(1)).unwrap();
        let results = retriever
            ._get_relevant_documents(long_query, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        // Should match doc with more common terms
        assert!(results[0].page_content.contains("machine"));
    }

    #[tokio::test]
    async fn test_duplicate_documents() {
        let docs = vec![
            Document::new("apple orange"),
            Document::new("apple orange"),
            Document::new("banana"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(3)).unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        // All duplicates should be retrievable
        assert_eq!(results.len(), 3);
    }

    // ========== TF-IDF Calculation Tests ==========

    #[test]
    fn test_tfidf_vector_computation() {
        let docs = vec![
            vec![
                "apple".to_string(),
                "apple".to_string(),
                "banana".to_string(),
            ],
            vec!["apple".to_string(), "cherry".to_string()],
        ];

        let vocab = TFIDFRetriever::build_vocabulary(&docs);
        let doc_freqs = TFIDFRetriever::calculate_doc_frequencies(&docs, &vocab);

        // Compute vector for first doc
        let vector = TFIDFRetriever::compute_tfidf_vector(&docs[0], &vocab, &doc_freqs, 2);

        assert_eq!(vector.len(), 3); // vocab size

        // Apple appears in both docs, should have lower IDF
        // Banana only in first doc, should have higher IDF
        let apple_idx = vocab["apple"];
        let banana_idx = vocab["banana"];

        // Check TF-IDF values are positive
        assert!(vector[apple_idx] > 0.0);
        assert!(vector[banana_idx] > 0.0);
    }

    #[test]
    fn test_document_frequencies() {
        let docs = vec![
            vec![
                "apple".to_string(),
                "apple".to_string(),
                "banana".to_string(),
            ],
            vec!["apple".to_string(), "cherry".to_string()],
            vec!["cherry".to_string(), "date".to_string()],
        ];

        let vocab = TFIDFRetriever::build_vocabulary(&docs);
        let doc_freqs = TFIDFRetriever::calculate_doc_frequencies(&docs, &vocab);

        // Check document frequencies
        assert_eq!(doc_freqs[vocab["apple"]], 2); // appears in 2 docs
        assert_eq!(doc_freqs[vocab["banana"]], 1); // appears in 1 doc
        assert_eq!(doc_freqs[vocab["cherry"]], 2); // appears in 2 docs
        assert_eq!(doc_freqs[vocab["date"]], 1); // appears in 1 doc
    }

    #[test]
    fn test_tfidf_term_frequency_scaling() {
        let docs = vec![
            // First doc: "apple" appears 3 times out of 4 words = 0.75 TF
            vec![
                "apple".to_string(),
                "apple".to_string(),
                "apple".to_string(),
                "banana".to_string(),
            ],
            // Second doc: "apple" appears 1 time out of 2 words = 0.5 TF
            vec!["apple".to_string(), "cherry".to_string()],
        ];

        let vocab = TFIDFRetriever::build_vocabulary(&docs);
        let doc_freqs = TFIDFRetriever::calculate_doc_frequencies(&docs, &vocab);

        let vector1 = TFIDFRetriever::compute_tfidf_vector(&docs[0], &vocab, &doc_freqs, 2);
        let vector2 = TFIDFRetriever::compute_tfidf_vector(&docs[1], &vocab, &doc_freqs, 2);

        let apple_idx = vocab["apple"];

        // Higher term frequency should result in higher TF-IDF
        assert!(vector1[apple_idx] > vector2[apple_idx]);
    }

    // ========== Cosine Similarity Tests ==========

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = TFIDFRetriever::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = TFIDFRetriever::cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = TFIDFRetriever::cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![0.0, 0.0, 0.0];
        let sim = TFIDFRetriever::cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_partial_match() {
        let a = vec![1.0, 1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = TFIDFRetriever::cosine_similarity(&a, &b);
        // Should be between 0 and 1
        assert!(sim > 0.0 && sim < 1.0);
        // cos(45°) ≈ 0.707
        assert!((sim - 0.707).abs() < 0.01);
    }

    // ========== Tokenization Tests ==========

    #[test]
    fn test_tokenize_case_insensitive() {
        let tokens = TFIDFRetriever::tokenize("HELLO WoRlD hello");
        assert_eq!(tokens, vec!["hello", "world", "hello"]);
    }

    #[test]
    fn test_tokenize_empty_string() {
        let tokens = TFIDFRetriever::tokenize("");
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_tokenize_whitespace_only() {
        let tokens = TFIDFRetriever::tokenize("   \t\n  ");
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_tokenize_multiple_spaces() {
        let tokens = TFIDFRetriever::tokenize("hello    world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_special_characters() {
        let tokens = TFIDFRetriever::tokenize("hello, world! how's it?");
        // Note: special characters are not removed, just split
        assert!(tokens.contains(&"hello,".to_string()));
        assert!(tokens.contains(&"world!".to_string()));
    }

    // ========== Metadata Handling ==========

    #[tokio::test]
    async fn test_from_texts_with_metadata() {
        let texts = vec!["first document".to_string(), "second document".to_string()];

        let mut meta1 = HashMap::new();
        meta1.insert("source".to_string(), serde_json::json!("file1.txt"));
        let mut meta2 = HashMap::new();
        meta2.insert("source".to_string(), serde_json::json!("file2.txt"));

        let metadatas = vec![meta1, meta2];

        let retriever = TFIDFRetriever::from_texts(texts, Some(metadatas), Some(2)).unwrap();
        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].metadata.contains_key("source"));
        assert!(results[1].metadata.contains_key("source"));
    }

    #[tokio::test]
    async fn test_metadata_preserved_after_retrieval() {
        let mut meta = HashMap::new();
        meta.insert("id".to_string(), serde_json::json!(123));
        meta.insert("author".to_string(), serde_json::json!("Alice"));

        let doc = Document {
            page_content: "test document".to_string(),
            metadata: meta,
            id: Some("doc-1".to_string()),
        };

        let retriever = TFIDFRetriever::from_documents(vec![doc], Some(1)).unwrap();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].metadata.get("id").unwrap(),
            &serde_json::json!(123)
        );
        assert_eq!(
            results[0].metadata.get("author").unwrap(),
            &serde_json::json!("Alice")
        );
        assert_eq!(results[0].id, Some("doc-1".to_string()));
    }

    // ========== K Parameter Tests ==========

    #[tokio::test]
    async fn test_k_equals_one() {
        let docs = vec![
            Document::new("machine learning"),
            Document::new("rust programming"),
            Document::new("deep learning"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(1)).unwrap();
        let results = retriever
            ._get_relevant_documents("learning", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        // Should return the most relevant document
        assert!(results[0].page_content.contains("learning"));
    }

    #[tokio::test]
    async fn test_k_default() {
        let docs = vec![
            Document::new("doc1"),
            Document::new("doc2"),
            Document::new("doc3"),
            Document::new("doc4"),
            Document::new("doc5"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, None).unwrap();
        let results = retriever._get_relevant_documents("doc", None).await.unwrap();

        // Default k is 4
        assert_eq!(results.len(), 4);
    }

    // ========== Vocabulary Edge Cases ==========

    #[test]
    fn test_vocabulary_empty_docs() {
        let docs: Vec<Vec<String>> = vec![vec![], vec![]];
        let vocab = TFIDFRetriever::build_vocabulary(&docs);
        assert_eq!(vocab.len(), 0);
    }

    #[test]
    fn test_vocabulary_with_duplicates_in_doc() {
        let docs = vec![vec![
            "apple".to_string(),
            "apple".to_string(),
            "apple".to_string(),
        ]];

        let vocab = TFIDFRetriever::build_vocabulary(&docs);
        assert_eq!(vocab.len(), 1);
        assert!(vocab.contains_key("apple"));
    }

    // ========== Runnable Trait Tests ==========

    #[tokio::test]
    async fn test_runnable_invoke() {
        let docs = vec![
            Document::new("rust programming"),
            Document::new("python scripting"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(1)).unwrap();

        let config = RunnableConfig::default();
        let results = retriever
            .invoke("rust".to_string(), Some(config))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].page_content.contains("rust"));
    }

    #[tokio::test]
    async fn test_runnable_name() {
        let docs = vec![Document::new("test")];
        let retriever = TFIDFRetriever::from_documents(docs, Some(1)).unwrap();

        // Test both Retriever and Runnable trait name methods
        assert_eq!(Retriever::name(&retriever), "TFIDFRetriever");
        assert_eq!(Runnable::name(&retriever), "TFIDFRetriever");
    }

    // ========== Stress Tests ==========

    #[tokio::test]
    async fn test_large_corpus() {
        let mut docs = Vec::new();
        for i in 0..100 {
            docs.push(Document::new(format!("document number {} with content", i)));
        }
        docs.push(Document::new("special document about rust programming"));

        let retriever = TFIDFRetriever::from_documents(docs, Some(10)).unwrap();
        let results = retriever
            ._get_relevant_documents("rust programming", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 10);
        // The special document should be highly ranked
        assert!(results.iter().any(|d| d.page_content.contains("special")));
    }

    #[tokio::test]
    async fn test_large_vocabulary() {
        let mut docs = Vec::new();
        // Create 100 documents with unique terms
        for i in 0..100 {
            docs.push(Document::new(format!("unique_term_{} common", i)));
        }

        let retriever = TFIDFRetriever::from_documents(docs, Some(5)).unwrap();
        let results = retriever
            ._get_relevant_documents("unique_term_42", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 5);
        // Should find the specific document
        assert!(results[0].page_content.contains("unique_term_42"));
    }

    #[tokio::test]
    async fn test_long_documents() {
        let long_doc1 = "machine ".repeat(100) + "learning";
        let long_doc2 = "rust ".repeat(100) + "programming";

        let docs = vec![Document::new(&long_doc1), Document::new(&long_doc2)];

        let retriever = TFIDFRetriever::from_documents(docs, Some(1)).unwrap();
        let results = retriever
            ._get_relevant_documents("learning", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].page_content.contains("learning"));
    }

    // ========== Ranking Quality Tests ==========

    #[tokio::test]
    async fn test_exact_match_ranks_highest() {
        let docs = vec![
            Document::new("machine learning is great"),
            Document::new("machine learning artificial intelligence"),
            Document::new("rust programming"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(3)).unwrap();
        let results = retriever
            ._get_relevant_documents("machine learning", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        // Both docs with "machine learning" should rank above rust doc
        assert!(results[0].page_content.contains("machine"));
        assert!(results[1].page_content.contains("machine"));
    }

    #[tokio::test]
    async fn test_rare_terms_boost_ranking() {
        let docs = vec![
            Document::new("common common common unique_term"),
            Document::new("common common common"),
            Document::new("common common common"),
        ];

        let retriever = TFIDFRetriever::from_documents(docs, Some(1)).unwrap();
        let results = retriever
            ._get_relevant_documents("unique_term", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        // Doc with unique term should be first
        assert!(results[0].page_content.contains("unique_term"));
    }
}
