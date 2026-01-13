//! LLM Factory - Provider-agnostic model selection
//!
//! Selects LLM based on environment and requirements, NOT hardcoded provider.
//!
//! # Usage
//!
//! ```rust,ignore
//! use common::llm_factory::{create_llm, LLMRequirements};
//!
//! // Basic usage - get any available LLM
//! let llm = create_llm(LLMRequirements::default())?;
//!
//! // With specific requirements
//! let llm = create_llm(LLMRequirements {
//!     needs_tools: true,
//!     prefer_local: true,
//!     ..Default::default()
//! })?;
//! ```
//!
//! # Provider Priority
//!
//! 1. **Ollama** (local) - If prefer_local is true and Ollama is available
//! 2. **AWS Bedrock** - If AWS credentials are available
//! 3. **OpenAI** - If OPENAI_API_KEY is set
//! 4. **Anthropic** - If ANTHROPIC_API_KEY is set

use dashflow::core::language_models::ChatModel;
use std::sync::Arc;

/// LLM requirements for provider selection
#[derive(Debug, Clone, Default)]
pub struct LLMRequirements {
    /// Minimum context window (tokens)
    pub min_context: Option<usize>,
    /// Requires function/tool calling
    pub needs_tools: bool,
    /// Requires JSON mode
    pub needs_json_mode: bool,
    /// Requires vision
    pub needs_vision: bool,
    /// Maximum cost per 1M tokens (USD)
    pub max_cost_per_million: Option<f64>,
    /// Prefer local (Ollama) if available
    pub prefer_local: bool,
}

/// Result of provider detection
#[derive(Debug)]
pub struct ProviderInfo {
    pub name: &'static str,
    pub model: String,
}

/// Create an LLM based on available credentials and requirements
///
/// Returns the first available provider that meets the requirements.
/// Providers are tried in this order:
/// 1. Ollama (if prefer_local and available)
/// 2. AWS Bedrock (if credentials available)
/// 3. OpenAI (if OPENAI_API_KEY set)
/// 4. Anthropic (if ANTHROPIC_API_KEY set)
pub async fn create_llm(requirements: LLMRequirements) -> anyhow::Result<Arc<dyn ChatModel>> {
    // 1. Try local (Ollama) if preferred
    #[cfg(feature = "ollama")]
    if requirements.prefer_local {
        if let Some(llm) = try_ollama(&requirements) {
            return Ok(llm);
        }
    }

    // 2. Try AWS Bedrock
    #[cfg(feature = "bedrock")]
    if std::env::var("AWS_ACCESS_KEY_ID").is_ok()
        || std::env::var("AWS_DEFAULT_REGION").is_ok()
        || std::env::var("AWS_REGION").is_ok()
    {
        if let Some(llm) = try_bedrock(&requirements).await {
            return Ok(llm);
        }
    }

    // 3. Try OpenAI
    if std::env::var("OPENAI_API_KEY").is_ok() {
        if let Some(llm) = try_openai(&requirements) {
            return Ok(llm);
        }
    }

    // 4. Try Anthropic
    #[cfg(feature = "anthropic")]
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        if let Some(llm) = try_anthropic(&requirements) {
            return Ok(llm);
        }
    }

    // 5. Last resort - try Ollama even if not preferred (for systems without API keys)
    #[cfg(feature = "ollama")]
    if !requirements.prefer_local {
        if let Some(llm) = try_ollama(&requirements) {
            return Ok(llm);
        }
    }

    anyhow::bail!(
        "No LLM provider available. Set one of: \
         OPENAI_API_KEY, ANTHROPIC_API_KEY, AWS_ACCESS_KEY_ID, \
         or install Ollama for local inference."
    )
}

/// Get information about available providers without creating an LLM
pub fn detect_available_providers() -> Vec<ProviderInfo> {
    let mut providers = Vec::new();

    #[cfg(feature = "ollama")]
    {
        // Check if Ollama is running by attempting a quick connection
        // For now, just check if the environment suggests Ollama
        if std::env::var("OLLAMA_HOST").is_ok()
            || std::path::Path::new("/usr/local/bin/ollama").exists()
            || std::path::Path::new("/opt/homebrew/bin/ollama").exists()
        {
            providers.push(ProviderInfo {
                name: "Ollama",
                model: "llama3.2".to_string(),
            });
        }
    }

    #[cfg(feature = "bedrock")]
    if std::env::var("AWS_ACCESS_KEY_ID").is_ok() || std::env::var("AWS_DEFAULT_REGION").is_ok() {
        providers.push(ProviderInfo {
            name: "AWS Bedrock",
            model: "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
        });
    }

    if std::env::var("OPENAI_API_KEY").is_ok() {
        providers.push(ProviderInfo {
            name: "OpenAI",
            model: "gpt-4o".to_string(),
        });
    }

    #[cfg(feature = "anthropic")]
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        providers.push(ProviderInfo {
            name: "Anthropic",
            model: "claude-3-sonnet-20240229".to_string(),
        });
    }

    providers
}

