# RAG Pipeline Example

This example demonstrates how to build a complete Retrieval-Augmented Generation (RAG) pipeline using DashFlow.

## What is RAG?

RAG combines information retrieval with text generation:
1. **Load** documents from various sources
2. **Split** documents into manageable chunks
3. **Embed** chunks into vectors
4. **Store** vectors in a database
5. **Retrieve** relevant chunks for a query
6. **Generate** answers using retrieved context

## Complete Example

```rust
use dashflow_openai::{ChatOpenAI, OpenAIEmbeddings};
use dashflow_qdrant::QdrantVectorStore;
use dashflow_text_splitters::RecursiveCharacterTextSplitter;
use dashflow::core::{
    document_loaders::{TextLoader, Loader},
    language_models::ChatModel,
    embeddings::Embeddings,
    vector_stores::VectorStore,
    text_splitters::TextSplitter,
    documents::Document,
};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Step 1: Load documents
    println!("Loading documents...");
    let loader = TextLoader::new();
    let documents = loader.load("knowledge_base.txt").await?;
    println!("Loaded {} documents", documents.len());

    // Step 2: Split documents into chunks
    println!("Splitting documents...");
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(1000)
        .with_chunk_overlap(200);
    let chunks = splitter.split_documents(&documents)?;
    println!("Created {} chunks", chunks.len());

    // Step 3: Create embeddings
    println!("Initializing embeddings...");
    let embeddings = OpenAIEmbeddings::default()
        .with_model("text-embedding-3-small");

    // Step 4: Initialize vector store
    println!("Connecting to vector store...");
    let vector_store = QdrantVectorStore::new("http://localhost:6333")
        .with_collection("knowledge_base")
        .with_embeddings(embeddings);

    // Step 5: Add documents to vector store
    println!("Adding documents to vector store...");
    let ids = vector_store.add_documents(&chunks).await?;
    println!("Added {} documents with IDs", ids.len());

    // Step 6: Query the knowledge base
    let query = "What are the main features of Rust?";
    println!("\nQuery: {}", query);

    println!("Retrieving relevant documents...");
    let relevant_docs = vector_store
        .similarity_search_with_score(query, 3)
        .await?;

    println!("Found {} relevant documents:", relevant_docs.len());
    for (i, (doc, score)) in relevant_docs.iter().enumerate() {
        println!("  {}. Score: {:.4}", i + 1, score);
        println!("     {}", &doc.page_content[..100.min(doc.page_content.len())]);
    }

    // Step 7: Build context from retrieved documents
    let context = relevant_docs
        .iter()
        .map(|(doc, _score)| doc.page_content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    // Step 8: Generate answer with LLM
    println!("\nGenerating answer...");
    let llm = ChatOpenAI::default()
        .with_model("gpt-4")
        .with_temperature(0.0);

    let prompt = format!(
        "You are a helpful assistant. Use the context below to answer the question.\n\n\
         Context:\n{}\n\n\
         Question: {}\n\n\
         Answer:",
        context, query
    );

    let response = llm.invoke(&prompt).await?;
    println!("\nAnswer: {}", response.content);

    Ok(())
}
```

## Step-by-Step Breakdown

### 1. Load Documents

```rust
let loader = TextLoader::new();
let documents = loader.load("knowledge_base.txt").await?;
```

You can use different loaders for different file types:
- `PDFLoader` for PDF files
- `HTMLLoader` for HTML pages
- `MarkdownLoader` for Markdown files
- `CSVLoader` for CSV data

### 2. Split Documents

```rust
let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(1000)      // ~1000 characters per chunk
    .with_chunk_overlap(200);   // 200 character overlap
```

**Why split?**
- LLMs have context limits
- Smaller chunks = better retrieval precision
- Overlap maintains context continuity

### 3. Create Embeddings

```rust
let embeddings = OpenAIEmbeddings::default()
    .with_model("text-embedding-3-small");
```

**Options:**
- `text-embedding-3-small`: Fast, cost-effective (1536 dimensions)
- `text-embedding-3-large`: Higher quality (3072 dimensions)
- `CohereEmbeddings`: Alternative provider
- `OllamaEmbeddings`: Local embeddings

### 4. Initialize Vector Store

```rust
let vector_store = QdrantVectorStore::new("http://localhost:6333")
    .with_collection("knowledge_base")
    .with_embeddings(embeddings);
```

**Alternatives:**
- `WeaviateVectorStore`
- `ChromaVectorStore`
- `PineconeVectorStore`

### 5. Add Documents

```rust
let ids = vector_store.add_documents(&chunks).await?;
```

This automatically:
1. Generates embeddings for each chunk
2. Stores embeddings in the vector database
3. Returns unique IDs for each document

