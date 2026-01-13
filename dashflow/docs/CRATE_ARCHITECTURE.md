# DashFlow Crate Architecture

**Last Updated:** 2026-01-05 (Worker #2505 - Fix stale Core Platform crate line counts: ~574k→~593k, 7 crates updated)
**Total Crates:** 108
**Total Lines:** ~815,000 Rust code

> **Note on Line Counts:** Line counts are measured using `find crates/<crate>/src -name "*.rs" | xargs wc -l`
> for the core dashflow crate and most LLM providers. Some integration crates (vector stores, tools) may
> include examples/ in their counts. All counts should be verified using the src/ methodology for consistency.

## Overview

DashFlow is organized into 108 crates following a hub-and-spoke architecture:

```
                                    ┌──────────────┐
                                    │   dashflow   │ (Core - 444k lines)
                                    │   (hub)      │
                                    └──────┬───────┘
                                           │
          ┌────────────────────────────────┼────────────────────────────────┐
          │                                │                                │
    ┌─────┴─────┐                  ┌───────┴───────┐                ┌───────┴───────┐
    │ Providers │                  │ Vector Stores │                │    Tools      │
    │ (17)      │                  │ (22)          │                │ (6)           │
    └───────────┘                  └───────────────┘                └───────────────┘
          │                                │                                │
    ┌─────┴─────┐                  ┌───────┴───────┐                ┌───────┴───────┐
    │Integrations│                │ Checkpointers │                │ Infrastructure│
    │ (24)      │                  │ (4)           │                │ (12)          │
    └───────────┘                  └───────────────┘                └───────────────┘
```

## Dependency Pattern

Almost all crates depend on `dashflow` (core):

```
dashflow ← dashflow-openai
        ← dashflow-anthropic
        ← dashflow-qdrant
        ← dashflow-cli
        ← ... (100+ crates)
```

The `dashflow` core provides:
- `ChatModel` and `Embedding` traits
- `Tool` trait and tool registry
- `VectorStore` trait
- `Checkpointer` trait
- Graph execution (`CompiledGraph`, `Node`)
- Streaming infrastructure
- Self-improvement and introspection

---

## Crate Categories

### 1. Core Platform (8 crates, ~593k lines)

The foundational crates that make up DashFlow's runtime.

| Crate | Lines | Description |
|-------|-------|-------------|
| `dashflow` | 444,016 | **Core library** - graph execution, traits, streaming, self-improvement |
| `dashflow-cli` | 30,670 | Command-line interface |
| `dashflow-streaming` | 32,749 | Message streaming, Kafka, DLQ, event sourcing |
| `dashflow-registry` | 21,591 | Component registries and factories |
| `dashflow-evals` | 20,099 | Evaluation framework, quality metrics |
| `dashflow-chains` | 17,293 | Pre-built chains and pipelines |
| `dashflow-memory` | 15,207 | Memory management for agents |
| `dashflow-standard-tests` | 11,507 | Test infrastructure |

### 2. LLM Providers (17 crates, ~37k lines)

External LLM integrations. Each implements `ChatModel` trait.

| Crate | Lines | Provider |
|-------|-------|----------|
| `dashflow-openai` | 6,055 | OpenAI (GPT-4, GPT-3.5) |
| `dashflow-anthropic` | 5,980 | Anthropic (Claude) |
| `dashflow-fireworks` | 2,900 | Fireworks.ai |
| `dashflow-xai` | 2,417 | xAI (Grok) |
| `dashflow-groq` | 2,289 | Groq |
| `dashflow-ollama` | 2,279 | Ollama (local LLMs) |
| `dashflow-cohere` | 2,153 | Cohere |
| `dashflow-mistral` | 2,122 | Mistral AI |
| `dashflow-bedrock` | 2,042 | AWS Bedrock |
| `dashflow-huggingface` | 1,699 | HuggingFace Inference |
| `dashflow-azure-openai` | 1,706 | Azure OpenAI |
| `dashflow-gemini` | 1,627 | Google Gemini |
| `dashflow-replicate` | 1,339 | Replicate |
| `dashflow-together` | 1,313 | Together.ai |
| `dashflow-cloudflare` | 523 | Cloudflare Workers AI |
| `dashflow-perplexity` | 499 | Perplexity |
| `dashflow-deepseek` | 445 | DeepSeek |

### 3. Vector Stores (22 crates, ~33k lines)

Vector databases and similarity search. Each implements `VectorStore` trait.

| Crate | Lines | Database |
|-------|-------|----------|
| `dashflow-qdrant` | 7,900 | Qdrant |
| `dashflow-chroma` | 1,313 | Chroma |
| `dashflow-redis` | 3,070 | Redis (with RediSearch) |
| `dashflow-elasticsearch` | 2,381 | Elasticsearch |
| `dashflow-opensearch` | 1,862 | OpenSearch |
| `dashflow-neo4j` | 1,404 | Neo4j (graph + vector) |
| `dashflow-pinecone` | 1,103 | Pinecone |
| `dashflow-hnsw` | 1,204 | HNSW (local) |
| `dashflow-faiss` | 982 | FAISS |
| `dashflow-mongodb` | 910 | MongoDB Atlas |
| `dashflow-cassandra` | 1,087 | Cassandra |
| `dashflow-pgvector` | 840 | PostgreSQL pgvector |
| `dashflow-weaviate` | 947 | Weaviate |
| `dashflow-timescale` | 1,063 | TimescaleDB |
| `dashflow-annoy` | 1,013 | Annoy (local) |
| `dashflow-milvus` | 1,009 | Milvus |
| `dashflow-clickhouse` | 836 | ClickHouse |
| `dashflow-sqlitevss` | 990 | SQLite VSS |
| `dashflow-lancedb` | 944 | LanceDB |
| `dashflow-usearch` | 862 | USearch (local) |
| `dashflow-typesense` | 578 | Typesense |
| `dashflow-supabase` | 332 | Supabase |

### 4. Embeddings (3 crates, ~3k lines)

Text embedding providers. Each implements `Embedding` trait.

> **Note:** `dashflow-huggingface` (in LLM Providers) also provides embeddings via the `Embedding` trait.

| Crate | Lines | Provider |
|-------|-------|----------|
| `dashflow-jina` | 1,209 | Jina AI |
| `dashflow-voyage` | 1,200 | Voyage AI |
| `dashflow-nomic` | 548 | Nomic |

### 5. Tools (6 crates, ~8k lines)

Callable tools for agents. Each implements `Tool` trait.

| Crate | Lines | Description |
|-------|-------|-------------|
| `dashflow-shell-tool` | 3,653 | Shell command execution |
| `dashflow-file-tool` | 1,908 | File system operations |
| `dashflow-git-tool` | 1,043 | Git operations |
| `dashflow-json-tool` | 593 | JSON manipulation |
| `dashflow-calculator` | 395 | Mathematical calculations |
| `dashflow-human-tool` | 243 | Human-in-the-loop |

### 6. Integrations (24 crates, ~20k lines)

External service integrations.

> **Note:** `dashflow-cloudflare` is in LLM Providers (implements `ChatModel` for Cloudflare Workers AI).

| Crate | Lines | Service |
|-------|-------|---------|
| `dashflow-youtube` | 1,901 | YouTube |
| `dashflow-github` | 1,405 | GitHub |
| `dashflow-clickup` | 1,331 | ClickUp |
| `dashflow-langsmith` | 1,083 | LangSmith tracing |
| `dashflow-google-search` | 942 | Google Search |
| `dashflow-tavily` | 1,026 | Tavily search |
| `dashflow-gitlab` | 862 | GitLab |
| `dashflow-arxiv` | 927 | ArXiv papers |
| `dashflow-office365` | 860 | Office 365 |
| `dashflow-pubmed` | 919 | PubMed |
| `dashflow-slack` | 916 | Slack |
| `dashflow-reddit` | 959 | Reddit |
| `dashflow-gmail` | 734 | Gmail |
| `dashflow-jira` | 834 | Jira |
| `dashflow-stackexchange` | 775 | Stack Exchange |
| `dashflow-openweathermap` | 717 | OpenWeatherMap |
| `dashflow-wikipedia` | 644 | Wikipedia |
| `dashflow-brave` | 600 | Brave Search |
| `dashflow-bing` | 596 | Bing Search |
| `dashflow-serper` | 589 | Serper.dev |
| `dashflow-graphql` | 770 | GraphQL |
| `dashflow-exa` | 550 | Exa Search |
| `dashflow-wolfram` | 580 | Wolfram Alpha |
| `dashflow-duckduckgo` | 436 | DuckDuckGo |

### 7. Checkpointers (4 crates, ~4k lines)

State persistence for graph execution. Each implements `Checkpointer` trait.

| Crate | Lines | Backend |
|-------|-------|---------|
| `dashflow-s3-checkpointer` | 974 | AWS S3 |
| `dashflow-redis-checkpointer` | 953 | Redis |
| `dashflow-postgres-checkpointer` | 933 | PostgreSQL |
| `dashflow-dynamodb-checkpointer` | 838 | AWS DynamoDB |

### 8. Infrastructure (12 crates, ~28k lines)

Supporting infrastructure for development, testing, and observability.

| Crate | Lines | Description |
|-------|-------|-------------|
| `dashflow-observability` | 13,443 | Observability, metrics, tracing |
| `dashflow-benchmarks` | 3,537 | Performance benchmarks |
| `dashflow-langserve` | 2,616 | LangServe compatible API |
| `dashflow-wasm-executor` | 2,532 | WASM execution runtime |
| `dashflow-prometheus-exporter` | 2,527 | Prometheus metrics |
| `dashflow-remote-node` | 1,830 | Remote graph node execution |
| `dashflow-http-requests` | 1,273 | HTTP client utilities |
| `dashflow-webscrape` | 932 | Web scraping |
| `dashflow-sql-database` | 846 | SQL query tool |
| `dashflow-playwright` | 831 | Browser automation |
| `dashflow-testing` | 401 | Test utilities |
| `dashflow-openapi` | 396 | OpenAPI spec generation |

### 9. Utilities (12 crates, ~22k lines)

Shared utilities, types, and helpers.

| Crate | Lines | Description |
|-------|-------|-------------|
| `dashflow-text-splitters` | 8,595 | Document chunking |
| `dashflow-module-discovery` | 2,484 | Module introspection |
| `dashflow-document-compressors` | 1,572 | Document compression |
| `dashflow-project` | 1,486 | Project templates |
| `dashflow-file-management` | 1,483 | File handling |
| `dashflow-prompts` | 1,179 | Prompt templates |
| `dashflow-json` | 959 | JSON utilities |
| `dashflow-context` | 939 | Context management |
| `dashflow-factories` | 927 | Factory patterns |
| `dashflow-derive` | 830 | Derive macros (GraphState, MergeableState) |
| `dashflow-macros` | 422 | Proc macros (#[tool], #[derive(GraphState)]) |
| `dashflow-compression` | 389 | Data compression |

---

## Size Distribution

```
Size Tier       | Count | Total Lines | Examples
----------------|-------|-------------|----------------------------------
Giant (>100k)   |   1   |   444,016   | dashflow
Large (10k-100k)|   8   |   163,446   | streaming, cli, registry, evals, chains, memory, observability, standard-tests
Medium (1k-10k) |  47   |   102,629   | text-splitters, qdrant, anthropic, wasm-executor, langserve, shell-tool
Small (<1k)     |  52   |    36,463   | sqlitevss, webscrape, reddit, json, lancedb, google-search
```

### Maintenance Burden Analysis

**High Maintenance (>10k lines, complex logic):**
- `dashflow` - Core logic, constantly evolving
- `dashflow-streaming` - Message infrastructure
- `dashflow-cli` - User-facing interface

**Medium Maintenance (1k-10k lines, stable APIs):**
- LLM providers - API changes from upstream
- Vector stores - Schema changes
- Checkpointers - Storage format stability

**Low Maintenance (<1k lines, simple wrappers):**
- Search integrations (brave, bing, serper)
- Small tools (calculator, human-tool)
- Utility crates (compression, json)

---

## Shared Code Patterns

### HTTP Client (All API Crates)

All HTTP-based crates use `dashflow::core::http_client`:

```rust
use dashflow::core::http_client::HttpClient;

let client = HttpClient::new();
let response = client.post(url).json(&request).send().await?;
```

### Error Types

All crates use `thiserror` with `#[from]` for composition:

```rust
use dashflow::core::error::Error;

#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error(transparent)]
    DashFlow(#[from] Error),
    #[error("specific error: {0}")]
    Specific(String),
}
```

### Trait Implementations

All provider crates follow the same pattern:

```rust
#[async_trait]
impl ChatModel for MyChatModel {
    async fn chat(&self, messages: Vec<Message>) -> Result<Response> {
        // Implementation
    }
}
```

---

## Known Issues

### 1. Proc Macro Overlap (Phase 259)

Both `dashflow-derive` and `dashflow-macros` define `#[derive(GraphState)]` with **different behavior**:

- `dashflow-derive`: Simple trait verification
- `dashflow-macros`: Includes reducer logic

**Recommendation:** Consolidate into single crate with clear naming.

### 2. Small Crates (<500 lines)

6 crates are under 500 lines. Consider consolidation:

- `dashflow-duckduckgo` (436) + similar search → `dashflow-web-search`
- `dashflow-human-tool` (443) + `dashflow-json-tool` (735) → `dashflow-tools`

**Decision:** Keep separate for now - clear boundaries outweigh consolidation savings.

---

## Adding New Crates

### 1. Create Crate

```bash
cargo new crates/dashflow-myservice --lib
```

### 2. Add to Workspace

In `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/dashflow-myservice",
    # ... other members
]
```

### 3. Add Core Dependency

In `crates/dashflow-myservice/Cargo.toml`:

```toml
[dependencies]
dashflow = { path = "../dashflow" }
```

### 4. Implement Required Trait

For LLM provider:
```rust
impl ChatModel for MyModel { ... }
```

For vector store:
```rust
impl VectorStore for MyStore { ... }
```

For tool:
```rust
impl Tool for MyTool { ... }
```

### 5. Add README

Create `crates/dashflow-myservice/README.md` using template from `templates/CRATE_README.md`.

---

## See Also

- [Workspace Cargo.toml](../Cargo.toml) - All crate definitions
- [DESIGN_INVARIANTS.md](../DESIGN_INVARIANTS.md) - Architectural laws
- [INDEX.md](./INDEX.md) - Documentation index
- [CONTRIBUTING_DOCS.md](./CONTRIBUTING_DOCS.md) - Documentation standards
