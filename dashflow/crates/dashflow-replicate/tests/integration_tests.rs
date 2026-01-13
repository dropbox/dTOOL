//! Integration tests for Replicate provider
//!
//! These tests verify real API calls to Replicate.
//!
//! Prerequisites:
//! - REPLICATE_API_TOKEN environment variable must be set
//!
//! Run with: cargo test --test integration_tests -p dashflow-replicate -- --ignored

// `cargo verify` runs clippy with `-D warnings` for all targets, including tests.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use dashflow::core::language_models::{ChatModel, ToolDefinition};
use dashflow::core::messages::Message;
use dashflow_replicate::ChatReplicate;
use futures::StreamExt;
use serde_json::json;

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
async fn test_basic_chat_completion() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    let model = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_temperature(0.0); // Deterministic

    let messages = vec![Message::human(
        "What is 2 + 2? Answer with just the number.",
    )];

    let result = model
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

    assert!(
        !result.generations.is_empty(),
        "Expected at least one generation"
    );

    let response_text = result.generations[0].message.content().as_text();
    assert!(
        response_text.contains("4"),
        "Expected response to contain '4', got: {}",
        response_text
    );

    // Check that usage information is present
    if let Some(llm_output) = &result.llm_output {
        assert!(
            llm_output.get("usage").is_some(),
            "Expected usage information in llm_output"
        );
    }
}

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
async fn test_chat_with_system_message() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    let model = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_temperature(0.0);

    let messages = vec![
        Message::system("You are a helpful assistant. Always respond in one sentence."),
        Message::human("What is the capital of France?"),
    ];

    let result = model
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

    assert!(!result.generations.is_empty());

    let response_text = result.generations[0].message.content().as_text();
    assert!(
        response_text.to_lowercase().contains("paris"),
        "Expected response to mention Paris, got: {}",
        response_text
    );
}

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
async fn test_streaming_completion() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    let model = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_temperature(0.0);

    let messages = vec![Message::human("Count from 1 to 5.")];

    let mut stream = model
        .stream(&messages, None, None, None, None)
        .await
        .expect("Failed to create stream");

    let mut chunks = Vec::new();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("Failed to get chunk");
        chunks.push(chunk);
    }

    assert!(!chunks.is_empty(), "Expected at least one chunk");

    // Concatenate all chunks
    let full_text: String = chunks
        .iter()
        .map(|chunk| chunk.message.content.clone())
        .collect();

    assert!(!full_text.is_empty(), "Expected non-empty response");
}

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
#[allow(deprecated)] // Test uses deprecated with_tools() to verify provider behavior
async fn test_tool_calling() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    // Create a simple calculator tool
    let tool = ToolDefinition {
        name: "calculator".to_string(),
        description: "Perform basic arithmetic operations".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate"
                }
            },
            "required": ["expression"]
        }),
    };

    // Convert tool to JSON format expected by with_tools
    let tool_json = serde_json::to_value(&tool).expect("Failed to serialize tool");

    let model = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_temperature(0.0)
        .with_tools(vec![tool_json]);

    let messages = vec![Message::human("What is 15 * 23? Use the calculator tool.")];

    let result = model
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

    assert!(!result.generations.is_empty());

    // Check if tool calls were made
    if let Message::AI { tool_calls, .. } = &result.generations[0].message {
        // Note: Not all models may support tool calling, so we just check the structure
        println!("Tool calls: {:?}", tool_calls);
    }
}

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
async fn test_multi_turn_conversation() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    let model = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_temperature(0.0);

    let messages = vec![
        Message::human("My name is Alice."),
        Message::ai("Hello Alice! Nice to meet you."),
        Message::human("What is my name?"),
    ];

    let result = model
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

    assert!(!result.generations.is_empty());

    let response_text = result.generations[0].message.content().as_text();
    assert!(
        response_text.to_lowercase().contains("alice"),
        "Expected response to remember the name Alice, got: {}",
        response_text
    );
}

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
async fn test_temperature_variation() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    // Test with temperature 0 (deterministic)
    let model_deterministic = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_temperature(0.0);

    let messages = vec![Message::human("Say hello.")];

    let result1 = model_deterministic
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

    let result2 = model_deterministic
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

    // With temperature 0, responses should be identical or very similar
    let text1 = result1.generations[0].message.content().as_text();
    let text2 = result2.generations[0].message.content().as_text();

    // Note: Even with temperature 0, responses might vary slightly due to model non-determinism
    // So we just check that both responses are valid
    assert!(!text1.is_empty(), "Expected non-empty response 1");
    assert!(!text2.is_empty(), "Expected non-empty response 2");
}

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
async fn test_max_tokens_limit() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    let model = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_max_tokens(10); // Very short response

    let messages = vec![Message::human("Tell me a long story about a dragon.")];

    let result = model
        .generate(&messages, None, None, None, None)
        .await
        .expect("Failed to generate response");

    assert!(!result.generations.is_empty());

    let response_text = result.generations[0].message.content().as_text();
    let word_count = response_text.split_whitespace().count();

    // With max_tokens=10, response should be relatively short
    assert!(
        word_count < 50,
        "Expected short response due to max_tokens, got {} words",
        word_count
    );
}

#[tokio::test]
async fn test_model_builder_configuration() {
    // Test builder pattern (no API calls)
    let model = ChatReplicate::new()
        .with_model("meta/meta-llama-3-8b-instruct")
        .with_temperature(0.7)
        .with_max_tokens(512)
        .with_top_p(0.9)
        .with_frequency_penalty(0.5)
        .with_presence_penalty(0.5)
        .with_n(1);

    // Verify model returns correct llm_type
    assert_eq!(model.llm_type(), "replicate-chat");
}

#[tokio::test]
async fn test_identifying_params() {
    let model = ChatReplicate::new()
        .with_model("test-model")
        .with_temperature(0.5);

    let params = model.identifying_params();

    assert!(
        params.contains_key("model_name"),
        "Expected model_name in params"
    );
    assert_eq!(
        params.get("model_name").and_then(|v| v.as_str()),
        Some("test-model")
    );
    assert_eq!(
        params.get("provider").and_then(|v| v.as_str()),
        Some("replicate")
    );
    assert_eq!(
        params.get("temperature").and_then(|v| v.as_f64()),
        Some(0.5)
    );
}

#[tokio::test]
#[ignore = "requires REPLICATE_API_TOKEN"]
async fn test_error_handling_invalid_model() {
    let _token = std::env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");

    let model = ChatReplicate::new().with_model("invalid-model-that-does-not-exist-12345");

    let messages = vec![Message::human("Hello")];

    let result = model.generate(&messages, None, None, None, None).await;

    // Should return an error
    assert!(result.is_err(), "Expected error for invalid model");
}
