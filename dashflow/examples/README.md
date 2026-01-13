# DashFlow Examples

This directory contains working examples for DashFlow, ranging from simple provider integrations to production-grade applications demonstrating advanced DashFlow patterns.

---

## Quick Start Examples

### Basic Provider Examples

Examples demonstrating individual LLM provider integrations:

```bash
# OpenAI
cargo run -p dashflow-openai --example basic_chat
cargo run -p dashflow-openai --example streaming
cargo run -p dashflow-openai --example embeddings

# Anthropic
cargo run -p dashflow-anthropic --example basic_chat

# Ollama (local LLMs)
cargo run -p dashflow-ollama --example basic_chat

# AWS Bedrock
cargo run --example bedrock_demo
```

### Vector Store Examples

Examples demonstrating various vector database integrations:

```bash
# Weaviate
cargo run --example weaviate_basic

# Cassandra
cargo run --example cassandra_basic

# Neo4j
cargo run --example neo4j_basic

# SQLite-VSS (local)
cargo run --example sqlitevss_basic

# HNSW (in-memory)
cargo run --example hnsw_basic

# TimescaleVector
cargo run --example timescale_basic
```

### Advanced Features

```bash
# Checkpointing with S3
cargo run --example s3_checkpointing

# Structured output (JSON schemas)
cargo run --example structured_output

# Custom metrics and observability
cargo run --example custom_metrics_observability

# Traced agent execution
cargo run --example traced_agent
```

---

## Production Applications

The `apps/` directory contains production-ready applications demonstrating advanced DashFlow patterns.

### Librarian - Ultimate RAG Paragon

**Location:** `apps/librarian/`
**Pattern:** Advanced RAG with multi-turn conversations, streaming, and observability
**Status:** Production-ready

The **librarian** application is the definitive example of DashFlow Rust capabilities. It demonstrates:

**Core Features:**
- Multi-turn conversational RAG with memory
- Real-time streaming responses
- Vector search with Qdrant integration
- Quality evaluation and metrics
- Cost tracking and monitoring
- DashStream telemetry integration

**Architecture Patterns:**
- StateGraph-based agent workflow
- Conditional routing based on query analysis
- Parallel tool execution
- Checkpointing and state persistence
- Human-in-the-loop capabilities

**Run:**
```bash
cd apps/librarian

# Basic query mode
cargo run -- query "What is async programming in Rust?"

# Interactive mode
cargo run -- interactive

# With streaming enabled
cargo run --features dashstream -- query "Explain tokio"

# Run evaluations
cargo run -- eval --suite data/eval_suite.json
```

**Documentation:** [apps/librarian/README.md](apps/librarian/README.md)

---

### Common Utilities

**Location:** `apps/common/`
**Purpose:** Shared utilities for example applications

Contains reusable components for:
- Configuration loading
- Logging setup
- Common test fixtures
- Shared types

---

> **Historical Note:** Previous versions of DashFlow included additional example applications
> (document_search, advanced_rag, code_assistant, research_team, checkpoint_demo, error_recovery,
> streaming_aggregator, python_parity, mcp_self_doc). These have been consolidated into the
> `librarian` paragon application, which demonstrates all major DashFlow patterns in a single,
> well-maintained codebase. See [docs/EXAMPLE_APPS.md](../docs/EXAMPLE_APPS.md) for details.

---

## Environment Variables

Most examples require API keys:

```bash
# Required for LLM examples
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."

# Optional: for specific examples
export TAVILY_API_KEY="tvly-..."  # Web search
export AWS_ACCESS_KEY_ID="..."    # For AWS Bedrock and S3
export AWS_SECRET_ACCESS_KEY="..."

# For Ollama examples
# Install Ollama: https://ollama.ai/
# Pull a model: ollama pull llama3.2
```

Store these in a `.env` file in the repository root:

```bash
# Load environment variables
source .env
```

---

## DashStream Telemetry (Opt-In)

