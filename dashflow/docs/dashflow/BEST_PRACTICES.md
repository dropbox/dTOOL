# DashFlow Rust: Best Practices

This guide covers best practices, design patterns, and common pitfalls when building DashFlow applications in Rust.

## Table of Contents

1. [State Design](#state-design)
2. [Node Implementation](#node-implementation)
3. [Graph Structure](#graph-structure)
4. [Error Handling](#error-handling)
5. [Testing](#testing)
6. [Performance](#performance)
7. [Debugging](#debugging)
8. [Production Deployment](#production-deployment)

---

## State Design

### Keep State Small

**Problem:** Large state = expensive cloning

**Bad:**
```rust
#[derive(Clone)]
struct State {
    // 10MB of data cloned on every node execution
    embeddings: Vec<Vec<f32>>,  // 10,000 x 256 floats
    messages: Vec<String>,
}
```

**Good:**
```rust
#[derive(Clone)]
struct State {
    // Cheap Arc pointer cloning (8 bytes)
    embeddings: Arc<Vec<Vec<f32>>>,
    messages: Vec<String>,
}
```

**Rule:** Use `Arc<T>` for large, immutable data.

### Use Option for Optional Fields

**Problem:** Need to represent "not yet computed" fields

**Bad:**
```rust
struct State {
    result: String,  // What if not computed yet?
}

// Workaround with empty string (error-prone)
State { result: String::new() }
```

**Good:**
```rust
struct State {
    result: Option<String>,  // Explicitly optional
}

// Clear semantics
State { result: None }

// In node
if let Some(result) = &state.result {
    // Use result
} else {
    // Compute result
}
```

### Separate Input from Computed State

**Problem:** Hard to distinguish inputs from computed fields

**Good:**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct WorkflowState {
    // Inputs (set by user)
    query: String,
    config: WorkflowConfig,

    // Computed (set by nodes)
    search_results: Option<Vec<String>>,
    summary: Option<String>,
    final_answer: Option<String>,
}
```

**Benefits:**
- Clear which fields are inputs
- Easy to validate initial state
- Easier to test (know what to provide)

### Use Enums for State Machines

**Problem:** State can be in one of several distinct modes

**Good:**
```rust
#[derive(Clone, Serialize, Deserialize)]
enum WorkflowPhase {
    Researching { query: String },
    Writing { research: Vec<String> },
    Reviewing { draft: String },
    Complete { final_doc: String },
}

#[derive(Clone, Serialize, Deserialize)]
struct State {
    phase: WorkflowPhase,
    metadata: Metadata,
}
```

**Benefits:**
- Impossible to have invalid states
- Type-safe pattern matching
- Self-documenting workflow stages

---

## Node Implementation

### Prefer Closures for Simple Nodes

**Simple logic:** Use closures

```rust
graph.add_node_from_fn("simple_node", |mut state| {
    Box::pin(async move {
        state.count += 1;
        Ok(state)
    })
});
```

**Complex logic or reusable nodes:** Use structs

```rust
struct ResearchNode {
    api_client: Arc<ApiClient>,
    max_results: usize,
}

impl Node<State> for ResearchNode {
    fn invoke(&self, mut state: State) -> BoxFuture<'_, Result<State>> {
        Box::pin(async move {
            let results = self.api_client
                .search(&state.query)
                .await?
                .take(self.max_results)
                .collect();
            state.results = results;
            Ok(state)
        })
    }
}
```

**When to use structs:**
- Node needs configuration (API keys, URLs, limits)
- Node needs state (caches, connections)
- Node is reused across graphs
- Node has complex logic (easier to test)

### Make Nodes Idempotent

**Problem:** Node runs twice due to retry/checkpoint resume

**Bad:**
```rust
|mut state| {
    Box::pin(async move {
        state.count += 1;  // ❌ Not idempotent
        Ok(state)
    })
}
```

**Good:**
```rust
|mut state| {
    Box::pin(async move {
        if state.processed {
            return Ok(state);  // Already processed
        }
        state.count += 1;
        state.processed = true;
        Ok(state)
    })
}
```

**Or use immutable pattern:**
```rust
|state| {
    Box::pin(async move {
        let new_state = State {
            count: state.count + 1,
            ..state
        };
        Ok(new_state)
    })
}
```

### Handle Errors Gracefully

**Bad:**
```rust
|state| {
    Box::pin(async move {
        let result = api_call().await.unwrap();  // ❌ Panics on error
        Ok(state)
    })
}
```

**Good:**
```rust
|mut state| {
    Box::pin(async move {
        match api_call().await {
            Ok(result) => {
                state.result = Some(result);
                Ok(state)
            }
            Err(e) => {
                state.error = Some(e.to_string());
                Ok(state)  // Continue workflow with error recorded
            }
        }
    })
}
```

**Or propagate error:**
```rust
|mut state| {
    Box::pin(async move {
        let result = api_call().await?;  // Stops workflow on error
        state.result = result;
        Ok(state)
    })
}
```

**Choose based on requirements:**
- **Record and continue:** For non-critical errors, user-facing workflows
- **Propagate and stop:** For critical errors, data pipelines

### Use Timeouts for External Calls

**Bad:**
```rust
|state| {
    Box::pin(async move {
        // Could hang forever
        let result = slow_api_call().await?;
        Ok(state)
    })
}
```

**Good:**
```rust
use tokio::time::{timeout, Duration};

|state| {
    Box::pin(async move {
        let result = timeout(
            Duration::from_secs(30),
            slow_api_call()
        ).await??;  // Note: ?? (unwrap timeout, then unwrap result)
        Ok(state)
    })
}
```

**Or use graph-level timeout:**
```rust
let app = graph.compile()?
    .with_node_timeout(Duration::from_secs(30));
```

---

## Graph Structure

### Start with Linear, Add Complexity Gradually

**Phase 1: Linear**
```rust
graph.add_edge("a", "b");
graph.add_edge("b", "c");
graph.add_edge("c", END);
```

**Phase 2: Add Branching**
```rust
graph.add_conditional_edges("b", route_fn, routes)?;
```

**Phase 3: Add Cycles**
```rust
graph.add_conditional_edges("c", |s| {
    if s.needs_retry { "a" } else { END }
}, routes)?;
```

**Phase 4: Add Parallelism**
```rust
graph.add_parallel_edges("a", &["b1", "b2", "b3"]);
```

**Don't start with complex graphs.** Build incrementally, test each phase.

### Keep Graphs Shallow

**Bad (deep nesting):**
```
a -> b -> c -> d -> e -> f -> g -> h -> i
```

**Good (parallel + aggregation):**
```
      -> b1 ->
a -> b2 -> aggregator -> next
      -> b3 ->
```

**Rule:** If graph is >7 nodes deep, look for parallelization opportunities.

### Use Conditional Edges for Routing, Not Nodes

**Bad:**
```rust
graph.add_node_from_fn("router", |state| {
    Box::pin(async move {
        if state.condition {
            // ❌ Can't actually route from inside node
        }
        Ok(state)
    })
});
```

**Good:**
```rust
graph.add_conditional_edges(
    "router",
    |state: &State| {
        if state.condition {
            "path_a"
        } else {
            "path_b"
        }
    },
    vec![
        ("path_a".to_string(), "node_a".to_string()),
        ("path_b".to_string(), "node_b".to_string()),
    ],
)?;
```

**Rule:** Nodes transform state, edges route execution.

### Name Nodes Descriptively

**Bad:**
```rust
graph.add_node_from_fn("n1", ...);
graph.add_node_from_fn("n2", ...);
graph.add_node_from_fn("n3", ...);
```

**Good:**
```rust
graph.add_node_from_fn("fetch_data", ...);
graph.add_node_from_fn("process_data", ...);
graph.add_node_from_fn("store_results", ...);
```

**Benefits:**
- Easier debugging (event logs are readable)
- Self-documenting graphs
- Easier to understand execution traces

---

## Error Handling

### Define Custom Error Types

**Bad:**
```rust
Err("Something went wrong".into())  // Loses context
```

**Good:**
```rust
#[derive(Debug, thiserror::Error)]
enum WorkflowError {
    #[error("API call failed: {0}")]
    ApiError(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}

// In node
return Err(WorkflowError::ApiError(e.to_string()).into());
```

**Benefits:**
- Structured error information
- Pattern matching on error types
- Better error messages

### Log Errors Before Propagating

**Bad:**
```rust
let result = api_call().await?;  // Error disappears into ether
```

**Good:**
```rust
let result = match api_call().await {
    Ok(r) => r,
    Err(e) => {
        eprintln!("API call failed: {}", e);
        return Err(e.into());
    }
};
```

**Or use event callbacks:**
```rust
impl EventCallback for LoggingCallback {
    fn on_node_end(&self, node_id: &str) {
        if let Some(error) = &self.last_error {
            log::error!("Node {} failed: {}", node_id, error);
        }
    }
}
```

### Handle Partial Failures in Parallel Blocks

**Problem:** One parallel branch fails, lose all results

**Bad:**
```rust
graph.add_parallel_edges("start", &["a", "b", "c"]);
// If "b" fails, entire parallel block fails
```

**Good:**
```rust
// Make each branch resilient
graph.add_node_from_fn("a", |mut state| {
    Box::pin(async move {
        match risky_operation().await {
            Ok(result) => state.result_a = Some(result),
            Err(e) => state.error_a = Some(e.to_string()),
        }
        Ok(state)  // Always succeeds, errors recorded in state
    })
});

// Aggregator checks which succeeded
graph.add_node_from_fn("aggregate", |state| {
    Box::pin(async move {
        let results = vec![
            state.result_a.as_ref(),
            state.result_b.as_ref(),
            state.result_c.as_ref(),
        ].into_iter().filter_map(|r| r).collect();

        // Continue with whatever succeeded
        Ok(state)
    })
});
```

---

## Testing

### Test Nodes in Isolation

**Bad:**
```rust
// Only test full graph
#[tokio::test]
async fn test_workflow() {
    let graph = build_entire_graph();
    let result = graph.compile()?.invoke(state).await?;
    assert_eq!(result.state().final_value, expected);
}
```

**Good:**
```rust
// Test individual nodes
#[tokio::test]
async fn test_research_node() {
    let node = ResearchNode::new(mock_client());
    let state = State { query: "test".to_string(), ..Default::default() };
    let result = node.invoke(state).await.unwrap();
    assert_eq!(result.results.len(), 3);
}

// Test small subgraphs
#[tokio::test]
async fn test_research_write_flow() {
    let mut graph = StateGraph::new();
    graph.add_node_from_fn("research", research_node);
    graph.add_node_from_fn("write", write_node);
    graph.add_edge("research", "write");
    graph.set_entry_point("research");

    let result = graph.compile()?.invoke(initial_state).await?;
    assert!(result.state().draft.is_some());
}
```

**Benefits:**
- Faster test execution
- Easier to pinpoint failures
- Easier to mock dependencies

### Use Snapshots for Complex State

```rust
use insta::assert_json_snapshot;

#[tokio::test]
async fn test_complex_workflow() {
    let result = graph.invoke(state).await?;
    assert_json_snapshot!(result.state());
}
```

**Benefits:**
- Catches unexpected state changes
- Easier than manual assertions for large state
- Visual diffs in failures

### Test Error Paths

**Don't just test happy path:**

```rust
#[tokio::test]
async fn test_api_failure_handling() {
    let node = ResearchNode::new(failing_mock_client());
    let result = node.invoke(state).await;

    match result {
        Err(e) => assert!(e.to_string().contains("API error")),
        Ok(_) => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_timeout_handling() {
    let app = graph.compile()?
        .with_node_timeout(Duration::from_millis(10));

    let result = app.invoke(slow_state).await;
    assert!(result.is_err());
}
```

### Test Conditional Routing

```rust
#[tokio::test]
async fn test_high_confidence_route() {
    let state = State { score: 0.9, .. };
    let result = graph.invoke(state).await?;
    assert!(result.execution_path.contains(&"high_confidence_handler"));
}

#[tokio::test]
async fn test_low_confidence_route() {
    let state = State { score: 0.3, .. };
    let result = graph.invoke(state).await?;
    assert!(result.execution_path.contains(&"low_confidence_handler"));
}
```

---

## Performance

### Profile Before Optimizing

**Don't guess, measure:**

```rust
use std::time::Instant;

let start = Instant::now();
let result = graph.invoke(state).await?;
println!("Execution time: {:?}", start.elapsed());
```

**Or use event callbacks:**
```rust
struct TimingCallback {
    timings: Arc<RwLock<HashMap<String, Duration>>>,
}

impl EventCallback for TimingCallback {
    fn on_node_start(&self, node_id: &str) {
        self.timings.write().unwrap().insert(
            node_id.to_string(),
            Instant::now()
        );
    }

    fn on_node_end(&self, node_id: &str) {
        let elapsed = self.timings.read().unwrap()
            .get(node_id)
            .unwrap()
            .elapsed();
        println!("{}: {:?}", node_id, elapsed);
    }
}
```

### Parallelize Independent Operations

**Bad:**
```rust
graph.add_edge("start", "fetch_weather");
graph.add_edge("fetch_weather", "fetch_news");
graph.add_edge("fetch_news", "fetch_stocks");
// Sequential: 3 API calls = 3x latency
```

**Good:**
```rust
graph.add_parallel_edges("start", &[
    "fetch_weather",
    "fetch_news",
    "fetch_stocks",
]);
// Parallel: 3 API calls = 1x latency
```

**Rule:** If operations don't depend on each other, run them in parallel.

### Use Streaming for Long Workflows

**Bad:**
```rust
// User waits for entire workflow
let result = graph.invoke(state).await?;
show_final_result(result);
```

**Good:**
```rust
// User sees progress
let mut stream = graph.stream(state).await;
while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::NodeEnd { node_id, state } => {
            show_progress(node_id, state);
        }
        _ => {}
    }
}
```

**Benefits:**
- Better UX (progressive disclosure)
- Easier debugging (see where it's slow)
- Can cancel long operations

### Cache Expensive Operations

**Bad:**
```rust
graph.add_node_from_fn("embed", |mut state| {
    Box::pin(async move {
        // Recomputes embeddings every time, even if query unchanged
        state.embeddings = compute_embeddings(&state.query).await?;
        Ok(state)
    })
});
```

**Good:**
```rust
struct EmbedNode {
    cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
}

impl Node<State> for EmbedNode {
    fn invoke(&self, mut state: State) -> BoxFuture<'_, Result<State>> {
        Box::pin(async move {
            if let Some(cached) = self.cache.read().unwrap().get(&state.query) {
                state.embeddings = cached.clone();
                return Ok(state);
            }

            let embeddings = compute_embeddings(&state.query).await?;
            self.cache.write().unwrap().insert(state.query.clone(), embeddings.clone());
            state.embeddings = embeddings;
            Ok(state)
        })
    }
}
```

---

## Debugging

### Use Event Callbacks for Tracing

```rust
use dashflow::PrintCallback;

let app = graph.compile()?
    .with_callback(Arc::new(PrintCallback));

let result = app.invoke(state).await?;

// Output:
// [NODE START] research
// [NODE END] research
// [EDGE] research -> writer (Simple)
// [NODE START] writer
// ...
```

### Enable Streaming for Visibility

```rust
use dashflow::StreamMode;
use futures::StreamExt;

let mut stream = graph.compile()?
    .with_stream_mode(StreamMode::Events)
    .stream(state)
    .await;

while let Some(event) = stream.next().await {
    println!("{:?}", event?);
}
```

### Add Logging to Nodes

```rust
graph.add_node_from_fn("process", |state| {
    Box::pin(async move {
        log::info!("Processing state: {:?}", state);
        // ... node logic ...
        log::info!("Processed result: {:?}", result);
        Ok(state)
    })
});
```

### Use Checkpointing to Inspect State

```rust
let checkpointer = FileCheckpointer::new("./debug_checkpoints")?;
let app = graph.compile()?
    .with_checkpointer(Arc::new(checkpointer))
    .with_thread_id("debug_session".to_string());

app.invoke(state).await?;

// Now inspect checkpoint files to see state at each step
```

### Test with Small Inputs First

**Don't debug with production data:**

```rust
// Bad
let state = State {
    documents: load_10000_documents(),  // Hard to debug
    ..
};

// Good
let state = State {
    documents: vec![
        "doc1".to_string(),
        "doc2".to_string(),
    ],  // Easy to trace
    ..
};
```

---

## Production Deployment

### Set Timeouts

```rust
let app = graph.compile()?
    .with_timeout(Duration::from_secs(300))      // 5 min graph timeout
    .with_node_timeout(Duration::from_secs(30)); // 30s per node
```

**Prevents:**
- Hung workflows
- Resource exhaustion
- Runaway costs (LLM APIs)

### Use File or Database Checkpointing

**Don't use MemoryCheckpointer in production:**

```rust
// ❌ Development only
let checkpointer = MemoryCheckpointer::new();

// ✅ Production
let checkpointer = FileCheckpointer::new("./checkpoints")?;
// Or future: DatabaseCheckpointer::new(db_url)?;
```

**Benefits:**
- Survives restarts
- Enables workflow resumption
- Provides audit trail

### Add Metrics and Monitoring

```rust
struct MetricsCallback {
    metrics_client: Arc<MetricsClient>,
}

impl EventCallback for MetricsCallback {
    fn on_node_end(&self, node_id: &str) {
        self.metrics_client.increment("node_executions", &[
            ("node", node_id),
        ]);
    }

    fn on_edge_traversed(&self, from: &str, to: &str, edge_type: EdgeType) {
        self.metrics_client.increment("edge_traversals", &[
            ("from", from),
            ("to", to),
            ("type", format!("{:?}", edge_type).as_str()),
        ]);
    }
}
```

### Handle Graceful Shutdown

```rust
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    let app = graph.compile()?
        .with_checkpointer(checkpointer);  // Ensure state saved

    tokio::select! {
        result = app.invoke(state) => {
            result?;
        }
        _ = signal::ctrl_c() => {
            eprintln!("Shutting down gracefully...");
            // Checkpoint saved automatically
        }
    }

    Ok(())
}
```

### Use Structured Logging

```rust
use tracing::{info, warn, error};

graph.add_node_from_fn("critical_step", |state| {
    Box::pin(async move {
        info!(node = "critical_step", state = ?state, "Starting critical operation");

        match risky_operation().await {
            Ok(result) => {
                info!(node = "critical_step", "Operation succeeded");
                Ok(state)
            }
            Err(e) => {
                error!(node = "critical_step", error = %e, "Operation failed");
                Err(e.into())
            }
        }
    })
});
```

### Version Your Graphs

```rust
#[derive(Clone, Serialize, Deserialize)]
struct State {
    #[serde(default = "default_version")]
    graph_version: String,
    // ... other fields
}

fn default_version() -> String {
    "v1.0.0".to_string()
}

// In node
if state.graph_version != "v1.0.0" {
    return Err("Incompatible graph version".into());
}
```

**Benefits:**
- Safe migrations
- Checkpoint compatibility checking
- Debugging (know which version produced results)

---

## Common Pitfalls

### Pitfall 1: Forgetting to Set Entry Point

```rust
let mut graph = StateGraph::new();
graph.add_node_from_fn("node", |s| Box::pin(async move { Ok(s) }));
graph.compile()?;  // ❌ Error: no entry point set
```

**Fix:** `graph.set_entry_point("node");`

### Pitfall 2: Cycles Without Exit Condition

```rust
graph.add_edge("a", "b");
graph.add_edge("b", "a");  // ❌ Infinite loop
```

**Fix:** Use conditional edge with END route:

```rust
graph.add_conditional_edges("b", |state| {
    if state.done { END } else { "a" }
}, routes)?;
```

### Pitfall 3: Not Handling Parallel Edge Failures

```rust
graph.add_parallel_edges("start", &["a", "b", "c"]);
// If any fails, entire block fails
```

**Fix:** Make branches resilient (return Ok, record errors in state).

### Pitfall 4: Expensive State Cloning

```rust
#[derive(Clone)]
struct State {
    large_data: Vec<u8>,  // Cloned on every operation
}
```

**Fix:** Use `Arc<T>`:

```rust
#[derive(Clone)]
struct State {
    large_data: Arc<Vec<u8>>,  // Cheap cloning
}
```

### Pitfall 5: Blocking Operations in Async Nodes

```rust
|state| {
    Box::pin(async move {
        let result = expensive_cpu_work();  // ❌ Blocks tokio thread
        Ok(state)
    })
}
```

**Fix:** Use `spawn_blocking`:

```rust
|state| {
    Box::pin(async move {
        let result = tokio::task::spawn_blocking(|| {
            expensive_cpu_work()
        }).await?;
        Ok(state)
    })
}
```

---

## Summary

**Key Best Practices:**

1. **State:** Keep small, use `Arc<T>` for large data, use `Option<T>` for optional fields
2. **Nodes:** Closures for simple, structs for complex; make idempotent; handle errors gracefully
3. **Graphs:** Start simple, add complexity gradually; keep shallow; use descriptive names
4. **Errors:** Define custom types, log before propagating, handle partial failures
5. **Testing:** Test nodes in isolation, use snapshots, test error paths and routing
6. **Performance:** Profile first, parallelize independent ops, use streaming, cache expensive ops
7. **Debugging:** Use event callbacks, enable streaming, add logging, use small test inputs
8. **Production:** Set timeouts, use persistent checkpointing, add metrics, handle graceful shutdown

**Resources:**

- Examples: `crates/dashflow/examples/`
- Architecture: `docs/dashflow/ARCHITECTURE.md`
- Golden Path: `docs/GOLDEN_PATH.md`
- API Docs: `cargo doc --open -p dashflow`
