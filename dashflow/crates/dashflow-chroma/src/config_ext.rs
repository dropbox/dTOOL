//! Build ChromaVectorStore from config
//!
//! This module provides a function to build a `ChromaVectorStore` from
//! a `VectorStoreConfig`.

use crate::ChromaVectorStore;
use dashflow::core::config_loader::VectorStoreConfig;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::Error as DashFlowError;
use std::sync::Arc;

/// Build a Chroma vector store from a VectorStoreConfig
///
/// This function creates a `ChromaVectorStore` from configuration, handling
/// embedding creation automatically.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{VectorStoreConfig, EmbeddingConfig, SecretReference};
/// use dashflow_chroma::build_vector_store;
///
/// let config = VectorStoreConfig::Chroma {
///     collection_name: "my_collection".to_string(),
///     url: "http://localhost:8000".to_string(),
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
/// - The config is not a Chroma config
/// - Connection to Chroma server fails
/// - Collection creation fails
pub async fn build_vector_store(
    config: &VectorStoreConfig,
    embeddings: Arc<dyn Embeddings>,
) -> Result<ChromaVectorStore, DashFlowError> {
    match config {
        VectorStoreConfig::Chroma {
            collection_name,
            url,
            embedding: _,
        } => ChromaVectorStore::new(collection_name, embeddings, Some(url))
            .await
            .map_err(|e| DashFlowError::other(format!("Failed to create Chroma store: {e}"))),
        other => Err(DashFlowError::InvalidInput(format!(
            "build_vector_store from dashflow-chroma only supports Chroma configs, \
             got {} config. Use the appropriate provider crate for this config type.",
            other.provider()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::{EmbeddingConfig, SecretReference};

    // ========================================================================
    // PROVIDER VALIDATION
    // ========================================================================

    #[test]
    fn test_qdrant_provider_name() {
        let config = VectorStoreConfig::Qdrant {
            collection_name: "test".to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: None,
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        assert_eq!(config.provider(), "qdrant");
    }

    #[test]
    fn test_qdrant_config_collection_name() {
        let config = VectorStoreConfig::Qdrant {
            collection_name: "my_vectors".to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: None,
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        assert_eq!(config.collection_name(), "my_vectors");
    }

    #[test]
    fn test_qdrant_config_url() {
        let config = VectorStoreConfig::Qdrant {
            collection_name: "test".to_string(),
            url: "http://qdrant.example.com:6333".to_string(),
            api_key: None,
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        assert_eq!(config.url(), "http://qdrant.example.com:6333");
    }

    #[test]
    fn test_qdrant_config_with_api_key() {
        let config = VectorStoreConfig::Qdrant {
            collection_name: "test".to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: Some(SecretReference::from_env("QDRANT_API_KEY")),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        assert_eq!(config.provider(), "qdrant");
    }

    #[test]
    fn test_qdrant_config_without_api_key() {
        let config = VectorStoreConfig::Qdrant {
            collection_name: "test".to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: None,
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        assert_eq!(config.provider(), "qdrant");
    }

    // ========================================================================
    // CHROMA CONFIG STRUCTURE
    // ========================================================================

    #[test]
    fn test_chroma_config_provider_name() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        assert_eq!(config.provider(), "chroma");
    }

    #[test]
    fn test_chroma_config_with_default_url() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "my_collection".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { url, .. } => {
                assert_eq!(url, "http://localhost:8000");
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_with_custom_url() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "my_collection".to_string(),
            url: "http://chroma-server:9000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { url, .. } => {
                assert_eq!(url, "http://chroma-server:9000");
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_collection_name() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "documents_v2".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma {
                collection_name, ..
            } => {
                assert_eq!(collection_name, "documents_v2");
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_empty_collection_name() {
        let config = VectorStoreConfig::Chroma {
            collection_name: String::new(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma {
                collection_name, ..
            } => {
                assert!(collection_name.is_empty());
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_special_chars_collection_name() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "my-collection_v2.3".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma {
                collection_name, ..
            } => {
                assert_eq!(collection_name, "my-collection_v2.3");
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    // ========================================================================
    // EMBEDDING CONFIG VARIANTS
    // ========================================================================

    #[test]
    fn test_chroma_config_with_openai_embedding() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { embedding, .. } => match embedding.as_ref() {
                EmbeddingConfig::OpenAI { model, .. } => {
                    assert_eq!(model, "text-embedding-3-small");
                }
                _ => panic!("Expected OpenAI embedding config"),
            },
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_with_openai_ada_model() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-ada-002".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 256,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { embedding, .. } => match embedding.as_ref() {
                EmbeddingConfig::OpenAI { model, batch_size, .. } => {
                    assert_eq!(model, "text-embedding-ada-002");
                    assert_eq!(*batch_size, 256);
                }
                _ => panic!("Expected OpenAI embedding config"),
            },
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_with_large_embedding_model() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-large".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 1024,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { embedding, .. } => match embedding.as_ref() {
                EmbeddingConfig::OpenAI { model, batch_size, .. } => {
                    assert_eq!(model, "text-embedding-3-large");
                    assert_eq!(*batch_size, 1024);
                }
                _ => panic!("Expected OpenAI embedding config"),
            },
            _ => panic!("Expected Chroma config"),
        }
    }

    // ========================================================================
    // SECRET REFERENCE VARIANTS
    // ========================================================================

    #[test]
    fn test_chroma_config_secret_from_env() {
        let secret = SecretReference::from_env("OPENAI_API_KEY");
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: secret,
                batch_size: 512,
            }),
        };

        assert_eq!(config.provider(), "chroma");
    }

    #[test]
    fn test_chroma_config_secret_from_inline() {
        let secret = SecretReference::from_inline("sk-test-key-12345");
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: secret,
                batch_size: 512,
            }),
        };

        assert_eq!(config.provider(), "chroma");
    }

    #[test]
    fn test_chroma_config_secret_from_inline_empty() {
        let secret = SecretReference::from_inline("");
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: secret,
                batch_size: 512,
            }),
        };

        assert_eq!(config.provider(), "chroma");
    }

    // ========================================================================
    // URL VALIDATION
    // ========================================================================

    #[test]
    fn test_chroma_config_localhost_url() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { url, .. } => {
                assert!(url.starts_with("http://"));
                assert!(url.contains("localhost"));
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_ip_address_url() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://192.168.1.100:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { url, .. } => {
                assert!(url.starts_with("http://"));
                assert!(url.contains("192.168.1.100"));
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_https_url() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "https://chroma.example.com".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { url, .. } => {
                assert!(url.starts_with("https://"));
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    #[test]
    fn test_chroma_config_url_with_path() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000/api/v1".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        match &config {
            VectorStoreConfig::Chroma { url, .. } => {
                assert!(url.contains("/api/v1"));
            }
            _ => panic!("Expected Chroma config"),
        }
    }

    // ========================================================================
    // CONFIG CLONE/DEBUG
    // ========================================================================

    #[test]
    fn test_chroma_config_debug() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Chroma"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_chroma_config_clone() {
        let config = VectorStoreConfig::Chroma {
            collection_name: "test".to_string(),
            url: "http://localhost:8000".to_string(),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        let cloned = config.clone();
        assert_eq!(config.provider(), cloned.provider());
    }

    // ========================================================================
    // ASYNC BUILD_VECTOR_STORE ERROR CASES
    // ========================================================================

    #[tokio::test]
    async fn test_build_vector_store_wrong_provider_error_message() {
        use dashflow_test_utils::MockEmbeddings;
        use std::sync::Arc;

        let embeddings: Arc<dyn dashflow::core::embeddings::Embeddings> =
            Arc::new(MockEmbeddings::with_dimensions(1536));

        let config = VectorStoreConfig::Qdrant {
            collection_name: "test".to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: None,
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        let result = build_vector_store(&config, embeddings).await;
        assert!(result.is_err());

        let err = result.err().unwrap();
        let err_str = err.to_string();
        assert!(
            err_str.contains("only supports Chroma configs"),
            "Error should mention Chroma support: {err_str}"
        );
        assert!(
            err_str.contains("qdrant"),
            "Error should mention qdrant: {err_str}"
        );
    }

    #[tokio::test]
    async fn test_build_vector_store_qdrant_returns_error() {
        use dashflow_test_utils::MockEmbeddings;
        use std::sync::Arc;

        let embeddings: Arc<dyn dashflow::core::embeddings::Embeddings> =
            Arc::new(MockEmbeddings::with_dimensions(1536));

        let config = VectorStoreConfig::Qdrant {
            collection_name: "test".to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: Some(SecretReference::from_inline("test-key")),
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        let result = build_vector_store(&config, embeddings).await;
        // Should return an error because this function only handles Chroma configs
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_build_vector_store_error_contains_provider_hint() {
        use dashflow_test_utils::MockEmbeddings;
        use std::sync::Arc;

        let embeddings: Arc<dyn dashflow::core::embeddings::Embeddings> =
            Arc::new(MockEmbeddings::with_dimensions(1536));

        let config = VectorStoreConfig::Qdrant {
            collection_name: "my_collection".to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: None,
            embedding: Box::new(EmbeddingConfig::OpenAI {
                model: "text-embedding-3-small".to_string(),
                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                batch_size: 512,
            }),
        };

        let result = build_vector_store(&config, embeddings).await;
        let err = result.err().unwrap();
        let err_str = err.to_string();

        // Error message should be helpful and mention using appropriate provider crate
        assert!(
            err_str.contains("appropriate provider crate"),
            "Error should suggest using the appropriate provider: {err_str}"
        );
    }
}
