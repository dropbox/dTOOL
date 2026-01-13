//! Anthropic Mock Server Error Handling Tests (M-326)
//!
//! These tests validate that `dashflow-anthropic` maps Anthropic API error responses into
//! typed `dashflow::core::error::Error` variants, enabling correct retry behavior.
//!
//! Run with: `cargo test -p dashflow-anthropic --test anthropic_mock_server_error_tests`

#![allow(clippy::expect_used, clippy::panic)]

use dashflow::core::error::Error as DashFlowError;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow::core::retry::{with_retry, RetryPolicy};
use dashflow_anthropic::ChatAnthropic;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn error_envelope(error_type: &str, message: &str) -> serde_json::Value {
    json!({
        "type": "error",
        "error": {
            "type": error_type,
            "message": message
        }
    })
}

fn success_response(text: &str) -> serde_json::Value {
    json!({
        "id": "msg_test_123",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": text}],
        "model": "claude-3-7-sonnet-20250219",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 10, "output_tokens": 5}
    })
}

fn create_mock_client(mock_server_uri: &str) -> ChatAnthropic {
    ChatAnthropic::try_new()
        .expect("ChatAnthropic::try_new")
        .with_api_key("test-key")
        .with_api_url(format!("{}/v1/messages", mock_server_uri))
        .with_api_version("2023-06-01")
}

#[tokio::test]
async fn test_rate_limit_error_maps_to_rate_limit_and_includes_retry_after() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                .set_body_json(error_envelope("rate_limit_error", "Too many requests")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];
    let err = model
        .generate(&messages, None, None, None, None)
        .await
        .expect_err("expected error");

    match err {
        DashFlowError::RateLimit(msg) => assert!(msg.contains("retry_after=30"), "{msg}"),
        other => panic!("expected RateLimit, got {other:?}"),
    }
}

#[tokio::test]
async fn test_invalid_request_error_maps_to_invalid_input() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(error_envelope(
                "invalid_request_error",
                "Bad request",
            )),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];
    let err = model
        .generate(&messages, None, None, None, None)
        .await
        .expect_err("expected error");

    assert!(matches!(err, DashFlowError::InvalidInput(_)));
}

#[tokio::test]
async fn test_authentication_error_maps_to_authentication() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_json(error_envelope(
            "authentication_error",
            "Invalid API key",
        )))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];
    let err = model
        .generate(&messages, None, None, None, None)
        .await
        .expect_err("expected error");

    assert!(matches!(err, DashFlowError::Authentication(_)));
}

#[tokio::test]
async fn test_permission_error_maps_to_authentication() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(403).set_body_json(error_envelope(
            "permission_error",
            "Forbidden",
        )))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];
    let err = model
        .generate(&messages, None, None, None, None)
        .await
        .expect_err("expected error");

    assert!(matches!(err, DashFlowError::Authentication(_)));
}

#[tokio::test]
async fn test_not_found_error_maps_to_invalid_input() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(error_envelope("not_found_error", "Not found")),
        )
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];
    let err = model
        .generate(&messages, None, None, None, None)
        .await
        .expect_err("expected error");

    assert!(matches!(err, DashFlowError::InvalidInput(_)));
}

#[tokio::test]
async fn test_overloaded_error_maps_to_network() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(529).set_body_json(error_envelope(
            "overloaded_error",
            "Overloaded",
        )))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];
    let err = model
        .generate(&messages, None, None, None, None)
        .await
        .expect_err("expected error");

    assert!(matches!(err, DashFlowError::Network(_)));
}

#[tokio::test]
async fn test_plain_text_rate_limit_error_maps_to_rate_limit() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(429).set_body_string("Too Many Requests"))
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];
    let err = model
        .generate(&messages, None, None, None, None)
        .await
        .expect_err("expected error");

    assert!(matches!(err, DashFlowError::RateLimit(_)));
}

#[tokio::test]
async fn test_retry_policy_retries_on_rate_limit_and_succeeds() {
    let mock_server = MockServer::start().await;
    let request_count = Arc::new(AtomicUsize::new(0));
    let request_count_for_responder = Arc::clone(&request_count);

    let responder = move |_req: &wiremock::Request| {
        let n = request_count_for_responder.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            ResponseTemplate::new(429).set_body_json(error_envelope("rate_limit_error", "Retry me"))
        } else {
            ResponseTemplate::new(200).set_body_json(success_response("ok"))
        }
    };

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let model = create_mock_client(&mock_server.uri());
    let messages = vec![Message::human("Test")];

    let policy = RetryPolicy::fixed(1, 0);
    let result = with_retry(&policy, || async {
        model.generate(&messages, None, None, None, None).await
    })
    .await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(request_count.load(Ordering::SeqCst), 2);
}
