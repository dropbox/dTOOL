# DashFlow Error Catalog

**A searchable catalog of errors with root causes and resolution guides**

---

## Quick Lookup Table

Jump to an error by keyword or pattern:

| Error Pattern | Category | Quick Resolution | Section |
|---------------|----------|------------------|---------|
| `invalid api key` | Authentication | Check env var, verify key format | [AUTH-001](#auth-001-invalid-api-key) |
| `insufficient credits` | Billing | Add credits to provider account | [BILL-001](#bill-001-insufficient-credits) |
| `rate limit exceeded` | Rate Limit | Add backoff, reduce request rate | [NET-001](#net-001-rate-limit-exceeded) |
| `connection refused` | Network | Check service is running | [NET-002](#net-002-connection-refused) |
| `timeout` | Network | Increase timeout, check network | [NET-003](#net-003-operation-timeout) |
| `no entry point` | Graph | Call `set_entry_point()` | [GRAPH-001](#graph-001-no-entry-point) |
| `node not found` | Graph | Add node before referencing | [GRAPH-002](#graph-002-node-not-found) |
| `checkpoint not found` | Checkpoint | Check thread_id, add checkpointer | [CHKPT-001](#chkpt-001-checkpoint-not-found) |
| `storage full` | Checkpoint | Free disk space | [CHKPT-002](#chkpt-002-storage-full) |
| `serialization failed` | Data | Check Serialize derives | [DATA-001](#data-001-serialization-failed) |
| `context limit exceeded` | LLM | Reduce input size | [LLM-001](#llm-001-context-limit-exceeded) |
| `tool execution failed` | Agent | Check tool implementation | [AGENT-001](#agent-001-tool-execution-failed) |
| `kafka` | Streaming | Check broker, topic config | [STREAM-001](#stream-001-kafka-connection-error) |
| `redis` | Storage | Check Redis running | [STORE-001](#store-001-redis-connection-error) |
| `vector store` | Search | Check embeddings, index | [SEARCH-001](#search-001-empty-search-results) |

---

## Error Categories

### Authentication & Authorization

#### AUTH-001: Invalid API Key

**Error Patterns:**
```
Authentication error: invalid api key
401 Unauthorized - Incorrect API key provided
invalid_api_key
```

**Root Causes:**
1. API key not set in environment
2. Key has been revoked or expired
3. Key copied with extra whitespace or quotes
4. Wrong environment variable name

**Resolution:**

1. **Verify environment variable is set:**
   ```bash
   echo $OPENAI_API_KEY  # Should print key (not empty)
   ```

2. **Check key format:**
   - OpenAI: starts with `sk-proj-` or `sk-`
   - Anthropic: starts with `sk-ant-`
   - Cohere: alphanumeric string

3. **Test key manually:**
   ```bash
   # OpenAI
   curl https://api.openai.com/v1/models \
     -H "Authorization: Bearer $OPENAI_API_KEY"

   # Anthropic
   curl https://api.anthropic.com/v1/messages \
     -H "x-api-key: $ANTHROPIC_API_KEY" \
     -H "anthropic-version: 2023-06-01" \
     -d '{"model":"claude-3-5-sonnet-20241022","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}'
   ```

4. **Check for whitespace in .env:**
   ```bash
   # Bad (has quotes inside, may cause issues)
   OPENAI_API_KEY="sk-proj-..."

   # Good (no extra quotes)
   OPENAI_API_KEY=sk-proj-...
   ```

**Related Docs:** [TROUBLESHOOTING.md#api-key-problems](TROUBLESHOOTING.md#api-key-problems)

---

#### AUTH-002: Expired Token

**Error Patterns:**
```
Authentication error: token expired
401 Unauthorized - Token has expired
```

**Root Causes:**
1. OAuth token expired (Google, Azure)
2. Session token timeout
3. Cached credentials are stale

**Resolution:**

1. **Refresh OAuth tokens:**
   ```rust
   // Google: yup_oauth2 handles refresh automatically
   let auth = InstalledFlowAuthenticator::builder(...)
       .persist_tokens_to_disk(true)  // Enables token refresh
       .build().await?;
   ```

2. **Clear cached credentials:**
   ```bash
   rm -rf ~/.dashflow/tokens/
   ```

3. **Re-authenticate:**
   ```bash
   # Azure
   az login --use-device-code

   # Google
   gcloud auth application-default login
   ```

---

### Billing & Quotas

#### BILL-001: Insufficient Credits

**Error Patterns:**
```
Account/Billing error: insufficient credits
credit balance too low
quota exceeded
payment required
```

**Root Causes:**
1. Account out of credits
2. Monthly quota exceeded
3. Plan limits reached

**Resolution:**

1. **Check account balance:**
   - OpenAI: [platform.openai.com/usage](https://platform.openai.com/usage)
   - Anthropic: Console → Billing
   - Cohere: Dashboard → Usage

2. **Add credits or upgrade plan:**
   - Most providers: Settings → Billing → Add credits

3. **Implement usage tracking:**
   ```rust
   use dashflow_observability::cost::{CostTracker, BudgetConfig, BudgetEnforcer};

   // Create tracker with budget enforcement
   let config = BudgetConfig::with_daily_limit(budget_limit)
       .enforce_hard_limit(true);
   let enforcer = BudgetEnforcer::new(CostTracker::with_defaults(), config);

   // This will error if budget is exceeded
   enforcer.check_budget()?;
   ```

**Note:** This is NOT a code bug. The application is working correctly; the account needs attention.

---

### Network & Infrastructure

#### NET-001: Rate Limit Exceeded

**Error Patterns:**
```
Rate limit exceeded: too many requests
429 Too Many Requests
rate_limit_exceeded
```

**Root Causes:**
1. Too many requests per minute
2. Concurrent request limit exceeded
3. Token per minute limit exceeded

**Resolution:**

1. **Add retry with exponential backoff:**
   ```rust
   use dashflow::resilience::RetryPolicy;
   use std::time::Duration;

   let policy = RetryPolicy::exponential_jitter(
       3,                           // max retries
       Duration::from_millis(100),  // initial delay
   );

   let result = policy.execute(|| async {
       llm.invoke(prompt).await
   }).await?;
   ```

2. **Use rate limiter:**
   ```rust
   use dashflow::core::rate_limiters::InMemoryRateLimiter;

   // 10 requests per minute
   let limiter = InMemoryRateLimiter::new(
       0.167,                       // tokens per second (10/60)
       Duration::from_millis(100),  // check interval
       5.0,                         // max burst
   );

   limiter.acquire().await?;
   let result = llm.invoke(prompt).await?;
   ```

3. **Check provider limits:**
   - OpenAI: [platform.openai.com/account/limits](https://platform.openai.com/account/limits)
   - Anthropic: Console → Rate Limits

**Related Docs:** [TROUBLESHOOTING.md#problem-rate-limit-errors-429](TROUBLESHOOTING.md#problem-rate-limit-errors-429)

---

#### NET-002: Connection Refused

**Error Patterns:**
```
Connection refused (ECONNREFUSED)
connection reset by peer
failed to connect
```

**Root Causes:**
1. Service not running
2. Wrong host/port
3. Firewall blocking connection
4. DNS resolution failure

**Resolution:**

1. **Check service status:**
   ```bash
   # Docker containers
   docker ps

   # Check port is listening
   lsof -i :8080  # macOS/Linux
   netstat -an | grep 8080  # Windows
   ```

2. **Verify connection URL:**
   ```rust
   // Common mistakes:
   // ❌ "kafka:9092"    (Docker internal name, not from host)
   // ✅ "localhost:9092" (from host machine)

   // ❌ "http://localhost:8000" (wrong protocol)
   // ✅ "https://api.openai.com" (correct)
   ```

3. **Test connectivity:**
   ```bash
   curl -I http://localhost:8000/health
   ping api.openai.com
   ```

4. **Check firewall:**
   ```bash
   # macOS
   sudo pfctl -s rules

   # Linux
   sudo iptables -L
   ```

---

#### NET-003: Operation Timeout

**Error Patterns:**
```
Operation timed out: 30s timeout exceeded
request timed out
deadline exceeded
```

**Root Causes:**
1. Slow network connection
2. Server overloaded
3. Request too complex (large prompt)
4. Default timeout too short

**Resolution:**

1. **Increase timeout:**
   ```rust
   use std::time::Duration;

   let chat = ChatOpenAI::new()
       .with_timeout(Duration::from_secs(120));  // 2 minutes
   ```

2. **Use shorter prompts:**
   ```rust
   // Split large requests
   let chunks = split_text(&large_text, 4000);
   for chunk in chunks {
       let result = llm.invoke(&chunk).await?;
   }
   ```

3. **Add timeout wrapper:**
   ```rust
   use tokio::time::timeout;

   match timeout(Duration::from_secs(60), llm.invoke(prompt)).await {
       Ok(Ok(result)) => Ok(result),
       Ok(Err(e)) => Err(e),
       Err(_) => Err(Error::timeout("Request exceeded 60s")),
   }
   ```

---

### Graph Execution

#### GRAPH-001: No Entry Point

**Error Patterns:**
```
Graph has no entry point defined
NoEntryPoint
```

**Root Cause:** Graph was compiled without calling `set_entry_point()`.

**Resolution:**

```rust
let mut graph = StateGraph::<MyState>::new();
graph.add_node("start", start_node);
graph.add_node("process", process_node);
graph.add_edge("start", "process");
graph.set_entry_point("start");  // ← Required!
let compiled = graph.compile()?;
```

**Related Docs:** [ERROR_TYPES.md#graph-error](ERROR_TYPES.md#graph-error-dashflowerrorerror)

---

#### GRAPH-002: Node Not Found

**Error Patterns:**
```
Node 'process' not found
NodeNotFound: process
Referenced node doesn't exist
```

**Root Causes:**
1. Node name typo
2. Node not added before edge
3. Case sensitivity issue

**Resolution:**

1. **Check node name spelling:**
   ```rust
   graph.add_node("start", start_fn);
   graph.add_edge("start", "end");  // ❌ "end" doesn't exist

   graph.add_node("start", start_fn);
   graph.add_node("end", end_fn);    // ✅ Add node first
   graph.add_edge("start", "end");
   ```

2. **List available nodes:**
   ```rust
   for node in graph.nodes() {
       println!("Node: {}", node);
   }
   ```

---

#### GRAPH-003: Recursion Limit

**Error Patterns:**
```
RecursionLimit: exceeded limit of 100 steps
Graph execution exceeded maximum steps
```

**Root Causes:**
1. Infinite loop in graph
2. Conditional edge always returns same node
3. Recursive pattern without termination

**Resolution:**

1. **Check for cycles:**
   ```rust
   // Ensure cycles have exit conditions
   graph.add_conditional_edges(
       "router",
       |state| {
           if state.done {
               "end"  // Exit condition
           } else if state.retries > 3 {
               "end"  // Prevent infinite retries
           } else {
               "process"  // Continue
           }
       },
       vec!["process", "end"],
   );
   ```

2. **Increase limit if needed:**
   ```rust
   let compiled = graph.compile()?
       .with_recursion_limit(500);  // Default is 100
   ```

3. **Debug execution path:**
   ```rust
   let mut stream = graph.stream(state).await?;
   while let Some((node, state)) = stream.next().await {
       println!("Executed: {} (step {})", node, state.step_count);
   }
   ```

---

### Checkpointing

#### CHKPT-001: Checkpoint Not Found

**Error Patterns:**
```
Checkpoint 'session-123' not found
NotFound: checkpoint_id
NoCheckpointToResume
```

**Root Causes:**
1. Wrong thread_id
2. Checkpoint was never created
3. Checkpointer not configured
4. Storage was cleared

**Resolution:**

1. **Verify thread_id consistency:**
   ```rust
   let thread_id = "session-123";

   // Save state
   let config = RunnableConfig::new().with_thread_id(thread_id);
   graph.invoke_with_config(input, config.clone()).await?;

   // Load state (use SAME thread_id)
   let state = graph.get_state(&config).await?;
   ```

2. **Ensure checkpointer is configured:**
   ```rust
   use dashflow::checkpoint::MemoryCheckpointer;
   use std::sync::Arc;

   let checkpointer = Arc::new(MemoryCheckpointer::new());
   let graph = graph_builder
       .build()
       .with_checkpointer(checkpointer);
   ```

3. **List available checkpoints:**
   ```rust
   let checkpoints = checkpointer.list_checkpoints().await?;
   for cp in checkpoints {
       println!("Thread: {}, Created: {:?}", cp.thread_id, cp.created_at);
   }
   ```

---

#### CHKPT-002: Storage Full

**Error Patterns:**
```
StorageFull: need 1048576 bytes, only 0 available
No space left on device
```

**Root Cause:** Disk space exhausted or quota reached.

**Resolution:**

1. **Free disk space:**
   ```bash
   # Check usage
   df -h

   # Clean up
   rm -rf /tmp/dashflow_checkpoints_old/
   ```

2. **Configure max checkpoint size:**
   ```rust
   let config = CheckpointerConfig {
       max_checkpoint_size: 100 * 1024 * 1024,  // 100MB limit
       ..Default::default()
   };
   ```

3. **Prune old checkpoints:**
   ```rust
   // Keep only last 10 checkpoints per thread
   checkpointer.prune(thread_id, 10).await?;
   ```

---

#### CHKPT-003: Connection Lost

**Error Patterns:**
```
ConnectionLost: connection to postgres lost
Connection reset by peer
Backend connection terminated
```

**Root Causes:**
1. Database server restarted
2. Network interruption
3. Idle connection timeout

**Resolution:**

1. **Enable connection pooling:**
   ```rust
   let checkpointer = PostgresCheckpointer::new(
       "postgres://...",
       Some(PoolConfig {
           max_connections: 10,
           idle_timeout: Duration::from_secs(300),
       }),
   ).await?;
   ```

2. **Implement retry logic:**
   ```rust
   async fn save_with_retry(checkpointer: &impl Checkpointer, state: &State) -> Result<()> {
       for attempt in 1..=3 {
           match checkpointer.save(state).await {
               Ok(()) => return Ok(()),
               Err(CheckpointError::ConnectionLost { .. }) => {
                   tracing::warn!("Connection lost, retrying ({}/3)", attempt);
                   tokio::time::sleep(Duration::from_secs(1)).await;
               }
               Err(e) => return Err(e),
           }
       }
       Err(CheckpointError::ConnectionLost {
           backend: "postgres".into(),
           reason: "Max retries exceeded".into()
       })
   }
   ```

---

### Data & Serialization

#### DATA-001: Serialization Failed

**Error Patterns:**
```
Serialization error: ...
SerializationFailed: cannot serialize type
serde_json::Error
```

**Root Causes:**
1. Missing `Serialize` derive
2. Non-serializable field type
3. Circular reference
4. Special float values (NaN, Infinity)

**Resolution:**

1. **Add required derives:**
   ```rust
   use serde::{Serialize, Deserialize};

   #[derive(Serialize, Deserialize)]  // ← Required
   struct MyState {
       messages: Vec<String>,
       counter: i32,
   }
   ```

2. **Handle non-serializable fields:**
   ```rust
   #[derive(Serialize, Deserialize)]
   struct MyState {
       #[serde(skip)]  // Skip non-serializable fields
       connection: Arc<Connection>,

       data: Vec<u8>,
   }
   ```

3. **Fix special float values:**
   ```rust
   let value = if value.is_nan() { 0.0 } else { value };
   ```

---

### LLM Errors

#### LLM-001: Context Limit Exceeded

**Error Patterns:**
```
Context limit exceeded: 150000 tokens > 128000 limit for model gpt-4
maximum context length exceeded
```

**Root Cause:** Input + expected output exceeds model's context window.

**Resolution:**

1. **Use model with larger context:**
   ```rust
   let chat = ChatOpenAI::new()
       .with_model("gpt-4o");  // 128K context (vs 8K for gpt-4)
   ```

2. **Truncate input:**
   ```rust
   use dashflow::truncation::truncate_to_tokens;

   let truncated = truncate_to_tokens(&input, 100000)?;  // Leave room for output
   ```

3. **Split into chunks:**
   ```rust
   use dashflow_text_splitters::RecursiveCharacterTextSplitter;

   let splitter = RecursiveCharacterTextSplitter::new()
       .with_chunk_size(10000)
       .with_chunk_overlap(500);

   let chunks = splitter.split_text(&long_text)?;
   ```

**Model Context Limits:**
| Model | Context |
|-------|---------|
| gpt-4o | 128K |
| gpt-4-turbo | 128K |
| gpt-4 | 8K |
| claude-3-5-sonnet | 200K |
| claude-3-opus | 200K |

---

### Agent Errors

#### AGENT-001: Tool Execution Failed

**Error Patterns:**
```
Tool execution failed: calculator
ToolExecution: error running tool
```

**Root Causes:**
1. Tool implementation bug
2. Missing tool dependency
3. Invalid tool input
4. Tool timeout

**Resolution:**

1. **Check tool implementation:**
   ```rust
   #[derive(Tool)]
   struct Calculator;

   impl Calculator {
       async fn run(&self, input: &str) -> Result<String> {
           // Validate input first
           let expr = input.trim();
           if expr.is_empty() {
               return Err(Error::invalid_input("Empty expression"));
           }
           // ... computation
       }
   }
   ```

2. **Add proper error handling:**
   ```rust
   async fn run(&self, input: &str) -> Result<String> {
       match self.calculate(input) {
           Ok(result) => Ok(result.to_string()),
           Err(e) => Err(Error::tool_error(format!("Calculator error: {}", e))),
       }
   }
   ```

3. **Test tool in isolation:**
   ```rust
   #[tokio::test]
   async fn test_calculator() {
       let calc = Calculator;
       assert_eq!(calc.run("2 + 2").await.unwrap(), "4");
   }
   ```

---

### Streaming Errors

#### STREAM-001: Kafka Connection Error

**Error Patterns:**
```
Failed to connect to Kafka broker
Kafka broker not available
metadata error
```

**Root Causes:**
1. Kafka not running
2. Wrong broker address
3. Network/firewall issues

**Resolution:**

1. **Start Kafka:**
   ```bash
   docker run -d -p 9092:9092 apache/kafka:latest
   ```

2. **Verify broker address:**
   ```rust
   // From host machine
   let producer = DashStreamProducer::new("localhost:9092", "topic").await?;

   // From Docker container to another container
   let producer = DashStreamProducer::new("kafka:29092", "topic").await?;
   ```

3. **Check Kafka health:**
   ```bash
   kafka-topics --bootstrap-server localhost:9092 --list
   ```

**Related Docs:** [DASHSTREAM_PROTOCOL.md](DASHSTREAM_PROTOCOL.md)

---

### Vector Store Errors

#### SEARCH-001: Empty Search Results

**Error Patterns:**
```
similarity_search returned 0 results
No documents found
```

**Root Causes:**
1. Documents not indexed yet
2. Wrong collection/index name
3. Embedding model mismatch
4. Query doesn't match any content

**Resolution:**

1. **Verify documents were added:**
   ```rust
   let count = vectorstore.count().await?;
   println!("Documents indexed: {}", count);
   ```

2. **Check collection name:**
   ```rust
   // Use same name for add and search
   let store = ChromaVectorStore::new("http://localhost:8000", embeddings)?
       .with_collection("my_docs");  // Must match
   ```

3. **Use same embedding model:**
   ```rust
   // Same model for indexing and querying
   let embeddings = OpenAIEmbeddings::new()
       .with_model("text-embedding-3-small");  // Consistent!
   ```

4. **Wait for indexing:**
   ```rust
   vectorstore.add_documents(&docs).await?;
   tokio::time::sleep(Duration::from_secs(1)).await;  // Allow indexing
   ```

**Related Docs:** [TROUBLESHOOTING.md#vector-store-problems](TROUBLESHOOTING.md#vector-store-problems)

---

### Storage Errors

#### STORE-001: Redis Connection Error

**Error Patterns:**
```
Redis connection failed: Connection refused
redis::Error
```

**Resolution:**

1. **Start Redis:**
   ```bash
   docker run -d -p 6379:6379 redis:alpine
   ```

2. **Check connection:**
   ```bash
   redis-cli ping  # Should return PONG
   ```

3. **Verify URL:**
   ```rust
   // Standard
   let checkpointer = RedisCheckpointer::new("redis://localhost:6379").await?;

   // With auth
   let checkpointer = RedisCheckpointer::new("redis://:password@localhost:6379").await?;

   // With TLS
   let checkpointer = RedisCheckpointer::new("rediss://localhost:6379").await?;
   ```

---

## Error Category Decision Tree

```
Is it a code bug?
├── Yes → Report bug, check for fix in newer version
└── No → Is it environmental?
    ├── Authentication → Check API keys, tokens
    ├── Billing → Add credits, upgrade plan
    ├── Network → Retry with backoff
    └── Validation → Fix input/configuration
```

## Programmatic Error Handling

```rust
use dashflow::core::{Error, ErrorCategory};

async fn handle_error(err: Error) -> Result<(), Error> {
    match err.category() {
        ErrorCategory::Network => {
            // Transient - retry
            tracing::warn!("Network error (retrying): {}", err);
            Err(err)  // Let retry logic handle
        }
        ErrorCategory::Authentication => {
            // Config issue - don't retry
            tracing::error!("Check API keys: {}", err);
            Err(err)
        }
        ErrorCategory::AccountBilling => {
            // Account issue - alert operator
            tracing::error!("Account needs attention: {}", err);
            Err(err)
        }
        ErrorCategory::Validation => {
            // Bad input - inform user
            tracing::warn!("Invalid input: {}", err);
            Err(err)
        }
        ErrorCategory::CodeBug | ErrorCategory::ApiFormat => {
            // Bug - report and fail
            tracing::error!("BUG: {}", err);
            panic!("Code bug detected: {}", err);
        }
        ErrorCategory::Unknown => {
            tracing::warn!("Unknown error: {}", err);
            Err(err)
        }
    }
}
```

## Related Documentation

| Document | Purpose |
|----------|---------|
| [ERROR_TYPES.md](ERROR_TYPES.md) | Detailed error type reference |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Step-by-step troubleshooting |
| [OBSERVABILITY_RUNBOOK.md](OBSERVABILITY_RUNBOOK.md) | Monitoring and alerting |
| [PRODUCTION_RUNBOOK.md](PRODUCTION_RUNBOOK.md) | Production operations |
| [BENCHMARK_RUNBOOK.md](BENCHMARK_RUNBOOK.md) | Performance debugging |

---

**Last Updated:** 2026-01-04 (Worker #2450 - Metadata sync)
**Version:** 1.11
