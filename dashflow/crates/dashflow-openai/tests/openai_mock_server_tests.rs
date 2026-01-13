// OpenAI Mock Server Integration Tests
// Author: Andrew Yates (ayates@dropbox.com) - 2025 Dropbox
//
//! Integration tests for OpenAI functionality using mock HTTP server.
//! These tests don't require an API key and can run without external dependencies.
//!
//! Run with: cargo test -p dashflow-openai --test openai_mock_server_tests

#![allow(clippy::unwrap_used)]

use async_openai::config::OpenAIConfig;
use dashflow::core::language_models::{ChatModel, ToolChoice, ToolDefinition};
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Create ChatOpenAI configured to use mock server
fn create_mock_client(mock_server_uri: &str) -> ChatOpenAI {
    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server_uri);
    ChatOpenAI::with_config(config).with_model("gpt-4o-mini")
}

/// Standard chat completion response
fn mock_chat_completion_response(content: &str) -> serde_json::Value {
    json!({
        "id": "chatcmpl-test-123",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
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

/// Tool call response
fn mock_tool_call_response(tool_name: &str, arguments: &str) -> serde_json::Value {
    json!({
        "id": "chatcmpl-test-456",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_test_1",
                    "type": "function",
                    "function": {
                        "name": tool_name,
                        "arguments": arguments
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 25,
            "total_tokens": 40
        }
    })
}

/// Error response
fn mock_error_response(code: &str, message: &str) -> serde_json::Value {
    json!({
        "error": {
            "message": message,
            "type": "invalid_request_error",
            "code": code
        }
    })
}

// ============= Basic Chat Completion Tests =============

#[tokio::test]
async fn test_mock_basic_chat_completion() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("Hello! How can I help you?")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Hello!")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(!response.generations.is_empty());
    let content = response.generations[0].message.content().as_text();
    assert!(content.contains("Hello"));
}

#[tokio::test]
async fn test_mock_multi_turn_conversation() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "The capital of France is Paris.",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("What is the capital of France?"),
    ];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    let content = response.generations[0].message.content().as_text();
    assert!(content.contains("Paris"));
}

#[tokio::test]
async fn test_mock_temperature_setting() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({"temperature": 0.7})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("Temperature test response")),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config)
        .with_model("gpt-4o-mini")
        .with_temperature(0.7);

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_max_tokens_setting() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({"max_tokens": 100})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("Max tokens test")),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config)
        .with_model("gpt-4o-mini")
        .with_max_tokens(100);

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_model_name() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({"model": "gpt-4"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("GPT-4 response")),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config).with_model("gpt-4");

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

// ============= Tool Calling Tests =============

#[tokio::test]
async fn test_mock_tool_call_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_tool_call_response(
                "get_weather",
                r#"{"location": "San Francisco", "unit": "celsius"}"#,
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());

    let tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get weather information".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"},
                "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
            },
            "required": ["location"]
        }),
    };

    let messages = vec![Message::human("What's the weather in San Francisco?")];
    let result = model
        .generate(&messages, None, Some(&[tool]), None, None)
        .await;
    assert!(result.is_ok());

    let response = result.unwrap();
    let message = &response.generations[0].message;
    assert!(!message.tool_calls().is_empty());
    assert_eq!(message.tool_calls()[0].name, "get_weather");
}

#[tokio::test]
async fn test_mock_tool_choice_required() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_tool_call_response("search", r#"{"query": "test"}"#)),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());

    let tool = ToolDefinition {
        name: "search".to_string(),
        description: "Search for information".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"]
        }),
    };

    let messages = vec![Message::human("Search for something")];
    let result = model
        .generate(
            &messages,
            None,
            Some(&[tool]),
            Some(&ToolChoice::Required),
            None,
        )
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_multiple_tool_calls() {
    let mock_server = MockServer::start().await;

    let response = json!({
        "id": "chatcmpl-test-789",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"NYC\"}"
                        }
                    },
                    {
                        "id": "call_2",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"LA\"}"
                        }
                    }
                ]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 20, "completion_tokens": 30, "total_tokens": 50}
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("What's the weather in NYC and LA?")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.generations[0].message.tool_calls().len(), 2);
}

// ============= Error Handling Tests =============

