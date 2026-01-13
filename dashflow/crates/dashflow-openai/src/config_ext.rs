//! Build ChatModel, Embeddings, and LLMNode from config
//!
//! This module provides functions to build `ChatOpenAI`, `OpenAIEmbeddings`,
//! and optimizable `LLMNode` instances from configuration.

use crate::{ChatOpenAI, OpenAIConfig, OpenAIEmbeddings};
use dashflow::core::config_loader::{ChatModelConfig, EmbeddingConfig};
use dashflow::core::embeddings::Embeddings;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build an OpenAI ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_openai::build_chat_model;
///
/// let config = ChatModelConfig::OpenAI {
///     model: "gpt-4o".to_string(),
///     api_key: SecretReference::from_env("OPENAI_API_KEY"),
///     temperature: Some(0.7),
///     max_tokens: None,
///     base_url: None,
///     organization: None,
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not an OpenAI config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::OpenAI {
            model,
            api_key,
            temperature,
            max_tokens,
            base_url,
            organization,
        } => {
            let key = api_key.resolve()?;

            // Build OpenAI config with API key
            let mut oai_config = OpenAIConfig::new().with_api_key(&key);
            if let Some(url) = base_url {
                oai_config = oai_config.with_api_base(url);
            }
            if let Some(org) = organization {
                oai_config = oai_config.with_org_id(org);
            }

            let mut llm = ChatOpenAI::with_config(oai_config).with_model(model);
            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }
            if let Some(max) = max_tokens {
                llm = llm.with_max_tokens(*max);
            }
            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-openai only supports OpenAI configs, \
             got {} config. Use the appropriate provider crate for this config type.",
            other.provider()
        ))),
    }
}

/// Build OpenAI Embeddings from an EmbeddingConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{EmbeddingConfig, SecretReference};
/// use dashflow_openai::build_embeddings;
///
/// let config = EmbeddingConfig::OpenAI {
///     model: "text-embedding-3-small".to_string(),
///     api_key: SecretReference::from_env("OPENAI_API_KEY"),
///     batch_size: 512,
/// };
///
/// let embeddings = build_embeddings(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not an OpenAI config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_embeddings(config: &EmbeddingConfig) -> Result<Arc<dyn Embeddings>, DashFlowError> {
    match config {
        EmbeddingConfig::OpenAI {
            model,
            api_key,
            batch_size,
        } => {
            let key = api_key.resolve()?;

            #[allow(clippy::disallowed_methods)] // new() may read defaults from env
            let embeddings = OpenAIEmbeddings::new()
                .with_model(model)
                .with_api_key(&key)
                .with_chunk_size(*batch_size);

            Ok(Arc::new(embeddings))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_embeddings from dashflow-openai only supports OpenAI configs, \
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
/// use dashflow_openai::build_llm_node;
///
/// let config = ChatModelConfig::OpenAI {
///     model: "gpt-4o".to_string(),
///     api_key: SecretReference::from_env("OPENAI_API_KEY"),
///     temperature: Some(0.7),
///     max_tokens: None,
///     base_url: None,
///     organization: None,
/// };
///
/// let signature = make_signature("question -> answer", "Answer accurately")?;
/// let llm_node: LLMNode<MyState> = build_llm_node(&config, signature)?;
///
/// // The LLMNode can now be optimized with training data
/// ```
///
/// # Type Parameters
///
/// * `S` - The graph state type (must implement `GraphState`)
///
/// # Errors
///
/// Returns an error if:
/// - The config is not an OpenAI config
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
    fn test_build_openai_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Set up test environment
        std::env::set_var("TEST_OPENAI_KEY", "sk-test-key");

        let config = ChatModelConfig::OpenAI {
            model: "gpt-4o".to_string(),
            api_key: SecretReference::from_env("TEST_OPENAI_KEY"),
            temperature: Some(0.7),
            max_tokens: Some(1000),
            base_url: None,
            organization: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "openai-chat");

        std::env::remove_var("TEST_OPENAI_KEY");
    }

    #[test]
    fn test_build_wrong_provider_fails() {
        // No env var manipulation - doesn't need mutex
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-5-sonnet".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            temperature: None,
            max_tokens: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        // Use match instead of unwrap_err() since Arc<dyn ChatModel> doesn't impl Debug
        match result {
            Err(err) => assert!(err.to_string().contains("only supports OpenAI configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }
}
