//! Build ChatModel, Embeddings, and LLMNode from config
//!
//! This module provides functions to build `ChatOllama`, `OllamaEmbeddings`,
//! and optimizable `LLMNode` instances from configuration.

use crate::{ChatOllama, OllamaEmbeddings};
use dashflow::core::config_loader::{ChatModelConfig, EmbeddingConfig};
use dashflow::core::embeddings::Embeddings;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build an Ollama ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::ChatModelConfig;
/// use dashflow_ollama::build_chat_model;
///
/// let config = ChatModelConfig::Ollama {
///     model: "llama3.2".to_string(),
///     base_url: "http://localhost:11434".to_string(),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if the config is not an Ollama config
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::Ollama {
            model,
            base_url,
            temperature,
        } => {
            let mut llm = ChatOllama::with_base_url(base_url).with_model(model);

            if let Some(t) = temperature {
                llm = llm.with_temperature(*t);
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-ollama only supports Ollama configs, \
             got {} config. Use the appropriate provider crate for this config type.",
            other.provider()
        ))),
    }
}

/// Build Ollama Embeddings from an EmbeddingConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::EmbeddingConfig;
/// use dashflow_ollama::build_embeddings;
///
/// let config = EmbeddingConfig::Ollama {
///     model: "nomic-embed-text".to_string(),
///     base_url: "http://localhost:11434".to_string(),
/// };
///
/// let embeddings = build_embeddings(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if the config is not an Ollama config
pub fn build_embeddings(config: &EmbeddingConfig) -> Result<Arc<dyn Embeddings>, DashFlowError> {
    match config {
        EmbeddingConfig::Ollama { model, base_url } => {
            let embeddings = OllamaEmbeddings::with_base_url(base_url).with_model(model);
            Ok(Arc::new(embeddings))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_embeddings from dashflow-ollama only supports Ollama configs, \
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
/// use dashflow::core::config_loader::ChatModelConfig;
/// use dashflow::optimize::{make_signature, Signature};
/// use dashflow_ollama::build_llm_node;
///
/// let config = ChatModelConfig::Ollama {
///     model: "llama3.2".to_string(),
///     base_url: "http://localhost:11434".to_string(),
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
/// Returns an error if the config is not an Ollama config
pub fn build_llm_node<S: GraphState>(
    config: &ChatModelConfig,
    signature: Signature,
) -> Result<LLMNode<S>, DashFlowError> {
    let llm = build_chat_model(config)?;
    Ok(LLMNode::new(signature, llm))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::SecretReference;
    use dashflow::optimize::make_signature;
    use serde::{Deserialize, Serialize};

    #[test]
    fn test_build_ollama_config() {
        let config = ChatModelConfig::Ollama {
            model: "llama3.2".to_string(),
            base_url: "http://localhost:11434".to_string(),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        assert_eq!(model.llm_type(), "ollama");
    }

    #[test]
    fn test_build_ollama_config_applies_temperature() {
        let config = ChatModelConfig::Ollama {
            model: "llama3.2".to_string(),
            base_url: "http://localhost:11434".to_string(),
            temperature: Some(0.7),
        };

        let model = build_chat_model(&config).unwrap();
        let params = model.identifying_params();
        assert_eq!(params.get("model").unwrap(), &serde_json::json!("llama3.2"));
        let temp = params.get("temperature").unwrap().as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_build_wrong_provider_fails() {
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
            Err(err) => assert!(err.to_string().contains("only supports Ollama configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }

    #[tokio::test]
    async fn test_build_embeddings_ollama_config_smoke() {
        let config = EmbeddingConfig::Ollama {
            model: "nomic-embed-text".to_string(),
            base_url: "http://localhost:11434".to_string(),
        };

        let embeddings = build_embeddings(&config).unwrap();
        let empty: Vec<String> = Vec::new();
        let vectors = embeddings._embed_documents(&empty).await.unwrap();
        assert_eq!(vectors.len(), 0);
    }

    #[test]
    fn test_build_embeddings_wrong_provider_fails() {
        let config = EmbeddingConfig::OpenAI {
            model: "text-embedding-3-small".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            batch_size: 32,
        };

        let result = build_embeddings(&config);
        assert!(result.is_err());
        match result {
            Ok(_) => panic!("Expected error for wrong provider"),
            Err(err) => assert!(err.to_string().contains("only supports Ollama configs")),
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestState {
        question: String,
    }

    #[test]
    fn test_build_llm_node_includes_signature_and_llm() {
        let config = ChatModelConfig::Ollama {
            model: "llama3.2".to_string(),
            base_url: "http://localhost:11434".to_string(),
            temperature: Some(0.7),
        };

        let signature = make_signature("question -> answer", "Answer accurately").unwrap();
        let node: LLMNode<TestState> = build_llm_node(&config, signature.clone()).unwrap();

        assert_eq!(node.signature.name, signature.name);
        assert_eq!(node.llm.llm_type(), "ollama");
    }

    #[test]
    fn test_build_llm_node_wrong_provider_fails() {
        let config = ChatModelConfig::OpenAI {
            model: "gpt-4o".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            temperature: None,
            max_tokens: None,
            base_url: None,
            organization: None,
        };

        let signature = make_signature("question -> answer", "Answer accurately").unwrap();
        let result: Result<LLMNode<TestState>, _> = build_llm_node(&config, signature);
        assert!(result.is_err());
    }
}
