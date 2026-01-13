# DashFlow Architecture

This document describes the internal architecture of DashFlow, a Rust framework for building stateful, multi-agent workflows with directed graphs.

## Overview

DashFlow enables complex agent workflows through a graph-based execution model. Applications define state transformations as nodes and control flow as edges, then execute graphs with features like conditional routing, parallel execution, checkpointing, and streaming.

**Key Design Principles:**
- **Type Safety**: Rust's type system ensures state transformations are correct at compile time
- **Async/Concurrent**: Built on Tokio for efficient async execution and parallel node processing
- **Composability**: Nodes, edges, and state are composable primitives
- **Persistence**: Checkpointing system enables pause/resume and fault tolerance
- **Observability**: Event callbacks provide monitoring and debugging hooks

## Core Components

### 1. State Management

**GraphState Trait** (`src/state.rs`)

```rust
pub trait GraphState:
    Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static
{}
```

State flows through the graph, being transformed by each node. Requirements:
- **Clone**: Enables parallel execution (each branch gets state copy)
- **Send + Sync**: Enables multi-threaded async execution
- **Serialize + Deserialize**: Enables checkpointing and state persistence
- **'static**: Required for async node execution

**Design Rationale:**
- Immutable-by-convention: Nodes return new/modified state rather than mutating in-place
- Cloning overhead is managed through Arc/Cow patterns in implementation
- Serialization enables checkpoint I/O and state inspection

### 2. Graph Building

**StateGraph** (`src/graph.rs`)

The builder for constructing graphs. Key structures:
- `nodes: HashMap<String, BoxedNode<S>>` - Node implementations keyed by name
- `edges: Vec<Edge>` - Simple transitions (A → B)
- `conditional_edges: Vec<Arc<ConditionalEdge<S>>>` - State-based routing (A → B|C|D)
- `parallel_edges: Vec<ParallelEdge>` - Fan-out/fan-in (A → [B, C, D])
- `entry_point: Option<String>` - Starting node

**Building Process:**
1. Create new graph: `StateGraph::new()`
2. Add nodes: `add_node(name, node)` or `add_node_from_fn(name, closure)`
3. Add edges: `add_edge()`, `add_conditional_edge()`, `add_parallel_edge()`
4. Set entry: `set_entry_point(name)`
5. Compile: `compile()` validates and creates CompiledGraph

**Validation During Compilation:**
- Entry point must be set and exist
- All edge targets must be valid nodes or END
- Conditional edge routes must be defined
- No orphaned nodes (warnings for unreachable nodes)

### 3. Execution Engine

**CompiledGraph** (`src/executor.rs`)

The compiled, validated graph ready for execution. Key execution methods:

**`invoke(state) -> Result<ExecutionResult<S>>`**
- Execute entire graph to completion
- Returns final state and execution metadata
- Single-threaded unless parallel edges present

**`stream(state, mode) -> Stream<StreamEvent<S>>`**
- Execute with streaming updates
- Modes: `Values` (state updates), `Updates` (node outputs), `Debug` (all events)
- Enables progress monitoring and real-time UI updates

**Execution Algorithm:**

```
1. Initialize: current_node = entry_point, state = initial_state
2. Loop:
   a. Save checkpoint (if checkpointer enabled)
   b. Emit "node_enter" event
   c. Execute node(state) -> Result<new_state>
   d. Emit "node_exit" event with timing
   e. Determine next node(s):
      - Check simple edges (A → B)
      - Evaluate conditional edges (returns node name based on state)
      - Handle parallel edges (fan-out to multiple nodes)
   f. If next == END or no edges, break
   g. If parallel: execute all nodes concurrently, merge results
   h. Set current_node = next, state = new_state
3. Save final checkpoint
4. Return ExecutionResult { state, checkpoints, metadata }
```

**Parallel Execution:**
- Uses `tokio::spawn` for concurrent node execution
- Each parallel branch receives state clone
- Results merged after all branches complete
- Errors from any branch propagate immediately

### 4. Node Abstraction

**Node Trait** (`src/node.rs`)

```rust
#[async_trait]
pub trait Node<S: GraphState>: Send + Sync {
    async fn invoke(&self, state: S) -> Result<S>;
}
```

Nodes are async functions that transform state. Implementations:

**FunctionNode**: Wraps closures/functions
```rust
graph.add_node_from_fn("process", |state| {
    Box::pin(async move {
        // Transform state
        Ok(state)
    })
});
```

**AgentNode**: Integrates DashFlow agents (calls agent, appends messages to state)

