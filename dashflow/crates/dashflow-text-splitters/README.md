# dashflow-text-splitters

Text splitting utilities for DashFlow - efficiently split documents into chunks for RAG pipelines, embeddings, and context window management.

## Overview

Text splitters are essential for processing large documents that exceed LLM context windows. This crate provides multiple splitting strategies optimized for different content types:

- **CharacterTextSplitter** - Split on a single separator (e.g., "\n\n")
- **RecursiveCharacterTextSplitter** - Recursively split on multiple separators to keep related content together
- **MarkdownTextSplitter** - Specialized splitter preserving Markdown structure
- **HTMLTextSplitter** - Specialized splitter preserving HTML tag structure
- **MarkdownHeaderTextSplitter** - Split Markdown with automatic header metadata extraction
- **HTMLHeaderTextSplitter** - Split HTML with automatic header metadata extraction
- **Language-Specific Splitting** - Code splitting for Python, Rust, JavaScript, TypeScript, Java, Go, and more

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-text-splitters = "1.11"
```

## Quick Start

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

// Create a splitter with chunk size and overlap
let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(50);

let text = "Your long document here...";
let chunks = splitter.split_text(text);

// Use chunks for RAG, embeddings, or batch processing
for chunk in chunks {
    println!("{}", chunk);
}
```

## Core Trait: TextSplitter

All text splitters implement the `TextSplitter` trait:

```rust
pub trait TextSplitter {
    /// Split text into chunks
    fn split_text(&self, text: &str) -> Vec<String>;

    /// Create documents from texts with optional metadata
    fn create_documents(
        &self,
        texts: &[impl AsRef<str>],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
    ) -> Vec<Document>;

    /// Split documents into smaller chunks
    fn split_documents(&self, documents: &[Document]) -> Vec<Document>;

    /// Get chunk size configuration
    fn chunk_size(&self) -> usize;

    /// Get chunk overlap configuration
    fn chunk_overlap(&self) -> usize;

    /// Whether to add start_index to metadata
    fn add_start_index(&self) -> bool;
}
```

## Splitter Types

### 1. CharacterTextSplitter

Splits text on a single separator. Best for simple documents with clear delimiters.

```rust
use dashflow_text_splitters::{CharacterTextSplitter, TextSplitter};

let splitter = CharacterTextSplitter::new()
    .with_chunk_size(100)
    .with_chunk_overlap(20)
    .with_separator("\n\n");  // Split on paragraph breaks

let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
let chunks = splitter.split_text(text);
```

**Configuration:**
- `chunk_size` - Maximum characters per chunk (default: 1000)
- `chunk_overlap` - Characters to overlap between chunks (default: 200)
- `separator` - String to split on (default: "\n\n")
- `keep_separator` - Where to keep separator: `False`, `Start`, or `End` (default: `False`)

**Use Cases:**
- Simple text documents
- Documents with consistent paragraph structure
- When you know the exact separator pattern

**Example:** `examples/basic_splitting.rs`

### 2. RecursiveCharacterTextSplitter

Recursively tries multiple separators to keep related content together. **Recommended for most use cases.**

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(150)
    .with_chunk_overlap(30);

// Tries separators in order: "\n\n", "\n", " ", ""
let chunks = splitter.split_text(text);
```

**How it works:**
1. Tries to split on double newlines (`\n\n`) first
2. Falls back to single newlines (`\n`)
3. Falls back to spaces (` `)
4. Finally splits on individual characters if needed

**Configuration:**
- `chunk_size` - Maximum characters per chunk (default: 1000)
- `chunk_overlap` - Characters to overlap between chunks (default: 200)
- `separators` - Custom separator list (default: `["\n\n", "\n", " ", ""]`)
- `keep_separator` - Where to keep separator (default: `False`)
- `add_start_index` - Add `start_index` to metadata tracking original position (default: `false`)

**Use Cases:**
- General-purpose text splitting
- Documents with varying structure
- When you want semantic preservation
- RAG pipelines (recommended default)

**Example:** `examples/recursive_splitting.rs`

### 3. MarkdownTextSplitter

Preserves Markdown structure by splitting on Markdown-specific boundaries.

```rust
use dashflow_text_splitters::{MarkdownTextSplitter, TextSplitter};

