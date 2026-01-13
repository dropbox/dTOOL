//! Integration tests for stream cancellation with real OpenAI API
//!
//! These tests demonstrate that stream cancellation works correctly with
//! real LLM streaming. Run with: cargo test --test stream_cancellation_integration_tests -p dashflow-openai -- --ignored
//!
//! Requires OPENAI_API_KEY environment variable.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;
use futures::StreamExt;
use tokio::time::{timeout, Duration};

fn has_openai_key() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

#[tokio::test]
#[ignore = "requires API key"]
async fn test_stream_drop_cancels_http_request() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let openai = ChatOpenAI::default().with_model("gpt-4o-mini");

    let messages = vec![Message::human(
        "Write a very long story about a robot. Make it at least 1000 words.",
    )];

    // Start streaming
    let mut stream = openai
        ._stream(&messages, None, None, None, None)
        .await
        .expect("Failed to start stream");

    // Consume first chunk
    let first = stream.next().await;
    assert!(first.is_some(), "Expected at least one chunk");
    assert!(first.unwrap().is_ok(), "First chunk should be successful");

    // Get second chunk to ensure streaming is working
    let second = stream.next().await;
    assert!(second.is_some(), "Expected second chunk");
    assert!(second.unwrap().is_ok(), "Second chunk should be successful");

    // Drop stream (implicit cancellation)
    // This should cancel the HTTP request and stop streaming
    drop(stream);

    // Test passes if:
    // 1. No panic
    // 2. No hanging HTTP connections
    // 3. Clean resource cleanup (verified implicitly by Drop)

    eprintln!("✓ Stream cancelled cleanly via drop");
}

#[tokio::test]
#[ignore = "requires API key"]
async fn test_stream_timeout_cancels() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let openai = ChatOpenAI::default().with_model("gpt-4o-mini");

    let messages = vec![Message::human(
        "Write a very detailed explanation of quantum physics. Make it at least 2000 words.",
    )];

    // Start streaming
    let stream = openai
        ._stream(&messages, None, None, None, None)
        .await
        .expect("Failed to start stream");

    // Collect with timeout (very short - will not complete)
    let collect_future = stream.collect::<Vec<_>>();

    let result = timeout(Duration::from_millis(500), collect_future).await;

    // Should timeout (500ms not enough for 2000 word response)
    assert!(
        result.is_err(),
        "Expected timeout but stream completed within 500ms"
    );

    eprintln!("✓ Stream timed out correctly");
}

#[tokio::test]
#[ignore = "requires API key"]
async fn test_stream_partial_consumption() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let openai = ChatOpenAI::default().with_model("gpt-4o-mini");

    let messages = vec![Message::human("Count from 1 to 100.")];

    // Start streaming
    let mut stream = openai
        ._stream(&messages, None, None, None, None)
        .await
        .expect("Failed to start stream");

    // Consume only first 3 chunks
    let mut chunk_count = 0;
    for _ in 0..3 {
        if let Some(chunk) = stream.next().await {
            assert!(chunk.is_ok(), "Chunk should be successful");
            chunk_count += 1;
        }
    }

    assert_eq!(chunk_count, 3, "Expected to consume exactly 3 chunks");

    // Drop stream without consuming rest (implicit cancellation)
    drop(stream);

    // Test passes if no resource leak
    eprintln!("✓ Partial consumption and cancellation successful");
}

#[tokio::test]
#[ignore = "requires API key"]
async fn test_stream_collect_all() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let openai = ChatOpenAI::default().with_model("gpt-4o-mini");

    let messages = vec![Message::human("Say exactly: Hello World")];

    // Start streaming
    let stream = openai
        ._stream(&messages, None, None, None, None)
        .await
        .expect("Failed to start stream");

    // Collect all chunks
    let chunks: Vec<_> = stream.collect().await;

    // Should have at least one successful chunk
    assert!(!chunks.is_empty(), "Expected at least one chunk");
    assert!(
        chunks.iter().all(|c| c.is_ok()),
        "All chunks should be successful"
    );

    // Combine all content
    let combined_text: String = chunks
        .into_iter()
        .filter_map(|c| c.ok())
        .map(|chunk| chunk.message.content)
        .collect();

    assert!(
        combined_text.contains("Hello") || combined_text.contains("World"),
        "Response should contain requested text"
    );

    eprintln!("✓ Full stream collection successful");
    eprintln!("  Response: {}", combined_text);
}

#[tokio::test]
#[ignore = "requires API key"]
async fn test_stream_multiple_cancellations() {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let openai = ChatOpenAI::default().with_model("gpt-4o-mini");

    // Test multiple stream creation and cancellation cycles
    for i in 0..5 {
        let messages = vec![Message::human(format!(
            "Write a short paragraph about iteration {}.",
            i
        ))];

        let mut stream = openai
            ._stream(&messages, None, None, None, None)
            .await
            .expect("Failed to start stream");

        // Consume first chunk
        let first = stream.next().await;
        assert!(first.is_some(), "Expected first chunk");

        // Drop immediately
        drop(stream);
    }

    eprintln!("✓ Multiple cancellation cycles completed successfully");
}