/// Try to create an OpenAI LLM
#[allow(deprecated)]
fn try_openai(req: &LLMRequirements) -> Option<Arc<dyn ChatModel>> {
    use dashflow_openai::ChatOpenAI;

    let model = if req.needs_vision {
        "gpt-4o" // Vision capable
    } else if req.max_cost_per_million.is_some_and(|c| c < 1.0) {
        "gpt-4o-mini" // Cheap
    } else {
        "gpt-4o" // Default
    };

    // ChatOpenAI uses builder pattern - always succeeds at construction
    #[allow(clippy::disallowed_methods)] // Uses Default::default() which reads OPENAI_API_KEY from env
    let llm = ChatOpenAI::with_config(Default::default()).with_model(model);
    Some(Arc::new(llm))
}

/// Try to create an AWS Bedrock LLM
#[cfg(feature = "bedrock")]
async fn try_bedrock(req: &LLMRequirements) -> Option<Arc<dyn ChatModel>> {
    use dashflow_bedrock::ChatBedrock;

    let model = if req.needs_tools {
        "anthropic.claude-3-sonnet-20240229-v1:0"
    } else if req.max_cost_per_million.is_some_and(|c| c < 1.0) {
        "anthropic.claude-3-haiku-20240307-v1:0"
    } else {
        "anthropic.claude-3-sonnet-20240229-v1:0"
    };

    // Get region from environment
    let region = std::env::var("AWS_DEFAULT_REGION")
        .or_else(|_| std::env::var("AWS_REGION"))
        .unwrap_or_else(|_| "us-east-1".to_string());

    // ChatBedrock::new is async
    match ChatBedrock::new(region).await {
        Ok(llm) => Some(Arc::new(llm.with_model(model))),
        Err(_) => None,
    }
}

/// Try to create an Anthropic LLM
#[cfg(feature = "anthropic")]
#[allow(deprecated)]
fn try_anthropic(req: &LLMRequirements) -> Option<Arc<dyn ChatModel>> {
    use dashflow::core::config_loader::{ChatModelConfig, SecretReference};

    let model = if req.needs_vision {
        "claude-3-sonnet-20240229"
    } else if req.max_cost_per_million.is_some_and(|c| c < 1.0) {
        "claude-3-haiku-20240307"
    } else {
        "claude-3-sonnet-20240229"
    };

    let config = ChatModelConfig::Anthropic {
        model: model.to_string(),
        api_key: SecretReference::EnvVar {
            env: "ANTHROPIC_API_KEY".to_string(),
        },
        temperature: None,
        max_tokens: None,
    };

    dashflow_anthropic::build_chat_model(&config).ok()
}

/// Try to create an Ollama LLM
#[cfg(feature = "ollama")]
fn try_ollama(req: &LLMRequirements) -> Option<Arc<dyn ChatModel>> {
    use dashflow_ollama::ChatOllama;

    let model = if req.needs_tools {
        // llama3.2 supports tool calling
        "llama3.2"
    } else if req.needs_vision {
        // llava supports vision
        "llava"
    } else {
        "llama3.2"
    };

    // ChatOllama uses builder pattern
    let llm = ChatOllama::with_base_url("http://localhost:11434").with_model(model);
    Some(Arc::new(llm))
}

/// Create an LLM from a ChatModelConfig
///
/// Routes to the appropriate provider-specific `build_chat_model()` function
/// based on the configuration type.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::ChatModelConfig;
/// use common::llm_factory::create_llm_from_config;
///
/// let config: ChatModelConfig = serde_yaml::from_str(yaml)?;
/// let llm = create_llm_from_config(&config)?;
/// ```
pub fn create_llm_from_config(
    config: &dashflow::core::config_loader::ChatModelConfig,
) -> anyhow::Result<Arc<dyn ChatModel>> {
    use dashflow::core::config_loader::ChatModelConfig;

    match config {
        ChatModelConfig::OpenAI { .. } => dashflow_openai::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("OpenAI build failed: {}", e)),
        #[cfg(feature = "anthropic")]
        ChatModelConfig::Anthropic { .. } => dashflow_anthropic::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("Anthropic build failed: {}", e)),
        #[cfg(not(feature = "anthropic"))]
        ChatModelConfig::Anthropic { .. } => {
            anyhow::bail!("Anthropic support not compiled in. Enable the 'anthropic' feature.")
        }
        #[cfg(feature = "ollama")]
        ChatModelConfig::Ollama { .. } => dashflow_ollama::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("Ollama build failed: {}", e)),
        #[cfg(not(feature = "ollama"))]
        ChatModelConfig::Ollama { .. } => {
            anyhow::bail!("Ollama support not compiled in. Enable the 'ollama' feature.")
        }
        ChatModelConfig::Groq { .. } => dashflow_groq::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("Groq build failed: {}", e)),
        ChatModelConfig::Mistral { .. } => dashflow_mistral::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("Mistral build failed: {}", e)),
        ChatModelConfig::DeepSeek { .. } => dashflow_deepseek::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("DeepSeek build failed: {}", e)),
        ChatModelConfig::Fireworks { .. } => dashflow_fireworks::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("Fireworks build failed: {}", e)),
        ChatModelConfig::XAI { .. } => dashflow_xai::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("xAI build failed: {}", e)),
        ChatModelConfig::Perplexity { .. } => dashflow_perplexity::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("Perplexity build failed: {}", e)),
        ChatModelConfig::HuggingFace { .. } => dashflow_huggingface::build_chat_model(config)
            .map_err(|e| anyhow::anyhow!("HuggingFace build failed: {}", e)),
    }
}