let splitter = MarkdownTextSplitter::new()
    .with_chunk_size(200)
    .with_chunk_overlap(50);

let markdown = r#"
# Main Title

Some content here.

## Section 1

Section content.

### Subsection 1.1

More details.
"#;

let chunks = splitter.split_text(markdown);
```

**Separator Hierarchy:**
1. `\n#{1,6} ` - Headers (# to ######)
2. `\n\n` - Paragraphs
3. `\n` - Lines
4. ` ` - Words
5. `""` - Characters

**Configuration:**
- `chunk_size` - Maximum characters per chunk
- `chunk_overlap` - Characters to overlap between chunks
- Automatically uses Markdown-specific separators

**Use Cases:**
- Technical documentation
- README files
- Blog posts in Markdown
- Documentation websites

**Example:** `examples/markdown_splitting.rs`

### 4. HTMLTextSplitter

Preserves HTML structure by splitting on HTML tags.

```rust
use dashflow_text_splitters::{HTMLTextSplitter, TextSplitter};

let splitter = HTMLTextSplitter::new()
    .with_chunk_size(150)
    .with_chunk_overlap(30);

let html = r#"
<html>
<body>
    <h1>Main Title</h1>
    <p>Paragraph content.</p>
    <ul>
        <li>Item 1</li>
        <li>Item 2</li>
    </ul>
</body>
</html>
"#;

let chunks = splitter.split_text(html);
```

**Separator Hierarchy:**
1. Block-level tags: `<div>`, `<p>`, `<section>`, etc.
2. Heading tags: `<h1>` through `<h6>`
3. List tags: `<ul>`, `<ol>`, `<li>`
4. Table tags: `<table>`, `<tr>`, `<td>`, `<th>`
5. Line breaks: `<br>`, `\n\n`, `\n`
6. Words and characters

**Configuration:**
- `chunk_size` - Maximum characters per chunk
- `chunk_overlap` - Characters to overlap between chunks
- Automatically uses HTML-specific separators

**Use Cases:**
- Web scraping results
- HTML documentation
- Email content
- Blog posts in HTML

**Example:** `examples/html_splitting.rs`

### 5. MarkdownHeaderTextSplitter

Splits Markdown and extracts header hierarchy as metadata.

```rust
use dashflow_text_splitters::{MarkdownHeaderTextSplitter, TextSplitter};

let splitter = MarkdownHeaderTextSplitter::new(vec![
    ("#".to_string(), "Header 1".to_string()),
    ("##".to_string(), "Header 2".to_string()),
    ("###".to_string(), "Header 3".to_string()),
]);

let markdown = r#"
# Chapter 1
Introduction content.

## Section 1.1
Section details.

### Subsection 1.1.1
More specific content.
"#;

let documents = splitter.split_text(markdown);
// Each document has metadata: {"Header 1": "Chapter 1", "Header 2": "Section 1.1", ...}
```

**Features:**
- Extracts header hierarchy as structured metadata
- Preserves document structure in metadata fields
- Ideal for question-answering over hierarchical documents

**Use Cases:**
- Technical manuals with nested sections
- Books with chapter/section hierarchy
- Documentation with clear structure
- When header context is important for retrieval

**Example:** `examples/markdown_header_extraction.rs`

### 6. HTMLHeaderTextSplitter

Splits HTML and extracts header hierarchy as metadata.

```rust
use dashflow_text_splitters::{HTMLHeaderTextSplitter, TextSplitter};

let splitter = HTMLHeaderTextSplitter::new(vec![
    ("h1".to_string(), "Header 1".to_string()),
    ("h2".to_string(), "Header 2".to_string()),
    ("h3".to_string(), "Header 3".to_string()),
]);

let html = r#"
<h1>Chapter 1</h1>
<p>Introduction content.</p>

<h2>Section 1.1</h2>
<p>Section details.</p>

<h3>Subsection 1.1.1</h3>
<p>More specific content.</p>
"#;

let documents = splitter.split_text(html);
// Each document has metadata: {"Header 1": "Chapter 1", "Header 2": "Section 1.1", ...}
```

**Features:**
- Extracts HTML header hierarchy as structured metadata
- Preserves document structure in metadata fields
- Handles nested HTML structures

