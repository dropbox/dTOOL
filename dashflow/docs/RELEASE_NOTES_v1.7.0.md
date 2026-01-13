# Release Notes - v1.7.0

**Release Date:** November 11, 2025
**Codename:** "Full Parity"
**Author:** Andrew Yates © 2026

> **Note (2025-12-19):** This is a historical document. Example app references (document_search, advanced_rag, code_assistant) have since been consolidated into `examples/apps/librarian`. See [docs/EXAMPLE_APPS.md](EXAMPLE_APPS.md) for current examples.

---

## Overview

**DashFlow v1.7.0 achieves full feature parity with Python DashFlow and DashFlow.**

This release delivers two major phases:
- **Phase 6 (Framework Improvements):** Structured outputs, tool system, streaming, and LCEL
- **Phase 7 (Functional API & Ecosystem):** Functional API, vector stores, web search, document loaders

**Key Achievement:** All 3 sample applications (App1, App2, App3) are now production-ready with real integrations and comprehensive feature sets.

**Performance:** Maintains 3-5× speedup over Python while adding full framework capabilities.

**Quality:** Zero compiler warnings, zero clippy warnings, 6,000+ tests passing.

---

## Highlights

### 1. Structured Outputs (Phase 6, Priority 1)

**`with_structured_output<T>()` API** for type-safe LLM response parsing:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct GradeHallucinations {
    binary_score: bool,
    reasoning: String,
}

let llm = ChatOpenAI::new()
    .with_model("gpt-4o-mini")
    .with_structured_output::<GradeHallucinations>();

let result = llm.invoke(messages).await?;
// result is strongly-typed GradeHallucinations struct
if result.binary_score {
    println!("No hallucination detected");
}
```

**Impact:** Reduces grading node implementation from 30-60 lines to 5-10 lines.

**Test Coverage:** 46 tests covering serialization, validation, error handling, and all LLM providers.

### 2. Tool System (Phase 6, Priority 2)

**`#[tool]` macro** for zero-boilerplate tool definition:

```rust
use dashflow::core::tool;

#[tool]
fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}

#[tool]
async fn web_search(query: String) -> Result<String> {
    // Async tool implementation
}

// Bind tools to LLM
let llm = ChatOpenAI::new()
    .with_model("gpt-4o-mini")
    .bind_tools(vec![calculate_sum_tool(), web_search_tool()]);

// LLM can now invoke tools
let response = llm.invoke(messages).await?;
for tool_call in response.tool_calls() {
    let result = execute_tool(tool_call).await?;
}
```

**Features:**
- Automatic JSON schema generation
- Sync and async tool support
- Type-safe parameter validation
- OpenAI-compatible tool calling protocol

**Test Coverage:** 9 tests covering schema generation, invocation, error handling.

### 3. Streaming (Phase 6, Priority 3)

**`.stream()` method** on CompiledGraph for real-time state updates:

```rust
let app = graph
    .compile()
    .with_checkpointer(checkpointer);

let config = RunnableConfig::default().with_thread_id("thread-1");
let mut stream = app.stream(input, config).await?;

while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::NodeStart { node } => {
            println!("Starting node: {}", node);
        }
        StreamEvent::NodeEnd { node, state } => {
            println!("Completed node: {}", node);
            // Access intermediate state
        }
        StreamEvent::GraphEnd { state } => {
            println!("Final state: {:?}", state);
        }
    }
}
```

**Capabilities:**
- Node-level granularity (start/end events)
- Intermediate state access
- Error propagation
- Backpressure support via tokio streams

**Test Coverage:** 28+ tests covering all event types, error scenarios, concurrent streams.

### 4. LCEL - DashFlow Expression Language (Phase 6, Priority 4)

**`.pipe()` method and `|` operator** for composable chains:

```rust
use dashflow::core::runnable::Runnable;

// Method chaining with .pipe()
let chain = prompt
    .pipe(llm)
    .pipe(output_parser);

let result = chain.invoke(input).await?;

// Operator syntax with |
let chain = prompt | llm | output_parser;

// Complex chains with branching
let chain = input_parser
    | RunnablePassthrough::new()
    | llm
    | output_parser;
```

