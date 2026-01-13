# DashFlow AI Agent Guide

**Purpose**: Single-file reference for AI agents building applications with DashFlow.
**Read this BEFORE implementing custom solutions** - DashFlow likely already has what you need.

---

## Quick Reference: Available Crates

### Core Platform
| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `dashflow` | StateGraph orchestration, core traits | `StateGraph`, `CompiledGraph`, `MergeableState` |
| `dashflow-streaming` | Real-time telemetry & debugging | `DashStreamCallback`, `StreamEvent` |

### LLM Providers
| Crate | Provider | Key Types |
|-------|----------|-----------|
| `dashflow-openai` | OpenAI GPT models | `ChatOpenAI`, `OpenAIEmbeddings` |
| `dashflow-anthropic` | Claude models | `ChatAnthropic` |
| `dashflow-bedrock` | AWS Bedrock | `BedrockChat` |
| `dashflow-ollama` | Local Ollama | `ChatOllama` |
| `dashflow-gemini` | Google Gemini | `ChatGemini` |
| `dashflow-groq` | Groq | `ChatGroq` |
| `dashflow-mistral` | Mistral AI | `ChatMistral` |
| `dashflow-fireworks` | Fireworks AI | `ChatFireworks` |
| `dashflow-together` | Together AI | `ChatTogether` |
| `dashflow-deepseek` | DeepSeek | `ChatDeepSeek` |
| `dashflow-xai` | xAI (Grok) | `ChatXAI` |
| `dashflow-perplexity` | Perplexity | `ChatPerplexity` |
| `dashflow-cohere` | Cohere | `ChatCohere` |
| `dashflow-replicate` | Replicate | `ReplicateChat` |

### Tools (Agent Actions)
| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `dashflow-shell-tool` | Execute shell commands (with security) | `ShellTool` |
| `dashflow-file-tool` | File read/write operations | `FileTool`, `ReadFileTool`, `WriteFileTool` |
| `dashflow-calculator` | Math calculations | `Calculator` |
| `dashflow-http-requests` | HTTP API calls | `HttpRequestTool` |
| `dashflow-webscrape` | Web scraping | `WebScrapeTool` |
| `dashflow-github` | GitHub API operations | `GitHubTool` |
| `dashflow-gitlab` | GitLab API operations | `GitLabTool` |
| `dashflow-jira` | Jira operations | `JiraTool` |
| `dashflow-slack` | Slack messaging | `SlackTool` |
| `dashflow-human-tool` | Human-in-the-loop | `HumanTool` |

### Checkpointers (Session Persistence)
| Crate | Backend | Use Case |
|-------|---------|----------|
| `dashflow-postgres-checkpointer` | PostgreSQL | Production, multi-node |
| `dashflow-redis-checkpointer` | Redis | Fast access, caching |
| `dashflow-s3-checkpointer` | AWS S3 | Serverless, archival |
| `dashflow-dynamodb-checkpointer` | DynamoDB | AWS native, serverless |

### Vector Stores (RAG)
| Crate | Backend |
|-------|---------|
| `dashflow-qdrant` | Qdrant |
| `dashflow-pinecone` | Pinecone |
| `dashflow-pgvector` | PostgreSQL pgvector |
| `dashflow-chroma` | Chroma |
| `dashflow-weaviate` | Weaviate |
| `dashflow-elasticsearch` | Elasticsearch |
| `dashflow-milvus` | Milvus |
| `dashflow-lancedb` | LanceDB |
| `dashflow-faiss` | FAISS |

### Search & Retrieval
| Crate | Service |
|-------|---------|
| `dashflow-tavily` | Tavily search |
| `dashflow-serper` | Google SERP |
| `dashflow-brave` | Brave Search |
| `dashflow-duckduckgo` | DuckDuckGo |
| `dashflow-exa` | Exa search |
| `dashflow-wikipedia` | Wikipedia |
| `dashflow-arxiv` | arXiv papers |
| `dashflow-pubmed` | PubMed |

---

## Adding Dependencies

All DashFlow crates are in the same repository. Add them via git:

```toml
[dependencies]
# Core (always needed)
dashflow = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }

# LLM Provider (pick one or more)
dashflow-openai = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }
dashflow-anthropic = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }

# Tools
dashflow-shell-tool = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }
dashflow-file-tool = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }

# Persistence
dashflow-postgres-checkpointer = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }

# Streaming/Observability
dashflow-streaming = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }
```

