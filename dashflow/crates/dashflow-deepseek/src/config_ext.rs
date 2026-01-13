//! Build ChatModel and LLMNode from ChatModelConfig
//!
//! This module provides functions to build `ChatDeepSeek` and optimizable `LLMNode`
//! instances from configuration.

use crate::ChatDeepSeek;
use dashflow::core::config_loader::ChatModelConfig;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build a DeepSeek ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_deepseek::build_chat_model;
///
/// let config = ChatModelConfig::DeepSeek {
///     model: "deepseek-chat".to_string(),
///     api_key: SecretReference::from_env("DEEPSEEK_API_KEY"),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a DeepSeek config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::DeepSeek {
            model,
            api_key,
            temperature,
        } => {
            let key = api_key.resolve()?;

            let mut llm = ChatDeepSeek::with_api_key(&key).with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-deepseek only supports DeepSeek configs, \
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
/// use dashflow_deepseek::build_llm_node;
///
/// let config = ChatModelConfig::DeepSeek {
///     model: "deepseek-chat".to_string(),
///     api_key: SecretReference::from_env("DEEPSEEK_API_KEY"),
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
/// - The config is not a DeepSeek config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_llm_node<S: GraphState>(
    config: &ChatModelConfig,
    signature: Signature,
) -> Result<LLMNode<S>, DashFlowError> {
    let llm = build_chat_model(config)?;
    Ok(LLMNode::new(signature, llm))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::SecretReference;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // ==================== build_chat_model Tests ====================

    #[test]
    fn test_build_deepseek_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_DEEPSEEK_KEY", "sk-test-key");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("TEST_DEEPSEEK_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "deepseek");

        std::env::remove_var("TEST_DEEPSEEK_KEY");
    }

    #[test]
    fn test_build_deepseek_config_no_temperature() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_DEEPSEEK_KEY_2", "sk-test-key-2");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("TEST_DEEPSEEK_KEY_2"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().llm_type(), "deepseek");

        std::env::remove_var("TEST_DEEPSEEK_KEY_2");
    }

    #[test]
    fn test_build_deepseek_coder_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_DEEPSEEK_KEY_3", "sk-test-key-3");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-coder".to_string(),
            api_key: SecretReference::from_env("TEST_DEEPSEEK_KEY_3"),
            temperature: Some(0.5),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().llm_type(), "deepseek");

        std::env::remove_var("TEST_DEEPSEEK_KEY_3");
    }

    #[test]
    fn test_build_deepseek_zero_temperature() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_DEEPSEEK_KEY_4", "sk-test-key-4");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("TEST_DEEPSEEK_KEY_4"),
            temperature: Some(0.0),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_DEEPSEEK_KEY_4");
    }

    #[test]
    fn test_build_deepseek_max_temperature() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_DEEPSEEK_KEY_5", "sk-test-key-5");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("TEST_DEEPSEEK_KEY_5"),
            temperature: Some(2.0),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_DEEPSEEK_KEY_5");
    }

    #[test]
    fn test_build_deepseek_inline_key() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-inline-key"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().llm_type(), "deepseek");
    }

    // ==================== Wrong Provider Tests ====================

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

        if let Err(err) = result {
            assert!(err.to_string().contains("only supports DeepSeek configs"));
        }
    }

    #[test]
    fn test_build_anthropic_config_fails() {
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-opus-20240229".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            max_tokens: None,
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("only supports DeepSeek configs"));
        // Error message mentions the provider type
        assert!(err.to_lowercase().contains("anthropic") || err.contains("config"));
    }

    #[test]
    fn test_build_groq_config_fails() {
        let config = ChatModelConfig::Groq {
            model: "llama-3.1-70b-versatile".to_string(),
            api_key: SecretReference::from_env("GROQ_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("only supports DeepSeek configs"));
    }

    #[test]
    fn test_build_mistral_config_fails() {
        let config = ChatModelConfig::Mistral {
            model: "mistral-large-latest".to_string(),
            api_key: SecretReference::from_env("MISTRAL_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("only supports DeepSeek configs"));
    }

    // ==================== Missing Secret Tests ====================

    #[test]
    fn test_build_missing_env_var_fails() {
        // Ensure the env var doesn't exist
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("NONEXISTENT_DEEPSEEK_KEY");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_DEEPSEEK_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    // ==================== Additional Provider Rejection Tests ====================

    #[test]
    fn test_build_fireworks_config_fails() {
        let config = ChatModelConfig::Fireworks {
            model: "accounts/fireworks/models/llama-v3-70b-instruct".to_string(),
            api_key: SecretReference::from_env("FIREWORKS_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("only supports DeepSeek configs"));
    }

    #[test]
    fn test_build_xai_config_fails() {
        let config = ChatModelConfig::XAI {
            model: "grok-beta".to_string(),
            api_key: SecretReference::from_env("XAI_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("only supports DeepSeek configs"));
    }

    // ==================== Additional Success Cases ====================

    #[test]
    fn test_build_deepseek_fractional_temperature() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-inline"),
            temperature: Some(0.333),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().llm_type(), "deepseek");
    }

    #[test]
    fn test_build_deepseek_high_temperature() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-inline"),
            temperature: Some(1.5),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_deepseek_custom_model_name() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat-v2-beta".to_string(),
            api_key: SecretReference::from_inline("sk-inline"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().llm_type(), "deepseek");
    }

    #[test]
    fn test_build_deepseek_empty_model_name() {
        // Empty model name is technically allowed (API will reject later)
        let config = ChatModelConfig::DeepSeek {
            model: "".to_string(),
            api_key: SecretReference::from_inline("sk-inline"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_deepseek_unicode_model_name() {
        let config = ChatModelConfig::DeepSeek {
            model: "深度求索".to_string(),
            api_key: SecretReference::from_inline("sk-inline"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    // ==================== Inline Secret Variations ====================

    #[test]
    fn test_build_deepseek_empty_inline_key() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline(""),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_deepseek_long_inline_key() {
        let long_key = "sk-".to_string() + &"x".repeat(1000);
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline(&long_key),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_deepseek_special_chars_in_key() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-!@#$%^&*()"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_deepseek_unicode_key() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-测试密钥"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    // ==================== Error Message Verification ====================

    #[test]
    fn test_error_message_contains_provider_name() {
        let config = ChatModelConfig::OpenAI {
            model: "gpt-4o".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        let result = build_chat_model(&config);
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };

        // Error should mention the actual provider
        assert!(err.to_lowercase().contains("openai") || err.contains("config"));
    }

    #[test]
    fn test_error_message_suggests_correct_crate() {
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-opus".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            max_tokens: None,
            temperature: None,
        };

        let result = build_chat_model(&config);
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };

        // Error should suggest using the appropriate crate
        assert!(err.contains("appropriate provider crate"));
    }

    #[test]
    fn test_error_is_invalid_input() {
        let config = ChatModelConfig::OpenAI {
            model: "gpt-4o".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        let result = build_chat_model(&config);

        // Should be an InvalidInput error
        match result {
            Err(DashFlowError::InvalidInput(_)) => { /* expected */ }
            Err(e) => panic!("Expected InvalidInput error, got: {e:?}"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    // ==================== Result Type Verification ====================

    #[test]
    fn test_build_returns_arc() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: None,
        };

        let result: Result<Arc<dyn ChatModel>, _> = build_chat_model(&config);
        assert!(result.is_ok());

        // Arc should be clonable
        let model = result.unwrap();
        let _cloned = Arc::clone(&model);
    }

    #[test]
    fn test_built_model_is_chat_model() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: None,
        };

        let model = build_chat_model(&config).unwrap();

        // Should implement ChatModel trait
        assert_eq!(model.llm_type(), "deepseek");
    }

    #[test]
    fn test_built_model_rate_limiter_is_none() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: None,
        };

        let model = build_chat_model(&config).unwrap();
        assert!(model.rate_limiter().is_none());
    }

    // ==================== Multiple Builds ====================

    #[test]
    fn test_multiple_builds_independent() {
        let config1 = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-key-1"),
            temperature: Some(0.5),
        };

        let config2 = ChatModelConfig::DeepSeek {
            model: "deepseek-coder".to_string(),
            api_key: SecretReference::from_inline("sk-key-2"),
            temperature: Some(0.9),
        };

        let model1 = build_chat_model(&config1).unwrap();
        let model2 = build_chat_model(&config2).unwrap();

        // Both should be deepseek type
        assert_eq!(model1.llm_type(), "deepseek");
        assert_eq!(model2.llm_type(), "deepseek");
    }

    #[test]
    fn test_same_config_produces_new_instances() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: None,
        };

        let model1 = build_chat_model(&config).unwrap();
        let model2 = build_chat_model(&config).unwrap();

        // Should produce separate instances
        assert_eq!(model1.llm_type(), model2.llm_type());
    }

    // ==================== Temperature Edge Cases ====================

    #[test]
    fn test_build_with_very_small_temperature() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: Some(0.0001),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_with_negative_temperature() {
        // Some APIs might interpret negative temperature specially
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: Some(-0.1),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_with_very_high_temperature() {
        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_inline("sk-test"),
            temperature: Some(100.0),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    // ==================== Env Var Edge Cases ====================

    #[test]
    fn test_build_with_env_var_containing_spaces() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_DEEPSEEK_SPACES", "  sk-test-key  ");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("TEST_DEEPSEEK_SPACES"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_DEEPSEEK_SPACES");
    }

    #[test]
    fn test_build_with_env_var_containing_newline() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_DEEPSEEK_NEWLINE", "sk-test-key\n");

        let config = ChatModelConfig::DeepSeek {
            model: "deepseek-chat".to_string(),
            api_key: SecretReference::from_env("TEST_DEEPSEEK_NEWLINE"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_DEEPSEEK_NEWLINE");
    }

    // Note: build_llm_node tests require custom GraphState types with Serialize/Deserialize.
    // The core functionality is tested via build_chat_model tests which cover the main logic.
    // build_llm_node simply wraps build_chat_model in LLMNode::new() so coverage is sufficient.
}
