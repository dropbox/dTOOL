//! Ollama embeddings implementation

use async_trait::async_trait;
use dashflow::core::{
    embeddings::Embeddings,
    error::{Error, Result},
    retry::{with_retry, RetryPolicy},
};
use ollama_rs::{
    generation::embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
    Ollama,
};
use std::sync::Arc;

/// Ollama embeddings model configuration and client
///
/// Provides text embeddings via local Ollama models, enabling embedding
/// generation without external API dependencies.
///
/// # Example
/// ```no_run
/// use dashflow_ollama::OllamaEmbeddings;
/// use dashflow::embed_query;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() {
///     let embedder = Arc::new(OllamaEmbeddings::new().with_model("nomic-embed-text"));
///
///     let query = "What is the meaning of life?";
///     let embedding = embed_query(embedder, query).await.unwrap();
///     println!("Embedding dimension: {}", embedding.len());
/// }
/// ```
#[derive(Clone, Debug)]
pub struct OllamaEmbeddings {
    /// Ollama client
    client: Arc<Ollama>,

    /// Model name (e.g., "nomic-embed-text", "mxbai-embed-large")
    model: String,

    /// Whether to truncate inputs that exceed the model's max context length
    truncate: bool,

    /// Retry policy for API calls
    retry_policy: RetryPolicy,
}

impl OllamaEmbeddings {
    /// Create a new `OllamaEmbeddings` instance with default settings
    ///
    /// Connects to Ollama at <http://localhost:11434> by default.
    /// Uses "nomic-embed-text" model as the default (a good general-purpose embedding model).
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_ollama::OllamaEmbeddings;
    ///
    /// let embedder = OllamaEmbeddings::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url("http://localhost:11434")
    }

    /// Create a new `OllamaEmbeddings` instance with custom base URL
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_ollama::OllamaEmbeddings;
    ///
    /// let embedder = OllamaEmbeddings::with_base_url("http://custom-host:8080")
    ///     .with_model("mxbai-embed-large");
    /// ```
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let ollama = Ollama::new(base_url.into(), 11434);
        Self {
            client: Arc::new(ollama),
            model: "nomic-embed-text".to_string(),
            truncate: true,
            retry_policy: RetryPolicy::exponential(3),
        }
    }

    /// Set the model name
    ///
    /// Common Ollama embedding models:
    /// - `nomic-embed-text`: 768-dim embeddings, good general purpose (137M params)
    /// - `mxbai-embed-large`: 1024-dim embeddings, higher quality (335M params)
    /// - `all-minilm`: 384-dim embeddings, fast and lightweight (22M params)
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_ollama::OllamaEmbeddings;
    ///
    /// let embedder = OllamaEmbeddings::new()
    ///     .with_model("mxbai-embed-large");
    /// ```
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set whether to truncate inputs that exceed the model's context length
    ///
    /// When true (default), inputs longer than the model's max context length
    /// will be truncated. When false, long inputs will cause an error.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_ollama::OllamaEmbeddings;
    ///
    /// let embedder = OllamaEmbeddings::new()
    ///     .with_truncate(false);
    /// ```
    #[must_use]
    pub fn with_truncate(mut self, truncate: bool) -> Self {
        self.truncate = truncate;
        self
    }
}

impl Default for OllamaEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaEmbeddings {
    /// Create Ollama embeddings from configuration
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow::core::config_loader::{DashFlowConfig, EmbeddingConfig};
    /// use dashflow_ollama::OllamaEmbeddings;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let yaml = r#"
    /// embeddings:
    ///   default:
    ///     type: ollama
    ///     model: nomic-embed-text
    ///     base_url: http://localhost:11434
    /// "#;
    ///
    /// let config = DashFlowConfig::from_yaml(yaml)?;
    /// let embedding_config = config.get_embedding("default").unwrap();
    /// let embedder = OllamaEmbeddings::from_config(embedding_config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config(config: &dashflow::core::config_loader::EmbeddingConfig) -> Result<Self> {
        match config {
            dashflow::core::config_loader::EmbeddingConfig::Ollama { model, base_url } => {
                let embedder =
                    OllamaEmbeddings::with_base_url(base_url.clone()).with_model(model.clone());

                Ok(embedder)
            }
            _ => Err(Error::Configuration(
                "Expected Ollama embedding config".to_string(),
            )),
        }
    }
}

#[async_trait]
impl Embeddings for OllamaEmbeddings {
    #[allow(clippy::clone_on_ref_ptr)] // Arc<Ollama> cloned for retry closure
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Ollama's embedding API supports batch embedding natively
        // We can send all texts in a single request
        let texts_vec = texts.to_vec();
        let model = self.model.clone();
        let truncate = self.truncate;
        let client = self.client.clone();