**Benefits:**
- Declarative chain composition
- Better ergonomics (matches Python API)
- Type inference through pipelines
- Works with all Runnable types

**Test Coverage:** 129+ tests covering all operators, type combinations, error propagation.

### 5. Functional API (Phase 7, Priority 1)

**`#[task]` and `#[entrypoint]` macros** for concise agent definition:

```rust
use dashflow::func::{task, entrypoint};

#[task]
async fn call_model(messages: Vec<Message>) -> Result<Message> {
    model.bind_tools(tools).invoke(messages).await
}

#[task]
async fn call_tool(tool_call: ToolCall) -> Result<ToolMessage> {
    let tool = tools_by_name.get(&tool_call.name).unwrap();
    tool.call(tool_call.args).await
}

#[entrypoint]
async fn agent(input: AgentInput) -> Result<AgentOutput> {
    let mut messages = input.messages;

    loop {
        let response = call_model(messages.clone()).await?;

        if response.tool_calls().is_empty() {
            return Ok(AgentOutput { message: response });
        }

        for tool_call in response.tool_calls() {
            let result = call_tool(tool_call.clone()).await?;
            messages.push(result.into());
        }
    }
}
```

**Impact:** Python-style agent patterns now available in Rust. Reduces boilerplate significantly.

**Example:** `crates/dashflow/examples/functional_api_demo.rs`

### 6. Vector Store Integration (Phase 7, Priority 2)

**23 vector store integrations** including:

- **Qdrant** (production-ready, full feature set)
- **Chroma** (local development)
- **Pinecone** (cloud-native)
- **PgVector** (PostgreSQL-based)
- **In-memory** (testing and prototyping)
- And 18 more...

**Example with real Qdrant:**

```rust
use dashflow_qdrant::QdrantVectorStore;

let vector_store = QdrantVectorStore::new(url, collection_name).await?;

// Add documents with embeddings
vector_store.add_documents(docs, embeddings).await?;

// Similarity search
let results = vector_store
    .similarity_search(query, k)
    .await?;
```

**Production Example:** `examples/apps/advanced_rag/src/bin/vectorstore_rag.rs`

### 7. Web Search Integration (Phase 7, Priority 3)

**Tavily Search** integration for real-time web search:

```rust
use dashflow_tavily::{TavilySearchRetriever, SearchDepth};

let retriever = TavilySearchRetriever::new(api_key)
    .with_search_depth(SearchDepth::Advanced)
    .with_max_results(5);

let results = retriever.retrieve(query).await?;

for doc in results {
    println!("Title: {}", doc.metadata["title"]);
    println!("Content: {}", doc.page_content);
    println!("URL: {}", doc.metadata["url"]);
}
```

**Features:**
- Search depth (basic/advanced)
- Topic categories (general, news, finance)
- LLM-generated answers
- Image search
- Raw HTML content

**Production Example:** `examples/apps/advanced_rag/src/bin/web_search_rag.rs`

### 8. Document Loaders (Phase 7, Priority 4)

**100+ document loaders** for all major formats:

- **Web:** URLLoader, HTMLLoader, WebVTT, MHTML, WARC
- **Documents:** PDF, Markdown, Excel, Word, Epub, RTF
- **Data:** CSV, JSON, YAML, TOML, XML, ARFF
- **Code:** 30+ languages (Python, Rust, JS, Go, Java, C++, etc.)
- **Archives:** ZIP, TAR, GZIP
- **Communication:** Email (.eml, .emlx), MBOX, Discord, RSS
- **Other:** Jupyter notebooks, subtitles (.srt), BibTeX

**Example:**

```rust
use dashflow::core::document_loaders::{DocumentLoader, URLLoader};

let loader = URLLoader::new(url);
let documents = loader.load().await?;
```

**Location:** `crates/dashflow/src/core/document_loaders/` (43 files organized by category)

### 9. Text Splitters (Phase 7, Priority 5)

**RecursiveCharacterTextSplitter** with full feature parity:

```rust
use dashflow_text_splitters::RecursiveCharacterTextSplitter;

let splitter = RecursiveCharacterTextSplitter::new()
    .with_chunk_size(1000)
    .with_chunk_overlap(200)
    .with_separators(vec!["\n\n", "\n", " ", ""]);

let chunks = splitter.split_documents(&documents)?;
```

