# Architecture Overview

**Last Updated:** 2025-12-16 (Worker #790 - Fix dashflow-core references and broken links)

DashFlow follows a modular, trait-based architecture that provides flexibility, type safety, and high performance.

## Design Philosophy

### 1. Trait-Based Polymorphism

Rather than inheritance, DashFlow uses **traits** to define behavior:

```rust
pub trait ChatModel: Send + Sync {
    async fn invoke(&self, input: &str) -> Result<ChatResponse>;
    async fn stream(&self, input: &str) -> Result<ChatStream>;
}
```

Any type implementing `ChatModel` can be used interchangeably, enabling:
- **Polymorphism**: Different implementations (OpenAI, Anthropic, Ollama)
- **Testability**: Easy mocking for unit tests
- **Extensibility**: Users can add custom implementations

### 2. Zero-Cost Abstractions

Rust's trait system compiles to efficient machine code:
- **No vtable overhead** for static dispatch
- **Trait objects (`dyn Trait`)** for dynamic dispatch when needed
- **Monomorphization** generates specialized code per concrete type

### 3. Async-First Design

All I/O operations are asynchronous using Tokio:
- **Non-blocking**: Efficient resource utilization
- **Concurrent**: Multiple operations in parallel
- **Cancellation-safe**: Proper cleanup on cancellation

### 4. Type Safety

Compile-time guarantees eliminate entire classes of errors:
- **No null pointers**: Using `Option<T>`
- **Error handling**: Using `Result<T, E>`
- **Thread safety**: `Send + Sync` traits
- **Lifetime tracking**: Memory safety without GC

## Module Structure

```
dashflow/
├── crates/
│   ├── dashflow/              # Core traits, abstractions, and graph framework
│   │   ├── core/               # Core module
│   │   │   ├── language_models/  # ChatModel, LLM traits
│   │   │   ├── embeddings/       # Embeddings trait
│   │   │   ├── vector_stores/    # VectorStore trait
│   │   │   ├── document_loaders/ # Loader trait + implementations
│   │   │   ├── text_splitters/   # TextSplitter trait
│   │   │   ├── tools/            # Tool execution
│   │   │   └── messages/         # Message types
│   │   ├── graph/              # StateGraph implementation
│   │   └── checkpoint/         # Checkpointing system
│   │
│   ├── dashflow-openai/       # OpenAI integration
│   ├── dashflow-anthropic/    # Anthropic integration
│   ├── dashflow-ollama/       # Ollama integration
│   ├── dashflow-cohere/       # Cohere integration
│   │
│   ├── dashflow-qdrant/       # Qdrant vector store
│   ├── dashflow-weaviate/     # Weaviate vector store
│   ├── dashflow-chroma/       # Chroma vector store
│   ├── dashflow-pinecone/     # Pinecone vector store
│   │
│   ├── dashflow-text-splitters/  # Text splitting implementations
│   │
│   └── dashflow-*/            # Additional integrations
│
├── docs/                        # Documentation
└── examples/                    # Example applications
```

## Core Traits

### 1. ChatModel

The `ChatModel` trait represents conversational language models:

```rust
#[async_trait]
pub trait ChatModel: Send + Sync {
    async fn invoke(&self, input: &str) -> Result<ChatResponse>;
    async fn stream(&self, input: &str) -> Result<ChatStream>;
    async fn batch(&self, inputs: &[String]) -> Result<Vec<ChatResponse>>;

    // Tool calling support
    async fn invoke_with_tools(
        &self,
        input: &str,
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse>;
}
```

**Implementations:**
- `ChatOpenAI` - OpenAI GPT models
- `ChatAnthropic` - Anthropic Claude
- `ChatOllama` - Local Ollama models
- `ChatCohere` - Cohere Command models

### 2. Embeddings

The `Embeddings` trait converts text to vectors:

```rust
#[async_trait]
pub trait Embeddings: Send + Sync {
    async fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
    fn dimension(&self) -> usize;
}
```

**Implementations:**
- `OpenAIEmbeddings`
- `CohereEmbeddings`
- `OllamaEmbeddings`

### 3. VectorStore

The `VectorStore` trait provides semantic search:

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn add_documents(&self, documents: &[Document]) -> Result<Vec<String>>;

    async fn similarity_search(
        &self,
        query: &str,
        k: usize,
    ) -> Result<Vec<Document>>;

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
    ) -> Result<Vec<(Document, f32)>>;

    async fn delete(&self, ids: &[String]) -> Result<()>;
}
```

**Implementations:**
- `QdrantVectorStore`
- `WeaviateVectorStore`
- `ChromaVectorStore`
- `PineconeVectorStore`

### 4. Loader

The `Loader` trait loads documents from sources:

```rust
#[async_trait]
pub trait Loader: Send + Sync {
    async fn load(&self, source: &str) -> Result<Vec<Document>>;
    async fn load_and_split(&self, source: &str) -> Result<Vec<Document>>;
}
```

**Implementations:** 100+ loaders (PDF, CSV, HTML, etc.)

### 5. TextSplitter

The `TextSplitter` trait splits documents into chunks:

```rust
pub trait TextSplitter: Send + Sync {
    fn split_text(&self, text: &str) -> Result<Vec<String>>;
    fn split_documents(&self, documents: &[Document]) -> Result<Vec<Document>>;
}
```

**Implementations:**
- `RecursiveCharacterTextSplitter`
- `TokenTextSplitter`
- `MarkdownTextSplitter`
- `CodeTextSplitter`

## Data Flow Architecture

### RAG Pipeline

```
┌──────────────┐
│   Document   │
│    Source    │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│    Loader    │  ← Load documents
└──────┬───────┘
       │
       ▼
