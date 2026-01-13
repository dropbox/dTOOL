# DashFlow Quickstart

Get started with DashFlow in 5 minutes. This guide covers installation, basic usage, and your first multi-agent workflow.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation](#installation)
3. [Your First LLM Call](#your-first-llm-call)
4. [Building a Multi-Agent Workflow](#building-a-multi-agent-workflow)
5. [Adding Persistence](#adding-persistence)
6. [Streaming Results](#streaming-results)
7. [Next Steps](#next-steps)

---

## Prerequisites

- **Rust 1.80+** - Install from [rustup.rs](https://rustup.rs/)
- **OpenAI API Key** - Get one from [platform.openai.com](https://platform.openai.com/)

```bash
# Check Rust version
rustc --version  # Should be 1.80+

# Set your API key (add to ~/.bashrc or ~/.zshrc for persistence)
export OPENAI_API_KEY="sk-..."
```

---

## Installation

### Option 1: Clone the Repository (Recommended)

```bash
# Clone the repository
git clone https://github.com/dropbox/dTOOL/dashflow
cd dashflow

# Build all crates (takes ~2 minutes first time)
cargo build --release

# Run tests to verify installation
cargo test --workspace --lib
```

### Option 2: Use as Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
dashflow = { path = "path/to/dashflow/crates/dashflow" }
dashflow-openai = { path = "path/to/dashflow/crates/dashflow-openai" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"
```

**Note:** This project is not published to crates.io (internal Dropbox use only).

---

## Your First LLM Call

Let's start with a simple chat completion:

```rust
use dashflow::prelude::*;  // Imports ChatModel, Message, etc.
use dashflow_openai::{ChatOpenAI, OpenAIConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an OpenAI chat model (reads OPENAI_API_KEY from env)
    let config = OpenAIConfig::default();  // Reads OPENAI_API_KEY from environment
    let chat = ChatOpenAI::with_config(config)
        .with_model("gpt-4o-mini")  // Fast and cost-effective
        .with_temperature(0.7);

    // Prepare messages
    let messages = vec![
        Message::system("You are a helpful AI assistant."),
        Message::human("What is the capital of France?"),
    ];

    // Generate response
    let result = chat
        .generate(&messages, None, None, None, None)
        .await?;

    println!("AI: {}", result.generations[0].message.as_text());

    Ok(())
}
```

**Try it:**

Save the above code to `main.rs` and run with `cargo run`, or explore similar patterns in:

```bash
# Run the basic graph example to see DashFlow in action
cargo run -p dashflow --example basic_graph
```

**Key Concepts:**
- `OpenAIConfig::default()` - Reads API key from `OPENAI_API_KEY` env var
- `ChatOpenAI::with_config(config)` - Creates an OpenAI client with configuration
- `Message::system()` - Sets the AI's behavior
- `Message::human()` - User's input
- `.generate()` - Makes the API call and returns the response

---

## Building a Multi-Agent Workflow

Now let's build a real-world application: a **Research Assistant** with three agents working together.

```rust
use dashflow::{StateGraph, MergeableState, Result, END};
use dashflow::prelude::*;  // Imports ChatModel, Message, etc.
use dashflow_openai::{ChatOpenAI, OpenAIConfig};

// Define the state that flows through the workflow
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    topic: String,
    outline: String,
    research_notes: Vec<String>,
    final_report: String,
}

impl MergeableState for ResearchState {
    fn merge(&mut self, other: &Self) {
        // Keep topic unchanged
        if !other.outline.is_empty() {
            self.outline = other.outline.clone();
        }
        self.research_notes.extend(other.research_notes.clone());
        if !other.final_report.is_empty() {
            self.final_report = other.final_report.clone();
        }
    }
}

// Agent 1: Creates research outline
async fn planner_node(mut state: ResearchState) -> Result<ResearchState> {
    let config = OpenAIConfig::default();
    let llm = ChatOpenAI::with_config(config).with_model("gpt-4o-mini");
    let prompt = format!("Create a 3-point outline for: {}", state.topic);
    let messages = vec![
        Message::system("You create research outlines."),
        Message::human(prompt),
    ];

    let result = llm.generate(&messages, None, None, None, None).await?;
    state.outline = result.generations[0].message.as_text().to_string();

    println!("📝 Planner: Created outline");
    Ok(state)
}

// Agent 2: Gathers research for each point
async fn researcher_node(mut state: ResearchState) -> Result<ResearchState> {
    let config = OpenAIConfig::default();
    let llm = ChatOpenAI::with_config(config).with_model("gpt-4o-mini");
    let prompt = format!(
        "Research these points:\n{}\n\nProvide 3 key insights.",
        state.outline
    );
    let messages = vec![
        Message::system("You gather key facts and insights."),
        Message::human(prompt),
    ];

    let result = llm.generate(&messages, None, None, None, None).await?;
    state.research_notes.push(result.generations[0].message.as_text().to_string());

    println!("🔍 Researcher: Gathered insights");
    Ok(state)
}

// Agent 3: Writes final report
async fn writer_node(mut state: ResearchState) -> Result<ResearchState> {
    let config = OpenAIConfig::default();
    let llm = ChatOpenAI::with_config(config).with_model("gpt-4o-mini");
    let prompt = format!(
        "Write a report on: {}\n\nOutline:\n{}\n\nResearch:\n{}",
        state.topic,
        state.outline,
        state.research_notes.join("\n\n")
    );
    let messages = vec![
        Message::system("You write concise, well-structured reports."),
        Message::human(prompt),
    ];

    let result = llm.generate(&messages, None, None, None, None).await?;
    state.final_report = result.generations[0].message.as_text().to_string();

    println!("✍️  Writer: Completed report");
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Research Workflow\n");

    // Build the workflow graph
    let mut graph = StateGraph::new();

    // Add three agents as nodes
    graph.add_node_from_fn("planner", |state| Box::pin(planner_node(state)));
    graph.add_node_from_fn("researcher", |state| Box::pin(researcher_node(state)));
    graph.add_node_from_fn("writer", |state| Box::pin(writer_node(state)));

    // Define the workflow: planner → researcher → writer → end
    graph.set_entry_point("planner");
    graph.add_edge("planner", "researcher");
    graph.add_edge("researcher", "writer");
    graph.add_edge("writer", END);

    // Compile the graph into an executable app
    let app = graph.compile()?;

    // Run the workflow
    let initial_state = ResearchState {
        topic: "Artificial Intelligence in Healthcare".to_string(),
        outline: String::new(),
        research_notes: vec![],
        final_report: String::new(),
    };

    let result = app.invoke(initial_state).await?;

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📄 FINAL REPORT\n");
    println!("{}", result.final_state.final_report);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}
```

**Run it:**

```bash
cargo run -p dashflow --example quickstart_research_workflow
```

**Output:**
```
🚀 Starting Research Workflow

📝 Planner: Created outline
🔍 Researcher: Gathered insights
✍️  Writer: Completed report

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📄 FINAL REPORT

[Full research report appears here...]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

**Key Concepts:**
- **StateGraph** - Defines your workflow as a directed graph
- **State** - Data structure that flows between nodes (must implement `Clone`, `Serialize`, `Deserialize`)
- **Nodes** - Async functions that transform state (`async fn(State) -> Result<State>`)
- **Edges** - Define execution order (`graph.add_edge("from", "to")`)
- **Compile & Invoke** - Turn graph into executable app, then run it

**Why This Matters:**
- **Sequential execution** - Each agent builds on previous work
- **Type safety** - Compiler catches errors before runtime
- **584x faster** - Than Python DashFlow (verified benchmarks)

---

## Adding Persistence

Want to resume workflows after interruption? Add checkpointing:

```rust
use dashflow::checkpoint::MemoryCheckpointer;

#[tokio::main]
async fn main() -> Result<()> {
    let mut graph = StateGraph::new();

    // ... add nodes and edges as before ...

    // Add in-memory checkpointing
    let checkpointer = MemoryCheckpointer::new();
    let app = graph.compile_with_checkpointer(checkpointer)?;

    // Run with thread ID for persistence
    let config = serde_json::json!({
        "configurable": {
            "thread_id": "user-123-research"
        }
    });

    let result = app.invoke_with_config(initial_state, config).await?;

    // If interrupted, resume from last checkpoint
    // (Works across program restarts with Postgres/Redis checkpointers)

    Ok(())
}
```

**Production Checkpointers:**
- **MemoryCheckpointer** - Development/testing (in-memory only)
- **PostgresCheckpointer** - Production (ACID, durable)
- **RedisCheckpointer** - Low-latency (sub-millisecond)
- **S3Checkpointer** - Cloud-native (unlimited scale)

**Benefits:**
- Resume after crashes
- Debug intermediate states
- Time-travel through execution
- 178-526x faster than Python (verified)

---

## Streaming Results

Stream agent progress in real-time:

```rust
use dashflow::StreamMode;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    let app = graph.compile()?;

    // Stream state updates as they happen
    let mut stream = app.stream(initial_state, Some(StreamMode::Values)).await?;

    while let Some(update) = stream.next().await {
        match update {
            Ok(state) => {
                println!("Update: {:?}", state);
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}
```

**Stream Modes:**
- `StreamMode::Values` - Full state after each node
- `StreamMode::Updates` - Only changes (delta updates)
- `StreamMode::Debug` - Includes internal execution details

**Use Cases:**
- Show progress to users
- Update UI in real-time
- Monitor long-running workflows
- Debug complex graphs

---

## Next Steps

### 📚 Learn More

**Core Concepts:**
- [Architecture Guide](docs/ARCHITECTURE.md) - System design patterns
- [Internal Architecture Guide](docs/INTERNAL_ARCHITECTURE.md) - Contributor guide to executor, telemetry, and persistence internals
- [Golden Path Guide](docs/GOLDEN_PATH.md) - Recommended APIs and patterns

**Advanced Topics:**
- [Conditional Routing](crates/dashflow/examples/conditional_branching.rs) - Dynamic workflow paths
- [Confidence Routing](crates/dashflow/examples/confidence_routing_agent.rs) - Model selection based on confidence
- [Multi-Agent Research](crates/dashflow/examples/multi_agent_research.rs) - Parallel agent execution
- [Committee Judge](crates/dashflow/examples/committee_judge.rs) - Multi-model voting patterns

**Production:**
- [Production Deployment](docs/PRODUCTION_DEPLOYMENT.md) - Docker, Kubernetes, scaling
- [Observability](docs/DISTRIBUTED_TRACING.md) - OpenTelemetry, metrics, tracing
- [Security](docs/SECURITY_AUDIT.md) - Security best practices

### 🛠️ Explore Examples

**215+ working examples** across multiple directories:

```bash
# Basic DashFlow examples
cargo run -p dashflow --example basic_graph
cargo run -p dashflow --example checkpointing_workflow
cargo run -p dashflow --example conditional_branching

# Advanced patterns
cargo run -p dashflow --example confidence_routing_agent
cargo run -p dashflow --example multi_agent_research
cargo run -p dashflow --example committee_judge

# Additional examples
cargo run --example structured_output     # Type-safe responses
cargo run --example s3_checkpointing      # S3 checkpoint backend

# Production app example
cargo run -p librarian -- query "What is async programming?"
```

### 🎯 Build Real Applications

**Sample Apps** (production-ready):

1. **Librarian** - Ultimate RAG Paragon
   - Location: `examples/apps/librarian/`
   - Pattern: Production-ready RAG agent with best practices
   - Features: StateGraph, vector search, quality evaluation, cost tracking
   - Performance: Optimized Rust implementation

**Note:** Example apps have been consolidated into `librarian`. See [docs/EXAMPLE_APPS.md](docs/EXAMPLE_APPS.md) for details.

### 📊 Performance

**Framework Performance** (verified benchmarks):
- **584x faster** than Python DashFlow on average
- **1054x faster** graph compilation
- **178-526x faster** checkpointing
- **73x less memory** (8.8 MB vs 644 MB)

**Full benchmarks:** [benchmarks/PERFORMANCE_COMPARISON_N48.md](benchmarks/PERFORMANCE_COMPARISON_N48.md)

### 🧪 Evaluation Framework

**World-class evaluation system** (exceeds OpenAI Evals, LangSmith):

```bash
# Run evaluation on your agent
cargo run --bin eval_runner -- \
  --app librarian \
  --scenarios 50 \
  --output results.json

# Generate beautiful HTML report
cargo run --bin generate_report -- \
  --input results.json \
  --output report.html
```

**Features:**
- 6D quality scoring (accuracy, relevance, completeness, safety, coherence, conciseness)
- Regression detection with statistical significance
- HTML/JSON/Markdown reports with charts
- CI/CD integration (any CI platform, git hooks)

**Guide:** [docs/EVALUATION_GUIDE.md](docs/EVALUATION_GUIDE.md)

### 🆘 Get Help

**Documentation:**
- Full API docs: `cargo doc --open`
- README: [README.md](README.md)
- This guide: [QUICKSTART.md](QUICKSTART.md)

**Community:**
- Issues: [GitHub Issues](https://github.com/dropbox/dTOOL/dashflow/issues)
- Discussions: Internal Dropbox Slack

**Debugging:**
- Enable logging: `RUST_LOG=debug cargo run`
- OpenTelemetry tracing: See [docs/DISTRIBUTED_TRACING.md](docs/DISTRIBUTED_TRACING.md)
- DashStream CLI: `cargo run --bin dashflow -- tail events.bin`

---

## Summary

You've learned:

✅ **Basic LLM calls** - Chat completions with OpenAI
✅ **Multi-agent workflows** - StateGraph with sequential execution
✅ **Persistence** - Checkpointing for resume/debug
✅ **Streaming** - Real-time progress updates

**You're ready to build production LLM applications with Rust!**

**Performance Benefits:**
- 584x faster framework operations
- 73x less memory usage
- Type-safe, compile-time guarantees
- Zero Python runtime dependency

**Next:** Explore [examples/](examples/) directory for more patterns, or dive into [docs/GOLDEN_PATH.md](docs/GOLDEN_PATH.md) for recommended APIs.

---

**Questions?** See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for system design details or [docs/GOLDEN_PATH.md](docs/GOLDEN_PATH.md) for recommended APIs.

**Happy building! 🚀**