        let response = with_retry(&self.retry_policy, move || {
            let client = client.clone();
            let model = model.clone();
            let texts_vec = texts_vec.clone();
            async move {
                let input = EmbeddingsInput::Multiple(texts_vec);
                let request = GenerateEmbeddingsRequest::new(model, input).truncate(truncate);
                client
                    .generate_embeddings(request)
                    .await
                    .map_err(|e| Error::api(format!("Ollama embeddings error: {e}")))
            }
        })
        .await?;

        Ok(response.embeddings)
    }

    #[allow(clippy::clone_on_ref_ptr)] // Arc<Ollama> cloned for retry closure
    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
        // Use the same embedding method for queries as documents
        // Some models may differentiate these, but Ollama treats them the same
        let text_string = text.to_string();
        let model = self.model.clone();
        let truncate = self.truncate;
        let client = self.client.clone();

        let response = with_retry(&self.retry_policy, move || {
            let client = client.clone();
            let model = model.clone();
            let text_string = text_string.clone();
            async move {
                let input = EmbeddingsInput::Single(text_string);
                let request = GenerateEmbeddingsRequest::new(model, input).truncate(truncate);
                client
                    .generate_embeddings(request)
                    .await
                    .map_err(|e| Error::api(format!("Ollama embeddings error: {e}")))
            }
        })
        .await?;

        // Extract the first (and only) embedding
        response
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| Error::api("No embeddings returned from Ollama".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::EmbeddingConfig as LoaderEmbeddingConfig;

    #[test]
    fn test_default_constructor() {
        let embedder = OllamaEmbeddings::new();
        assert_eq!(embedder.model, "nomic-embed-text");
        assert!(embedder.truncate);
    }

    #[test]
    fn test_with_model() {
        let embedder = OllamaEmbeddings::new().with_model("mxbai-embed-large");
        assert_eq!(embedder.model, "mxbai-embed-large");
    }

    #[test]
    fn test_with_truncate() {
        let embedder = OllamaEmbeddings::new().with_truncate(false);
        assert!(!embedder.truncate);
    }

    #[test]
    fn test_builder_chaining() {
        let embedder = OllamaEmbeddings::new()
            .with_model("all-minilm")
            .with_truncate(false);

        assert_eq!(embedder.model, "all-minilm");
        assert!(!embedder.truncate);
    }

    #[test]
    fn test_with_base_url() {
        let embedder = OllamaEmbeddings::with_base_url("http://custom-host:8080");
        assert_eq!(embedder.model, "nomic-embed-text");
    }

    #[test]
    fn test_from_config_ok() {
        let config = LoaderEmbeddingConfig::Ollama {
            model: "mxbai-embed-large".to_string(),
            base_url: "http://localhost:11434".to_string(),
        };

        let embedder = OllamaEmbeddings::from_config(&config).unwrap();
        assert_eq!(embedder.model, "mxbai-embed-large");
        assert!(embedder.truncate);
    }

    #[test]
    fn test_from_config_wrong_provider_fails() {
        use dashflow::core::config_loader::SecretReference;

        let config = LoaderEmbeddingConfig::OpenAI {
            model: "text-embedding-3-small".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            batch_size: 16,
        };

        let err = OllamaEmbeddings::from_config(&config).unwrap_err();
        assert!(err.to_string().contains("Expected Ollama embedding config"));
    }

    #[tokio::test]
    async fn test_embed_documents_empty_short_circuits() {
        let embedder = OllamaEmbeddings::new();
        let texts: Vec<String> = Vec::new();
        let vectors = embedder._embed_documents(&texts).await.unwrap();
        assert_eq!(vectors.len(), 0);
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that OllamaEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;

    /// Helper function to create a test embeddings model
    fn create_test_embeddings() -> OllamaEmbeddings {
        // Note: Ollama runs locally and doesn't require API keys.
        // Environmental error handling in standard tests will skip if service is unavailable.
        OllamaEmbeddings::new().with_model("nomic-embed-text") // Default Ollama embedding model
    }

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_embed_query_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_embed_query(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_embed_documents_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_embed_documents(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_empty_input_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_empty_input(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_dimension_consistency_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_dimension_consistency(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_semantic_similarity_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_semantic_similarity(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 6: Large text handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_large_text_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_large_text(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 7: Special characters
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_special_characters_embeddings_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_special_characters_embeddings(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 8: Batch consistency
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_batch_consistency_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_batch_consistency(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 9: Whitespace handling
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_whitespace_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_whitespace(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 10: Repeated embeddings
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_repeated_embeddings_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_repeated_embeddings(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 11: Concurrent embeddings
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_concurrent_embeddings_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_concurrent_embeddings(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 12: Numeric text
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_numeric_text_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_numeric_text(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 13: Single character
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_single_character_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_single_character(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 14: Large batch
    #[tokio::test]
    #[ignore = "requires Ollama server on localhost:11434"]
    async fn test_large_batch_embeddings_standard() {
        if dashflow_test_utils::ollama_credentials().is_err() {
            return;
        }
        test_large_batch_embeddings(Arc::new(create_test_embeddings())).await;
    }
}
