# DashFlow Rust Architecture Guide

This document explains the design decisions, architecture, and implementation details of DashFlow Rust.

## Table of Contents

1. [Overview](#overview)
2. [Core Architecture](#core-architecture)
3. [Type System](#type-system)
4. [Graph Execution Model](#graph-execution-model)
5. [State Management](#state-management)
6. [Checkpointing System](#checkpointing-system)
7. [Streaming Architecture](#streaming-architecture)
8. [Event System](#event-system)
9. [Integration Layer](#integration-layer)
10. [Design Decisions](#design-decisions)
11. [Performance Considerations](#performance-considerations)

---

## Overview

DashFlow Rust is a graph-based workflow framework for building stateful, multi-agent LLM applications. It's inspired by Python DashFlow but redesigned for Rust's type system and async model.

**Key Design Goals:**

1. **Type Safety**: Compile-time guarantees for graph structure and state transitions
2. **Performance**: Zero-cost abstractions, efficient async execution
3. **Ergonomics**: Clean API that feels natural in Rust
4. **Flexibility**: Generic over state type, extensible node system
5. **Production-Ready**: Checkpointing, streaming, error handling, timeouts

---

## Core Architecture

DashFlow consists of several layers:

```
┌─────────────────────────────────────────────┐
│           User API Layer                    │
│  (StateGraph, add_node, add_edge, etc.)    │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│         Graph Compilation Layer             │
│    (Validation, optimization, builder)      │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│          Execution Engine                   │
│  (Async executor, routing, parallelism)    │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│       Supporting Systems                    │
│  (Checkpointing, Streaming, Events)        │
└─────────────────────────────────────────────┘
```

### Module Structure

```
dashflow/
├── src/
│   ├── graph.rs           # StateGraph builder and compilation
│   ├── node.rs            # Node trait and implementations
│   ├── edge.rs            # Edge types (simple, conditional, parallel)
│   ├── state.rs           # GraphState trait
│   ├── executor.rs        # CompiledGraph and execution engine
│   ├── checkpoint.rs      # Checkpointer trait and implementations
│   ├── stream.rs          # Streaming types and modes
│   ├── event.rs           # Event system and callbacks
│   ├── integration.rs     # DashFlow integration (RunnableNode, etc.)
│   └── error.rs           # Error types
```

---

## Type System

### Generic State

DashFlow is generic over the state type:

```rust
pub struct StateGraph<S> {
    // ...
}

pub trait GraphState: Clone + Send + Sync + 'static {}
```

**Why generic?**
- Type safety: State structure enforced at compile time
- Performance: No dynamic dispatch for state access
- Flexibility: Users define their own state types

**State Requirements:**
- `Clone`: Nodes need to clone state for transformations
- `Send + Sync`: State can be sent between threads (async)
- `'static`: State can live for entire graph execution

### Node Trait

```rust
pub trait Node<S>: Send + Sync {
    fn invoke(&self, state: S) -> BoxFuture<'_, Result<S>>;
}
```

**Key Points:**
- Generic over state type `S`
- Async via `BoxFuture` (allows different node implementations)
- Returns `Result<S>` for error handling
- `Send + Sync` for thread safety

**Why BoxFuture?**
- Allows different node types with different future sizes
- Enables trait objects (`Box<dyn Node<S>>`)
- Avoids complex HRTB (higher-ranked trait bounds) issues

### Closures as Nodes

Closures are automatically converted to nodes:

```rust
graph.add_node_from_fn("my_node", |state| {
    Box::pin(async move {
        // Transform state
        Ok(state)
    })
});
```

**Implementation:**
- Uses `FunctionNode<S, F>` wrapper
- `F: Fn(S) -> BoxFuture<'_, Result<S>>`
- Zero-cost abstraction (monomorphized at compile time)

---

## Graph Execution Model

### Compilation Phase

When you call `graph.compile()`, the following happens:

1. **Validation**: Check for missing nodes, unreachable nodes, invalid entry point
2. **Topology Analysis**: Build adjacency list, identify cycles
3. **Optimization**: (Future) Dead code elimination, parallel edge identification
4. **Construction**: Create `CompiledGraph` with validated structure

**Validation Rules:**
- Entry point must be set
- Entry point must be a valid node
- All edges must reference valid nodes
- Warn on unreachable nodes (not an error - may be intentional)
- Warn on cycles (not an error - cycles are a feature)

### Execution Algorithm

```rust
async fn invoke(&self, initial_state: S) -> Result<ExecutionResult<S>>
```

**Algorithm:**

1. Start at entry point with initial state
2. Execute current node with current state
3. If error: return error (no rollback - nodes are responsible for consistency)
4. Determine next node:
   - Check conditional edges first (in order added)
   - Fall back to simple edges
   - If next is END: terminate
   - If no edges and not END: implicit END
5. If checkpointing enabled: save state
6. If streaming enabled: emit event
7. If timeout exceeded: return timeout error
8. Repeat from step 2

**Cycle Handling:**
- Cycles are allowed (this is a key DashFlow feature)
- No cycle detection during execution
- Protection: graph-level timeout, node-level timeout
- User responsibility: Ensure conditional edges eventually lead to END

**Parallel Execution:**

Parallel edges execute concurrently:

```rust
graph.add_parallel_edges("node_a", &["node_b", "node_c", "node_d"]);
```

**Implementation:**
- Uses `tokio::spawn` for concurrent execution
- Each parallel branch gets cloned state
- Results are collected and merged
- If any branch errors: entire parallel block errors
- Timeout applies to entire parallel block

---

## State Management

### State Trait

```rust
pub trait GraphState: Clone + Send + Sync + 'static {}
```

**Automatic Implementation:**
Any type that satisfies the bounds automatically implements `GraphState`.

### State Transformation Pattern

Nodes transform state in one of two ways:

**1. Immutable Pattern (Recommended):**
```rust
|state| {
    Box::pin(async move {
        let mut new_state = state.clone();
        new_state.field = new_value;
        Ok(new_state)
    })
}
```

**2. Mutable Pattern:**
```rust
|mut state| {
    Box::pin(async move {
        state.field = new_value;
        Ok(state)
    })
}
```

Both patterns work. Immutable pattern is safer for debugging (original state preserved in calling scope).

### State Serialization

For checkpointing, state must implement `Serialize` + `Deserialize`:

```rust
#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    // ...
}
```

**Format:** JSON (via serde_json)
**Why JSON?** Human-readable, debuggable, compatible with Python DashFlow

---

## Checkpointing System

### Architecture

```
┌─────────────────────────────────────────────┐
│           Checkpointer Trait                │
│  (save, load, list, delete operations)     │
└─────────────────────────────────────────────┘
                    ↓
        ┌───────────────────────┐
        │                       │
┌───────▼──────┐       ┌────────▼─────────┐
│   Memory     │       │      File        │
│ Checkpointer │       │  Checkpointer    │
└──────────────┘       └──────────────────┘
```

### Checkpointer Trait

```rust
pub trait Checkpointer: Send + Sync {
    async fn save(&self, checkpoint: Checkpoint) -> Result<()>;
    async fn load(&self, id: CheckpointId) -> Result<Option<Checkpoint>>;
    async fn list(&self, thread_id: Option<ThreadId>) -> Result<Vec<Checkpoint>>;
    async fn delete(&self, id: CheckpointId) -> Result<()>;
    async fn delete_thread(&self, thread_id: ThreadId) -> Result<()>;
    async fn get_latest(&self, thread_id: ThreadId) -> Result<Option<Checkpoint>>;
}
```

### Checkpoint Structure

```rust
pub struct Checkpoint {
    pub id: CheckpointId,           // Unique ID
    pub thread_id: ThreadId,        // Conversation/workflow thread
    pub timestamp: SystemTime,      // When checkpoint was created
    pub state: serde_json::Value,   // Serialized state
    pub metadata: CheckpointMetadata, // User-defined metadata
}
```

**Design Decisions:**

- **ID Generation**: UUIDs for uniqueness across distributed systems
- **Thread IDs**: Group related checkpoints (e.g., conversation history)
- **Timestamps**: Automatic, for ordering and expiration
- **JSON State**: Type-erased for storage flexibility
- **Metadata**: Extensible (HashMap<String, String>) for custom tags

### Memory Checkpointer

**Storage:** `Arc<RwLock<HashMap<CheckpointId, Checkpoint>>>`

**Characteristics:**
- Fast (in-memory)
- Lost on process exit
- Suitable for development, testing, short-lived workflows

### File Checkpointer

**Storage:** One JSON file per checkpoint in specified directory

**Characteristics:**
- Persistent across restarts
- Human-readable (JSON files)
- Simple recovery (just read directory)
- Not suitable for high-frequency checkpointing (disk I/O)

**File Naming:** `{checkpoint_id}.json`

**Future:** Database checkpointer (Postgres, SQLite) for production use

---

## Streaming Architecture

### Stream Modes

DashFlow supports three streaming modes:

**1. Values (Default):**
```rust
StreamMode::Values
```
Emit full state after each node execution.

**Use Case:** Monitor state evolution, debug workflows

**2. Updates:**
```rust
StreamMode::Updates
```
Emit only state changes (diff from previous state).

**Use Case:** Efficient updates, incremental UI rendering

**3. Events:**
```rust
StreamMode::Events
```
Emit detailed events (node_start, node_end, error).

**Use Case:** Fine-grained monitoring, metrics, debugging

### Stream Implementation

```rust
pub async fn stream(&self, initial_state: S) -> impl Stream<Item = Result<StreamEvent<S>>>
```

**Implementation Details:**

- Uses `tokio::sync::mpsc::channel` for event passing
- Executor sends events, stream consumers receive
- Backpressure via bounded channel
- Error propagation: errors appear as stream items, then stream ends

**StreamEvent Structure:**

```rust
pub enum StreamEvent<S> {
    Values(S),                           // Full state
    Updates(serde_json::Value),          // State diff (future)
    NodeStart { node_id: String },       // Node execution started
    NodeEnd { node_id: String, state: S }, // Node execution finished
    Error { error: String },             // Node error
}
```

---

## Event System

### Architecture

```rust
pub trait EventCallback: Send + Sync {
    fn on_node_start(&self, node_id: &str);
    fn on_node_end(&self, node_id: &str);
    fn on_edge_traversed(&self, from: &str, to: &str, edge_type: EdgeType);
}
```

**Callback Registration:**

```rust
let graph = graph.compile()?
    .with_callback(PrintCallback)
    .with_callback(MetricsCallback::new());
```

**Multiple Callbacks:** Supported. All registered callbacks are called in order.

### Built-in Callbacks

**1. PrintCallback:**
Prints execution trace to stdout (debugging).

**2. CollectingCallback:**
Collects events in memory (testing, analysis).

### Custom Callbacks

Users implement `EventCallback` for:
- Metrics (Prometheus, StatsD)
- Logging (structured logs)
- Tracing (OpenTelemetry)
- Auditing (security, compliance)

**Example:**

```rust
struct MetricsCallback { /* ... */ }

impl EventCallback for MetricsCallback {
    fn on_node_start(&self, node_id: &str) {
        METRICS.node_executions.inc(node_id);
        METRICS.node_start_time.set(node_id, Instant::now());
    }

    fn on_node_end(&self, node_id: &str) {
        let duration = METRICS.node_start_time.get(node_id).elapsed();
        METRICS.node_duration.observe(node_id, duration);
    }
}
```

---

## Integration Layer

### RunnableNode

Wraps any DashFlow `Runnable` as a graph node:

```rust
pub struct RunnableNode<R, S> {
    runnable: Arc<R>,
    input_fn: Arc<dyn Fn(&S) -> R::Input + Send + Sync>,
    output_fn: Arc<dyn Fn(S, R::Output) -> S + Send + Sync>,
}
```

**Purpose:** Use DashFlow chains/components as graph nodes

**Input/Output Mapping:**
- `input_fn`: Extract chain input from graph state
- `output_fn`: Merge chain output into graph state

**Example:**

```rust
let chain = create_my_chain();
let node = RunnableNode::new(
    chain,
    |state: &MyState| state.query.clone(),  // Extract input
    |mut state, output| {                    // Merge output
        state.response = output;
        state
    },
);
graph.add_node("chain", node);
```

### AgentNode

Wraps DashFlow agent executors:

```rust
pub struct AgentNode<A, S> {
    agent: Arc<A>,
    input_fn: Arc<dyn Fn(&S) -> String + Send + Sync>,
    output_fn: Arc<dyn Fn(S, String) -> S + Send + Sync>,
}
```

**Similar to RunnableNode but specialized for agents:**
- Input: Extract query string
- Output: Agent response string

### ToolNode

Wraps tools with input/output mapping:

```rust
pub struct ToolNode<S> {
    tool: Arc<dyn Tool>,
    input_fn: Arc<dyn Fn(S) -> ToolInput + Send + Sync>,
    output_fn: Arc<dyn Fn(S, String) -> S + Send + Sync>,
}
```

**Key Feature:** `ToolInput` enum supports multiple input types:
- `String`: Simple string input
- `Structured`: JSON object input

**Pattern:** Input function extracts tool parameters from state, output function merges tool results back into state.

---

## Design Decisions

### Why Not Petgraph?

Initial plan included `petgraph` for graph algorithms. **Decision: Not needed.**

**Reasons:**

1. **Simple Structure**: DashFlow graphs are simple (nodes + edges + routing functions)
2. **Custom Routing**: Conditional edges require custom logic (not standard graph algorithms)
3. **Performance**: Direct adjacency list is faster for our use case
4. **Dependencies**: One less dependency to maintain

**Current Approach:**
- Nodes: `HashMap<String, Box<dyn Node<S>>>`
- Edges: `Vec<Edge>` (from/to pairs)
- Conditional Edges: `Vec<ConditionalEdge<S>>` (from/condition/routes)

### Why Arc Instead of Rc?

All shared state uses `Arc` (not `Rc`):

**Reason:** `Send + Sync` requirement for async. `Rc` is not thread-safe.

**Usage:**
- `Arc<dyn Node<S>>`: Nodes shared across async tasks
- `Arc<dyn Checkpointer>`: Checkpointer shared across executor
- `Arc<RwLock<...>>`: Mutable shared state (e.g., MemoryCheckpointer)

### Why BoxFuture Instead of async fn?

Node trait uses `BoxFuture<'_, Result<S>>` instead of `async fn`.

**Reason:** Trait objects.

`async fn` in traits creates unnamed opaque future types. These can't be type-erased into `Box<dyn Node<S>>`.

`BoxFuture` is explicit heap allocation, enables trait objects.

**Trade-off:** Small allocation cost (one Box per node invocation) for flexibility.

### Why JSON for Checkpoints?

Checkpoint state is serialized as `serde_json::Value`.

**Reasons:**

1. **Debugging**: Human-readable checkpoint files
2. **Compatibility**: Python DashFlow uses JSON
3. **Flexibility**: Schema evolution (add fields without breaking old checkpoints)
4. **Universality**: Any language can read JSON checkpoints

**Alternative Considered:** Bincode (binary, faster, smaller)
**Decision:** Prioritize debuggability over performance for v0.1

### Why Clone State?

State must implement `Clone`.

**Reasons:**

1. **Parallel Execution**: Each parallel branch needs its own state copy
2. **Checkpointing**: Save state without consuming it
3. **Retry Logic**: Retry failed nodes with original state (via `with_retry_policy()`)
4. **Immutability**: Functional programming style (safer, easier to reason about)

**Performance:** For large state, use `Arc<T>` fields:

```rust
#[derive(Clone)]
struct MyState {
    large_data: Arc<Vec<u8>>,  // Cheap to clone (just Arc pointer)
    small_data: String,         // Cloned as needed
}
```

---

## Performance Considerations

### Zero-Cost Abstractions

DashFlow uses Rust's zero-cost abstractions:

1. **Generics**: `StateGraph<S>` is monomorphized (no runtime cost)
2. **Trait Objects**: Only where necessary (`Node` trait)
3. **Async**: Tokio's async runtime (efficient scheduling)
4. **No Dynamic Dispatch**: State access is compile-time

### Async Execution

**Executor is fully async:**

- Nodes are async functions (can await HTTP, DB, LLM calls)
- Parallel edges use `tokio::spawn` (true parallelism)
- No blocking (entire executor is non-blocking)

**Best Practices:**

- Use async LLM clients (don't block tokio threads)
- Use `tokio::task::spawn_blocking` for CPU-bound work
- Set timeouts to prevent hanging nodes

### Memory Usage

**Per-Execution Overhead:**

- State clones (depends on state size)
- One `Box<dyn Node<S>>` per node
- One `BoxFuture` per node invocation
- Event channel buffer (bounded, configurable)

**Recommendations:**

- Keep state small (use `Arc` for large data)
- Disable streaming if not needed (saves channel overhead)
- Disable checkpointing if not needed (saves serialization)

### Optimization Opportunities

**Future Optimizations:**

1. **Arena Allocation**: Allocate nodes/states from arena (fewer allocations)
2. **Parallel Compilation**: Parallelize graph validation
3. **State Diffing**: Only serialize state changes (smaller checkpoints)
4. **Lazy Checkpointing**: Checkpoint on-demand, not every node
5. **Node Fusion**: Merge sequential nodes into single node (fewer dispatches)

---

## Comparison to Python DashFlow

| Aspect | Python DashFlow | Rust DashFlow |
|--------|-----------------|----------------|
| State Type | Untyped dict | Generic `S: GraphState` |
| Node Type | Any callable | `Fn(S) -> BoxFuture<Result<S>>` |
| Error Handling | Exceptions | `Result<S, Error>` |
| Async | asyncio | Tokio |
| Parallelism | asyncio.gather | tokio::spawn |
| Checkpointing | Protocol (structural typing) | Trait (nominal typing) |
| Streaming | Async generator | Stream trait |
| Performance | Slower (Python overhead) | Faster (compiled, no GIL) |

**Key Difference: Type Safety**

Python DashFlow has no type safety:
```python
# Python: No compile-time check
state["typo_in_field_name"] = value
```

Rust DashFlow is type-safe:
```rust
// Rust: Compile error if field doesn't exist
state.typo_in_field_name = value;  // ❌ Compile error
```

---

## Summary

**DashFlow Rust Architecture:**

- **Type-Safe**: Generic state, compile-time guarantees
- **Async-First**: Built on Tokio, non-blocking execution
- **Modular**: Clean separation (graph, executor, checkpointing, streaming)
- **Extensible**: Traits for nodes, checkpointers, callbacks
- **Production-Ready**: Error handling, timeouts, persistence

**Design Philosophy:**

1. Safety without sacrificing performance
2. Ergonomics without sacrificing flexibility
3. Compatibility with Python DashFlow (where reasonable)
4. Idiomatic Rust (traits, ownership, async)

**Next Steps:**

- See `../MIGRATION_GUIDE.md` for migration guide
- See `BEST_PRACTICES.md` for usage patterns
- See examples/ for complete workflows
