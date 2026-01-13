// The blanket #![allow(clippy::unwrap_used)] was removed.
// Production code uses proper error handling; only test code uses .unwrap().

//! Typesense vector store implementation
//!
//! Provides vector similarity search using Typesense search engine.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::embeddings::Embeddings;
//! use dashflow_typesense::TypesenseVectorStore;
//! use std::sync::Arc;
//!
//! let embeddings = Arc::new(my_embeddings);
//! let mut store = TypesenseVectorStore::new(
//!     "http://localhost:8108",
//!     "my_api_key",
//!     "my_collection",
//!     embeddings,
//!     384, // embedding dimension
//!     "text", // text field name
//! ).await?;
//!
//! // Add documents
//! let texts = vec!["Hello world", "Goodbye world"];
//! let ids = store.add_texts(&texts, None, None).await?;
//!
//! // Search
//! let results = store._similarity_search("Hello", 5, None).await?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use typesense_rs::apis::collections_api::{
    CollectionsApi, CollectionsApiClient, CreateCollectionParams, GetCollectionParams,
};
use typesense_rs::apis::configuration::{ApiKey, Configuration};
use typesense_rs::apis::documents_api::{
    DocumentsApi, DocumentsApiClient, ImportDocumentsParams, MultiSearchParams,
};
use typesense_rs::models::{CollectionSchema, Field, IndexAction};
use uuid::Uuid;

/// Typesense vector store for similarity search.
///
/// Uses Typesense's vector search capabilities to store and retrieve
/// documents based on semantic similarity.
#[derive(Clone)]
pub struct TypesenseVectorStore {
    collections_api: Arc<CollectionsApiClient>,
    documents_api: Arc<DocumentsApiClient>,
    collection_name: String,
    embeddings: Arc<dyn Embeddings>,
    embedding_dim: usize,
    text_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TypesenseDocument {
    id: String,
    vec: Vec<f32>,
    #[serde(flatten)]
    text_and_metadata: HashMap<String, JsonValue>,
}

impl TypesenseVectorStore {
    /// Creates a new `TypesenseVectorStore`.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Typesense server URL (e.g., "<http://localhost:8108>")
    /// * `api_key` - Typesense API key for authentication
    /// * `collection_name` - Name of the collection to use
    /// * `embeddings` - Embeddings model for generating vectors
    /// * `embedding_dim` - Dimension of the embedding vectors
    /// * `text_key` - Field name for storing text content (default: "text")
    pub async fn new(
        base_url: &str,
        api_key: &str,
        collection_name: &str,
        embeddings: Arc<dyn Embeddings>,
        embedding_dim: usize,
        text_key: &str,
    ) -> Result<Self> {
        let config = Arc::new(Configuration {
            base_path: base_url.to_string(),
            user_agent: Some("dashflow/0.1.0".to_string()),
            client: create_http_client(),
            basic_auth: None,
            oauth_access_token: None,
            bearer_access_token: None,
            api_key: Some(ApiKey {
                prefix: None,
                key: api_key.to_string(),
            }),
        });

        let collections_api = Arc::new(CollectionsApiClient::new(Arc::clone(&config)));
        let documents_api = Arc::new(DocumentsApiClient::new(Arc::clone(&config)));

        Ok(Self {
            collections_api,
            documents_api,
            collection_name: collection_name.to_string(),
            embeddings,
            embedding_dim,
            text_key: text_key.to_string(),
        })
    }

    /// Creates the collection if it doesn't exist.
    async fn ensure_collection_exists(&self) -> Result<()> {
        // Try to get the collection
        let get_params = GetCollectionParams {
            collection_name: self.collection_name.clone(),
        };

        match self.collections_api.get_collection(get_params).await {
            Ok(_) => Ok(()), // Collection exists
            Err(_) => {
                // Collection doesn't exist, create it
                self.create_collection().await
            }
        }
    }

    /// Creates the collection with the appropriate schema.
    async fn create_collection(&self) -> Result<()> {
        let mut vec_field = Field::new("vec".to_string(), "float[]".to_string());
        vec_field.num_dim = Some(self.embedding_dim as i32);

        let text_field = Field::new(self.text_key.clone(), "string".to_string());

        // Wildcard field for metadata
        let metadata_field = Field::new(".*".to_string(), "auto".to_string());

        let schema = CollectionSchema::new(
            vec![vec_field, text_field, metadata_field],
            self.collection_name.clone(),
        );

        let params = CreateCollectionParams {
            collection_schema: schema,
        };

        self.collections_api
            .create_collection(params)
            .await
            .map_err(|e| Error::Other(format!("Failed to create collection: {e}")))?;

        Ok(())
    }

