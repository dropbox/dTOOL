//! Build ChatModel and LLMNode from ChatModelConfig
//!
//! This module provides functions to build `ChatAnthropic` and optimizable `LLMNode`
//! instances from configuration.
//!
//! ## Macros
//!
//! This module uses macros from `dashflow::core::config_loader::provider_helpers`
//! to reduce boilerplate:
//! - `impl_build_llm_node!` - Generates the `build_llm_node` function
//! - `wrong_provider_error!` - Generates consistent error messages

use crate::ChatAnthropic;
use dashflow::core::config_loader::ChatModelConfig;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::{impl_build_llm_node, wrong_provider_error};
use std::sync::Arc;

/// Build an Anthropic ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_anthropic::build_chat_model;
///
/// let config = ChatModelConfig::Anthropic {
///     model: "claude-3-5-sonnet-20241022".to_string(),
///     api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
///     temperature: Some(0.7),
///     max_tokens: Some(4096),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not an Anthropic config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::Anthropic {
            model,
            api_key,
            temperature,
            max_tokens,
        } => {
            let key = api_key.resolve()?;

            let mut llm = ChatAnthropic::try_new()?
                .with_api_key(&key)
                .with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }
            if let Some(max) = max_tokens {
                llm = llm.with_max_tokens(*max);
            }

            Ok(Arc::new(llm))
        }
        other => Err(wrong_provider_error!(
            "dashflow-anthropic",
            "Anthropic",
            other
        )),
    }
}

// Generate the build_llm_node function using the macro from dashflow core.
// This eliminates ~35 lines of boilerplate that was duplicated across all provider crates.
impl_build_llm_node!();

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::SecretReference;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_build_anthropic_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_ANTHROPIC_KEY", "sk-test-key");

        let config = ChatModelConfig::Anthropic {
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: SecretReference::from_env("TEST_ANTHROPIC_KEY"),
            temperature: Some(0.7),
            max_tokens: Some(4096),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "anthropic");

        std::env::remove_var("TEST_ANTHROPIC_KEY");
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
            Err(err) => assert!(err.to_string().contains("only supports Anthropic configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }
}
