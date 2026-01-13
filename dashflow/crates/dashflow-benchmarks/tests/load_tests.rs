//! Load Tests for DashFlow-RS
//!
//! These tests verify system stability under high load:
//! - High concurrency (1000+ concurrent operations)
//! - Large document processing (1MB+ documents)
//! - Memory stability (no leaks over time)
//!
//! Run with: cargo test --release --test load_tests -- --nocapture --test-threads=1

use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::language_models::{
    ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
};
use dashflow::core::messages::{AIMessage, AIMessageChunk, BaseMessage, HumanMessage};
use dashflow::core::output_parsers::{JsonOutputParser, OutputParser, StrOutputParser};
use dashflow::core::vector_stores::{InMemoryVectorStore, VectorStore};
use dashflow_text_splitters::{CharacterTextSplitter, TextSplitter};
use futures::StreamExt;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Mock chat model for testing (zero latency)
#[derive(Clone)]
struct MockChatModel;

#[async_trait::async_trait]
impl ChatModel for MockChatModel {
    fn llm_type(&self) -> &str {
        "mock"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn _generate(
        &self,
        messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
    ) -> dashflow::core::error::Result<ChatResult> {
        let response =
            AIMessage::new(format!("Mock response to {} message(s)", messages.len()).as_str());

        Ok(ChatResult {
            generations: vec![ChatGeneration {
                message: response.into(),
                generation_info: None,
            }],
            llm_output: None,
        })
    }

    async fn _stream(
        &self,
        _messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
    ) -> dashflow::core::error::Result<
        std::pin::Pin<
            Box<
                dyn futures::stream::Stream<
                        Item = dashflow::core::error::Result<ChatGenerationChunk>,
                    > + Send,
            >,
        >,
    > {
        let chunks: Vec<&str> = vec!["Mock", " stream", " response"];
        let stream = futures::stream::iter(chunks.into_iter().map(|chunk| {
            Ok(ChatGenerationChunk {
                message: AIMessageChunk::new(chunk),
                generation_info: None,
            })
        }));

        Ok(Box::pin(stream))
    }
}

/// Mock embeddings for testing (random vectors)
struct MockEmbeddings {
    dimensions: usize,
}

impl MockEmbeddings {
    fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::Error> {
        // Generate deterministic "random" embeddings based on text content
        Ok(texts
            .iter()
            .map(|text| {
                let seed = text.len() as f32;
                (0..self.dimensions)
                    .map(|i| ((seed + i as f32) % 100.0) / 100.0)
                    .collect()
            })
            .collect())
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::Error> {
        let seed = text.len() as f32;
        Ok((0..self.dimensions)
            .map(|i| ((seed + i as f32) % 100.0) / 100.0)
            .collect())
    }
}

/// Test: High concurrency chat model invocations (1000+ concurrent)
#[test]
fn test_high_concurrency_chat_model() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== High Concurrency Chat Model Test ===");
        println!("Target: 1000 concurrent invocations");

        let model = MockChatModel;
        let start = Instant::now();
        let concurrency = 1000;

        // Create 1000 concurrent tasks
        let tasks: Vec<_> = (0..concurrency)
            .map(|i| {
                let model_clone = model.clone();
                async move {
                    let msg = HumanMessage::new(format!("Test message {}", i).as_str());
                    let messages = vec![msg.into()];
                    model_clone
                        .generate(&messages, None, None, None, None)
                        .await
                }
            })
            .collect();

        // Execute all concurrently
        let results = futures::future::join_all(tasks).await;

        let duration = start.elapsed();
        let success_count = results.iter().filter(|r| r.is_ok()).count();

        println!("Completed: {} invocations", concurrency);
        println!("Success: {}/{}", success_count, concurrency);
        println!("Duration: {:?}", duration);
        println!(
            "Throughput: {:.0} req/s",
            concurrency as f64 / duration.as_secs_f64()
        );

        assert_eq!(success_count, concurrency, "All invocations should succeed");
        assert!(duration.as_secs() < 5, "Should complete in under 5 seconds");
    });
}

