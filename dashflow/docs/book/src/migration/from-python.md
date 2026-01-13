# Migrating from Python LangChain to DashFlow

This guide helps you migrate from Python LangChain to DashFlow (Rust).

## Overview

DashFlow is a pure Rust implementation of LangChain-compatible APIs, maintaining **95% API compatibility** with Python LangChain while providing:

- **2-10x performance improvement** (compiled code, zero-copy operations)
- **Memory safety guarantees** (no GC pauses, compile-time safety)
- **Zero Python runtime dependency** (single binary deployment)
- **Strong type checking** (catch errors at compile time)

**LangChain Baseline:** v1.0.1 (October 2025) - API compatibility maintained for easy migration.

---

## Quick Reference: Import Mapping

| Python Package | DashFlow Crate |
|----------------|----------------|
| `langchain-openai` | `dashflow-openai` |
| `langchain-anthropic` | `dashflow-anthropic` |
| `langchain-community` | Various specialized crates |
| `langchain-core` | `dashflow` (core crate) |
| `langchain-qdrant` | `dashflow-qdrant` |
| `langgraph` | `dashflow` (StateGraph module) |

---

## Key Differences

### 1. Async/Await

**Python** uses `asyncio`:
```python
import asyncio
from langchain_openai import ChatOpenAI

llm = ChatOpenAI()
response = await llm.ainvoke("Hello")
```

**Rust** uses `tokio`:
```rust
use dashflow_openai::ChatOpenAI;
use dashflow::core::language_models::ChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();
    let response = llm.invoke("Hello").await?;
    Ok(())
}
```

**Migration Tips:**
- Replace `import asyncio` with `#[tokio::main]`
- No separate `ainvoke` - all methods are async by default in DashFlow
- Add `.await?` to async calls
- Use `Result` return types

### 2. Error Handling

**Python** uses exceptions:
```python
try:
    response = llm.invoke("Hello")
except Exception as e:
    print(f"Error: {e}")
```

**Rust** uses `Result` types:
```rust
match llm.invoke("Hello").await {
    Ok(response) => println!("Success: {}", response.content),
    Err(e) => eprintln!("Error: {}", e),
}

// Or use ? operator for propagation
let response = llm.invoke("Hello").await?;
```

**Migration Tips:**
- Replace `try/except` with `match` or `?` operator
- Return `Result<T, E>` instead of raising exceptions
- Use `?` to propagate errors up the call stack

### 3. Type Annotations

**Python** (optional typing):
```python
from langchain_core.documents import Document
from typing import List

def process_docs(docs: List[Document]) -> List[str]:
    return [doc.page_content for doc in docs]
```

**Rust** (required typing):
```rust
use dashflow::core::documents::Document;

fn process_docs(docs: &[Document]) -> Vec<String> {
    docs.iter()
        .map(|doc| doc.page_content.clone())
        .collect()
}
```

**Migration Tips:**
- All types are required in Rust
- Use `&[T]` for slices (read-only arrays)
- Use `Vec<T>` for owned vectors
- Use `&str` for string slices, `String` for owned strings

### 4. Memory Management

**Python** (garbage collected):
```python
doc = Document(page_content="text")
process(doc)
print(doc.page_content)  # Still accessible
```

**Rust** (ownership):
```rust
let doc = Document::new("text");
process(doc);  // doc is moved
// println!("{}", doc.page_content);  // Compile error!

// Use borrowing instead
let doc = Document::new("text");
process(&doc);  // Borrow instead of move
println!("{}", doc.page_content);  // OK!
```

**Migration Tips:**
- Understand ownership, borrowing, and lifetimes
- Use `&` for borrowing (read-only)
- Use `&mut` for mutable borrowing
- Clone only when necessary (`.clone()`)

---

## LCEL (LangChain Expression Language)

DashFlow implements LCEL through the **`Runnable` trait** and composition operators.

### Basic Composition

**Python LCEL:**
```python
from langchain_core.prompts import ChatPromptTemplate
from langchain_openai import ChatOpenAI
from langchain_core.output_parsers import StrOutputParser

prompt = ChatPromptTemplate.from_messages([
    ("system", "You are a helpful assistant"),
    ("human", "{question}")
])
llm = ChatOpenAI()
parser = StrOutputParser()

# Pipe composition
chain = prompt | llm | parser
response = await chain.ainvoke({"question": "What is AI?"})
```

