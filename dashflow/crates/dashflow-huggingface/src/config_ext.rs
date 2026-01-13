//! Build ChatModel, Embeddings, and LLMNode from config
//!
//! This module provides functions to build `ChatHuggingFace`, `HuggingFaceEmbeddings`,
//! and optimizable `LLMNode` instances from configuration.

use crate::{ChatHuggingFace, HuggingFaceEmbeddings};
use dashflow::core::config_loader::{ChatModelConfig, EmbeddingConfig};
use dashflow::core::embeddings::Embeddings;
use dashflow::core::language_models::ChatModel;
use dashflow::core::Error as DashFlowError;
use dashflow::optimize::{LLMNode, Signature};
use dashflow::state::GraphState;
use std::sync::Arc;

/// Build a HuggingFace ChatModel from a ChatModelConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
/// use dashflow_huggingface::build_chat_model;
///
/// let config = ChatModelConfig::HuggingFace {
///     model: "meta-llama/Llama-2-7b-chat-hf".to_string(),
///     api_key: SecretReference::from_env("HF_TOKEN"),
///     temperature: Some(0.7),
/// };
///
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a HuggingFace config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
    match config {
        ChatModelConfig::HuggingFace {
            model,
            api_key,
            temperature,
        } => {
            let key = api_key.resolve()?;

            let mut llm = ChatHuggingFace::with_api_token(model, &key);

            if let Some(t) = temperature {
                llm = llm.with_temperature(f64::from(*t));
            }

            Ok(Arc::new(llm))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_chat_model from dashflow-huggingface only supports HuggingFace configs, \
             got {} config. Use the appropriate provider crate for this config type.",
            other.provider()
        ))),
    }
}