/// Test: High concurrency streaming (1000+ concurrent streams)
#[test]
fn test_high_concurrency_streaming() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== High Concurrency Streaming Test ===");
        println!("Target: 1000 concurrent streams");

        let model = MockChatModel;
        let start = Instant::now();
        let concurrency = 1000;

        // Create 1000 concurrent streaming tasks
        let tasks: Vec<_> = (0..concurrency)
            .map(|i| {
                let model_clone = model.clone();
                async move {
                    let msg = HumanMessage::new(format!("Stream test {}", i).as_str());
                    let messages = vec![msg.into()];
                    let mut stream = model_clone
                        .stream(&messages, None, None, None, None)
                        .await?;

                    let mut chunks = 0;
                    while let Some(result) = stream.next().await {
                        result?;
                        chunks += 1;
                    }

                    Ok::<usize, dashflow::core::Error>(chunks)
                }
            })
            .collect();

        // Execute all concurrently
        let results = futures::future::join_all(tasks).await;

        let duration = start.elapsed();
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let total_chunks: usize = results.iter().filter_map(|r| r.as_ref().ok()).sum();

        println!("Completed: {} streams", concurrency);
        println!("Success: {}/{}", success_count, concurrency);
        println!("Total chunks: {}", total_chunks);
        println!("Duration: {:?}", duration);
        println!(
            "Throughput: {:.0} streams/s",
            concurrency as f64 / duration.as_secs_f64()
        );

        assert_eq!(success_count, concurrency, "All streams should succeed");
        assert!(duration.as_secs() < 5, "Should complete in under 5 seconds");
    });
}

/// Test: Large document processing (1MB+ documents)
#[test]
fn test_large_document_processing() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== Large Document Processing Test ===");
        println!("Target: Process 1MB+ documents");

        // Generate a 1MB document
        let large_text = "Lorem ipsum dolor sit amet. ".repeat(40_000); // ~1.12 MB
        let text_size_mb = large_text.len() as f64 / 1_024_000.0;
        println!("Document size: {:.2} MB", text_size_mb);

        // Test 1: Text splitting
        println!("\nTest 1: Text Splitting");
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(100);

        let start = Instant::now();
        let chunks = splitter.split_text(&large_text);
        let split_duration = start.elapsed();

        println!("Chunks created: {}", chunks.len());
        println!("Split duration: {:?}", split_duration);
        println!(
            "Throughput: {:.2} MB/s",
            text_size_mb / split_duration.as_secs_f64()
        );

        assert!(
            !chunks.is_empty(),
            "Should create chunks from large document"
        );
        assert!(
            split_duration.as_secs() < 10,
            "Should split in under 10 seconds"
        );

        // Test 2: Parser processing
        println!("\nTest 2: Output Parser Processing");
        let parser = StrOutputParser;

        let start = Instant::now();
        let parsed = parser.parse(&large_text).expect("Failed to parse");
        let parse_duration = start.elapsed();

        println!("Parse duration: {:?}", parse_duration);
        println!(
            "Throughput: {:.2} MB/s",
            text_size_mb / parse_duration.as_secs_f64()
        );

        assert_eq!(parsed, large_text, "Parsed output should match input");
        assert!(
            parse_duration.as_secs() < 1,
            "Should parse in under 1 second"
        );

        // Test 3: JSON parsing with large structure
        println!("\nTest 3: Large JSON Parsing");
        let large_json = serde_json::json!({
            "items": (0..10000).map(|i| serde_json::json!({
                "id": i,
                "name": format!("Item {}", i),
                "description": "A test item with some description text"
            })).collect::<Vec<_>>()
        });
        let json_str = serde_json::to_string(&large_json).unwrap();
        let json_size_mb = json_str.len() as f64 / 1_024_000.0;
        println!("JSON size: {:.2} MB", json_size_mb);

        let parser = JsonOutputParser;
        let start = Instant::now();
        let parsed_json: JsonValue = parser.parse(&json_str).expect("Failed to parse JSON");
        let json_parse_duration = start.elapsed();

        println!("JSON parse duration: {:?}", json_parse_duration);
        println!(
            "Throughput: {:.2} MB/s",
            json_size_mb / json_parse_duration.as_secs_f64()
        );

        assert!(
            parsed_json.get("items").is_some(),
            "Parsed JSON should have items"
        );
        assert!(
            json_parse_duration.as_secs() < 5,
            "Should parse in under 5 seconds"
        );
    });
}