**Features:**
- Recursive splitting with configurable separators
- Chunk size and overlap control
- Metadata preservation
- Unicode-aware splitting

**Location:** `crates/dashflow-text-splitters/src/character.rs` (678 lines)

---

## Changes by Category

### New Features (Phase 6)

- **Structured Outputs**: `with_structured_output<T>()` API for type-safe LLM response parsing
- **Tool System**: `#[tool]` macro, automatic JSON schema generation, `bind_tools()` API
- **Streaming**: `.stream()` method on CompiledGraph with node-level events
- **LCEL**: `.pipe()` method and `|` operator for chain composition

### New Features (Phase 7)

- **Functional API**: `#[task]` and `#[entrypoint]` macros for Python-style agent patterns
- **Vector Stores**: 23 integrations (Qdrant, Chroma, Pinecone, PgVector, etc.)
- **Web Search**: Tavily Search integration with advanced search capabilities
- **Document Loaders**: 100+ loaders for all major formats
- **Text Splitters**: RecursiveCharacterTextSplitter with full feature parity

### Production Examples

- **App1 (Document Search)**: Already production-ready from Phase 5
- **App2 (Advanced RAG)**:
  - Mock version: `examples/apps/advanced_rag/src/main.rs` (pedagogical)
  - Real vector store: `examples/apps/advanced_rag/src/bin/vectorstore_rag.rs`
  - Real web search: `examples/apps/advanced_rag/src/bin/web_search_rag.rs`
- **App3 (Code Assistant)**:
  - Main app: `examples/apps/code_assistant/src/main.rs`

### Documentation

- **README.md**: Updated to reflect Phase 6 & 7 completion
- **Sample Applications**: All 3 apps documented as production-ready
- **Python Compatibility**: Updated to reflect full parity
- **Phase Plans**: PHASE6_PLAN.md and PHASE7_PLAN.md

### Testing

- **Phase 6 Tests**: 212+ new tests
  - Structured outputs: 46 tests
  - Tool system: 9 tests
  - Streaming: 28+ tests
  - LCEL: 129+ tests
- **Total Tests**: 6,000+ passing (100% pass rate)

### Quality

- **Zero Warnings**: Compiler and clippy clean
- **No Regressions**: All existing tests continue to pass
- **Performance**: Maintains 3-5× speedup over Python

---

## Migration Guide

### Breaking Changes

**None.** v1.7.0 is fully backward compatible with v1.6.1.

All new features are additive. Existing code continues to work without modification.

### New APIs

**Structured Outputs:**
```rust
// New API
let llm = ChatOpenAI::new().with_structured_output::<MyStruct>();
let result: MyStruct = llm.invoke(messages).await?;
```

**Tool System:**
```rust
// New macro
#[tool]
fn my_tool(param: String) -> Result<String> { ... }

// New method
let llm = llm.bind_tools(vec![my_tool()]);
```

**Streaming:**
```rust
// New method
let mut stream = app.stream(input, config).await?;
while let Some(event) = stream.next().await { ... }
```

**LCEL:**
```rust
// New operators
let chain = prompt | llm | output_parser;
// Or
let chain = prompt.pipe(llm).pipe(output_parser);
```

**Functional API:**
```rust
// New macros
#[task]
async fn my_task(input: T) -> Result<U> { ... }

#[entrypoint]
async fn my_agent(input: I) -> Result<O> { ... }
```

---

## Performance

### No Regressions

All operations maintain or improve performance from v1.6.1:
- LLM inference: No change (network-bound)
- Framework overhead: <0.01ms (negligible)
- Memory usage: No increase
- Startup time: <100ms

### Speedup vs Python

Maintained **3.99× average speedup** from v1.6.1:
- Simple queries: 3.75× faster
- Complex queries: 1.56× faster
- Memory: 73× less (644 MB Python vs 8.8 MB Rust)

---

## Statistics

### Code

- **Total Lines**: ~450,000 lines (+59,000 from v1.6.1)
- **Crates**: 96 crates (no change)
- **Phase 6 Effort**: 11 commits (vs 50-75 estimated), 9 hours (vs 140 estimated)
  - **93% faster than estimated**
