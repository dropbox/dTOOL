# Core Concepts

**Last Updated:** 2025-12-16 (Worker #790 - Fix dashflow-core references and broken links)

Understanding these core concepts will help you build effective applications with DashFlow.

## Architecture Overview

DashFlow follows a modular, trait-based architecture:

```
┌─────────────────────────────────────────┐
│         Your Application                │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│      DashFlow Core Traits              │
│  (ChatModel, Embeddings, VectorStore)   │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│    Provider-Specific Implementations    │
│  (OpenAI, Anthropic, Qdrant, etc.)      │
└─────────────────────────────────────────┘
```

## Core Traits

### 1. ChatModel

The `ChatModel` trait represents a language model that can generate text:

```rust
pub trait ChatModel {
    async fn invoke(&self, input: &str) -> Result<ChatResponse>;
    async fn stream(&self, input: &str) -> Result<ChatStream>;
}
```

**Implementations:**
- `ChatOpenAI` - OpenAI GPT models
- `ChatAnthropic` - Anthropic Claude models
- `ChatOllama` - Local Ollama models
- `ChatCohere` - Cohere Command models

**Usage:**
```rust
let llm = ChatOpenAI::default();
let response = llm.invoke("Hello!").await?;
```

### 2. Embeddings

The `Embeddings` trait converts text into vector representations:

```rust
pub trait Embeddings {
    async fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
}
```

**Implementations:**
- `OpenAIEmbeddings` - OpenAI embedding models
- `CohereEmbeddings` - Cohere embedding models
- `OllamaEmbeddings` - Local Ollama embeddings

**Usage:**
```rust
let embeddings = OpenAIEmbeddings::default();
let vector = embeddings.embed_query("Hello world").await?;
```

### 3. VectorStore

The `VectorStore` trait provides semantic search over documents:

```rust
pub trait VectorStore {
    async fn add_documents(&self, documents: &[Document]) -> Result<Vec<String>>;
    async fn similarity_search(&self, query: &str, k: usize) -> Result<Vec<Document>>;
    async fn similarity_search_with_score(&self, query: &str, k: usize)
        -> Result<Vec<(Document, f32)>>;
}
```

**Implementations:**
- `QdrantVectorStore` - Qdrant vector database
- `WeaviateVectorStore` - Weaviate vector database
- `ChromaVectorStore` - Chroma vector database
- `PineconeVectorStore` - Pinecone vector database

**Usage:**
```rust
let store = QdrantVectorStore::new("http://localhost:6333")
    .with_collection("docs")
    .with_embeddings(embeddings);

store.add_documents(&documents).await?;
let results = store.similarity_search("query", 5).await?;
```

### 4. DocumentLoader

The `Loader` trait loads documents from various sources:

```rust
pub trait Loader {
    async fn load(&self, source: &str) -> Result<Vec<Document>>;
}
```

**Implementations:**
- `TextLoader` - Plain text files
- `PDFLoader` - PDF documents
- `CSVLoader` - CSV files
- `HTMLLoader` - HTML documents
- `MarkdownLoader` - Markdown files
- 100+ more loaders

**Usage:**
```rust
let loader = PDFLoader::new();
let documents = loader.load("document.pdf").await?;
```

### 5. TextSplitter

The `TextSplitter` trait splits documents into chunks:

```rust
pub trait TextSplitter {
    fn split_text(&self, text: &str) -> Result<Vec<String>>;
    fn split_documents(&self, documents: &[Document]) -> Result<Vec<Document>>;
}
```

**Implementations:**
- `RecursiveCharacterTextSplitter` - Split by characters recursively
- `TokenTextSplitter` - Split by tokens
- `MarkdownTextSplitter` - Markdown-aware splitting
- `CodeTextSplitter` - Code-aware splitting

**Usage:**
```rust
let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(1000)
    .with_chunk_overlap(200);

let chunks = splitter.split_documents(&documents)?;
```

## Key Data Structures

### Document

Represents a piece of text with metadata:

```rust
pub struct Document {
    pub page_content: String,
    pub metadata: HashMap<String, Value>,
}
```

**Example:**
```rust
let doc = Document {
    page_content: "Rust is a systems programming language...".to_string(),
    metadata: HashMap::from([
        ("source".to_string(), json!("rust_book.pdf")),
        ("page".to_string(), json!(42)),
    ]),
};
```

### Message Types

Chat models work with message types:

```rust
pub enum Message {
    System(String),      // System instructions
    Human(String),       // User input
    AI(String),         // Assistant response
    Function(String),    // Function call result
}
```

**Example:**
```rust
let messages = vec![
    Message::System("You are a helpful assistant".to_string()),
    Message::Human("What is Rust?".to_string()),
];
```

## Design Patterns

### 1. Builder Pattern

Most components use the builder pattern:

```rust
let llm = ChatOpenAI::default()
    .with_model("gpt-4")
    .with_temperature(0.7)
    .with_max_tokens(1000);

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(1000)
    .with_chunk_overlap(200)
    .with_separators(vec!["\n\n", "\n", " "]);
```

### 2. Trait Objects

Use trait objects for polymorphism:

```rust
async fn generate_response(llm: &dyn ChatModel, prompt: &str) -> Result<String> {
    let response = llm.invoke(prompt).await?;
    Ok(response.content)
}

// Can accept any ChatModel implementation
let openai = ChatOpenAI::default();
generate_response(&openai, "Hello").await?;

let anthropic = ChatAnthropic::default();
generate_response(&anthropic, "Hello").await?;
```

### 3. Async/Await

All I/O operations are async:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Sequential execution
    let response1 = llm.invoke("Question 1").await?;
    let response2 = llm.invoke("Question 2").await?;

    // Parallel execution
    let (response1, response2) = tokio::join!(
        llm.invoke("Question 1"),
        llm.invoke("Question 2")
    );

    Ok(())
}
```

### 4. Error Handling

Use `Result` types consistently:

```rust
use dashflow::core::error::DashFlowError;