**DashFlow:**
```rust
use dashflow::core::prompts::ChatPromptTemplate;
use dashflow::core::output_parsers::StrOutputParser;
use dashflow::core::runnable::Runnable;
use dashflow_openai::ChatOpenAI;

let prompt = ChatPromptTemplate::from_messages(vec![
    ("system", "You are a helpful assistant"),
    ("human", "{question}")
])?;
let llm = ChatOpenAI::default();
let parser = StrOutputParser::new();

// Method chaining (preferred)
let chain = prompt.pipe(llm).pipe(parser);
let response = chain.invoke(serde_json::json!({"question": "What is AI?"})).await?;

// Or use BitOr operator (| operator)
let chain = prompt | llm | parser;
```

### Runnable Trait Methods

| Python Method | DashFlow Method | Notes |
|--------------|-----------------|-------|
| `invoke()` | `invoke()` | Sync call |
| `ainvoke()` | `invoke().await` | No separate method |
| `batch()` | `batch()` | Process multiple inputs |
| `abatch()` | `batch().await` | No separate method |
| `stream()` | `stream()` | Streaming results |
| `astream()` | `stream().await` | No separate method |

### RunnableParallel

**Python:**
```python
from langchain_core.runnables import RunnableParallel

parallel = RunnableParallel(
    summary=summary_chain,
    translation=translation_chain
)
```

**DashFlow:**
```rust
use dashflow::core::runnable::RunnableParallel;

let parallel = RunnableParallel::new()
    .add("summary", summary_chain)
    .add("translation", translation_chain);
```

### RunnablePassthrough

**Python:**
```python
from langchain_core.runnables import RunnablePassthrough

chain = {"context": retriever, "question": RunnablePassthrough()} | prompt | llm
```

**DashFlow:**
```rust
use dashflow::core::runnable::{RunnableParallel, RunnablePassthrough};

let chain = RunnableParallel::new()
    .add("context", retriever)
    .add("question", RunnablePassthrough::new())
    .pipe(prompt)
    .pipe(llm);
```

---

## Tools and Agents

### Tool Definition

**Python:**
```python
from langchain_core.tools import tool

@tool
def calculator(expression: str) -> str:
    """Evaluate a mathematical expression."""
    return str(eval(expression))
```

**DashFlow:**
```rust
use dashflow::core::tools::{sync_function_tool, Tool};

let calculator = sync_function_tool(
    "calculator",
    "Evaluate a mathematical expression",
    |expression: String| -> Result<String, String> {
        // Safe evaluation (do not use eval!)
        Ok(format!("Result: {}", expression))
    }
);
```

### Agent Creation

**Python:**
```python
from langchain.agents import create_react_agent
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(model="gpt-4")
tools = [calculator, search_tool]
agent = create_react_agent(llm, tools, prompt)
```

**DashFlow:**
```rust
use dashflow::core::agents::{ReActAgent, AgentExecutor};
use dashflow_openai::ChatOpenAI;

let llm = ChatOpenAI::default().with_model("gpt-4");
let tools = vec![calculator, search_tool];
let agent = ReActAgent::new(llm, tools)?;
let executor = AgentExecutor::new(agent);

let result = executor.run("Calculate 2 + 2").await?;
```

### Agent Types

| Python | DashFlow | Use Case |
|--------|----------|----------|
| `create_react_agent` | `ReActAgent` | Reasoning + Acting |
| `create_structured_chat_agent` | `StructuredChatAgent` | Multi-input tools |
| `create_openai_functions_agent` | `OpenAIFunctionsAgent` | OpenAI function calling |
| `create_tool_calling_agent` | `ToolCallingAgent` | Generic tool calling |

### Key Difference: Tool Binding

**Python** uses `bind_tools()`:
```python
llm_with_tools = llm.bind_tools(tools)
```

**DashFlow** passes tools directly:
```rust
// Tools are passed to the agent constructor, not bound to the LLM
let agent = ReActAgent::new(llm, tools)?;
```

---

## StateGraph (LangGraph)