**RunnableNode**: Wraps any DashFlow `Runnable` (chains, prompts, models)

**ToolNode**: Executes tools based on state tool calls

**Design Notes:**
- `BoxedNode<S> = Arc<dyn Node<S>>` enables dynamic dispatch and shared ownership
- Arc enables node reuse and cheap cloning during graph compilation
- Async trait required for I/O-bound operations (LLM calls, DB queries)

### 5. Edge Types

**Simple Edge** (`src/edge.rs`)
```rust
pub struct Edge {
    from: String,
    to: String,
}
```
Unconditional transition: always go from A → B.

**Conditional Edge**
```rust
pub struct ConditionalEdge<S: GraphState> {
    from: String,
    condition: Arc<dyn Fn(&S) -> String + Send + Sync>,
    routes: HashMap<String, String>,
}
```
State-based routing: condition function examines state, returns next node name.

Example:
```rust
graph.add_conditional_edge("checker", |state: &MyState| {
    if state.score > 0.8 {
        "pass".to_string()
    } else {
        "fail".to_string()
    }
}, routes);
```

**Parallel Edge**
```rust
pub struct ParallelEdge {
    from: String,
    to: Vec<String>,
}
```
Fan-out: execute multiple nodes concurrently, then continue.

### 6. Checkpointing System

**Checkpointer Trait** (`src/checkpoint.rs`)

```rust
#[async_trait]
pub trait Checkpointer<S: GraphState>: Send + Sync {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()>;
    async fn load(&self, thread_id: &ThreadId) -> Result<Option<Checkpoint<S>>>;
    async fn list(&self, thread_id: &ThreadId) -> Result<Vec<CheckpointMetadata>>;
}
```

**Checkpoint Structure:**
- `id`: Unique identifier (thread_id + UUID)
- `thread_id`: Execution context identifier (isolates different sessions)
- `state`: Full state snapshot
- `node`: Node that was just executed
- `timestamp`: Creation time
- `parent_id`: Previous checkpoint (enables history traversal)
- `metadata`: Arbitrary key-value data

**Implementations:**

**MemoryCheckpointer**: In-memory storage (testing, single-process)
- Uses `Arc<Mutex<HashMap>>` for thread-safe access
- Fast, no I/O overhead
- Data lost on process restart

**FileCheckpointer**: Filesystem persistence (production)
- One file per checkpoint: `{checkpoint_dir}/{thread_id}/{checkpoint_id}.json`
- Enables resume after crashes
- Human-readable JSON format for debugging

**Checkpoint Workflow:**
1. Before executing each node: `checkpointer.save(current_state, node)`
2. On error: Latest checkpoint contains last good state
3. Resume: `checkpointer.load(thread_id)` returns last checkpoint
4. Continue from `checkpoint.node` with `checkpoint.state`

**Use Cases:**
- **Human-in-the-loop**: Pause execution, wait for human input, resume
- **Fault tolerance**: Recover from crashes without re-executing completed nodes
- **Debugging**: Inspect state at each step, replay from any checkpoint
- **Audit trails**: Track execution history for compliance

### 7. Event System

**EventCallback Trait** (`src/event.rs`)

```rust
pub trait EventCallback<S: GraphState>: Send + Sync {
    fn on_event(&self, event: &GraphEvent<S>);
}
```

**Event Types:**
- `GraphStart`: Graph execution begins
- `GraphEnd`: Graph execution completes
- `NodeEnter`: About to execute node
- `NodeExit`: Node execution finished (includes duration, error if failed)
- `EdgeTraversal`: Moving from node A → B (includes edge type)
- `StateUpdate`: State changed (includes before/after snapshots)
- `CheckpointSaved`: Checkpoint persisted

**Built-in Callbacks:**
- `PrintCallback`: Prints events to stdout (debugging)
- `CollectingCallback`: Collects events in Vec (testing, analysis)

**Custom Callbacks:**
```rust
struct MetricsCallback;
impl<S: GraphState> EventCallback<S> for MetricsCallback {
    fn on_event(&self, event: &GraphEvent<S>) {
        match event {
            GraphEvent::NodeExit { node, duration, .. } => {
                metrics::histogram!("node_duration", duration.as_secs_f64(),
                    "node" => node.clone());
            }
            _ => {}
        }
    }
}
```

### 8. Streaming Execution

**StreamMode** (`src/stream.rs`)

Execution can stream incremental results:

**`StreamMode::Values`**
- Emits state after each node execution
- Use case: Display current state in UI, monitor progress

**`StreamMode::Updates`**
- Emits node outputs (state diffs if nodes report changes)
- Use case: Show what each node produced

