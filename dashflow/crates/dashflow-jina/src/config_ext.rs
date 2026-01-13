//! Build DocumentCompressor from config
//!
//! This module provides functions to build `JinaRerank` instances from configuration.

use crate::rerank::JinaRerank;
use dashflow::core::config_loader::RerankerConfig;
use dashflow::core::documents::DocumentCompressor;
use dashflow::core::Error as DashFlowError;
use std::sync::Arc;

/// Build a Jina Reranker from a RerankerConfig
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::{RerankerConfig, SecretReference};
/// use dashflow_jina::build_reranker;
///
/// let config = RerankerConfig::Jina {
///     model: "jina-reranker-v1-base-en".to_string(),
///     api_key: SecretReference::from_env("JINA_API_KEY"),
///     top_n: Some(3),
/// };
///
/// let reranker = build_reranker(&config)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The config is not a Jina config
/// - Secret resolution fails (e.g., environment variable not set)
pub fn build_reranker(
    config: &RerankerConfig,
) -> Result<Arc<dyn DocumentCompressor>, DashFlowError> {
    match config {
        RerankerConfig::Jina {
            model,
            api_key,
            top_n,
        } => {
            let key = api_key.resolve()?;

            let reranker = JinaRerank::builder()
                .api_key(key)
                .model(model.clone())
                .top_n(*top_n)
                .build()
                .map_err(|e| {
                    DashFlowError::InvalidInput(format!("Failed to build Jina reranker: {e}"))
                })?;

            Ok(Arc::new(reranker))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::SecretReference;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // ==================== Basic Builder Tests ====================

    #[test]
    fn test_build_reranker_jina() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Set up test API key
        std::env::set_var("TEST_JINA_API_KEY", "test-key-12345");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-turbo-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_API_KEY"),
            top_n: Some(5),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        // Clean up
        std::env::remove_var("TEST_JINA_API_KEY");
    }

    #[test]
    fn test_build_reranker_missing_api_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Make sure the env var doesn't exist
        std::env::remove_var("NONEXISTENT_JINA_KEY");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("NONEXISTENT_JINA_KEY"),
            top_n: None,
        };

        let result = build_reranker(&config);
        assert!(result.is_err());
    }

    // ==================== Model Variants Tests ====================

    #[test]
    fn test_build_reranker_base_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_BASE", "base-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_BASE"),
            top_n: Some(3),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_BASE");
    }

    #[test]
    fn test_build_reranker_turbo_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_TURBO", "turbo-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-turbo-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_TURBO"),
            top_n: Some(10),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_TURBO");
    }

    #[test]
    fn test_build_reranker_tiny_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_TINY", "tiny-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-tiny-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_TINY"),
            top_n: Some(1),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_TINY");
    }

    // ==================== Top N Variations ====================

    #[test]
    fn test_build_reranker_top_n_none() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_NONE", "none-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_NONE"),
            top_n: None,
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_NONE");
    }

    #[test]
    fn test_build_reranker_top_n_one() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_ONE", "one-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_ONE"),
            top_n: Some(1),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_ONE");
    }

    #[test]
    fn test_build_reranker_top_n_large() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_LARGE", "large-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_LARGE"),
            top_n: Some(100),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_LARGE");
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_build_reranker_error_message_contains_context() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("MISSING_ENV_KEY");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("MISSING_ENV_KEY"),
            top_n: Some(3),
        };

        let result = build_reranker(&config);
        assert!(result.is_err());
        // Verify error is related to secret resolution
        if let Err(err) = result {
            let err_str = err.to_string();
            // The error should indicate the missing env var
            assert!(err_str.contains("MISSING_ENV_KEY") || err_str.contains("environment"));
        }
    }

    // ==================== Return Type Tests ====================

    #[test]
    fn test_build_reranker_returns_arc() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_ARC", "arc-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_ARC"),
            top_n: Some(3),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());
        // Verify it's an Arc<dyn DocumentCompressor>
        let compressor = result.unwrap();
        // Arc has strong_count method - verify we can access it
        assert!(Arc::strong_count(&compressor) >= 1);

        std::env::remove_var("TEST_JINA_KEY_ARC");
    }

    // ==================== SecretReference Tests ====================

    #[test]
    fn test_build_reranker_with_direct_secret() {
        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_inline("direct-api-key"),
            top_n: Some(5),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_reranker_with_different_env_vars() {
        let _guard = ENV_MUTEX.lock().unwrap();

        // Test with multiple different env var names
        let test_cases = [
            ("JINA_API_KEY_TEST1", "key1"),
            ("JINA_API_KEY_TEST2", "key2"),
            ("MY_CUSTOM_JINA_KEY", "key3"),
        ];

        for (env_var, key_value) in test_cases {
            std::env::set_var(env_var, key_value);

            let config = RerankerConfig::Jina {
                model: "jina-reranker-v1-base-en".to_string(),
                api_key: SecretReference::from_env(env_var),
                top_n: Some(3),
            };

            let result = build_reranker(&config);
            assert!(result.is_ok(), "Failed for env var: {}", env_var);

            std::env::remove_var(env_var);
        }
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_build_reranker_empty_model_name() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_EMPTY", "empty-key");

        let config = RerankerConfig::Jina {
            model: String::new(), // Empty model name
            api_key: SecretReference::from_env("TEST_JINA_KEY_EMPTY"),
            top_n: Some(3),
        };

        // Should still build (model validation is server-side)
        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_EMPTY");
    }

    #[test]
    fn test_build_reranker_custom_model_name() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_CUSTOM", "custom-key");

        let config = RerankerConfig::Jina {
            model: "custom-model-name-v99".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_CUSTOM"),
            top_n: Some(3),
        };

        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_CUSTOM");
    }

    #[test]
    fn test_build_reranker_top_n_zero() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("TEST_JINA_KEY_ZERO", "zero-key");

        let config = RerankerConfig::Jina {
            model: "jina-reranker-v1-base-en".to_string(),
            api_key: SecretReference::from_env("TEST_JINA_KEY_ZERO"),
            top_n: Some(0),
        };

        // Should build (validation is server-side or runtime)
        let result = build_reranker(&config);
        assert!(result.is_ok());

        std::env::remove_var("TEST_JINA_KEY_ZERO");
    }
}