DashFlow includes a full LangGraph-compatible StateGraph implementation.

### Basic Graph

**Python (LangGraph):**
```python
from langgraph.graph import StateGraph, END
from typing import TypedDict

class MyState(TypedDict):
    messages: list
    context: str

def process_node(state: MyState) -> MyState:
    return {"messages": state["messages"], "context": "processed"}

graph = StateGraph(MyState)
graph.add_node("processor", process_node)
graph.set_entry_point("processor")
graph.add_edge("processor", END)
app = graph.compile()

result = await app.ainvoke({"messages": [], "context": ""})
```

**DashFlow:**
```rust
use dashflow::graph::{StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
struct MyState {
    messages: Vec<String>,
    context: String,
}

fn process_node(state: &mut MyState) -> Result<(), DashFlowError> {
    state.context = "processed".to_string();
    Ok(())
}

let mut graph = StateGraph::<MyState>::new();
graph.add_node_from_fn("processor", process_node)?;
graph.set_entry_point("processor")?;
graph.add_edge("processor", END)?;

let app = graph.compile()?;
let result = app.invoke(MyState::default()).await?;
```

### Conditional Edges

**Python:**
```python
def route(state: MyState) -> str:
    if state["context"] == "error":
        return "error_handler"
    return "success_handler"

graph.add_conditional_edges("processor", route, {
    "error_handler": "error_node",
    "success_handler": "success_node"
})
```

**DashFlow:**
```rust
fn route(state: &MyState) -> String {
    if state.context == "error" {
        "error_handler".to_string()
    } else {
        "success_handler".to_string()
    }
}

graph.add_conditional_edges(
    "processor",
    route,
    vec![
        ("error_handler", "error_node"),
        ("success_handler", "success_node"),
    ]
)?;
```

### Checkpointing

**Python:**
```python
from langgraph.checkpoint.sqlite import SqliteSaver

memory = SqliteSaver.from_conn_string("checkpoints.db")
app = graph.compile(checkpointer=memory)
```

**DashFlow:**
```rust
use dashflow::checkpoint::FileCheckpointer;

let checkpointer = FileCheckpointer::new("checkpoints/")?;
let app = graph.compile_with_checkpointer(checkpointer)?;
```

---

## Chains

DashFlow provides all standard LangChain chains:

### LLMChain

**Python:**
```python
from langchain.chains import LLMChain

chain = LLMChain(llm=llm, prompt=prompt)
result = await chain.ainvoke({"topic": "AI"})
```

**DashFlow:**
```rust
use dashflow_chains::LLMChain;

let chain = LLMChain::new(llm, prompt);
let result = chain.invoke(serde_json::json!({"topic": "AI"})).await?;
```

### RetrievalQA

**Python:**
```python
from langchain.chains import RetrievalQA

qa = RetrievalQA.from_chain_type(
    llm=llm,
    retriever=retriever,
    chain_type="stuff"
)
```

**DashFlow:**
```rust
use dashflow_chains::RetrievalQA;

let qa = RetrievalQA::new(llm, retriever)
    .with_chain_type(ChainType::Stuff);
```

### Document Chains

| Python Chain | DashFlow | Description |
|--------------|----------|-------------|
| `StuffDocumentsChain` | `StuffDocumentsChain` | All docs in one prompt |
| `MapReduceDocumentsChain` | `MapReduceDocumentsChain` | Map then reduce |
| `RefineDocumentsChain` | `RefineDocumentsChain` | Iterative refinement |
| `MapRerankDocumentsChain` | `MapRerankDocumentsChain` | Map and rerank |

---

## API Comparison

### ChatModel

**Python:**
```python
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(model="gpt-4", temperature=0.7)
response = await llm.ainvoke("Hello")
print(response.content)
```

**DashFlow:**
```rust
use dashflow_openai::ChatOpenAI;

let llm = ChatOpenAI::default()
    .with_model("gpt-4")
    .with_temperature(0.7);

let response = llm.invoke("Hello").await?;
println!("{}", response.content);
```

### Embeddings

**Python:**
```python
from langchain_openai import OpenAIEmbeddings

embeddings = OpenAIEmbeddings()
vector = await embeddings.aembed_query("Hello")
```