**Use Cases:**
- Web scraped content with headers
- HTML documentation
- Blog posts with section headers
- When header context is important for retrieval

**Example:** `examples/html_header_extraction.rs`

### 7. Language-Specific Code Splitting

Split source code along natural boundaries (functions, classes, control flow).

```rust
use dashflow_text_splitters::{Language, RecursiveCharacterTextSplitter, TextSplitter};

// Python code splitting
let python_splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
    .with_chunk_size(200)
    .with_chunk_overlap(20);

let python_code = r#"
class DataProcessor:
    def __init__(self, name):
        self.name = name

    def process(self):
        return self.name.upper()
"#;

let chunks = python_splitter.split_text(python_code);
```

**Supported Languages:**

| Language | Separators | Use Case |
|----------|-----------|----------|
| **Python** | `\nclass `, `\ndef `, `\n\tdef ` | Classes, functions, methods |
| **Rust** | `\nfn `, `\nconst `, `\nlet `, `\nif `, `\nwhile `, `\nfor `, `\nloop `, `\nmatch ` | Functions, control flow |
| **JavaScript** | `\nfunction `, `\nconst `, `\nlet `, `\nvar `, `\nclass `, `\nif `, `\nfor `, `\nwhile ` | Functions, classes, variables |
| **TypeScript** | JS separators + `\nenum `, `\ninterface `, `\nnamespace `, `\ntype ` | Type definitions |
| **Java** | `\nclass `, `\npublic `, `\nprotected `, `\nprivate `, `\nstatic ` | Classes, methods |
| **Go** | `\nfunc `, `\nvar `, `\nconst `, `\ntype `, `\nif `, `\nfor `, `\nswitch ` | Functions, types |
| **C++** | `\nclass `, `\nvoid `, `\nint `, `\nfloat `, `\ndouble `, `\nif `, `\nfor `, `\nwhile ` | Classes, functions |
| **Markdown** | Headers, paragraphs, lines | Documentation |
| **HTML** | Block tags, headers, lists | Web content |

**Configuration:**
```rust
let splitter = RecursiveCharacterTextSplitter::from_language(Language::Rust)
    .with_chunk_size(250)
    .with_chunk_overlap(20);

// Or get separators for custom use
let separators = Language::Python.get_separators();
```

**Use Cases:**
- RAG over codebases
- Code search and analysis
- Documentation generation
- Code understanding for LLMs
- Training code models

**Example:** `examples/code_splitting.rs` (comprehensive multi-language demo)

## Configuration Best Practices

### Chunk Size

Choose chunk size based on your use case:

| Use Case | Recommended Size | Reasoning |
|----------|------------------|-----------|
| **Embeddings** | 200-500 chars | Embedding models work best with focused content |
| **RAG retrieval** | 500-1000 chars | Balance between context and precision |
| **LLM context** | 1000-2000 chars | Larger chunks provide more context |
| **Question answering** | 300-700 chars | Focused chunks match question granularity |

### Chunk Overlap

Overlap prevents important information from being split across chunks:

```rust
// 10-20% overlap is typical
let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(50);  // 10% overlap
```

**Guidelines:**
- **Low overlap (5-10%)**: Fast processing, more chunks
- **Medium overlap (10-20%)**: Balanced (recommended)
- **High overlap (20-30%)**: Better context preservation, slower processing

### Choosing a Splitter

**Decision tree:**

1. **Is it code?** → Use `RecursiveCharacterTextSplitter::from_language(Language::*)`
2. **Is it Markdown?** → Use `MarkdownTextSplitter` or `MarkdownHeaderTextSplitter`
3. **Is it HTML?** → Use `HTMLTextSplitter` or `HTMLHeaderTextSplitter`
4. **Is it plain text with clear structure?** → Use `CharacterTextSplitter`
5. **General purpose / unsure?** → Use `RecursiveCharacterTextSplitter` (recommended default)

## Advanced Usage

### Adding Start Index to Metadata

Track where each chunk came from in the original document:

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use std::collections::HashMap;

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(100)
    .with_chunk_overlap(20)
    .with_add_start_index(true);

let texts = vec!["Long document text..."];
let documents = splitter.create_documents(&texts, None);