**`StreamMode::Debug`**
- Emits all events (enter, exit, edges, checkpoints)
- Use case: Detailed debugging, performance analysis

**Implementation:**
- Uses `async-stream` crate for async generators
- Buffers events minimally (backpressure handled by consumer)
- Compatible with `futures::Stream` trait

## Performance Characteristics

### Time Complexity

**Graph Compilation:** O(N + E) where N = nodes, E = edges
- Validates all edges point to existing nodes
- Builds lookup structures (hashmaps)

**Single Execution (Linear Graph):** O(N)
- Each node executed once
- Edge evaluation is O(1) per transition

**Parallel Execution:** O(N / P) where P = parallelism factor
- Parallel edges execute concurrently
- Limited by number of Tokio worker threads

**Conditional Edge Evaluation:** O(1)
- Condition function must be O(1) or fast
- Route lookup in HashMap is O(1)

**Checkpointing Overhead:**
- **Memory**: O(1) per checkpoint (state serialization)
- **File**: O(S) where S = serialized state size (disk I/O)
- Asynchronous, non-blocking

### Memory Usage

**Graph Structure:** O(N + E)
- Each node: ~200 bytes overhead (Arc, vtable pointer)
- Each edge: ~100 bytes (strings, enum discriminant)
- Example: 100 nodes, 150 edges ≈ 35KB

**State During Execution:**
- Single execution: 1 state copy in memory
- Parallel execution: P state copies (one per concurrent branch)
- Checkpointing: +1 copy per checkpoint (serialized to disk/memory)

**Optimization Techniques:**
- Node Arc sharing reduces duplication
- State clones are shallow where possible (Vec, HashMap use reference counting internally)
- Conditional edges use Arc for condition functions (shared across compilations)

### Benchmark Results

From `benches/graph_benchmarks.rs` (measured on M1 Mac, 8 cores):

**Compilation:**
- Simple graph (3 nodes, linear): ~15μs
- Complex graph (10 nodes, mixed edges): ~45μs
- Very fast, negligible overhead

**Execution (Simple State):**
- Sequential 3 nodes: ~12μs
- Sequential 5 nodes: ~18μs
- Conditional branching: ~15μs
- Parallel 3 nodes: ~35μs (overhead from spawning)

**Execution (Complex State with 5 messages, HashMap):**
- Sequential 5 nodes: ~25μs
- Parallel 3 nodes: ~55μs

**Checkpointing:**
- Memory checkpoint (save): ~3μs
- Memory checkpoint (load): ~2μs
- File checkpoint (save): ~150μs (includes JSON serialization + disk I/O)
- File checkpoint (load): ~100μs (includes disk I/O + JSON deserialization)

**Streaming:**
- Values mode: +5% overhead vs invoke
- Debug mode: +15% overhead (many events emitted)

**Key Insights:**
- Execution is very fast for CPU-bound nodes (~3-5μs per node)
- Real-world performance dominated by node work (LLM calls: 100ms-2s)
- Parallel execution has ~15-20μs spawn overhead, break-even at ~3 nodes
- File checkpointing adds ~150μs per node, acceptable for long-running workflows
- Memory checkpointing is nearly free (~3μs), suitable for frequent checkpoints

## Comparison with Python DashFlow

**Performance Advantages:**
- **Compilation**: 10-20x faster (Rust compilation vs Python interpretation overhead)
- **Execution**: 5-10x faster for CPU-bound nodes (no GIL, compiled code)
- **Memory**: 2-3x more efficient (no Python object overhead)
- **Parallel execution**: True parallelism (no GIL), scales linearly with cores

**Feature Parity:**
- ✅ StateGraph with typed state
- ✅ Simple, conditional, and parallel edges
- ✅ Checkpointing (memory and file)
- ✅ Streaming execution
- ✅ Event callbacks
- ✅ Human-in-the-loop patterns
- ✅ DashFlow integration (agents, tools, runnables)

**Differences:**
- Rust requires more explicit type definitions (tradeoff for safety)
- Python has more dynamic routing patterns (Rust requires trait objects)
- Rust compilation step adds development friction (but catches errors early)

## Design Patterns

### Pattern 1: Sequential Processing

```rust
graph.add_node("step1", step1_fn);
graph.add_node("step2", step2_fn);
graph.add_edge("step1", "step2");
graph.add_edge("step2", END);
```

Use when: Each step depends on previous step's output.

### Pattern 2: Conditional Routing

```rust
graph.add_conditional_edge("checker",
    |state| if state.score > threshold { "pass" } else { "retry" },
    routes);
```