**DashFlow:**
```rust
use dashflow_openai::OpenAIEmbeddings;

let embeddings = OpenAIEmbeddings::default();
let vector = embeddings.embed_query("Hello").await?;
```

### Vector Stores

**Python:**
```python
from langchain_qdrant import QdrantVectorStore
from langchain_openai import OpenAIEmbeddings

embeddings = OpenAIEmbeddings()
store = QdrantVectorStore(
    url="http://localhost:6333",
    collection_name="docs",
    embeddings=embeddings
)

await store.aadd_documents(documents)
results = await store.asimilarity_search("query", k=5)
```

**DashFlow:**
```rust
use dashflow_qdrant::QdrantVectorStore;
use dashflow_openai::OpenAIEmbeddings;

let embeddings = OpenAIEmbeddings::default();
let store = QdrantVectorStore::new("http://localhost:6333")
    .with_collection("docs")
    .with_embeddings(embeddings);

store.add_documents(&documents).await?;
let results = store.similarity_search("query", 5).await?;
```

### Document Loaders

**Python:**
```python
from langchain_community.document_loaders import PDFLoader

loader = PDFLoader("document.pdf")
documents = loader.load()
```

**DashFlow:**
```rust
use dashflow::core::document_loaders::{PDFLoader, Loader};

let loader = PDFLoader::new();
let documents = loader.load("document.pdf").await?;
```

### Text Splitters

**Python:**
```python
from langchain.text_splitter import RecursiveCharacterTextSplitter

splitter = RecursiveCharacterTextSplitter(
    chunk_size=1000,
    chunk_overlap=200
)
chunks = splitter.split_documents(documents)
```

**DashFlow:**
```rust
use dashflow_text_splitters::RecursiveCharacterTextSplitter;

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(1000)
    .with_chunk_overlap(200);

let chunks = splitter.split_documents(&documents)?;
```

### Memory Systems

**Python:**
```python
from langchain.memory import ConversationBufferMemory

memory = ConversationBufferMemory()
await memory.asave_context({"input": "Hi"}, {"output": "Hello"})
history = await memory.aload_memory_variables({})
```

**DashFlow:**
```rust
use dashflow_memory::{BaseMemory, ConversationBufferMemory};

let mut memory = ConversationBufferMemory::new();
memory.save_context("Hi", "Hello").await?;
let history = memory.load_memory_variables().await?;
```

---

## Complete Example Migration

### Python RAG Pipeline

```python
from langchain_openai import ChatOpenAI, OpenAIEmbeddings
from langchain_qdrant import QdrantVectorStore
from langchain.text_splitter import RecursiveCharacterTextSplitter
from langchain_community.document_loaders import TextLoader

async def rag_pipeline():
    # Load documents
    loader = TextLoader("knowledge.txt")
    documents = loader.load()

    # Split
    splitter = RecursiveCharacterTextSplitter(
        chunk_size=1000,
        chunk_overlap=200
    )
    chunks = splitter.split_documents(documents)

    # Embed and store
    embeddings = OpenAIEmbeddings()
    store = QdrantVectorStore(
        url="http://localhost:6333",
        collection_name="kb",
        embeddings=embeddings
    )
    await store.aadd_documents(chunks)

    # Query
    results = await store.asimilarity_search("query", k=3)
    context = "\n\n".join([doc.page_content for doc in results])

    # Generate
    llm = ChatOpenAI(model="gpt-4")
    prompt = f"Context:\n{context}\n\nQuestion: query\nAnswer:"
    response = await llm.ainvoke(prompt)

    return response.content
```

### DashFlow RAG Pipeline

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
};