DashStream provides real-time execution telemetry streaming to Kafka. This feature is **opt-in** via the `dashstream` feature flag.

### Enabling DashStream

Add the `dashstream` feature to your dependency:

```toml
[dependencies]
dashflow = { version = "1.11", features = ["dashstream"] }
```

### Using DashStream in Your Application

```rust
use dashflow::dashstream_callback::DashStreamCallback;
use dashflow::StateGraph;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create DashStream callback (connects to Kafka)
    let callback = DashStreamCallback::new(
        "localhost:9092",     // Kafka broker address
        "dashstream-events",  // Topic name
        "my-tenant",          // Tenant ID for multi-tenancy
        "session-123"         // Session ID for this run
    ).await?;

    // Create and compile your graph with callback
    let mut graph = StateGraph::new();
    // ... add nodes ...
    let compiled = graph.compile()?.with_callback(callback);

    // Invoke (telemetry is streamed automatically)
    compiled.invoke(initial_state).await?;

    Ok(())
}
```

### DashStream Examples

```bash
# DashStream integration example
cargo run -p dashflow --example dashstream_integration --features dashstream

# Quality monitoring with DashStream
cargo run -p dashflow --example dashstream_quality_monitoring --features dashstream

# Streaming node demonstration
cargo run -p dashflow --example streaming_node --features dashstream
```

### What DashStream Provides

- **Event Streaming**: Real-time GraphEvent emission to Kafka
- **State Diffing**: Incremental state change tracking (JSON Patch RFC 6902)
- **Multi-tenancy**: Thread ID isolation for observability
- **Time-Travel Debugging**: UI for scrubbing through execution history (see `observability-ui/`)
- **Performance**: Async, non-blocking callbacks with backpressure handling

### When to Use DashStream

| Scenario | Recommendation |
|----------|----------------|
| Development/prototyping | Not needed - use `ConsoleCallbackHandler` |
| Production with monitoring | Enable DashStream for observability |
| Debugging complex graphs | Enable for time-travel debugging |
| Multi-agent systems | Highly recommended for tracing |

### Alternative: Local Observability

For local development without Kafka, use the built-in callbacks:

```rust
use dashflow::core::callbacks::ConsoleCallbackHandler;

// Logs events to console instead of Kafka
let callback = ConsoleCallbackHandler::new();
```

---

## Testing

Run tests for examples:

```bash
# All workspace tests
cargo test

# Librarian tests
cd apps/librarian && cargo test

# Run librarian E2E tests (requires OPENAI_API_KEY)
cargo test --package librarian -- --ignored --nocapture
```

---

## Framework Features Demonstrated

The librarian application demonstrates all core DashFlow capabilities:

| Feature | Librarian |
|---------|-----------|
| **Parallel Execution** | Parallel tool calls |
| **Conditional Routing** | Query complexity routing |
| **Subgraph Composition** | Modular agent design |
| **Checkpointing** | State persistence |
| **Human-in-Loop** | Interactive mode |
| **Error Handling** | Comprehensive |
| **Feedback Loops** | Quality evaluation |
| **Streaming** | Real-time responses |
| **State Merging** | MergeableState trait |

---

## Performance

**Measured Performance (vs Python DashFlow):**
- Document Search: **3.99x faster** (8s Rust vs 30s Python)
- Memory Usage: **50-80% less** (no interpreter, no GC)
- Latency: **Deterministic** (no GC pauses)

---

## Next Steps

1. **Start Simple:** Try basic provider examples first
2. **Explore Librarian:** Run the production app to see advanced patterns
3. **Read Documentation:** See [apps/librarian/README.md](apps/librarian/README.md)
4. **Build Your Own:** Use librarian as a template for your use cases

**Main Documentation:** [../README.md](../README.md)
**Example Apps Guide:** [../docs/EXAMPLE_APPS.md](../docs/EXAMPLE_APPS.md)

---

**Last Updated:** December 19, 2025
**Status:** Production-ready framework with librarian paragon application
