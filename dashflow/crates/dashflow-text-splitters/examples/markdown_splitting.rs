//! Example demonstrating MarkdownTextSplitter
//!
//! This example shows how to split Markdown documents while preserving structure.

use dashflow_text_splitters::{MarkdownTextSplitter, TextSplitter};

fn main() {
    // Create a MarkdownTextSplitter with custom configuration
    let splitter = MarkdownTextSplitter::new()
        .with_chunk_size(200)
        .with_chunk_overlap(50);

    // Sample markdown document
    let markdown = r#"# Introduction to Rust

Rust is a systems programming language that focuses on safety, speed, and concurrency.

## Memory Safety

Rust's ownership system guarantees memory safety without needing a garbage collector.

### The Borrow Checker

The borrow checker ensures that references are always valid.

## Concurrency

Rust makes it easy to write concurrent code that is safe and efficient.

```rust
use std::thread;

fn main() {
    let handle = thread::spawn(|| {
        println!("Hello from a thread!");
    });
    handle.join().unwrap();
}
```

## Performance

Rust provides zero-cost abstractions, meaning you don't pay for features you don't use.
"#;

    println!(
        "Original Markdown ({} characters):\n{}\n",
        markdown.len(),
        markdown
    );
    println!("{}", "=".repeat(80));

    // Split the markdown
    let chunks = splitter.split_text(markdown);

    println!("\nSplit into {} chunks:\n", chunks.len());

    for (i, chunk) in chunks.iter().enumerate() {
        println!("--- Chunk {} ({} characters) ---", i + 1, chunk.len());
        println!("{}", chunk);
        println!();
    }
}
