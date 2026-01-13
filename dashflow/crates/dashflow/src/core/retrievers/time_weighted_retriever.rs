//! Time-Weighted Vector Store Retriever
//!
//! Combines embedding similarity with recency using exponential time decay.
//! Documents are scored by both their relevance to the query and how recently
//! they were accessed.
//!
//! # Use Cases
//!
//! - Chat applications where recent messages are more relevant
//! - News/social media where recency matters
//! - Personal memory systems that favor recent interactions
//!
//! # Algorithm
//!
//! For each document, the final score is:
//! ```text
//! score = recency_score + vector_relevance + other_scores
//! recency_score = (1.0 - decay_rate)^hours_passed
//! ```
//!
//! Documents that are retrieved have their `last_accessed_at` timestamp updated,
//! ensuring frequently accessed memories remain fresh.

use crate::core::{
    config::RunnableConfig,
    documents::Document,
    error::{Error, Result},
    retrievers::Retriever,
    runnable::Runnable,
    vector_stores::VectorStore,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Get the hours passed between two timestamps.
fn get_hours_passed(time: DateTime<Utc>, ref_time: DateTime<Utc>) -> f64 {
    let duration = time.signed_duration_since(ref_time);
    duration.num_seconds() as f64 / 3600.0
}

/// Time-weighted vector store retriever.
///
/// Combines embedding similarity with recency using exponential time decay.
/// Documents are scored by both relevance to the query and how recently they
/// were accessed.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retrievers::TimeWeightedVectorStoreRetriever;
///
/// let retriever = TimeWeightedVectorStoreRetriever::new(
///     vectorstore,
///     0.01,  // decay_rate
///     4,     // k (number of docs to return)
/// );
///
/// // Add documents with automatic timestamp management
/// retriever.add_documents(vec![
///     Document::new("Recent news about Rust"),
///     Document::new("Yesterday's update on DashFlow"),
/// ]).await?;
///
/// // Retrieve with time-weighted scoring
/// let docs = retriever._get_relevant_documents("What's new?", None).await?;
/// // Recent documents and frequently accessed ones rank higher
/// ```
///
/// # Scoring Formula
///
/// The final score for each document is:
/// ```text
/// score = (1.0 - decay_rate)^hours_passed + vector_relevance + other_scores
/// ```
///
/// Where:
/// - `decay_rate`: Controls how quickly older documents lose relevance (default 0.01)
/// - `hours_passed`: Time since last access in hours
/// - `vector_relevance`: Similarity score from vector search
/// - `other_scores`: Additional metadata scores (e.g., importance)
#[derive(Clone)]
pub struct TimeWeightedVectorStoreRetriever<VS> {
    /// The vectorstore to store documents and determine salience.
    pub vectorstore: VS,

    /// Keyword arguments to pass to `VectorStore` similarity search.
    pub search_kwargs: HashMap<String, serde_json::Value>,

    /// The memory stream of documents to search through.
    /// Acts as a queue of all documents in chronological order.
    /// Wrapped in `Arc<RwLock>` to allow interior mutability for timestamp updates.
    pub memory_stream: Arc<RwLock<Vec<Document>>>,

    /// The exponential decay factor used as (1.0 - `decay_rate)^(hrs_passed`).
    /// Default: 0.01
    pub decay_rate: f64,

    /// The maximum number of documents to retrieve in a given call.
    /// Default: 4
    pub k: usize,

    /// Other keys in the metadata to factor into the score (e.g., "importance").
    pub other_score_keys: Vec<String>,

    /// The salience to assign memories not retrieved from the vector store.
    /// None assigns no salience to documents not fetched from the vector store.
    pub default_salience: Option<f64>,
}

impl<VS> TimeWeightedVectorStoreRetriever<VS>
where
    VS: VectorStore,
{
    /// Create a new time-weighted retriever.
    ///
    /// # Arguments
    ///
    /// * `vectorstore` - Vector store for similarity search
    /// * `decay_rate` - Exponential decay factor (default 0.01)
    /// * `k` - Number of documents to retrieve (default 4)
    pub fn new(vectorstore: VS, decay_rate: f64, k: usize) -> Self {
        let mut search_kwargs = HashMap::new();
        search_kwargs.insert("k".to_string(), serde_json::json!(100));

        Self {
            vectorstore,
            search_kwargs,
            memory_stream: Arc::new(RwLock::new(Vec::new())),
            decay_rate,
            k,
            other_score_keys: Vec::new(),
            default_salience: None,
        }
    }

    /// Create a retriever with default settings.
    pub fn from_vectorstore(vectorstore: VS) -> Self {
        Self::new(vectorstore, 0.01, 4)
    }

    /// Set the decay rate for time-based scoring.
    #[must_use]
    pub fn with_decay_rate(mut self, decay_rate: f64) -> Self {
        self.decay_rate = decay_rate;
        self
    }

    /// Set the number of documents to retrieve.
    #[must_use]
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set the default salience for documents not in vector store results.
    #[must_use]
    pub fn with_default_salience(mut self, salience: Option<f64>) -> Self {
        self.default_salience = salience;
        self
    }

    /// Add metadata keys to include in scoring.
    #[must_use]
    pub fn with_other_score_keys(mut self, keys: Vec<String>) -> Self {
        self.other_score_keys = keys;
        self
    }

    /// Set custom search kwargs for the vector store.
    #[must_use]
    pub fn with_search_kwargs(mut self, kwargs: HashMap<String, serde_json::Value>) -> Self {
        self.search_kwargs = kwargs;
        self
    }

    /// Get the date from a document's metadata field.
    ///
    /// Supports both datetime objects and Unix timestamps (as floats).
    /// Returns current time if field is missing.
    fn document_get_date(&self, field: &str, document: &Document) -> DateTime<Utc> {
        if let Some(value) = document.metadata.get(field) {
            match value {
                serde_json::Value::String(s) => {
                    // Try to parse ISO 8601 datetime
                    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                        return dt.with_timezone(&Utc);
                    }
                }
                serde_json::Value::Number(n) => {
                    // Unix timestamp (seconds since epoch)
                    if let Some(timestamp) = n.as_f64() {
                        if let Some(dt) = DateTime::from_timestamp(timestamp as i64, 0) {
                            return dt;
                        }
                    }
                }
                _ => {}
            }
        }
        Utc::now()
    }

    /// Compute the combined score for a document.
    ///
    /// Score = `recency_score` + `vector_relevance` + `other_metadata_scores`
    fn get_combined_score(
        &self,
        document: &Document,
        vector_relevance: Option<f64>,
        current_time: DateTime<Utc>,
    ) -> f64 {
        let last_accessed = self.document_get_date("last_accessed_at", document);
        let hours_passed = get_hours_passed(current_time, last_accessed);

        // Exponential decay: (1.0 - decay_rate)^hours_passed
        let mut score = (1.0 - self.decay_rate).powf(hours_passed);

        // Add other metadata scores
        for key in &self.other_score_keys {
            if let Some(value) = document.metadata.get(key) {
                if let Some(score_value) = value.as_f64() {
                    score += score_value;
                }
            }
        }

        // Add vector relevance score
        if let Some(relevance) = vector_relevance {
            score += relevance;
        }

        score
    }

    /// Get salient documents from vector store.
    ///
    /// Returns a map from `buffer_idx` to (document, `relevance_score`).
    async fn get_salient_docs(&self, query: &str) -> Result<HashMap<usize, (Document, f64)>> {
        // Perform similarity search with relevance scores
        let k = self
            .search_kwargs
            .get("k")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(100) as usize;

        let docs_and_scores = self
            .vectorstore
            .similarity_search_with_score(query, k, None)
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Similarity search failed for time-weighted retrieval: {e}"
                ))
            })?;

        let mut results = HashMap::new();
        let memory_stream = self.memory_stream.read().await;

        for (fetched_doc, relevance) in docs_and_scores {
            // Extract buffer_idx from metadata
            if let Some(buffer_idx_value) = fetched_doc.metadata.get("buffer_idx") {
                if let Some(buffer_idx) = buffer_idx_value.as_u64() {
                    let buffer_idx = buffer_idx as usize;
                    // Look up the original document in memory_stream
                    if buffer_idx < memory_stream.len() {
                        let doc = memory_stream[buffer_idx].clone();
                        results.insert(buffer_idx, (doc, f64::from(relevance)));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Re-score and rank documents by combined score.
    ///
    /// Updates `last_accessed_at` for returned documents.
    async fn get_rescored_docs(
        &self,
        docs_and_scores: HashMap<usize, (Document, Option<f64>)>,
    ) -> Vec<Document> {
        let current_time = Utc::now();

        // Compute combined scores
        let mut rescored_docs: Vec<(Document, f64)> = docs_and_scores
            .into_values()
            .map(|(doc, relevance)| {
                let score = self.get_combined_score(&doc, relevance, current_time);
                (doc, score)
            })
            .collect();

        // Sort by score (descending)
        rescored_docs.sort_by(|(_, score_a), (_, score_b)| {
            score_b
                .partial_cmp(score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top k and update last_accessed_at
        let mut result = Vec::new();
        let mut memory_stream = self.memory_stream.write().await;

        for (doc, _) in rescored_docs.into_iter().take(self.k) {
            if let Some(buffer_idx_value) = doc.metadata.get("buffer_idx") {
                if let Some(buffer_idx) = buffer_idx_value.as_u64() {
                    let buffer_idx = buffer_idx as usize;
                    if buffer_idx < memory_stream.len() {
                        // Update timestamp in memory_stream
                        let buffered_doc = &mut memory_stream[buffer_idx];
                        buffered_doc.metadata.insert(
                            "last_accessed_at".to_string(),
                            serde_json::json!(current_time.to_rfc3339()),
                        );
                        result.push(buffered_doc.clone());
                    }
                }
            }
        }

        result
    }

    /// Add documents to the retriever.
    ///
    /// Documents are timestamped with `created_at` and `last_accessed_at`,
    /// assigned a `buffer_idx`, and added to both `memory_stream` and vectorstore.
    ///
    /// # Arguments
    ///
    /// * `documents` - Documents to add
    ///
    /// # Returns
    ///
    /// List of document IDs from the vector store
    pub async fn add_documents(&mut self, documents: Vec<Document>) -> Result<Vec<String>> {
        self.add_documents_with_time(documents, None).await
    }

    /// Add documents with a specific timestamp.
    ///
    /// Internal method that allows specifying `current_time` for testing.
    async fn add_documents_with_time(
        &mut self,
        documents: Vec<Document>,
        current_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<String>> {
        let current_time = current_time.unwrap_or_else(Utc::now);
        let current_time_str = current_time.to_rfc3339();

        let mut memory_stream = self.memory_stream.write().await;

        // Clone and augment documents with timestamps and buffer_idx
        let mut dup_docs = Vec::new();
        for (i, mut doc) in documents.into_iter().enumerate() {
            // Set timestamps if not present
            if !doc.metadata.contains_key("last_accessed_at") {
                doc.metadata.insert(
                    "last_accessed_at".to_string(),
                    serde_json::json!(current_time_str),
                );
            }
            if !doc.metadata.contains_key("created_at") {
                doc.metadata.insert(
                    "created_at".to_string(),
                    serde_json::json!(current_time_str),
                );
            }

            // Assign buffer_idx
            let buffer_idx = memory_stream.len() + i;
            doc.metadata
                .insert("buffer_idx".to_string(), serde_json::json!(buffer_idx));

            dup_docs.push(doc);
        }

        // Add to memory stream
        memory_stream.extend(dup_docs.clone());
        drop(memory_stream); // Release lock before async operation

        // Add to vector store
        let ids = self
            .vectorstore
            .add_documents(&dup_docs, None)
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to add {} documents to time-weighted vector store: {e}",
                    dup_docs.len()
                ))
            })?;

        Ok(ids)
    }
}

#[async_trait]
impl<VS> Retriever for TimeWeightedVectorStoreRetriever<VS>
where
    VS: VectorStore + Send + Sync,
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Start with most recent k documents from memory stream
        let memory_stream = self.memory_stream.read().await;
        let recent_count = self.k.min(memory_stream.len());
        let recent_docs: Vec<Document> =
            memory_stream[memory_stream.len() - recent_count..].to_vec();
        drop(memory_stream); // Release read lock

        let mut docs_and_scores: HashMap<usize, (Document, Option<f64>)> = recent_docs
            .iter()
            .map(|doc| {
                let buffer_idx = doc
                    .metadata
                    .get("buffer_idx")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize;
                (buffer_idx, (doc.clone(), self.default_salience))
            })
            .collect();

        // Get salient documents from vector store and update scores
        let salient_docs = self.get_salient_docs(query).await?;
        for (buffer_idx, (doc, relevance)) in salient_docs {
            docs_and_scores.insert(buffer_idx, (doc, Some(relevance)));
        }

        // Re-score and rank documents
        Ok(self.get_rescored_docs(docs_and_scores).await)
    }

    fn name(&self) -> String {
        "TimeWeightedVectorStoreRetriever".to_string()
    }
}

#[async_trait]
impl<VS> Runnable for TimeWeightedVectorStoreRetriever<VS>
where
    VS: VectorStore + Send + Sync,
{
    type Input = String;
    type Output = Vec<Document>;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.invoke(input, config).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }

    fn name(&self) -> String {
        "TimeWeightedVectorStoreRetriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::get_hours_passed;
    use crate::core::embeddings::Embeddings;
    use crate::core::vector_stores::InMemoryVectorStore;
    use crate::test_prelude::*;
    use std::sync::Arc;

    // Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| vec![i as f32, 0.5, 0.1])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32, 0.5, 0.1])
        }
    }

    #[tokio::test]
    async fn test_time_weighted_retriever_basic() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        // Add documents
        let docs = vec![
            Document::new("The sky is blue"),
            Document::new("The grass is green"),
            Document::new("The sun is bright"),
        ];
        retriever.add_documents(docs).await.unwrap();

        // Verify documents were added to memory stream
        {
            let memory_stream = retriever.memory_stream.read().await;
            assert_eq!(memory_stream.len(), 3);
        }

        // Retrieve documents
        let results = retriever._get_relevant_documents("sky", None).await.unwrap();

        // Should return k=2 documents
        assert_eq!(results.len(), 2);

        // All returned documents should have buffer_idx
        for doc in &results {
            assert!(doc.metadata.contains_key("buffer_idx"));
            assert!(doc.metadata.contains_key("last_accessed_at"));
            assert!(doc.metadata.contains_key("created_at"));
        }
    }

    #[tokio::test]
    async fn test_time_weighted_retriever_timestamp_update() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 1);

        // Add a document
        let docs = vec![Document::new("Important document")];
        retriever.add_documents(docs).await.unwrap();

        let original_timestamp = {
            let memory_stream = retriever.memory_stream.read().await;
            memory_stream[0]
                .metadata
                .get("last_accessed_at")
                .cloned()
                .unwrap()
        };

        // Wait a bit to ensure timestamp difference
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Retrieve the document
        retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        // Timestamp should be updated
        let new_timestamp = {
            let memory_stream = retriever.memory_stream.read().await;
            memory_stream[0]
                .metadata
                .get("last_accessed_at")
                .cloned()
                .unwrap()
        };

        assert_ne!(original_timestamp, new_timestamp);
    }

    #[tokio::test]
    async fn test_time_weighted_retriever_with_default_salience() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever =
            TimeWeightedVectorStoreRetriever::new(store, 0.01, 3).with_default_salience(Some(0.5));

        // Add documents
        let docs = vec![
            Document::new("Alpha"),
            Document::new("Beta"),
            Document::new("Gamma"),
        ];
        retriever.add_documents(docs).await.unwrap();

        // Retrieve - should use default salience for recent docs
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_time_weighted_retriever_other_score_keys() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2)
            .with_other_score_keys(vec!["importance".to_string()]);

        // Add documents with importance scores
        let mut doc1 = Document::new("Low importance");
        doc1.metadata
            .insert("importance".to_string(), serde_json::json!(0.1));

        let mut doc2 = Document::new("High importance");
        doc2.metadata
            .insert("importance".to_string(), serde_json::json!(10.0));

        retriever.add_documents(vec![doc1, doc2]).await.unwrap();

        // Retrieve - high importance doc should rank higher
        let results = retriever
            ._get_relevant_documents("importance", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // First result should be the high importance doc
        assert!(results[0].page_content.contains("High"));
    }

    #[test]
    fn test_get_hours_passed() {
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        let two_hours_ago = now - chrono::Duration::hours(2);

        assert!((get_hours_passed(now, one_hour_ago) - 1.0).abs() < 0.01);
        assert!((get_hours_passed(now, two_hours_ago) - 2.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_time_decay_scoring() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 3);

        let current_time = Utc::now();
        let one_hour_ago = current_time - chrono::Duration::hours(1);

        // Create document with old timestamp
        let mut doc = Document::new("Old document");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(one_hour_ago.to_rfc3339()),
        );
        doc.metadata
            .insert("buffer_idx".to_string(), serde_json::json!(0));

        // Calculate score
        let score = retriever.get_combined_score(&doc, Some(0.5), current_time);

        // Expected: (1.0 - 0.01)^1 + 0.5 ≈ 0.99 + 0.5 = 1.49
        let expected = (1.0 - 0.01_f64).powf(1.0) + 0.5;
        assert!((score - expected).abs() < 0.01);
    }

    // --- New comprehensive tests ---

    #[tokio::test]
    async fn test_empty_query() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        let docs = vec![Document::new("Test document")];
        retriever.add_documents(docs).await.unwrap();

        // Empty query should still work
        let results = retriever._get_relevant_documents("", None).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_empty_memory_stream() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        // No documents added, should return empty
        let results = retriever
            ._get_relevant_documents("query", None)
            .await
            .unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_k_larger_than_documents() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 10);

        let docs = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
        ];
        retriever.add_documents(docs).await.unwrap();

        // k=10 but only 3 docs, should return all 3
        let results = retriever
            ._get_relevant_documents("query", None)
            .await
            .unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_very_high_decay_rate() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.99, 2);

        let current_time = Utc::now();
        let one_hour_ago = current_time - chrono::Duration::hours(1);

        let mut doc = Document::new("Old document");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(one_hour_ago.to_rfc3339()),
        );

        // With decay_rate=0.99, old docs score very low: (1-0.99)^1 = 0.01
        let score = retriever.get_combined_score(&doc, None, current_time);
        let expected = (1.0 - 0.99_f64).powf(1.0);
        assert!((score - expected).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_zero_decay_rate() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.0, 2);

        let current_time = Utc::now();
        let ten_hours_ago = current_time - chrono::Duration::hours(10);

        let mut doc = Document::new("Very old document");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(ten_hours_ago.to_rfc3339()),
        );

        // With decay_rate=0.0, time doesn't matter: (1-0)^10 = 1.0
        let score = retriever.get_combined_score(&doc, None, current_time);
        let expected = 1.0_f64.powf(10.0);
        assert!((score - expected).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_document_with_unix_timestamp() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        // Test Unix timestamp parsing
        let unix_timestamp = 1609459200.0; // 2021-01-01 00:00:00 UTC
        let mut doc = Document::new("Document with Unix timestamp");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(unix_timestamp),
        );

        let dt = retriever.document_get_date("last_accessed_at", &doc);
        assert_eq!(dt.timestamp(), 1609459200);
    }

    #[tokio::test]
    async fn test_document_with_invalid_timestamp() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        // Invalid timestamp should default to current time
        let mut doc = Document::new("Document with invalid timestamp");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!("invalid-date"),
        );

        let dt = retriever.document_get_date("last_accessed_at", &doc);
        let now = Utc::now();
        // Should be very close to current time
        assert!((dt.timestamp() - now.timestamp()).abs() < 5);
    }

    #[tokio::test]
    async fn test_document_with_missing_timestamp() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        // Missing timestamp should default to current time
        let doc = Document::new("Document without timestamp");
        let dt = retriever.document_get_date("last_accessed_at", &doc);
        let now = Utc::now();
        assert!((dt.timestamp() - now.timestamp()).abs() < 5);
    }

    #[tokio::test]
    async fn test_multiple_other_score_keys() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2)
            .with_other_score_keys(vec![
                "importance".to_string(),
                "urgency".to_string(),
                "relevance".to_string(),
            ]);

        let mut doc = Document::new("Multi-scored document");
        doc.metadata
            .insert("importance".to_string(), serde_json::json!(2.0));
        doc.metadata
            .insert("urgency".to_string(), serde_json::json!(3.0));
        doc.metadata
            .insert("relevance".to_string(), serde_json::json!(1.5));

        retriever.add_documents(vec![doc]).await.unwrap();

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_other_score_key_non_numeric() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2)
            .with_other_score_keys(vec!["importance".to_string()]);

        let current_time = Utc::now();
        let mut doc = Document::new("Non-numeric score");
        doc.metadata.insert(
            "importance".to_string(),
            serde_json::json!("very important"), // String, not number
        );
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(current_time.to_rfc3339()),
        );

        // Should not crash, just ignore the non-numeric value
        let score = retriever.get_combined_score(&doc, None, current_time);
        // Score should be just the time decay component
        let expected = (1.0 - 0.01_f64).powf(0.0);
        assert!((score - expected).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::from_vectorstore(store)
            .with_decay_rate(0.05)
            .with_k(5)
            .with_default_salience(Some(0.7))
            .with_other_score_keys(vec!["priority".to_string()]);

        assert_eq!(retriever.decay_rate, 0.05);
        assert_eq!(retriever.k, 5);
        assert_eq!(retriever.default_salience, Some(0.7));
        assert_eq!(retriever.other_score_keys, vec!["priority".to_string()]);
    }

    #[tokio::test]
    async fn test_custom_search_kwargs() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut custom_kwargs = HashMap::new();
        custom_kwargs.insert("k".to_string(), serde_json::json!(50));
        custom_kwargs.insert("filter".to_string(), serde_json::json!({"type": "test"}));

        let retriever = TimeWeightedVectorStoreRetriever::from_vectorstore(store)
            .with_search_kwargs(custom_kwargs);

        assert_eq!(retriever.search_kwargs.get("k").unwrap().as_u64(), Some(50));
        assert!(retriever.search_kwargs.contains_key("filter"));
    }

    #[tokio::test]
    async fn test_buffer_idx_assignment() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 5);

        // Add first batch
        let docs1 = vec![Document::new("Doc 1"), Document::new("Doc 2")];
        retriever.add_documents(docs1).await.unwrap();

        // Add second batch
        let docs2 = vec![Document::new("Doc 3"), Document::new("Doc 4")];
        retriever.add_documents(docs2).await.unwrap();

        // Check buffer_idx values
        let memory_stream = retriever.memory_stream.read().await;
        assert_eq!(
            memory_stream[0]
                .metadata
                .get("buffer_idx")
                .unwrap()
                .as_u64()
                .unwrap(),
            0
        );
        assert_eq!(
            memory_stream[1]
                .metadata
                .get("buffer_idx")
                .unwrap()
                .as_u64()
                .unwrap(),
            1
        );
        assert_eq!(
            memory_stream[2]
                .metadata
                .get("buffer_idx")
                .unwrap()
                .as_u64()
                .unwrap(),
            2
        );
        assert_eq!(
            memory_stream[3]
                .metadata
                .get("buffer_idx")
                .unwrap()
                .as_u64()
                .unwrap(),
            3
        );
    }

    #[tokio::test]
    async fn test_created_at_preserved() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        // Document with pre-set created_at
        let mut doc = Document::new("Pre-timestamped doc");
        let custom_time = Utc::now() - chrono::Duration::days(1);
        doc.metadata.insert(
            "created_at".to_string(),
            serde_json::json!(custom_time.to_rfc3339()),
        );

        retriever.add_documents(vec![doc]).await.unwrap();

        let memory_stream = retriever.memory_stream.read().await;
        let stored_created_at = memory_stream[0]
            .metadata
            .get("created_at")
            .unwrap()
            .as_str()
            .unwrap();

        // Should preserve the custom created_at
        assert_eq!(stored_created_at, custom_time.to_rfc3339());
    }

    #[tokio::test]
    async fn test_runnable_invoke() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        let docs = vec![Document::new("Runnable test")];
        retriever.add_documents(docs).await.unwrap();

        // Test invoke through Runnable trait
        let results = retriever.invoke("test".to_string(), None).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_runnable_batch() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        let docs = vec![Document::new("Batch test")];
        retriever.add_documents(docs).await.unwrap();

        // Test batch through Runnable trait
        let queries = vec!["query1".to_string(), "query2".to_string()];
        let results = retriever.batch(queries, None).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 1);
        assert_eq!(results[1].len(), 1);
    }

    #[tokio::test]
    async fn test_runnable_stream() {
        use futures::StreamExt;

        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        let docs = vec![Document::new("Stream test")];
        retriever.add_documents(docs).await.unwrap();

        // Test stream through Runnable trait
        let mut stream = retriever.stream("test".to_string(), None).await.unwrap();

        let result = stream.next().await.unwrap().unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_retriever_name() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        assert_eq!(
            Retriever::name(&retriever),
            "TimeWeightedVectorStoreRetriever"
        );
    }

    #[tokio::test]
    async fn test_runnable_name() {
        use crate::core::runnable::Runnable;

        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever: TimeWeightedVectorStoreRetriever<_> =
            TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        assert_eq!(
            Runnable::name(&retriever),
            "TimeWeightedVectorStoreRetriever"
        );
    }

    #[tokio::test]
    async fn test_very_old_documents() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        let current_time = Utc::now();
        let one_year_ago = current_time - chrono::Duration::days(365);

        let mut doc = Document::new("Very old document");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(one_year_ago.to_rfc3339()),
        );

        // Score should be very low for 1-year-old doc
        let score = retriever.get_combined_score(&doc, None, current_time);
        let hours_in_year = 365.0 * 24.0;
        let expected = (1.0 - 0.01_f64).powf(hours_in_year);
        assert!((score - expected).abs() < 0.001);
        assert!(score < 0.001); // Should be nearly zero
    }

    #[tokio::test]
    async fn test_large_batch_documents() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 10);

        // Add 100 documents
        let docs: Vec<Document> = (0..100)
            .map(|i| Document::new(format!("Document {}", i)))
            .collect();
        retriever.add_documents(docs).await.unwrap();

        let memory_stream = retriever.memory_stream.read().await;
        assert_eq!(memory_stream.len(), 100);
    }

    #[tokio::test]
    async fn test_retrieval_updates_only_returned_docs() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 1);

        let docs = vec![Document::new("Doc 1"), Document::new("Doc 2")];
        retriever.add_documents(docs).await.unwrap();

        let original_timestamps: Vec<_> = {
            let memory_stream = retriever.memory_stream.read().await;
            memory_stream
                .iter()
                .map(|doc| doc.metadata.get("last_accessed_at").cloned().unwrap())
                .collect()
        };

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Retrieve with k=1, only one doc should be updated
        retriever
            ._get_relevant_documents("query", None)
            .await
            .unwrap();

        let new_timestamps: Vec<_> = {
            let memory_stream = retriever.memory_stream.read().await;
            memory_stream
                .iter()
                .map(|doc| doc.metadata.get("last_accessed_at").cloned().unwrap())
                .collect()
        };

        // At least one timestamp should be different (the returned doc)
        assert!(
            original_timestamps[0] != new_timestamps[0]
                || original_timestamps[1] != new_timestamps[1]
        );
    }

    #[tokio::test]
    async fn test_negative_hours_passed() {
        // Edge case: future timestamp (shouldn't happen but test handling)
        let now = Utc::now();
        let future = now + chrono::Duration::hours(1);

        let hours = get_hours_passed(now, future);
        assert!(hours < 0.0);
        assert!((hours + 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_score_components_additive() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2)
            .with_other_score_keys(vec!["importance".to_string()]);

        let current_time = Utc::now();
        let one_hour_ago = current_time - chrono::Duration::hours(1);

        let mut doc = Document::new("Scored document");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(one_hour_ago.to_rfc3339()),
        );
        doc.metadata
            .insert("importance".to_string(), serde_json::json!(5.0));

        let vector_relevance = 2.0;
        let score = retriever.get_combined_score(&doc, Some(vector_relevance), current_time);

        // Score = time_decay + importance + vector_relevance
        let time_decay = (1.0 - 0.01_f64).powf(1.0);
        let expected = time_decay + 5.0 + 2.0;
        assert!((score - expected).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_concurrent_add_and_retrieve() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = Arc::new(tokio::sync::RwLock::new(
            TimeWeightedVectorStoreRetriever::new(store, 0.01, 2),
        ));

        let retriever_clone = retriever.clone();
        let add_handle = tokio::spawn(async move {
            let mut r = retriever_clone.write().await;
            r.add_documents(vec![Document::new("Concurrent doc")])
                .await
                .unwrap();
        });

        // Wait for add to potentially start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let retriever_clone2 = retriever.clone();
        let retrieve_handle = tokio::spawn(async move {
            let r = retriever_clone2.read().await;
            r._get_relevant_documents("query", None).await.unwrap()
        });

        add_handle.await.unwrap();
        let results = retrieve_handle.await.unwrap();

        // Should complete without deadlock
        assert!(results.len() <= 1);
    }

    #[tokio::test]
    async fn test_document_with_all_metadata_fields() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let mut retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 1)
            .with_other_score_keys(vec!["importance".to_string(), "priority".to_string()]);

        let custom_time = Utc::now() - chrono::Duration::hours(2);
        let mut doc = Document::new("Fully specified document");
        doc.metadata.insert(
            "created_at".to_string(),
            serde_json::json!(custom_time.to_rfc3339()),
        );
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(custom_time.to_rfc3339()),
        );
        doc.metadata
            .insert("importance".to_string(), serde_json::json!(3.0));
        doc.metadata
            .insert("priority".to_string(), serde_json::json!(2.5));

        retriever.add_documents(vec![doc]).await.unwrap();

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Verify all metadata is preserved
        let result = &results[0];
        assert!(result.metadata.contains_key("created_at"));
        assert!(result.metadata.contains_key("last_accessed_at"));
        assert!(result.metadata.contains_key("importance"));
        assert!(result.metadata.contains_key("priority"));
        assert!(result.metadata.contains_key("buffer_idx"));
    }

    #[test]
    fn test_get_hours_passed_zero() {
        let now = Utc::now();
        let hours = get_hours_passed(now, now);
        assert!((hours - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_get_hours_passed_fractional() {
        let now = Utc::now();
        let half_hour_ago = now - chrono::Duration::minutes(30);
        let hours = get_hours_passed(now, half_hour_ago);
        assert!((hours - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_decay_rate_boundary_one() {
        // Edge case: decay_rate = 1.0 means instant decay
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 1.0, 2);

        let current_time = Utc::now();
        let one_hour_ago = current_time - chrono::Duration::hours(1);

        let mut doc = Document::new("Instantly decaying");
        doc.metadata.insert(
            "last_accessed_at".to_string(),
            serde_json::json!(one_hour_ago.to_rfc3339()),
        );

        // (1 - 1.0)^1 = 0^1 = 0
        let score = retriever.get_combined_score(&doc, None, current_time);
        assert!((score - 0.0).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_empty_batch_invoke() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);
        let retriever = TimeWeightedVectorStoreRetriever::new(store, 0.01, 2);

        // Empty batch should return empty results
        let results = retriever.batch(vec![], None).await.unwrap();
        assert_eq!(results.len(), 0);
    }
}
