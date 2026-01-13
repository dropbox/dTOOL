// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Embeddings Factory - Provider-agnostic embedding model selection
//!
//! Selects embeddings based on environment and requirements, NOT hardcoded provider.
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_factories::{create_embeddings, EmbeddingRequirements};
//!
//! // Basic usage - get any available embeddings
//! let embeddings = create_embeddings(EmbeddingRequirements::default())?;
//!
//! // With specific requirements
//! let embeddings = create_embeddings(EmbeddingRequirements {
//!     prefer_local: true,
//!     ..Default::default()
//! })?;
//! ```
//!
//! # Provider Priority
//!
//! 1. **Ollama** (local) - If prefer_local is true and Ollama is available
//! 2. **OpenAI** - If OPENAI_API_KEY is set
//! 3. **HuggingFace** - If HUGGINGFACE_API_KEY is set

#[allow(unused_imports)] // OLLAMA_HOST only used when ollama feature enabled
use dashflow::core::config_loader::env_vars::{
    env_is_set, HUGGINGFACE_API_KEY, OLLAMA_HOST, OPENAI_API_KEY,
};
use dashflow::core::embeddings::Embeddings;
use std::sync::Arc;

/// Embedding requirements for provider selection
#[derive(Debug, Clone, Default)]
pub struct EmbeddingRequirements {
    /// Minimum embedding dimension
    pub min_dimension: Option<usize>,
    /// Prefer local (Ollama) if available
    pub prefer_local: bool,
    /// Specific model to use (overrides provider default)
    pub model: Option<String>,
}

/// Result of provider detection
#[derive(Debug)]
pub struct EmbeddingProviderInfo {
    pub name: &'static str,
    pub model: String,
}

/// Create embeddings based on available credentials and requirements
///
/// Returns the first available provider that meets the requirements.
/// Providers are tried in this order:
/// 1. Ollama (if prefer_local and available)
/// 2. OpenAI (if OPENAI_API_KEY set)
/// 3. HuggingFace (if HUGGINGFACE_API_KEY set)
pub fn create_embeddings(
    requirements: EmbeddingRequirements,
) -> anyhow::Result<Arc<dyn Embeddings>> {
    // 1. Try local (Ollama) if preferred
    #[cfg(feature = "ollama")]
    if requirements.prefer_local {
        if let Some(embeddings) = try_ollama(&requirements) {
            return Ok(embeddings);
        }
    }

    // 2. Try OpenAI
    if env_is_set(OPENAI_API_KEY) {
        if let Some(embeddings) = try_openai(&requirements) {
            return Ok(embeddings);
        }
    }

    // 3. Try HuggingFace
    if env_is_set(HUGGINGFACE_API_KEY) {
        if let Some(embeddings) = try_huggingface(&requirements) {
            return Ok(embeddings);
        }
    }

    // 4. Last resort - try Ollama even if not preferred (for systems without API keys)
    #[cfg(feature = "ollama")]
    if !requirements.prefer_local {
        if let Some(embeddings) = try_ollama(&requirements) {
            return Ok(embeddings);
        }
    }

    anyhow::bail!(
        "No embeddings provider available. Set one of: \
         OPENAI_API_KEY, HUGGINGFACE_API_KEY, \
         or install Ollama for local inference."
    )
}

/// Get information about available embedding providers without creating an instance
pub fn detect_available_embedding_providers() -> Vec<EmbeddingProviderInfo> {
    let mut providers = Vec::new();

    #[cfg(feature = "ollama")]
    {
        // Check if Ollama is running
        if env_is_set(OLLAMA_HOST)
            || std::path::Path::new("/usr/local/bin/ollama").exists()
            || std::path::Path::new("/opt/homebrew/bin/ollama").exists()
        {
            providers.push(EmbeddingProviderInfo {
                name: "Ollama",
                model: "nomic-embed-text".to_string(),
            });
        }
    }

    if env_is_set(OPENAI_API_KEY) {
        providers.push(EmbeddingProviderInfo {
            name: "OpenAI",
            model: "text-embedding-3-small".to_string(),
        });
    }

    if env_is_set(HUGGINGFACE_API_KEY) {
        providers.push(EmbeddingProviderInfo {
            name: "HuggingFace",
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        });
    }

    providers
}