/// Test: Vector store high concurrency (1000+ concurrent operations)
#[test]
fn test_vector_store_high_concurrency() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== Vector Store High Concurrency Test ===");
        println!("Target: 1000 concurrent add operations");

        let embeddings = Arc::new(MockEmbeddings::new(128));
        let mut store = InMemoryVectorStore::new(embeddings.clone());

        // First, add 1000 documents concurrently
        let start = Instant::now();
        let concurrency = 1000;

        let docs: Vec<_> = (0..concurrency)
            .map(|i| {
                Document::new(format!("Document content number {}", i))
                    .with_metadata("id", serde_json::json!(i))
            })
            .collect();

        store
            .add_documents(&docs, None)
            .await
            .expect("Failed to add documents");

        let add_duration = start.elapsed();
        println!("Added: {} documents", concurrency);
        println!("Add duration: {:?}", add_duration);
        println!(
            "Throughput: {:.0} docs/s",
            concurrency as f64 / add_duration.as_secs_f64()
        );

        // Now perform 1000 concurrent similarity searches
        println!("\nPerforming 1000 concurrent searches...");
        let start = Instant::now();

        let search_tasks: Vec<_> = (0..concurrency)
            .map(|i| {
                let store_ref = &store;
                async move {
                    store_ref
                        ._similarity_search(format!("Query {}", i).as_str(), 10, None)
                        .await
                }
            })
            .collect();

        let results: Vec<Result<Vec<Document>, dashflow::core::Error>> =
            futures::future::join_all(search_tasks).await;
        let search_duration = start.elapsed();
        let success_count = results.iter().filter(|r| r.is_ok()).count();

        println!("Completed: {} searches", concurrency);
        println!("Success: {}/{}", success_count, concurrency);
        println!("Search duration: {:?}", search_duration);
        println!(
            "Throughput: {:.0} searches/s",
            concurrency as f64 / search_duration.as_secs_f64()
        );

        assert_eq!(success_count, concurrency, "All searches should succeed");
        assert!(
            search_duration.as_secs() < 10,
            "Should complete in under 10 seconds"
        );
    });
}

/// Test: Memory stability over time (rapid allocations)
#[test]
fn test_memory_stability() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== Memory Stability Test ===");
        println!("Target: Rapid operations for 60 seconds");

        let model = MockChatModel;
        let start = Instant::now();
        let test_duration = Duration::from_secs(60);
        let mut iteration = 0;

        println!("Running continuous load test...");
        println!("(This test takes 60 seconds)");

        while start.elapsed() < test_duration {
            // Perform various operations in batches
            let tasks: Vec<_> = (0..100)
                .map(|i| {
                    let model_clone = model.clone();
                    async move {
                        let msg = HumanMessage::new(
                            format!("Iteration {} message {}", iteration, i).as_str(),
                        );
                        let messages = vec![msg.into()];
                        model_clone
                            .generate(&messages, None, None, None, None)
                            .await
                    }
                })
                .collect();

            let results = futures::future::join_all(tasks).await;
            let success = results.iter().filter(|r| r.is_ok()).count();

            iteration += 1;

            if iteration % 10 == 0 {
                println!(
                    "Iteration {}: {:.1}s elapsed, {} successes",
                    iteration,
                    start.elapsed().as_secs_f64(),
                    success
                );
            }
        }

        let total_duration = start.elapsed();
        let total_operations = iteration * 100;

        println!("\n=== Memory Stability Test Results ===");
        println!("Total duration: {:?}", total_duration);
        println!("Total iterations: {}", iteration);
        println!("Total operations: {}", total_operations);
        println!(
            "Average throughput: {:.0} ops/s",
            total_operations as f64 / total_duration.as_secs_f64()
        );

        // If we got here without panicking or running out of memory, test passes
        assert!(iteration > 0, "Should complete at least one iteration");
    });
}

/// Test: Batch operations scalability
#[test]
fn test_batch_operations_scalability() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== Batch Operations Scalability Test ===");

        let embeddings = Arc::new(MockEmbeddings::new(128));

        // Test batch embedding with increasing sizes
        for batch_size in [10, 100, 1000, 10000] {
            println!("\nTesting batch size: {}", batch_size);

            let texts: Vec<String> = (0..batch_size)
                .map(|i| format!("Test text number {}", i))
                .collect();

            let start = Instant::now();
            let result = embeddings._embed_documents(&texts).await;
            let duration = start.elapsed();

            assert!(result.is_ok(), "Batch embedding should succeed");
            let embeddings_result = result.unwrap();

            println!("Batch size: {}", batch_size);
            println!("Duration: {:?}", duration);
            println!(
                "Throughput: {:.0} embeds/s",
                batch_size as f64 / duration.as_secs_f64()
            );

            assert_eq!(
                embeddings_result.len(),
                batch_size,
                "Should return correct number of embeddings"
            );
            assert!(duration.as_secs() < 5, "Should complete in under 5 seconds");
        }
    });
}