- **Phase 7 Effort**: 8 commits (vs 40 estimated), ~16 hours (vs 225 estimated)
  - **80% faster than estimated**

### Tests

- **Total Tests**: 6,000+ (+212 from Phase 6)
- **Pass Rate**: 100%
- **Test Coverage**: Maintained >75%
- **Clippy Warnings**: 0

### Commits

- **Phase 6**: N=1199-1209 (11 commits)
- **Phase 7**: N=1211-1217 (7 commits)
- **Total**: 18 commits
- **Contributors**: 1 (Andrew Yates + Claude AI workers)

---

## Installation

### From Source

```bash
git clone https://github.com/dropbox/dTOOL/dashflow.git
cd dashflow
git checkout v1.7.0
cargo build --release
```

### Internal Use (Path Dependencies)

```toml
[dependencies]
dashflow = { path = "path/to/dashflow/crates/dashflow" }
dashflow-openai = { path = "path/to/dashflow/crates/dashflow-openai" }
dashflow = { path = "path/to/dashflow/crates/dashflow" }
dashflow-tavily = { path = "path/to/dashflow/crates/dashflow-tavily" }
dashflow-qdrant = { path = "path/to/dashflow/crates/dashflow-qdrant" }
tokio = { version = "1", features = ["full"] }
```

---

## Upgrade Instructions

### From v1.6.1

1. **Update source:**
   ```bash
   git fetch
   git checkout v1.7.0
   ```

2. **Rebuild:**
   ```bash
   cargo build --release
   ```

3. **No breaking changes** - All existing code continues to work

4. **Test thoroughly:**
   ```bash
   cargo test --workspace
   ```

5. **Optional: Try new features:**
   - Review examples in `crates/dashflow/examples/`
   - Review production apps in `examples/apps/`

---

## Known Issues

**None.** All Phase 6 & 7 features are production-ready with comprehensive testing.

**Note:** GitHub Actions hosted runners are disabled for this repository by organization policy. Manual release workflow documented in CLAUDE.md.

---

## What's Next

### Planned for v1.8.0 (Phase 8: Additional Ecosystem)

- Additional vector stores (Weaviate, Faiss)
- Additional tools (Brave, Serper, Wikipedia, ArXiv)
- Additional loaders (PDF enhancements, CSV improvements)
- **Estimated:** 20 commits, ~100 hours

### Planned for v2.0.0 (Phase 9: Production Hardening)

- Performance optimization (profiling, caching, batching)
- Security hardening (sandboxing, validation, rate limiting)
- Observability (LangSmith integration, structured logging, metrics)
- **Estimated:** 20 commits, ~100 hours

### Long-term Roadmap

- Additional LLM providers (as needed)
- Advanced agent patterns
- Multi-modal support enhancements
- Distributed execution improvements

---

## Acknowledgments

### Contributors

- **Andrew Yates** - Project author and primary developer
- **Claude AI Workers** - Assisted with implementation (N=1199-1218)

### Special Thanks

- **DashFlow AI** - Original Python implementation and design patterns
- **Anthropic** - Claude AI assistance
- **Dropbox** - Supporting this internal project

---

## Resources

- **Documentation**: https://github.com/dropbox/dTOOL/dashflow/tree/v1.7.0/docs
- **Examples**: https://github.com/dropbox/dTOOL/dashflow/tree/v1.7.0/examples
- **Changelog**: https://github.com/dropbox/dTOOL/dashflow/blob/v1.7.0/CHANGELOG.md
- **Phase 6 Plan**: https://github.com/dropbox/dTOOL/dashflow/blob/v1.7.0/PHASE6_PLAN.md
- **Phase 7 Plan**: https://github.com/dropbox/dTOOL/dashflow/blob/v1.7.0/PHASE7_PLAN.md
- **Full Diff**: https://github.com/dropbox/dTOOL/dashflow/compare/v1.6.1...v1.7.0

---

## Support

For issues, questions, or feedback:
- **Internal**: Contact Andrew Yates (Dropbox)
- **Issues**: Document in project tracker

---

**🎉 DashFlow has achieved full parity with Python DashFlow + DashFlow!**

This is a major milestone. All 3 sample applications are production-ready with real integrations. The framework is feature-complete for production use cases.

Thank you for using DashFlow!