/// Build HuggingFace Embeddings from an EmbeddingConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{EmbeddingConfig, SecretReference};
/// use dashflow_huggingface::build_embeddings;
///
/// let config = EmbeddingConfig::HuggingFace {
///     model: "sentence-transformers/all-mpnet-base-v2".to_string(),
///     api_key: SecretReference::from_env("HF_TOKEN"),
/// };
///
/// let embeddings = build_embeddings(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a HuggingFace config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_embeddings(config: &EmbeddingConfig) -> Result<Arc<dyn Embeddings>, DashFlowError> {
    match config {
        EmbeddingConfig::HuggingFace { model, api_key } => {
            let key = api_key.resolve()?;

            let embeddings = HuggingFaceEmbeddings::new_without_token()
                .with_model(model)
                .with_api_token(&key);

            Ok(Arc::new(embeddings))
        }
        other => Err(DashFlowError::InvalidInput(format!(
            "build_embeddings from dashflow-huggingface only supports HuggingFace configs, \
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
/// use dashflow_huggingface::build_llm_node;
///
/// let config = ChatModelConfig::HuggingFace {
///     model: "meta-llama/Llama-2-7b-chat-hf".to_string(),
///     api_key: SecretReference::from_env("HF_TOKEN"),
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
/// - The config is not a HuggingFace config
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

    // ============================================
    // build_chat_model tests
    // ============================================

    #[test]
    fn test_build_huggingface_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_HF_TOKEN", "hf-test-token");

        let config = ChatModelConfig::HuggingFace {
            model: "meta-llama/Llama-2-7b-chat-hf".to_string(),
            api_key: SecretReference::from_env("TEST_HF_TOKEN"),
            temperature: Some(0.7),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        // Verify the model was configured correctly
        let model = result.unwrap();
        // llm_type() returns the model_id for HuggingFace
        assert_eq!(model.llm_type(), "meta-llama/Llama-2-7b-chat-hf");

        std::env::remove_var("TEST_HF_TOKEN");
    }

    #[test]
    fn test_build_chat_model_without_temperature() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_HF_TOKEN_2", "hf-test-token-2");

        let config = ChatModelConfig::HuggingFace {
            model: "test-model".to_string(),
            api_key: SecretReference::from_env("TEST_HF_TOKEN_2"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_HF_TOKEN_2");
    }

    #[test]
    fn test_build_chat_model_various_models() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_HF_TOKEN_3", "hf-test-token-3");

        let models = [
            "mistralai/Mistral-7B-Instruct-v0.2",
            "HuggingFaceH4/zephyr-7b-beta",
            "tiiuae/falcon-7b-instruct",
        ];

        for model_name in models {
            let config = ChatModelConfig::HuggingFace {
                model: model_name.to_string(),
                api_key: SecretReference::from_env("TEST_HF_TOKEN_3"),
                temperature: None,
            };

            let result = build_chat_model(&config);
            assert!(result.is_ok(), "Failed for model: {model_name}");
            assert_eq!(result.unwrap().llm_type(), model_name);
        }

        std::env::remove_var("TEST_HF_TOKEN_3");
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
            Err(err) => assert!(err
                .to_string()
                .contains("only supports HuggingFace configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }

    #[test]
    fn test_build_chat_model_missing_env_var() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Ensure the env var doesn't exist
        std::env::remove_var("NONEXISTENT_HF_TOKEN");

        let config = ChatModelConfig::HuggingFace {
            model: "test-model".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_HF_TOKEN"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_chat_model_temperature_edge_cases() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_HF_TOKEN_TEMP", "hf-test-token");

        // Test with temperature 0.0
        let config1 = ChatModelConfig::HuggingFace {
            model: "test-model".to_string(),
            api_key: SecretReference::from_env("TEST_HF_TOKEN_TEMP"),
            temperature: Some(0.0),
        };
        assert!(build_chat_model(&config1).is_ok());

        // Test with temperature 2.0
        let config2 = ChatModelConfig::HuggingFace {
            model: "test-model".to_string(),
            api_key: SecretReference::from_env("TEST_HF_TOKEN_TEMP"),
            temperature: Some(2.0),
        };
        assert!(build_chat_model(&config2).is_ok());

        std::env::remove_var("TEST_HF_TOKEN_TEMP");
    }

    // ============================================
    // build_embeddings tests
    // ============================================

    #[test]
    fn test_build_embeddings_success() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_HF_EMB_TOKEN", "hf-test-emb-token");

        let config = EmbeddingConfig::HuggingFace {
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            api_key: SecretReference::from_env("TEST_HF_EMB_TOKEN"),
        };

        let result = build_embeddings(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_HF_EMB_TOKEN");
    }

    #[test]
    fn test_build_embeddings_various_models() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_HF_EMB_TOKEN_2", "hf-test-emb-token-2");

        let models = [
            "sentence-transformers/all-mpnet-base-v2",
            "BAAI/bge-large-en-v1.5",
            "thenlper/gte-large",
        ];

        for model_name in models {
            let config = EmbeddingConfig::HuggingFace {
                model: model_name.to_string(),
                api_key: SecretReference::from_env("TEST_HF_EMB_TOKEN_2"),
            };

            let result = build_embeddings(&config);
            assert!(result.is_ok(), "Failed for embedding model: {model_name}");
        }

        std::env::remove_var("TEST_HF_EMB_TOKEN_2");
    }

    #[test]
    fn test_build_embeddings_wrong_provider() {
        let config = EmbeddingConfig::OpenAI {
            model: "text-embedding-ada-002".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            batch_size: 100,
        };

        let result = build_embeddings(&config);
        assert!(result.is_err());

        match result {
            Err(err) => assert!(err
                .to_string()
                .contains("only supports HuggingFace configs")),
            Ok(_) => panic!("Expected error for wrong provider"),
        }
    }

    #[test]
    fn test_build_embeddings_missing_env_var() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("NONEXISTENT_EMB_TOKEN");

        let config = EmbeddingConfig::HuggingFace {
            model: "test-model".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_EMB_TOKEN"),
        };

        let result = build_embeddings(&config);
        assert!(result.is_err());
    }

    // ============================================
    // Error message tests
    // ============================================

    #[test]
    fn test_error_message_contains_huggingface() {
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-opus-20240229".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            temperature: None,
            max_tokens: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());

        // Check the error message without using unwrap_err (which requires Debug)
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(
                err_msg.contains("Anthropic") || err_msg.contains("HuggingFace"),
                "Error message should mention provider: {err_msg}"
            );
        }
    }

    #[test]
    fn test_embeddings_error_message_contains_openai() {
        let config = EmbeddingConfig::OpenAI {
            model: "text-embedding-ada-002".to_string(),
            api_key: SecretReference::from_env("OPENAI_API_KEY"),
            batch_size: 100,
        };

        let result = build_embeddings(&config);
        assert!(result.is_err());

        // Check the error message without using unwrap_err (which requires Debug)
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(
                err_msg.contains("OpenAI") || err_msg.contains("HuggingFace"),
                "Error message should mention provider: {err_msg}"
            );
        }
    }

    // ============================================
    // Inline secret tests
    // ============================================

    #[test]
    fn test_build_chat_model_with_inline_secret() {
        let config = ChatModelConfig::HuggingFace {
            model: "test-model".to_string(),
            api_key: SecretReference::from_inline("hf_inline_token"),
            temperature: Some(0.5),
        };

        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_embeddings_with_inline_secret() {
        let config = EmbeddingConfig::HuggingFace {
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            api_key: SecretReference::from_inline("hf_inline_emb_token"),
        };

        let result = build_embeddings(&config);
        assert!(result.is_ok());
    }

    // ============================================
    // Provider error coverage tests
    // ============================================

    #[test]
    fn test_build_chat_model_mistral_provider_fails() {
        let config = ChatModelConfig::Mistral {
            model: "mistral-large-latest".to_string(),
            api_key: SecretReference::from_env("MISTRAL_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_chat_model_groq_provider_fails() {
        let config = ChatModelConfig::Groq {
            model: "llama2-70b-4096".to_string(),
            api_key: SecretReference::from_env("GROQ_API_KEY"),
            temperature: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_chat_model_anthropic_provider_fails() {
        let config = ChatModelConfig::Anthropic {
            model: "claude-3-sonnet-20240229".to_string(),
            api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
            temperature: None,
            max_tokens: None,
        };

        let result = build_chat_model(&config);
        assert!(result.is_err());
    }

    // ============================================
    // Empty model name tests
    // ============================================

    #[test]
    fn test_build_chat_model_empty_model_name() {
        let config = ChatModelConfig::HuggingFace {
            model: "".to_string(),
            api_key: SecretReference::from_inline("hf_test_token"),
            temperature: None,
        };

        // Empty model name should still create a model (API will reject it)
        let result = build_chat_model(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_embeddings_empty_model_name() {
        let config = EmbeddingConfig::HuggingFace {
            model: "".to_string(),
            api_key: SecretReference::from_inline("hf_test_token"),
        };

        // Empty model name should still create embeddings (API will reject it)
        let result = build_embeddings(&config);
        assert!(result.is_ok());
    }
}
