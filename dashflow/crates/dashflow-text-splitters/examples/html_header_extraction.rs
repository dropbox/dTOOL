//! HTML Header Text Splitter Example
//!
//! This example demonstrates the HTMLHeaderTextSplitter, which extracts HTML header hierarchy
//! (h1, h2, h3, etc.) as document metadata. This is useful for RAG systems where maintaining
//! document structure context improves retrieval quality.
//!
//! Run with: cargo run --example html_header_extraction

use dashflow_text_splitters::HTMLHeaderTextSplitter;

fn main() {
    println!("=== HTML Header Text Splitter Example ===\n");

    // Create HTML Header Text Splitter
    let splitter = HTMLHeaderTextSplitter::new(vec![
        ("h1".to_string(), "Header 1".to_string()),
        ("h2".to_string(), "Header 2".to_string()),
        ("h3".to_string(), "Header 3".to_string()),
    ]);

    // Example HTML document with hierarchical headers
    let html = r#"
    <html>
        <body>
            <h1>Introduction to Rust</h1>
            <p>Rust is a systems programming language that focuses on safety, speed, and concurrency.</p>

            <h2>Memory Safety</h2>
            <p>Rust achieves memory safety without garbage collection through its ownership system.</p>

            <h3>Ownership Rules</h3>
            <p>Each value has a variable called its owner. There can only be one owner at a time.
            When the owner goes out of scope, the value is dropped.</p>

            <h3>Borrowing</h3>
            <p>Borrowing allows you to reference a value without taking ownership. References must
            always be valid.</p>

            <h2>Concurrency</h2>
            <p>Rust's ownership system makes it easier to write concurrent code safely.</p>

            <h1>Getting Started</h1>
            <p>To get started with Rust, install the Rust toolchain using rustup.</p>

            <h2>Installation</h2>
            <p>Visit https://rustup.rs and follow the installation instructions for your platform.</p>
        </body>
    </html>
    "#;

    // Split the HTML content
    let documents = splitter.split_text(html);

    println!("Split HTML into {} documents:\n", documents.len());

    // Display each document with its metadata
    for (i, doc) in documents.iter().enumerate() {
        println!("Document {}:", i + 1);

        // Display metadata (header hierarchy)
        if !doc.metadata.is_empty() {
            println!("  Metadata:");
            if let Some(h1) = doc.metadata.get("Header 1") {
                println!("    Header 1: {}", h1);
            }
            if let Some(h2) = doc.metadata.get("Header 2") {
                println!("    Header 2: {}", h2);
            }
            if let Some(h3) = doc.metadata.get("Header 3") {
                println!("    Header 3: {}", h3);
            }
        }

        // Display content (truncated if long)
        let content = &doc.page_content;
        if content.len() > 100 {
            println!(
                "  Content ({} chars): {}...",
                content.len(),
                &content[..100]
            );
        } else {
            println!("  Content ({} chars): {}", content.len(), content);
        }
        println!();
    }

    // Example: Find all content under "Memory Safety"
    println!("--- Content under 'Memory Safety' ---\n");
    let memory_safety_docs: Vec<_> = documents
        .iter()
        .filter(|d| d.metadata.get("Header 2").and_then(|v| v.as_str()) == Some("Memory Safety"))
        .collect();

    for doc in memory_safety_docs {
        let h3 = doc
            .metadata
            .get("Header 3")
            .and_then(|v| v.as_str())
            .unwrap_or("[no subsection]");
        println!("  [{}]: {}", h3, doc.page_content);
    }

    // Example: return_each_element mode
    println!("\n--- With return_each_element=true ---\n");

    let splitter_individual = HTMLHeaderTextSplitter::new(vec![
        ("h1".to_string(), "H1".to_string()),
        ("h2".to_string(), "H2".to_string()),
    ])
    .with_return_each_element(true);

    let simple_html = r#"
    <body>
        <h1>Chapter 1</h1>
        <p>First paragraph.</p>
        <p>Second paragraph.</p>
        <h2>Section 1.1</h2>
        <p>Section content.</p>
    </body>
    "#;

    let individual_docs = splitter_individual.split_text(simple_html);
    println!("Split into {} individual elements:", individual_docs.len());

    for (i, doc) in individual_docs.iter().enumerate() {
        let h1 = doc
            .metadata
            .get("H1")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let h2 = doc
            .metadata
            .get("H2")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        println!(
            "  {}: [H1: {}, H2: {}] {}",
            i + 1,
            h1,
            h2,
            doc.page_content.trim()
        );
    }

    println!("\n--- Use Case: RAG with Structured Context ---");
    println!("\nWhen retrieving chunks for RAG, header metadata provides structure:");
    println!("  - 'What are ownership rules?' → retrieves chunks with H3='Ownership Rules'");
    println!("  - 'How does Rust handle concurrency?' → retrieves chunks with H2='Concurrency'");
    println!("  - Header context helps LLM understand document structure when answering");
}
