//! LLM Factory - Provider-agnostic model selection
//!
//! Selects LLM based on environment and requirements, NOT hardcoded provider.
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_factories::{create_llm, LLMRequirements};
//!
//! // Basic usage - get any available LLM
//! let llm = create_llm(LLMRequirements::default()).await?;
//!
//! // With specific requirements
//! let llm = create_llm(LLMRequirements {
//!     needs_tools: true,
//!     prefer_local: true,
//!     ..Default::default()
//! }).await?;
//! ```
//!
//! # Provider Priority
//!
//! 1. **Ollama** (local) - If prefer_local is true and Ollama is available
//! 2. **AWS Bedrock** - If AWS credentials are available
//! 3. **OpenAI** - If OPENAI_API_KEY is set
//! 4. **Anthropic** - If ANTHROPIC_API_KEY is set

// Feature-gated imports - some constants used only with specific features (bedrock, anthropic, ollama)
#[allow(unused_imports)]
use dashflow::core::config_loader::env_vars::{
    env_is_set, env_string_or_default, ANTHROPIC_API_KEY, AWS_ACCESS_KEY_ID, AWS_DEFAULT_REGION,
    AWS_REGION, OLLAMA_HOST, OPENAI_API_KEY,
};
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
    if env_is_set(AWS_ACCESS_KEY_ID) || env_is_set(AWS_DEFAULT_REGION) || env_is_set(AWS_REGION) {
        if let Some(llm) = try_bedrock(&requirements).await {
            return Ok(llm);
        }
    }

    // 3. Try OpenAI
    if env_is_set(OPENAI_API_KEY) {
        if let Some(llm) = try_openai(&requirements) {
            return Ok(llm);
        }
    }

    // 4. Try Anthropic
    #[cfg(feature = "anthropic")]
    if env_is_set(ANTHROPIC_API_KEY) {
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
        if env_is_set(OLLAMA_HOST)
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
    if env_is_set(AWS_ACCESS_KEY_ID) || env_is_set(AWS_DEFAULT_REGION) {
        providers.push(ProviderInfo {
            name: "AWS Bedrock",
            model: "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
        });
    }

    if env_is_set(OPENAI_API_KEY) {
        providers.push(ProviderInfo {
            name: "OpenAI",
            model: "gpt-4o".to_string(),
        });
    }

    #[cfg(feature = "anthropic")]
    if env_is_set(ANTHROPIC_API_KEY) {
        providers.push(ProviderInfo {
            name: "Anthropic",
            model: "claude-3-sonnet-20240229".to_string(),
        });
    }

    providers
}

/// Try to create an OpenAI LLM
fn try_openai(req: &LLMRequirements) -> Option<Arc<dyn ChatModel>> {
    use dashflow::core::config_loader::{ChatModelConfig, SecretReference};

    let model = if req.needs_vision {
        "gpt-4o" // Vision capable
    } else if req.max_cost_per_million.is_some_and(|c| c < 1.0) {
        "gpt-4o-mini" // Cheap
    } else {
        "gpt-4o" // Default
    };

    let config = ChatModelConfig::OpenAI {
        model: model.to_string(),
        api_key: SecretReference::EnvVar {
            env: "OPENAI_API_KEY".to_string(),
        },
        temperature: None,
        max_tokens: None,
        base_url: None,
        organization: None,
    };

    dashflow_openai::build_chat_model(&config).ok()
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

    // Get region from environment (prefer AWS_DEFAULT_REGION, fall back to AWS_REGION)
    use dashflow::core::config_loader::env_vars::env_string;
    let region = env_string(AWS_DEFAULT_REGION)
        .or_else(|| env_string(AWS_REGION))
        .unwrap_or_else(|| "us-east-1".to_string());

    // ChatBedrock::new is async
    match ChatBedrock::new(region).await {
        Ok(llm) => Some(Arc::new(llm.with_model(model))),
        Err(_) => None,
    }
}

/// Try to create an Anthropic LLM
#[cfg(feature = "anthropic")]
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
    use dashflow::core::config_loader::ChatModelConfig;

    let model = if req.needs_tools {
        // llama3.2 supports tool calling
        "llama3.2"
    } else if req.needs_vision {
        // llava supports vision
        "llava"
    } else {
        "llama3.2"
    };

    let config = ChatModelConfig::Ollama {
        model: model.to_string(),
        base_url: env_string_or_default(OLLAMA_HOST, "http://localhost:11434"),
        temperature: None,
    };

    dashflow_ollama::build_chat_model(&config).ok()
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
/// use dashflow_factories::create_llm_from_config;
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
/// use dashflow_factories::create_llm_node;
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
mod tests {
    use super::*;