async fn rag_pipeline() -> Result<String, Box<dyn std::error::Error>> {
    // Load documents
    let loader = TextLoader::new();
    let documents = loader.load("knowledge.txt").await?;

    // Split
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(1000)
        .with_chunk_overlap(200);
    let chunks = splitter.split_documents(&documents)?;

    // Embed and store
    let embeddings = OpenAIEmbeddings::default();
    let store = QdrantVectorStore::new("http://localhost:6333")
        .with_collection("kb")
        .with_embeddings(embeddings);
    store.add_documents(&chunks).await?;

    // Query
    let results = store.similarity_search("query", 3).await?;
    let context = results.iter()
        .map(|doc| doc.page_content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    // Generate
    let llm = ChatOpenAI::default().with_model("gpt-4");
    let prompt = format!(
        "Context:\n{}\n\nQuestion: query\nAnswer:",
        context
    );
    let response = llm.invoke(&prompt).await?;

    Ok(response.content)
}
```

---

## Common Patterns

### Pattern 1: List Comprehensions to Iterator Chains

**Python:**
```python
texts = [doc.page_content for doc in documents]
filtered = [doc for doc in documents if doc.metadata.get("page") > 10]
```

**DashFlow:**
```rust
let texts: Vec<String> = documents.iter()
    .map(|doc| doc.page_content.clone())
    .collect();

let filtered: Vec<_> = documents.iter()
    .filter(|doc| doc.metadata.get("page")
        .and_then(|v| v.as_i64())
        .map(|page| page > 10)
        .unwrap_or(false))
    .collect();
```

### Pattern 2: Dictionaries to HashMap

**Python:**
```python
metadata = {"page": 1, "source": "doc.pdf"}
page = metadata.get("page", 0)
```

**DashFlow:**
```rust
use std::collections::HashMap;
use serde_json::json;

let mut metadata = HashMap::new();
metadata.insert("page".to_string(), json!(1));
metadata.insert("source".to_string(), json!("doc.pdf"));

let page = metadata.get("page")
    .and_then(|v| v.as_i64())
    .unwrap_or(0);
```

### Pattern 3: Optional Values

**Python:**
```python
value = config.get("key")
if value is not None:
    process(value)
```

**DashFlow:**
```rust
if let Some(value) = config.get("key") {
    process(value);
}

// Or use map
config.get("key").map(|value| process(value));
```

### Pattern 4: Streaming

**Python:**
```python
async for chunk in llm.astream("prompt"):
    print(chunk.content, end="")
```

**DashFlow:**
```rust
use futures::StreamExt;

let mut stream = llm.stream("prompt").await?;
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
}
```

---

## Feature Parity Notes

### Fully Supported

| Feature | Status | Notes |
|---------|--------|-------|
| Chat Models (OpenAI, Anthropic, etc.) | Complete | All major providers |
| Embeddings | Complete | OpenAI, Cohere, HuggingFace, etc. |
| Vector Stores | Complete | 15+ backends |
| Document Loaders | Complete | PDF, HTML, JSON, CSV, etc. |
| Text Splitters | Complete | Character, token, semantic |
| Memory Systems | Complete | Buffer, window, summary, entity |
| LCEL/Runnables | Complete | Full composition support |
| StateGraph (LangGraph) | Complete | Conditional edges, checkpointing |
| Agents | Complete | ReAct, OpenAI Functions, Tool Calling |
| Chains | Complete | LLM, QA, Summarization, etc. |
| Callbacks | Complete | For tracing and logging |
| Caching | Complete | In-memory, Redis, filesystem |

### Not Yet Implemented

| Feature | Status | Alternative |
|---------|--------|-------------|
| Hub (langchainhub) | Not implemented | Use local prompts |
| LangSmith integration | Partial | Use DashFlow observability |
| LangServe | Not implemented | Build custom REST API |

### Enhanced Beyond Python

| Feature | Status | Notes |
|---------|--------|-------|
| Type Safety | Enhanced | Compile-time guarantees |
| Performance | 2-10x faster | Native compilation |
| Observability | Built-in | Prometheus, Grafana, tracing |
| Self-Improvement | Unique | AI agents can modify themselves |
| Introspection | Unique | Query platform capabilities |

---

## Common Pitfalls

### Pitfall 1: Forgetting `.await`

```rust
// Wrong - returns Future, not Response
let response = llm.invoke("Hello");

// Correct
let response = llm.invoke("Hello").await?;
```

### Pitfall 2: Moving Instead of Borrowing

```rust
// Wrong - doc is moved, can't use after
for doc in documents {
    process(doc);
}
// documents is now empty!

// Correct - borrow instead
for doc in &documents {
    process(doc);
}
// documents still available
```

### Pitfall 3: Forgetting Error Handling

```rust
// Wrong - ignores errors
let _ = llm.invoke("Hello").await;

