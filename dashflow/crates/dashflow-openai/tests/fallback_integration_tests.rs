//! Integration tests for fallback functionality with real LLM providers
//!
//! These tests verify that fallback chains work correctly with real API calls.
//!
//! Prerequisites:
//! - OPENAI_API_KEY environment variable must be set
//! - ANTHROPIC_API_KEY environment variable may be set for cross-provider tests
//!
//! Run with: cargo test --test fallback_integration_tests -p dashflow-openai -- --ignored

#![allow(clippy::expect_used, clippy::unwrap_used)]

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::config::RunnableConfig;
use dashflow::core::error::Result;
use dashflow::core::language_models::{ChatModel, ChatResult};
use dashflow::core::messages::{BaseMessage, Message};
use dashflow::core::runnable::{Runnable, RunnableWithFallbacks};
use dashflow_anthropic::build_chat_model as build_anthropic_chat_model;
use dashflow_openai::build_chat_model as build_openai_chat_model;
use std::sync::{Arc, Mutex};

// Mutex to serialize env var access across tests.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Wrapper to make ChatModel work as a Runnable for fallback testing
struct ChatModelRunnable {
    model: Arc<dyn ChatModel>,
}

impl ChatModelRunnable {
    fn new(model: Arc<dyn ChatModel>) -> Self {
        Self { model }
    }
}

#[async_trait::async_trait]
impl Runnable for ChatModelRunnable {
    type Input = Vec<BaseMessage>;
    type Output = ChatResult;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.model.generate(&input, None, None, None, None).await
    }
}

/// Helper to check if OpenAI API key is available
fn has_openai_key() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

/// Helper to check if Anthropic API key is available
fn has_anthropic_key() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_ok()
}

fn openai_model(model: &str, temperature: f32) -> Arc<dyn ChatModel> {
    let config = ChatModelConfig::OpenAI {
        model: model.to_string(),
        api_key: SecretReference::from_env("OPENAI_API_KEY"),
        temperature: Some(temperature),
        max_tokens: None,
        base_url: None,
        organization: None,
    };
    build_openai_chat_model(&config).expect("Failed to build OpenAI chat model")
}