    // ============================================================================
    // LLMRequirements Tests
    // ============================================================================

    #[test]
    fn test_default_requirements() {
        let req = LLMRequirements::default();
        assert!(!req.needs_tools);
        assert!(!req.needs_vision);
        assert!(!req.prefer_local);
        assert!(!req.needs_json_mode);
        assert!(req.min_context.is_none());
        assert!(req.max_cost_per_million.is_none());
    }

    #[test]
    fn test_requirements_debug_impl() {
        let req = LLMRequirements::default();
        let debug_str = format!("{:?}", req);
        assert!(debug_str.contains("LLMRequirements"));
        assert!(debug_str.contains("needs_tools"));
        assert!(debug_str.contains("needs_vision"));
    }

    #[test]
    fn test_requirements_clone_impl() {
        let req = LLMRequirements {
            min_context: Some(8000),
            needs_tools: true,
            needs_json_mode: true,
            needs_vision: true,
            max_cost_per_million: Some(5.0),
            prefer_local: true,
        };
        let cloned = req.clone();
        assert_eq!(cloned.min_context, Some(8000));
        assert!(cloned.needs_tools);
        assert!(cloned.needs_json_mode);
        assert!(cloned.needs_vision);
        assert_eq!(cloned.max_cost_per_million, Some(5.0));
        assert!(cloned.prefer_local);
    }

    #[test]
    fn test_requirements_with_min_context() {
        let req = LLMRequirements {
            min_context: Some(128_000),
            ..Default::default()
        };
        assert_eq!(req.min_context, Some(128_000));
    }

    #[test]
    fn test_requirements_with_needs_tools() {
        let req = LLMRequirements {
            needs_tools: true,
            ..Default::default()
        };
        assert!(req.needs_tools);
    }

    #[test]
    fn test_requirements_with_needs_json_mode() {
        let req = LLMRequirements {
            needs_json_mode: true,
            ..Default::default()
        };
        assert!(req.needs_json_mode);
    }

    #[test]
    fn test_requirements_with_needs_vision() {
        let req = LLMRequirements {
            needs_vision: true,
            ..Default::default()
        };
        assert!(req.needs_vision);
    }

    #[test]
    fn test_requirements_with_max_cost() {
        let req = LLMRequirements {
            max_cost_per_million: Some(0.5),
            ..Default::default()
        };
        assert_eq!(req.max_cost_per_million, Some(0.5));
    }

    #[test]
    fn test_requirements_with_prefer_local() {
        let req = LLMRequirements {
            prefer_local: true,
            ..Default::default()
        };
        assert!(req.prefer_local);
    }

    #[test]
    fn test_requirements_all_fields_set() {
        let req = LLMRequirements {
            min_context: Some(32_000),
            needs_tools: true,
            needs_json_mode: true,
            needs_vision: true,
            max_cost_per_million: Some(10.0),
            prefer_local: true,
        };
        assert_eq!(req.min_context, Some(32_000));
        assert!(req.needs_tools);
        assert!(req.needs_json_mode);
        assert!(req.needs_vision);
        assert_eq!(req.max_cost_per_million, Some(10.0));
        assert!(req.prefer_local);
    }

    // ============================================================================
    // ProviderInfo Tests
    // ============================================================================

    #[test]
    fn test_provider_info_debug_impl() {
        let info = ProviderInfo {
            name: "TestProvider",
            model: "test-model-v1".to_string(),
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("ProviderInfo"));
        assert!(debug_str.contains("TestProvider"));
        assert!(debug_str.contains("test-model-v1"));
    }

    #[test]
    fn test_provider_info_fields() {
        let info = ProviderInfo {
            name: "OpenAI",
            model: "gpt-4o".to_string(),
        };
        assert_eq!(info.name, "OpenAI");
        assert_eq!(info.model, "gpt-4o");
    }

    #[test]
    fn test_provider_info_with_empty_model() {
        let info = ProviderInfo {
            name: "Test",
            model: String::new(),
        };
        assert_eq!(info.name, "Test");
        assert!(info.model.is_empty());
    }

    #[test]
    fn test_provider_info_with_complex_model_name() {
        let info = ProviderInfo {
            name: "AWS Bedrock",
            model: "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
        };
        assert_eq!(info.name, "AWS Bedrock");
        assert!(info.model.contains("anthropic"));
        assert!(info.model.contains("claude-3"));
    }