---

## Common Patterns

### 1. Basic Chat with Tool Calling

```rust
use dashflow::core::language_models::llm::LLM;
use dashflow_openai::{ChatOpenAI, OpenAIConfig};
use dashflow_shell_tool::ShellTool;

// Create LLM with tool calling (reads OPENAI_API_KEY from env)
let config = OpenAIConfig::from_env();
let llm = ChatOpenAI::with_config(config)
    .with_model("gpt-4")
    .with_tools(vec![
        Box::new(ShellTool::new()
            .with_allowed_commands(vec!["ls", "pwd", "cat", "head"]))
    ]);

// Call with messages
let response = llm.invoke(&messages).await?;
```

### 2. StateGraph with Checkpointing

```rust
use dashflow::{StateGraph, CompiledGraph};
use dashflow_postgres_checkpointer::PostgresCheckpointer;

// Build graph
let mut graph: StateGraph<MyState> = StateGraph::new();
graph.add_node_from_fn("node1", my_node_fn);
graph.set_entry_point("node1");
let compiled = graph.compile()?;

// Add checkpointing
let checkpointer = PostgresCheckpointer::new("postgresql://localhost/mydb").await?;
let config = GraphConfig::default()
    .with_checkpointer(checkpointer)
    .with_thread_id("session-123");

// Invoke with persistence
let result = compiled.invoke_with_config(initial_state, config).await?;

// Resume later
let resumed = compiled.invoke_with_config(state, config).await?;
```

### 3. Streaming with Telemetry

```rust
use dashflow::dashstream_callback::DashStreamCallback;

// Create streaming callback
let callback = DashStreamCallback::new(
    "localhost:9092",
    "agent-events",
    "my-tenant",
    "session-123"
).await?;

// Compile graph with callback and invoke
let compiled = graph.compile()?.with_callback(callback);
let result = compiled.invoke(state).await?;

// Debug with CLI:
// $ dashflow tail --thread session-123
// $ dashflow profile --thread session-123
```

### 4. Agent with Tools Pattern

```rust
use dashflow::core::tools::Tool;
use dashflow_shell_tool::ShellTool;
use dashflow_file_tool::{ReadFileTool, WriteFileTool};

// Create tool registry
let tools: Vec<Box<dyn Tool>> = vec![
    Box::new(ShellTool::new()
        .with_allowed_commands(vec!["ls", "pwd", "git"])
        .with_timeout(30)),
    Box::new(ReadFileTool::new()),
    Box::new(WriteFileTool::new()
        .with_allowed_directories(vec!["/workspace"])),
];

// Tools implement the Tool trait:
// - fn name(&self) -> &str
// - fn description(&self) -> &str
// - fn args_schema(&self) -> serde_json::Value
// - async fn call(&self, input: ToolInput) -> Result<String, Error>
```

### 5. RAG Pipeline

```rust
use dashflow::core::embeddings::Embeddings;
use dashflow_openai::OpenAIEmbeddings;
use dashflow_qdrant::Qdrant;

// Embeddings
let embeddings = OpenAIEmbeddings::new()
    .with_model("text-embedding-3-small");

// Vector store
let store = Qdrant::new("http://localhost:6333", "my-collection")
    .with_embeddings(embeddings);

// Store documents
store.add_documents(&documents).await?;

// Similarity search
let results = store.similarity_search("query", 5).await?;
```

---

## Security Best Practices

### Shell Tool Security
```rust
// NEVER use unrestricted shell in production
let unsafe_tool = ShellTool::new(); // DANGEROUS!

// ALWAYS restrict commands
let safe_tool = ShellTool::new()
    .with_allowed_commands(vec!["ls", "pwd", "cat", "head", "tail"])
    .with_allowed_prefixes(vec!["git "]) // Only git commands
    .with_working_dir("/workspace")       // Restrict directory
    .with_timeout(30)                     // Prevent hangs
    .with_max_output_bytes(1024 * 1024);  // Limit output
```

### File Tool Security
```rust
// Restrict file operations to specific directories
let file_tool = WriteFileTool::new()
    .with_allowed_directories(vec!["/workspace", "/tmp"]);
```

---

## Environment Variables

Common environment variables used by DashFlow crates:

| Variable | Used By | Purpose |
|----------|---------|---------|
| `OPENAI_API_KEY` | dashflow-openai | OpenAI authentication |
| `ANTHROPIC_API_KEY` | dashflow-anthropic | Claude authentication |
| `AWS_ACCESS_KEY_ID` | dashflow-bedrock, dashflow-s3-* | AWS auth |
| `AWS_SECRET_ACCESS_KEY` | dashflow-bedrock, dashflow-s3-* | AWS auth |
| `DATABASE_URL` | dashflow-postgres-* | PostgreSQL connection |
| `REDIS_URL` | dashflow-redis-* | Redis connection |
| `QDRANT_URL` | dashflow-qdrant | Qdrant connection |

---

## Debugging

### Enable Tracing
```rust
// Add to your main.rs
tracing_subscriber::fmt()
    .with_env_filter("dashflow=debug,dashflow_openai=debug")
    .init();
```

### Stream Inspection
```bash
# Watch events in real-time
dashflow tail --thread <session-id>

# Profile execution time
dashflow profile --thread <session-id>

# Calculate LLM costs
dashflow costs --thread <session-id>
```

---

## DO NOT Reimplement

Before writing custom code, check if DashFlow provides it:

| If you need... | Use this crate |
|----------------|----------------|
| LLM API calls | `dashflow-openai`, `dashflow-anthropic`, etc. |
| Shell execution | `dashflow-shell-tool` |
| File operations | `dashflow-file-tool` |
| HTTP requests | `dashflow-http-requests` |
| Session persistence | `dashflow-*-checkpointer` |
| Vector search | `dashflow-qdrant`, `dashflow-pgvector`, etc. |
| Web search | `dashflow-tavily`, `dashflow-serper`, etc. |
| Streaming telemetry | `dashflow-streaming` |

---

## Getting Help

1. **Check crate README**: `~/dashflow/crates/<crate-name>/README.md`
2. **Check examples**: `~/dashflow/crates/<crate-name>/examples/`
3. **Check tests**: `~/dashflow/crates/<crate-name>/src/*.rs` (look for `#[cfg(test)]`)
4. **Check lib.rs docs**: Most crates have extensive doc comments in `src/lib.rs`

---

## Extending the Platform

**You are encouraged to improve and extend DashFlow** when you discover:
- Bugs in existing crates
- Missing features you need
- Better design patterns
- New integrations

### How to Contribute Platform Improvements

1. **Create a feature branch in ~/dashflow/**:
   ```bash
   cd ~/dashflow
   git checkout -b feature/my-improvement
   ```

2. **Use [PLATFORM] prefix for commits**:
   ```bash
   git commit -m "[PLATFORM] Add retry logic to ChatOpenAI"
   ```

3. **Keep changes generic** - Platform improvements should benefit all applications, not just yours.

4. **Add tests** - Platform code must be well-tested.

5. **Update AI_AGENT_GUIDE.md** if adding new crates or capabilities.

### When to Extend vs When to Use

| Situation | Action |
|-----------|--------|
| DashFlow has what you need | Use the existing crate |
| DashFlow crate is buggy | Fix it on a feature branch |
| DashFlow crate missing feature | Add it on a feature branch |
| Need something DashFlow doesn't have | Add new crate on feature branch |
| Need app-specific customization | Keep in your application repo |

### Redundancy Detection

Run the redundancy checker before committing:
```bash
~/dashflow/scripts/check-redundancy.sh [files...]
```

Or install as pre-commit hook:
```bash
cp ~/dashflow/scripts/check-redundancy.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

This warns when you might be reimplementing existing DashFlow functionality.

---

## Upgrading DashFlow

See `~/dashflow/UPGRADE.md` for the complete upgrade runbook.

### Quick Reference

| Dependency Type | Upgrade Command |
|-----------------|-----------------|
| Local path | `cd ~/dashflow && git pull` (auto on build) |
| Git branch | `cargo update -p dashflow` |
| Pinned rev | Edit `rev = "..."` in Cargo.toml |

### Recommended for Applications

```toml
# Development (local path - immediate updates)
dashflow = { path = "../dashflow/crates/dashflow" }

# Production (git - controlled updates)
dashflow = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }

# Maximum stability (pinned commit)
dashflow = { git = "https://github.com/dropbox/dTOOL/dashflow.git", rev = "da2423f" }
```

---

## Version Information

This guide reflects DashFlow crates as of the current repository state.
Always check the actual crate source for the most up-to-date API.