async fn process() -> Result<String, DashFlowError> {
    let llm = ChatOpenAI::default();

    // ? operator propagates errors
    let response = llm.invoke("Hello").await?;

    Ok(response.content)
}

// Handle errors
match process().await {
    Ok(result) => println!("Success: {}", result),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Common Workflows

### RAG (Retrieval-Augmented Generation)

```
┌─────────┐      ┌──────────┐      ┌────────────┐
│Documents│─────▶│ Splitter │─────▶│ Embeddings │
└─────────┘      └──────────┘      └──────┬─────┘
                                           │
                                           ▼
┌─────────┐      ┌──────────┐      ┌────────────┐
│  Query  │─────▶│ Retrieve │◀─────│VectorStore │
└─────────┘      └────┬─────┘      └────────────┘
                      │
                      ▼
                 ┌──────────┐
                 │   LLM    │
                 └────┬─────┘
                      │
                      ▼
                 ┌──────────┐
                 │ Response │
                 └──────────┘
```

### Agent Loop

```
┌────────┐
│ Input  │
└───┬────┘
    │
    ▼
┌────────┐      ┌────────┐
│  LLM   │◀────▶│ Tools  │
└───┬────┘      └────────┘
    │
    ▼
┌────────┐
│ Output │
└────────┘
```

## Memory Management

### Rust Ownership

DashFlow leverages Rust's ownership system:

```rust
// Ownership transfer
let doc = Document::new("content");
process_document(doc);  // doc is moved

// Borrowing (read-only)
let doc = Document::new("content");
read_document(&doc);    // doc is borrowed
println!("{}", doc.page_content);  // Still accessible

// Mutable borrowing
let mut doc = Document::new("content");
modify_document(&mut doc);  // Mutable borrow
```

### Reference Counting

Use `Arc` for shared ownership:

```rust
use std::sync::Arc;

let embeddings = Arc::new(OpenAIEmbeddings::default());

// Multiple vector stores can share the same embeddings
let store1 = QdrantVectorStore::new("http://localhost:6333")
    .with_embeddings(Arc::clone(&embeddings));

let store2 = WeaviateVectorStore::new("http://localhost:8080")
    .with_embeddings(Arc::clone(&embeddings));
```

## Performance Considerations

### 1. Batch Operations

Prefer batch operations for efficiency:

```rust
// Bad: One request per document
for doc in &documents {
    embeddings.embed_query(&doc.page_content).await?;
}

// Good: Batch request
let texts: Vec<String> = documents.iter()
    .map(|d| d.page_content.clone())
    .collect();
embeddings.embed_documents(&texts).await?;
```

### 2. Concurrent Execution

Use `tokio::spawn` for CPU-bound work:

```rust
let tasks: Vec<_> = documents.iter()
    .map(|doc| {
        let doc = doc.clone();
        tokio::spawn(async move {
            process_document(doc).await
        })
    })
    .collect();

let results = futures::future::join_all(tasks).await;
```

### 3. Streaming

Use streaming for long responses:

```rust
let mut stream = llm.stream("Write a long essay").await?;

while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
}
```

## Best Practices

1. **Use Type System**: Let Rust's type system catch errors at compile time
2. **Handle Errors**: Always handle `Result` types, don't unwrap unless certain
3. **Batch Operations**: Batch API calls when possible
4. **Stream Large Outputs**: Stream responses for better UX
5. **Clone Sparingly**: Avoid unnecessary clones, use references
6. **Test Components**: Unit test individual components before integration
7. **Monitor Resources**: Track token usage and costs
8. **Cache When Possible**: Cache embeddings and LLM responses

## Next Steps

- **[Language Models](../core/language-models.md)**: Learn about LLM providers
- **[RAG Example](../examples/rag.md)**: Build a retrieval-augmented generation system
- **[Architecture Overview](../architecture/overview.md)**: Understand the system design
- **[Migration Guide](../migration/from-python.md)**: Migrate from Python DashFlow