┌──────────────┐
│Text Splitter │  ← Split into chunks
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Embeddings  │  ← Generate vectors
└──────┬───────┘
       │
       ▼
┌──────────────┐
│VectorStore   │  ← Store vectors
└──────┬───────┘
       │
       ▼
    ┌──────────────┐
    │    Query     │
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │   Retrieve   │  ← Similarity search
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │   ChatModel  │  ← Generate response
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │   Response   │
    └──────────────┘
```

### Agent Loop

```
┌─────────┐
│  Input  │
└────┬────┘
     │
     ▼
┌─────────────┐
│  ChatModel  │  ← Plan action
└────┬────────┘
     │
     ▼
┌─────────────┐
│   Action?   │  ◄─────┐
└────┬────────┘        │
     │                 │
     ├─[Tool Call]─────┤
     │                 │
     ▼                 │
┌─────────────┐        │
│   Execute   │        │
│    Tool     │        │
└────┬────────┘        │
     │                 │
     └─────────────────┘
     │
     ├─[Final Answer]
     │
     ▼
┌─────────────┐
│   Output    │
└─────────────┘
```

## Memory Management

### Ownership Model

```rust
// Ownership transfer
let doc = Document::new("content");
consume(doc);  // doc moved, no longer accessible

// Borrowing (immutable)
let doc = Document::new("content");
read(&doc);    // doc borrowed
// doc still accessible here

// Mutable borrowing
let mut doc = Document::new("content");
modify(&mut doc);  // Mutably borrowed
// doc still accessible here
```

### Smart Pointers

```rust
use std::sync::Arc;

// Shared ownership with reference counting
let embeddings = Arc::new(OpenAIEmbeddings::default());

let store1 = QdrantVectorStore::new("...")
    .with_embeddings(Arc::clone(&embeddings));

let store2 = WeaviateVectorStore::new("...")
    .with_embeddings(Arc::clone(&embeddings));
```

## Async Runtime

### Tokio Integration

All async operations use Tokio:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Concurrent execution
    let (result1, result2) = tokio::join!(
        operation1(),
        operation2()
    );

    // Spawn background task
    let handle = tokio::spawn(async {
        background_work().await
    });

    handle.await?
}
```

