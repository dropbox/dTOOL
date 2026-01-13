# DashFlow Troubleshooting Guide

**A comprehensive guide to diagnosing and solving common issues**

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

---

## Table of Contents

1. [Installation Issues](#installation-issues)
2. [API Key Problems](#api-key-problems)
3. [Build and Compilation Errors](#build-and-compilation-errors)
4. [Runtime Errors](#runtime-errors)
5. [Performance Issues](#performance-issues)
6. [Vector Store Problems](#vector-store-problems)
7. [DashFlow Workflow Issues](#dashflow-workflow-issues)
8. [Integration Issues](#integration-issues)
9. [Debugging Tips](#debugging-tips)
10. [Getting Help](#getting-help)

---

## Installation Issues

### Problem: Cargo build fails with dependency errors

**Symptoms:**
```
error: failed to select a version for `tokio`
```

**Solutions:**

1. **Update Cargo.lock:**
   ```bash
   rm Cargo.lock
   cargo build
   ```

2. **Clear cargo cache:**
   ```bash
   cargo clean
   cargo build
   ```

3. **Check Rust version:**
   ```bash
   rustc --version  # Should be 1.80 or later
   rustup update
   ```

### Problem: Missing system dependencies (macOS/Linux)

**Symptoms:**
```
error: linking with `cc` failed
```

**Solutions:**

**macOS:**
```bash
xcode-select --install
brew install cmake pkg-config openssl
```

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install build-essential cmake pkg-config libssl-dev
```

**Fedora/RHEL:**
```bash
sudo dnf install cmake gcc openssl-devel
```

### Problem: Slow initial compilation

**Explanation:** First build compiles all dependencies (~5-10 minutes normal).

**Solutions:**
- Use `cargo build --release` only when needed (production builds)
- Use `cargo check` for faster feedback during development
- Enable `sccache` for cached builds:
  ```bash
  cargo install sccache
  export RUSTC_WRAPPER=sccache
  ```

---

## API Key Problems

### Problem: "API key not found" error

**Symptoms:**
```
Error: environment variable OPENAI_API_KEY not found
```

**Solutions:**

1. **Set environment variable:**
   ```bash
   export OPENAI_API_KEY="sk-proj-..."
   ```

2. **Use .env file:**
   ```bash
   # Create .env in project root
   echo 'OPENAI_API_KEY="sk-proj-..."' > .env

   # Load it before running
   source .env
   # or
   export $(cat .env | xargs)
   ```

3. **Verify it's loaded:**
   ```bash
   echo $OPENAI_API_KEY  # Should print your key
   ```

### Problem: "Invalid API key" error

**Symptoms:**
```
Error: 401 Unauthorized - Incorrect API key provided
```

**Checklist:**
- ✅ Key starts with correct prefix (`sk-proj-` for OpenAI, etc.)
- ✅ No extra quotes or whitespace in `.env` file
- ✅ Key hasn't been revoked in provider dashboard
- ✅ Account has sufficient credits/quota
- ✅ Using correct environment variable name:
  - OpenAI: `OPENAI_API_KEY`
  - Anthropic: `ANTHROPIC_API_KEY`
  - Cohere: `COHERE_API_KEY`
  - Replicate: `REPLICATE_API_TOKEN`

**Test API key manually:**
```bash
# OpenAI
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer $OPENAI_API_KEY"

# Anthropic
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01"
```

### Problem: Multiple API keys needed

**Solution:**
```bash
# .env file
OPENAI_API_KEY="sk-proj-..."
ANTHROPIC_API_KEY="sk-ant-..."
COHERE_API_KEY="..."
PINECONE_API_KEY="..."
PINECONE_ENVIRONMENT="us-west1-gcp"
```

---

## Build and Compilation Errors

### Problem: Type mismatch errors after updating

**Symptoms:**
```
error[E0308]: mismatched types
expected `Vec<Message>`, found `Vec<HumanMessage>`
```

**Solution:** Check migration guide for version changes:
- [Migration Guide](MIGRATION_v1.0_to_v1.6.md) (v1.0 → v1.6)
- Run `cargo update` to sync dependencies
- Check release notes for breaking changes

### Problem: Trait not implemented errors

**Symptoms:**
```
error[E0277]: the trait `Runnable` is not implemented for `MyChain`
```

**Common Causes:**
1. Missing trait import:
   ```rust
   use dashflow::core::runnable::Runnable;
   ```

2. Missing async trait:
   ```rust
   #[async_trait]
   impl Runnable for MyChain {
       async fn invoke(&self, input: String) -> Result<String> {
           // ...
       }
   }
   ```

3. Wrong method signature (check trait definition)

### Problem: "no method named `invoke`" error

**Solution:** Import the trait:
```rust
// ❌ Without trait import
let result = chain.invoke(input).await?;  // ERROR

// ✅ With trait import
use dashflow::core::runnable::Runnable;
let result = chain.invoke(input).await?;  // Works
```

---

## Runtime Errors

### Problem: Timeout errors with LLM requests

**Symptoms:**
```
Error: operation timed out (30s timeout exceeded)
```

**Solutions:**

1. **Increase timeout:**
   ```rust
   use std::time::Duration;

   let chat = ChatOpenAI::new()
       .with_timeout(Duration::from_secs(120));  // 2 minutes
   ```

2. **Check network connectivity:**
   ```bash
   curl -I https://api.openai.com
   ```

3. **Enable retry with backoff:**
   ```rust
   let retryable_chat = chat.with_retry(
       Some(5),      // 5 retries
       Some(true),   // exponential backoff with jitter
       Some(100),    // 100ms initial delay
       Some(10000),  // 10s max delay
       Some(2.0),    // 2x multiplier
       Some(1000),   // 1s jitter
   );
   ```

### Problem: Rate limit errors (429)

**Symptoms:**
```
Error: 429 Too Many Requests - Rate limit exceeded
```

**Solutions:**

1. **Use built-in rate limiter:**
   ```rust
   use dashflow::core::rate_limiters::InMemoryRateLimiter;
   use std::time::Duration;

   // Token bucket: 0.167 tokens/sec (~10/min), check every 100ms, max burst of 5
   let limiter = InMemoryRateLimiter::new(0.167, Duration::from_millis(100), 5.0);

   for prompt in prompts {
       limiter.acquire().await?;
       let result = chat.invoke(prompt).await?;
   }
   ```

2. **Add delays between requests:**
   ```rust
   use tokio::time::{sleep, Duration};

   for prompt in prompts {
       let result = chat.invoke(prompt).await?;
       sleep(Duration::from_millis(100)).await;  // 100ms between requests
   }
   ```

3. **Check your rate limits:**
   - OpenAI: [platform.openai.com/account/limits](https://platform.openai.com/account/limits)
   - Anthropic: Dashboard → Usage → Rate Limits

### Problem: Out of memory errors

**Symptoms:**
```
thread 'main' panicked at 'out of memory'
```

**Common Causes:**

1. **Loading too many documents at once:**
   ```rust
   // ❌ Loads all documents into memory
   let docs = loader.load_all().await?;

   // ✅ Process in batches
   let mut loader_iter = loader.load_iter();
   while let Some(batch) = loader_iter.next_batch(100).await? {
       vectorstore.add_documents(&batch).await?;
   }
   ```

2. **Large embeddings in memory:**
   ```rust
   // ✅ Use batch processing
   for chunk in texts.chunks(50) {
       let embeddings = embedder.embed_documents(chunk).await?;
       vectorstore.add_embeddings(&embeddings).await?;
   }
   ```

3. **Vector store memory limits:**
   - Use persistent stores (Pinecone, Weaviate, etc.) instead of in-memory
   - Configure index size limits
   - Use pagination for queries

### Problem: Deadlocks or hanging

**Symptoms:**
- Program hangs indefinitely
- No output, no error messages

**Debugging:**

1. **Enable tokio tracing:**
   ```rust
   // Cargo.toml
   [dependencies]
   tokio = { version = "1.38", features = ["full", "tracing"] }
   tracing-subscriber = "0.3"

   // main.rs
   tracing_subscriber::fmt::init();
   ```

2. **Check for blocking operations:**
   ```rust
   // ❌ Blocking in async context
   async fn bad() {
       std::thread::sleep(Duration::from_secs(10));  // BLOCKS RUNTIME
   }

   // ✅ Use async sleep
   async fn good() {
       tokio::time::sleep(Duration::from_secs(10)).await;
   }
   ```

3. **Use timeout wrapper:**
   ```rust
   use tokio::time::timeout;

   match timeout(Duration::from_secs(30), my_async_fn()).await {
       Ok(result) => println!("Completed: {:?}", result),
       Err(_) => eprintln!("Operation timed out after 30s"),
   }
   ```

---

## Performance Issues

### Problem: Slow embedding generation

**Symptoms:**
- Takes 10+ seconds to embed a few documents
- Much slower than Python equivalent

**Checklist:**
- ✅ Using release build? (`cargo build --release`)
- ✅ Batching embed calls? (see below)
- ✅ Reusing embeddings client? (don't create new client per call)
- ✅ Network latency? (check API endpoint location)

**Solutions:**

1. **Batch operations:**
   ```rust
   // ❌ Slow - one API call per document
   for doc in documents {
       let embedding = embedder.embed_query(&doc.text).await?;
   }

   // ✅ Fast - single API call
   let texts: Vec<_> = documents.iter().map(|d| d.text.as_str()).collect();
   let embeddings = embedder.embed_documents(&texts).await?;
   ```

2. **Use release build:**
   ```bash
   # 10-100× faster than debug build
   cargo build --release
   ./target/release/my_app
   ```

3. **Parallel processing:**
   ```rust
   use futures::future::join_all;

   let tasks: Vec<_> = chunks
       .into_iter()
       .map(|chunk| embedder.embed_documents(&chunk))
       .collect();

   let results = join_all(tasks).await;
   ```

### Problem: High memory usage

**Diagnosis:**
```bash
# Monitor memory during execution
/usr/bin/time -v ./target/release/my_app

# Or use heaptrack (Linux)
heaptrack ./target/release/my_app
heaptrack_gui heaptrack.my_app.*.gz
```

**Solutions:**

1. **Use streaming where possible:**
   ```rust
   // ✅ Stream tokens instead of buffering
   let mut stream = chat.stream(messages).await?;
   while let Some(chunk) = stream.next().await {
       print!("{}", chunk?);
   }
   ```

2. **Clear caches periodically:**
   ```rust
   // If using custom caching
   cache.clear_old_entries(Duration::from_hours(1));
   ```

3. **Use efficient data structures:**
   - `Arc<str>` instead of `String` for shared strings
   - `Vec<u8>` instead of `String` for binary data
   - Consider `bytes::Bytes` for zero-copy buffers

### Problem: Slow vector search

**Solutions:**

1. **Check index configuration:**
   ```rust
   // HNSW: tune M and ef_construction
   let config = HnswConfig {
       m: 16,              // Default: 16 (higher = better recall, more memory)
       ef_construction: 200, // Default: 200 (higher = better quality, slower build)
       ..Default::default()
   };
   ```

2. **Use approximate search:**
   ```rust
   // Most vector stores default to approximate (fast)
   let results = vectorstore.similarity_search("query", 10).await?;

   // For exact search (slow), check store-specific options
   ```

3. **Optimize embedding dimensions:**
   - Use smaller models for faster search (e.g., text-embedding-3-small)
   - Consider dimensionality reduction for large datasets

---

## Vector Store Problems

### Problem: Connection refused to vector database

**Symptoms:**
```
Error: Connection refused (ECONNREFUSED) - localhost:8000
```

**Checklist:**

1. **Is the database running?**
   ```bash
   # Chroma
   docker ps | grep chroma
   # or
   docker run -p 8000:8000 chromadb/chroma

   # Weaviate
   docker ps | grep weaviate
   # or
   docker-compose up -d  # if using docker-compose.yml

   # Pinecone (cloud-hosted, check API key)
   curl https://api.pinecone.io/
   ```

2. **Check connection string:**
   ```rust
   // ❌ Wrong port
   let store = ChromaVectorStore::new("http://localhost:9000", embeddings)?;

   // ✅ Correct port (Chroma default: 8000)
   let store = ChromaVectorStore::new("http://localhost:8000", embeddings)?;
   ```

3. **Firewall/network issues:**
   ```bash
   # Test connection
   curl http://localhost:8000/api/v1/heartbeat

   # Check if port is listening
   lsof -i :8000  # macOS/Linux
   netstat -an | grep 8000  # Windows
   ```

### Problem: Documents not found after adding

**Symptoms:**
- `add_documents` succeeds
- `similarity_search` returns empty results

**Debugging:**

1. **Verify documents were added:**
   ```rust
   let count = vectorstore.count().await?;
   println!("Total documents: {}", count);
   ```

2. **Check collection/index name:**
   ```rust
   // Ensure using same collection name
   let store = ChromaVectorStore::new("http://localhost:8000", embeddings)?
       .with_collection("my_docs");  // Must match when querying
   ```

3. **Verify embeddings are compatible:**
   ```rust
   // Use same embedding model for add and search
   let embeddings = Arc::new(OpenAIEmbeddings::new()
       .with_model("text-embedding-3-small"));  // Match model
   ```

4. **Check for indexing delay:**
   ```rust
   // Some stores need time to index
   vectorstore.add_documents(&docs).await?;
   tokio::time::sleep(Duration::from_secs(1)).await;  // Wait for indexing
   let results = vectorstore.similarity_search("query", 5).await?;
   ```

### Problem: Poor search quality

**Symptoms:**
- Irrelevant results returned
- Expected documents not in top results

**Solutions:**

1. **Improve document chunking:**
   ```rust
   use dashflow_text_splitters::RecursiveCharacterTextSplitter;

   let splitter = RecursiveCharacterTextSplitter::new()
       .with_chunk_size(500)      // Smaller chunks = better precision
       .with_chunk_overlap(50);   // Overlap to preserve context
   ```

2. **Use better embedding model:**
   ```rust
   // ✅ OpenAI text-embedding-3-large (best quality)
   let embeddings = OpenAIEmbeddings::new()
       .with_model("text-embedding-3-large");

   // Or try different provider
   let embeddings = CohereEmbeddings::new()
       .with_model("embed-english-v3.0");
   ```

3. **Tune search parameters:**
   ```rust
   // Increase k to get more candidates
   let results = vectorstore.similarity_search("query", 20).await?;

   // Then re-rank with more sophisticated logic
   let reranked = rerank_by_metadata(results)?;
   ```

4. **Use hybrid search:**
   ```rust
   // Combine vector search with keyword search
   let vector_results = vectorstore.similarity_search(query, 20).await?;
   let keyword_results = bm25_search(documents, query)?;
   let combined = merge_results(vector_results, keyword_results)?;
   ```

---

## DashFlow Workflow Issues

### Problem: Workflow hangs or doesn't complete

**Symptoms:**
- Graph execution starts but never finishes
- No errors, just hangs

**Debugging:**

1. **Enable tracing:**
   ```rust
   use dashflow::core::callbacks::ConsoleCallbackHandler;

   let graph = graph_builder.build()
       .with_callbacks(vec![Box::new(ConsoleCallbackHandler::new(true))]);

   graph.invoke(input).await?;  // Will print each node execution
   ```

2. **Check for missing edges:**
   ```rust
   // Ensure all nodes are reachable
   graph_builder
       .add_node("start", start_node)
       .add_node("process", process_node)
       .add_node("end", end_node)
       .add_edge("start", "process")
       .add_edge("process", "end");  // ← Missing this causes hang
   ```

3. **Verify conditional edges:**
   ```rust
   // Ensure all conditions are handled
   graph_builder.add_conditional_edges(
       "router",
       |state| {
           if state.needs_tool {
               "use_tool"
           } else if state.needs_human {
               "human_input"
           } else {
               "end"  // ← Always have default case
           }
       },
       vec!["use_tool", "human_input", "end"],
   );
   ```

### Problem: Checkpoint not saved/restored

**Symptoms:**
```
Error: Checkpoint not found for thread_id
```

**Checklist:**

1. **Verify checkpointer is configured:**
   ```rust
   use dashflow::checkpoint::MemoryCheckpointer;

   let checkpointer = Arc::new(MemoryCheckpointer::new());
   let graph = graph_builder
       .build()
       .with_checkpointer(checkpointer);
   ```

2. **Check thread_id is consistent:**
   ```rust
   let thread_id = "session-123";

   // Save
   graph.invoke(input).await?;
   graph.checkpoint(thread_id).await?;

   // Restore (use SAME thread_id)
   let state = graph.get_state(thread_id).await?;
   ```

3. **Verify checkpoint storage:**
   ```rust
   // For persistent checkpointing (requires dashflow-s3-checkpointer crate)
   use dashflow_s3_checkpointer::S3Checkpointer;

   let checkpointer = S3Checkpointer::new(
       "my-bucket",
       "checkpoints/",
       aws_config,
   ).await?;
   ```

### Problem: State not updating correctly

**Debugging:**

1. **Check reducer configuration:**
   ```rust
   use dashflow::reducer::AddMessagesReducer;

   #[derive(GraphState)]
   struct MyState {
       #[reducer(AddMessagesReducer)]  // ← Ensure reducer is set
       messages: Vec<Message>,

       counter: i32,  // Default reducer (replace)
   }
   ```

2. **Verify state updates in nodes:**
   ```rust
   async fn my_node(state: MyState) -> Result<MyState> {
       let mut new_state = state.clone();
       new_state.counter += 1;
       Ok(new_state)  // ← Must return updated state
   }
   ```

3. **Enable state debugging:**
   ```rust
   let mut stream = graph.stream(input).await?;
   while let Some((node, state)) = stream.next().await {
       println!("After {}: {:?}", node, state);  // Print state after each node
   }
   ```

---

## Integration Issues

### Problem: Kafka connection errors (DashFlow Streaming)

**Symptoms:**
```
Error: Failed to connect to Kafka broker at localhost:9092
```

**Solutions:**

1. **Start Kafka:**
   ```bash
   # Using Docker
   docker run -d -p 9092:9092 apache/kafka:latest

   # Or using docker-compose
   docker-compose up -d kafka
   ```

2. **Verify broker address:**
   ```rust
   // ❌ Wrong address
   let producer = DashStreamProducer::new("kafka:9092", "topic").await?;

   // ✅ Correct for localhost
   let producer = DashStreamProducer::new("localhost:9092", "topic").await?;

   // ✅ For Docker Compose service name
   let producer = DashStreamProducer::new("kafka:9092", "topic").await?;
   ```

3. **Check Kafka health:**
   ```bash
   # List topics
   kafka-topics --bootstrap-server localhost:9092 --list
   ```

### Problem: Redis checkpointer connection issues

**Symptoms:**
```
Error: Redis connection failed: Connection refused
```

**Solutions:**

1. **Start Redis:**
   ```bash
   docker run -d -p 6379:6379 redis:alpine
   # or
   redis-server
   ```

2. **Check connection:**
   ```bash
   redis-cli ping  # Should return "PONG"
   ```

3. **Verify connection string:**
   ```rust
   // Requires dashflow-redis-checkpointer crate
   use dashflow_redis_checkpointer::RedisCheckpointer;

   // ✅ Default localhost
   let checkpointer = RedisCheckpointer::new("redis://localhost:6379").await?;

   // ✅ With auth
   let checkpointer = RedisCheckpointer::new("redis://:password@localhost:6379").await?;
   ```

### Problem: AWS credentials not found (S3, Bedrock)

**Symptoms:**
```
Error: Unable to load credentials from any provider
```

**Solutions:**

1. **Configure AWS credentials:**
   ```bash
   # Option 1: AWS CLI
   aws configure

   # Option 2: Environment variables
   export AWS_ACCESS_KEY_ID="..."
   export AWS_SECRET_ACCESS_KEY="..."
   export AWS_REGION="us-west-2"

   # Option 3: ~/.aws/credentials file
   mkdir -p ~/.aws
   cat > ~/.aws/credentials <<EOF
   [default]
   aws_access_key_id = ...
   aws_secret_access_key = ...
   EOF
   ```

2. **Verify credentials:**
   ```bash
   aws sts get-caller-identity
   ```

3. **Specify profile:**
   ```rust
   use aws_config::profile::ProfileFileCredentialsProvider;

   let config = aws_config::from_env()
       .credentials_provider(ProfileFileCredentialsProvider::builder()
           .profile_name("my-profile")
           .build())
       .load()
       .await;
   ```

---

## Debugging Tips

### Enable Detailed Logging

```rust
// Cargo.toml
[dependencies]
env_logger = "0.11"

// main.rs
fn main() {
    env_logger::init();  // Reads RUST_LOG env var
    // ...
}
```

```bash
# Run with logging
RUST_LOG=debug cargo run

# Filter by crate
RUST_LOG=dashflow_core=debug cargo run

# Multiple levels
RUST_LOG=dashflow_core=debug,dashflow_openai=trace cargo run
```

### Use Rust Backtrace

```bash
# Full backtrace on panic
RUST_BACKTRACE=1 cargo run

# Even more detail
RUST_BACKTRACE=full cargo run
```

### Inspect HTTP Requests

```rust
// Add reqwest tracing
let client = reqwest::Client::builder()
    .connection_verbose(true)  // Log connection details
    .build()?;
```

Or use external tools:
```bash
# macOS/Linux
export https_proxy=http://localhost:8888  # Proxy through Charles/Fiddler

# Or use mitmproxy
mitmproxy -p 8888
export https_proxy=http://localhost:8888
cargo run
```

### Profile Performance

```bash
# CPU profiling (macOS)
cargo install cargo-instruments
cargo instruments --release --template time

# Linux
cargo install cargo-flamegraph
cargo flamegraph --bin my_app

# Memory profiling
cargo install cargo-valgrind
cargo valgrind --bin my_app
```

### Check Resource Limits

```bash
# File descriptor limits (fix "Too many open files")
ulimit -n        # Current limit
ulimit -n 10000  # Increase to 10k

# For macOS (persistent)
sudo launchctl limit maxfiles 10000 unlimited
```

---

## Getting Help

### Before Asking for Help

**Checklist:**
- ✅ Read this troubleshooting guide
- ✅ Check relevant docs:
  - [Cookbook](COOKBOOK.md) - Practical examples
  - [Best Practices](BEST_PRACTICES.md) - Recommended patterns
  - [API Docs](https://docs.rs/dashflow) - API reference
- ✅ Search existing issues: [GitHub Issues](https://github.com/dropbox/dTOOL/dashflow/issues)
- ✅ Enable debug logging (`RUST_LOG=debug`)
- ✅ Try minimal reproduction (remove unrelated code)

### How to Report Issues

**Good Bug Report Template:**

```markdown
**Environment:**
- OS: macOS 14.0 / Ubuntu 22.04 / Windows 11
- Rust version: `rustc --version`
- DashFlow version: 1.11.3
- LLM provider: OpenAI GPT-4o

**Description:**
Clear description of the problem.

**Reproduction:**
Minimal code to reproduce:
\`\`\`rust
use dashflow_openai::OpenAI;

#[tokio::main]
async fn main() {
    let chat = OpenAI::new();
    let result = chat.invoke("test").await;
    println!("{:?}", result);  // ERROR HERE
}
\`\`\`

**Expected behavior:**
Should return chat response.

**Actual behavior:**
```
Error: API timeout after 30s
```

**Logs:**
```
RUST_LOG=debug output here
```
```

### Community Resources

- **GitHub Discussions:** [github.com/dropbox/dTOOL/dashflow/discussions](https://github.com/dropbox/dTOOL/dashflow/discussions)
- **Discord:** [DashFlow Discord](https://discord.gg/dashflow) (check #rust channel)
- **Stack Overflow:** Tag questions with `dashflow` + `rust`
- **Documentation:** [docs.rs/dashflow](https://docs.rs/dashflow)

### Common "Not a Bug" Issues

These are expected behavior:

1. **Slow debug builds** - Use `--release` for performance testing
2. **API rate limits** - These are provider limits, not library bugs
3. **Non-deterministic LLM outputs** - LLMs are probabilistic (set `temperature=0` for more determinism)
4. **Timeout errors with large prompts** - Increase timeout or reduce prompt size
5. **First run slow** - Compilation is normal, subsequent runs are fast

---

## Quick Reference: Common Commands

```bash
# Build and run
cargo build --release
cargo run --release

# Testing
cargo test
cargo test --package dashflow

# Check without building
cargo check

# Fix formatting and lints
cargo fmt
cargo clippy --fix

# Update dependencies
cargo update

# Clean build artifacts
cargo clean

# Install tools
cargo install cargo-audit
cargo install cargo-outdated

# Security audit
cargo audit

# Check for outdated deps
cargo outdated
```

---

## Appendix: Error Code Reference

| Error Code | Meaning | Common Fix |
|------------|---------|------------|
| `E0308` | Type mismatch | Check types, add type conversions |
| `E0277` | Trait not implemented | Import trait or implement it |
| `E0597` | Borrowed value does not live long enough | Use `.to_owned()` or adjust lifetimes |
| `E0599` | Method not found | Import trait, check method name |
| `E0425` | Cannot find value | Import or define the value |
| `E0433` | Failed to resolve import | Add dependency to Cargo.toml |

**Async-specific:**
- `thread 'main' panicked at 'Cannot block in async context'` - Use async sleep, not thread::sleep
- `error: future cannot be sent between threads safely` - Wrap non-Send types in Arc<Mutex<T>>

---

**Last Updated:** 2026-01-04
**Version:** 1.11.3

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