/// Create an optimizable LLMNode from an LLMNodeConfig
///
/// Routes to the appropriate provider-specific `build_llm_node()` function
/// based on the configuration type, then applies the signature.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{LLMNodeConfig, ChatModelConfig, SignatureConfig};
/// use common::llm_factory::create_llm_node;
///
/// let config = LLMNodeConfig {
///     provider: ChatModelConfig::OpenAI { ... },
///     signature: SignatureConfig { spec: "q -> a".into(), instruction: "Answer".into() },
///     optimization: None,
/// };
/// let node: LLMNode<MyState> = create_llm_node(&config)?;
/// ```
pub fn create_llm_node<S: dashflow::state::GraphState>(
    config: &dashflow::core::config_loader::LLMNodeConfig,
) -> anyhow::Result<dashflow::optimize::LLMNode<S>> {
    use dashflow::core::config_loader::ChatModelConfig;
    use dashflow::optimize::make_signature;

    // Parse the signature using make_signature helper
    let signature = make_signature(&config.signature.spec, &config.signature.instruction)
        .map_err(|e| anyhow::anyhow!("Invalid signature: {}", e))?;

    // Route to the appropriate provider
    match &config.provider {
        ChatModelConfig::OpenAI { .. } => {
            dashflow_openai::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("OpenAI LLMNode build failed: {}", e))
        }
        #[cfg(feature = "anthropic")]
        ChatModelConfig::Anthropic { .. } => {
            dashflow_anthropic::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("Anthropic LLMNode build failed: {}", e))
        }
        #[cfg(not(feature = "anthropic"))]
        ChatModelConfig::Anthropic { .. } => {
            anyhow::bail!("Anthropic support not compiled in. Enable the 'anthropic' feature.")
        }
        #[cfg(feature = "ollama")]
        ChatModelConfig::Ollama { .. } => {
            dashflow_ollama::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("Ollama LLMNode build failed: {}", e))
        }
        #[cfg(not(feature = "ollama"))]
        ChatModelConfig::Ollama { .. } => {
            anyhow::bail!("Ollama support not compiled in. Enable the 'ollama' feature.")
        }
        ChatModelConfig::Groq { .. } => dashflow_groq::build_llm_node(&config.provider, signature)
            .map_err(|e| anyhow::anyhow!("Groq LLMNode build failed: {}", e)),
        ChatModelConfig::Mistral { .. } => {
            dashflow_mistral::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("Mistral LLMNode build failed: {}", e))
        }
        ChatModelConfig::DeepSeek { .. } => {
            dashflow_deepseek::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("DeepSeek LLMNode build failed: {}", e))
        }
        ChatModelConfig::Fireworks { .. } => {
            dashflow_fireworks::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("Fireworks LLMNode build failed: {}", e))
        }
        ChatModelConfig::XAI { .. } => dashflow_xai::build_llm_node(&config.provider, signature)
            .map_err(|e| anyhow::anyhow!("xAI LLMNode build failed: {}", e)),
        ChatModelConfig::Perplexity { .. } => {
            dashflow_perplexity::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("Perplexity LLMNode build failed: {}", e))
        }
        ChatModelConfig::HuggingFace { .. } => {
            dashflow_huggingface::build_llm_node(&config.provider, signature)
                .map_err(|e| anyhow::anyhow!("HuggingFace LLMNode build failed: {}", e))
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_default_requirements() {
        let req = LLMRequirements::default();
        assert!(!req.needs_tools);
        assert!(!req.needs_vision);
        assert!(!req.prefer_local);
    }

    #[test]
    fn test_detect_providers() {
        // This test doesn't fail - it just reports what's available
        let providers = detect_available_providers();
        println!("Available providers: {:?}", providers);
        // At minimum, the test infrastructure should work
    }
}