### Streaming Architecture

```rust
use futures::stream::{Stream, StreamExt};

async fn stream_response(llm: &dyn ChatModel) -> Result<()> {
    let mut stream = llm.stream("prompt").await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.content);
    }

    Ok(())
}
```

## Error Handling

### Result Types

```rust
pub type Result<T> = std::result::Result<T, DashFlowError>;

pub enum DashFlowError {
    InvalidInput(String),
    ApiError(String),
    NetworkError(String),
    ParseError(String),
    // ... more variants
}
```

### Error Propagation

```rust
async fn process() -> Result<String> {
    let llm = ChatOpenAI::default();

    // ? operator propagates errors
    let response = llm.invoke("Hello").await?;

    Ok(response.content)
}
```

## Configuration

### Builder Pattern

```rust
let llm = ChatOpenAI::default()
    .with_model("gpt-4")
    .with_temperature(0.7)
    .with_max_tokens(1000)
    .with_timeout(Duration::from_secs(30));
```

### Environment Variables

```rust
use std::env;

let api_key = env::var("OPENAI_API_KEY")
    .expect("OPENAI_API_KEY not set");
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chat_model() {
        let llm = MockChatModel::new();
        let response = llm.invoke("test").await.unwrap();
        assert_eq!(response.content, "mock response");
    }
}
```

### Integration Tests

```rust
#[tokio::test]
#[ignore]  // Run only with --ignored flag
async fn test_openai_integration() {
    let llm = ChatOpenAI::default();
    let response = llm.invoke("Hello").await.unwrap();
    assert!(!response.content.is_empty());
}
```

## Performance Considerations

### 1. Zero-Copy Operations

```rust
// Avoid unnecessary clones
fn process(doc: &Document) {  // Borrow, don't clone
    println!("{}", doc.page_content);
}
```

### 2. Batch Processing

```rust
// Batch API calls
let texts: Vec<String> = documents.iter()
    .map(|d| d.page_content.clone())
    .collect();

let vectors = embeddings.embed_documents(&texts).await?;
```

### 3. Parallel Execution

```rust
use rayon::prelude::*;

// CPU-bound work
let processed: Vec<_> = documents.par_iter()
    .map(|doc| expensive_computation(doc))
    .collect();
```

## Extension Points

### Custom ChatModel

```rust
pub struct CustomChatModel {
    // Your fields
}

#[async_trait]
impl ChatModel for CustomChatModel {
    async fn invoke(&self, input: &str) -> Result<ChatResponse> {
        // Your implementation
        todo!()
    }

    async fn stream(&self, input: &str) -> Result<ChatStream> {
        // Your implementation
        todo!()
    }
}
```

### Custom VectorStore

```rust
pub struct CustomVectorStore {
    // Your fields
}

#[async_trait]
impl VectorStore for CustomVectorStore {
    async fn add_documents(&self, documents: &[Document]) -> Result<Vec<String>> {
        // Your implementation
        todo!()
    }

    async fn similarity_search(&self, query: &str, k: usize) -> Result<Vec<Document>> {
        // Your implementation
        todo!()
    }
}
```

## Best Practices

1. **Use Traits**: Program to interfaces (traits), not implementations
2. **Borrow by Default**: Prefer `&T` over `T` to avoid unnecessary clones
3. **Error Handling**: Always use `Result`, never `unwrap()` in production
4. **Async All the Way**: Don't block async context with sync operations
5. **Test at Trait Level**: Write tests against traits for flexibility
6. **Document Publicly**: All public APIs should have rustdoc comments

## Next Steps

- **[API Documentation](../api/rustdoc.md)**: Full API documentation via rustdoc
- **[RAG Example](../examples/rag.md)**: Build a retrieval-augmented generation system
- **[Language Models](../core/language-models.md)**: Working with LLMs
