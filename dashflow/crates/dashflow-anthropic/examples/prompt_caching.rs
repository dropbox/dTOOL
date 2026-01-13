// Anthropic Prompt Caching Example
//
// Run with: cargo run -p dashflow-anthropic --example prompt_caching
//
// Required environment variable:
// - ANTHROPIC_API_KEY: Your Anthropic API key
//
// Prompt caching reduces costs by up to 90% for repeated context.
// Best for:
// - Large system prompts or documents (>1024 tokens)
// - Repeated API calls with similar context
// - Interactive applications with persistent context

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_anthropic::ChatAnthropic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Anthropic Prompt Caching Example ===\n");

    let chat = ChatAnthropic::try_new()?
        .with_model("claude-3-5-sonnet-20241022") // Prompt caching supported
        .with_max_tokens(1024);

    // Scenario: We have a large knowledge base document that we want to query multiple times
    let knowledge_base = r#"
    # Rust Programming Language Guide

    ## Overview
    Rust is a systems programming language focused on safety, concurrency, and performance.
    It achieves memory safety without garbage collection through its ownership system.

    ## Key Features
    1. **Ownership System**: Each value has a single owner, preventing memory leaks and data races
    2. **Borrowing**: Values can be borrowed immutably (multiple times) or mutably (exclusively)
    3. **Lifetimes**: Compiler ensures references are always valid
    4. **Zero-Cost Abstractions**: High-level code compiles to efficient machine code
    5. **Fearless Concurrency**: Ownership system prevents data races at compile time

    ## Memory Safety
    Rust provides memory safety guarantees without runtime overhead:
    - No null pointers (Option<T> instead)
    - No buffer overflows (bounds checking)
    - No use-after-free (ownership tracking)
    - No data races (ownership + borrowing rules)

    ## Type System
    Rust has a strong, static type system with:
    - Algebraic data types (enums with variants)
    - Pattern matching for exhaustive case analysis
    - Generic types with trait bounds
    - Associated types and lifetime parameters

    ## Concurrency
    Common concurrency primitives:
    - Threads: std::thread for OS threads
    - Async/await: tokio, async-std for async I/O
    - Channels: std::sync::mpsc for message passing
    - Mutexes and RwLocks: std::sync for shared state

    ## Performance
    Rust compiles to native machine code via LLVM, achieving:
    - C/C++ level performance
    - No garbage collection pauses
    - Minimal runtime overhead
    - Efficient memory layout control

    ## Ecosystem
    Key tools and crates:
    - Cargo: Build system and package manager
    - crates.io: Package registry (50,000+ crates)
    - serde: Serialization/deserialization
    - tokio: Async runtime
    - actix/axum: Web frameworks
    "#
    .trim();

    println!("📄 Knowledge base: {} characters\n", knowledge_base.len());

    // NOTE: In the current implementation, cache control is handled internally
    // by the Anthropic API. The CacheControl struct exists for future API enhancements.
    //
    // For now, Anthropic automatically caches repeated content based on:
    // 1. Content must be >1024 tokens
    // 2. Content must be identical across requests
    // 3. Requests must be within 5 minutes of each other

    // First query - This will NOT use cache (first time)
    println!("Query 1: What are the key memory safety features in Rust?");
    println!("(This request builds the cache...)\n");

    let messages1 = vec![
        Message::system(knowledge_base),
        Message::human("What are the key memory safety features in Rust? Be concise."),
    ];

    let result1 = chat.generate(&messages1, None, None, None, None).await?;
    println!("Response: {}\n", result1.generations[0].message.as_text());

    // Print token usage
    if let Some(usage) = &result1.generations[0].generation_info {
        if let Some(input_tokens) = usage.get("input_tokens") {
            println!("✓ Input tokens: {}", input_tokens);
        }
        if let Some(output_tokens) = usage.get("output_tokens") {
            println!("✓ Output tokens: {}", output_tokens);
        }
        if let Some(cache_creation) = usage.get("cache_creation_input_tokens") {
            println!("💾 Cache created: {} tokens", cache_creation);
        }
    }

    println!("\n{}\n", "=".repeat(60));

    // Second query - This WILL use cache (same system message)
    println!("Query 2: Explain Rust's concurrency model.");
    println!("(This request should use the cached context...)\n");

    let messages2 = vec![
        Message::system(knowledge_base), // Same content = cache hit!
        Message::human("Explain Rust's concurrency model. Be concise."),
    ];

    let result2 = chat.generate(&messages2, None, None, None, None).await?;
    println!("Response: {}\n", result2.generations[0].message.as_text());

    // Print token usage - should show cache_read_input_tokens
    if let Some(usage) = &result2.generations[0].generation_info {
        if let Some(input_tokens) = usage.get("input_tokens") {
            println!("✓ Input tokens: {}", input_tokens);
        }
        if let Some(output_tokens) = usage.get("output_tokens") {
            println!("✓ Output tokens: {}", output_tokens);
        }
        if let Some(cache_read) = usage.get("cache_read_input_tokens") {
            println!("💰 Cache hit: {} tokens (90% cost savings!)", cache_read);
        }
    }

    println!("\n✅ Prompt caching example complete\n");
    println!("💡 Key Takeaways:");
    println!("  • Prompt caching reduces costs by 90% for cached tokens");
    println!("  • Minimum 1024 tokens required for caching");
    println!("  • Cache persists for 5 minutes");
    println!("  • Perfect for repeated queries against large context");
    println!("  • Works with system messages, documents, and tool definitions");

    Ok(())
}
