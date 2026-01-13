//! Streaming integration tests
//!
//! Tests SSE streaming functionality

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::Router;
use dashflow::core::config::RunnableConfig;
use dashflow::core::runnable::Runnable;
use dashflow_langserve::{add_routes, create_server, RemoteRunnable, RouteConfig};
use futures::StreamExt;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::TcpListener;

/// Test runnable that streams character by character
struct CharStreamRunnable;

#[async_trait::async_trait]
impl Runnable for CharStreamRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        Ok(input)
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
        if let Some(text) = input.as_str() {
            let chunks: Vec<_> = text.chars().map(|c| Ok(json!(c.to_string()))).collect();
            Ok(Box::pin(stream::iter(chunks)))
        } else {
            Ok(Box::pin(stream::once(async move { Ok(input) })))
        }
    }
}

/// Test runnable that streams numbers
struct CounterRunnable {
    max: usize,
}

#[async_trait::async_trait]
impl Runnable for CounterRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        _input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        Ok(json!(self.max))
    }

    async fn stream(
        &self,
        _input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<
        std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<Self::Output, dashflow::core::Error>> + Send>,
        >,
        dashflow::core::Error,
    > {
        use futures::stream;
        let chunks: Vec<_> = (0..self.max).map(|i| Ok(json!(i))).collect();
        Ok(Box::pin(stream::iter(chunks)))
    }
}

/// Test runnable that streams slowly
struct SlowStreamRunnable {
    delay_ms: u64,
}

#[async_trait::async_trait]
impl Runnable for SlowStreamRunnable {
    type Input = Value;
    type Output = Value;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, dashflow::core::Error> {
        Ok(input)
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
        let delay = self.delay_ms;
        if let Some(text) = input.as_str() {
            let chars: Vec<char> = text.chars().collect();
            Ok(Box::pin(stream::unfold(0, move |idx| {
                let chars = chars.clone();
                async move {
                    if idx < chars.len() {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        Some((Ok(json!(chars[idx].to_string())), idx + 1))
                    } else {
                        None
                    }
                }
            })))
        } else {
            Ok(Box::pin(stream::once(async move { Ok(input) })))
        }
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
async fn test_stream_endpoint_basic() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/stream/stream", url))
        .json(&json!({"input": "Hello"}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let text = response.text().await.unwrap();
    // Verify SSE format
    assert!(text.contains("event:"));
    assert!(text.contains("data:"));
}

#[tokio::test]
async fn test_stream_client() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();
    let mut stream = client.stream(json!("Hi"), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            // Stream output is wrapped in {"output": ...}
            if let Some(output) = value.get("output") {
                chunks.push(output.clone());
            }
        }
    }

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], json!("H"));
    assert_eq!(chunks[1], json!("i"));
}

#[tokio::test]
async fn test_stream_empty_string() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();
    let mut stream = client.stream(json!(""), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            chunks.push(value);
        }
    }

    // Empty string produces no character chunks
    assert_eq!(chunks.len(), 0);
}

#[tokio::test]
async fn test_stream_long_text() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();
    let long_text = "The quick brown fox jumps over the lazy dog";
    let mut stream = client.stream(json!(long_text), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            chunks.push(value);
        }
    }

    // Should have one chunk per character
    assert_eq!(chunks.len(), long_text.len());
}

#[tokio::test]
async fn test_stream_counter() {
    let app = create_server();
    let app = add_routes(
        app,
        CounterRunnable { max: 5 },
        RouteConfig::new("/counter"),
    );
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/counter", url)).unwrap();
    let mut stream = client.stream(json!(null), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            // Stream output is wrapped in {"output": ...}
            if let Some(output) = value.get("output") {
                chunks.push(output.clone());
            }
        }
    }

    assert_eq!(chunks.len(), 5);
    assert_eq!(chunks[0], json!(0));
    assert_eq!(chunks[1], json!(1));
    assert_eq!(chunks[2], json!(2));
    assert_eq!(chunks[3], json!(3));
    assert_eq!(chunks[4], json!(4));
}

#[tokio::test]
async fn test_stream_non_string_input() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();
    let mut stream = client.stream(json!({"key": "value"}), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            // Stream output is wrapped in {"output": ...}
            if let Some(output) = value.get("output") {
                chunks.push(output.clone());
            }
        }
    }

    // Non-string input returns single chunk
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], json!({"key": "value"}));
}

#[tokio::test]
async fn test_stream_with_config() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();
    let config = dashflow_langserve::RunnableConfig {
        tags: vec!["streaming".to_string()],
        ..Default::default()
    };

    let mut stream = client.stream(json!("Hi"), Some(config)).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            chunks.push(value);
        }
    }

    assert_eq!(chunks.len(), 2);
}

#[tokio::test]
async fn test_stream_multiple_concurrent() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();

    // Start multiple streams concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let mut stream = client
                .stream(json!(format!("text{}", i)), None)
                .await
                .unwrap();
            let mut count = 0;
            while let Some(chunk) = stream.next().await {
                if chunk.is_ok() {
                    count += 1;
                }
            }
            count
        });
        handles.push(handle);
    }

    // Wait for all streams to complete
    let results = futures::future::join_all(handles).await;

    // Each should have streamed 5 characters ("text0", "text1", etc.)
    for result in results {
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5);
    }
}

#[tokio::test]
async fn test_stream_slow_producer() {
    let app = create_server();
    let app = add_routes(
        app,
        SlowStreamRunnable { delay_ms: 10 },
        RouteConfig::new("/slow"),
    );
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/slow", url)).unwrap();
    let start = std::time::Instant::now();
    let mut stream = client.stream(json!("Hi"), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            chunks.push(value);
        }
    }

    let elapsed = start.elapsed();

    // Should have 2 chunks
    assert_eq!(chunks.len(), 2);
    // Should take at least 20ms (2 chunks * 10ms delay)
    assert!(elapsed >= Duration::from_millis(20));
}

#[tokio::test]
async fn test_stream_special_characters() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();
    let special = "Hello\nWorld\t!";
    let mut stream = client.stream(json!(special), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            chunks.push(value);
        }
    }

    assert_eq!(chunks.len(), special.len());
}

#[tokio::test]
async fn test_stream_unicode() {
    let app = create_server();
    let app = add_routes(app, CharStreamRunnable, RouteConfig::new("/stream"));
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/stream", url)).unwrap();
    let unicode = "Hello 世界 🌍";
    let mut stream = client.stream(json!(unicode), None).await.unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let Ok(value) = chunk {
            chunks.push(value);
        }
    }

    // Should handle Unicode correctly
    assert_eq!(chunks.len(), unicode.chars().count());
}

#[tokio::test]
async fn test_stream_disabled() {
    let app = create_server();
    let config = RouteConfig::new("/nostream").with_stream(false);
    let app = add_routes(app, CharStreamRunnable, config);
    let url = start_test_server(app).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/nostream/stream", url))
        .json(&json!({"input": "test"}))
        .send()
        .await
        .unwrap();

    // Should return 404 since streaming is disabled
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_stream_large_chunks() {
    let app = create_server();
    let app = add_routes(
        app,
        CounterRunnable { max: 100 },
        RouteConfig::new("/large"),
    );
    let url = start_test_server(app).await;

    let client = RemoteRunnable::new(&format!("{}/large", url)).unwrap();
    let mut stream = client.stream(json!(null), None).await.unwrap();

    let mut count = 0;
    while let Some(chunk) = stream.next().await {
        if chunk.is_ok() {
            count += 1;
        }
    }

    assert_eq!(count, 100);
}