// Each document has metadata: {"start_index": 0}
for doc in documents {
    println!("Chunk starts at position: {}",
        doc.metadata.get("start_index").unwrap());
}
```

### Custom Separators

Create a splitter with custom separator hierarchy:

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter, KeepSeparator};

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(200)
    .with_chunk_overlap(20)
    .with_separators(vec![
        "\n\n\n".to_string(),  // Triple newline
        "\n\n".to_string(),    // Double newline
        "\n".to_string(),      // Single newline
        ". ".to_string(),      // Sentences
        " ".to_string(),       // Words
    ])
    .with_keep_separator(KeepSeparator::End);

let chunks = splitter.split_text(text);
```

### Separator Positioning

Control where separators appear in chunks:

```rust
use dashflow_text_splitters::{CharacterTextSplitter, TextSplitter, KeepSeparator};

// Keep separator at start of each chunk
let splitter = CharacterTextSplitter::new()
    .with_separator("\n")
    .with_keep_separator(KeepSeparator::Start);

// Keep separator at end of each chunk
let splitter = CharacterTextSplitter::new()
    .with_separator("\n")
    .with_keep_separator(KeepSeparator::End);

// Don't keep separator
let splitter = CharacterTextSplitter::new()
    .with_separator("\n")
    .with_keep_separator(KeepSeparator::False);
```

### Working with Documents

Split existing documents while preserving metadata:

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter, Document};
use std::collections::HashMap;

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(50);

let mut metadata = HashMap::new();
metadata.insert("source".to_string(), serde_json::json!("document.pdf"));
metadata.insert("page".to_string(), serde_json::json!(1));

let doc = Document {
    page_content: "Very long document content...".to_string(),
    metadata: metadata.clone(),
    id: None,
};

// Split while preserving metadata
let chunks = splitter.split_documents(&[doc]);
// Each chunk has the same metadata: {"source": "document.pdf", "page": 1}
```

### Batch Processing with Metadata

Process multiple texts with different metadata:

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use std::collections::HashMap;

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(50);

let texts = vec!["First document...", "Second document..."];

let mut metadata1 = HashMap::new();
metadata1.insert("source".to_string(), serde_json::json!("doc1.txt"));

let mut metadata2 = HashMap::new();
metadata2.insert("source".to_string(), serde_json::json!("doc2.txt"));

let metadatas = vec![metadata1, metadata2];

let documents = splitter.create_documents(&texts, Some(&metadatas));
// Each chunk is tagged with its source document
```

## RAG Pipeline Integration

Complete example integrating text splitting with RAG:

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use dashflow::core::documents::Document;
use std::collections::HashMap;

async fn rag_pipeline(document_text: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Split document into chunks
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(500)
        .with_chunk_overlap(50)
        .with_add_start_index(true);

    let chunks = splitter.split_text(document_text);

    // 2. Create documents with metadata
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), serde_json::json!("manual.pdf"));

    let documents = splitter.create_documents(&[document_text], Some(&[metadata]));

    // 3. Embed chunks (pseudocode)
    // let embeddings = embed_documents(&chunks).await?;

    // 4. Store in vector database (pseudocode)
    // vector_store.add_documents(&documents, &embeddings).await?;

    // 5. Query (pseudocode)
    // let query = "How do I configure the system?";
    // let results = vector_store.similarity_search(query, 5).await?;

    Ok(())
}
```

## Python to Rust Migration

DashFlow Python users will find familiar APIs:

### Python
```python
from dashflow.text_splitter import RecursiveCharacterTextSplitter

splitter = RecursiveCharacterTextSplitter(
    chunk_size=500,
    chunk_overlap=50,
    add_start_index=True
)
chunks = splitter.split_text(text)
documents = splitter.create_documents([text], metadatas=[{"source": "doc.txt"}])
```

### Rust
```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use std::collections::HashMap;

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(50)
    .with_add_start_index(true);

let chunks = splitter.split_text(text);

let mut metadata = HashMap::new();
metadata.insert("source".to_string(), serde_json::json!("doc.txt"));

