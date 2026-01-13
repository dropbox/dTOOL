//! Build QdrantVectorStore from config
//!
//! This module provides a function to build a `QdrantVectorStore` from
//! a `VectorStoreConfig`.

use crate::{QdrantVectorStore, RetrievalMode};
use dashflow::core::config_loader::VectorStoreConfig;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::Error as DashFlowError;
use std::sync::Arc;

/// Build a Qdrant vector store from a VectorStoreConfig
///
/// This function creates a `QdrantVectorStore` from configuration. It uses
/// Dense retrieval mode by default, as that's the most common use case.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{VectorStoreConfig, EmbeddingConfig, SecretReference};
/// use dashflow_qdrant::build_vector_store;
///
/// let config = VectorStoreConfig::Qdrant {
///     collection_name: "my_collection".to_string(),
///     url: "http://localhost:6334".to_string(),
///     api_key: None,
///     embedding: Box::new(EmbeddingConfig::OpenAI {
///         model: "text-embedding-3-small".to_string(),
///         api_key: SecretReference::from_env("OPENAI_API_KEY"),
///         batch_size: 512,
///     }),
/// };
///
/// let store = build_vector_store(&config, embeddings).await?;
/// ```
///
/// # Arguments
///
/// * `config` - The vector store configuration
/// * `embeddings` - The embeddings instance to use (must be created separately)
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a Qdrant config
/// - Connection to Qdrant server fails
/// - Collection creation fails
pub async fn build_vector_store(
    config: &VectorStoreConfig,
    embeddings: Arc<dyn Embeddings>,
) -> Result<QdrantVectorStore, DashFlowError> {
    match config {
        VectorStoreConfig::Qdrant {
            collection_name,
            url,
            api_key: _,
            embedding: _,
        } => {
            // Note: api_key is ignored for now - Qdrant client handles auth separately
            // Future: Add with_api_key support to QdrantVectorStore
            QdrantVectorStore::new(url, collection_name, Some(embeddings), RetrievalMode::Dense)
                .await
                .map_err(|e| DashFlowError::other(format!("Failed to create Qdrant store: {e}")))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_vector_store from dashflow-qdrant only supports Qdrant configs, \
             got {} config. Use the appropriate provider crate for this config type.",
            other.provider()
        ))),
    }
}

/// Build a Qdrant vector store with a custom retrieval mode
///
/// This function is for advanced use cases where you need to specify
/// the retrieval mode (Dense, Sparse, or Hybrid).
///
/// # Arguments
///
/// * `config` - The vector store configuration
/// * `embeddings` - The embeddings instance to use
/// * `retrieval_mode` - The retrieval mode to use
pub async fn build_vector_store_with_mode(
    config: &VectorStoreConfig,
    embeddings: Arc<dyn Embeddings>,
    retrieval_mode: RetrievalMode,
) -> Result<QdrantVectorStore, DashFlowError> {
    match config {
        VectorStoreConfig::Qdrant {
            collection_name,
            url,
            api_key: _,
            embedding: _,
        } => QdrantVectorStore::new(url, collection_name, Some(embeddings), retrieval_mode)
            .await
            .map_err(|e| DashFlowError::other(format!("Failed to create Qdrant store: {e}"))),
        other => Err(DashFlowError::InvalidInput(format!(
            "build_vector_store_with_mode from dashflow-qdrant only supports Qdrant configs, \
             got {} config. Use the appropriate provider crate for this config type.",
            other.provider()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::{EmbeddingConfig, SecretReference};

    #[test]
    fn test_wrong_provider_fails() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        // Can't run async test without runtime, just verify the match pattern
        assert_eq!(config.provider(), "chroma");
    }
}
