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
//! use common::embeddings_factory::{create_embeddings, EmbeddingRequirements};
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
    if std::env::var("OPENAI_API_KEY").is_ok() {
        if let Some(embeddings) = try_openai(&requirements) {
            return Ok(embeddings);
        }
    }

    // 3. Try HuggingFace
    if std::env::var("HUGGINGFACE_API_KEY").is_ok() {
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
        if std::env::var("OLLAMA_HOST").is_ok()
            || std::path::Path::new("/usr/local/bin/ollama").exists()
            || std::path::Path::new("/opt/homebrew/bin/ollama").exists()
        {
            providers.push(EmbeddingProviderInfo {
                name: "Ollama",
                model: "nomic-embed-text".to_string(),
            });
        }
    }

    if std::env::var("OPENAI_API_KEY").is_ok() {
        providers.push(EmbeddingProviderInfo {
            name: "OpenAI",
            model: "text-embedding-3-small".to_string(),
        });
    }

    if std::env::var("HUGGINGFACE_API_KEY").is_ok() {
        providers.push(EmbeddingProviderInfo {
            name: "HuggingFace",
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        });
    }

    providers
}

/// Try to create OpenAI embeddings
#[allow(clippy::disallowed_methods)] // Uses Default::default() which reads OPENAI_API_KEY from env
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
/// use common::embeddings_factory::create_embeddings_from_config;
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
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_default_requirements() {
        let req = EmbeddingRequirements::default();
        assert!(!req.prefer_local);
        assert!(req.min_dimension.is_none());
        assert!(req.model.is_none());
    }

    #[test]
    fn test_detect_providers() {
        // This test doesn't fail - it just reports what's available
        let providers = detect_available_embedding_providers();
        println!("Available embedding providers: {:?}", providers);
    }
}
