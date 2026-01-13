//! Client-server integration tests
//!
//! Tests RemoteRunnable client communicating with a live server

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::Router;
use dashflow::core::config::RunnableConfig;
use dashflow::core::runnable::Runnable;
use dashflow_langserve::{add_routes, create_server, RemoteRunnable, RouteConfig};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// M-582: Guard that aborts server task on drop (prevents leaks on panic)
///
/// This ensures the spawned server is properly cleaned up even if the test panics.
struct ServerGuard {
    handle: JoinHandle<()>,
    url: String,
}

impl ServerGuard {
    fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

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
        // Stream the input as chunks if it's a string
        if let Some(text) = input.as_str() {
            let chunks: Vec<_> = text.chars().map(|c| Ok(json!(c.to_string()))).collect();
            Ok(Box::pin(stream::iter(chunks)))
        } else {
            Ok(Box::pin(stream::once(async move { Ok(input) })))
        }
    }
}

/// Test runnable that uppercases strings
struct UppercaseRunnable;

#[async_trait::async_trait]
impl Runnable for UppercaseRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        if let Some(text) = input.as_str() {
            Ok(json!(text.to_uppercase()))
        } else {
            Ok(input)
        }
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>, dashflow::core::Error> {
        let mut outputs = Vec::new();
        for input in inputs {
            if let Some(text) = input.as_str() {
                outputs.push(json!(text.to_uppercase()));
            } else {
                outputs.push(input);
            }
        }
        Ok(outputs)
    }
}

/// M-578: Helper to wait for HTTP server readiness with retry
async fn wait_for_server_ready(url: &str, max_retries: u32) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();

    for attempt in 0..max_retries {
        // Try to connect to any endpoint
        match client.get(format!("{}/health", url)).send().await {
            Ok(_) => return true,
            Err(_) => {
                // Exponential backoff: 10ms, 20ms, 40ms, 80ms, 160ms
                let delay = Duration::from_millis(10 * (1 << attempt.min(4)));
                tokio::time::sleep(delay).await;
            }
        }
    }
    false
}

/// Helper to start a test server and return a guard (which aborts server on drop)
///
/// M-582: Returns ServerGuard that ensures cleanup on test panic
async fn start_test_server(app: Router) -> ServerGuard {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://127.0.0.1:{}", addr.port());

    let handle = tokio::spawn(async move {
        // Ignore shutdown errors when server is aborted
        let _ = axum::serve(listener, app).await;
    });

    // M-578: Use readiness check with retry instead of fixed sleep
    let ready = wait_for_server_ready(&url, 10).await;
    assert!(ready, "Server failed to start within timeout");

    ServerGuard { handle, url }
}

#[tokio::test]
async fn test_remote_runnable_invoke() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client
    let client = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();

    // Test invoke
    let result = client.invoke(json!("Hello from client!"), None).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!("Hello from client!"));
}

#[tokio::test]
async fn test_remote_runnable_batch() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client
    let client = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();

    // Test batch
    let inputs = vec![json!("input1"), json!("input2"), json!("input3")];
    let result = client.batch(inputs.clone(), None).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), inputs);
}

#[tokio::test]
async fn test_remote_runnable_stream() {
    use futures::StreamExt;

    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client
    let client = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();

    // Test stream
    let mut stream = client.stream(json!("Hi"), None).await.unwrap();
    let mut chunks = Vec::new();

    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.expect("stream chunk should be Ok"));
    }

    let outputs: Vec<Value> = chunks
        .iter()
        .map(|chunk| {
            chunk
                .get("output")
                .cloned()
                .expect("stream chunk should contain an 'output' field")
        })
        .collect();
    assert_eq!(outputs, vec![json!("H"), json!("i")]);
}

#[tokio::test]
async fn test_client_with_uppercase_runnable() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, UppercaseRunnable, RouteConfig::new("/upper"));
    let guard = start_test_server(app).await;

    // Create client
    let client = RemoteRunnable::new(&format!("{}/upper", guard.url())).unwrap();

    // Test invoke
    let result = client.invoke(json!("hello world"), None).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!("HELLO WORLD"));

    // Test batch
    let inputs = vec![json!("hello"), json!("world")];
    let result = client.batch(inputs, None).await;
    assert!(result.is_ok());
    let outputs = result.unwrap();
    assert_eq!(outputs[0], json!("HELLO"));
    assert_eq!(outputs[1], json!("WORLD"));
}

