/// Example demonstrating MarkdownHeaderTextSplitter's metadata extraction
///
/// This example shows how MarkdownHeaderTextSplitter tracks header hierarchy
/// and adds header text as metadata to each document chunk. This is useful
/// for RAG systems where knowing the document structure improves retrieval.
use dashflow_text_splitters::MarkdownHeaderTextSplitter;

fn main() {
    println!("=== Markdown Header Text Splitter Demo ===\n");

    // Define which headers to track and what to call them in metadata
    let headers_to_split_on = vec![
        ("#".to_string(), "H1".to_string()),
        ("##".to_string(), "H2".to_string()),
        ("###".to_string(), "H3".to_string()),
    ];

    let splitter = MarkdownHeaderTextSplitter::new(headers_to_split_on);

    // Example markdown document with nested headers
    let markdown = r#"# Introduction to Rust

Rust is a systems programming language focused on safety, speed, and concurrency.

## Memory Safety

Rust achieves memory safety without garbage collection through its ownership system.

### Ownership Rules

1. Each value has a variable called its owner
2. There can only be one owner at a time
3. When the owner goes out of scope, the value is dropped

## Performance

Rust provides zero-cost abstractions, meaning the abstractions don't impose runtime overhead.

### Benchmarks

Rust typically matches or beats C++ performance in most benchmarks.

# Advanced Topics

## Unsafe Rust

When you need to opt out of Rust's safety guarantees, you can use unsafe code.

### Use Cases

Unsafe code is needed for:
- Interfacing with C libraries (FFI)
- Implementing low-level data structures
- Inline assembly
"#;

    println!("Original Markdown:\n{}\n", markdown);
    println!("{}", "=".repeat(70));

    // Split the markdown into documents with header metadata
    let documents = splitter.split_text(markdown);

    println!(
        "\nExtracted {} documents with header metadata:\n",
        documents.len()
    );

    for (i, doc) in documents.iter().enumerate() {
        println!("Document {}:", i + 1);
        println!("  Metadata:");
        if let Some(h1) = doc.metadata.get("H1") {
            println!("    H1: {}", h1.as_str().unwrap_or("N/A"));
        }
        if let Some(h2) = doc.metadata.get("H2") {
            println!("    H2: {}", h2.as_str().unwrap_or("N/A"));
        }
        if let Some(h3) = doc.metadata.get("H3") {
            println!("    H3: {}", h3.as_str().unwrap_or("N/A"));
        }

        println!("  Content ({} chars):", doc.page_content.len());
        // Show first 80 chars of content
        let preview = if doc.page_content.len() > 80 {
            format!("{}...", &doc.page_content[..80])
        } else {
            doc.page_content.clone()
        };
        println!("    {}\n", preview.replace('\n', "\n    "));
    }

    println!("{}", "=".repeat(70));
    println!("\nHeader Hierarchy Tracking:");
    println!("- Each document knows which headers it appears under");
    println!("- H1 'Introduction to Rust' applies to first set of chunks");
    println!("- When we hit '# Advanced Topics', H1 changes for subsequent chunks");
    println!("- H2 and H3 nest properly under their parent headers");
    println!("\nUse Case for RAG:");
    println!("- When retrieving chunks, metadata provides document structure context");
    println!("- LLM can understand where content came from in the document hierarchy");
    println!("- Improves answer quality by providing section/subsection information");
}