// Correct - handle or propagate
let response = llm.invoke("Hello").await?;
```

### Pitfall 4: Double Braces in Prompts

```python
# Python uses double braces for escaping
prompt = "Hello {{name}}"  # Literal {name}
```

```rust
// Rust uses single braces
let prompt = "Hello {name}";  // Variable substitution
```

### Pitfall 5: Async in Sync Context

```rust
// Wrong - can't use await in non-async function
fn process() {
    let response = llm.invoke("Hello").await;  // Error!
}

// Correct - make function async
async fn process() {
    let response = llm.invoke("Hello").await?;
}
```

---

## Configuration

### Environment Variables

**Python:**
```python
import os
api_key = os.getenv("OPENAI_API_KEY")
```

**DashFlow:**
```rust
use std::env;

let api_key = env::var("OPENAI_API_KEY")?;

// Or use dotenv
use dotenv::dotenv;
dotenv().ok();
```

### API Keys

Both Python and Rust read from the same environment variables:
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `COHERE_API_KEY`

---

## Performance Tips

### 1. Avoid Cloning

**Inefficient:**
```rust
fn process(doc: Document) {  // Takes ownership
    println!("{}", doc.page_content);
}

for doc in documents {
    process(doc.clone());  // Unnecessary clone
}
```

**Efficient:**
```rust
fn process(doc: &Document) {  // Borrows
    println!("{}", doc.page_content);
}

for doc in &documents {
    process(doc);  // No clone needed
}
```

### 2. Use References in Loops

**Inefficient:**
```rust
for doc in documents {  // Moves documents
    // Can't use documents after loop
}
```

**Efficient:**
```rust
for doc in &documents {  // Borrows
    // Can still use documents after loop
}
```

### 3. Batch API Calls

```rust
// Bad: Sequential
for doc in &documents {
    embeddings.embed_query(&doc.page_content).await?;
}

// Good: Batch
let texts: Vec<_> = documents.iter()
    .map(|d| d.page_content.clone())
    .collect();
embeddings.embed_documents(&texts).await?;
```

---

## Testing

### Python

```python
import pytest

@pytest.mark.asyncio
async def test_llm():
    llm = ChatOpenAI()
    response = await llm.ainvoke("Hello")
    assert len(response.content) > 0
```

### Rust

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_llm() {
        let llm = ChatOpenAI::default();
        let response = llm.invoke("Hello").await.unwrap();
        assert!(!response.content.is_empty());
    }
}
```

---

## Common Migration Errors

### Error 1: Borrow After Move

```rust
let doc = Document::new("text");
process(doc);  // doc moved here
println!("{}", doc.page_content);  // Error: doc was moved
```

**Fix:** Use borrowing
```rust
let doc = Document::new("text");
process(&doc);  // Borrow instead
println!("{}", doc.page_content);  // OK
```

### Error 2: Missing `.await`

```rust
let response = llm.invoke("Hello");  // Error: wrong type
```

**Fix:** Add `.await`
```rust
let response = llm.invoke("Hello").await?;
```

### Error 3: Unused `Result`

```rust
llm.invoke("Hello").await;  // Warning: unused Result
```

**Fix:** Handle the result
```rust
let response = llm.invoke("Hello").await?;
```

---

## Resources

- **Rust Book**: [doc.rust-lang.org/book](https://doc.rust-lang.org/book/)
- **Async Book**: [rust-lang.github.io/async-book](https://rust-lang.github.io/async-book/)
- **Examples**: [GitHub examples/](https://github.com/dropbox/dTOOL/dashflow/tree/main/examples)
- **API Docs**: Run `cargo doc --open` or see `docs/API_INDEX.md`
- **Python Parity Report**: [docs/PYTHON_PARITY_REPORT.md](../../../PYTHON_PARITY_REPORT.md)

---

## Next Steps

- **[Core Concepts](../getting-started/core-concepts.md)**: Understand Rust patterns
- **[Examples](../examples/rag.md)**: Study complete examples
- **[API Reference](../api/rustdoc.md)**: Browse API documentation
- **[Cookbook](../../../COOKBOOK.md)**: Common recipes and patterns
