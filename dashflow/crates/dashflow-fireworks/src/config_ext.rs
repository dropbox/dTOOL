//! Build ChatModel and LLMNode from ChatModelConfig
//!
//! This module provides functions to build `ChatFireworks` and optimizable `LLMNode`
//! instances from configuration.

use crate::{ChatFireworks, FIREWORKS_API_BASE};
use async_openai::config::OpenAIConfig;
use dashflow::core::config_loader::ChatModelConfig;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build a Fireworks ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_fireworks::build_chat_model;
///
/// let config = ChatModelConfig::Fireworks {
///     model: "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
///     api_key: SecretReference::from_env("FIREWORKS_API_KEY"),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a Fireworks config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::Fireworks {
            model,
            api_key,
            temperature,
        } => {
            let key = api_key.resolve()?;

            let openai_config = OpenAIConfig::new()
                .with_api_key(&key)
                .with_api_base(FIREWORKS_API_BASE);

            let mut llm = ChatFireworks::with_config(openai_config).with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-fireworks only supports Fireworks configs, \
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
/// use dashflow_fireworks::build_llm_node;
///
/// let config = ChatModelConfig::Fireworks {
///     model: "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
///     api_key: SecretReference::from_env("FIREWORKS_API_KEY"),
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
/// - The config is not a Fireworks config
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
    fn test_build_fireworks_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_FIREWORKS_KEY", "fw-test-key");

        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
            api_key: SecretReference::from_env("TEST_FIREWORKS_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "fireworks-chat");

        std::env::remove_var("TEST_FIREWORKS_KEY");
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
            Err(err) => assert!(err.to_string().contains("only supports Fireworks configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }

    // ========================================================================
    // COMPREHENSIVE CONFIG TESTS
    // ========================================================================

    #[test]
    fn test_build_fireworks_config_no_temperature() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_FIREWORKS_KEY_2", "fw-test-key");

        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v3p3-70b-instruct".to_string(),
            api_key: SecretReference::from_env("TEST_FIREWORKS_KEY_2"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        let model = result.unwrap();
        assert_eq!(model.llm_type(), "fireworks-chat");

        std::env::remove_var("TEST_FIREWORKS_KEY_2");
    }

    #[test]
    fn test_build_fireworks_config_various_models() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_FIREWORKS_KEY_3", "fw-test-key");

        let models = [
            "accounts/fireworks/models/llama-v3p1-8b-instruct",
            "accounts/fireworks/models/llama-v3p3-70b-instruct",
            "accounts/fireworks/models/qwen2p5-72b-instruct",
            "accounts/fireworks/models/mixtral-8x7b-instruct",
        ];

        for model_name in models {
            let config = ChatModelConfig::Fireworks {
                model: model_name.to_string(),
                api_key: SecretReference::from_env("TEST_FIREWORKS_KEY_3"),
                temperature: None,
            };

            let result = build_chat_model(&config);
            assert!(result.is_ok(), "Failed for model: {}", model_name);
        }

        std::env::remove_var("TEST_FIREWORKS_KEY_3");
    }

    #[test]
    fn test_build_fireworks_config_temperature_boundary() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_FIREWORKS_KEY_4", "fw-test-key");

        // Test temperature = 0.0
        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
            api_key: SecretReference::from_env("TEST_FIREWORKS_KEY_4"),
            temperature: Some(0.0),
        };
        assert!(build_chat_model(&config).is_ok());

        // Test temperature = 2.0
        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
            api_key: SecretReference::from_env("TEST_FIREWORKS_KEY_4"),
            temperature: Some(2.0),
        };
        assert!(build_chat_model(&config).is_ok());

        std::env::remove_var("TEST_FIREWORKS_KEY_4");
    }

    #[test]
    fn test_build_wrong_provider_anthropic() {
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-opus-20240229".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            temperature: None,
            max_tokens: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            err.to_string().contains("only supports Fireworks configs"),
            "Error was: {}",
            err
        );
    }

    #[test]
    fn test_build_missing_env_var_fails() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Make sure the env var doesn't exist
        std::env::remove_var("NONEXISTENT_FIREWORKS_KEY");

        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_FIREWORKS_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_inline_api_key() {
        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
            api_key: SecretReference::from_inline("fw-inline-key"),
            temperature: Some(0.5),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        let model = result.unwrap();
        assert_eq!(model.llm_type(), "fireworks-chat");
    }

    #[test]
    fn test_config_provider_name() {
        let config = ChatModelConfig::Fireworks {
            model: "test-model".to_string(),
            api_key: SecretReference::from_inline("test-key"),
            temperature: None,
        };

        assert_eq!(config.provider(), "fireworks");
    }
}
