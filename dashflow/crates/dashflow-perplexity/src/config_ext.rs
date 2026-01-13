//! Build ChatModel and LLMNode from ChatModelConfig
//!
//! This module provides functions to build `ChatPerplexity` and optimizable `LLMNode`
//! instances from configuration.

use crate::ChatPerplexity;
use dashflow::core::config_loader::ChatModelConfig;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build a Perplexity ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_perplexity::build_chat_model;
///
/// let config = ChatModelConfig::Perplexity {
///     model: "llama-3.1-sonar-small-128k-online".to_string(),
///     api_key: SecretReference::from_env("PPLX_API_KEY"),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a Perplexity config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::Perplexity {
            model,
            api_key,
            temperature,
        } => {
            let key = api_key.resolve()?;

            let mut llm = ChatPerplexity::with_api_key(&key).with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-perplexity only supports Perplexity configs, \
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
/// use dashflow_perplexity::build_llm_node;
///
/// let config = ChatModelConfig::Perplexity {
///     model: "llama-3.1-sonar-small-128k-online".to_string(),
///     api_key: SecretReference::from_env("PPLX_API_KEY"),
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
/// - The config is not a Perplexity config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_llm_node<S: GraphState>(
    config: &ChatModelConfig,
    signature: Signature,
) -> Result<LLMNode<S>, DashFlowError> {
    let llm = build_chat_model(config)?;
    Ok(LLMNode::new(signature, llm))
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::SecretReference;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // ========================================================================
    // build_chat_model - Basic Tests
    // ========================================================================

    #[test]
    fn test_build_perplexity_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY", "pplx-test-key");

        let config = ChatModelConfig::Perplexity {
            model: "llama-3.1-sonar-small-128k-online".to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "perplexity");

        std::env::remove_var("TEST_PPLX_KEY");
    }

    #[test]
    fn test_build_perplexity_config_no_temperature() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_2", "pplx-test-key-2");

        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_2"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().llm_type(), "perplexity");

        std::env::remove_var("TEST_PPLX_KEY_2");
    }

    #[test]
    fn test_build_perplexity_config_with_sonar_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_3", "pplx-test-key-3");

        let config = ChatModelConfig::Perplexity {
            model: crate::models::SONAR.to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_3"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_PPLX_KEY_3");
    }

    #[test]
    fn test_build_perplexity_config_with_sonar_pro_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_4", "pplx-test-key-4");

        let config = ChatModelConfig::Perplexity {
            model: crate::models::SONAR_PRO.to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_4"),
            temperature: Some(0.5),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_PPLX_KEY_4");
    }

    #[test]
    fn test_build_perplexity_config_with_sonar_reasoning_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_5", "pplx-test-key-5");

        let config = ChatModelConfig::Perplexity {
            model: crate::models::SONAR_REASONING.to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_5"),
            temperature: Some(0.3),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_PPLX_KEY_5");
    }

    #[test]
    fn test_build_perplexity_temperature_zero() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_6", "pplx-test-key-6");

        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_6"),
            temperature: Some(0.0),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_PPLX_KEY_6");
    }

    #[test]
    fn test_build_perplexity_temperature_one() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_7", "pplx-test-key-7");

        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_7"),
            temperature: Some(1.0),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_PPLX_KEY_7");
    }

    #[test]
    fn test_build_perplexity_temperature_high() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_8", "pplx-test-key-8");

        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_8"),
            temperature: Some(2.0),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_PPLX_KEY_8");
    }

    // ========================================================================
    // build_chat_model - Error Tests
    // ========================================================================

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
            Err(err) => assert!(err.to_string().contains("only supports Perplexity configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }

    #[test]
    fn test_build_wrong_provider_anthropic_fails() {
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-sonnet".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            temperature: None,
            max_tokens: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
        // Verify error message content using match
        match result {
            Err(err) => assert!(err.to_string().contains("only supports Perplexity configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }

    #[test]
    fn test_build_missing_env_var_fails() {
        // Use a non-existent environment variable
        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_PPLX_KEY_12345"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    // ========================================================================
    // build_chat_model - Return Type Tests
    // ========================================================================

    #[test]
    fn test_build_returns_arc() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_9", "pplx-test-key-9");

        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_9"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify we can clone the Arc (which wouldn't work if it wasn't an Arc)
        let model = result.unwrap();
        let _cloned = Arc::clone(&model);

        std::env::remove_var("TEST_PPLX_KEY_9");
    }

    #[test]
    fn test_build_returns_dyn_chat_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_PPLX_KEY_10", "pplx-test-key-10");

        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("TEST_PPLX_KEY_10"),
            temperature: None,
        };

        let model = build_chat_model(&config).unwrap();

        // Verify we can call ChatModel trait methods
        assert_eq!(model.llm_type(), "perplexity");

        std::env::remove_var("TEST_PPLX_KEY_10");
    }

    // ========================================================================
    // build_llm_node - Tests
    // ========================================================================
    // Note: build_llm_node tests are limited because they require custom GraphState
    // types with serde derives. The functionality is tested indirectly through
    // build_chat_model tests since build_llm_node calls build_chat_model internally.

    #[test]
    fn test_build_llm_node_wrong_provider_returns_error() {
        let config = ChatModelConfig::OpenAI {
            model: "gpt-4o".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        // We can test that the underlying build_chat_model fails
        // without needing to construct the full LLMNode
        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_llm_node_missing_env_fails_at_build_chat_model() {
        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_PPLX_KEY_67890"),
            temperature: None,
        };

        // The build_llm_node would fail at the build_chat_model step
        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    // ========================================================================
    // SecretReference Tests
    // ========================================================================

    #[test]
    fn test_secret_reference_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_SECRET_VAR", "secret-value");

        let secret = SecretReference::from_env("TEST_SECRET_VAR");
        let resolved = secret.resolve();
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), "secret-value");

        std::env::remove_var("TEST_SECRET_VAR");
    }

    #[test]
    fn test_secret_reference_missing_env() {
        let secret = SecretReference::from_env("NONEXISTENT_SECRET_VAR_999");
        let resolved = secret.resolve();
        assert!(resolved.is_err());
    }

    // ========================================================================
    // ChatModelConfig Provider Tests
    // ========================================================================

    #[test]
    fn test_perplexity_config_provider() {
        let config = ChatModelConfig::Perplexity {
            model: "sonar".to_string(),
            api_key: SecretReference::from_env("KEY"),
            temperature: None,
        };

        assert_eq!(config.provider(), "perplexity");
    }

    #[test]
    fn test_openai_config_provider() {
        let config = ChatModelConfig::OpenAI {
            model: "gpt-4o".to_string(),
            api_key: SecretReference::from_env("KEY"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        assert_eq!(config.provider(), "openai");
    }
}