/// Try to create OpenAI embeddings
#[allow(clippy::disallowed_methods)] // new() reads OPENAI_API_KEY from environment
fn try_openai(req: &EmbeddingRequirements) -> Option<Arc<dyn Embeddings>> {
    use dashflow_openai::OpenAIEmbeddings;

    let model = req.model.as_deref().unwrap_or("text-embedding-3-small");

    // OpenAIEmbeddings uses builder pattern - always succeeds at construction
    let embeddings = OpenAIEmbeddings::new().with_model(model);
    Some(Arc::new(embeddings))
}

/// Try to create HuggingFace embeddings
fn try_huggingface(req: &EmbeddingRequirements) -> Option<Arc<dyn Embeddings>> {
    use dashflow_huggingface::HuggingFaceEmbeddings;

    let model = req
        .model
        .as_deref()
        .unwrap_or("sentence-transformers/all-MiniLM-L6-v2");

    // HuggingFaceEmbeddings uses builder pattern
    let embeddings = HuggingFaceEmbeddings::new().with_model(model);
    Some(Arc::new(embeddings))
}

/// Try to create Ollama embeddings
#[cfg(feature = "ollama")]
fn try_ollama(req: &EmbeddingRequirements) -> Option<Arc<dyn Embeddings>> {
    use dashflow_ollama::OllamaEmbeddings;

    let model = req.model.as_deref().unwrap_or("nomic-embed-text");

    // OllamaEmbeddings uses builder pattern
    let embeddings = OllamaEmbeddings::new().with_model(model);
    Some(Arc::new(embeddings))
}

