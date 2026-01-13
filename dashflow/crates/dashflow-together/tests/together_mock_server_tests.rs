// Together AI Mock Server Integration Tests
// Author: Andrew Yates (ayates@dropbox.com) - 2025 Dropbox
//
//! Integration tests for Together AI using mock HTTP server.
//! Together AI uses an OpenAI-compatible API, so the mock responses follow the same format.
//!
//! Run with: cargo test -p dashflow-together --test together_mock_server_tests

use async_openai::config::OpenAIConfig;
use dashflow::core::error::Result;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_together::ChatTogether;
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Create ChatTogether configured to use mock server
fn create_mock_client(mock_server_uri: &str) -> ChatTogether {
    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server_uri);
    ChatTogether::with_config(config).with_model("meta-llama/Llama-3-8b-chat-hf")
}

/// Standard chat completion response (OpenAI format)
fn mock_chat_completion_response(content: &str, model: &str) -> serde_json::Value {
    json!({
        "id": "chatcmpl-together-123",
        "object": "chat.completion",
        "created": 1699000000,
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    })
}

fn mock_error_response(code: &str, message: &str) -> serde_json::Value {
    json!({
        "error": {
            "message": message,
            "type": "invalid_request_error",
            "code": code
        }
    })
}

#[tokio::test]
async fn test_together_basic_chat_completion() -> Result<()> {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Hello from Together AI!",
                "meta-llama/Llama-3-8b-chat-hf",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Hello!")];

    let response = model.generate(&messages, None, None, None, None).await?;
    assert!(!response.generations.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_together_llama_model() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(
            json!({"model": "meta-llama/Llama-3-70b-chat-hf"}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Llama response",
                "meta-llama/Llama-3-70b-chat-hf",
            )),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatTogether::with_config(config).with_model("meta-llama/Llama-3-70b-chat-hf");

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_together_mistral_model() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(
            json!({"model": "mistralai/Mixtral-8x7B-Instruct-v0.1"}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Mixtral response",
                "mistralai/Mixtral-8x7B-Instruct-v0.1",
            )),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model =
        ChatTogether::with_config(config).with_model("mistralai/Mixtral-8x7B-Instruct-v0.1");

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_together_temperature_setting() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({"temperature": 0.7})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Temperature response",
                "meta-llama/Llama-3-8b-chat-hf",
            )),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatTogether::with_config(config)
        .with_model("meta-llama/Llama-3-8b-chat-hf")
        .with_temperature(0.7);

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_together_rate_limit_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(mock_error_response(
                "rate_limit_exceeded",
                "Rate limit reached",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_together_invalid_api_key() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(mock_error_response("invalid_api_key", "Invalid API key")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_together_multi_turn_conversation() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Multi-turn response",
                "meta-llama/Llama-3-8b-chat-hf",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("What is AI?"),
        Message::ai("AI is artificial intelligence."),
        Message::human("Tell me more."),
    ];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_together_max_tokens() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({"max_tokens": 100})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Max tokens response",
                "meta-llama/Llama-3-8b-chat-hf",
            )),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatTogether::with_config(config)
        .with_model("meta-llama/Llama-3-8b-chat-hf")
        .with_max_tokens(100);

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}