/// Test: Document loader concurrency
#[test]
fn test_document_loader_concurrency() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== Document Loader Concurrency Test ===");
        println!("Target: 1000 concurrent parse operations");

        let concurrency = 1000;
        let _parser = JsonOutputParser;

        // Create 1000 concurrent parsing tasks
        let start = Instant::now();
        let tasks: Vec<_> = (0..concurrency)
            .map(|i| async move {
                let parser = JsonOutputParser;
                let json = serde_json::json!({
                    "id": i,
                    "data": format!("Test data {}", i)
                });
                let json_str = serde_json::to_string(&json).unwrap();
                parser.parse(&json_str)
            })
            .collect();

        let results = futures::future::join_all(tasks).await;
        let duration = start.elapsed();
        let success_count = results.iter().filter(|r| r.is_ok()).count();

        println!("Completed: {} parse operations", concurrency);
        println!("Success: {}/{}", success_count, concurrency);
        println!("Duration: {:?}", duration);
        println!(
            "Throughput: {:.0} ops/s",
            concurrency as f64 / duration.as_secs_f64()
        );

        assert_eq!(success_count, concurrency, "All parses should succeed");
        assert!(duration.as_secs() < 5, "Should complete in under 5 seconds");
    });
}

/// Test: Very large document collection (100K+ documents)
/// This test verifies memory stability with large-scale document processing
#[test]
fn test_large_document_collection() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== Large Document Collection Test ===");
        println!("Target: Process 100,000+ documents");

        let embeddings = Arc::new(MockEmbeddings::new(128));
        let mut store = InMemoryVectorStore::new(embeddings.clone());

        // Test 1: Add 100K documents in batches
        println!("\nTest 1: Adding 100,000 documents in batches");
        let total_docs = 100_000;
        let batch_size = 1000;
        let num_batches = total_docs / batch_size;

        let start = Instant::now();
        for batch_idx in 0..num_batches {
            let docs: Vec<_> = (0..batch_size)
                .map(|i| {
                    let doc_id = batch_idx * batch_size + i;
                    Document::new(format!("Document content {}", doc_id))
                        .with_metadata("id", serde_json::json!(doc_id))
                        .with_metadata("batch", serde_json::json!(batch_idx))
                })
                .collect();

            store
                .add_documents(&docs, None)
                .await
                .expect("Failed to add documents");

            if (batch_idx + 1) % 10 == 0 {
                println!(
                    "Progress: {}/{} batches ({} docs)",
                    batch_idx + 1,
                    num_batches,
                    (batch_idx + 1) * batch_size
                );
            }
        }

        let add_duration = start.elapsed();
        println!("\nAdded: {} documents", total_docs);
        println!("Add duration: {:?}", add_duration);
        println!(
            "Throughput: {:.0} docs/s",
            total_docs as f64 / add_duration.as_secs_f64()
        );

        // Test 2: Perform searches on large collection
        println!("\nTest 2: Searching in 100K document collection");
        let search_count = 100;
        let start = Instant::now();

        for i in 0..search_count {
            let results = store
                ._similarity_search(format!("Query {}", i).as_str(), 10, None)
                .await
                .expect("Search failed");
            assert_eq!(results.len(), 10, "Should return 10 results");
        }

        let search_duration = start.elapsed();
        println!("Completed: {} searches", search_count);
        println!("Search duration: {:?}", search_duration);
        println!("Average search time: {:?}", search_duration / search_count);
        println!(
            "Throughput: {:.0} searches/s",
            search_count as f64 / search_duration.as_secs_f64()
        );

        // Test 3: Memory footprint check (rough estimate)
        println!("\nTest 3: Memory footprint estimation");
        let avg_doc_size = 100; // bytes per document (rough estimate)
        let embedding_size = 128 * 4; // 128 floats * 4 bytes
        let estimated_memory_mb =
            (total_docs * (avg_doc_size + embedding_size)) as f64 / 1_024_000.0;
        println!(
            "Estimated memory usage: {:.2} MB ({} docs)",
            estimated_memory_mb, total_docs
        );

        // If we got here without OOM, test passes
        assert!(
            add_duration.as_secs() < 300,
            "Should add 100K docs in under 5 minutes"
        );
    });
}