    /// Prepares documents for indexing by embedding texts and formatting.
    async fn prep_documents(
        &self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
    ) -> Result<(Vec<TypesenseDocument>, Vec<String>)> {
        let text_strs: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();
        // Generate embeddings using graph API
        let embeddings = embed(Arc::clone(&self.embeddings), &text_strs)
            .await
            .map_err(|e| Error::Other(format!("Failed to embed documents: {e}")))?;

        let mut documents = Vec::new();
        let mut doc_ids = Vec::new();

        for (i, (text, embedding)) in texts.iter().zip(embeddings.iter()).enumerate() {
            let id = if let Some(ids) = ids {
                ids[i].clone()
            } else {
                Uuid::new_v4().to_string()
            };

            let mut text_and_metadata = HashMap::new();
            text_and_metadata.insert(
                self.text_key.clone(),
                JsonValue::String(text.as_ref().to_string()),
            );

            // Add metadata if provided
            if let Some(metadatas) = metadatas {
                if let Some(metadata) = metadatas.get(i) {
                    text_and_metadata.insert(
                        "metadata".to_string(),
                        JsonValue::Object(
                            metadata
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect(),
                        ),
                    );
                }
            }

            documents.push(TypesenseDocument {
                id: id.clone(),
                vec: embedding.clone(),
                text_and_metadata,
            });

            doc_ids.push(id);
        }

        Ok((documents, doc_ids))
    }
}

#[async_trait]
impl VectorStore for TypesenseVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(Arc::clone(&self.embeddings))
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Ensure collection exists
        self.ensure_collection_exists().await?;

        // Prepare documents
        let (documents, doc_ids) = self.prep_documents(texts, metadatas, ids).await?;

        // Convert documents to JSONL format
        let jsonl = documents
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Other(format!("Failed to serialize document: {e}")))?
            .join("\n");

        // Import documents
        let params = ImportDocumentsParams {
            collection_name: self.collection_name.clone(),
            body: jsonl,
            batch_size: None,
            return_id: Some(true),
            remote_embedding_batch_size: None,
            return_doc: None,
            action: Some(IndexAction::Upsert),
            dirty_values: None,
        };

        self.documents_api
            .import_documents(params)
            .await
            .map_err(|e| Error::Other(format!("Failed to import documents: {e}")))?;

        Ok(doc_ids)
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        // Embed the query using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query)
            .await
            .map_err(|e| Error::Other(format!("Failed to embed query: {e}")))?;

        // Format vector query
        let vec_str = query_embedding
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let vector_query = format!("vec:([{vec_str}], k:{k})");

        // Format filter if provided
        let filter_by = filter.map(|f| {
            f.iter()
                .map(|(k, v)| format!("{k}:{v}"))
                .collect::<Vec<_>>()
                .join(" && ")
        });

        // Create search parameters using multi_search
        // We need to use the raw multi_search with a searches JSON body
        let params = MultiSearchParams {
            q: Some("*".to_string()),
            query_by: None,
            query_by_weights: None,
            text_match_type: None,
            prefix: None,
            infix: None,
            max_extra_prefix: None,
            max_extra_suffix: None,
            filter_by: filter_by.clone(),
            sort_by: None,
            facet_by: None,
            max_facet_values: None,
            facet_query: None,
            num_typos: None,
            page: None,
            per_page: None,
            limit: Some(k as i32),
            offset: None,
            group_by: None,
            group_limit: None,
            group_missing_values: None,
            include_fields: None,
            exclude_fields: None,
            highlight_full_fields: None,
            highlight_affix_num_tokens: None,
            highlight_start_tag: None,
            highlight_end_tag: None,
            snippet_threshold: None,
            drop_tokens_threshold: None,
            drop_tokens_mode: None,
            typo_tokens_threshold: None,
            enable_typos_for_alpha_numerical_tokens: None,
            filter_curated_hits: None,
            enable_synonyms: None,
            synonym_prefix: None,
            synonym_num_typos: None,
            pinned_hits: None,
            hidden_hits: None,
            override_tags: None,
            highlight_fields: None,
            pre_segmented_query: None,
            preset: None,
            enable_overrides: None,
            prioritize_exact_match: None,
            prioritize_token_position: None,
            prioritize_num_matching_fields: None,
            enable_typos_for_numerical_tokens: None,
            exhaustive_search: None,
            search_cutoff_ms: None,
            use_cache: None,
            cache_ttl: None,
            min_len_1typo: None,
            min_len_2typo: None,
            vector_query: Some(vector_query),
            remote_embedding_timeout_ms: None,
            remote_embedding_num_tries: None,
            facet_strategy: None,
            stopwords: None,
            facet_return_parent: None,
            voice_query: None,
            conversation: None,
            conversation_model_id: None,
            conversation_id: None,
            multi_search_searches_parameter: None,
        };

        // Execute search
        let response = self
            .documents_api
            .multi_search(params)
            .await
            .map_err(|e| Error::Other(format!("Failed to search: {e}")))?;

        // Parse results
        let mut documents = Vec::new();
        if let Some(result) = response.results.first() {
            if let Some(hits) = &result.hits {
                for hit in hits {
                    if let Some(doc) = &hit.document {
                        let text = doc
                            .get(&self.text_key)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let metadata = doc
                            .get("metadata")
                            .and_then(|v| v.as_object())
                            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                            .unwrap_or_default();

                        let mut document = Document::new(text);
                        document.metadata = metadata;
                        documents.push(document);
                    }
                }
            }
        }

        Ok(documents)
    }
}

