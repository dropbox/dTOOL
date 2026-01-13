//! Integration tests for retry functionality with OpenAI provider
//!
//! These tests verify that retry logic works correctly with real API calls.
//!
//! Prerequisites:
//! - OPENAI_API_KEY environment variable must be set
//!
//! Run with: cargo test --test retry_integration_tests -p dashflow-openai -- --ignored

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::await_holding_lock)]

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow::core::retry::RetryPolicy;
use dashflow_openai::ChatOpenAI;
use std::sync::Mutex;

// Mutex to serialize env var access across tests.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Helper to check if OpenAI API key is available
fn has_openai_key() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_retry_with_successful_call() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create a ChatOpenAI instance with retry policy
    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0)
        .with_retry_policy(RetryPolicy::exponential(3));

    let messages = vec![Message::human(
        "What is 2 + 2? Answer with just the number.",
    )];

    // This should succeed on the first try (no retries needed)
    let result = model
        .generate(&messages, None, None, None, None)
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
async fn test_retry_with_custom_policy() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0)
        .with_retry_policy(RetryPolicy::exponential_with_params(5, 500, 5000));

    let messages = vec![Message::human("Say 'hello' in one word.")];

    let result = model
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

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
async fn test_retry_with_invalid_api_key_fails_fast() {
    // This test verifies that non-retryable errors (like invalid API key)
    // don't trigger retries and fail immediately

    let _guard = ENV_MUTEX.lock().unwrap();

    // Save original key if it exists
    let original_key = std::env::var("OPENAI_API_KEY").ok();

    // Set an invalid API key
    std::env::set_var("OPENAI_API_KEY", "sk-invalid-key-for-testing");

    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0)
        .with_retry_policy(RetryPolicy::exponential(3));

    let messages = vec![Message::human("Test message")];

    let start = std::time::Instant::now();
    let result = model.generate(&messages, None, None, None, None).await;
    let elapsed = start.elapsed();

    // Restore original key
    if let Some(key) = original_key {
        std::env::set_var("OPENAI_API_KEY", key);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    // Should fail immediately (non-retryable error)
    assert!(result.is_err(), "Expected error with invalid API key");

    // Should fail quickly (< 2 seconds) because it's not retrying
    assert!(
        elapsed.as_secs() < 2,
        "Expected fast failure, but took {} seconds",
        elapsed.as_secs()
    );
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_retry_policy_exponential_jitter() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0)
        .with_retry_policy(RetryPolicy::exponential_jitter(3, 200, 2000, 2.0, 100));

    let messages = vec![Message::human(
        "What is the capital of France? One word answer.",
    )];

    let result = model
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

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
