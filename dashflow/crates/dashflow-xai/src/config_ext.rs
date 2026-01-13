//! Build ChatModel and LLMNode from ChatModelConfig
//!
//! This module provides functions to build `ChatXAI` and optimizable `LLMNode`
//! instances from configuration.

use crate::{ChatXAI, XAI_DEFAULT_API_BASE};
use async_openai::config::OpenAIConfig;
use dashflow::core::config_loader::ChatModelConfig;
use dashflow::core::config_loader::env_vars::{
    env_string_or_default, XAI_API_BASE as XAI_API_BASE_ENV,
};
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build an xAI ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_xai::build_chat_model;
///
/// let config = ChatModelConfig::XAI {
///     model: "grok-beta".to_string(),
///     api_key: SecretReference::from_env("XAI_API_KEY"),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not an XAI config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::XAI {
            model,
            api_key,
            temperature,
        } => {
            let key = api_key.resolve()?;

            let api_base = env_string_or_default(XAI_API_BASE_ENV, XAI_DEFAULT_API_BASE);

            let openai_config = OpenAIConfig::new()
                .with_api_key(&key)
                .with_api_base(&api_base);

            let mut llm = ChatXAI::with_config(openai_config).with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-xai only supports XAI configs, \
             got {} config. Use the appropriate provider crate for this config type.",
            other.provider()
        ))),
    }
}

/// Build an optimizable LLMNode from a ChatModelConfig and Signature
///
/// This creates an LLMNode that can be used with DashOptimize algorithms
/// (BootstrapFewShot, MIPROv2, GRPO, etc.) for automatic prompt optimization.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow::optimize::{make_signature, Signature};
/// use dashflow_xai::build_llm_node;
///
/// let config = ChatModelConfig::XAI {
///     model: "grok-beta".to_string(),
///     api_key: SecretReference::from_env("XAI_API_KEY"),
///     temperature: Some(0.7),
/// };
///
/// let signature = make_signature("question -> answer", "Answer accurately")?;
/// let llm_node: LLMNode<MyState> = build_llm_node(&config, signature)?;
/// ```
///
/// # Type Parameters
///
/// * `S` - The graph state type (must implement `GraphState`)
///
/// # Errors
///
/// Returns an error if:
/// - The config is not an XAI config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_llm_node<S: GraphState>(
    config: &ChatModelConfig,
    signature: Signature,
) -> Result<LLMNode<S>, DashFlowError> {
    let llm = build_chat_model(config)?;
    Ok(LLMNode::new(signature, llm))
}