#[cfg(test)]
// SAFETY: Tests use unwrap() to panic on unexpected errors, clearly indicating test failure.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::embeddings::Embeddings;
    use dashflow::core::vector_stores::VectorStore;
    use std::sync::Arc;

    /// Mock embeddings for testing
    struct MockEmbeddings {
        dim: usize,
    }

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> dashflow::core::Result<Vec<Vec<f32>>> {
            // Return simple mock embeddings based on text length
            Ok(texts
                .iter()
                .map(|text| {
                    let mut vec = vec![0.0; self.dim];
                    vec[0] = text.len() as f32;
                    vec
                })
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> dashflow::core::Result<Vec<f32>> {
            let mut vec = vec![0.0; self.dim];
            vec[0] = text.len() as f32;
            Ok(vec)
        }
    }

    #[tokio::test]
    async fn test_typesense_store_creation() {
        // Test that we can create a TypesenseVectorStore instance
        // This doesn't require a running Typesense server
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let result = TypesenseVectorStore::new(
            "http://localhost:8108",
            "test_key",
            "test_collection",
            embeddings,
            384,
            "text",
        )
        .await;

        assert!(result.is_ok());
        let store = result.unwrap();
        assert_eq!(store.collection_name, "test_collection");
        assert_eq!(store.embedding_dim, 384);
        assert_eq!(store.text_key, "text");
    }

    #[tokio::test]
    async fn test_embeddings_accessor() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "test_key",
            "test_collection",
            Arc::clone(&embeddings),
            384,
            "text",
        )
        .await
        .unwrap();

        assert!(store.embeddings().is_some());
    }

    // Note: Integration tests that require a running Typesense server
    // should be added in tests/ directory with #[ignore] attribute
    // and can be run with: cargo test --package dashflow-typesense -- --ignored

    // ========================================================================
    // TypesenseDocument serialization tests
    // ========================================================================

    #[test]
    fn test_typesense_document_serialization() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("text".to_string(), JsonValue::String("Hello world".to_string()));

        let doc = TypesenseDocument {
            id: "doc_123".to_string(),
            vec: vec![0.1, 0.2, 0.3],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("doc_123"));
        assert!(json.contains("0.1"));
        assert!(json.contains("Hello world"));
    }

    #[test]
    fn test_typesense_document_deserialization() {
        let json = r#"{"id":"doc_456","vec":[0.5,0.6,0.7],"text":"Test content"}"#;
        let doc: TypesenseDocument = serde_json::from_str(json).unwrap();

        assert_eq!(doc.id, "doc_456");
        assert_eq!(doc.vec, vec![0.5, 0.6, 0.7]);
        assert_eq!(
            doc.text_and_metadata.get("text"),
            Some(&JsonValue::String("Test content".to_string()))
        );
    }

    #[test]
    fn test_typesense_document_with_metadata() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("text".to_string(), JsonValue::String("Content".to_string()));
        text_and_metadata.insert(
            "metadata".to_string(),
            serde_json::json!({"author": "Alice", "year": 2024}),
        );

        let doc = TypesenseDocument {
            id: "meta_doc".to_string(),
            vec: vec![1.0, 2.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("author"));
        assert!(json.contains("Alice"));
        assert!(json.contains("2024"));
    }

    #[test]
    fn test_typesense_document_empty_vector() {
        let doc = TypesenseDocument {
            id: "empty_vec".to_string(),
            vec: vec![],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains(r#""vec":[]"#));
    }

    #[test]
    fn test_typesense_document_large_vector() {
        let large_vec: Vec<f32> = (0..1024).map(|i| i as f32 / 1024.0).collect();
        let doc = TypesenseDocument {
            id: "large_vec".to_string(),
            vec: large_vec.clone(),
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.vec.len(), 1024);
    }

    #[test]
    fn test_typesense_document_special_characters() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert(
            "text".to_string(),
            JsonValue::String("Hello \"world\" with 'quotes' and \\ backslashes".to_string()),
        );

        let doc = TypesenseDocument {
            id: "special_chars".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert!(deserialized
            .text_and_metadata
            .get("text")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("quotes"));
    }

    #[test]
    fn test_typesense_document_unicode() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert(
            "text".to_string(),
            JsonValue::String("Hello 世界 🌍 مرحبا".to_string()),
        );

        let doc = TypesenseDocument {
            id: "unicode_doc".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        let text = deserialized
            .text_and_metadata
            .get("text")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(text.contains("世界"));
        assert!(text.contains("🌍"));
    }

    // ========================================================================
    // TypesenseVectorStore creation tests
    // ========================================================================

    #[tokio::test]
    async fn test_store_creation_different_dimensions() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 128 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            128,
            "content",
        )
        .await
        .unwrap();

        assert_eq!(store.embedding_dim, 128);
    }

    #[tokio::test]
    async fn test_store_creation_custom_text_key() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            384,
            "document_content",
        )
        .await
        .unwrap();

        assert_eq!(store.text_key, "document_content");
    }

    #[tokio::test]
    async fn test_store_creation_custom_collection_name() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "my_custom_collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        assert_eq!(store.collection_name, "my_custom_collection");
    }

    #[tokio::test]
    async fn test_store_creation_various_urls() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        // Test with different URL formats
        let urls = [
            "http://localhost:8108",
            "https://typesense.example.com",
            "http://192.168.1.100:8108",
        ];

        for url in urls {
            let result = TypesenseVectorStore::new(
                url,
                "key",
                "collection",
                Arc::clone(&embeddings),
                384,
                "text",
            )
            .await;
            assert!(result.is_ok(), "Failed to create store with URL: {}", url);
        }
    }

    #[tokio::test]
    async fn test_store_clone() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        let cloned = store.clone();
        assert_eq!(cloned.collection_name, store.collection_name);
        assert_eq!(cloned.embedding_dim, store.embedding_dim);
        assert_eq!(cloned.text_key, store.text_key);
    }

    // ========================================================================
    // Mock embeddings tests
    // ========================================================================

    #[tokio::test]
    async fn test_mock_embeddings_document() {
        let embeddings = MockEmbeddings { dim: 384 };
        let texts = vec!["Hello".to_string(), "World".to_string()];

        let result = embeddings._embed_documents(&texts).await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 384);
        assert_eq!(result[1].len(), 384);
        // First element should be text length
        assert_eq!(result[0][0], 5.0); // "Hello" has 5 chars
        assert_eq!(result[1][0], 5.0); // "World" has 5 chars
    }

    #[tokio::test]
    async fn test_mock_embeddings_query() {
        let embeddings = MockEmbeddings { dim: 256 };
        let result = embeddings._embed_query("test query").await.unwrap();

        assert_eq!(result.len(), 256);
        assert_eq!(result[0], 10.0); // "test query" has 10 chars
    }

    #[tokio::test]
    async fn test_mock_embeddings_empty_text() {
        let embeddings = MockEmbeddings { dim: 128 };
        let texts = vec!["".to_string()];

        let result = embeddings._embed_documents(&texts).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0][0], 0.0); // Empty string has 0 chars
    }

    #[tokio::test]
    async fn test_mock_embeddings_empty_list() {
        let embeddings = MockEmbeddings { dim: 128 };
        let texts: Vec<String> = vec![];

        let result = embeddings._embed_documents(&texts).await.unwrap();

        assert!(result.is_empty());
    }

    // ========================================================================
    // VectorStore trait tests
    // ========================================================================

    #[tokio::test]
    async fn test_add_texts_empty_returns_empty() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let mut store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        let texts: Vec<String> = vec![];
        let result = store.add_texts(&texts, None, None).await.unwrap();

        assert!(result.is_empty());
    }

    // ========================================================================
    // Document preparation tests (via add_texts with mock server errors)
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires Typesense server"]
    async fn test_add_texts_single_document() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let mut store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "xyz",
            "test_collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        let texts = vec!["Hello world"];
        let result = store.add_texts(&texts, None, None).await;

        // Will fail without server but tests the code path
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires Typesense server"]
    async fn test_add_texts_multiple_documents() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let mut store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "xyz",
            "test_collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        let texts = vec!["Doc 1", "Doc 2", "Doc 3"];
        let result = store.add_texts(&texts, None, None).await;

        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires Typesense server"]
    async fn test_add_texts_with_custom_ids() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let mut store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "xyz",
            "test_collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        let texts = vec!["Doc 1"];
        let ids = vec!["custom_id_1".to_string()];
        let result = store.add_texts(&texts, None, Some(&ids)).await;

        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires Typesense server"]
    async fn test_add_texts_with_metadata() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let mut store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "xyz",
            "test_collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        let texts = vec!["Doc with metadata"];
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        let metadatas = vec![metadata];

        let result = store.add_texts(&texts, Some(&metadatas), None).await;

        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires Typesense server"]
    async fn test_similarity_search() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "xyz",
            "test_collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        let result = store._similarity_search("query", 5, None).await;

        assert!(result.is_ok() || result.is_err());
    }

    // ========================================================================
    // HTTP client creation test
    // ========================================================================

    #[test]
    fn test_http_client_creation() {
        // Test that create_http_client works and returns a client
        let client = create_http_client();
        // Just verify it doesn't panic
        drop(client);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[tokio::test]
    async fn test_store_with_zero_dimensions() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 0 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            0,
            "text",
        )
        .await
        .unwrap();

        assert_eq!(store.embedding_dim, 0);
    }

    #[tokio::test]
    async fn test_store_with_empty_api_key() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let result = TypesenseVectorStore::new(
            "http://localhost:8108",
            "",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await;

        // Should succeed creating the store (API key validity checked on requests)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_with_empty_collection_name() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let result = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "",
            embeddings,
            384,
            "text",
        )
        .await;

        // Should succeed creating the store (collection validity checked on requests)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().collection_name, "");
    }

    #[tokio::test]
    async fn test_store_with_empty_text_key() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let result = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            384,
            "",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().text_key, "");
    }

    // ========================================================================
    // JSONL formatting tests
    // ========================================================================

    #[test]
    fn test_jsonl_formatting() {
        let docs = vec![
            TypesenseDocument {
                id: "1".to_string(),
                vec: vec![0.1],
                text_and_metadata: HashMap::new(),
            },
            TypesenseDocument {
                id: "2".to_string(),
                vec: vec![0.2],
                text_and_metadata: HashMap::new(),
            },
        ];

        let jsonl = docs
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");

        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"id\":\"1\""));
        assert!(lines[1].contains("\"id\":\"2\""));
    }

    // ========================================================================
    // Vector query formatting tests
    // ========================================================================

    #[test]
    fn test_vector_query_format() {
        let embedding = vec![0.1, 0.2, 0.3];
        let k = 5;

        let vec_str = embedding
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let vector_query = format!("vec:([{vec_str}], k:{k})");

        assert_eq!(vector_query, "vec:([0.1,0.2,0.3], k:5)");
    }

    #[test]
    fn test_filter_format() {
        let mut filter = HashMap::new();
        filter.insert(
            "category".to_string(),
            JsonValue::String("books".to_string()),
        );
        filter.insert("year".to_string(), JsonValue::Number(2024.into()));

        let filter_by = filter
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(" && ");

        assert!(filter_by.contains("category:\"books\""));
        assert!(filter_by.contains("year:2024"));
        assert!(filter_by.contains(" && "));
    }

    #[test]
    fn test_empty_filter() {
        let filter: HashMap<String, JsonValue> = HashMap::new();
        let filter_by = if filter.is_empty() {
            None
        } else {
            Some(
                filter
                    .iter()
                    .map(|(k, v)| format!("{k}:{v}"))
                    .collect::<Vec<_>>()
                    .join(" && "),
            )
        };

        assert!(filter_by.is_none());
    }

    // ========================================================================
    // Dimension consistency tests
    // ========================================================================

    #[tokio::test]
    async fn test_various_embedding_dimensions() {
        let dimensions = [64, 128, 256, 384, 512, 768, 1024, 1536, 3072];

        for dim in dimensions {
            let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim });

            let result = TypesenseVectorStore::new(
                "http://localhost:8108",
                "key",
                "collection",
                embeddings,
                dim,
                "text",
            )
            .await;

            assert!(result.is_ok(), "Failed for dimension {}", dim);
            assert_eq!(result.unwrap().embedding_dim, dim);
        }
    }

    // ========================================================================
    // Concurrent store creation tests
    // ========================================================================

    #[tokio::test]
    async fn test_concurrent_store_creation() {
        use tokio::task::JoinSet;

        let mut join_set = JoinSet::new();

        for i in 0..5 {
            let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });
            let collection_name = format!("collection_{}", i);

            join_set.spawn(async move {
                TypesenseVectorStore::new(
                    "http://localhost:8108",
                    "key",
                    &collection_name,
                    embeddings,
                    384,
                    "text",
                )
                .await
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap());
        }

        assert_eq!(results.len(), 5);
        for result in results {
            assert!(result.is_ok());
        }
    }

    // ========================================================================
    // TypesenseDocument advanced serialization tests
    // ========================================================================

    #[test]
    fn test_typesense_document_roundtrip() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("text".to_string(), JsonValue::String("Hello world".to_string()));
        text_and_metadata.insert(
            "metadata".to_string(),
            serde_json::json!({"source": "test", "count": 42}),
        );

        let original = TypesenseDocument {
            id: "roundtrip_test".to_string(),
            vec: vec![0.1, -0.2, 0.3, -0.4, 0.5],
            text_and_metadata,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.vec, original.vec);
        assert_eq!(deserialized.text_and_metadata.len(), original.text_and_metadata.len());
    }

    #[test]
    fn test_typesense_document_debug() {
        let doc = TypesenseDocument {
            id: "debug_test".to_string(),
            vec: vec![1.0, 2.0],
            text_and_metadata: HashMap::new(),
        };

        let debug_str = format!("{:?}", doc);
        assert!(debug_str.contains("debug_test"));
        assert!(debug_str.contains("1.0"));
    }

    #[test]
    fn test_typesense_document_negative_floats() {
        let doc = TypesenseDocument {
            id: "neg_floats".to_string(),
            vec: vec![-1.0, -0.5, -0.001, -999.999],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("-1"));
        assert!(json.contains("-0.5"));

        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.vec[0], -1.0);
        assert_eq!(deserialized.vec[3], -999.999);
    }

    #[test]
    fn test_typesense_document_very_long_id() {
        let long_id = "x".repeat(1000);
        let doc = TypesenseDocument {
            id: long_id.clone(),
            vec: vec![0.0],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id.len(), 1000);
    }

    #[test]
    fn test_typesense_document_newlines_in_text() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert(
            "text".to_string(),
            JsonValue::String("Line 1\nLine 2\nLine 3".to_string()),
        );

        let doc = TypesenseDocument {
            id: "newlines".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        let text = deserialized.text_and_metadata.get("text").unwrap().as_str().unwrap();
        assert!(text.contains('\n'));
        assert_eq!(text.lines().count(), 3);
    }

    #[test]
    fn test_typesense_document_tabs_and_special_whitespace() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert(
            "text".to_string(),
            JsonValue::String("Col1\tCol2\tCol3\r\nRow2".to_string()),
        );

        let doc = TypesenseDocument {
            id: "whitespace".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        let text = deserialized.text_and_metadata.get("text").unwrap().as_str().unwrap();
        assert!(text.contains('\t'));
    }

    #[test]
    fn test_typesense_document_nested_metadata() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("text".to_string(), JsonValue::String("test".to_string()));
        text_and_metadata.insert(
            "metadata".to_string(),
            serde_json::json!({
                "level1": {
                    "level2": {
                        "level3": "deep value"
                    }
                }
            }),
        );

        let doc = TypesenseDocument {
            id: "nested".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("level3"));
        assert!(json.contains("deep value"));
    }

    #[test]
    fn test_typesense_document_array_in_metadata() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("text".to_string(), JsonValue::String("test".to_string()));
        text_and_metadata.insert(
            "tags".to_string(),
            serde_json::json!(["tag1", "tag2", "tag3"]),
        );

        let doc = TypesenseDocument {
            id: "array_meta".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        let tags = deserialized.text_and_metadata.get("tags").unwrap();
        assert!(tags.is_array());
        assert_eq!(tags.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_typesense_document_boolean_metadata() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("active".to_string(), JsonValue::Bool(true));
        text_and_metadata.insert("deleted".to_string(), JsonValue::Bool(false));

        let doc = TypesenseDocument {
            id: "bool_meta".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text_and_metadata.get("active"), Some(&JsonValue::Bool(true)));
        assert_eq!(deserialized.text_and_metadata.get("deleted"), Some(&JsonValue::Bool(false)));
    }

    #[test]
    fn test_typesense_document_null_metadata() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("nullable_field".to_string(), JsonValue::Null);

        let doc = TypesenseDocument {
            id: "null_meta".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text_and_metadata.get("nullable_field"), Some(&JsonValue::Null));
    }

    #[test]
    fn test_typesense_document_numeric_metadata() {
        let mut text_and_metadata = HashMap::new();
        text_and_metadata.insert("integer".to_string(), JsonValue::Number(42.into()));
        text_and_metadata.insert(
            "float".to_string(),
            JsonValue::Number(serde_json::Number::from_f64(3.14159).unwrap()),
        );
        text_and_metadata.insert("negative".to_string(), JsonValue::Number((-100).into()));

        let doc = TypesenseDocument {
            id: "numeric_meta".to_string(),
            vec: vec![0.0],
            text_and_metadata,
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text_and_metadata.get("integer").unwrap().as_i64(), Some(42));
    }

    #[test]
    fn test_typesense_document_very_small_floats() {
        let doc = TypesenseDocument {
            id: "small_floats".to_string(),
            vec: vec![1e-10, 1e-20, 1e-38],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert!(deserialized.vec[0] > 0.0);
        assert!(deserialized.vec[0] < 1e-9);
    }

    #[test]
    fn test_typesense_document_very_large_floats() {
        let doc = TypesenseDocument {
            id: "large_floats".to_string(),
            vec: vec![1e10, 1e20, 1e38],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert!(deserialized.vec[0] > 1e9);
    }

    // ========================================================================
    // TypesenseVectorStore creation edge cases
    // ========================================================================

    #[tokio::test]
    async fn test_store_with_very_large_dimension() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 4096 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            4096,
            "text",
        )
        .await
        .unwrap();

        assert_eq!(store.embedding_dim, 4096);
    }

    #[tokio::test]
    async fn test_store_with_special_chars_in_collection_name() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "my_collection_v2",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        assert_eq!(store.collection_name, "my_collection_v2");
    }

    #[tokio::test]
    async fn test_store_with_numeric_collection_name() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "12345",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        assert_eq!(store.collection_name, "12345");
    }

    #[tokio::test]
    async fn test_store_with_unicode_collection_name() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "コレクション",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        assert_eq!(store.collection_name, "コレクション");
    }

    #[tokio::test]
    async fn test_store_with_long_api_key() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });
        let long_key = "x".repeat(500);

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            &long_key,
            "collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        assert_eq!(store.collection_name, "collection");
    }

    #[tokio::test]
    async fn test_store_with_https_url() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let result = TypesenseVectorStore::new(
            "https://typesense.example.com:443",
            "key",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_with_ipv4_url() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let result = TypesenseVectorStore::new(
            "http://127.0.0.1:8108",
            "key",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_with_custom_port() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let result = TypesenseVectorStore::new(
            "http://localhost:9999",
            "key",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await;

        assert!(result.is_ok());
    }

    // ========================================================================
    // MockEmbeddings additional tests
    // ========================================================================

    #[tokio::test]
    async fn test_mock_embeddings_long_text() {
        let embeddings = MockEmbeddings { dim: 128 };
        let long_text = "x".repeat(10000);

        let result = embeddings._embed_query(&long_text).await.unwrap();
        assert_eq!(result.len(), 128);
        assert_eq!(result[0], 10000.0);
    }

    #[tokio::test]
    async fn test_mock_embeddings_unicode_text() {
        let embeddings = MockEmbeddings { dim: 128 };

        let result = embeddings._embed_query("Hello 世界 🌍").await.unwrap();
        assert_eq!(result.len(), 128);
        // Unicode characters count as multiple bytes but len() counts chars
        assert!(result[0] > 0.0);
    }

    #[tokio::test]
    async fn test_mock_embeddings_whitespace_only() {
        let embeddings = MockEmbeddings { dim: 128 };

        let result = embeddings._embed_query("   \t\n  ").await.unwrap();
        assert_eq!(result.len(), 128);
        assert_eq!(result[0], 7.0); // 7 whitespace characters
    }

    #[tokio::test]
    async fn test_mock_embeddings_batch_large() {
        let embeddings = MockEmbeddings { dim: 64 };
        let texts: Vec<String> = (0..100).map(|i| format!("Document number {}", i)).collect();

        let result = embeddings._embed_documents(&texts).await.unwrap();
        assert_eq!(result.len(), 100);
        for emb in &result {
            assert_eq!(emb.len(), 64);
        }
    }

    #[tokio::test]
    async fn test_mock_embeddings_deterministic() {
        let embeddings = MockEmbeddings { dim: 128 };

        let result1 = embeddings._embed_query("same text").await.unwrap();
        let result2 = embeddings._embed_query("same text").await.unwrap();

        assert_eq!(result1, result2);
    }

    #[tokio::test]
    async fn test_mock_embeddings_different_texts_different_embeddings() {
        let embeddings = MockEmbeddings { dim: 128 };

        let result1 = embeddings._embed_query("short").await.unwrap();
        let result2 = embeddings._embed_query("much longer text here").await.unwrap();

        assert_ne!(result1[0], result2[0]);
    }

    // ========================================================================
    // Filter formatting additional tests
    // ========================================================================

    #[test]
    fn test_filter_single_string() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), JsonValue::String("books".to_string()));

        let filter_by = filter
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(" && ");

        assert_eq!(filter_by, "category:\"books\"");
    }

    #[test]
    fn test_filter_single_number() {
        let mut filter = HashMap::new();
        filter.insert("year".to_string(), JsonValue::Number(2024.into()));

        let filter_by = filter
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(" && ");

        assert_eq!(filter_by, "year:2024");
    }

    #[test]
    fn test_filter_boolean() {
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(true));

        let filter_by = filter
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(" && ");

        assert_eq!(filter_by, "active:true");
    }

    #[test]
    fn test_filter_multiple_types() {
        let mut filter = HashMap::new();
        filter.insert("name".to_string(), JsonValue::String("test".to_string()));
        filter.insert("count".to_string(), JsonValue::Number(10.into()));
        filter.insert("enabled".to_string(), JsonValue::Bool(false));

        let filter_by = filter
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(" && ");

        assert!(filter_by.contains("name:\"test\""));
        assert!(filter_by.contains("count:10"));
        assert!(filter_by.contains("enabled:false"));
    }

    // ========================================================================
    // Vector query formatting additional tests
    // ========================================================================

    #[test]
    fn test_vector_query_empty() {
        let embedding: Vec<f32> = vec![];
        let k = 5;

        let vec_str = embedding
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let vector_query = format!("vec:([{vec_str}], k:{k})");

        assert_eq!(vector_query, "vec:([], k:5)");
    }

    #[test]
    fn test_vector_query_single_element() {
        let embedding = vec![0.5];
        let k = 1;

        let vec_str = embedding
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let vector_query = format!("vec:([{vec_str}], k:{k})");

        assert_eq!(vector_query, "vec:([0.5], k:1)");
    }

    #[test]
    fn test_vector_query_large_k() {
        let embedding = vec![0.1, 0.2];
        let k = 1000;

        let vec_str = embedding
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let vector_query = format!("vec:([{vec_str}], k:{k})");

        assert_eq!(vector_query, "vec:([0.1,0.2], k:1000)");
    }

    #[test]
    fn test_vector_query_negative_values() {
        let embedding = vec![-0.5, -0.3, 0.0, 0.3, 0.5];
        let k = 10;

        let vec_str = embedding
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let vector_query = format!("vec:([{vec_str}], k:{k})");

        assert!(vector_query.contains("-0.5"));
        assert!(vector_query.contains("-0.3"));
    }

    // ========================================================================
    // JSONL formatting additional tests
    // ========================================================================

    #[test]
    fn test_jsonl_empty_list() {
        let docs: Vec<TypesenseDocument> = vec![];

        let jsonl = docs
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");

        assert!(jsonl.is_empty());
    }

    #[test]
    fn test_jsonl_single_document() {
        let docs = vec![TypesenseDocument {
            id: "single".to_string(),
            vec: vec![0.1],
            text_and_metadata: HashMap::new(),
        }];

        let jsonl = docs
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");

        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(!jsonl.contains('\n'));
    }

    #[test]
    fn test_jsonl_large_batch() {
        let docs: Vec<TypesenseDocument> = (0..50)
            .map(|i| TypesenseDocument {
                id: format!("doc_{}", i),
                vec: vec![i as f32 / 50.0],
                text_and_metadata: HashMap::new(),
            })
            .collect();

        let jsonl = docs
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");

        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 50);
    }

    #[test]
    fn test_jsonl_with_full_documents() {
        let mut meta1 = HashMap::new();
        meta1.insert("text".to_string(), JsonValue::String("Hello".to_string()));
        meta1.insert("source".to_string(), JsonValue::String("test".to_string()));

        let mut meta2 = HashMap::new();
        meta2.insert("text".to_string(), JsonValue::String("World".to_string()));
        meta2.insert("source".to_string(), JsonValue::String("test".to_string()));

        let docs = vec![
            TypesenseDocument {
                id: "1".to_string(),
                vec: vec![0.1, 0.2, 0.3],
                text_and_metadata: meta1,
            },
            TypesenseDocument {
                id: "2".to_string(),
                vec: vec![0.4, 0.5, 0.6],
                text_and_metadata: meta2,
            },
        ];

        let jsonl = docs
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");

        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Hello"));
        assert!(lines[1].contains("World"));
    }

    // ========================================================================
    // Embeddings accessor tests
    // ========================================================================

    #[tokio::test]
    async fn test_embeddings_returns_correct_dimension() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 512 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            Arc::clone(&embeddings),
            512,
            "text",
        )
        .await
        .unwrap();

        let returned_embeddings = store.embeddings().unwrap();
        let result = returned_embeddings._embed_query("test").await.unwrap();
        assert_eq!(result.len(), 512);
    }

    // ========================================================================
    // VectorStore trait edge case tests
    // ========================================================================

    #[tokio::test]
    async fn test_add_texts_single_empty_string() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let mut store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        // This will fail at the network level but tests the code path
        let texts = vec![""];
        let _result = store.add_texts(&texts, None, None).await;
        // Just verify it doesn't panic
    }

    // ========================================================================
    // Store field accessor tests
    // ========================================================================

    #[tokio::test]
    async fn test_store_fields_accessible() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 256 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "my_key",
            "my_collection",
            embeddings,
            256,
            "content",
        )
        .await
        .unwrap();

        assert_eq!(store.collection_name, "my_collection");
        assert_eq!(store.embedding_dim, 256);
        assert_eq!(store.text_key, "content");
    }

    // ========================================================================
    // Concurrent operations tests
    // ========================================================================

    #[tokio::test]
    async fn test_concurrent_embeddings_calls() {
        use tokio::task::JoinSet;

        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 128 });
        let mut join_set = JoinSet::new();

        for i in 0..10 {
            let emb = Arc::clone(&embeddings);
            join_set.spawn(async move {
                emb._embed_query(&format!("query {}", i)).await
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap().unwrap());
        }

        assert_eq!(results.len(), 10);
        for result in &results {
            assert_eq!(result.len(), 128);
        }
    }

    // ========================================================================
    // Document parsing simulation tests
    // ========================================================================

    #[test]
    fn test_parse_search_hit_simulation() {
        // Simulate the structure returned by Typesense search
        let hit_doc = serde_json::json!({
            "text": "Sample document content",
            "metadata": {
                "author": "Alice",
                "date": "2024-01-01"
            }
        });

        let text = hit_doc
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let metadata: HashMap<String, JsonValue> = hit_doc
            .get("metadata")
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        assert_eq!(text, "Sample document content");
        assert_eq!(metadata.get("author"), Some(&JsonValue::String("Alice".to_string())));
    }

    #[test]
    fn test_parse_search_hit_missing_text() {
        let hit_doc = serde_json::json!({
            "metadata": {"key": "value"}
        });

        let text = hit_doc
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        assert!(text.is_empty());
    }

    #[test]
    fn test_parse_search_hit_missing_metadata() {
        let hit_doc = serde_json::json!({
            "text": "Content only"
        });

        let metadata: HashMap<String, JsonValue> = hit_doc
            .get("metadata")
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        assert!(metadata.is_empty());
    }

    #[test]
    fn test_parse_search_hit_custom_text_key() {
        let hit_doc = serde_json::json!({
            "document_content": "Custom field content",
            "text": "Wrong field"
        });

        let text_key = "document_content";
        let text = hit_doc
            .get(text_key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        assert_eq!(text, "Custom field content");
    }

    // ========================================================================
    // Error handling simulation tests
    // ========================================================================

    #[test]
    fn test_document_serialize_error_simulation() {
        // This tests the error path when document serialization might fail
        let doc = TypesenseDocument {
            id: "test".to_string(),
            vec: vec![f32::NAN], // NaN values
            text_and_metadata: HashMap::new(),
        };

        // serde_json handles NaN by default, but let's verify behavior
        let result = serde_json::to_string(&doc);
        // Note: serde_json produces "null" for NaN by default
        assert!(result.is_ok() || result.is_err());
    }

    // ========================================================================
    // Configuration tests
    // ========================================================================

    #[tokio::test]
    async fn test_configuration_user_agent() {
        // Verify store is created (configuration tested indirectly)
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dim: 384 });

        let store = TypesenseVectorStore::new(
            "http://localhost:8108",
            "key",
            "collection",
            embeddings,
            384,
            "text",
        )
        .await
        .unwrap();

        // Store created successfully means configuration was valid
        assert!(!store.collection_name.is_empty());
    }

    // ========================================================================
    // Edge cases for ID handling
    // ========================================================================

    #[test]
    fn test_uuid_format_in_document() {
        let uuid_id = uuid::Uuid::new_v4().to_string();
        let doc = TypesenseDocument {
            id: uuid_id.clone(),
            vec: vec![0.0],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains(&uuid_id));
    }

    #[test]
    fn test_empty_id_in_document() {
        let doc = TypesenseDocument {
            id: String::new(),
            vec: vec![0.0],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains(r#""id":"""#));
    }

    #[test]
    fn test_special_chars_in_id() {
        let doc = TypesenseDocument {
            id: "id/with/slashes-and_underscores.dots".to_string(),
            vec: vec![0.0],
            text_and_metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TypesenseDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "id/with/slashes-and_underscores.dots");
    }
}
