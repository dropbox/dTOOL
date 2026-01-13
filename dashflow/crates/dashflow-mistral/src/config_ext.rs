//! Build ChatModel and LLMNode from ChatModelConfig
//!
//! This module provides functions to build `ChatMistralAI` and optimizable `LLMNode`
//! instances from configuration.

use crate::ChatMistralAI;
use dashflow::core::config_loader::ChatModelConfig;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build a Mistral ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_mistral::build_chat_model;
///
/// let config = ChatModelConfig::Mistral {
///     model: "mistral-small-latest".to_string(),
///     api_key: SecretReference::from_env("MISTRAL_API_KEY"),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a Mistral config
/// - Secret resolution fails (e.g., environment variable not set)
/// - Mistral client creation fails
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::Mistral {
            model,
            api_key,
            temperature,
        } => {
            let key = api_key.resolve()?;

            let mut llm = ChatMistralAI::with_api_key(&key)
                .map_err(|e| {
                    DashFlowError::InvalidInput(format!("Failed to create Mistral client: {}", e))
                })?
                .with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-mistral only supports Mistral configs, \
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
/// use dashflow_mistral::build_llm_node;
///
/// let config = ChatModelConfig::Mistral {
///     model: "mistral-small-latest".to_string(),
///     api_key: SecretReference::from_env("MISTRAL_API_KEY"),
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
/// - The config is not a Mistral config
/// - Secret resolution fails (e.g., environment variable not set)
/// - Mistral client creation fails
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

    #[test]
    fn test_build_mistral_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_MISTRAL_KEY", "test-key");

        let config = ChatModelConfig::Mistral {
            model: "mistral-small-latest".to_string(),
            api_key: SecretReference::from_env("TEST_MISTRAL_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        // llm_type() returns the model name for Mistral
        assert_eq!(model.llm_type(), "mistral-small-latest");

        std::env::remove_var("TEST_MISTRAL_KEY");
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
            Err(err) => assert!(err.to_string().contains("only supports Mistral configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }
}
