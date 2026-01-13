# DashFlow Cookbook

**Last Updated:** 2026-01-03 (Worker #2361 - Document internal architecture guide)

**A comprehensive guide to building AI applications with DashFlow**

This cookbook provides practical, runnable examples for common use cases. Each recipe includes:
- Complete working code
- Explanation of key concepts
- Best practices and tips
- Links to relevant API documentation

> **Note on Model Instantiation**: The recommended way to create chat models is via config-driven instantiation:
> ```rust
> use dashflow_openai::build_chat_model;
> let config: ChatModelConfig = serde_yaml::from_str("type: openai\nmodel: gpt-4o\napi_key: { env: OPENAI_API_KEY }")?;
> let model = build_chat_model(&config)?;
> ```
> See [dashflow-openai README](../crates/dashflow-openai/README.md) for details.

---

## Table of Contents

### 🚀 Getting Started
- [Simple LLM Chain](#simple-llm-chain)
- [Basic Agent with Tools](#basic-agent-with-tools)
- [Structured Output](#structured-output)
- [Prompt Templates](#prompt-templates)

### 📚 Vector Stores & Retrieval
- [HNSW Vector Store](#hnsw-vector-store)
- [Weaviate Integration](#weaviate-integration)
- [Cassandra Vector Store](#cassandra-vector-store)
- [Neo4j Vector Store](#neo4j-vector-store)
- [Timescale Vector Store](#timescale-vector-store)
- [OpenSearch Integration](#opensearch-integration)
- [SQLiteVSS Vector Store](#sqlitevss-vector-store)
- [Hybrid Search](#hybrid-search)

### 🔄 DashFlow Workflows
- [Multi-Agent Research Team](#multi-agent-research-team)
- [Error Recovery Patterns](#error-recovery-patterns)
- [Checkpointing & Resume](#checkpointing-resume)
- [S3 Checkpointing](#s3-checkpointing)
- [Streaming Aggregation](#streaming-aggregation)
- [Multi-Model Comparison](#multi-model-comparison)

### 📊 Observability & Monitoring
- [Traced Agent](#traced-agent)
- [Custom Metrics](#custom-metrics)
- [DashFlow Streaming Telemetry](#dashstream-telemetry)
- [Distributed Tracing](#distributed-tracing-example)

### 🏗️ Production Applications
- [Document Search (RAG)](#document-search-rag)
- [Code Assistant](#code-assistant)
- [Advanced RAG with Web Search](#advanced-rag-web-search)
- [Optimized Document Search](#optimized-document-search)

### ⚡ Performance Optimization Recipes
- [Recipe 1: Reduce Latency with Streaming](#recipe-1-reduce-latency-with-streaming)
- [Recipe 2: Parallelize Independent Operations](#recipe-2-parallelize-independent-operations)
- [Recipe 3: Batch API Calls](#recipe-3-batch-api-calls)
- [Recipe 4: Cache Expensive Operations](#recipe-4-cache-expensive-operations)
- [Recipe 5: Limit Concurrent Requests](#recipe-5-limit-concurrent-requests)
- [Recipe 6: Use Connection Pooling](#recipe-6-use-connection-pooling)
- [Recipe 7: Optimize Token Usage](#recipe-7-optimize-token-usage)
- [Recipe 8: Choose the Right Data Structure](#recipe-8-choose-the-right-data-structure)
- [Recipe 9: Profile Before Optimizing](#recipe-9-profile-before-optimizing)
- [Recipe 10: Use Release Builds for Benchmarking](#recipe-10-use-release-builds-for-benchmarking)

### 🛡️ Resilience Patterns
- [Circuit Breaker Pattern](#circuit-breaker-pattern)
- [Retry with Exponential Backoff](#retry-with-exponential-backoff)
- [Timeout Patterns](#timeout-patterns)
- [Graceful Degradation](#graceful-degradation)

### 💰 Cost Optimization Patterns
- [Pattern 1: Model Selection and Routing](#pattern-1-model-selection-and-routing)
- [Pattern 2: Token Budgets and Limits](#pattern-2-token-budgets-and-limits)
- [Pattern 3: Caching and Deduplication](#pattern-3-caching-and-deduplication)
- [Pattern 4: Batch Processing](#pattern-4-batch-processing)
- [Pattern 5: Prompt Optimization](#pattern-5-prompt-optimization)

### 🎯 Advanced Patterns
- [Browser Automation (NatBot)](#natbot-browser-automation)
- [AWS Bedrock Integration](#aws-bedrock-integration)
- [Quality Evaluation](#quality-evaluation)

### 🧪 Prompt Optimization
- [Bootstrap FewShot](#bootstrap-fewshot)
- [MIPROv2 Optimizer](#miprov2-optimizer)
- [COPRO Instruction Optimization](#copro-instruction-optimization)
- [COPROv2 Confidence-Based Optimization](#coprov2-confidence-based-optimization)
- [GRPO Group Optimization](#grpo-group-optimization)
- [SIMBA Self-Improvement](#simba-self-improvement)
- [KNN FewShot Selection](#knn-fewshot-selection)
- [AutoPrompt Gradient-Free Search](#autoprompt-gradient-free-search)

### 🔗 Graph Composition
- [ReAct Agent Pattern](#react-agent-pattern)
- [Subgraph Composition](#subgraph-composition)
- [Conditional Edge Routing](#conditional-edge-routing)
- [Parallel Branch Execution](#parallel-branch-execution)
- [Human-in-the-Loop Approval](#human-in-the-loop-approval)

### 📦 State Management
- [MergeableState Pattern](#mergeablestate-pattern)
- [State Reducers](#state-reducers)

### 💾 Database Checkpointing
- [SQLite Checkpointing](#sqlite-checkpointing)
- [Multi-Tier Checkpointing](#multi-tier-checkpointing)
- [Checkpoint Migration](#checkpoint-migration)

### 📊 Evaluation Metrics
- [Exact Match Evaluation](#exact-match-evaluation)
- [F1 Score Evaluation](#f1-score-evaluation)
- [SemanticF1 LLM-as-Judge](#semanticf1-llm-as-judge)
- [Custom Metric Functions](#custom-metric-functions)

---

## 🚀 Getting Started

### Simple LLM Chain

**Use Case:** Create a basic chain that calls an LLM with a prompt.

**Example:** See [examples/structured_output.rs](../examples/structured_output.rs)

```rust
use dashflow::core::language_models::ChatModel;
use dashflow_openai::OpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an LLM client
    let llm = OpenAI::default();

    // Simple invoke
    let response = llm.invoke("What is the capital of France?").await?;
    println!("Response: {}", response);

    Ok(())
}
```

**Key Concepts:**
- `ChatModel` trait provides unified interface for all LLM providers
- `invoke()` is the simplest way to call an LLM
- Supports streaming via `stream()` method

**Best Practices:**
- Set API keys via environment variables (e.g., `OPENAI_API_KEY`)
- Use `Result` for error handling
- Consider rate limiting for production use

**Related Documentation:**
- [LLM Providers](AI_PARTS_CATALOG.md#llm-providers)
- [OpenAI Integration](AI_PARTS_CATALOG.md#openai)

---

### Basic Agent with Tools

**Use Case:** Create an agent that can use tools to answer questions.

**Example:** See [examples/traced_agent.rs](../examples/traced_agent.rs)

```rust
// M-657: Fixed imports to use actual crate paths
use dashflow::core::agents::{Agent, AgentExecutor};
use dashflow::core::tools::Tool;
use dashflow_calculator::Calculator;
use dashflow_openai::ChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create tools
    let calculator = Calculator::new();

    // Create agent with tools
    let agent = Agent::builder()
        .llm(ChatOpenAI::default())
        .tools(vec![Box::new(calculator)])
        .build()?;

    // Execute agent
    let executor = AgentExecutor::new(agent);
    let result = executor.run("What is 25 * 4 + 17?").await?;

    println!("Result: {}", result);
    Ok(())
}
```

**Key Concepts:**
- Agents use ReAct pattern (Reason + Act)
- Tools extend agent capabilities
- AgentExecutor handles tool execution loop

**Best Practices:**
- Provide clear tool descriptions
- Set max iterations to prevent infinite loops
- Use tracing to debug agent decisions

**Related Documentation:**
- [Agents Guide](AI_PARTS_CATALOG.md#agents)
- [Tools Catalog](AI_PARTS_CATALOG.md#tools)

---

### Structured Output

**Use Case:** Extract structured data from text using LLMs.

**Example:** [examples/structured_output.rs](../examples/structured_output.rs)

```rust
use serde::{Deserialize, Serialize};
use dashflow::core::language_models::ChatModel;
use dashflow_openai::OpenAI;

#[derive(Debug, Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    occupation: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = OpenAI::default();

    let prompt = "Extract information about: John is a 35 year old software engineer";
    let person: Person = llm.invoke_structured(prompt).await?;

    println!("Extracted: {:?}", person);
    Ok(())
}
```

**Key Concepts:**
- Use Rust structs with `serde` for schema definition
- LLMs extract data into structured format
- Type safety ensures valid outputs

**Best Practices:**
- Define clear struct fields with good names
- Add field descriptions via doc comments
- Validate extracted data before use

**Related Documentation:**
- [Structured Outputs](AI_PARTS_CATALOG.md#structured-outputs-v170)

---

### Prompt Templates

**Use Case:** Reusable prompts with variable substitution.

```rust
use dashflow::core::prompts::PromptTemplate;

let template = PromptTemplate::new(
    "You are a {role}. Answer this question: {question}"
);

let prompt = template.format(&[
    ("role", "helpful assistant"),
    ("question", "What is Rust?"),
])?;

// Use with LLM
let response = llm.invoke(&prompt).await?;
```

**Key Concepts:**
- Templates separate prompt structure from content
- Variable substitution with `{variable}` syntax
- Supports few-shot examples

**Best Practices:**
- Store templates separately from code
- Version control your prompts
- Test prompts with multiple variable values

---

## 📚 Vector Stores & Retrieval

### HNSW Vector Store

**Use Case:** Fast in-memory approximate nearest neighbor search.

**Example:** [examples/hnsw_basic.rs](../examples/hnsw_basic.rs)

```rust
// M-657: Fixed imports to use actual crate paths
use dashflow_hnsw::{HNSWVectorStore, HNSWConfig, DistanceMetric};
use dashflow_openai::OpenAIEmbeddings;
use dashflow::core::vector_stores::VectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create embeddings provider
    let embeddings = OpenAIEmbeddings::default();

    // Create HNSW config
    let config = HNSWConfig {
        dimension: 1536, // text-embedding-3-small dimension
        max_elements: 10000,
        m: 16,
        ef_construction: 200,
        distance_metric: DistanceMetric::Cosine,
    };

    // Create HNSW vector store
    let mut store = HNSWVectorStore::new(embeddings, config)?;

    // Add documents
    store.add_texts(vec![
        "The sky is blue",
        "Grass is green",
        "The ocean is deep",
    ], None).await?;

    // Search
    let results = store.similarity_search("water", 2).await?;
    for doc in results {
        println!("Found: {}", doc.page_content);
    }

    Ok(())
}
```

**Key Concepts:**
- HNSW = Hierarchical Navigable Small World graphs
- Fast approximate search (not exact)
- Configurable M (connections) and ef_construction (accuracy)

**Performance:**
- **Memory-based** - entire index in RAM
- **Very fast** - ~1ms query latency
- **Scalable** - handles millions of vectors

**Best Practices:**
- Higher M = better recall, more memory
- Higher ef_construction = better quality, slower build
- Use for prototyping and small-to-medium datasets (<1M vectors)

**Related Documentation:**
- [Vector Stores Guide](AI_PARTS_CATALOG.md#vector-stores)
- [HNSW Algorithm](https://arxiv.org/abs/1603.09320)

---

### Weaviate Integration

**Use Case:** Production-grade vector database with hybrid search.

**Example:** [examples/weaviate_basic.rs](../examples/weaviate_basic.rs)

```rust
use dashflow_weaviate::Weaviate;
use dashflow_embeddings::openai::OpenAIEmbeddings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let embeddings = OpenAIEmbeddings::default();

    let store = Weaviate::builder()
        .url("http://localhost:8080")
        .class_name("Document")
        .embeddings(embeddings)
        .build()?;

    // Add documents with metadata
    store.add_documents_with_metadata(vec![
        ("Rust is fast", vec![("language", "rust")]),
        ("Python is popular", vec![("language", "python")]),
    ]).await?;

    // Hybrid search (vector + keyword)
    let results = store.hybrid_search("programming languages", 0.7).await?;

    Ok(())
}
```

**Key Concepts:**
- Weaviate is a standalone vector database
- Supports hybrid search (vector + BM25)
- Schema-based with automatic vectorization

**Performance:**
- **Persistent** - data stored on disk
- **Scalable** - handles billions of vectors
- **Production-ready** - replication, backups, monitoring

**Best Practices:**
- Define schemas before indexing
- Use filters for metadata-based search
- Configure replication for high availability
- Monitor memory usage for large datasets

**Related Documentation:**
- [Vector Store Implementations](AI_PARTS_CATALOG.md#vector-store-implementations)
- [Weaviate Docs](https://weaviate.io/developers/weaviate)

---

### Cassandra Vector Store

**Use Case:** Distributed vector storage with Cassandra/DataStax Astra.

**Example:** [examples/cassandra_basic.rs](../examples/cassandra_basic.rs)

**Key Features:**
- **Distributed** - horizontally scalable
- **Durable** - replication across data centers
- **Integrated** - vector search + traditional database

**Best Practices:**
- Use partition keys for efficient queries
- Configure consistency level (LOCAL_QUORUM recommended)
- Enable compression for storage efficiency

**Related Documentation:**
- [Vector Store Implementations](AI_PARTS_CATALOG.md#vector-store-implementations)

---

### Neo4j Vector Store

**Use Case:** Graph database with vector search for knowledge graphs.

**Example:** [examples/neo4j_basic.rs](../examples/neo4j_basic.rs)

**Key Features:**
- **Graph + Vector** - combines graph traversal with semantic search
- **Cypher queries** - powerful graph query language
- **Relationships** - model connections between entities

**Best Practices:**
- Model domain as graph (nodes + relationships)
- Use vector search for initial retrieval
- Use graph queries for contextual exploration

**Related Documentation:**
- [Vector Store Implementations](AI_PARTS_CATALOG.md#vector-store-implementations)

---

### Timescale Vector Store

**Use Case:** Time-series + vector search with PostgreSQL.

**Example:** [examples/timescale_basic.rs](../examples/timescale_basic.rs)

**Key Features:**
- **Time-series optimized** - efficient time-based queries
- **PostgreSQL extension** - familiar SQL interface
- **Hybrid queries** - combine time, metadata, and vector search

**Best Practices:**
- Use time partitioning for large datasets
- Create indexes on frequently queried columns
- Configure retention policies

**Related Documentation:**
- [Vector Store Implementations](AI_PARTS_CATALOG.md#vector-store-implementations)

---

### OpenSearch Integration

**Use Case:** Search and analytics with vector capabilities.

**Example:** [examples/opensearch_basic.rs](../examples/opensearch_basic.rs)

**Key Features:**
- **Full-text + Vector** - combines BM25 with k-NN
- **Analytics** - aggregations and dashboards
- **Scalable** - distributed across nodes

**Best Practices:**
- Use index aliases for zero-downtime updates
- Configure shard count based on data size
- Enable slow query logging

**Related Documentation:**
- [Vector Store Implementations](AI_PARTS_CATALOG.md#vector-store-implementations)

---

### SQLiteVSS Vector Store

**Use Case:** Embedded vector search with SQLite.

**Example:** [examples/sqlitevss_basic.rs](../examples/sqlitevss_basic.rs)

**Key Features:**
- **Embedded** - no separate database server
- **Lightweight** - single file database
- **Fast** - uses FAISS under the hood

**Best Practices:**
- Use for development and small applications
- Single-writer limitation (SQLite constraint)
- Consider Write-Ahead Logging (WAL) mode

**Related Documentation:**
- [Vector Store Implementations](AI_PARTS_CATALOG.md#vector-store-implementations)

---

### Hybrid Search

**Use Case:** Combine semantic (vector) and keyword (BM25) search.

**Example:** See [librarian](../examples/apps/librarian/) for production RAG implementation

```rust
// Hybrid search combines vector similarity with keyword matching
let results = store.hybrid_search(
    query,
    alpha: 0.7,  // 0.7 = 70% vector, 30% keyword
    limit: 10,
).await?;
```

**Key Concepts:**
- Alpha parameter balances vector vs keyword
- Vector search: semantic similarity
- Keyword search: exact matches (BM25)
- Fusion strategies: RRF (Reciprocal Rank Fusion)

**Best Practices:**
- Start with alpha=0.5, tune based on results
- Use vector for conceptual queries
- Use keyword for names, codes, exact phrases
- Evaluate with metrics (MRR, NDCG)

---

## 🔄 DashFlow Workflows

### Multi-Agent Research Team

**Use Case:** Coordinate multiple specialized agents for complex tasks.

**Example:** Conceptual example (pattern demonstrated in [librarian](../examples/apps/librarian/))

```rust
use dashflow::{Graph, StateGraph};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define agents
    let researcher = Agent::new("researcher", research_tool);
    let writer = Agent::new("writer", writing_tool);
    let reviewer = Agent::new("reviewer", review_tool);

    // Build graph
    let mut graph = StateGraph::new();
    graph.add_node("research", researcher);
    graph.add_node("write", writer);
    graph.add_node("review", reviewer);

    // Define workflow
    graph.add_edge("research", "write");
    graph.add_edge("write", "review");
    graph.add_conditional_edge("review", should_revise, vec![
        ("revise", "write"),
        ("approve", END),
    ]);

    let app = graph.compile()?;
    let result = app.invoke(initial_state).await?;

    Ok(())
}
```

**Key Concepts:**
- **StateGraph** - stateful workflow with cycles
- **Nodes** - individual agent functions
- **Edges** - control flow between nodes
- **Conditional edges** - dynamic routing based on state

**Best Practices:**
- Design clear agent responsibilities
- Use shared state for communication
- Add human-in-the-loop for critical decisions
- Monitor token usage across agents

**Related Documentation:**
- [DashFlow Components](AI_PARTS_CATALOG.md#dashflow-components)
- [Multi-Agent Patterns](ADVANCED_AGENT_PATTERNS.md)

---

### Error Recovery Patterns

**Use Case:** Gracefully handle failures and retry with different strategies.

**Example:** See `examples/apps/librarian/` for error handling patterns in production code.

```rust
// Self-correcting loop with max retries
graph.add_conditional_edge("execute", check_result, vec![
    ("success", END),
    ("retry", "execute"),
    ("fallback", "use_simpler_model"),
]);
```

**Key Patterns:**
1. **Retry with backoff** - exponential delay between attempts
2. **Model cascade** - fall back to cheaper/simpler model
3. **Human escalation** - request human help after N failures
4. **Validation loops** - verify output quality before proceeding

**Best Practices:**
- Set max retries to prevent infinite loops
- Log failures for debugging
- Use structured error types
- Implement circuit breakers for external services

**Related Documentation:**
- [Error Recovery Patterns](ADVANCED_AGENT_PATTERNS.md#error-recovery)

---

### Checkpointing & Resume

**Use Case:** Save workflow state and resume from interruption.

**Example:** See `dashflow-postgres-checkpointer` or `dashflow-redis-checkpointer` crates for production implementations.

```rust
use dashflow::checkpoint::MemoryCheckpointer;

// Create checkpointer
let checkpointer = MemoryCheckpointer::new();

// Compile graph with checkpointing
let app = graph.compile_with_checkpointer(checkpointer)?;

// Run with thread_id for state tracking
let config = RunnableConfig {
    thread_id: Some("session-123".to_string()),
    ..Default::default()
};

// First run - will checkpoint after each node
let result = app.invoke(state, config.clone()).await?;

// Resume from checkpoint
let resumed = app.invoke(new_input, config).await?;
```

**Key Concepts:**
- **Checkpointer** - persists state after each node
- **thread_id** - identifies conversation/session
- **Resume** - continue from last checkpoint
- **Time travel** - replay from any checkpoint

**Checkpoint Backends:**
- **Memory** - development/testing (not persistent)
- **Postgres** - production (JSONB column)
- **Redis** - fast access (TTL support)
- **S3** - archival (compressed)
- **DynamoDB** - serverless (auto-scaling)

**Best Practices:**
- Always use persistent checkpointer in production
- Set retention policies to limit storage
- Use compression for large states
- Test resume paths thoroughly

**Related Documentation:**
- [Checkpointing Guide](AI_PARTS_CATALOG.md#checkpointing)

---

### S3 Checkpointing

**Use Case:** Store checkpoints in S3 for serverless applications.

**Example:** [examples/s3_checkpointing.rs](../examples/s3_checkpointing.rs)

```rust
// Requires dashflow-s3-checkpointer crate
use dashflow_s3_checkpointer::S3Checkpointer;

let checkpointer = S3Checkpointer::builder()
    .bucket("my-checkpoints")
    .prefix("agent-sessions/")
    .compression(true)  // gzip compression
    .build()?;

let app = graph.compile_with_checkpointer(checkpointer)?;
```

**Key Features:**
- **Serverless** - no database to manage
- **Scalable** - unlimited storage
- **Durable** - 99.999999999% durability
- **Cost-effective** - pay for storage used

**Best Practices:**
- Enable compression to reduce costs
- Use lifecycle policies for automatic cleanup
- Set object tags for organization
- Use S3 Select for analytics

**Related Documentation:**
- [Checkpointing](AI_PARTS_CATALOG.md#checkpointing)

---

### Streaming Aggregation

**Use Case:** Stream partial results as they become available.

**Example:** See `examples/apps/librarian/` for streaming patterns with DashStream integration.

```rust
// Stream events from graph execution
let mut stream = app.stream(input, config).await?;

while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::NodeStart(name) => {
            println!("Starting node: {}", name);
        }
        StreamEvent::NodeEnd { name, output } => {
            println!("Completed {}: {:?}", name, output);
        }
        StreamEvent::LLMToken(token) => {
            print!("{}", token);  // Real-time token streaming
        }
    }
}
```

**Key Concepts:**
- **Event stream** - async iterator of execution events
- **Partial results** - intermediate outputs
- **Token streaming** - word-by-word LLM output
- **Progress tracking** - monitor long-running workflows

**Best Practices:**
- Buffer tokens for smoother UX
- Handle backpressure with bounded channels
- Implement timeouts for each node
- Log events for debugging

**Related Documentation:**
- [Streaming Guide](AI_PARTS_CATALOG.md#streaming-enhanced-in-v170)

---

### Multi-Model Comparison

**Use Case:** Compare outputs from multiple LLM models.

**Example:** See `dashflow-evals` crate for multi-model comparison patterns.

```rust
// Run same prompt through multiple models in parallel
let models: Vec<Box<dyn ChatModel>> = vec![
    Box::new(ChatOpenAI::new().with_model("gpt-4")),
    Box::new(ChatAnthropic::new().with_model("claude-3-sonnet-20240229")),
    Box::new(ChatBedrock::new().with_model("amazon.titan-text-express-v1")),
];

let results = futures::future::join_all(
    models.iter().map(|model| model.invoke(prompt))
).await;

// Compare outputs
for (model, result) in models.iter().zip(results) {
    println!("{}: {}", model.name(), result?);
}
```

**Use Cases:**
- **Quality comparison** - which model is better?
- **Cost optimization** - cheapest model that meets quality bar
- **Fallback strategies** - use backup if primary fails
- **Ensemble voting** - combine multiple model outputs

**Best Practices:**
- Normalize outputs before comparison
- Use LLM-as-judge for quality scoring
- Track cost and latency metrics
- A/B test model choices in production

**Related Documentation:**
- [Multi-Model Patterns](ADVANCED_AGENT_PATTERNS.md#multi-model)

---

## 📊 Observability & Monitoring

### Traced Agent

**Use Case:** Debug agent behavior with distributed tracing.

**Example:** [examples/traced_agent.rs](../examples/traced_agent.rs)

```rust
use dashflow_observability::{TracingConfig, init_tracing};

// Configure OpenTelemetry tracing (supports Jaeger, OTLP endpoints)
let config = TracingConfig::new()
    .with_service_name("my-agent")
    .with_otlp_endpoint("http://localhost:4317");
init_tracing(config).await?;

let agent = Agent::builder()
    .llm(llm)
    .tools(tools)
    .build()?;

// All agent steps will be traced via OpenTelemetry
let result = agent.run("What is 2+2?").await?;
```

**What Gets Traced:**
- **LLM calls** - prompts, responses, tokens, latency
- **Tool executions** - inputs, outputs, duration
- **Agent reasoning** - thought process and decisions
- **Errors** - full stack traces with context

**Best Practices:**
- Use different projects for dev/staging/prod
- Add custom metadata with `with_metadata()`
- Sample traces in high-traffic environments
- Set up alerts for error rate spikes

**Related Documentation:**
- [Distributed Tracing Guide](DISTRIBUTED_TRACING.md)
- [LangSmith Integration](AI_PARTS_CATALOG.md#langsmith-integration)

---

### Custom Metrics

**Use Case:** Track application-specific metrics.

**Example:** [examples/custom_metrics_observability.rs](../examples/custom_metrics_observability.rs)

```rust
use dashflow_observability::{Metrics, MetricsConfig};

let metrics = Metrics::new(MetricsConfig {
    prometheus_port: 9090,
    ..Default::default()
})?;

// Custom counters
metrics.counter("queries_total").increment();
metrics.counter("errors_total").increment();

// Histograms for latency
metrics.histogram("query_duration_seconds").observe(0.123);

// Gauges for current state
metrics.gauge("active_sessions").set(42);

// Prometheus endpoint at http://localhost:9090/metrics
```

**Standard Metrics:**
- `llm_calls_total` - total LLM invocations
- `llm_tokens_total` - tokens consumed
- `llm_cost_dollars` - estimated cost
- `llm_duration_seconds` - latency distribution
- `tool_calls_total` - tool usage

**Best Practices:**
- Use labels for dimensionality (model, user, etc.)
- Avoid high-cardinality labels
- Set up Grafana dashboards
- Alert on p99 latency and error rate

**Related Documentation:**
- [Observability Guide](AI_PARTS_CATALOG.md#observability)

---

### DashFlow Streaming Telemetry

**Use Case:** Ultra-efficient binary telemetry for production.

**Key Features:**
- **10-100× more efficient** than JSON logging
- **Diff-based encoding** - only send state changes
- **Kafka streaming** - distributed collection
- **CLI inspector** - rich debugging tools

**Architecture:**
```
Agent → DashFlow Streaming Encoder → Kafka → Aggregator → Storage
                                   ↓
                              CLI Inspector
```

**Commands:**
```bash
# Tail live events
dashflow tail --thread session-123

# Inspect full conversation
dashflow inspect --thread session-123

# Profile token usage
dashflow profile --thread session-123

# Cost breakdown
dashflow costs --thread session-123 --by-node

# Generate flamegraph
dashflow flamegraph --thread session-123

# Replay conversation
dashflow timeline replay --thread session-123 --from-checkpoint checkpoint-42

# Diff checkpoints
dashflow diff --thread session-123 --checkpoint1 checkpoint-40 --checkpoint2 checkpoint-42

# Export to JSON
dashflow export --thread session-123 --format json > trace.json
```

**Best Practices:**
- Enable DashFlow Streaming in production (minimal overhead)
- Use Kafka for high-throughput scenarios
- Set retention policies on Kafka topics
- Use CLI for debugging production issues

**Related Documentation:**
- [DashFlow Streaming Protocol Spec](DASHSTREAM_PROTOCOL.md)
- [CLI Reference Guide](CLI_REFERENCE.md)

---

### Distributed Tracing Example

**Use Case:** Trace requests across multiple services.

```rust
// M-657: Fixed imports to use actual crate paths
use dashflow_observability::{TracingConfig, init_tracing};
use tracing::{info_span, Instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure OpenTelemetry tracing
    let config = TracingConfig::new()
        .with_service_name("my-service")
        .with_otlp_endpoint("http://jaeger:4317")
        .with_sampling_rate(1.0);

    // Initialize tracing (exports to Jaeger via OTLP)
    init_tracing(config).await?;

    // Create spans using tracing macros
    let span = info_span!("llm_call", model = "gpt-4");

    // Execute with span context
    let result = async {
        llm.invoke(prompt).await
    }
    .instrument(span)
    .await?;

    Ok(())
}
```

**Supported Backends:**
- **Jaeger** - distributed tracing
- **Zipkin** - alternative tracing
- **Honeycomb** - observability platform
- **Datadog** - full-stack monitoring

**Best Practices:**
- Use semantic conventions for span names
- Add context attributes (user_id, request_id)
- Sample traces (e.g., 1% in production)
- Correlate traces with logs

**Related Documentation:**
- [Distributed Tracing](DISTRIBUTED_TRACING.md)

---

## 🏗️ Production Applications

### Document Search (RAG)

**Use Case:** Production-ready retrieval-augmented generation.

**Example:** [examples/apps/librarian/](../examples/apps/librarian/) (production RAG paragon)

**Architecture:**
```
Query → Retriever → Reranker → Context Builder → LLM → Response
          ↓
    Vector Store
```

**Key Features:**
- **Hybrid search** - vector + keyword
- **Reranking** - improve relevance with cross-encoder
- **Citation tracking** - source attribution
- **Quality scoring** - LLM-as-judge evaluation
- **Streaming responses** - real-time output

**Performance (Measured):**
- **73× less memory** than Python (8.8 MB vs 644 MB)
- **1.5-4× faster** end-to-end latency
- **100% success rate** on evaluation suite

**Best Practices:**
- Use semantic chunking (not fixed-size)
- Enable caching for repeated queries
- Implement query expansion
- Track retrieval metrics (MRR, NDCG)

**Related Documentation:**
- [RAG Best Practices](book/src/examples/rag.md)
- [Document Search Architecture](APP_ARCHITECTURE_GUIDE.md)

---

### Code Assistant

**Use Case:** AI-powered coding assistant with tool use.

**Example:** See the paragon app implementation in [`examples/apps/codex-dashflow/`](../examples/apps/codex-dashflow/) (legacy repo archival note: [`docs/CODEX_DASHFLOW_ARCHIVE_NOTICE.md`](CODEX_DASHFLOW_ARCHIVE_NOTICE.md)).

**Tools Available:**
- File operations (read, write, search)
- Code analysis (AST parsing, type checking)
- Git operations (diff, commit, branch)
- Terminal execution (compile, test, run)
- Web search (documentation lookup)

**Safety Features:**
- Sandbox execution for untrusted code
- Approval required for destructive operations
- Token limits per operation
- Audit logging

**Best Practices:**
- Provide project context in system prompt
- Use specialized models for code (GPT-4, Claude)
- Implement undo/rollback
- Test assistant on real coding tasks

**Related Documentation:**
- [Code Assistant Guide](APP_ARCHITECTURE_GUIDE.md#code-assistant)

---

### Advanced RAG with Web Search

**Use Case:** Combine document retrieval with real-time web search.

**Example:** Conceptual example (pattern demonstrated in [librarian](../examples/apps/librarian/))

**Architecture:**
```
Query → Router → [Retriever] → Fusion → Context → LLM
            ↓      [Web Search]
       Classification
```

**Router Logic:**
- **Retrieval** - known domain questions
- **Web search** - current events, recent info
- **Both** - hybrid queries
- **Neither** - factual knowledge in LLM

**Best Practices:**
- Cache web search results
- Set recency thresholds (e.g., > 30 days old)
- Use specialized search APIs (e.g., Tavily)
- Verify source credibility

**Related Documentation:**
- [Advanced RAG Patterns](ADVANCED_AGENT_PATTERNS.md#advanced-rag)

---

### Optimized Document Search

**Use Case:** Production deployment with performance optimizations.

**Example:** Conceptual example (optimization patterns available in [librarian](../examples/apps/librarian/))

**Optimizations:**
1. **Query caching** - Redis cache for repeated queries
2. **Result caching** - Cache retrieval results
3. **Batch embeddings** - Process multiple queries together
4. **Parallel retrieval** - Query multiple stores concurrently
5. **Connection pooling** - Reuse database connections
6. **Compression** - Compress checkpoints and cache entries

**Performance Impact:**
- **10× faster** for cache hits
- **50% cost reduction** from caching
- **5× higher throughput** from batching

**Best Practices:**
- Monitor cache hit rate
- Set TTLs based on data freshness requirements
- Use LRU eviction for memory-constrained environments
- Profile before optimizing

**Related Documentation:**
- [Performance Optimization](BEST_PRACTICES.md#performance-optimization)

---

## ⚡ Performance Optimization Recipes

Quick-reference patterns for optimizing common performance bottlenecks.

### Recipe 1: Reduce Latency with Streaming

**Problem:** Users wait too long for first response token.

**Solution:** Enable streaming for immediate feedback.

```rust
use dashflow::core::language_models::ChatModel;
use dashflow_openai::build_chat_model;
use futures::StreamExt;

// See note at top of cookbook for config-driven instantiation
let config: ChatModelConfig = serde_yaml::from_str("type: openai\nmodel: gpt-4o-mini\napi_key: { env: OPENAI_API_KEY }")?;
let chat = build_chat_model(&config)?;
let mut stream = chat.stream(messages, None).await?;

// Process tokens as they arrive
while let Some(chunk) = stream.next().await {
    let token = chunk?;
    print!("{}", token);  // Immediate user feedback
}
```

**Impact:** First token in ~200ms vs ~2000ms for full response.

**Best For:** Chat interfaces, real-time applications.

---

### Recipe 2: Parallelize Independent Operations

**Problem:** Sequential execution wastes time on independent operations.

**Solution:** Use tokio::join! or futures::join_all for parallel execution.

```rust
use tokio::join;

// Bad: Sequential (6 seconds total)
let embeddings1 = embed_model.embed_query("query1").await?;  // 3s
let embeddings2 = embed_model.embed_query("query2").await?;  // 3s

// Good: Parallel (3 seconds total)
let (embeddings1, embeddings2) = join!(
    embed_model.embed_query("query1"),
    embed_model.embed_query("query2"),
);
let embeddings1 = embeddings1?;
let embeddings2 = embeddings2?;
```

**Impact:** 2-10× faster for independent operations.

**Best For:** Multiple retrieval queries, multi-model comparisons, batch processing.

---

### Recipe 3: Batch API Calls

**Problem:** Making N separate API calls for N items.

**Solution:** Use batch APIs when available.

```rust
// Bad: N API calls
for doc in docs {
    let embedding = model.embed_query(&doc).await?;
}

// Good: 1 API call
let texts: Vec<String> = docs.iter().map(|d| d.to_string()).collect();
let embeddings = model.embed_documents(&texts).await?;
```

**Impact:** 5-10× faster, lower API costs.

**Best For:** Embeddings, classification, validation tasks.

**Note:** Check provider rate limits and batch size limits.

---

### Recipe 4: Cache Expensive Operations

**Problem:** Repeated queries with identical results.

**Solution:** Add Redis or in-memory caching layer.

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

type Cache = Arc<RwLock<HashMap<String, Vec<Document>>>>;

async fn retrieve_with_cache(
    query: &str,
    retriever: &impl Retriever,
    cache: &Cache,
) -> Result<Vec<Document>> {
    // Check cache first
    if let Some(docs) = cache.read().await.get(query) {
        return Ok(docs.clone());  // Cache hit
    }

    // Cache miss - fetch and store
    let docs = retriever.get_relevant_documents(query, None).await?;
    cache.write().await.insert(query.to_string(), docs.clone());
    Ok(docs)
}
```

**Impact:** 10-100× faster for cache hits, 50%+ cost reduction.

**Best For:** Retrieval systems, FAQ bots, repeated queries.

**Trade-offs:** Stale data risk (use TTLs), memory usage.

---

### Recipe 5: Limit Concurrent Requests

**Problem:** Too many concurrent API calls hit rate limits or overwhelm resources.

**Solution:** Use tokio::sync::Semaphore to limit concurrency.

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

let semaphore = Arc::new(Semaphore::new(10));  // Max 10 concurrent

let tasks: Vec<_> = queries.into_iter().map(|query| {
    let sem = semaphore.clone();
    let model = model.clone();
    tokio::spawn(async move {
        let _permit = sem.acquire().await.unwrap();  // Wait for slot
        model.invoke(query).await  // Execute with limit
    })
}).collect();

let results = futures::future::join_all(tasks).await;
```

**Impact:** Avoids rate limit errors, prevents resource exhaustion.

**Best For:** Batch processing, background jobs, bulk operations.

**Tuning:** Start with provider's rate limit ÷ 2, adjust based on monitoring.

---

### Recipe 6: Use Connection Pooling

**Problem:** Creating new database connections for every request.

**Solution:** Use sqlx/deadpool for connection pooling.

```rust
use sqlx::postgres::PgPoolOptions;

// Bad: New connection per request
let conn = PgConnection::connect(&db_url).await?;
let result = query.fetch_one(&conn).await?;
conn.close().await?;

// Good: Reuse pooled connections
let pool = PgPoolOptions::new()
    .max_connections(20)
    .connect(&db_url).await?;

// Automatically returns connection to pool when dropped
let result = query.fetch_one(&pool).await?;
```

**Impact:** 10-50× faster connection reuse, lower resource usage.

**Best For:** Database checkpointers, vector stores, any DB access.

**Configuration:** `max_connections = (CPU cores × 2) + disk spindles`

---

### Recipe 7: Optimize Token Usage

**Problem:** High API costs from excessive token usage.

**Solution:** Reduce input/output tokens without sacrificing quality.

**Techniques:**

```rust
use dashflow_openai::build_chat_model;

// 1. Use smaller models for simple tasks (config-driven, see note at top)
let cheap_config: ChatModelConfig = serde_yaml::from_str("type: openai\nmodel: gpt-4o-mini\napi_key: { env: OPENAI_API_KEY }")?;
let expensive_config: ChatModelConfig = serde_yaml::from_str("type: openai\nmodel: gpt-4o\napi_key: { env: OPENAI_API_KEY }")?;
let cheap_model = build_chat_model(&cheap_config)?;     // 60% cheaper
let expensive_model = build_chat_model(&expensive_config)?;

// Route based on complexity
let model = if is_simple_query { &cheap_model } else { &expensive_model };

// 2. Limit max_tokens for responses
let chat = chat.with_max_tokens(Some(500));  // Prevent verbose responses

// 3. Trim context to essentials
let context = documents.iter()
    .take(5)  // Top 5 docs only
    .map(|d| d.page_content.chars().take(500).collect::<String>())  // 500 chars each
    .collect::<Vec<_>>()
    .join("\n\n");

// 4. Use prompt compression (hypothetical)
let compressed_prompt = compress_prompt(&long_prompt)?;
```

**Impact:** 40-60% cost reduction, similar quality.

**Best For:** High-volume applications, cost-sensitive deployments.

**Monitoring:** Track cost per query, quality metrics (don't optimize blindly).

---

### Recipe 8: Choose the Right Data Structure

**Problem:** Slow retrieval from inefficient data structures.

**Solution:** Match data structure to access pattern.

```rust
// Bad: Vec for frequent lookups (O(n))
let documents: Vec<Document> = load_docs();
let doc = documents.iter().find(|d| d.id == target_id);  // Linear search

// Good: HashMap for ID lookups (O(1))
let documents: HashMap<String, Document> = load_docs()
    .into_iter()
    .map(|d| (d.id.clone(), d))
    .collect();
let doc = documents.get(&target_id);  // Constant time

// Good: BTreeMap for range queries (O(log n))
use std::collections::BTreeMap;
let by_timestamp: BTreeMap<u64, Document> = load_docs()
    .into_iter()
    .map(|d| (d.timestamp, d))
    .collect();
let recent = by_timestamp.range(start_time..end_time);  // Efficient range
```

**Impact:** 10-1000× faster lookups depending on size.

**Best For:** In-memory caches, document stores, state management.

---

### Recipe 9: Profile Before Optimizing

**Problem:** Optimizing the wrong thing, wasting time.

**Solution:** Measure with cargo-flamegraph or tokio-console.

```bash
# Install profiling tools
cargo install flamegraph
cargo install --locked tokio-console

# Generate flamegraph
cargo flamegraph --bin my_app -- --input test.txt

# Monitor async tasks
RUSTFLAGS="--cfg tokio_unstable" cargo run --features tokio-console
# Open http://127.0.0.1:6669 in browser
```

**Flamegraph:** Shows where CPU time is spent (red = hot path).

**tokio-console:** Shows async task performance (waiting, polling, spawning).

**Impact:** Identifies real bottlenecks, prevents premature optimization.

**Best For:** Performance tuning, debugging slow applications.

---

### Recipe 10: Use Release Builds for Benchmarking

**Problem:** Benchmarking debug builds gives misleading results.

**Solution:** Always use --release for performance measurement.

```bash
# Bad: Debug build (10-100× slower)
cargo run --bin my_app

# Good: Release build (optimized)
cargo run --release --bin my_app

# Benchmark with criterion
cargo bench --bench my_benchmark
```

**Impact:** Debug: ~100ms/query. Release: ~5ms/query (20× faster).

**Best For:** All performance measurements, production deployments.

**Note:** Debug builds have bounds checking, no inlining, no optimizations.

---

### Performance Checklist

Before deploying to production:

- [ ] Use release builds (`cargo build --release`)
- [ ] Enable streaming for user-facing responses
- [ ] Parallelize independent operations (tokio::join!)
- [ ] Batch API calls where possible
- [ ] Add caching for repeated queries (Redis/in-memory)
- [ ] Limit concurrent requests (Semaphore)
- [ ] Use connection pooling for databases
- [ ] Optimize token usage (model selection, max_tokens)
- [ ] Profile with flamegraph/tokio-console
- [ ] Monitor: latency, throughput, cost, error rate

**Related Documentation:**
- [BEST_PRACTICES.md - Performance Optimization](BEST_PRACTICES.md#performance-optimization)
- [MEMORY_BENCHMARKS.md](MEMORY_BENCHMARKS.md)
- [PERFORMANCE_BASELINE.md](PERFORMANCE_BASELINE.md)

---

## 🛡️ Resilience Patterns

Production LLM applications must handle failures gracefully. These patterns help build robust systems that recover from errors, prevent cascading failures, and maintain service quality.

### Circuit Breaker Pattern

**Problem:** When an external service (LLM API, vector store) fails, retrying immediately can overwhelm the failing service and cascade failures across your system.

**Solution:** Implement a circuit breaker that "opens" after consecutive failures, preventing requests until the service recovers.

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
enum CircuitState {
    Closed,           // Normal operation
    Open(Instant),    // Blocking requests (stores when opened)
    HalfOpen,        // Testing if service recovered
}

struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: u32,
    failure_count: Arc<RwLock<u32>>,
    timeout: Duration,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_threshold,
            failure_count: Arc::new(RwLock::new(0)),
            timeout,
        }
    }

    async fn call<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        // Check circuit state
        let state = self.state.read().await.clone();
        match state {
            CircuitState::Open(opened_at) => {
                if opened_at.elapsed() > self.timeout {
                    // Timeout expired, try half-open
                    *self.state.write().await = CircuitState::HalfOpen;
                } else {
                    return Err(/* circuit open error */);
                }
            }
            CircuitState::Closed | CircuitState::HalfOpen => {}
        }

        // Execute the operation
        match f.await {
            Ok(result) => {
                // Success - reset failure count and close circuit
                *self.failure_count.write().await = 0;
                *self.state.write().await = CircuitState::Closed;
                Ok(result)
            }
            Err(e) => {
                // Failure - increment count and possibly open circuit
                let mut count = self.failure_count.write().await;
                *count += 1;

                if *count >= self.failure_threshold {
                    *self.state.write().await = CircuitState::Open(Instant::now());
                }

                Err(e)
            }
        }
    }
}

// Usage with LLM
use dashflow::core::language_models::ChatModel;
use dashflow_openai::ChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();
    let circuit = CircuitBreaker::new(3, Duration::from_secs(30));

    // Call LLM through circuit breaker
    let result = circuit.call(async {
        llm.invoke("What is Rust?").await
    }).await?;

    Ok(())
}
```

**Impact:** Prevents cascading failures, reduces load on failing services, improves overall system stability.

**Best For:** External API calls (LLM providers, databases, vector stores), microservice communication.

**Configuration:**
- `failure_threshold`: 3-5 consecutive failures (start conservative)
- `timeout`: 30-60 seconds (time to wait before retrying)
- Monitor: circuit state changes, blocked request count

---

### Retry with Exponential Backoff

**Problem:** Transient failures (rate limits, temporary network issues) should be retried, but constant retries can overwhelm services.

**Solution:** Retry with exponentially increasing delays between attempts.

```rust
use tokio::time::{sleep, Duration};
use std::cmp::min;

async fn retry_with_backoff<F, T, E>(
    mut f: impl FnMut() -> F,
    max_retries: u32,
    initial_delay: Duration,
    max_delay: Duration,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt >= max_retries => {
                return Err(e);
            }
            Err(e) => {
                attempt += 1;

                // Calculate backoff: initial_delay * 2^attempt
                let delay_ms = initial_delay.as_millis() * 2_u128.pow(attempt - 1);
                let delay = Duration::from_millis(
                    min(delay_ms as u64, max_delay.as_millis() as u64)
                );

                eprintln!("Attempt {} failed: {}. Retrying in {:?}", attempt, e, delay);
                sleep(delay).await;
            }
        }
    }
}

// Usage with LLM
use dashflow_openai::ChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();

    let result = retry_with_backoff(
        || async {
            llm.invoke("Explain quantum computing").await
        },
        max_retries: 3,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(10),
    ).await?;

    println!("Result: {}", result);
    Ok(())
}
```

**Backoff Schedule Example:**
```
Attempt 1: Immediate
Attempt 2: 500ms delay
Attempt 3: 1s delay
Attempt 4: 2s delay
Attempt 5: 4s delay (capped at max_delay)
```

**Impact:** 90%+ success rate on transient failures, avoids overwhelming services.

**Best For:** API rate limits, temporary network issues, database connection errors.

**Configuration:**
- `max_retries`: 3-5 attempts (more wastes time on permanent failures)
- `initial_delay`: 500ms-1s
- `max_delay`: 10-30s (prevent unbounded waits)

**Advanced:** Add jitter (random delay variation) to prevent thundering herd:
```rust
use rand::Rng;

let jitter = rand::thread_rng().gen_range(0..100);
let delay = delay + Duration::from_millis(jitter);
```

---

### Timeout Patterns

**Problem:** LLM requests can hang indefinitely, blocking resources and degrading user experience.

**Solution:** Enforce timeouts on all external operations.

```rust
use tokio::time::{timeout, Duration};

async fn llm_with_timeout(
    prompt: &str,
    timeout_duration: Duration,
) -> Result<String, Box<dyn std::error::Error>> {
    use dashflow_openai::ChatOpenAI;
    let llm = ChatOpenAI::default();

    match timeout(timeout_duration, llm.invoke(prompt)).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err("Request timed out".into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Short timeout for user-facing requests
    match llm_with_timeout("Quick summary of Rust", Duration::from_secs(5)).await {
        Ok(result) => println!("Result: {}", result),
        Err(e) => eprintln!("Request failed: {}", e),
    }

    Ok(())
}
```

**Timeout Budgets by Operation:**

| Operation Type | Recommended Timeout | Rationale |
|---|---|---|
| Simple queries | 5-10s | User-facing, expect fast response |
| RAG retrieval | 15-30s | Multiple steps: embed + search + generate |
| Batch processing | 60-120s | Background job, not user-facing |
| Streaming (first token) | 2-5s | User expects immediate feedback |

**Impact:** Prevents resource exhaustion, improves user experience (fail fast).

**Best For:** All external operations (LLM calls, database queries, vector searches).

**Advanced - Cascading Timeouts:**

```rust
async fn rag_query_with_timeouts(query: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Overall budget: 30 seconds
    timeout(Duration::from_secs(30), async {
        // Embed query: 5 second timeout
        let embedding = timeout(
            Duration::from_secs(5),
            embed_query(query)
        ).await??;

        // Vector search: 10 second timeout
        let docs = timeout(
            Duration::from_secs(10),
            vector_search(embedding)
        ).await??;

        // LLM generation: 15 second timeout (remaining budget)
        let result = timeout(
            Duration::from_secs(15),
            generate_response(query, docs)
        ).await??;

        Ok(result)
    }).await?
}
```

---

### Graceful Degradation

**Problem:** When primary services fail, completely failing the request provides poor user experience.

**Solution:** Implement fallback strategies that provide partial functionality.

```rust
use dashflow_openai::ChatOpenAI;
use dashflow_anthropic::ChatAnthropic;

async fn multi_provider_fallback(
    prompt: &str
) -> Result<String, Box<dyn std::error::Error>> {
    // Try primary provider (OpenAI)
    let primary = ChatOpenAI::default();
    match timeout(Duration::from_secs(10), primary.invoke(prompt)).await {
        Ok(Ok(result)) => return Ok(result),
        Ok(Err(e)) => eprintln!("Primary provider failed: {}", e),
        Err(_) => eprintln!("Primary provider timed out"),
    }

    // Fallback to secondary provider (Anthropic)
    let secondary = ChatAnthropic::default();
    match timeout(Duration::from_secs(10), secondary.invoke(prompt)).await {
        Ok(Ok(result)) => return Ok(result),
        Ok(Err(e)) => eprintln!("Secondary provider failed: {}", e),
        Err(_) => eprintln!("Secondary provider timed out"),
    }

    // Final fallback: cached response or default message
    Ok("Service temporarily unavailable. Please try again.".to_string())
}

// Cached fallback for common queries
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

struct CachedLLM {
    llm: ChatOpenAI,
    cache: Arc<RwLock<HashMap<String, String>>>,
}

impl CachedLLM {
    async fn invoke_with_fallback(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Try LLM first
        match self.llm.invoke(prompt).await {
            Ok(result) => {
                // Cache successful response
                self.cache.write().await.insert(prompt.to_string(), result.clone());
                Ok(result)
            }
            Err(_) => {
                // Fallback to cache
                if let Some(cached) = self.cache.read().await.get(prompt) {
                    Ok(format!("[Cached] {}", cached))
                } else {
                    Err("No cached response available".into())
                }
            }
        }
    }
}
```

**Degradation Strategies:**

1. **Provider Fallback:** Try alternative LLM providers (OpenAI → Anthropic → Cohere)
2. **Model Fallback:** Use cheaper/faster models (GPT-4 → GPT-3.5 → cached)
3. **Cached Response:** Return stale but useful data
4. **Partial Results:** Return what you have (e.g., search results without LLM summary)
5. **Default Message:** Better than nothing (e.g., "Service unavailable")

**Impact:** Improved availability (99.9% → 99.99%), better user experience.

**Best For:** User-facing applications, critical paths.

**Metrics to Monitor:**
- Fallback trigger rate (should be <1% in healthy system)
- Response time by tier (primary vs fallback)
- Cache hit rate for fallback tier

---

### Resilience Checklist

Before deploying to production, verify:

- [ ] **Circuit breakers** on all external service calls
- [ ] **Retry logic** with exponential backoff (3-5 attempts)
- [ ] **Timeouts** on all operations (appropriate to operation type)
- [ ] **Fallback strategies** for critical user paths
- [ ] **Health checks** to detect failing services early
- [ ] **Monitoring** for: error rates, timeout rates, circuit breaker state, fallback usage
- [ ] **Alerting** when error/fallback rates exceed thresholds
- [ ] **Load testing** to verify resilience under stress

**Related Documentation:**
- [Error Recovery Patterns](ADVANCED_AGENT_PATTERNS.md#error-recovery)
- [BEST_PRACTICES.md - Error Handling](BEST_PRACTICES.md#error-handling)
- [PRODUCTION_DEPLOYMENT.md](PRODUCTION_DEPLOYMENT.md)

---

## 💰 Cost Optimization Patterns

LLM API costs can quickly become the largest expense in production applications. These patterns help reduce costs while maintaining quality and performance.

### Pattern 1: Model Selection and Routing

**Problem:** Using expensive models (GPT-4, Claude Opus) for all tasks wastes money on simple queries.

**Solution:** Route queries to the cheapest model that can handle them.

```rust
use dashflow::core::language_models::ChatModel;

#[derive(Debug)]
enum QueryComplexity {
    Simple,      // Factual, structured data extraction
    Medium,      // Analysis, summarization
    Complex,     // Creative writing, reasoning, code generation
}

async fn route_query(
    query: &str,
    complexity: QueryComplexity,
) -> Result<String, Box<dyn std::error::Error>> {
    // Route based on complexity
    let response = match complexity {
        QueryComplexity::Simple => {
            // Use cheapest model: GPT-3.5-turbo ($0.0005/1K tokens) or Claude Haiku ($0.00025/1K tokens)
            let llm = dashflow_openai::OpenAI::default()
                .with_model("gpt-3.5-turbo");
            llm.invoke(query).await?
        }
        QueryComplexity::Medium => {
            // Use mid-tier: GPT-4-mini ($0.00015/1K in, $0.0006/1K out)
            let llm = dashflow_openai::OpenAI::default()
                .with_model("gpt-4o-mini");
            llm.invoke(query).await?
        }
        QueryComplexity::Complex => {
            // Use flagship: GPT-4 ($0.03/1K in, $0.06/1K out) or Claude Opus ($0.015/1K in, $0.075/1K out)
            let llm = dashflow_openai::OpenAI::default()
                .with_model("gpt-4");
            llm.invoke(query).await?
        }
    };

    Ok(response)
}
```

**Complexity Classification:**

```rust
fn classify_query_complexity(query: &str) -> QueryComplexity {
    // Simple heuristics (consider using a classifier model for production)
    let has_keywords = |keywords: &[&str]| {
        keywords.iter().any(|k| query.to_lowercase().contains(k))
    };

    // Complex indicators: creative, reasoning, code
    if has_keywords(&["write", "create", "design", "explain why", "analyze", "code", "function", "implement"]) {
        return QueryComplexity::Complex;
    }

    // Medium indicators: summarization, comparison
    if has_keywords(&["summarize", "compare", "what's the difference", "pros and cons"]) {
        return QueryComplexity::Medium;
    }

    // Default to simple for factual/extraction queries
    QueryComplexity::Simple
}
```

**Impact:** 60-80% cost reduction with minimal quality loss (most queries are simple/medium).

**Best For:** High-volume applications, chatbots, Q&A systems.

**Monitoring:** Track cost per complexity tier, quality metrics by tier (A/B test routing logic).

---

### Pattern 2: Token Budgets and Limits

**Problem:** Long conversations or large contexts consume excessive tokens.

**Solution:** Set token budgets per operation and trim context when needed.

```rust
use dashflow::core::language_models::ChatModel;

const MAX_INPUT_TOKENS: usize = 4000;  // Budget per query
const MAX_OUTPUT_TOKENS: usize = 500;  // Limit response length

async fn query_with_budget(
    query: &str,
    context: &str,
    llm: &impl ChatModel,
) -> Result<String, Box<dyn std::error::Error>> {
    // Trim context if needed
    let context = trim_to_token_budget(context, MAX_INPUT_TOKENS - estimate_tokens(query));

    let prompt = format!("Context: {}\n\nQuery: {}", context, query);

    // Set max_tokens to limit output (prevents runaway generation)
    let response = llm
        .invoke(&prompt)
        .await?;

    Ok(response)
}

fn trim_to_token_budget(text: &str, max_tokens: usize) -> String {
    // Rough estimate: 1 token ≈ 4 characters (English text)
    let max_chars = max_tokens * 4;

    if text.len() <= max_chars {
        return text.to_string();
    }

    // Smart trimming: keep beginning and end, drop middle
    let keep_start = max_chars / 2;
    let keep_end = max_chars / 2;

    format!(
        "{}...\n[Content trimmed to fit token budget]\n...{}",
        &text[..keep_start],
        &text[text.len() - keep_end..]
    )
}

fn estimate_tokens(text: &str) -> usize {
    // Rough estimate: 1 token ≈ 4 characters
    // For production, use tiktoken library for accurate counts
    text.len() / 4
}
```

**Token Budgets by Operation:**

| Operation Type | Input Budget | Output Budget | Rationale |
|---------------|--------------|---------------|-----------|
| Simple Q&A | 500 tokens | 100 tokens | Short context, short answer |
| Summarization | 8,000 tokens | 500 tokens | Full document, brief summary |
| RAG retrieval | 4,000 tokens | 500 tokens | Retrieved chunks + query |
| Code generation | 2,000 tokens | 1,000 tokens | Context + detailed code |
| Chat conversation | 4,000 tokens | 500 tokens | Recent messages only |

**Impact:** 40-60% cost reduction by preventing context bloat.

**Best For:** RAG systems, chatbots, long documents.

**Advanced:** Use [tiktoken-rs](https://github.com/zurawiki/tiktoken-rs) for accurate token counting (different tokenizers per model).

---

### Pattern 3: Caching and Deduplication

**Problem:** Repeated identical queries waste money.

**Solution:** Cache responses and deduplicate similar queries.

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CachedResponse {
    response: String,
    created_at: Instant,
    cost: f64,  // Track savings
}

struct CostOptimizedLLM {
    llm: Arc<dyn ChatModel>,
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
    ttl: Duration,
    stats: Arc<RwLock<CostStats>>,
}

#[derive(Default)]
struct CostStats {
    cache_hits: u64,
    cache_misses: u64,
    cost_saved: f64,  // Dollars saved by caching
}

impl CostOptimizedLLM {
    async fn invoke(&self, query: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Normalize query for cache key (lowercase, trim whitespace)
        let cache_key = query.to_lowercase().trim().to_string();

        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                // Check TTL
                if cached.created_at.elapsed() < self.ttl {
                    // Cache hit!
                    let mut stats = self.stats.write().await;
                    stats.cache_hits += 1;
                    stats.cost_saved += cached.cost;
                    return Ok(cached.response.clone());
                }
            }
        }

        // Cache miss - call LLM
        let response = self.llm.invoke(query).await?;
        let cost = estimate_cost(query, &response, "gpt-4o-mini");

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, CachedResponse {
                response: response.clone(),
                created_at: Instant::now(),
                cost,
            });
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.cache_misses += 1;
        }

        Ok(response)
    }

    async fn get_stats(&self) -> CostStats {
        self.stats.read().await.clone()
    }
}

fn estimate_cost(input: &str, output: &str, model: &str) -> f64 {
    // Rough cost estimates (per 1K tokens, as of Nov 2025)
    let (input_cost_per_1k, output_cost_per_1k) = match model {
        "gpt-3.5-turbo" => (0.0005, 0.0015),
        "gpt-4o-mini" => (0.00015, 0.0006),
        "gpt-4" => (0.03, 0.06),
        "claude-3-haiku" => (0.00025, 0.00125),
        "claude-3-opus" => (0.015, 0.075),
        _ => (0.001, 0.002),  // Default estimate
    };

    let input_tokens = input.len() / 4;  // Rough estimate
    let output_tokens = output.len() / 4;

    (input_tokens as f64 / 1000.0) * input_cost_per_1k +
    (output_tokens as f64 / 1000.0) * output_cost_per_1k
}
```

**Impact:** 70-90% cost reduction for repeated queries (common in FAQs, support systems).

**Cache TTL Guidance:**
- FAQ/support: 24 hours (questions/answers rarely change)
- News/current events: 1 hour (content changes frequently)
- User-specific: 15 minutes (avoid stale personalization)

**Best For:** FAQ bots, customer support, documentation search.

**Monitoring:** Cache hit rate (target 60%+), cost saved, cache memory usage.

---

### Pattern 4: Batch Processing

**Problem:** Processing items one-by-one incurs overhead and misses batch discounts.

**Solution:** Batch requests when possible (especially for embeddings).

```rust
use dashflow::core::embeddings::Embeddings;

async fn embed_documents_batched(
    texts: Vec<String>,
    embeddings: &impl Embeddings,
) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    const BATCH_SIZE: usize = 100;  // Provider-specific limit

    let mut all_embeddings = Vec::new();

    // Process in batches
    for chunk in texts.chunks(BATCH_SIZE) {
        let batch_embeddings = embeddings.embed_documents(chunk.to_vec()).await?;
        all_embeddings.extend(batch_embeddings);
    }

    Ok(all_embeddings)
}
```

**Batch Size Guidance:**

| Provider | Embeddings | Chat Completion | Rate Limit |
|----------|-----------|----------------|------------|
| OpenAI | 2,048 texts | N/A (single) | 3,000 RPM |
| Anthropic | N/A | N/A (single) | 1,000 RPM |
| Cohere | 96 texts | N/A | 10,000 RPM |
| Voyage AI | 128 texts | N/A | 300 RPM |

**Impact:** 5-10× faster, same cost but better throughput.

**Best For:** Document ingestion, bulk analysis, data pipelines.

**Note:** Some providers offer batch APIs with significant discounts (50% off) but with higher latency (24-48 hours). See OpenAI Batch API.

---

### Pattern 5: Prompt Optimization

**Problem:** Verbose prompts waste tokens.

**Solution:** Compress prompts without losing information.

**Before (Verbose):**

```rust
let prompt = format!(
    "You are a helpful assistant. Please analyze the following document and provide a comprehensive summary. \
    Make sure to include the main points, key findings, and important details. \
    Be thorough but concise. Here is the document:\n\n{}\n\n\
    Please provide your summary below:",
    document
);
```

**After (Optimized):**

```rust
let prompt = format!(
    "Summarize key points:\n\n{}\n\nSummary:",
    document
);
```

**Token Savings:** 85% reduction (120 tokens → 18 tokens) with same output quality.

**Prompt Compression Techniques:**

1. **Remove filler words:** "please", "make sure to", "be thorough but concise"
2. **Use shorter instructions:** "Summarize" vs "provide a comprehensive summary"
3. **Remove redundant context:** LLMs don't need "You are a helpful assistant"
4. **Use examples instead of instructions:** Few-shot prompting is more concise than long explanations
5. **Use structured formats:** JSON schema vs natural language instructions

**Example - Structured vs Natural:**

```rust
// Natural language (verbose): ~200 tokens
let prompt = "Extract the person's name, age, and location from the text.
Format the output as JSON with fields 'name', 'age', and 'location'.";

// Structured (concise): ~50 tokens
let prompt = "Extract as JSON: {name: str, age: int, location: str}";
```

**Impact:** 40-70% token reduction, same quality.

**Best For:** High-volume applications where every token counts.

**Testing:** A/B test compressed prompts to ensure quality doesn't degrade.

---

### Cost Optimization Checklist

Before deploying to production, ensure:

- [ ] **Model routing** to use cheaper models for simple queries
- [ ] **Token budgets** set per operation type (prevent runaway costs)
- [ ] **Caching** for repeated queries (target 50%+ hit rate)
- [ ] **Batch processing** for embeddings and bulk operations
- [ ] **Prompt optimization** (remove verbose instructions)
- [ ] **Monitoring:** Cost per query, token usage, cache hit rate
- [ ] **Alerting:** Cost spike detection (daily budget exceeded)
- [ ] **Fallbacks:** Cheaper models when primary is unavailable

**Related Documentation:**
- [Recipe 7: Optimize Token Usage](#recipe-7-optimize-token-usage)
- [BEST_PRACTICES.md - Cost Management](BEST_PRACTICES.md)
- Provider Rate Limits - see individual provider crate documentation

---

## 🎯 Advanced Patterns

### NatBot Browser Automation

**Use Case:** LLM-controlled browser automation.

**Example:** [examples/natbot_example.rs](../examples/natbot_example.rs)

**Capabilities:**
- Navigate web pages
- Fill forms
- Click buttons
- Extract information
- Take screenshots

**Architecture:**
```
LLM → Actions → Browser (Playwright) → DOM → Observation → LLM
       ↑                                                      ↓
       └──────────────── Reasoning Loop ────────────────────┘
```

**Best Practices:**
- Add delays for page loading
- Validate actions before execution
- Use selectors defensively
- Handle popups and alerts
- Set max steps to prevent infinite loops

**Related Documentation:**
- [NatBot Pattern](ADVANCED_AGENT_PATTERNS.md#natbot)

---

### AWS Bedrock Integration

**Use Case:** Use AWS-managed foundation models.

**Example:** [examples/bedrock_demo.rs](../examples/bedrock_demo.rs)

```rust
use dashflow_aws_bedrock::Bedrock;

let bedrock = Bedrock::builder()
    .model_id("amazon.titan-text-express-v1")
    .region("us-east-1")
    .build()?;

let response = bedrock.invoke("Tell me about Rust").await?;
```

**Supported Models:**
- Amazon Titan
- Anthropic Claude
- AI21 Jurassic
- Cohere Command
- Meta Llama

**Best Practices:**
- Use IAM roles for authentication (not access keys)
- Enable CloudWatch logging
- Set up billing alerts
- Test cross-region failover

**Related Documentation:**
- [AWS Bedrock](AI_PARTS_CATALOG.md#aws-bedrock)

---

### Quality Evaluation

**Use Case:** Systematic evaluation of AI application quality.

**Example:** See [dashflow-evals](../crates/dashflow-evals/) and [librarian](../examples/apps/librarian/)

**Evaluation Framework:**
```rust
use dashflow_evals::{Evaluator, QualityDimensions};

let evaluator = Evaluator::builder()
    .judge_model(OpenAI::new("gpt-4"))
    .dimensions(QualityDimensions::all())
    .build()?;

// Run evaluation
let results = evaluator.evaluate_dataset(
    test_cases,
    |input| app.invoke(input)
).await?;

// Generate report
results.save_html_report("eval_report.html")?;
```

**Quality Dimensions:**
1. **Accuracy** - factual correctness
2. **Relevance** - answers the question
3. **Completeness** - comprehensive coverage
4. **Safety** - no harmful content
5. **Coherence** - logical flow
6. **Conciseness** - not too verbose

**Metrics:**
- Per-dimension scores (0.0-1.0)
- Overall quality score
- Pass/fail rate
- Regression detection
- Cost per query
- P50/P90/P95/P99 latency

**Best Practices:**
- Create diverse test scenarios
- Use LLM-as-judge for subjective metrics
- Track metrics over time
- Set quality gates in CI/CD
- Investigate regressions immediately

**Related Documentation:**
- [Evaluation Guide](EVALUATION_GUIDE.md)
- [Evaluation Best Practices](EVALUATION_BEST_PRACTICES.md)
- [Evaluation Troubleshooting](EVALUATION_TROUBLESHOOTING.md)
- [World-Class Evals System](COMPLETED_INITIATIVES.md#world-class-evals-system)

---

## 🧪 Prompt Optimization

DashFlow includes 14 optimization algorithms for improving prompt performance automatically.

### Bootstrap FewShot

**Use Case:** Automatically select few-shot examples that improve prompt accuracy.

```rust
use dashflow::optimize::{Bootstrap, BootstrapConfig};

let optimizer = Bootstrap::new(BootstrapConfig {
    max_bootstrapped_demos: 4,
    max_labeled_demos: 16,
    max_rounds: 1,
    ..Default::default()
});

let optimized_prompt = optimizer.compile(
    program,
    training_set,
    metric_fn,
).await?;
```

**Key Concepts:**
- Bootstrapping generates synthetic examples from model outputs
- Examples are filtered by the metric function
- Iterative refinement improves quality over rounds

**Best Practices:**
- Start with `max_rounds: 1` for quick iteration
- Use diverse training examples
- Monitor metric scores across rounds

---

### MIPROv2 Optimizer

**Use Case:** LLM-based instruction and example optimization.

```rust
use dashflow::optimize::{MIPROv2, MIPROv2Config};

let optimizer = MIPROv2::new(MIPROv2Config {
    num_candidates: 10,
    init_temperature: 1.4,
    num_batches: 40,
    ..Default::default()
});

let result = optimizer.compile(
    program,
    training_set,
    eval_metric,
).await?;
```

**Key Concepts:**
- Uses LLM to propose improved instructions
- Bayesian optimization for hyperparameter search
- Evaluates candidates against training data

**Best Practices:**
- Larger `num_candidates` improves quality but costs more
- Use `verbose: true` to monitor progress
- Save best prompts for production use

---

### COPRO Instruction Optimization

**Use Case:** Coordinate-ascent prompt optimization for instructions.

```rust
use dashflow::optimize::{COPRO, COPROConfig};

let optimizer = COPRO::new(COPROConfig {
    breadth: 10,
    depth: 3,
    ..Default::default()
});

let optimized = optimizer.compile(program, training_set, metric).await?;
```

**Key Concepts:**
- Iteratively proposes instruction variations
- Evaluates each variation against the metric
- Keeps best-performing instruction

**Best Practices:**
- Higher `breadth` explores more variations
- Higher `depth` refines better but takes longer
- Combine with Bootstrap for best results

---

### COPROv2 Confidence-Based Optimization

**Use Case:** Enhanced COPRO with confidence-based selection.

```rust
use dashflow::optimize::{COPROv2, COPROv2Config};

let optimizer = COPROv2::new(COPROv2Config {
    breadth: 10,
    depth: 3,
    confidence_threshold: 0.8,
    ..Default::default()
});

let optimized = optimizer.compile(program, training_set, metric).await?;
```

**Key Concepts:**
- Adds confidence scoring to instruction selection
- Filters candidates below confidence threshold
- More reliable than standard COPRO

---

### GRPO Group Optimization

**Use Case:** Optimize multiple prompts together as a group.

```rust
use dashflow::optimize::{GRPO, GRPOConfig};

let optimizer = GRPO::new(GRPOConfig {
    num_groups: 4,
    group_size: 5,
    ..Default::default()
});

let result = optimizer.compile(programs, training_set, metric).await?;
```

**Key Concepts:**
- Optimizes prompts that work together
- Parallel trace collection for efficiency
- Group-level scoring for coherence

**Best Practices:**
- Use for multi-step pipelines
- Group related prompts together
- Monitor per-group metrics

---

### SIMBA Self-Improvement

**Use Case:** Self-improving prompts through demonstration and rule learning.

```rust
use dashflow::optimize::{SIMBA, SIMBAConfig, SIMBAStrategy};

let optimizer = SIMBA::new(SIMBAConfig {
    strategy: SIMBAStrategy::AppendADemo,
    max_demos: 5,
    ..Default::default()
});

let optimized = optimizer.compile(program, training_set, metric).await?;
```

**Strategies:**
- `AppendADemo` - Add successful examples as demonstrations
- `AppendARule` - Extract and append reasoning rules

**Best Practices:**
- Start with `AppendADemo` for simpler tasks
- Use `AppendARule` for complex reasoning
- Monitor demo quality over iterations

---

### KNN FewShot Selection

**Use Case:** Select few-shot examples using similarity search.

```rust
use dashflow::optimize::{KNNFewShot, KNNConfig};

let selector = KNNFewShot::new(KNNConfig {
    k: 3,
    embeddings: embeddings_model,
    ..Default::default()
});

let examples = selector.select(query, candidate_pool).await?;
```

**Key Concepts:**
- Uses embeddings to find similar examples
- Selects `k` nearest neighbors as few-shot
- Dynamic selection per query

**Best Practices:**
- Use high-quality embeddings model
- Pre-embed candidate pool for speed
- Tune `k` based on context length limits

---

### AutoPrompt Gradient-Free Search

**Use Case:** Automatically discover optimal prompt tokens.

```rust
use dashflow::optimize::{AutoPrompt, AutoPromptConfig};

let optimizer = AutoPrompt::new(AutoPromptConfig {
    num_candidates: 100,
    num_iterations: 10,
    ..Default::default()
});

let optimized = optimizer.compile(program, training_set, metric).await?;
```

**Key Concepts:**
- Discrete token-level optimization
- Gradient-free search over vocabulary
- Finds non-obvious prompt improvements

**Best Practices:**
- Computationally expensive - use sparingly
- Best for critical high-volume prompts
- Combine with manual review of results

---

## 🔗 Graph Composition

DashFlow enables complex workflows through composable graph patterns.

### ReAct Agent Pattern

**Use Case:** Create reasoning and acting agents with the prebuilt pattern.

```rust
use dashflow::prebuilt::{create_react_agent, AgentState};
use dashflow::core::tools::Tool;

let tools: Vec<Arc<dyn Tool>> = vec![
    Arc::new(SearchTool::new()),
    Arc::new(CalculatorTool::new()),
];

let agent = create_react_agent(model, tools)?;

let state = AgentState::with_human_message("What is 25 * 4?");
let result = agent.invoke(state).await?;
```

**Key Concepts:**
- Standard ReAct loop: think → act → observe → repeat
- Automatic tool execution handling
- Standard AgentState with message history

**Best Practices:**
- Provide clear tool descriptions
- Set max iterations for safety
- Use tracing to debug agent reasoning

---

### Subgraph Composition

**Use Case:** Build complex workflows by composing smaller graphs.

```rust
use dashflow::{StateGraph, Subgraph};

// Define a reusable subgraph
let summarizer = StateGraph::new()
    .add_node("summarize", summarize_fn)
    .add_edge(START, "summarize")
    .add_edge("summarize", END)
    .compile()?;

// Compose into larger graph
let main_graph = StateGraph::new()
    .add_node("fetch", fetch_fn)
    .add_subgraph("summarize", Subgraph::new(summarizer))
    .add_edge(START, "fetch")
    .add_edge("fetch", "summarize")
    .add_edge("summarize", END)
    .compile()?;
```

**Key Concepts:**
- Subgraphs encapsulate reusable logic
- State flows through subgraph boundaries
- Enables modular, testable workflows

**Best Practices:**
- Keep subgraphs focused on single concerns
- Test subgraphs independently
- Document subgraph interfaces

---

### Conditional Edge Routing

**Use Case:** Route graph execution based on state conditions.

```rust
use dashflow::{StateGraph, START, END};

fn route_by_type(state: &MyState) -> String {
    match state.query_type.as_str() {
        "simple" => "fast_path".to_string(),
        "complex" => "full_analysis".to_string(),
        _ => "default".to_string(),
    }
}

let graph = StateGraph::new()
    .add_node("classify", classify_fn)
    .add_node("fast_path", fast_fn)
    .add_node("full_analysis", analysis_fn)
    .add_node("default", default_fn)
    .add_edge(START, "classify")
    .add_conditional_edges("classify", route_by_type, vec![
        ("fast_path", "fast_path"),
        ("full_analysis", "full_analysis"),
        ("default", "default"),
    ])
    .add_edge("fast_path", END)
    .add_edge("full_analysis", END)
    .add_edge("default", END)
    .compile()?;
```

**Key Concepts:**
- Routing functions inspect state and return target node
- Multiple targets enable flexible workflows
- Fallback routes handle unexpected cases

**Best Practices:**
- Always include a default route
- Keep routing logic simple
- Log routing decisions for debugging

---

### Parallel Branch Execution

**Use Case:** Execute multiple nodes concurrently for performance.

```rust
use dashflow::{StateGraph, START, END};

let graph = StateGraph::new()
    .add_node("fetch_a", fetch_a_fn)
    .add_node("fetch_b", fetch_b_fn)
    .add_node("fetch_c", fetch_c_fn)
    .add_node("combine", combine_fn)
    // Parallel edges from START
    .add_edge(START, "fetch_a")
    .add_edge(START, "fetch_b")
    .add_edge(START, "fetch_c")
    // All merge into combine
    .add_edge("fetch_a", "combine")
    .add_edge("fetch_b", "combine")
    .add_edge("fetch_c", "combine")
    .add_edge("combine", END)
    .compile()?;
```

**Key Concepts:**
- Multiple edges from same source run in parallel
- Join node waits for all incoming edges
- State merges using `MergeableState` trait

**Best Practices:**
- Use for independent operations
- Implement `MergeableState` for correct merging
- Monitor per-branch latency

---

### Human-in-the-Loop Approval

**Use Case:** Pause workflow for human review and approval.

```rust
use dashflow::approval::{ApprovalConfig, ApprovalState};

let config = ApprovalConfig {
    timeout: Duration::from_secs(3600),
    auto_approve_on_timeout: false,
    ..Default::default()
};

let graph = StateGraph::new()
    .add_node("generate", generate_fn)
    .add_node("await_approval", approval_node(config))
    .add_node("execute", execute_fn)
    .add_edge(START, "generate")
    .add_edge("generate", "await_approval")
    .add_conditional_edges("await_approval", check_approval, vec![
        ("approved", "execute"),
        ("rejected", END),
    ])
    .add_edge("execute", END)
    .compile()?;
```

**Key Concepts:**
- Approval nodes pause graph execution
- State persisted while awaiting approval
- Supports timeout-based auto-actions

**Best Practices:**
- Set reasonable timeout values
- Log approval decisions for audit
- Implement rejection handlers

---

## 📦 State Management

### MergeableState Pattern

**Use Case:** Define how parallel branch results combine.

```rust
use dashflow::state::MergeableState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    query: String,
    sources: Vec<Source>,
    findings: Vec<Finding>,
}

impl MergeableState for ResearchState {
    fn merge(&mut self, other: &Self) {
        // Append sources from parallel branches
        self.sources.extend(other.sources.clone());
        // Combine findings, removing duplicates
        for finding in &other.findings {
            if !self.findings.contains(finding) {
                self.findings.push(finding.clone());
            }
        }
    }
}
```

**Key Concepts:**
- `MergeableState` defines parallel merge behavior
- Called when parallel branches rejoin
- Must handle duplicate data correctly

**Best Practices:**
- Implement idempotent merge logic
- Handle empty/partial state gracefully
- Test merge with various branch orderings

---

### State Reducers

**Use Case:** Control how node outputs update state fields.

```rust
use dashflow::reducer::{Reducer, ReducerType};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChatState {
    #[reducer(ReducerType::Append)]
    messages: Vec<Message>,

    #[reducer(ReducerType::Replace)]
    current_response: String,

    #[reducer(ReducerType::Sum)]
    total_tokens: u32,
}
```

**Reducer Types:**
- `Replace` - Overwrites value (default)
- `Append` - Extends collections
- `Sum` - Adds numeric values
- `Max/Min` - Keeps extreme values
- `Custom` - User-defined function

**Best Practices:**
- Choose reducers based on data semantics
- Use `Append` for message histories
- Use `Sum` for cumulative metrics

---

## 💾 Database Checkpointing

### SQLite Checkpointing

**Use Case:** Lightweight persistent checkpointing with SQLite.

```rust
use dashflow::checkpoint::SqliteCheckpointer;

let checkpointer = SqliteCheckpointer::new("checkpoints.db").await?;

let graph = StateGraph::new()
    .add_node("process", process_fn)
    .checkpointer(checkpointer)
    .compile()?;

// Run with checkpointing
let thread_id = "user-123";
let result = graph.invoke_with_thread(state, thread_id).await?;

// Resume from checkpoint
let resumed = graph.get_state(thread_id).await?;
```

**Key Concepts:**
- SQLite provides ACID guarantees
- Single-file database for simplicity
- Thread-safe with connection pooling

**Best Practices:**
- Use WAL mode for concurrent reads
- Set appropriate cache size
- Periodically vacuum old checkpoints

---

### Multi-Tier Checkpointing

**Use Case:** Hierarchical storage for cost-effective persistence.

```rust
use dashflow::checkpoint::{MultiTierCheckpointer, MemoryCheckpointer};
// External checkpointer crates
use dashflow_redis_checkpointer::RedisCheckpointer;
use dashflow_s3_checkpointer::S3Checkpointer;

let multi_tier = MultiTierCheckpointer::new()
    .tier(MemoryCheckpointer::new(), Duration::from_secs(60))
    .tier(RedisCheckpointer::new(redis_url)?, Duration::from_hours(1))
    .tier(S3Checkpointer::new(bucket, region)?, Duration::MAX)
    .build()?;
```

**Key Concepts:**
- Hot tier (memory) for active sessions
- Warm tier (Redis) for recent checkpoints
- Cold tier (S3) for long-term storage
- Automatic promotion/demotion

**Best Practices:**
- Tune tier durations for your access patterns
- Monitor tier hit rates
- Use compression for cold storage

---

### Checkpoint Migration

**Use Case:** Handle state schema changes across versions.

```rust
use dashflow::checkpoint::{
    VersionedCheckpointer, Migration, MigrationChain
};

let migrations = MigrationChain::new()
    .add(Migration::new(1, 2, |state| {
        // Add new field with default
        state["new_field"] = json!("default");
        Ok(state)
    }))
    .add(Migration::new(2, 3, |state| {
        // Rename field
        state["renamed"] = state["old_name"].clone();
        state.remove("old_name");
        Ok(state)
    }));

let checkpointer = VersionedCheckpointer::new(base_checkpointer)
    .migrations(migrations)
    .current_version(3)
    .build()?;
```

**Key Concepts:**
- Version tracking enables safe upgrades
- Migration chain applies incrementally
- Backward compatibility preserved

**Best Practices:**
- Always bump version on schema change
- Test migrations with real checkpoint data
- Keep migrations simple and reversible

---

## 📊 Evaluation Metrics

### Exact Match Evaluation

**Use Case:** Evaluate strict correctness of outputs.

```rust
use dashflow::optimize::metrics::exact_match;

let score = exact_match(
    &predicted_answer,
    &expected_answer,
);

println!("Exact match: {:.1}%", score * 100.0);
```

**Key Concepts:**
- Binary metric: 1.0 if equal, 0.0 otherwise
- Case-sensitive by default
- Good for factual answers

**Best Practices:**
- Normalize whitespace before comparison
- Consider case-insensitive for text
- Use for multiple-choice or categorical outputs

---

### F1 Score Evaluation

**Use Case:** Measure precision and recall for text overlap.

```rust
use dashflow::optimize::metrics::{f1_score, precision_score, recall_score};

let f1 = f1_score(&predicted, &expected);
let prec = precision_score(&predicted, &expected);
let rec = recall_score(&predicted, &expected);

println!("F1: {:.2}, Precision: {:.2}, Recall: {:.2}", f1, prec, rec);
```

**Key Concepts:**
- Token-level overlap measurement
- Balances precision and recall
- Better than exact match for longer outputs

**Best Practices:**
- Use consistent tokenization
- Report all three metrics
- Set thresholds based on task requirements

---

### SemanticF1 LLM-as-Judge

**Use Case:** Semantic evaluation using LLM comparison.

```rust
use dashflow::optimize::metrics::{SemanticF1, SemanticF1Config};

let metric = SemanticF1::new(SemanticF1Config {
    judge_model: judge_llm,
    threshold: 0.7,
    ..Default::default()
});

let score = metric.evaluate(&predicted, &expected).await?;
println!("Semantic similarity: {:.2}", score);
```

**Key Concepts:**
- LLM judges semantic equivalence
- Handles paraphrasing and synonyms
- More accurate for open-ended tasks

**Best Practices:**
- Use capable judge model (GPT-4, Claude)
- Cache judgments for cost efficiency
- Calibrate threshold on validation set

---

### Custom Metric Functions

**Use Case:** Define task-specific evaluation criteria.

```rust
use dashflow::optimize::metrics::MetricFn;
use std::sync::Arc;

// Create a custom metric function
fn code_quality_metric(predicted: &str, expected: &str) -> Result<f64> {
    // Check if code compiles (simplified example)
    let compiles = predicted.contains("fn ") || predicted.contains("struct ");

    // Check for expected patterns
    let has_expected = predicted.contains(expected);

    // Combined score
    Ok(if compiles && has_expected { 1.0 } else if compiles { 0.5 } else { 0.0 })
}

// Wrap as MetricFn for use with optimization
let metric: MetricFn<String> = Arc::new(|pred, exp| {
    code_quality_metric(pred, exp)
});
```

**Key Concepts:**
- Use `MetricFn<S>` type alias for custom metrics
- Wrap functions in `Arc` for thread-safe sharing
- Combine multiple signals into single score

**Best Practices:**
- Document scoring criteria clearly
- Handle edge cases gracefully
- Cache expensive computations

---

## 📚 Additional Resources

### Documentation
- [AI Parts Catalog](AI_PARTS_CATALOG.md) - Complete component reference
- [Architecture Guide](ARCHITECTURE.md) - System design and patterns
- [Best Practices](BEST_PRACTICES.md) - Production recommendations
- [API Stability](API_STABILITY.md) - Versioning and compatibility

### Guides
- [Getting Started](QUICK_START_PRODUCTION.md)
- [Advanced Agent Patterns](ADVANCED_AGENT_PATTERNS.md)
- [App Architecture](APP_ARCHITECTURE_GUIDE.md)
- [Developer Experience](DEVELOPER_EXPERIENCE.md)

### Performance
- [Performance Benchmarks](../benchmarks/)
- [Memory Systems](AI_PARTS_CATALOG.md#memory-systems)
- [Caching with CachedEmbeddings](AI_PARTS_CATALOG.md#cachedembeddings-production-optimization)

### Operations
- [Distributed Tracing](DISTRIBUTED_TRACING.md)
- [DashFlow Streaming Protocol](DASHSTREAM_PROTOCOL.md)

---

## 🎓 Learning Path

**Beginner** (1-2 days):
1. Simple LLM Chain
2. Basic Agent with Tools
3. Structured Output
4. HNSW Vector Store

**Intermediate** (3-5 days):
1. DashFlow Workflows
2. Checkpointing & Resume
3. Document Search (RAG)
4. Custom Metrics

**Advanced** (1-2 weeks):
1. Multi-Agent Research Team
2. DashFlow Streaming Telemetry
3. Production Optimizations
4. Quality Evaluation

**Production** (ongoing):
1. Security hardening
2. Performance tuning
3. Cost optimization
4. Observability setup

---

## 🤝 Contributing

Found an issue or want to add a recipe? Open an issue or pull request on GitHub.

---

## 📄 License

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>) - Licensed under Apache License 2.0

---