fn anthropic_model(model: &str, temperature: f32) -> Arc<dyn ChatModel> {
    let config = ChatModelConfig::Anthropic {
        model: model.to_string(),
        api_key: SecretReference::from_env("ANTHROPIC_API_KEY"),
        temperature: Some(temperature),
        max_tokens: None,
    };
    build_anthropic_chat_model(&config).expect("Failed to build Anthropic chat model")
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_fallback_primary_succeeds_no_fallback_invoked() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create primary and fallback models
    let primary = ChatModelRunnable::new(openai_model("gpt-4o-mini", 0.0));

    let fallback = ChatModelRunnable::new(openai_model("gpt-3.5-turbo", 0.0));

    // Create fallback chain
    let chain = RunnableWithFallbacks::new(primary).add_fallback(fallback);

    let messages = vec![Message::human(
        "What is 2 + 2? Answer with just the number.",
    )];

    // Primary should succeed, fallback not invoked
    let result = chain
        .invoke(messages, None)
        .await
        .expect("Failed to generate response");

    let response_text = result.generations[0].message.content().as_text();
    assert!(
        response_text.contains("4"),
        "Expected response to contain '4', got: {}",
        response_text
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_fallback_with_invalid_model_uses_fallback() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create primary with invalid model (will fail)
    let primary = ChatModelRunnable::new(openai_model("gpt-nonexistent-model", 0.0));

    // Create fallback with valid model
    let fallback = ChatModelRunnable::new(openai_model("gpt-4o-mini", 0.0));

    // Create fallback chain
    let chain = RunnableWithFallbacks::new(primary).add_fallback(fallback);

    let messages = vec![Message::human("Say 'hello' in one word.")];

    // Primary should fail (invalid model), fallback should succeed
    let result = chain
        .invoke(messages, None)
        .await
        .expect("Failed to generate response with fallback");

    let response_text = result.generations[0]
        .message
        .content()
        .as_text()
        .to_lowercase();
    assert!(
        response_text.contains("hello"),
        "Expected response to contain 'hello', got: {}",
        response_text
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_fallback_multiple_fallbacks_in_sequence() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create primary with invalid model (will fail)
    let primary = ChatModelRunnable::new(openai_model("gpt-invalid-1", 0.0));

    // Create first fallback with invalid model (will also fail)
    let fallback1 = ChatModelRunnable::new(openai_model("gpt-invalid-2", 0.0));

    // Create second fallback with valid model (should succeed)
    let fallback2 = ChatModelRunnable::new(openai_model("gpt-4o-mini", 0.0));

    // Create fallback chain
    let chain = RunnableWithFallbacks::new(primary)
        .add_fallback(fallback1)
        .add_fallback(fallback2);

    let messages = vec![Message::human(
        "What is the capital of France? One word answer.",
    )];

    // First two should fail, third should succeed
    let result = chain
        .invoke(messages, None)
        .await
        .expect("Failed to generate response with multiple fallbacks");

    let response_text = result.generations[0]
        .message
        .content()
        .as_text()
        .to_lowercase();
    assert!(
        response_text.contains("paris"),
        "Expected response to contain 'paris', got: {}",
        response_text
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_fallback_all_fail_returns_error() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create primary with invalid model
    let primary = ChatModelRunnable::new(openai_model("gpt-invalid-primary", 0.0));

    // Create fallback with invalid model
    let fallback = ChatModelRunnable::new(openai_model("gpt-invalid-fallback", 0.0));

    // Create fallback chain
    let chain = RunnableWithFallbacks::new(primary).add_fallback(fallback);

    let messages = vec![Message::human("Test message")];

    // Both should fail, should return error
    let result = chain.invoke(messages, None).await;

    assert!(
        result.is_err(),
        "Expected error when all fallbacks fail, but got success"
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY and ANTHROPIC_API_KEY"]
async fn test_fallback_cross_provider_openai_to_anthropic() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");
    assert!(has_anthropic_key(), "ANTHROPIC_API_KEY must be set");

    // Create primary with invalid OpenAI model (will fail)
    let primary = ChatModelRunnable::new(openai_model("gpt-nonexistent", 0.0));

    // Create fallback with Anthropic
    let fallback =
        ChatModelRunnable::new(anthropic_model("claude-3-5-sonnet-20241022", 0.0));

    // Create fallback chain (cross-provider)
    let chain = RunnableWithFallbacks::new(primary).add_fallback(fallback);

    let messages = vec![Message::human(
        "What is the largest planet in our solar system? One word answer.",
    )];

    // OpenAI should fail, Anthropic should succeed
    let result = chain
        .invoke(messages, None)
        .await
        .expect("Failed to generate response with cross-provider fallback");

    let response_text = result.generations[0]
        .message
        .content()
        .as_text()
        .to_lowercase();
    assert!(
        response_text.contains("jupiter"),
        "Expected response to contain 'jupiter', got: {}",
        response_text
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_fallback_with_invalid_api_key_fails_to_fallback() {
    // This test verifies that authentication errors can trigger fallbacks
    // when the fallback has valid credentials

    let (_original_key, primary, fallback) = {
        let _guard = ENV_MUTEX.lock().unwrap();

        // Save original key if it exists
        let original_key = std::env::var("OPENAI_API_KEY").ok();

        // Set an invalid API key for primary
        std::env::set_var("OPENAI_API_KEY", "sk-invalid-key-for-testing");

        let primary = ChatModelRunnable::new(openai_model("gpt-4o-mini", 0.0));

        // Restore key for fallback
        if let Some(ref key) = original_key {
            std::env::set_var("OPENAI_API_KEY", key);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }

        let fallback = ChatModelRunnable::new(openai_model("gpt-4o-mini", 0.0));

        // Restore original key before releasing the lock.
        if let Some(ref key) = original_key {
            std::env::set_var("OPENAI_API_KEY", key);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }

        (original_key, primary, fallback)
    };

    // Create fallback chain
    let chain = RunnableWithFallbacks::new(primary).add_fallback(fallback);

    let messages = vec![Message::human("Test message")];

    let start = std::time::Instant::now();
    let result = chain.invoke(messages, None).await;
    let elapsed = start.elapsed();

    // Should succeed via fallback
    assert!(
        result.is_ok(),
        "Expected success via fallback, but got error: {:?}",
        result
    );

    // Should be reasonably fast (fallback after first failure)
    assert!(
        elapsed.as_secs() < 10,
        "Expected fast fallback, but took {} seconds",
        elapsed.as_secs()
    );
}