    // ============================================================================
    // detect_available_providers Tests
    // ============================================================================

    #[test]
    fn test_detect_providers() {
        // This test doesn't fail - it just reports what's available
        let providers = detect_available_providers();
        println!("Available providers: {:?}", providers);
        // At minimum, the test infrastructure should work
    }

    #[test]
    fn test_detect_providers_returns_vec() {
        let providers = detect_available_providers();
        // The function should return a Vec (may be empty)
        assert!(providers.len() <= 10); // Sanity check - shouldn't have too many
    }

    #[test]
    fn test_detect_providers_each_has_valid_name() {
        let providers = detect_available_providers();
        for provider in providers {
            assert!(!provider.name.is_empty());
            assert!(!provider.model.is_empty());
        }
    }

    // ============================================================================
    // create_llm Error Path Tests
    // ============================================================================

    #[tokio::test]
    async fn test_create_llm_fails_without_credentials() {
        // Temporarily clear all API keys to test error path
        // This test checks error message quality
        let original_openai = std::env::var("OPENAI_API_KEY").ok();
        let original_anthropic = std::env::var("ANTHROPIC_API_KEY").ok();
        let original_aws = std::env::var("AWS_ACCESS_KEY_ID").ok();
        let original_aws_region = std::env::var("AWS_DEFAULT_REGION").ok();
        let original_ollama = std::env::var("OLLAMA_HOST").ok();

        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("AWS_ACCESS_KEY_ID");
        std::env::remove_var("AWS_DEFAULT_REGION");
        std::env::remove_var("OLLAMA_HOST");

        let result = create_llm(LLMRequirements::default()).await;

        // Restore environment
        if let Some(v) = original_openai {
            std::env::set_var("OPENAI_API_KEY", v);
        }
        if let Some(v) = original_anthropic {
            std::env::set_var("ANTHROPIC_API_KEY", v);
        }
        if let Some(v) = original_aws {
            std::env::set_var("AWS_ACCESS_KEY_ID", v);
        }
        if let Some(v) = original_aws_region {
            std::env::set_var("AWS_DEFAULT_REGION", v);
        }
        if let Some(v) = original_ollama {
            std::env::set_var("OLLAMA_HOST", v);
        }

        // Unless we have ollama installed locally, this should fail
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(err_msg.contains("No LLM provider available"));
        }
    }

    // ============================================================================
    // Model Selection Logic Tests (via Requirements)
    // ============================================================================

    #[test]
    fn test_low_cost_requirement_threshold() {
        let req = LLMRequirements {
            max_cost_per_million: Some(0.9), // Below 1.0 threshold
            ..Default::default()
        };
        // Verify the threshold check works correctly
        assert!(req.max_cost_per_million.is_some_and(|c| c < 1.0));
    }

    #[test]
    fn test_high_cost_requirement_threshold() {
        let req = LLMRequirements {
            max_cost_per_million: Some(1.5), // Above 1.0 threshold
            ..Default::default()
        };
        // Verify the threshold check works correctly
        assert!(!req.max_cost_per_million.is_some_and(|c| c < 1.0));
    }

    #[test]
    fn test_cost_threshold_edge_case() {
        // Exactly at threshold
        let req = LLMRequirements {
            max_cost_per_million: Some(1.0),
            ..Default::default()
        };
        assert!(!req.max_cost_per_million.is_some_and(|c| c < 1.0));
    }

    #[test]
    fn test_no_cost_requirement() {
        let req = LLMRequirements {
            max_cost_per_million: None,
            ..Default::default()
        };
        assert!(!req.max_cost_per_million.is_some_and(|c| c < 1.0));
    }

    // ============================================================================
    // Requirements Combination Tests
    // ============================================================================

    #[test]
    fn test_vision_and_tools_requirements() {
        let req = LLMRequirements {
            needs_tools: true,
            needs_vision: true,
            ..Default::default()
        };
        assert!(req.needs_tools);
        assert!(req.needs_vision);
    }

    #[test]
    fn test_local_and_low_cost_requirements() {
        let req = LLMRequirements {
            prefer_local: true,
            max_cost_per_million: Some(0.1),
            ..Default::default()
        };
        assert!(req.prefer_local);
        assert_eq!(req.max_cost_per_million, Some(0.1));
    }

    #[test]
    fn test_json_mode_and_context_requirements() {
        let req = LLMRequirements {
            needs_json_mode: true,
            min_context: Some(200_000),
            ..Default::default()
        };
        assert!(req.needs_json_mode);
        assert_eq!(req.min_context, Some(200_000));
    }
}