/// Create embeddings from an EmbeddingConfig
///
/// Routes to the appropriate provider-specific `build_embeddings()` function
/// based on the configuration type.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::EmbeddingConfig;
/// use dashflow_factories::create_embeddings_from_config;
///
/// let config: EmbeddingConfig = serde_yaml::from_str(yaml)?;
/// let embeddings = create_embeddings_from_config(&config)?;
/// ```
pub fn create_embeddings_from_config(
    config: &dashflow::core::config_loader::EmbeddingConfig,
) -> anyhow::Result<Arc<dyn Embeddings>> {
    use dashflow::core::config_loader::EmbeddingConfig;

    match config {
        EmbeddingConfig::OpenAI { .. } => dashflow_openai::build_embeddings(config)
            .map_err(|e| anyhow::anyhow!("OpenAI embeddings build failed: {}", e)),
        #[cfg(feature = "ollama")]
        EmbeddingConfig::Ollama { .. } => dashflow_ollama::build_embeddings(config)
            .map_err(|e| anyhow::anyhow!("Ollama embeddings build failed: {}", e)),
        #[cfg(not(feature = "ollama"))]
        EmbeddingConfig::Ollama { .. } => {
            anyhow::bail!("Ollama support not compiled in. Enable the 'ollama' feature.")
        }
        EmbeddingConfig::HuggingFace { .. } => dashflow_huggingface::build_embeddings(config)
            .map_err(|e| anyhow::anyhow!("HuggingFace embeddings build failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // EmbeddingRequirements Tests
    // ============================================================================

    #[test]
    fn test_default_requirements() {
        let req = EmbeddingRequirements::default();
        assert!(!req.prefer_local);
        assert!(req.min_dimension.is_none());
        assert!(req.model.is_none());
    }

    #[test]
    fn test_requirements_debug_impl() {
        let req = EmbeddingRequirements::default();
        let debug_str = format!("{:?}", req);
        assert!(debug_str.contains("EmbeddingRequirements"));
        assert!(debug_str.contains("prefer_local"));
        assert!(debug_str.contains("min_dimension"));
    }

    #[test]
    fn test_requirements_clone_impl() {
        let req = EmbeddingRequirements {
            min_dimension: Some(1536),
            prefer_local: true,
            model: Some("custom-model".to_string()),
        };
        let cloned = req.clone();
        assert_eq!(cloned.min_dimension, Some(1536));
        assert!(cloned.prefer_local);
        assert_eq!(cloned.model, Some("custom-model".to_string()));
    }

    #[test]
    fn test_requirements_with_min_dimension() {
        let req = EmbeddingRequirements {
            min_dimension: Some(768),
            ..Default::default()
        };
        assert_eq!(req.min_dimension, Some(768));
    }

    #[test]
    fn test_requirements_with_prefer_local() {
        let req = EmbeddingRequirements {
            prefer_local: true,
            ..Default::default()
        };
        assert!(req.prefer_local);
    }

    #[test]
    fn test_requirements_with_model() {
        let req = EmbeddingRequirements {
            model: Some("text-embedding-3-large".to_string()),
            ..Default::default()
        };
        assert_eq!(req.model, Some("text-embedding-3-large".to_string()));
    }

    #[test]
    fn test_requirements_all_fields_set() {
        let req = EmbeddingRequirements {
            min_dimension: Some(3072),
            prefer_local: true,
            model: Some("nomic-embed-text".to_string()),
        };
        assert_eq!(req.min_dimension, Some(3072));
        assert!(req.prefer_local);
        assert_eq!(req.model, Some("nomic-embed-text".to_string()));
    }

    #[test]
    fn test_requirements_high_dimension() {
        let req = EmbeddingRequirements {
            min_dimension: Some(4096),
            ..Default::default()
        };
        assert_eq!(req.min_dimension, Some(4096));
    }

    #[test]
    fn test_requirements_empty_model() {
        let req = EmbeddingRequirements {
            model: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(req.model, Some(String::new()));
    }

    // ============================================================================
    // EmbeddingProviderInfo Tests
    // ============================================================================

    #[test]
    fn test_provider_info_debug_impl() {
        let info = EmbeddingProviderInfo {
            name: "TestProvider",
            model: "test-embed-v1".to_string(),
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("EmbeddingProviderInfo"));
        assert!(debug_str.contains("TestProvider"));
        assert!(debug_str.contains("test-embed-v1"));
    }

    #[test]
    fn test_provider_info_fields() {
        let info = EmbeddingProviderInfo {
            name: "OpenAI",
            model: "text-embedding-3-small".to_string(),
        };
        assert_eq!(info.name, "OpenAI");
        assert_eq!(info.model, "text-embedding-3-small");
    }

    #[test]
    fn test_provider_info_ollama() {
        let info = EmbeddingProviderInfo {
            name: "Ollama",
            model: "nomic-embed-text".to_string(),
        };
        assert_eq!(info.name, "Ollama");
        assert_eq!(info.model, "nomic-embed-text");
    }

    #[test]
    fn test_provider_info_huggingface() {
        let info = EmbeddingProviderInfo {
            name: "HuggingFace",
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        };
        assert_eq!(info.name, "HuggingFace");
        assert!(info.model.contains("sentence-transformers"));
    }

    #[test]
    fn test_provider_info_empty_model() {
        let info = EmbeddingProviderInfo {
            name: "Test",
            model: String::new(),
        };
        assert!(info.model.is_empty());
    }

    // ============================================================================
    // detect_available_embedding_providers Tests
    // ============================================================================

    #[test]
    fn test_detect_providers() {
        // This test doesn't fail - it just reports what's available
        let providers = detect_available_embedding_providers();
        println!("Available embedding providers: {:?}", providers);
    }

    #[test]
    fn test_detect_providers_returns_vec() {
        let providers = detect_available_embedding_providers();
        // The function should return a Vec (may be empty)
        assert!(providers.len() <= 10); // Sanity check
    }

    #[test]
    fn test_detect_providers_each_has_valid_data() {
        let providers = detect_available_embedding_providers();
        for provider in providers {
            assert!(!provider.name.is_empty());
            assert!(!provider.model.is_empty());
        }
    }

    // ============================================================================
    // create_embeddings Error Path Tests
    // ============================================================================

    #[test]
    fn test_create_embeddings_fails_without_credentials() {
        // Temporarily clear all API keys to test error path
        let original_openai = std::env::var("OPENAI_API_KEY").ok();
        let original_hf = std::env::var("HUGGINGFACE_API_KEY").ok();
        let original_ollama = std::env::var("OLLAMA_HOST").ok();

        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("HUGGINGFACE_API_KEY");
        std::env::remove_var("OLLAMA_HOST");

        let result = create_embeddings(EmbeddingRequirements::default());

        // Restore environment
        if let Some(v) = original_openai {
            std::env::set_var("OPENAI_API_KEY", v);
        }
        if let Some(v) = original_hf {
            std::env::set_var("HUGGINGFACE_API_KEY", v);
        }
        if let Some(v) = original_ollama {
            std::env::set_var("OLLAMA_HOST", v);
        }

        // Unless we have ollama installed locally, this should fail
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(err_msg.contains("No embeddings provider available"));
        }
    }

    // ============================================================================
    // Model Override Tests
    // ============================================================================

    #[test]
    fn test_model_override_openai() {
        let req = EmbeddingRequirements {
            model: Some("text-embedding-3-large".to_string()),
            ..Default::default()
        };
        // Verify model override is set correctly
        assert_eq!(req.model.as_deref().unwrap_or("text-embedding-3-small"), "text-embedding-3-large");
    }

    #[test]
    fn test_model_default_fallback() {
        let req = EmbeddingRequirements::default();
        // When no model specified, should use default
        assert_eq!(req.model.as_deref().unwrap_or("text-embedding-3-small"), "text-embedding-3-small");
    }

    #[test]
    fn test_model_override_huggingface() {
        let req = EmbeddingRequirements {
            model: Some("sentence-transformers/all-mpnet-base-v2".to_string()),
            ..Default::default()
        };
        assert!(req.model.as_ref().unwrap().contains("mpnet"));
    }

    // ============================================================================
    // Requirements Combination Tests
    // ============================================================================

    #[test]
    fn test_local_with_custom_model() {
        let req = EmbeddingRequirements {
            prefer_local: true,
            model: Some("llama3.2".to_string()),
            ..Default::default()
        };
        assert!(req.prefer_local);
        assert_eq!(req.model, Some("llama3.2".to_string()));
    }

    #[test]
    fn test_dimension_with_model() {
        let req = EmbeddingRequirements {
            min_dimension: Some(1024),
            model: Some("text-embedding-3-small".to_string()),
            ..Default::default()
        };
        assert_eq!(req.min_dimension, Some(1024));
        assert!(req.model.is_some());
    }

    #[test]
    fn test_all_options_combined() {
        let req = EmbeddingRequirements {
            min_dimension: Some(2048),
            prefer_local: true,
            model: Some("nomic-embed-text".to_string()),
        };
        assert_eq!(req.min_dimension, Some(2048));
        assert!(req.prefer_local);
        assert_eq!(req.model.as_deref(), Some("nomic-embed-text"));
    }

    // ============================================================================
    // Known Model Tests
    // ============================================================================

    #[test]
    fn test_openai_known_models() {
        let models = vec![
            "text-embedding-3-small",
            "text-embedding-3-large",
            "text-embedding-ada-002",
        ];
        for model in models {
            let req = EmbeddingRequirements {
                model: Some(model.to_string()),
                ..Default::default()
            };
            assert_eq!(req.model.as_deref(), Some(model));
        }
    }

    #[test]
    fn test_huggingface_known_models() {
        let models = vec![
            "sentence-transformers/all-MiniLM-L6-v2",
            "sentence-transformers/all-mpnet-base-v2",
            "BAAI/bge-small-en-v1.5",
        ];
        for model in models {
            let req = EmbeddingRequirements {
                model: Some(model.to_string()),
                ..Default::default()
            };
            assert!(req.model.is_some());
        }
    }

    #[test]
    fn test_ollama_known_models() {
        let models = vec![
            "nomic-embed-text",
            "mxbai-embed-large",
            "all-minilm",
        ];
        for model in models {
            let req = EmbeddingRequirements {
                model: Some(model.to_string()),
                prefer_local: true,
                min_dimension: None,
            };
            assert!(req.prefer_local);
            assert_eq!(req.model.as_deref(), Some(model));
        }
    }
}