#[tokio::test]
async fn test_client_with_config() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client with timeout
    let client = RemoteRunnable::with_timeout(&format!("{}/echo", guard.url()), 30).unwrap();

    // Test invoke with config
    let config = dashflow_langserve::RunnableConfig {
        tags: vec!["test".to_string()],
        ..Default::default()
    };

    let result = client.invoke(json!("test"), Some(config)).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!("test"));
}

#[tokio::test]
async fn test_client_error_handling() {
    // Bind to an ephemeral port to guarantee it was free, then drop the listener so the
    // subsequent request deterministically fails (connection refused) without relying on
    // "9999 is unused" assumptions.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Create client pointing to a non-existent server
    let client = RemoteRunnable::new(&format!("http://127.0.0.1:{}/nonexistent", port)).unwrap();

    // Test invoke should fail
    let result = client.invoke(json!("test"), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_client_invalid_response() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client pointing to wrong endpoint (schema endpoint instead of invoke)
    let client = RemoteRunnable::new(&format!("{}/echo/input_schema", guard.url())).unwrap();

    // Test invoke should fail (wrong endpoint)
    let result = client.invoke(json!("test"), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_clients_same_server() {
    // Start server with two runnables (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let app = add_routes(app, UppercaseRunnable, RouteConfig::new("/upper"));
    let guard = start_test_server(app).await;

    // Create two clients
    let client1 = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();
    let client2 = RemoteRunnable::new(&format!("{}/upper", guard.url())).unwrap();

    // Test both clients work independently
    let result1 = client1.invoke(json!("test"), None).await.unwrap();
    let result2 = client2.invoke(json!("test"), None).await.unwrap();

    assert_eq!(result1, json!("test"));
    assert_eq!(result2, json!("TEST"));
}

#[tokio::test]
async fn test_client_concurrent_requests() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client
    let client = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();

    // Send multiple concurrent requests
    let mut handles = vec![];
    for i in 0..10 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            client
                .invoke(json!(format!("request_{}", i)), None)
                .await
                .unwrap()
        });
        handles.push(handle);
    }

    // Wait for all to complete
    let results = futures::future::join_all(handles).await;
    assert_eq!(results.len(), 10);

    // Verify all succeeded
    for result in results {
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_client_chaining_runnables() {
    // Start server with two runnables that can be chained (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let app = add_routes(app, UppercaseRunnable, RouteConfig::new("/upper"));
    let guard = start_test_server(app).await;

    // Create two remote runnables
    let echo_client = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();
    let upper_client = RemoteRunnable::new(&format!("{}/upper", guard.url())).unwrap();

    // Chain: input -> echo -> uppercase
    let input = json!("hello");
    let echo_result = echo_client.invoke(input, None).await.unwrap();
    let final_result = upper_client.invoke(echo_result, None).await.unwrap();

    assert_eq!(final_result, json!("HELLO"));
}

#[tokio::test]
async fn test_client_complex_json() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client
    let client = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();

    // Test with complex JSON structure
    let complex_input = json!({
        "name": "test",
        "age": 25,
        "nested": {
            "field1": "value1",
            "field2": [1, 2, 3]
        },
        "array": ["a", "b", "c"]
    });

    let result = client.invoke(complex_input.clone(), None).await.unwrap();
    assert_eq!(result, complex_input);
}

#[tokio::test]
async fn test_client_empty_input() {
    // Start server (guard ensures cleanup on panic)
    let app = create_server();
    let app = add_routes(app, EchoRunnable, RouteConfig::new("/echo"));
    let guard = start_test_server(app).await;

    // Create client
    let client = RemoteRunnable::new(&format!("{}/echo", guard.url())).unwrap();

    // Test with null
    let result = client.invoke(json!(null), None).await.unwrap();
    assert_eq!(result, json!(null));

    // Test with empty string
    let result = client.invoke(json!(""), None).await.unwrap();
    assert_eq!(result, json!(""));

    // Test with empty object
    let result = client.invoke(json!({}), None).await.unwrap();
    assert_eq!(result, json!({}));

    // Test with empty array
    let result = client.invoke(json!([]), None).await.unwrap();
    assert_eq!(result, json!([]));
}