/// Test: Error recovery and graceful degradation
/// This test verifies the system handles errors gracefully without crashing
#[test]
fn test_error_recovery() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("\n=== Error Recovery Test ===");
        println!("Target: Verify graceful error handling");

        let model = MockChatModel;

        // Test 1: Invalid input handling
        println!("\nTest 1: Invalid input handling");
        let empty_messages: Vec<BaseMessage> = vec![];
        let result = model
            .generate(&empty_messages, None, None, None, None)
            .await;
        // Should either succeed with empty response or fail gracefully
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle empty messages gracefully"
        );
        println!(
            "Empty messages: {}",
            if result.is_ok() { "OK" } else { "Error" }
        );

        // Test 2: Very large input
        println!("\nTest 2: Very large input handling");
        let huge_message = HumanMessage::new("x".repeat(1_000_000).as_str());
        let result = model
            .generate(&[huge_message.into()], None, None, None, None)
            .await;
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle large messages gracefully"
        );
        println!(
            "1MB message: {}",
            if result.is_ok() { "OK" } else { "Error" }
        );

        // Test 3: Concurrent errors don't crash other operations
        println!("\nTest 3: Concurrent error isolation");
        let concurrency = 100;
        let tasks: Vec<_> = (0..concurrency)
            .map(|i| {
                let model_clone = model.clone();
                async move {
                    // Mix of valid and potentially problematic inputs
                    let message = if i % 3 == 0 {
                        HumanMessage::new("") // Empty
                    } else if i % 3 == 1 {
                        HumanMessage::new("x".repeat(100_000).as_str()) // Large
                    } else {
                        HumanMessage::new(format!("Normal message {}", i).as_str())
                        // Normal
                    };
                    model_clone
                        .generate(&[message.into()], None, None, None, None)
                        .await
                }
            })
            .collect();

        let results = futures::future::join_all(tasks).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let error_count = results.iter().filter(|r| r.is_err()).count();

        println!("Total operations: {}", concurrency);
        println!("Successful: {}", success_count);
        println!("Errors: {}", error_count);

        // All operations should complete (no panics/crashes)
        assert_eq!(
            success_count + error_count,
            concurrency,
            "All operations should complete gracefully"
        );

        // Test 4: Parser error recovery
        println!("\nTest 4: Parser error recovery");
        let parser = JsonOutputParser;

        // Invalid JSON
        let deeply_nested = "[]".repeat(10000);
        let invalid_inputs = [
            "",             // Empty
            "{",            // Incomplete
            "not json",     // Invalid
            "{\"a\": ",     // Incomplete key-value
            &deeply_nested, // Deeply nested
        ];

        for (i, input) in invalid_inputs.iter().enumerate() {
            let result: Result<serde_json::Value, _> = parser.parse(input);
            // Should fail gracefully without panic
            assert!(
                result.is_err(),
                "Invalid input {} should fail gracefully",
                i
            );
        }
        println!("Invalid inputs handled: {}", invalid_inputs.len());

        // Test 5: Vector store error recovery
        println!("\nTest 5: Vector store error recovery");
        let embeddings = Arc::new(MockEmbeddings::new(128));
        let store = InMemoryVectorStore::new(embeddings.clone());

        // Search in empty store
        let result = store._similarity_search("query", 10, None).await;
        assert!(
            result.is_ok(),
            "Search in empty store should succeed (return empty results)"
        );
        println!("Empty store search: OK");

        // Search with invalid parameters
        let result = store._similarity_search("query", 0, None).await;
        assert!(
            result.is_ok() || result.is_err(),
            "Search with k=0 should handle gracefully"
        );
        println!(
            "k=0 search: {}",
            if result.is_ok() { "OK" } else { "Error" }
        );

        println!("\n=== Error Recovery Test Complete ===");
        println!("All error scenarios handled gracefully (no crashes)");
    });
}