let documents = splitter.create_documents(&[text], Some(&[metadata]));
```

**Key Differences:**
- Use builder pattern: `.with_chunk_size()` instead of constructor args
- Metadata is `HashMap<String, serde_json::Value>` instead of `dict`
- `Option<&[HashMap]>` for optional metadata instead of `Optional[list[dict]]`

## Running the Examples

All examples are runnable and demonstrate real-world usage:

```bash
# Basic character splitting
cargo run --example basic_splitting

# Recursive splitting (recommended starting point)
cargo run --example recursive_splitting

# Markdown splitting
cargo run --example markdown_splitting

# Markdown with header extraction
cargo run --example markdown_header_extraction

# HTML splitting
cargo run --example html_splitting

# HTML with header extraction
cargo run --example html_header_extraction

# Code splitting (multi-language demo)
cargo run --example code_splitting

# Compare with Python output
cargo run --example compare_with_python

# Test Python parity
cargo run --example text_splitter_parity
```

## Performance Characteristics

Text splitters are designed for high performance:

- **Zero-copy string operations** where possible
- **Minimal allocations** during splitting
- **Efficient separator matching** using string searching algorithms
- **Streaming support** for large documents (via iterator patterns)

**Benchmarks (typical usage):**
- Character splitting: ~10,000 chunks/sec
- Recursive splitting: ~5,000 chunks/sec (due to multiple separator attempts)
- Code splitting: ~4,000 chunks/sec (language-specific separators)

*Note: Actual performance depends on chunk size, overlap, and content characteristics.*

## Testing

The crate includes comprehensive tests:

```bash
# Run all tests
cargo test --package dashflow-text-splitters

# Run with output
cargo test --package dashflow-text-splitters -- --nocapture

# Run specific test
cargo test --package dashflow-text-splitters test_recursive_splitter
```

## Documentation

- **[AI Parts Catalog](../../docs/AI_PARTS_CATALOG.md)** - Component reference for AI workers
- **API Reference** - Generate with `cargo doc --package dashflow-text-splitters --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Common Patterns

### Pattern 1: Simple Document Chunking

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(50);

let chunks = splitter.split_text(document);
```

### Pattern 2: RAG with Metadata

```rust
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use std::collections::HashMap;

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(50)
    .with_add_start_index(true);

let mut metadata = HashMap::new();
metadata.insert("source".to_string(), serde_json::json!("doc.pdf"));
metadata.insert("page".to_string(), serde_json::json!(1));

let documents = splitter.create_documents(&[text], Some(&[metadata]));
```

### Pattern 3: Code RAG

```rust
use dashflow_text_splitters::{Language, RecursiveCharacterTextSplitter, TextSplitter};

let splitter = RecursiveCharacterTextSplitter::from_language(Language::Rust)
    .with_chunk_size(500)
    .with_chunk_overlap(50);

let chunks = splitter.split_text(source_code);
```

### Pattern 4: Hierarchical Document Splitting

```rust
use dashflow_text_splitters::{MarkdownHeaderTextSplitter, TextSplitter};

let splitter = MarkdownHeaderTextSplitter::new(vec![
    ("#".to_string(), "Chapter".to_string()),
    ("##".to_string(), "Section".to_string()),
    ("###".to_string(), "Subsection".to_string()),
]);

let documents = splitter.split_text(markdown);
// Documents have metadata: {"Chapter": "...", "Section": "...", "Subsection": "..."}
```

## Troubleshooting

### Chunks are too large

Reduce `chunk_size`:
```rust
let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(300)  // Smaller chunks
    .with_chunk_overlap(30);
```

### Losing context between chunks

Increase `chunk_overlap`:
```rust
let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(500)
    .with_chunk_overlap(100);  // 20% overlap
```

### Code splitting at wrong boundaries

Use language-specific separators:
```rust
// Instead of:
let splitter = RecursiveCharacterTextSplitter::new();

// Use:
let splitter = RecursiveCharacterTextSplitter::from_language(Language::Python);
```

### Need to track original positions

Enable start index:
```rust
let splitter = RecursiveCharacterTextSplitter::new()
    .with_add_start_index(true);
```

## Version History

- **1.11** - Current version with 7 text splitters and language-specific code splitting
- **1.9.0** - Added language-specific code splitting
- **1.6.0** - Initial release with basic splitting functionality

## License

MIT