#[tokio::test]
async fn test_mock_rate_limit_error() {
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
async fn test_mock_invalid_api_key_error() {
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
async fn test_mock_model_not_found_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(mock_error_response("model_not_found", "Model not found")),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config).with_model("nonexistent-model");

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mock_server_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(mock_error_response("server_error", "Internal server error")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mock_context_length_exceeded() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(mock_error_response(
                "context_length_exceeded",
                "This model's maximum context length is 8192 tokens",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("A".repeat(100000))];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_err());
}

// ============= Usage Metadata Tests =============

#[tokio::test]
async fn test_mock_usage_metadata() {
    let mock_server = MockServer::start().await;

    let response = json!({
        "id": "chatcmpl-test-usage",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Test"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 50,
            "total_tokens": 150
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    if let Some(llm_output) = &response.llm_output {
        // Usage is stored directly in llm_output HashMap with keys like "prompt_tokens"
        assert!(
            llm_output.contains_key("prompt_tokens") || llm_output.contains_key("total_tokens")
        );
    }
}

// ============= Response Format Tests =============

#[tokio::test]
async fn test_mock_json_mode_response() {
    let mock_server = MockServer::start().await;

    let response = json!({
        "id": "chatcmpl-json",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "{\"name\": \"John\", \"age\": 30}"
            },
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 15, "total_tokens": 25}
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(
            json!({"response_format": {"type": "json_object"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config)
        .with_model("gpt-4o-mini")
        .with_json_mode();

    let messages = vec![Message::human("Return JSON with name and age")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    let content = response.generations[0].message.content().as_text();
    assert!(content.contains("John"));
}

// ============= Message Type Tests =============

#[tokio::test]
async fn test_mock_system_message_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user"}
            ]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("I am ready to help!")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("Hello"),
    ];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_assistant_message_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Continuing our conversation...",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![
        Message::human("Hello"),
        Message::ai("Hi there!"),
        Message::human("How are you?"),
    ];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

// ============= Multiple Choices Tests =============

#[tokio::test]
async fn test_mock_multiple_choices() {
    let mock_server = MockServer::start().await;

    let response = json!({
        "id": "chatcmpl-multi",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": "Response A"},
                "finish_reason": "stop"
            },
            {
                "index": 1,
                "message": {"role": "assistant", "content": "Response B"},
                "finish_reason": "stop"
            }
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30}
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config)
        .with_model("gpt-4o-mini")
        .with_n(2);

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.generations.len(), 2);
}

// ============= Finish Reason Tests =============

#[tokio::test]
async fn test_mock_finish_reason_length() {
    let mock_server = MockServer::start().await;

    let response = json!({
        "id": "chatcmpl-length",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "This is trunca..."},
            "finish_reason": "length"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 100, "total_tokens": 110}
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_finish_reason_content_filter() {
    let mock_server = MockServer::start().await;

    let response = json!({
        "id": "chatcmpl-filter",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": ""},
            "finish_reason": "content_filter"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 0, "total_tokens": 10}
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

// ============= Parameter Combination Tests =============

#[tokio::test]
async fn test_mock_all_parameters() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("Full parameter test")),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-key")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config)
        .with_model("gpt-4")
        .with_temperature(0.5)
        .with_max_tokens(500)
        .with_top_p(0.9)
        .with_frequency_penalty(0.5)
        .with_presence_penalty(0.5);

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

// ============= Empty and Edge Case Tests =============

#[tokio::test]
async fn test_mock_empty_response_content() {
    let mock_server = MockServer::start().await;

    let response = json!({
        "id": "chatcmpl-empty",
        "object": "chat.completion",
        "created": 1699000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": ""},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 5, "completion_tokens": 0, "total_tokens": 5}
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_unicode_content() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("Привет! 你好! \u{1F600}")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test unicode")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    let content = response.generations[0].message.content().as_text();
    assert!(content.contains("Привет"));
}

#[tokio::test]
async fn test_mock_long_message() {
    let mock_server = MockServer::start().await;

    let long_content = "A".repeat(10000);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(&long_content)),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Generate long response")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    let content = response.generations[0].message.content().as_text();
    assert_eq!(content.len(), 10000);
}

#[tokio::test]
async fn test_mock_special_characters_in_content() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_completion_response(
                "Code: `fn main() { println!(\"Hello\"); }`",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Write some code")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

// ============= Request Validation Tests =============

#[tokio::test]
async fn test_mock_authorization_header_sent() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer test-api-key-123"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("Authenticated")),
        )
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig::new()
        .with_api_key("test-api-key-123")
        .with_api_base(mock_server.uri());
    let model = ChatOpenAI::with_config(config).with_model("gpt-4o-mini");

    let messages = vec![Message::human("Test")];
    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_content_type_header() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_completion_response("Content type ok")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let result = model.generate(&messages, None, None, None, None).await;
    assert!(result.is_ok());
}
