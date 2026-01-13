//! Build ChatModel and LLMNode from ChatModelConfig
//!
//! This module provides functions to build `ChatGroq` and optimizable `LLMNode`
//! instances from configuration.

use crate::{ChatGroq, GROQ_API_BASE};
use async_openai::config::OpenAIConfig;
use dashflow::core::config_loader::ChatModelConfig;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build a Groq ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_groq::build_chat_model;
///
/// let config = ChatModelConfig::Groq {
///     model: "llama-3.1-8b-instant".to_string(),
///     api_key: SecretReference::from_env("GROQ_API_KEY"),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a Groq config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::Groq {
            model,
            api_key,
            temperature,
        } => {
            let key = api_key.resolve()?;

            let openai_config = OpenAIConfig::new()
                .with_api_key(&key)
                .with_api_base(GROQ_API_BASE);

            let mut llm = ChatGroq::with_config(openai_config).with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-groq only supports Groq configs, \
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
/// use dashflow_groq::build_llm_node;
///
/// let config = ChatModelConfig::Groq {
///     model: "llama-3.1-8b-instant".to_string(),
///     api_key: SecretReference::from_env("GROQ_API_KEY"),
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
/// - The config is not a Groq config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_llm_node<S: GraphState>(
    config: &ChatModelConfig,
    signature: Signature,
) -> Result<LLMNode<S>, DashFlowError> {
    let llm = build_chat_model(config)?;
    Ok(LLMNode::new(signature, llm))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::SecretReference;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_build_groq_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_GROQ_KEY", "gsk-test-key");

        let config = ChatModelConfig::Groq {
            model: "llama-3.1-8b-instant".to_string(),
            api_key: SecretReference::from_env("TEST_GROQ_KEY"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "groq-chat");

        std::env::remove_var("TEST_GROQ_KEY");
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

        let err = build_chat_model(&config)
            .err()
            .expect("Expected error for wrong provider");
        assert!(err.to_string().contains("only supports Groq configs"));
    }
}