#[cfg(test)]
// SAFETY: Tests use unwrap() to panic on unexpected errors, clearly indicating test failure.
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::SecretReference;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_build_xai_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_XAI_KEY", "xai-test-key");

        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("TEST_XAI_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "grok-beta");

        std::env::remove_var("TEST_XAI_KEY");
    }

    #[test]
    fn test_build_wrong_provider_fails() {
        // No env var manipulation - doesn't need mutex
        let config = ChatModelConfig::OpenAI {
            model: "gpt-4o".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        match result {
            Err(err) => assert!(err.to_string().contains("only supports XAI configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }

    // ========================================================================
    // ADDITIONAL CONFIG_EXT TESTS
    // ========================================================================

    #[test]
    fn test_build_xai_config_without_temperature() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_XAI_KEY_2", "xai-test-key-2");

        let config = ChatModelConfig::XAI {
            model: "grok-vision-beta".to_string(),
            api_key: SecretReference::from_env("TEST_XAI_KEY_2"),
            temperature: None, // No temperature set
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        let model = result.unwrap();
        assert_eq!(model.llm_type(), "grok-beta");

        std::env::remove_var("TEST_XAI_KEY_2");
    }

    #[test]
    fn test_build_xai_config_various_models() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_XAI_KEY_3", "xai-test-key-3");

        let model_names = ["grok-beta", "grok-vision-beta", "grok-2", "grok-2-mini"];

        for model_name in model_names {
            let config = ChatModelConfig::XAI {
                model: model_name.to_string(),
                api_key: SecretReference::from_env("TEST_XAI_KEY_3"),
                temperature: Some(0.5),
            };

            let result = build_chat_model(&config);
            assert!(result.is_ok(), "Failed to build model: {}", model_name);
        }

        std::env::remove_var("TEST_XAI_KEY_3");
    }

    #[test]
    fn test_build_xai_config_temperature_zero() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_XAI_KEY_4", "xai-test-key-4");

        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("TEST_XAI_KEY_4"),
            temperature: Some(0.0), // Zero temperature for deterministic output
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_XAI_KEY_4");
    }

    #[test]
    fn test_build_xai_config_temperature_max() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_XAI_KEY_5", "xai-test-key-5");

        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("TEST_XAI_KEY_5"),
            temperature: Some(2.0), // Max temperature
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_XAI_KEY_5");
    }

    #[test]
    fn test_build_xai_config_missing_env_var() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Ensure the env var doesn't exist
        std::env::remove_var("NONEXISTENT_XAI_KEY");

        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_XAI_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_wrong_provider_anthropic() {
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            temperature: None,
            max_tokens: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        // Can't use unwrap_err since Arc<dyn ChatModel> doesn't implement Debug
        match result {
            Err(e) => assert!(e.to_string().contains("only supports XAI configs")),
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_build_wrong_provider_mistral() {
        let config = ChatModelConfig::Mistral {
            model: "mistral-large-latest".to_string(),
            api_key: SecretReference::from_env("MISTRAL_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("only supports XAI configs")),
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_build_wrong_provider_groq() {
        let config = ChatModelConfig::Groq {
            model: "llama2-70b-4096".to_string(),
            api_key: SecretReference::from_env("GROQ_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("only supports XAI configs")),
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_build_wrong_provider_fireworks() {
        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v2-70b-chat".to_string(),
            api_key: SecretReference::from_env("FIREWORKS_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("only supports XAI configs")),
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_build_wrong_provider_ollama() {
        let config = ChatModelConfig::Ollama {
            model: "llama2".to_string(),
            base_url: "http://localhost:11434".to_string(),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("only supports XAI configs")),
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_build_wrong_provider_deepseek() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("DEEPSEEK_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("only supports XAI configs")),
            Ok(_) => panic!("Expected error"),
        }
    }

    // ========================================================================
    // BUILD_LLM_NODE TESTS
    // ========================================================================

    #[test]
    fn test_build_llm_node_xai_config() {
        use dashflow::optimize::make_signature;
        use dashflow::state::AgentState;

        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_XAI_KEY_NODE", "xai-test-key-node");

        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("TEST_XAI_KEY_NODE"),
            temperature: Some(0.5),
        };

        let signature = make_signature("question -> answer", "Answer the question").unwrap();
        let result = build_llm_node::<AgentState>(&config, signature);
        assert!(result.is_ok());

        std::env::remove_var("TEST_XAI_KEY_NODE");
    }

    #[test]
    fn test_build_llm_node_wrong_provider() {
        use dashflow::optimize::make_signature;
        use dashflow::state::AgentState;

        let config = ChatModelConfig::OpenAI {
            model: "gpt-4".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        let signature = make_signature("input -> output", "Test").unwrap();
        let result = build_llm_node::<AgentState>(&config, signature);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_llm_node_missing_key() {
        use dashflow::optimize::make_signature;
        use dashflow::state::AgentState;

        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("MISSING_XAI_NODE_KEY");

        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("MISSING_XAI_NODE_KEY"),
            temperature: Some(0.5),
        };

        let signature = make_signature("q -> a", "Test").unwrap();
        let result = build_llm_node::<AgentState>(&config, signature);
        assert!(result.is_err());
    }

    // ========================================================================
    // API BASE URL TESTS
    // ========================================================================

    #[test]
    fn test_default_api_base_used() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_XAI_KEY_BASE", "xai-test-key-base");
        // Ensure custom base is not set
        std::env::remove_var("XAI_API_BASE");

        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("TEST_XAI_KEY_BASE"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_XAI_KEY_BASE");
    }

    #[test]
    fn test_build_xai_literal_api_key() {
        // Test with a literal/direct API key instead of env reference
        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_inline("literal-test-key"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_xai_empty_model_name() {
        let config = ChatModelConfig::XAI {
            model: String::new(), // Empty model name
            api_key: SecretReference::from_inline("test-key"),
            temperature: None,
        };

        // Should succeed at build time (API will reject empty model)
        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_xai_whitespace_model_name() {
        let config = ChatModelConfig::XAI {
            model: "  grok-beta  ".to_string(), // Whitespace around model name
            api_key: SecretReference::from_inline("test-key"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_provider_name() {
        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_inline("test-key"),
            temperature: None,
        };

        assert_eq!(config.provider(), "xai");
    }
}