Use when: Next step depends on state content (validation, classification, routing).

### Pattern 3: Parallel Map-Reduce

```rust
graph.add_parallel_edge("splitter", vec!["worker1", "worker2", "worker3"]);
graph.add_node("aggregator", merge_fn);
// Workers all connect to aggregator
```

Use when: Independent work can be parallelized, then combined.

### Pattern 4: Human-in-the-Loop

```rust
let checkpointer = FileCheckpointer::new("checkpoints")?;
let app = graph.compile()?
    .with_checkpointer(checkpointer)
    .with_thread_id("user-session-123");

// Execute until node needs human input
let result = app.invoke(state).await?;

// Later: resume from checkpoint
let checkpoint = checkpointer.load(&"user-session-123".into()).await?;
if let Some(cp) = checkpoint {
    // Resume from cp.state, starting at cp.node
}
```

Use when: Workflow requires human approval, input, or decision.

### Pattern 5: Supervisor-Worker

```rust
graph.add_node("supervisor", supervisor_fn); // Routes work
graph.add_node("worker1", worker1_fn);
graph.add_node("worker2", worker2_fn);
graph.add_conditional_edge("supervisor", route_fn, routes);
// Workers return to supervisor
graph.add_edge("worker1", "supervisor");
graph.add_edge("worker2", "supervisor");
```

Use when: Central coordinator delegates to specialized agents.

## Thread Safety

All components are thread-safe:
- **StateGraph**: Not Sync during building (single-threaded construction), compiled graph is Sync
- **CompiledGraph**: Send + Sync, can be shared across threads (Arc<CompiledGraph>)
- **Checkpointers**: Use Arc<Mutex<>> or async locks internally
- **Node execution**: Each invocation is independent, no shared mutable state

Safe patterns:
```rust
let app = Arc::new(graph.compile()?);

// Multiple threads can execute same graph concurrently
let handles: Vec<_> = (0..10).map(|i| {
    let app = app.clone();
    tokio::spawn(async move {
        let state = MyState { id: i };
        app.invoke(state).await
    })
}).collect();
```

## Error Handling

**Error Types:**
- `Error::NodeExecution`: Node returned error (includes node name, source error)
- `Error::InvalidGraph`: Compilation failed (missing entry, invalid edges)
- `Error::CheckpointError`: Save/load failed (I/O error, serialization error)
- `Error::Timeout`: Execution exceeded timeout

**Error Propagation:**
- Node errors: Stop execution immediately, return error with partial state
- Parallel node errors: Any branch error cancels all branches, returns first error
- Checkpoint errors: Non-fatal, execution continues (warning logged)

**Best Practices:**
- Use `Result<S>` return type in nodes
- Implement `std::error::Error` for custom node errors
- Use `?` operator for error propagation
- Add context with `.map_err(|e| Error::NodeExecution(...))`

## Testing Strategy

**Unit Tests:**
- Each module has tests: `cargo test --lib`
- Test individual components (edge evaluation, checkpointing, state)

**Integration Tests:**
- `tests/` directory: End-to-end graph execution
- Test patterns: sequential, parallel, conditional, checkpointing

**Examples as Tests:**
- `examples/` directory: 12 comprehensive examples
- Validated in CI: `cargo test --examples`

**Benchmarks:**
- `benches/graph_benchmarks.rs`: 19 performance benchmarks
- Run: `cargo bench --package dashflow`

## Future Optimizations

**Potential Improvements:**
1. **State Cloning**: Use Arc<State> + interior mutability to reduce clones
2. **Checkpoint Compression**: Compress state before saving (zstd, lz4)
3. **Incremental Checkpoints**: Only save state diffs, not full state
4. **Parallel Edge Optimization**: Reuse state references when nodes are read-only
5. **Streaming Checkpoint I/O**: Async write checkpoints without blocking execution
6. **Graph Analysis**: Detect parallelizable subgraphs automatically
7. **State Versioning**: Handle state schema evolution across checkpoints

**Current Status:**
These optimizations are not critical (current performance is excellent). Focus is on feature completeness and API stability.

## Conclusion

DashFlow's architecture prioritizes:
- **Safety**: Rust's type system catches errors at compile time
- **Performance**: Async execution, parallelism, minimal overhead
- **Reliability**: Checkpointing, error handling, observability
- **Usability**: Ergonomic API, comprehensive examples, integration with DashFlow

The result is a production-ready framework for building complex, stateful agent workflows with excellent performance characteristics and strong safety guarantees.