### 6. Similarity Search

```rust
let relevant_docs = vector_store
    .similarity_search_with_score(query, 3)
    .await?;
```

**Parameters:**
- `query`: The search query
- `k` (3): Number of results to return
- Returns: `Vec<(Document, f32)>` with similarity scores

### 7. Build Context

```rust
let context = relevant_docs
    .iter()
    .map(|(doc, _score)| doc.page_content.as_str())
    .collect::<Vec<_>>()
    .join("\n\n");
```

Concatenate retrieved documents into a single context string.

### 8. Generate Answer

```rust
let llm = ChatOpenAI::default()
    .with_model("gpt-4")
    .with_temperature(0.0);  // Deterministic output

let prompt = format!(
    "Context:\n{}\n\nQuestion: {}\n\nAnswer:",
    context, query
);

let response = llm.invoke(&prompt).await?;
```

## Advanced Features

### Metadata Filtering

```rust
// Add metadata to documents
let mut doc = Document::new("Rust is a systems language");
doc.metadata.insert("language".to_string(), json!("rust"));
doc.metadata.insert("year".to_string(), json!(2024));

// Search with metadata filter
let results = vector_store
    .similarity_search_with_filter(
        query,
        5,
        |doc| doc.metadata.get("language") == Some(&json!("rust"))
    )
    .await?;
```

### Streaming Responses

```rust
use futures::StreamExt;

let mut stream = llm.stream(&prompt).await?;

print!("Answer: ");
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
}
println!();
```

### Multiple Document Sources

```rust
// Load from multiple sources
let pdf_loader = PDFLoader::new();
let html_loader = HTMLLoader::new();

let mut all_docs = Vec::new();
all_docs.extend(pdf_loader.load("doc1.pdf").await?);
all_docs.extend(html_loader.load("page.html").await?);

// Process all documents
let chunks = splitter.split_documents(&all_docs)?;
vector_store.add_documents(&chunks).await?;
```

### Reranking Results

```rust
use dashflow_cohere::CohereRerank;

// Initial retrieval
let candidates = vector_store.similarity_search(query, 10).await?;

// Rerank for better precision
let reranker = CohereRerank::default();
let reranked = reranker.rerank(query, &candidates, 3).await?;
```

## Production Considerations

### 1. Error Handling

```rust
async fn rag_pipeline(query: &str) -> Result<String, Box<dyn Error>> {
    let vector_store = QdrantVectorStore::new("http://localhost:6333")
        .with_collection("kb")
        .with_embeddings(OpenAIEmbeddings::default());

    let docs = match vector_store.similarity_search(query, 3).await {
        Ok(docs) => docs,
        Err(e) => {
            eprintln!("Vector search failed: {}", e);
            return Err(e.into());
        }
    };

    // Continue processing...
    Ok("answer".to_string())
}
```

### 2. Caching Embeddings

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct CachedEmbeddings {
    embeddings: OpenAIEmbeddings,
    cache: Arc<Mutex<HashMap<String, Vec<f32>>>>,
}

impl CachedEmbeddings {
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        // Check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(vector) = cache.get(text) {
                return Ok(vector.clone());
            }
        }

        // Generate and cache
        let vector = self.embeddings.embed_query(text).await?;
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(text.to_string(), vector.clone());
        }

        Ok(vector)
    }
}
```

### 3. Monitoring

```rust
use std::time::Instant;

async fn rag_with_metrics(query: &str) -> Result<String> {
    let start = Instant::now();

    // Retrieval
    let retrieval_start = Instant::now();
    let docs = vector_store.similarity_search(query, 3).await?;
    let retrieval_time = retrieval_start.elapsed();

    // Generation
    let generation_start = Instant::now();
    let response = llm.invoke(&prompt).await?;
    let generation_time = generation_start.elapsed();

    let total_time = start.elapsed();

    println!("Metrics:");
    println!("  Retrieval: {:?}", retrieval_time);
    println!("  Generation: {:?}", generation_time);
    println!("  Total: {:?}", total_time);

    Ok(response.content)
}
```

## Running the Example

### Prerequisites

1. **Start Qdrant**:
```bash
docker run -p 6333:6333 qdrant/qdrant
```

2. **Set API Key**:
```bash
export OPENAI_API_KEY="sk-..."
```

3. **Create Knowledge Base**:
```bash
echo "Rust is a systems programming language..." > knowledge_base.txt
```

### Run

```bash
cargo run --example rag
```

## Next Steps

- **[Language Models](../core/language-models.md)**: Deep dive into ChatModel trait
- **[Architecture Overview](../architecture/overview.md)**: Understand the system design
- **[API Documentation](../api/rustdoc.md)**: Complete API reference
