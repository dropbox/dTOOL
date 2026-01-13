//! Integration tests for LangServe
//!
//! These tests verify end-to-end functionality by:
//! - Starting a real server
//! - Making HTTP requests
//! - Verifying responses
//!
//! Tests cover:
//! - Invoke endpoint
//! - Batch endpoint
//! - Stream endpoint
//! - Schema endpoints
//! - Client-server communication
//! - Error scenarios
//! - Concurrent requests

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::Router;
use dashflow::core::config::RunnableConfig;
use dashflow::core::runnable::Runnable;
use dashflow_langserve::{add_routes, create_server, RouteConfig};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::TcpListener;

/// Test runnable that echoes input
struct EchoRunnable;

#[async_trait::async_trait]
impl Runnable for EchoRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        Ok(input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>, dashflow::core::Error> {
        Ok(inputs)
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<
        std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<Self::Output, dashflow::core::Error>> + Send>,
        >,
        dashflow::core::Error,
    > {
        use futures::stream;
        // Stream the input as chunks if it's a string, otherwise stream it once
        if let Some(text) = input.as_str() {
            let chunks: Vec<_> = text.chars().map(|c| Ok(json!(c.to_string()))).collect();
            Ok(Box::pin(stream::iter(chunks)))
        } else {
            Ok(Box::pin(stream::once(async move { Ok(input) })))
        }
    }
}

/// Test runnable that adds a prefix
struct PrefixRunnable {
    prefix: String,
}

#[async_trait::async_trait]
impl Runnable for PrefixRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        if let Some(text) = input.as_str() {
            Ok(json!(format!("{}{}", self.prefix, text)))
        } else {
            Ok(json!(format!("{}{}", self.prefix, input)))
        }
    }
}

/// Test runnable that fails
struct FailingRunnable;

#[async_trait::async_trait]
impl Runnable for FailingRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        _input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        Err(dashflow::core::Error::other("Intentional test failure"))
    }
}

/// Helper to start a test server and return its URL
async fn start_test_server(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://127.0.0.1:{}", addr.port());

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait a bit for server to be ready
    tokio::time::sleep(Duration::from_millis(100)).await;

    url
}

#[tokio::test]
async fn test_invoke_endpoint() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/echo/invoke", url))
        .json(&json!({
            "input": "Hello LangServe!"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["output"], "Hello LangServe!");
    assert!(body["metadata"]["run_id"].is_string());
}

#[tokio::test]
async fn test_batch_endpoint() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/echo/batch", url))
        .json(&json!({
            "inputs": ["input1", "input2", "input3"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["output"].as_array().unwrap().len(), 3);
    assert_eq!(body["output"][0], "input1");
    assert_eq!(body["output"][1], "input2");
    assert_eq!(body["output"][2], "input3");
    assert!(body["metadata"]["run_ids"].as_array().unwrap().len() == 3);
}

#[tokio::test]
async fn test_stream_endpoint() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/echo/stream", url))
        .json(&json!({
            "input": "Hi"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let text = response.text().await.unwrap();
    // Verify it contains SSE formatted data
    assert!(text.contains("event:"));
    assert!(text.contains("data:"));
}

#[tokio::test]
async fn test_input_schema_endpoint() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/echo/input_schema", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert!(body["title"].is_string());
    assert!(body["type"].is_string());
}

#[tokio::test]
async fn test_output_schema_endpoint() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/echo/output_schema", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert!(body["title"].is_string());
    assert!(body["type"].is_string());
}

#[tokio::test]
async fn test_config_schema_endpoint() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/echo/config_schema", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert!(body["title"].is_string());
}

#[tokio::test]
async fn test_playground_endpoint() {
    let app = create_server();
    let config = RouteConfig::new("/echo").with_playground(true);
    let app = add_routes(app, EchoRunnable, config);
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/echo/playground", url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let text = response.text().await.unwrap();
    // Verify it's HTML
    assert!(text.contains("<!DOCTYPE html>"));
    assert!(text.contains("LangServe Playground"));
}

#[tokio::test]
async fn test_multiple_runnables() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let app = add_routes(
        app,
        PrefixRunnable {
            prefix: "PREFIX: ".to_string(),
        },
        RouteConfig::new("/prefix"),
    );
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();

    // Test echo endpoint
    let response = client
        .post(format!("{}/echo/invoke", url))
        .json(&json!({"input": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["output"], "test");

    // Test prefix endpoint
    let response = client
        .post(format!("{}/prefix/invoke", url))
        .json(&json!({"input": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["output"], "PREFIX: test");
}

#[tokio::test]
async fn test_disabled_endpoints() {
    let app = create_server();
    let config = RouteConfig::new("/echo")
        .with_batch(false)
        .with_stream(false)
        .with_playground(false);
    let app = add_routes(app, EchoRunnable, config);
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();

    // Invoke should work
    let response = client
        .post(format!("{}/echo/invoke", url))
        .json(&json!({"input": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Batch should fail (404)
    let response = client
        .post(format!("{}/echo/batch", url))
        .json(&json!({"inputs": ["test"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // Stream should fail (404)
    let response = client
        .post(format!("{}/echo/stream", url))
        .json(&json!({"input": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // Playground should fail (404)
    let response = client
        .get(format!("{}/echo/playground", url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_error_handling() {
    let app = create_server();
    let app = add_routes(app, FailingRunnable, RouteConfig::new("/fail"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/fail/invoke", url))
        .json(&json!({"input": "test"}))
        .send()
        .await
        .unwrap();

    // Server should return an error status
    assert!(response.status().is_server_error() || response.status().is_client_error());
}

#[tokio::test]
async fn test_invalid_json() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/echo/invoke", url))
        .header("Content-Type", "application/json")
        .body("invalid json {{{")
        .send()
        .await
        .unwrap();

    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn test_missing_input_field() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/echo/invoke", url))
        .json(&json!({"wrong_field": "value"}))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn test_cors_headers() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();

    // Just verify the endpoint is accessible (CORS is configured at Axum layer)
    let response = client
        .post(format!("{}/echo/invoke", url))
        .json(&json!({"input": "test"}))
        .send()
        .await
        .unwrap();

    // Should succeed - CORS is permissive by default in create_server()
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_concurrent_requests() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let mut handles = vec![];

    // Send 10 concurrent requests
    for i in 0..10 {
        let client = client.clone();
        let url = url.clone();
        let handle = tokio::spawn(async move {
            let response = client
                .post(format!("{}/echo/invoke", url))
                .json(&json!({"input": format!("request_{}", i)}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            response.json::<Value>().await.unwrap()
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let results = futures::future::join_all(handles).await;
    assert_eq!(results.len(), 10);

    // Verify all succeeded
    for result in results {
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_batch_with_config() {
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/echo/batch", url))
        .json(&json!({
            "inputs": ["input1", "input2"],
            "config": {
                "tags": ["test"]
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["output"].as_array().unwrap().len(), 2);
}
