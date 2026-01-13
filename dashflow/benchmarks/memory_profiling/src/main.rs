// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

use dashflow::core::messages::{AIMessage, HumanMessage, Message};
use dashflow_text_splitters::{
    CharacterTextSplitter, RecursiveCharacterTextSplitter, TextSplitter,
};

/// Memory benchmark for `DashFlow` Rust implementation
///
/// Performs typical `DashFlow` operations to measure memory footprint:
/// - Message creation and cloning
/// - Text splitting
/// - Serialization
///
/// Run with: /usr/bin/time -l ./`memory_bench`
fn main() {
    println!("=== DashFlow Rust Memory Benchmark ===\n");

    // Allocate collections to hold data (prevent premature deallocation)
    let mut messages: Vec<Message> = Vec::new();
    let mut serialized = Vec::new();
    let mut splits = Vec::new();
    let mut formatted_strings = Vec::new();

    // 1. Message operations (2000 messages)
    println!("Creating 2000 messages...");
    for i in 0..1000 {
        messages.push(
            HumanMessage::new(format!(
                "Message {i}: This is a test message with some content."
            ))
            .into(),
        );
        messages.push(AIMessage::new(format!("Response {i}: This is an AI response.")).into());
    }

    // 2. Message cloning (test Arc efficiency)
    println!("Cloning 2000 messages...");
    let cloned_messages = messages.clone();
    assert_eq!(messages.len(), cloned_messages.len());

    // 3. Serialization (2000 messages)
    println!("Serializing 2000 messages...");
    for msg in &messages {
        serialized.push(serde_json::to_string(msg).unwrap());
    }

    // 4. String formatting (1000 renders simulating prompt templates)
    println!("Formatting 1000 strings...");
    for i in 0..1000 {
        formatted_strings.push(format!("Hello User_{i}, your ID is {i}!"));
    }

    // 5. Text splitting (100 documents)
    println!("Splitting 100 documents...");
    let doc = "This is a test document. It contains multiple sentences. \
               We will split it in various ways. This helps us measure memory usage. \
               The document is long enough to create multiple chunks. \
               Each chunk will be stored separately. "
        .repeat(10);

    let char_splitter = CharacterTextSplitter::new()
        .with_chunk_size(100)
        .with_chunk_overlap(20)
        .with_separator("\n\n");
    let rec_splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(100)
        .with_chunk_overlap(20);

    for _ in 0..50 {
        splits.extend(char_splitter.split_text(&doc));
        splits.extend(rec_splitter.split_text(&doc));
    }

    // 6. Additional string processing (1000 operations)
    println!("Processing 1000 string transformations...");
    let mut results = Vec::new();
    for i in 0..1000 {
        let input = format!("input_{i}");
        results.push(format!("Processed: {input}"));
    }

    // 7. Additional message operations (1000 more operations)
    println!("Creating additional 1000 messages for memory pressure...");
    let mut additional_messages: Vec<Message> = Vec::new();
    for i in 0..1000 {
        additional_messages.push(HumanMessage::new(format!("Additional message {i}")).into());
    }

    // Print summary statistics
    println!("\n=== Summary ===");
    println!("Messages created: {}", messages.len());
    println!("Messages cloned: {}", cloned_messages.len());
    println!("Serialized strings: {}", serialized.len());
    println!("Formatted strings: {}", formatted_strings.len());
    println!("Text splits: {}", splits.len());
    println!("String processing results: {}", results.len());
    println!("Additional messages: {}", additional_messages.len());
    println!("\nMemory stats will be printed by /usr/bin/time -l");
}
