# DashFlow Components Catalog

**For AI Assistants:** This document is part of the [AI Parts Catalog](AI_PARTS_CATALOG.md). It covers DashFlow-specific components including state management, graph execution, checkpointing, introspection, optimization, and self-improvement systems.

**Version:** 1.11.3
**Last Updated:** 2026-01-04 (Worker #2502 - Fix stale executor line count 14,833→13,089)

**Parent Document:** [AI_PARTS_CATALOG.md](AI_PARTS_CATALOG.md)

---

## DashFlow Components

### GraphState Trait (State Management)
**Location:** `crates/dashflow/src/state.rs`

GraphState is the foundational trait for all state types used in DashFlow. State is the data that flows through the graph, being transformed by each node. Every graph requires a state type implementing GraphState to track progress, store intermediate results, and enable routing decisions.

**Core Concept:**

State is the shared context that persists across node executions. When a graph executes:
1. **Initial state** is passed to the first node
2. **Each node** receives current state, transforms it, returns updated state
3. **Updated state** flows to next node(s) based on edges
4. **Final state** is returned when graph completes

This design enables:
- **Data flow through graph:** State carries information between nodes
- **Stateful workflows:** Nodes can inspect previous results (stored in state)
- **Conditional routing:** Edges examine state to decide next node
- **Checkpointing:** State can be serialized and restored (pause/resume workflows)
- **Parallel execution:** State cloned for concurrent node execution

**GraphState Trait:**

The trait is automatically implemented for any type meeting these requirements:

```rust
pub trait GraphState:
    Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static
{}
```

**Requirements Explained:**

| Requirement | Purpose | Example |
|-------------|---------|---------|
| `Clone` | State cloned for parallel nodes | `state.clone()` for each parallel branch |
| `Send` | State can cross thread boundaries | Tokio spawns nodes on different threads |
| `Sync` | State can be shared across threads | Multiple nodes may read state concurrently |
| `Serialize` | State can be serialized to bytes | Checkpointing saves state to Redis/Postgres |
| `Deserialize` | State can be deserialized from bytes | Restore graph execution from checkpoint |
| `'static` | State has no borrowed references | State must outlive node executions |

**Why These Constraints:**

**Clone:** Parallel edges require independent state copies. Without Clone, concurrent execution impossible (one state, multiple nodes need it simultaneously).

**Send + Sync:** Tokio runtime executes nodes on thread pool. Without Send, state cannot move across threads (blocks parallelism). Without Sync, shared references impossible (nodes can't read state concurrently).

**Serialize + Deserialize:** Checkpointing requires state persistence. Without serialization, cannot save/restore workflows (no pause/resume capability).

**'static:** State cannot borrow from local scopes. Without 'static, state might reference stack data that's deallocated before node completes (undefined behavior).

**Defining Custom State:**

```rust
use serde::{Deserialize, Serialize};
use dashflow::GraphState;

// Define your state type
#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    messages: Vec<String>,
    user_id: String,
    iteration: u32,
}

// GraphState is automatically implemented (blanket implementation)
// No need to manually implement the trait!
```

**AgentState (Example State Type):**

DashFlow provides AgentState as a reference implementation for multi-agent workflows:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Messages exchanged between agents
    pub messages: Vec<String>,
    /// Current iteration count
    pub iteration: u32,
    /// Next node to execute (for conditional routing)
    pub next: Option<String>,
    /// Arbitrary metadata
    pub metadata: serde_json::Value,
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            iteration: 0,
            next: None,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn add_message(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
    }

    pub fn increment_iteration(&mut self) {
        self.iteration += 1;
    }

    pub fn set_next(&mut self, next: impl Into<String>) {
        self.next = Some(next.into());
    }
}
```

**AgentState Design Rationale:**

- **messages:** Core data (conversation history, tool results)
- **iteration:** Loop counter (prevent infinite loops in ReAct agents)
- **next:** Explicit routing (nodes can set next node, edges use this field)
- **metadata:** Extensibility (user_id, session_id, tags, etc. without changing state type)

**State Design Patterns:**

**Pattern 1: Minimal State (Only What's Needed)**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct SearchState {
    query: String,              // Input
    results: Vec<String>,       // Output
}
```

**When to Use:**
- Simple workflows (few nodes, clear data flow)
- Stateless operations (nodes don't depend on previous results)
- Performance-critical (minimal cloning overhead)

**Pattern 2: Rich State (Comprehensive Context)**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<Message>,     // Conversation history
    tools: Vec<String>,         // Available tools
    iteration: u32,             // Loop counter
    user_id: String,            // User context
    session_id: String,         // Session tracking
    metadata: serde_json::Value, // Arbitrary data
}
```

**When to Use:**
- Complex workflows (many nodes, interdependent operations)
- Stateful operations (nodes inspect previous results)
- Conditional routing (edges need rich state to decide next node)
- Production systems (user tracking, session management, telemetry)

**Pattern 3: Nested State (Subgraph Composition)**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct ParentState {
    input: String,
    child_result: Option<ChildState>,
    output: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct ChildState {
    intermediate: String,
}
```

**When to Use:**
- Hierarchical workflows (subgraphs need independent state)
- Reusable components (child state encapsulates subgraph logic)
- Clear separation (parent doesn't know child internals)

**State Lifecycle:**

```
1. Initialize: state = MyState::default()
2. Graph invocation: app.invoke(state)
3. Node execution: state = node1.execute(state).await?
4. Edge routing: next = edge.condition(&state)
5. Next node: state = node2.execute(state).await?
6. Repeat until END: ...
7. Return final state: Result<MyState>
```

**State Cloning (Parallel Execution):**

Parallel edges clone state for each target node:

```rust
// Parallel edge: node1 → [node2, node3, node4]
let state_original = node1.execute(state).await?;

// Clone state for each parallel node
let state2 = state_original.clone();  // Independent copy
let state3 = state_original.clone();  // Independent copy
let state4 = state_original.clone();  // Independent copy

// Execute in parallel (Tokio spawn)
let (result2, result3, result4) = tokio::join!(
    node2.execute(state2),
    node3.execute(state3),
    node4.execute(state4),
);

// Merge results using reducers
let final_state = reduce(state_original, result2, result3, result4);
```

**Clone Performance:**

- **Cheap clones:** Small state (few fields) ≈ 100-500ns
- **Expensive clones:** Large state (many messages) ≈ 1-10μs
- **Optimization:** Use Arc<T> for large read-only data (cheap pointer copy)

**Example: Arc for Large Data**
```rust
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
struct OptimizedState {
    // Frequently modified (cloned per node)
    messages: Vec<String>,
    iteration: u32,

    // Read-only, large (Arc clone is cheap pointer copy)
    #[serde(skip)]
    model_config: Arc<LargeConfig>,  // 10KB config, cloned many times
}
```

**State Serialization (Checkpointing):**

State must be serializable for checkpointing (save/restore workflows):

```rust
use serde_json;

// Serialize state to JSON
let state = AgentState::new();
let json = serde_json::to_string(&state).unwrap();

// Deserialize state from JSON
let restored: AgentState = serde_json::from_str(&json).unwrap();
```

**Checkpointer Support:**
- **MemorySaver:** In-memory (testing)
- **Redis:** Remote (distributed systems)
- **Postgres:** Remote (persistent storage)
- **S3:** Remote (large state, cold storage)
- **DynamoDB:** Remote (AWS-native)

**State Field Reducers:**

Reducers define how partial state updates merge with existing state:

```rust
#[derive(GraphState)]
struct State {
    #[add_messages]  // Reducer: append messages (not replace)
    messages: Vec<Message>,

    // Default reducer: last write wins
    user_id: String,
}

// Node returns partial update
node_output = State {
    messages: vec![new_message],
    user_id: "".to_string(),  // Empty (not changed)
};

// Reducer merges intelligently
// - messages: appends new_message to existing messages
// - user_id: keeps existing user_id (not overwritten by empty string)
```

**See State Reducers section** (line 720 below) for details.

**Use Cases:**

**Use Case 1: ReAct Agent (Tool Calling)**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<Message>,  // Conversation history
    iteration: u32,          // Loop counter (max 10 iterations)
}

// Node: LLM generates response (may include tool calls)
// Node: Tool executor runs tools, adds results to messages
// Edge: Route to tools if tool_calls present, END if none
// Node: LLM sees tool results, generates next response
```

**Use Case 2: Data Pipeline**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct PipelineState {
    raw_data: String,        // Input (scrape)
    parsed_data: Vec<Item>,  // Intermediate (parse)
    validated: bool,         // Flag (validate)
    stored: bool,            // Flag (store)
}

// Nodes: scrape → parse → validate → transform → store
// Each node updates one field, passes state to next node
```

**Use Case 3: Multi-Agent Workflow**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct MultiAgentState {
    task: String,                      // Original task
    research_results: Vec<String>,     // Researcher output
    analysis: String,                  // Analyst output
    report: String,                    // Writer output
}

// Nodes: researcher → analyst → writer
// Each agent adds to state, next agent uses previous results
```

**Performance Characteristics:**

- **State creation:** 10-100ns (depends on field count)
- **State clone (small):** 100-500ns (few fields, simple types)
- **State clone (large):** 1-10μs (many fields, complex types)
- **State serialization:** 1-100μs (depends on size, format)
- **State deserialization:** 1-100μs (depends on size, format)

**Typical state sizes:**
- Minimal state (query + result): ~100 bytes
- Agent state (10-20 messages): ~5-10 KB
- Rich state (100+ messages + metadata): ~50-100 KB

**State overhead is negligible** compared to node execution (LLM calls dominate: 500-2000ms).

**upstream Python DashFlow Compatibility:**

Rust GraphState matches upstream Python DashFlow TypedDict state semantics:

| Feature | upstream Python DashFlow | DashFlow | Notes |
|---------|------------------|----------------|-------|
| State type | `TypedDict` | `#[derive(Clone, Serialize, Deserialize)]` struct | Both require field declarations |
| Requirements | Python typing (runtime) | Trait bounds (compile-time) | Rust enforces at compile time |
| Serialization | Implicit (dict) | Explicit (Serialize/Deserialize) | Rust requires derive macro |
| Cloning | Implicit (shallow copy) | Explicit (Clone trait) | Rust requires explicit clone |
| Thread safety | GIL (implicit) | Send + Sync (explicit) | Rust requires explicit bounds |

**Python Example (Equivalent):**

```python
from typing import TypedDict, List

# Python: TypedDict state
class AgentState(TypedDict):
    messages: List[str]
    iteration: int
    next: str | None

# Rust: Struct state
#[derive(Clone, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<String>,
    iteration: u32,
    next: Option<String>,
}
```

**Best Practices:**

1. **Keep state minimal:** Only fields needed for decision-making (avoid bloat)
2. **Use Option<T> for optional fields:** Explicit None vs Some(value) (clearer intent)
3. **Avoid large clones:** Use Arc<T> for read-only large data (cheap pointer copy)
4. **Serialize efficiently:** Use bincode for binary checkpoints (smaller, faster than JSON)
5. **Version state types:** Add version field for schema evolution (backward compatibility)
6. **Document state fields:** Each field should have clear purpose (maintainability)
7. **Test serialization:** Verify state can roundtrip (serialize → deserialize) without loss
8. **Avoid non-serializable types:** File handles, sockets, function pointers (checkpointing breaks)

**Common Pitfalls:**

**Pitfall 1: Forgetting Clone**
```rust
// ❌ Missing Clone
#[derive(Serialize, Deserialize)]
struct State {
    data: String,
}
// Parallel edges fail: "the trait `Clone` is not implemented"
```

**Fix:** Add `#[derive(Clone)]`

**Pitfall 2: Non-Serializable Fields**
```rust
// ❌ Function pointer (cannot serialize)
#[derive(Clone, Serialize, Deserialize)]
struct State {
    callback: fn(),  // Error: function pointers cannot be serialized
}
```

**Fix:** Use `#[serde(skip)]` or remove non-serializable fields

**Pitfall 3: Borrowed References**
```rust
// ❌ Borrowed reference (not 'static)
#[derive(Clone, Serialize, Deserialize)]
struct State<'a> {
    data: &'a str,  // Lifetime parameter prevents 'static
}
```

**Fix:** Use owned types (`String` instead of `&str`)

**Code Pointers:**

- Core module: `crates/dashflow/src/state.rs`
- GraphState trait: `crates/dashflow/src/state.rs:30-33`
- AgentState example: `crates/dashflow/src/state.rs:264-319`
- Tests: `crates/dashflow/src/state.rs:602-1137` (comprehensive test suite)
- State reducers: `crates/dashflow/src/reducer.rs` (field-specific merge logic)
- Examples: `crates/dashflow/examples/` (all examples define custom state types)

### MergeableState Trait & Derive Macro (Parallel Execution)
**Location:** `crates/dashflow/src/state.rs` (trait), `crates/dashflow-derive/src/lib.rs` (derive macro)

MergeableState enables automatic merging of parallel execution results in DashFlow. When multiple nodes execute in parallel on cloned state, their results must be merged back into a single state. The MergeableState trait defines how this merge happens.

**Core Concept:**

In parallel execution:
1. **State cloned** for each parallel branch
2. **Nodes execute** independently (concurrent modifications)
3. **Results merged** using MergeableState::merge() to reconcile changes
4. **Merged state** flows to next node

**MergeableState Trait:**

```rust
pub trait MergeableState {
    fn merge(&mut self, other: &Self);
}
```

**Derive Macro (Automatic Implementation):**

The `#[derive(MergeableState)]` macro automatically generates merge implementations based on field types:

```rust
use dashflow_derive::MergeableState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ResearchState {
    findings: Vec<String>,           // Extends with other's elements
    queue: VecDeque<String>,          // Extends with other's elements
    tags: HashSet<String>,            // Extends (deduplicates automatically)
    ordered_tags: BTreeSet<String>,   // Extends (maintains sorted order)
    metadata: HashMap<String, i32>,   // Extends (overwrites on key collision)
    ordered_meta: BTreeMap<String, i32>, // Extends (overwrites, maintains order)
    iteration: usize,                 // Takes max value
    summary: String,                  // Concatenates with newline
    completed: bool,                  // Logical OR (true if either is true)
}
```

**Merge Strategies by Type:**

| Type | Merge Strategy | Example |
|------|---------------|---------|
| `Vec<T>` | Extend (append other's elements) | `[1,2]` + `[3,4]` = `[1,2,3,4]` |
| `VecDeque<T>` | Extend (append other's elements) | `[1,2]` + `[3,4]` = `[1,2,3,4]` |
| `HashSet<T>` | Extend (auto-deduplicates) | `{1,2}` + `{2,3}` = `{1,2,3}` |
| `BTreeSet<T>` | Extend (deduplicates, sorted) | `{2,1}` + `{3,2}` = `{1,2,3}` |
| `HashMap<K,V>` | Extend (overwrites keys) | `{a:1}` + `{a:2,b:3}` = `{a:2,b:3}` |
| `BTreeMap<K,V>` | Extend (overwrites, sorted keys) | `{b:1}` + `{a:2}` = `{a:2,b:1}` |
| `Option<T>` | Take other if self is None | `None` + `Some(5)` = `Some(5)` |
| `String` | Concatenate with newline | `"a"` + `"b"` = `"a\nb"` |
| `bool` | Logical OR | `false` + `true` = `true` |
| Numeric types | Take max value | `5` + `10` = `10` |
| Other types | Keep self (no merge) | `self` + `other` = `self` |

**Performance Characteristics:**

| Type | Time Complexity | Notes |
|------|----------------|-------|
| `Vec<T>`, `VecDeque<T>` | O(n) | n = size of other |
| `HashSet<T>`, `HashMap<K,V>` | O(n) | n = size of other |
| `BTreeSet<T>`, `BTreeMap<K,V>` | O(n log m) | n = other size, m = self size |
| `Option<T>`, `String`, `bool`, numeric | O(1) | Constant time |

**When to Use Derive Macro:**

Use `#[derive(MergeableState)]` when merge logic is standard:
- Parallel searches (accumulate results in Vec/HashSet)
- Multi-agent workflows (collect findings from agents)
- Aggregation pipelines (merge statistics, counts)

**When to Implement Manually:**

Implement `MergeableState` manually when custom logic is needed:
- Prefer longer/newer value over shorter/older
- Complex conflict resolution (e.g., CRDTs)
- Domain-specific merge rules (e.g., take minimum instead of maximum)

**Example: Manual Implementation (Custom Logic)**

```rust
impl MergeableState for CodeAssistantState {
    fn merge(&mut self, other: &Self) {
        // Standard: Extend messages
        self.messages.extend(other.messages.clone());

        // Custom: Prefer longer code (more complete solution)
        if other.current_code.len() > self.current_code.len() {
            self.current_code = other.current_code.clone();
        }

        // Custom: Concatenate with separator
        if !other.test_results.is_empty() {
            if !self.test_results.is_empty() {
                self.test_results.push_str("\n---\n");
            }
            self.test_results.push_str(&other.test_results);
        }
    }
}
```

**Parallel Execution Example:**

```rust
use dashflow::{StateGraph, END};

// State with derive macro
#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ParallelState {
    results: Vec<String>,  // Accumulated from parallel nodes
}

let mut graph = StateGraph::new();
graph.add_node("search1", search_node1);
graph.add_node("search2", search_node2);
graph.add_node("search3", search_node3);
graph.add_node("aggregate", aggregate_node);

// Parallel edges (state cloned 3 times)
graph.add_edge("__start__", "search1");
graph.add_edge("__start__", "search2");
graph.add_edge("__start__", "search3");

// Results merged automatically before aggregate
graph.add_edge("search1", "aggregate");
graph.add_edge("search2", "aggregate");
graph.add_edge("search3", "aggregate");
graph.add_edge("aggregate", END);

let app = graph.compile();
let result = app.invoke(ParallelState { results: vec![] }).await?;
// result.results contains merged output from all 3 search nodes
```

**How Merging Works (Internal):**

```rust
// 1. Initial state
let state = ParallelState { results: vec![] };

// 2. Clone for parallel nodes
let mut state1 = state.clone();
let mut state2 = state.clone();
let mut state3 = state.clone();

// 3. Parallel execution (independent modifications)
state1.results.push("result from search1".to_string());
state2.results.push("result from search2".to_string());
state3.results.push("result from search3".to_string());

// 4. Merge results (MergeableState::merge)
let mut merged = state.clone();
merged.merge(&state1);  // merged.results = ["result from search1"]
merged.merge(&state2);  // merged.results = ["result from search1", "result from search2"]
merged.merge(&state3);  // merged.results = ["...", "...", "result from search3"]

// 5. Continue with merged state
aggregate_node.execute(merged).await?;
```

**Common Patterns:**

**Pattern 1: Multi-Agent Research (Accumulate Findings)**
```rust
#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ResearchState {
    findings: Vec<String>,     // Each agent adds findings
    sources: HashSet<String>,  // Auto-deduplication
}
```

**Pattern 2: Parallel Tool Calls (Merge Metadata)**
```rust
#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ToolState {
    messages: Vec<Message>,         // Accumulate tool results
    metadata: HashMap<String, i32>, // Merge statistics
}
```

**Pattern 3: Aggregation Pipeline (Max/Counts)**
```rust
#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct AggregationState {
    max_score: f64,     // Takes maximum
    total_items: usize, // Takes maximum (or sum if custom impl)
}
```

**BTree vs Hash Collections:**

- **Hash variants (HashSet, HashMap):** Faster (O(n) merge), unordered
- **BTree variants (BTreeSet, BTreeMap):** Slower (O(n log m) merge), deterministic iteration order

**Choose BTree when:**
- Deterministic output required (testing, reproducibility)
- Sorted iteration needed
- Small collections (performance difference negligible)

**Choose Hash when:**
- Performance critical
- Order doesn't matter
- Large collections

**Pitfalls:**

**Pitfall 1: Forgetting to Implement MergeableState**
```rust
// Without MergeableState, parallel edges fail at runtime
#[derive(Clone, Serialize, Deserialize)]
struct State {
    results: Vec<String>,
}
// Error: "State does not implement MergeableState"
```

**Fix:** Add `#[derive(MergeableState)]`

**Pitfall 2: Manual Implementation Without Cloning**
```rust
// Moves data instead of cloning
fn merge(&mut self, other: &Self) {
    self.results.extend(other.results); // Error: cannot move out of &Self
}
```

**Fix:** Clone other's data: `self.results.extend(other.results.clone())`

**Pitfall 3: Order-Dependent Merge**
```rust
// If merge order matters, results are non-deterministic
fn merge(&mut self, other: &Self) {
    self.value = other.value; // Last merge wins (depends on execution timing)
}
```

**Fix:** Use commutative operations (Vec extend, HashSet extend, max, OR)

**Code Pointers:**

- MergeableState trait: `crates/dashflow/src/state.rs:223-258`
- Derive macro: `crates/dashflow-derive/src/lib.rs:175-391`
- Derive tests: `crates/dashflow-derive/tests/derive_tests.rs` (comprehensive tests)
- Parallel state merge: `crates/dashflow/src/state.rs:149-215` (merge_parallel_custom examples)
- Examples: `examples/apps/librarian/src/main.rs` (DeriveMergeableState usage)

### StateGraph
**Location:** `crates/dashflow/src/graph.rs`

Build multi-agent workflows as directed graphs.

```rust
use dashflow::StateGraph;

let mut graph = StateGraph::new();
graph.add_node("researcher", research_fn);
graph.add_node("writer", write_fn);
graph.add_edge("researcher", "writer");
graph.set_entry_point("researcher");

let app = graph.compile()?;
let result = app.invoke(state).await?;
```

**Key Methods:**
- `add_node(name, fn)` - Add node
- `add_edge(from, to)` - Add transition
- `add_conditional_edge(from, condition, mapping)` - Conditional routing
- `add_parallel_edge(from, targets)` - Parallel execution
- `set_entry_point(name)` - Set start node
- `compile()` - Build executable graph

**Code Pointer:** `crates/dashflow/src/graph/mod.rs:100-600`

### State Reducers
**Location:** `crates/dashflow/src/reducer.rs`

State field reducers for merging partial state updates during graph execution. When nodes return partial state (only some fields), reducers define how those updates merge with existing state.

**Core Concept:**
```rust
// Node returns partial update (only changes some fields)
node_output = AgentState { messages: new_messages, ..Default::default() };

// Reducer merges partial update with existing state
// Instead of replacing entire state, it intelligently merges fields
final_state = reducer.reduce(existing_state, node_output);
```

**Built-in Reducers:**

#### add_messages Reducer

The `add_messages` reducer implements upstream DashFlow's message list merging semantics:
- **Append** new messages to the list
- **Update** existing messages by ID (if IDs match)
- **Auto-assign** UUIDs to messages without IDs

```rust
use dashflow::reducer::{add_messages, MessageExt};
use dashflow::core::messages::Message;

// Example 1: Append new messages
let left = vec![Message::human("Hello")];
let right = vec![Message::ai("Hi there!")];
let merged = add_messages(left, right);
assert_eq!(merged.len(), 2);  // Both messages preserved

// Example 2: Update by ID
let msg1 = Message::human("Draft").with_id("msg1");
let msg1_updated = Message::human("Final version").with_id("msg1");
let merged = add_messages(vec![msg1], vec![msg1_updated]);
assert_eq!(merged.len(), 1);  // Updated in place
assert_eq!(merged[0].as_text(), "Final version");

// Example 3: Mixed operations (update + append)
let existing = vec![
    Message::human("Hello").with_id("id1"),
    Message::ai("Hi").with_id("id2"),
];
let updates = vec![
    Message::ai("Hi there!").with_id("id2"),  // Updates id2
    Message::human("How are you?"),            // Appends new message
];
let merged = add_messages(existing, updates);
assert_eq!(merged.len(), 3);  // 2 existing (1 updated) + 1 new
```

**Algorithm:**
1. **Assign UUIDs** to messages without IDs (ensures every message is trackable)
2. **Build index** of existing message IDs → position
3. **Merge updates**:
   - If ID exists: replace existing message (in-place update)
   - If ID doesn't exist: append to list (new message)

**Use Cases:**

1. **Multi-agent conversations**: Each agent adds messages, preserving conversation history
```rust
#[derive(GraphState, Clone)]
struct ConversationState {
    #[add_messages]  // Macro applies reducer
    messages: Vec<Message>,
}
```

2. **Message editing**: Update specific messages by ID (e.g., streaming LLM responses)
```rust
// Initial response
let msg = Message::ai("Thinking...").with_id("response");

// Stream updates (same ID = in-place update)
let updated1 = Message::ai("I think...").with_id("response");
let updated2 = Message::ai("I think the answer is 42").with_id("response");

let result = add_messages(vec![msg], vec![updated2]);
assert_eq!(result.len(), 1);  // Only one message, fully updated
```

3. **Checkpoint replay**: Reconstruct exact message list state from checkpoint history
```rust
// Checkpoint 1: Initial messages
let cp1_messages = vec![Message::human("Hello")];

// Checkpoint 2: Add AI response
let cp2_updates = vec![Message::ai("Hi!").with_id("ai1")];
let cp2_messages = add_messages(cp1_messages, cp2_updates);

// Checkpoint 3: Update AI response (same ID)
let cp3_updates = vec![Message::ai("Hi there!").with_id("ai1")];
let cp3_messages = add_messages(cp2_messages, cp3_updates);

assert_eq!(cp3_messages.len(), 2);  // Human + updated AI
```

**State Macro Integration:**

The `#[add_messages]` attribute macro automatically applies the reducer:

```rust
use dashflow::prelude::*;

#[derive(GraphState, Clone, Serialize, Deserialize)]
struct AgentState {
    #[add_messages]  // Automatically uses add_messages reducer
    messages: Vec<Message>,

    // Other fields use default replacement semantics
    user_id: String,
    context: HashMap<String, String>,
}

// When node returns partial state:
fn node_fn(state: AgentState) -> AgentState {
    AgentState {
        messages: vec![Message::ai("New message")],  // Uses add_messages reducer
        user_id: state.user_id,  // Other fields must be manually preserved
        context: state.context,
    }
}
```

**Custom Reducers:**

Implement the `Reducer<T>` trait for custom merge logic:

```rust
use dashflow::reducer::Reducer;

// Example: Merge HashMaps by key
struct MergeHashMap;

impl Reducer<HashMap<String, String>> for MergeHashMap {
    fn reduce(&self, mut left: HashMap<String, String>, right: HashMap<String, String>) -> HashMap<String, String> {
        left.extend(right);  // Add/overwrite keys from right
        left
    }
}

// Example: Append-only Vec (no deduplication)
struct AppendVec;

impl<T> Reducer<Vec<T>> for AppendVec {
    fn reduce(&self, mut left: Vec<T>, right: Vec<T>) -> Vec<T> {
        left.extend(right);
        left
    }
}
```

**MessageExt Helper Trait:**

Builder pattern for setting message IDs:

```rust
use dashflow::reducer::MessageExt;

let msg = Message::human("Hello")
    .with_id("custom-id-123");

assert_eq!(msg.fields().id.as_deref(), Some("custom-id-123"));
```

**upstream Python DashFlow Compatibility:**

The `add_messages` reducer matches upstream Python DashFlow behavior exactly:

| Feature | Python | Rust |
|---------|--------|------|
| Append new messages | ✓ | ✓ |
| Update by ID | ✓ | ✓ |
| Auto-assign UUIDs | ✓ | ✓ |
| Preserve message order | ✓ | ✓ |
| In-place updates | ✓ | ✓ |

**Performance Characteristics:**

- **Time complexity**: O(n + m) where n = existing messages, m = new messages
- **Space complexity**: O(n + m) for merged result
- **ID lookup**: O(1) via HashMap index
- **UUID generation**: ~100ns per message (uuid v4)

**Code Pointers:**
- Core module: `crates/dashflow/src/reducer.rs`
- Reducer trait: `crates/dashflow/src/reducer.rs:15-18`
- add_messages implementation: `crates/dashflow/src/reducer.rs:83-116`
- MessageExt trait: `crates/dashflow/src/reducer.rs:132-142`
- Example: `crates/dashflow/examples/state_reducers.rs`
- Tests: `crates/dashflow/src/reducer.rs:144-261` (comprehensive test suite)

### Subgraph (Nested Graph Composition)
**Location:** `crates/dashflow/src/subgraph.rs`

Subgraphs enable building complex workflows from reusable graph components. A subgraph is a complete graph (with its own state type) embedded as a node within a parent graph. State mapping functions convert between parent and child state types, allowing independent state evolution while maintaining data flow.

**Core Concept:**

Subgraphs solve the composition problem: how to build large workflows from smaller, reusable, independently testable graph components. Each subgraph can have its own state type optimized for its task, with explicit mapping functions defining data exchange with the parent graph.

```rust
// Parent graph has ProjectState (high-level project workflow)
// Child subgraph has ResearchState (focused research workflow)
// Mapping functions convert between parent ↔ child state types

parent_graph.add_subgraph_with_mapping(
    "research",
    research_subgraph,
    |parent: &ProjectState| ResearchState { /* parent → child */ },
    |parent: ProjectState, child: ResearchState| ProjectState { /* merge child result into parent */ }
);
```

**Why Subgraphs:**

1. **Modularity:** Reusable components (research subgraph used in multiple projects)
2. **State isolation:** Child state focused on specific task (research findings), parent state tracks overall project
3. **Independent testing:** Test subgraph separately from parent workflow
4. **Hierarchical composition:** Subgraphs can contain subgraphs (arbitrary nesting depth)
5. **Different state types:** Parent and child optimized for their respective tasks

**SubgraphNode Type:**

The `SubgraphNode<P, C>` type wraps a compiled child graph and implements the `Node<P>` trait, making it usable as a regular node in the parent graph.

```rust
pub struct SubgraphNode<P, C>
where
    P: GraphState,  // Parent state type
    C: GraphState,  // Child state type
{
    name: String,
    subgraph: Arc<CompiledGraph<C>>,
    map_to_child: Arc<dyn Fn(&P) -> C + Send + Sync>,
    map_from_child: Arc<dyn Fn(P, C) -> P + Send + Sync>,
}
```

**Type Parameters:**
- `P` - Parent graph state type
- `C` - Child (subgraph) state type

**Fields:**
- `name` - Subgraph identifier (for debugging, appears in node traces)
- `subgraph` - Compiled child graph (Arc for efficient sharing)
- `map_to_child` - Function to map parent state → child initial state
- `map_from_child` - Function to merge child final state back into parent state

**State Mapping Functions:**

Two mapping functions define data flow between parent and child:

**1. map_to_child: `Fn(&P) -> C`**

Converts parent state (reference) to child initial state. Called once before subgraph execution.

```rust
|parent: &ProjectState| ResearchState {
    query: parent.task_description.clone(),  // Extract query from parent
    findings: Vec::new(),                     // Start with empty findings
}
```

**Use cases:**
- Extract relevant fields from parent (parent.task → child.query)
- Initialize child state with defaults (empty lists, counters at zero)
- Transform data format (parent's String → child's structured Query type)

**2. map_from_child: `Fn(P, C) -> P`**

Merges child final state back into parent state. Called once after subgraph completion. Takes ownership of both parent and child states, returns updated parent.

```rust
|parent: ProjectState, child: ResearchState| ProjectState {
    research_results: child.findings,  // Copy child results into parent
    task_status: "research_complete".to_string(),  // Update parent fields
    ..parent  // Preserve other parent fields unchanged
}
```

**Use cases:**
- Copy child results into parent (child.findings → parent.research_results)
- Update parent status/flags based on child execution
- Transform child data format (child's structured results → parent's summary String)
- Preserve unrelated parent fields (spread operator `..parent`)

**Creating Subgraph Nodes:**

**Method 1: SubgraphNode::new (explicit)**

```rust
use dashflow::subgraph::SubgraphNode;

// Compile child graph first
let compiled_child = child_graph.compile()?;

// Create subgraph node
let subgraph_node = SubgraphNode::new(
    "research",
    compiled_child,
    |parent: &ProjectState| ResearchState { /* ... */ },
    |parent: ProjectState, child: ResearchState| ProjectState { /* ... */ }
);

// Add to parent graph manually
parent_graph.add_node("research", subgraph_node);
```

**Method 2: StateGraph::add_subgraph_with_mapping (convenience)**

```rust
// Compile and add in one step (recommended)
parent_graph.add_subgraph_with_mapping(
    "research",
    child_graph,  // Uncompiled graph (method compiles internally)
    |parent: &ProjectState| ResearchState { /* ... */ },
    |parent: ProjectState, child: ResearchState| ProjectState { /* ... */ }
)?;
```

**add_subgraph_with_mapping is preferred:** Compiles child graph, creates SubgraphNode, adds to parent graph as one atomic operation. Less boilerplate, clearer intent.

**Execution Flow:**

When a subgraph node executes:

1. **Map to child:** Call `map_to_child(&parent_state)` → produces `child_initial_state`
2. **Execute child graph:** Call `subgraph.invoke(child_initial_state)` → runs entire child graph
3. **Extract result:** Get `child_final_state` from child graph result
4. **Map from child:** Call `map_from_child(parent_state, child_final_state)` → produces `updated_parent_state`
5. **Return:** Updated parent state flows to next node in parent graph

**Important:** Parent state is cloned before passing to `map_from_child`, preserving original if mapping fails. Subgraph execution is atomic: either completes fully (returning updated parent state) or fails (returning error, parent state unchanged).

**Complete Example: Research + Analysis Workflow**

```rust
use dashflow::{StateGraph, END};
use serde::{Deserialize, Serialize};

// Parent state: high-level project tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProjectState {
    task_description: String,
    research_results: Option<Vec<String>>,
    analysis_report: Option<String>,
}

// Child state: focused research workflow
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    query: String,
    findings: Vec<String>,
}

// Build research subgraph
let mut research_graph = StateGraph::<ResearchState>::new();
research_graph.add_node_from_fn("search", |state| {
    Box::pin(async move {
        let mut state = state;
        state.findings.push(format!("Found data for: {}", state.query));
        Ok(state)
    })
});
research_graph.add_edge("search", END);
research_graph.set_entry_point("search");

// Build parent graph with subgraph
let mut main_graph = StateGraph::<ProjectState>::new();

main_graph.add_subgraph_with_mapping(
    "research",
    research_graph,
    // Map parent → child
    |parent: &ProjectState| ResearchState {
        query: parent.task_description.clone(),
        findings: Vec::new(),
    },
    // Map child → parent
    |parent: ProjectState, child: ResearchState| ProjectState {
        research_results: Some(child.findings),
        ..parent
    }
)?;

main_graph.add_edge("research", END);
main_graph.set_entry_point("research");

let compiled = main_graph.compile()?;

let result = compiled.invoke(ProjectState {
    task_description: "market trends".to_string(),
    research_results: None,
    analysis_report: None,
}).await?;

// Result: research_results populated from child subgraph
assert!(result.final_state.research_results.is_some());
```

**Use Cases:**

**1. Multi-Stage Workflows (Sequential Subgraphs)**

Parent graph orchestrates multiple specialized subgraphs in sequence:

```rust
parent_graph
    .add_subgraph_with_mapping("research", research_graph, /* ... */)?
    .add_subgraph_with_mapping("analysis", analysis_graph, /* ... */)?
    .add_subgraph_with_mapping("reporting", reporting_graph, /* ... */)?
    .add_edge("research", "analysis")
    .add_edge("analysis", "reporting")
    .add_edge("reporting", END)
    .set_entry_point("research");

// Executes: research → analysis → reporting
// Each subgraph has its own state type optimized for its task
```

**2. Parallel Subgraphs (Fan-out/Fan-in)**

Execute multiple subgraphs concurrently, merge results:

```rust
parent_graph
    .add_subgraph_with_mapping("research_a", research_graph_a, /* ... */)?
    .add_subgraph_with_mapping("research_b", research_graph_b, /* ... */)?
    .add_node_from_fn("merge", |state| { /* merge results */ })
    .add_parallel_edges("research_a", vec!["research_b"])
    .add_edge("research_b", "merge")
    .add_edge("merge", END)
    .set_entry_point("research_a");

// research_a and research_b execute concurrently
// merge combines results from both subgraphs
```

**3. Nested Subgraphs (Hierarchical Composition)**

Subgraphs can contain subgraphs, enabling arbitrary nesting depth:

```rust
// Level 3 (innermost): data processing
let mut process_graph = StateGraph::<ProcessState>::new();
// ... (add nodes for data processing)

// Level 2: analysis (contains process subgraph)
let mut analysis_graph = StateGraph::<AnalysisState>::new();
analysis_graph.add_subgraph_with_mapping("process", process_graph, /* ... */)?;

// Level 1: main workflow (contains analysis subgraph)
let mut main_graph = StateGraph::<MainState>::new();
main_graph.add_subgraph_with_mapping("analysis", analysis_graph, /* ... */)?;

// Execution: main → analysis → process (3 levels deep)
```

**4. Reusable Subgraphs (DRY Principle)**

Define a subgraph once, use in multiple parent graphs:

```rust
// Define reusable validation subgraph
let validation_graph = build_validation_subgraph();

// Use in multiple workflows
customer_workflow.add_subgraph_with_mapping("validate", validation_graph.clone(), /* ... */)?;
product_workflow.add_subgraph_with_mapping("validate", validation_graph.clone(), /* ... */)?;
order_workflow.add_subgraph_with_mapping("validate", validation_graph.clone(), /* ... */)?;
```

Note: Subgraphs are wrapped in Arc internally, so cloning is cheap (pointer copy, not deep clone).

**5. Conditional Subgraph Execution**

Use conditional edges to route to different subgraphs based on state:

```rust
use std::collections::HashMap;

parent_graph
    .add_node_from_fn("router", |state| Box::pin(async move { Ok(state) }))
    .add_subgraph_with_mapping("simple_path", simple_subgraph, /* ... */)?
    .add_subgraph_with_mapping("complex_path", complex_subgraph, /* ... */)?;

let mut routes = HashMap::new();
routes.insert("simple".to_string(), "simple_path".to_string());
routes.insert("complex".to_string(), "complex_path".to_string());

parent_graph.add_conditional_edges(
    "router",
    |state: &ParentState| {
        if state.complexity < 5 { "simple".to_string() }
        else { "complex".to_string() }
    },
    routes
);

// Routes to different subgraphs based on state.complexity
```

**State Isolation and Data Flow:**

**Isolation:** Child state is completely independent from parent state. Changes to child state during subgraph execution do NOT affect parent state until `map_from_child` explicitly merges results.

```rust
// Parent state before subgraph
let parent = ParentState { counter: 10, result: None };

// Subgraph executes (child state starts at 0, ends at 100)
// Parent state remains unchanged during child execution

// After subgraph, map_from_child merges child result
let updated_parent = map_from_child(parent, child_final);
// Only now does parent.counter update to 100
```

**Data flow:**
1. Parent state → map_to_child → child initial state (extraction/transformation)
2. Child initial → child graph execution → child final (independent evolution)
3. Parent state + child final → map_from_child → updated parent (merge/integration)

**No shared references:** Parent and child states are separate memory. Child cannot accidentally modify parent fields. All data exchange explicit through mapping functions.

**Error Handling:**

Errors from subgraph execution propagate to parent graph:

```rust
// If subgraph node fails:
let result = parent_graph.invoke(initial_state).await;

// Result is Err(Error::NodeExecution { node: "subgraph_name", ... })
// Parent graph execution stops, no partial state update
```

Subgraph errors behave like regular node errors: execution halts, error returned to caller, state unchanged (transaction semantics).

**Mapping function errors:** If `map_to_child` or `map_from_child` panics, parent graph execution fails (panic propagates). Use `Result` types in mapping functions if fallible transformations needed.

**Performance Characteristics:**

**Subgraph overhead:**
- Mapping functions: ~100-500ns per call (2 calls per subgraph execution)
- Arc<CompiledGraph> clone: ~10ns (pointer copy, shared ownership)
- State cloning for map_from_child: ~100ns-10μs (depends on state size)

**Nesting overhead:** Subgraphs can nest arbitrarily deep with minimal overhead (each level adds ~1μs for mapping functions). Tested with 4-level nesting (main → sub1 → sub2 → sub3) with no performance degradation.

**Execution time:** Dominated by child graph execution, not subgraph infrastructure. Mapping overhead negligible (<0.1%) compared to typical node execution (LLM calls, I/O operations).

**Memory:** Arc-based sharing means multiple parent graphs can reference the same child subgraph with minimal memory cost (compiled graph shared, not duplicated).

**Common Patterns:**

**Pattern 1: Identity Mapping (Same State Type)**

Unusual but valid: parent and child have same state type.

```rust
// Both parent and child use AgentState
parent_graph.add_subgraph_with_mapping(
    "subworkflow",
    child_graph,
    |parent: &AgentState| parent.clone(),  // Identity mapping
    |_parent: AgentState, child: AgentState| child  // Use child result directly
)?;
```

Use case: Logical grouping (subgraph represents a reusable workflow stage) without needing different state types.

**Pattern 2: Extract-Transform-Load (ETL)**

Child subgraph performs ETL on a subset of parent data:

```rust
parent_graph.add_subgraph_with_mapping(
    "etl",
    etl_subgraph,
    // Extract: Pull raw data from parent
    |parent: &ParentState| ETLState {
        raw_data: parent.raw_input.clone(),
        processed_data: Vec::new(),
    },
    // Load: Store processed data back to parent
    |parent: ParentState, child: ETLState| ParentState {
        processed_output: child.processed_data,
        ..parent
    }
)?;
```

**Pattern 3: Accumulator Pattern**

Child subgraph runs multiple times, accumulating results in parent:

```rust
parent_graph.add_node_from_fn("loop", |state| { /* ... */ });
parent_graph.add_subgraph_with_mapping(
    "process_item",
    process_subgraph,
    |parent: &ParentState| ItemState {
        item: parent.current_item.clone(),
    },
    |parent: ParentState, child: ItemState| ParentState {
        results: {
            let mut results = parent.results;
            results.push(child.item);  // Accumulate results
            results
        },
        ..parent
    }
)?;

// Add loop: loop → process_item → (continue or exit)
```

**Pattern 4: Fan-out with State Cloning**

Parallel edges clone parent state, each subgraph processes independent copy:

```rust
parent_graph
    .add_subgraph_with_mapping("branch_a", subgraph_a, /* ... */)?
    .add_subgraph_with_mapping("branch_b", subgraph_b, /* ... */)?
    .add_parallel_edges("branch_a", vec!["branch_b".to_string()]);

// branch_a and branch_b receive cloned parent state
// Each processes independently, results merged via map_from_child
```

**upstream Python DashFlow Compatibility:**

Rust subgraphs match upstream Python DashFlow subgraph semantics:

| Feature | upstream Python DashFlow | DashFlow |
|---------|-----------------|----------------|
| Nested graphs | `add_node(subgraph.compile())` | `add_subgraph_with_mapping()` |
| State mapping | Implicit (duck typing) | Explicit (typed functions) |
| State isolation | ✓ (child state separate) | ✓ (child state separate) |
| Error propagation | ✓ (bubbles up) | ✓ (bubbles up) |
| Arbitrary nesting | ✓ (unlimited depth) | ✓ (unlimited depth) |
| Reusable subgraphs | ✓ (same graph, multiple parents) | ✓ (Arc sharing) |

**Key difference:** Rust requires explicit state mapping functions (compile-time type safety), Python uses implicit duck typing (runtime checks). Rust approach catches state mismatches at compile time, preventing runtime errors.

**Best Practices:**

1. **Minimize state mapping overhead:** Keep mapping functions simple (field copies, basic transformations). Avoid expensive operations (network calls, heavy computation) in mapping functions. They're called twice per subgraph execution.

2. **Use subgraphs for logical boundaries:** Good: "research", "analysis", "reporting" subgraphs (clear conceptual boundaries). Avoid: overly granular subgraphs (single node per subgraph defeats composition benefits).

3. **Design child state types for task:** Child state should contain only fields needed for subgraph logic. Don't include parent fields unless subgraph needs them. Smaller child state = faster cloning, clearer intent.

4. **Test subgraphs independently:** Write tests for child graphs before integrating into parent. Subgraphs are independent units, should be testable in isolation.

5. **Document state mapping:** Complex transformations in mapping functions should have comments explaining parent → child data flow and child → parent integration logic.

6. **Use descriptive subgraph names:** Subgraph names appear in node execution traces. Use clear, descriptive names ("customer_validation", not "sg1").

7. **Avoid deep nesting without justification:** Nesting is powerful but adds conceptual complexity. 2-3 levels typical, >4 levels rare. Flatten if possible.

8. **Preserve unrelated parent fields:** In `map_from_child`, use spread operator `..parent` to preserve fields not touched by child. Prevents accidental data loss.

**Common Pitfalls:**

**Pitfall 1: Forgetting to preserve parent fields**

```rust
// BAD: Overwrites all parent fields
|_parent: ParentState, child: ChildState| ParentState {
    results: child.data,  // Only sets results
    // All other parent fields reset to default!
}

// GOOD: Preserves other parent fields
|parent: ParentState, child: ChildState| ParentState {
    results: child.data,
    ..parent  // Keeps task, metadata, etc.
}
```

Fix: Always use `..parent` spread unless explicitly resetting parent state.

**Pitfall 2: Expensive operations in mapping functions**

```rust
// BAD: Heavy computation in mapping function
|parent: &ParentState| ChildState {
    data: expensive_transformation(parent.raw_data),  // Blocks execution
}

// GOOD: Move expensive work into child graph nodes
|parent: &ParentState| ChildState {
    raw_data: parent.raw_data.clone(),  // Fast copy
}
// Child graph has "transform" node that does expensive work asynchronously
```

Fix: Mapping functions should be fast field copies/basic transformations. Complex logic belongs in graph nodes (async, parallelizable).

**Pitfall 3: Circular subgraph references**

```rust
// BAD: graph_a contains graph_b, graph_b contains graph_a
graph_a.add_subgraph_with_mapping("b", graph_b, /* ... */)?;
graph_b.add_subgraph_with_mapping("a", graph_a, /* ... */)?;  // CYCLE!

// Compilation error: "graph_a moved" or infinite loop at runtime
```

Fix: Design acyclic composition hierarchy. Use shared state patterns (state carries "next subgraph" field) instead of circular nesting.

**Pitfall 4: Overly complex state mapping**

```rust
// BAD: Mapping function with complex business logic
|parent: &ParentState| ChildState {
    data: if parent.mode == "production" {
        transform_prod(parent.data)
    } else if parent.mode == "staging" {
        transform_staging(parent.data)
    } else {
        transform_dev(parent.data)
    },
    // 50 more lines...
}

// GOOD: Simple mapping, complex logic in child graph
|parent: &ParentState| ChildState {
    mode: parent.mode.clone(),
    data: parent.data.clone(),
}
// Child graph has "transform" node that handles mode-specific logic
```

Fix: Mapping functions should be ~5-10 lines max. Complex transformations belong in child graph nodes (testable, composable, debuggable).

**Code Pointers:**

- Core module: `crates/dashflow/src/subgraph.rs`
- SubgraphNode type: `crates/dashflow/src/subgraph.rs:58-72`
- SubgraphNode::new constructor: `crates/dashflow/src/subgraph.rs:98-116`
- Node trait implementation: `crates/dashflow/src/subgraph.rs:119-148`
- SubgraphNode::execute (execution flow): `crates/dashflow/src/subgraph.rs:124-135`
- StateGraph::add_subgraph_with_mapping: `crates/dashflow/src/graph/mod.rs:504-534`
- Tests: `crates/dashflow/src/subgraph.rs:191-1488` (22 comprehensive tests)
  - Basic execution: `test_subgraph_basic_execution` (line 192)
  - State isolation: `test_subgraph_state_isolation` (line 240)
  - Nested subgraphs (3 levels): `test_nested_subgraphs` (line 568)
  - Parallel subgraphs: `test_parallel_subgraphs` (line 649)
  - Conditional edges in subgraph: `test_subgraph_with_conditional_edges` (line 884)
  - Loop within subgraph: `test_subgraph_with_loop` (line 967)
  - Deep nesting (4 levels): `test_subgraph_deep_nesting_4_levels` (line 1345)
  - Error propagation: `test_subgraph_error_propagation` (line 524)
  - Multiple subgraphs: `test_multiple_subgraphs` (line 419)
  - Same state type: `test_subgraph_with_same_state_type` (line 1449)

### Checkpointing
**Location:** `crates/dashflow/src/checkpoint/`

State persistence and resumption.

```rust
use dashflow::checkpoint::{MemoryCheckpointer, FileCheckpointer};
use dashflow_postgres_checkpointer::PostgresCheckpointer;
use dashflow_redis_checkpointer::RedisCheckpointer;
use dashflow_s3_checkpointer::S3Checkpointer;
use dashflow_dynamodb_checkpointer::DynamoDbCheckpointer;

// Memory-based (fast, ephemeral)
let checkpointer = MemoryCheckpointer::new();

// File-based (persistent, single process)
let checkpointer = FileCheckpointer::new("./checkpoints")?;

// Redis-based (fast, distributed cache)
let checkpointer = RedisCheckpointer::new("redis://localhost:6379").await?;

// PostgreSQL-based (production, durable, ACID)
let checkpointer = PostgresCheckpointer::connect(
    "host=localhost user=postgres password=secret dbname=dashflow"
).await?;
checkpointer.init_schema().await?;

// S3-based (serverless, durable, low cost)
let checkpointer = S3Checkpointer::new(
    "my-checkpoint-bucket",
    "us-east-1"
).await?;

// DynamoDB-based (serverless, fast, scalable)
let checkpointer = DynamoDbCheckpointer::new(
    "dashflow-checkpoints",
    "us-east-1"
).await?;

let app = graph.compile()?
    .with_checkpointer(checkpointer)
    .with_thread_id("session-123");
```

**Checkpoint Types:**
- `MemoryCheckpointer` - In-memory, fast (ephemeral, single process)
- `FileCheckpointer` - JSON files, persistent (single process, local development)
- `RedisCheckpointer` - Redis cache, fast distributed (high performance, TTL support)
- `PostgresCheckpointer` - PostgreSQL database (ACID, complex queries, strong consistency)
- `S3Checkpointer` - S3 object storage (serverless, durable, cost-effective)
- `DynamoDbCheckpointer` - DynamoDB NoSQL (serverless, fast, auto-scaling)

**Redis Features:**
- Sub-millisecond checkpoint access (5-10x faster than PostgreSQL)
- Automatic expiration with TTL (time-to-live)
- Compression support (Zstd, Snappy) for large states
- Atomic operations with pipelining
- Retention policies (keep last N, time-based, size-based)
- Key prefix isolation for multi-tenant deployments

**PostgreSQL Features:**
- Distributed execution support (multiple processes)
- ACID guarantees for state persistence
- Checkpoint history with parent_id tracking
- JSONB metadata for custom indexing
- Optimized queries with composite indexes
- Thread-safe concurrent access

**S3 Features:**
- Serverless architecture (no infrastructure management)
- 99.999999999% durability (11 nines)
- Lifecycle policies for automatic cleanup
- Compression support (Gzip, Zstd)
- Cross-region replication for disaster recovery
- Cost-effective for infrequent access ($0.023/GB/month)

**DynamoDB Features:**
- Serverless NoSQL with auto-scaling
- Single-digit millisecond latency
- Global tables for multi-region deployments
- Point-in-time recovery (PITR)
- On-demand capacity mode (pay per request)
- Time-to-live (TTL) for automatic cleanup

**Code Pointers:**
- Core trait: `crates/dashflow/src/checkpoint.rs` (main module with Checkpointer trait)
- Memory: `crates/dashflow/src/checkpoint.rs:849` (MemoryCheckpointer)
- SQLite: `crates/dashflow/src/checkpoint/sqlite.rs`
- Redis: `crates/dashflow-redis-checkpointer/src/lib.rs`
- PostgreSQL: `crates/dashflow-postgres-checkpointer/src/lib.rs`
- S3: `crates/dashflow-s3-checkpointer/src/lib.rs`
- DynamoDB: `crates/dashflow-dynamodb-checkpointer/src/lib.rs`
- Example: `crates/dashflow/examples/postgres_checkpointing.rs`

### Retention Policies
**Location:** `crates/dashflow/src/retention.rs`

Automatic checkpoint cleanup policies to manage storage costs and prevent unbounded growth. Retention policies define time-based and count-based rules for keeping or deleting old checkpoints.

**Priority Order (checkpoints kept if matching ANY rule):**
1. **Keep last N** - Always keep the most recent N checkpoints (regardless of age)
2. **Keep daily** - Keep one checkpoint per day within time window
3. **Keep weekly** - Keep one checkpoint per week within time window
4. **Delete after** - Delete all checkpoints older than max age

```rust
use dashflow::retention::RetentionPolicy;
use dashflow_redis_checkpointer::RedisCheckpointer;
use std::time::Duration;

// Define retention policy
let policy = RetentionPolicy::builder()
    .keep_last_n(10)  // Always keep last 10 checkpoints
    .keep_daily_for(Duration::from_secs(30 * 86400))  // Keep one per day for 30 days
    .keep_weekly_for(Duration::from_secs(12 * 7 * 86400))  // Keep one per week for 12 weeks
    .delete_after(Duration::from_secs(90 * 86400))  // Delete everything older than 90 days
    .build();

// Apply to checkpointer
let checkpointer = RedisCheckpointer::new("redis://localhost")
    .await?
    .with_retention_policy(policy);

// Manual application (explicit cleanup)
let deleted_count = checkpointer.apply_retention("thread-123").await?;
println!("Deleted {} old checkpoints", deleted_count);

// Automatic application (run periodically with scheduler)
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Every hour
    loop {
        interval.tick().await;
        if let Err(e) = checkpointer.apply_retention("thread-123").await {
            eprintln!("Retention policy failed: {}", e);
        }
    }
});
```

**Policy Rules:**

- **`keep_last_n(n)`** - Always keep the most recent N checkpoints
  - Priority: Highest (never deleted by other rules)
  - Use case: Ensure recent history always available for debugging
  - Example: `keep_last_n(10)` keeps last 10 checkpoints even if older than max age

- **`keep_daily_for(duration)`** - Keep one checkpoint per day within time window
  - Selects first checkpoint of each calendar day
  - Use case: Long-term history with daily granularity
  - Example: `keep_daily_for(Duration::from_secs(30 * 86400))` keeps one per day for 30 days

- **`keep_weekly_for(duration)`** - Keep one checkpoint per week within time window
  - Selects first checkpoint of each ISO week (starting Monday)
  - Use case: Very long-term history with coarse granularity
  - Example: `keep_weekly_for(Duration::from_secs(12 * 7 * 86400))` keeps one per week for 12 weeks

- **`delete_after(duration)`** - Hard cutoff for checkpoint age
  - Deletes all checkpoints older than this (unless kept by other rules)
  - Use case: Enforce compliance requirements (e.g., GDPR data retention limits)
  - Example: `delete_after(Duration::from_secs(90 * 86400))` deletes everything older than 90 days

**Example Policies:**

```rust
// Development: Keep last 5, delete after 7 days
let dev_policy = RetentionPolicy::builder()
    .keep_last_n(5)
    .delete_after(Duration::from_secs(7 * 86400))
    .build();

// Production: Tiered retention (recent → daily → weekly)
let prod_policy = RetentionPolicy::builder()
    .keep_last_n(20)  // Keep last 20 (most recent operations)
    .keep_daily_for(Duration::from_secs(30 * 86400))  // Daily for 1 month
    .keep_weekly_for(Duration::from_secs(26 * 7 * 86400))  // Weekly for 6 months
    .delete_after(Duration::from_secs(180 * 86400))  // Hard cutoff at 180 days
    .build();

// Compliance: Aggressive deletion (GDPR 30-day limit)
let gdpr_policy = RetentionPolicy::builder()
    .keep_last_n(3)  // Minimal recent history
    .delete_after(Duration::from_secs(30 * 86400))  // Delete after 30 days
    .build();

// Archival: Keep everything with granular history
let archive_policy = RetentionPolicy::builder()
    .keep_last_n(100)  // Keep last 100 checkpoints
    .keep_daily_for(Duration::from_secs(365 * 86400))  // Daily for 1 year
    .keep_weekly_for(Duration::from_secs(5 * 365 * 86400))  // Weekly for 5 years
    .build();  // No delete_after (keep everything)
```

**Integration with Checkpointers:**

| Checkpointer | Retention Support | Implementation |
|--------------|------------------|----------------|
| MemoryCheckpointer | No | Ephemeral (no persistence) |
| FileCheckpointer | No | Manual file system cleanup |
| RedisCheckpointer | Yes | `apply_retention(thread_id)` method |
| PostgresCheckpointer | Yes | `apply_retention(thread_id)` method |
| S3Checkpointer | Partial | Use S3 lifecycle policies instead |
| DynamoDbCheckpointer | Partial | Use DynamoDB TTL instead |

**Storage Cost Optimization:**

```rust
// Calculate retention impact
let policy = RetentionPolicy::builder()
    .keep_last_n(10)
    .keep_daily_for(Duration::from_secs(30 * 86400))
    .build();

// Before: 1000 checkpoints @ 1MB each = 1GB storage
// After (30 days): ~10 recent + 30 daily = 40 checkpoints = 40MB storage
// Savings: 96% storage reduction
```

**Scheduling Cleanup:**

```rust
use tokio::time::{interval, Duration};

// Option 1: Fixed interval (simple)
async fn schedule_retention_fixed(checkpointer: RedisCheckpointer, thread_id: String) {
    let mut interval = interval(Duration::from_secs(3600)); // Every hour
    loop {
        interval.tick().await;
        let _ = checkpointer.apply_retention(&thread_id).await;
    }
}

// Option 2: After each checkpoint save (aggressive)
async fn schedule_retention_immediate(checkpointer: RedisCheckpointer, thread_id: String) {
    // After saving checkpoint
    checkpointer.save_checkpoint(&checkpoint).await?;
    checkpointer.apply_retention(&thread_id).await?;
}

// Option 3: Daily cleanup (batch)
async fn schedule_retention_daily(checkpointer: RedisCheckpointer, threads: Vec<String>) {
    let mut interval = interval(Duration::from_secs(86400)); // Every 24 hours
    loop {
        interval.tick().await;
        for thread_id in &threads {
            let _ = checkpointer.apply_retention(thread_id).await;
        }
    }
}
```

**Best Practices:**

1. **Start conservative**: Use `keep_last_n` to ensure recent history always available
2. **Tiered retention**: Recent (all) → medium-term (daily) → long-term (weekly)
3. **Compliance first**: Set `delete_after` to meet regulatory requirements (GDPR, HIPAA)
4. **Monitor storage**: Track checkpoint count and size before/after retention
5. **Test policies**: Apply to test threads first, verify expected deletions
6. **Automate cleanup**: Run retention periodically (hourly or daily) via scheduler
7. **Per-thread policies**: Different threads may need different retention rules

**Code Pointers:**
- Core module: `crates/dashflow/src/retention.rs`
- Redis integration: `crates/dashflow-redis-checkpointer/src/lib.rs:164-197`
- PostgreSQL integration: `crates/dashflow-postgres-checkpointer/src/lib.rs:139-180`
- Tests: `crates/dashflow/src/retention.rs:220-691` (comprehensive test suite)

### Prebuilt Agent Patterns
**Location:** `crates/dashflow/src/prebuilt.rs`

Pre-configured graph templates for common agent workflows. These convenience functions construct complete StateGraph instances with standard patterns, reducing boilerplate and ensuring best practices.

**Available Patterns:**

#### ReAct Agent (Reasoning and Acting)

The `create_react_agent()` function creates the Rust equivalent of Python's `dashflow.prebuilt.create_react_agent`. It builds a standard tool-using agent that:
1. Calls LLM with bound tools
2. Executes tool calls if requested
3. Returns tool results to LLM
4. Repeats until final answer (no tool calls)

```rust
use dashflow::prebuilt::{create_react_agent, AgentState};
use dashflow_openai::ChatOpenAI;
use dashflow::core::language_models::ChatModelToolBindingExt;
use dashflow::core::messages::Message;
use std::sync::Arc;

// Create model with bound tools
let model = ChatOpenAI::new()
    .bind_tools(vec![Arc::new(search_tool), Arc::new(calculator_tool)], None);

// Create ReAct agent graph (pre-configured workflow)
let agent = create_react_agent(model, vec![
    Arc::new(search_tool),
    Arc::new(calculator_tool),
])?;

// Run agent with human message
let initial_state = AgentState::with_human_message("Search for Rust async patterns");
let result = agent.invoke(initial_state).await?;

// Access conversation history
for message in result.messages {
    println!("{}", message.as_text());
}
```

**AgentState Structure:**

The `AgentState` type is the standard message-based state for ReAct agents:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentState {
    pub messages: Vec<Message>,  // Conversation history (Human, AI, Tool messages)
}

// Convenience constructors
let state1 = AgentState::new(Message::human("Hello"));
let state2 = AgentState::with_human_message("Hello");  // Shorthand
```

**Graph Structure:**

The ReAct agent creates this graph internally:

```
START -> agent -> [conditional]
                    ├─> tools -> agent (if tool_calls present)
                    └─> END (if no tool_calls)
```

**Nodes:**
- **agent**: Calls LLM with message history, appends AI response to state
- **tools**: Executes tool calls from last AI message, appends tool responses

**Edges:**
- **Entry point**: `agent` (starts workflow)
- **Conditional routing**: After `agent`, routes to `tools` if AI message has tool_calls, otherwise END
- **Feedback loop**: `tools` → `agent` (returns tool results to LLM for next iteration)

**upstream Python DashFlow Compatibility:**

```python
# Python equivalent
from dashflow.prebuilt import create_react_agent
from dashflow_openai import ChatOpenAI

model = ChatOpenAI()
agent = create_react_agent(model, tools)
result = agent.invoke({"messages": [("human", "query")]})
```

**Use Cases:**

1. **Tool-using chatbots**: Search, calculator, API calls
2. **Multi-step problem solving**: Breaking down complex queries
3. **Information retrieval**: Search → summarize → answer
4. **Data processing**: Extract → transform → analyze

**When to Use Prebuilt vs Custom Graphs:**

**Use `create_react_agent()` when:**
- Standard tool-using agent pattern fits your needs
- Single agent with tools (no multi-agent collaboration)
- Message-based state is sufficient
- Fast prototyping and iteration

**Use Custom StateGraph when:**
- Multiple agents with different roles
- Custom state beyond messages (context, metadata, etc.)
- Complex routing logic (parallel branches, conditional paths)
- Custom node implementations (specialized processing)

**Example: Custom State Extension**

You can wrap the prebuilt agent in a custom graph with richer state:

```rust
#[derive(Clone, GraphState)]
struct CustomState {
    #[add_messages]
    messages: Vec<Message>,
    user_id: String,
    context: HashMap<String, String>,
}

// Create prebuilt agent
let react_agent = create_react_agent(model, tools)?;

// Wrap in custom graph
let mut graph: StateGraph<CustomState> = StateGraph::new();

graph.add_node_from_fn("react", move |state: CustomState| {
    let react_agent = react_agent.clone();
    Box::pin(async move {
        // Extract messages for react agent
        let agent_state = AgentState { messages: state.messages };

        // Run react agent
        let result = react_agent.invoke(agent_state).await?;

        // Merge back with custom state
        Ok(CustomState {
            messages: result.messages,
            user_id: state.user_id,
            context: state.context,
        })
    })
});
```

**Performance:**

The prebuilt ReAct agent has minimal overhead:
- **Compilation**: <1ms (graph construction is fast)
- **Per-iteration**: ~50-100ms overhead vs raw LLM call (routing, state management)
- **Tool execution**: Depends on tool latency (network, compute)

**Code Pointers:**
- Core module: `crates/dashflow/src/prebuilt.rs`
- create_react_agent: `crates/dashflow/src/prebuilt.rs:154-335`
- AgentState: `crates/dashflow/src/prebuilt.rs:64-89`
- Integration helpers: `crates/dashflow/src/integration.rs` (tools_condition, auto_tool_executor)
- Example: `crates/dashflow-standard-tests/tests/complete_eval_loop.rs` (multi-turn with DashStream)
- Tests: `crates/dashflow/src/prebuilt.rs:337-1119` (comprehensive test suite)

### DashFlow Component Integration
**Location:** `crates/dashflow/src/integration.rs`

Adapters to use DashFlow Agents, Chains, and Tools as DashFlow nodes. These wrappers bridge the Runnable abstraction (LCEL) with the graph Node abstraction.

**Core Problem:**

DashFlow uses the `Runnable` trait for composable components (chains, agents). DashFlow uses the `Node` trait for graph vertices. Integration adapters convert Runnables to Nodes, enabling seamless composition.

**Adapter Types:**

#### RunnableNode (Generic Adapter)

Wraps any `Runnable<S, S>` (where input/output types match) as a graph node:

```rust
use dashflow::integration::RunnableNode;
use dashflow::core::chains::Chain;
use dashflow::core::runnable::Runnable;

// Create a chain (Runnable)
let summarize_chain = prompt.pipe(llm).pipe(output_parser);

// Wrap as graph node
let node = RunnableNode::new("summarizer", summarize_chain)
    .with_config(RunnableConfig::default().with_tags(vec!["summarization"]));

// Use in graph
let mut graph: StateGraph<MyState> = StateGraph::new();
graph.add_node("summarize", node);
```

**When to Use:**
- State type matches Runnable's input/output exactly
- No need to transform state before/after Runnable execution
- Simple pass-through nodes

#### AgentNode (State Transformation Adapter)

Wraps agents that work with different state types than the graph state. Provides extract/update functions to adapt state:

```rust
use dashflow::integration::AgentNode;
use dashflow::core::agents::Agent;

#[derive(Clone)]
struct MyState {
    messages: Vec<Message>,
    user_id: String,
    context: HashMap<String, String>,
}

let agent = Agent::new(/* ... */);

// Create node with state adapters
let agent_node = AgentNode::new(
    "assistant",
    agent,
    // Extract agent input from graph state
    |state: MyState| state.messages,
    // Update graph state with agent output
    |state, agent_output| MyState {
        messages: agent_output,  // Update messages from agent
        user_id: state.user_id,  // Preserve user_id
        context: state.context,  // Preserve context
    }
);

graph.add_node("assistant", agent_node);
```

**When to Use:**
- Agent input/output types differ from graph state
- Need to preserve additional state fields (user_id, context)
- Multi-step state transformation (extract → process → merge)

#### ToolNode (Tool Execution Adapter)

Wraps individual tools as graph nodes. Useful for graphs where tools are first-class nodes (not auto-executed):

```rust
use dashflow::integration::ToolNode;
use dashflow::core::tools::{Tool, ToolInput};

#[derive(Clone)]
struct MyState {
    query: String,
    search_result: String,
}

let search_tool = /* create tool */;

// Create tool node with state adapters
let tool_node = ToolNode::new(
    search_tool,
    // Extract tool input from state
    |state: MyState| ToolInput::String(state.query),
    // Update state with tool output
    |mut state, output| {
        state.search_result = output;
        state
    }
);

graph.add_node("search", tool_node);
```

**When to Use:**
- Tools as explicit graph nodes (not auto-executed in agent loop)
- Custom tool routing logic (different tools on different paths)
- Tool results need custom state merging

**Helper Functions:**

#### tools_condition

Conditional routing function to check if AI message has tool calls:

```rust
use dashflow::integration::tools_condition;
use std::collections::HashMap;

// In graph setup:
let mut routes = HashMap::new();
routes.insert("tools".to_string(), "tools".to_string());
routes.insert(END.to_string(), END.to_string());

graph.add_conditional_edges(
    "assistant",
    |state: &AgentState| tools_condition(&state.messages).to_string(),
    routes,
);
```

**Returns:**
- `"tools"` if last AI message has tool_calls
- `END` if no tool_calls (agent is done)

**Algorithm:**
1. Check if last message is AI message
2. Check if AI message has non-empty tool_calls list
3. Return "tools" or END accordingly

#### auto_tool_executor

Executes tool calls from the last AI message automatically:

```rust
use dashflow::integration::auto_tool_executor;
use dashflow::core::messages::Message;
use std::sync::Arc;

// Get tool messages from executing tool calls
let messages = vec![
    Message::human("Search for Rust async"),
    Message::ai_with_tool_calls(vec![
        ToolCall { id: "1", name: "search", input: "Rust async" }
    ]),
];

let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(search_tool)];

// Execute tool calls from last AI message
let tool_messages = auto_tool_executor(&messages, &tools).await?;

// tool_messages contains ToolMessage responses for each tool call
```

**Returns:** Vector of `ToolMessage` responses (one per tool call)

**Use Cases:**

1. **Agent-in-Graph**: Wrap DashFlow agents in DashFlow workflows
```rust
// DashFlow Agent -> DashFlow Node
let agent = Agent::new(model, tools);
let agent_node = RunnableNode::new("agent", agent);
graph.add_node("agent", agent_node);
```

2. **Chain-in-Graph**: Use LCEL chains as graph nodes
```rust
// LCEL Chain -> DashFlow Node
let chain = prompt.pipe(llm).pipe(parser);
let chain_node = RunnableNode::new("process", chain);
graph.add_node("process", chain_node);
```

3. **Tool-First Graphs**: Explicit tool routing (not auto-executed)
```rust
// Tools as first-class nodes
let search_node = ToolNode::new(search_tool, /* ... */);
let calc_node = ToolNode::new(calculator_tool, /* ... */);

graph.add_node("search", search_node);
graph.add_node("calculate", calc_node);
graph.add_conditional_edges("router", route_fn, routes);
```

4. **Hybrid Workflows**: Mix DashFlow components with custom nodes
```rust
// Combine DashFlow agent with custom nodes
graph.add_node("agent", RunnableNode::new("agent", agent));
graph.add_node_from_fn("custom_process", custom_fn);
graph.add_edge("agent", "custom_process");
```

**upstream Python DashFlow Compatibility:**

These adapters implement the same integration patterns as upstream Python DashFlow:
- RunnableNode ≈ Python's RunnablePassthrough
- AgentNode ≈ Python's custom node with state extraction
- ToolNode ≈ Python's ToolNode
- tools_condition ≈ Python's tools_condition function

**Performance:**

Integration adapters have minimal overhead:
- **RunnableNode**: ~10-20μs (thin wrapper)
- **AgentNode**: ~50-100μs (extract + invoke + merge)
- **ToolNode**: ~30-50μs per tool (invoke + state update)
- **tools_condition**: ~1-2μs (message inspection)
- **auto_tool_executor**: ~50μs + tool latency

**Code Pointers:**
- Core module: `crates/dashflow/src/integration.rs`
- RunnableNode: `crates/dashflow/src/integration.rs:53-121`
- AgentNode: `crates/dashflow/src/integration.rs:150-237`
- ToolNode: `crates/dashflow/src/integration.rs:268-342`
- tools_condition: `crates/dashflow/src/integration.rs:375-385`
- auto_tool_executor: `crates/dashflow/src/integration.rs:429-609`
- Example: `crates/dashflow/examples/tool_using_workflow.rs` (demonstrates ToolNode usage)
- Tests: `crates/dashflow/src/integration.rs:612-2376` (comprehensive test suite)

### Node Trait and Implementations
**Location:** `crates/dashflow/src/node.rs`

Nodes are the fundamental computational units in DashFlow. Each node receives state, processes it (LLM calls, tool execution, data transformations), and returns updated state. Nodes are connected by edges to form directed graphs that define execution flow.

**Core Concept:**

DashFlow execution is node-driven: the executor traverses the graph, invoking nodes in topological order (respecting edge dependencies). Each node is a black box to the executor—it only knows:
1. **Input:** Current state (generic type `S`)
2. **Output:** Updated state (same type `S`)
3. **Error handling:** Node can return `Result<S>` to signal failures

This design enables:
- **Composability:** Nodes can be any computation (LLM, database, API call, pure function)
- **Type safety:** State type `S` ensures all nodes in a graph use compatible state
- **Testability:** Nodes are independent, easily tested in isolation
- **Reusability:** Nodes can be shared across multiple graphs

**Node Trait (Primary Interface):**

```rust
use dashflow::{Node, Result};
use async_trait::async_trait;

// Define state type
#[derive(Clone)]
struct AgentState {
    messages: Vec<String>,
    user_id: String,
}

// Implement Node trait for custom logic
struct ResearchNode;

#[async_trait]
impl Node<AgentState> for ResearchNode {
    async fn execute(&self, state: AgentState) -> Result<AgentState> {
        // Perform research (API call, database query, etc.)
        let mut state = state;
        state.messages.push("Research complete".to_string());
        Ok(state)
    }

    fn name(&self) -> String {
        "research".to_string()  // Used for tracing/debugging
    }
}

// Add node to graph
graph.add_node("research", Arc::new(ResearchNode));
```

**Node Trait Definition:**

```rust
#[async_trait]
pub trait Node<S>: Send + Sync
where
    S: Send + Sync,
{
    /// Execute this node with the given state
    async fn execute(&self, state: S) -> Result<S>;

    /// Get the name of this node (for debugging and tracing)
    /// Default: extracts last component of type name
    fn name(&self) -> String {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .unwrap_or("Node")
            .to_string()
    }
}
```

**Key Characteristics:**
- **Async by default:** `async fn execute` enables I/O operations (LLM calls, API requests)
- **Generic over state:** `Node<S>` works with any state type implementing `Send + Sync`
- **Move semantics:** State is moved into node, returned as updated state (ownership model)
- **Error propagation:** Return `Result<S>` to signal failures (halts graph execution)
- **Thread-safe:** `Send + Sync` bounds enable parallel node execution
- **Type-erased:** Stored as `BoxedNode<S> = Arc<dyn Node<S>>` in graph

**Why This Design:**

**Problem 1: Heterogeneous Computation**
Graphs need different computation types (LLM, tools, routing logic, data transforms). Without a trait abstraction:
- Executor must know about every node type (tight coupling)
- Adding new node types requires executor changes (not extensible)
- Cannot store different node types in a collection (no common interface)

**Problem 2: Async Execution**
DashFlow operations are I/O-bound (LLM API calls, database queries). Synchronous nodes would block threads:
- One slow node blocks entire graph (no concurrency)
- Cannot leverage Tokio runtime (inefficient resource usage)
- Parallel node execution impossible (linear performance)

**Solution: Trait-Based Abstraction**
- **Node trait:** Common interface for all computational units
- **Type erasure (BoxedNode):** Store different node types in graph (`Arc<dyn Node<S>>`)
- **Async trait:** Enable concurrent I/O operations (Tokio-powered)
- **Generic state:** Type safety without rigid state structure

**FunctionNode (Convenience Wrapper):**

For simple nodes, implementing the `Node` trait is verbose. `FunctionNode` wraps async functions as nodes:

```rust
use dashflow::node::FunctionNode;

// Create node from async closure
let double_node = FunctionNode::new("double", |state: AgentState| {
    Box::pin(async move {
        let mut state = state;
        state.messages.push("Doubled".to_string());
        Ok(state)
    })
});

graph.add_node("double", Arc::new(double_node));
```

**When to Use FunctionNode:**
- **Simple logic:** Single-purpose transformations (parse JSON, filter messages)
- **Stateless operations:** No internal state beyond input (pure functions)
- **Rapid prototyping:** Quick node creation without boilerplate
- **Testing:** Easy to create test nodes inline

**When to Implement Node Trait:**
- **Complex logic:** Multiple methods, internal state, configuration
- **Reusable components:** Nodes used across multiple graphs
- **Custom name():** Override default name for better tracing
- **Struct-based state:** Node holds configuration (API keys, model settings)

**FunctionNode Example (Stateful Logic):**

```rust
// Conditional transformation
let clamp_node = FunctionNode::new("clamp", |state: NumericState| {
    Box::pin(async move {
        let value = if state.value < 0 {
            0  // Floor at zero
        } else if state.value > 100 {
            100  // Cap at 100
        } else {
            state.value
        };
        Ok(NumericState { value })
    })
});
```

**BoxedNode (Type-Erased Storage):**

Graphs store nodes with different implementations (custom structs, FunctionNode) in a single collection. `BoxedNode` provides type erasure:

```rust
pub type BoxedNode<S> = Arc<dyn Node<S>>;

// Store different node types in graph
let nodes: HashMap<String, BoxedNode<AgentState>> = HashMap::new();
nodes.insert("research", Arc::new(ResearchNode));
nodes.insert("double", Arc::new(FunctionNode::new("double", |s| Box::pin(async { Ok(s) }))));
nodes.insert("llm", Arc::new(LLMNode::new(model)));
```

**Why Arc (Not Box):**
- **Cloning:** `Arc::clone()` is cheap (pointer copy), enables sharing nodes across threads
- **Thread-safety:** `Arc<dyn Node<S>>` is `Send + Sync` for parallel execution
- **Shared ownership:** Multiple graph references can hold same node (e.g., subgraphs)

**Use Cases:**

**Use Case 1: LLM Node**
Wrap ChatModel as a node to call LLM during graph execution:

```rust
struct LLMNode {
    model: Arc<dyn ChatModel>,
}

#[async_trait]
impl Node<AgentState> for LLMNode {
    async fn execute(&self, state: AgentState) -> Result<AgentState> {
        // Convert state to prompt
        let prompt = format!("Messages: {:?}", state.messages);

        // Call LLM
        let response = self.model.generate(&prompt).await?;

        // Update state with LLM response
        let mut state = state;
        state.messages.push(response);
        Ok(state)
    }
}
```

**Use Case 2: Tool Execution Node**
Execute tools based on LLM tool calls:

```rust
struct ToolNode {
    tools: HashMap<String, Arc<dyn Tool>>,
}

#[async_trait]
impl Node<AgentState> for ToolNode {
    async fn execute(&self, state: AgentState) -> Result<AgentState> {
        let last_message = state.messages.last().unwrap();

        // Parse tool calls from last message
        let tool_calls = parse_tool_calls(last_message)?;

        // Execute each tool
        let mut results = Vec::new();
        for call in tool_calls {
            let tool = self.tools.get(&call.name)
                .ok_or_else(|| Error::ToolNotFound(call.name.clone()))?;
            let result = tool.invoke(&call.args).await?;
            results.push(result);
        }

        // Add tool results to state
        let mut state = state;
        state.messages.push(format!("Tool results: {:?}", results));
        Ok(state)
    }
}
```

**Use Case 3: Data Transformation Node**
Transform state data (parsing, filtering, aggregation):

```rust
let parse_json_node = FunctionNode::new("parse_json", |state: AgentState| {
    Box::pin(async move {
        let last_message = state.messages.last().unwrap();

        // Parse JSON from last message
        let parsed: serde_json::Value = serde_json::from_str(last_message)
            .map_err(|e| Error::Validation(format!("Invalid JSON: {}", e)))?;

        let mut state = state;
        state.messages.push(format!("Parsed: {:?}", parsed));
        Ok(state)
    })
});
```

**Use Case 4: Conditional Routing Node**
Inspect state and decide next path (routing logic often in edges, but can be in nodes):

```rust
let route_node = FunctionNode::new("route", |state: AgentState| {
    Box::pin(async move {
        let last_message = state.messages.last().unwrap();

        // Set routing flag based on message content
        let mut state = state;
        if last_message.contains("urgent") {
            state.priority = "high";
        } else {
            state.priority = "normal";
        }
        Ok(state)
    })
});
```

**Performance Characteristics:**

- **Node invocation overhead:** ~10-50μs (trait vtable dispatch + async runtime)
- **FunctionNode overhead:** ~5-10μs (closure call + boxing)
- **BoxedNode overhead:** ~2-5μs (Arc deref)
- **Total overhead per node:** ~15-65μs (negligible compared to LLM calls)

**Typical node execution times:**
- LLM node: 500-2000ms (API call dominates)
- Tool node: 10-500ms (depends on tool complexity)
- Data transformation: 1-10ms (parsing, filtering)
- Routing logic: <1ms (simple conditionals)

**Node overhead is <0.01% of typical graph execution time** (dominated by I/O).

**upstream Python DashFlow Compatibility:**

Rust Node trait matches upstream Python DashFlow node semantics:

| Feature | upstream Python DashFlow | DashFlow | Notes |
|---------|------------------|----------------|-------|
| Node as function | `def my_node(state): return state` | `FunctionNode::new("my_node", \|s\| ...)` | Both support functions as nodes |
| Node as class | `class MyNode: def __call__(self, state): ...` | `impl Node<S> for MyNode` | Both support struct-based nodes |
| Async execution | `async def my_node(state): ...` | `async fn execute(&self, state: S)` | Both async by default |
| Type erasure | Duck typing (no explicit trait) | `BoxedNode<S> = Arc<dyn Node<S>>` | Rust requires explicit trait |
| Error handling | Raise exceptions | `Result<S>` return type | Both halt graph on error |

**Python Example (Equivalent):**

```python
# upstream Python DashFlow node
def research_node(state: AgentState) -> AgentState:
    state["messages"].append("Research complete")
    return state

# DashFlow node
let research_node = FunctionNode::new("research", |state: AgentState| {
    Box::pin(async move {
        let mut state = state;
        state.messages.push("Research complete".to_string());
        Ok(state)
    })
});
```

**Default name() Behavior:**

If you don't override `name()`, the trait extracts the last component of the type name:

```rust
struct MyCustomNode;

#[async_trait]
impl Node<AgentState> for MyCustomNode {
    async fn execute(&self, state: AgentState) -> Result<AgentState> {
        Ok(state)
    }
    // name() not overridden - defaults to "MyCustomNode"
}

let node = MyCustomNode;
assert_eq!(node.name(), "MyCustomNode");
```

**Custom name() for Clarity:**

```rust
struct MultiplyNode { factor: i32 }

#[async_trait]
impl Node<NumericState> for MultiplyNode {
    async fn execute(&self, state: NumericState) -> Result<NumericState> {
        Ok(NumericState { value: state.value * self.factor })
    }

    fn name(&self) -> String {
        format!("MultiplyBy{}", self.factor)  // More descriptive
    }
}

let node = MultiplyNode { factor: 5 };
assert_eq!(node.name(), "MultiplyBy5");  // Better tracing output
```

**Node Testing:**

Nodes are independent units, easily tested in isolation:

```rust
#[tokio::test]
async fn test_research_node() {
    let node = ResearchNode;
    let state = AgentState {
        messages: vec!["Start".to_string()],
        user_id: "user123".to_string(),
    };

    let result = node.execute(state).await.unwrap();
    assert_eq!(result.messages.len(), 2);
    assert_eq!(result.messages[1], "Research complete");
}
```

**Error Handling:**

Nodes return `Result<S>` to propagate errors to the executor:

```rust
let validation_node = FunctionNode::new("validate", |state: AgentState| {
    Box::pin(async move {
        if state.messages.is_empty() {
            return Err(Error::Validation("No messages in state".to_string()));
        }
        Ok(state)
    })
});

// Error halts graph execution
// Executor propagates error to caller
```

**Best Practices:**

1. **Keep nodes focused:** One responsibility per node (single-purpose)
2. **Minimize state mutations:** Only modify fields the node owns
3. **Use FunctionNode for simple logic:** Avoid boilerplate for trivial nodes
4. **Implement Node trait for reusability:** Share complex nodes across graphs
5. **Override name() for tracing:** Use descriptive names for production debugging
6. **Return errors early:** Use `?` operator for error propagation
7. **Avoid cloning large data:** Use `Arc` for shared read-only data
8. **Test nodes independently:** Unit test each node before adding to graph

**Code Pointers:**

- Core module: `crates/dashflow/src/node.rs`
- Node trait definition: `crates/dashflow/src/node.rs:70-193`
- FunctionNode implementation: `crates/dashflow/src/node.rs:253-264`
- BoxedNode type alias: `crates/dashflow/src/node.rs:218`
- Tests: `crates/dashflow/src/node.rs:761-2225` (69 test cases)
- Example: `crates/dashflow/examples/` (all examples use nodes)
- Integration: `crates/dashflow/src/integration.rs` (RunnableNode wraps Runnable as Node)

---

### NodeContext: Intra-Node Streaming Telemetry
**Location:** `crates/dashflow/src/node.rs`
**Added in:** v1.1.0 (November 2025)
**Status:** Production-ready

**Overview:**

Traditional nodes are "black boxes" — execution is opaque between NodeStart and NodeEnd events. For long-running operations, LLM reasoning, or multi-step processes, this creates severe observability gaps. NodeContext solves this by enabling nodes to emit telemetry **during execution**.

**The Problem:**

```rust
// Traditional node (black box)
async fn research_node(state: State) -> Result<State> {
    // User sees: NodeStart (T0)
    let results = expensive_search().await?;  // 2 seconds - NO VISIBILITY
    let analysis = analyze(results).await?;   // 1 second - NO VISIBILITY
    let summary = summarize(analysis).await?; // 1 second - NO VISIBILITY
    // User sees: NodeEnd (T0+4000ms)
    Ok(state.with_summary(summary))
}
```

**The Solution (Streaming Node):**

```rust
// Streaming node with NodeContext
struct ResearchNode;

#[async_trait]
impl Node<State> for ResearchNode {
    fn supports_streaming(&self) -> bool { true }

    async fn execute_with_context(&self, state: State, ctx: &NodeContext) -> Result<State> {
        ctx.send_progress("Starting search", 0.1).await?;
        let results = expensive_search().await?;

        ctx.send_progress("Analyzing results", 0.5).await?;
        let analysis = analyze(results).await?;

        ctx.send_progress("Generating summary", 0.9).await?;
        let summary = summarize(analysis).await?;

        ctx.send_progress("Complete", 1.0).await?;
        Ok(state.with_summary(summary))
    }

    async fn execute(&self, state: State) -> Result<State> {
        self.execute_with_context(state, &NodeContext::empty()).await
    }
}
```

**Key Concepts:**

1. **Opt-In**: Nodes implement `supports_streaming() -> bool` to enable telemetry
2. **NodeContext**: Execution context passed to `execute_with_context()`
3. **Fire-and-Forget**: All sends are async spawned (< 0.01% overhead)
4. **Backward Compatible**: Existing nodes work unchanged
5. **No-Op Mode**: If no DashStream producer, sends are no-ops

**NodeContext API:**

**High-Level API (Recommended):**
```rust
// Progress updates (0.0 to 1.0)
ctx.send_progress("Processing documents", 0.5).await?;

// LLM reasoning steps
ctx.send_thinking("User wants information about X", 1).await?;

// Internal substep completion
ctx.send_substep("validate_input", "complete").await?;
```

**Low-Level API (Full Protocol Access):**
```rust
// Token streaming (for LLM responses)
ctx.send_token("The", 0, false, "req-123").await?;

// Tool execution tracking
ctx.send_tool_event("call-1", "search_wikipedia", STAGE_COMPLETE, 500_000).await?;

// Custom metrics
ctx.send_metric("documents_processed", 150.0, "documents").await?;

// Non-fatal errors/warnings
ctx.send_error("PARTIAL_FAILURE", "2/10 sources failed", SEVERITY_WARNING).await?;
```

**Enhanced Node Trait Methods:**

```rust
#[async_trait]
pub trait Node<S>: Send + Sync {
    /// Execute with streaming context (v1.1.0+)
    /// Override this to emit telemetry during execution
    async fn execute_with_context(&self, state: S, ctx: &NodeContext) -> Result<S> {
        // Default: calls execute() (backward compatible)
        self.execute(state).await
    }

    /// Original execution method (unchanged)
    async fn execute(&self, state: S) -> Result<S>;

    /// Does this node emit telemetry? (v1.1.0+)
    /// Return true to enable context creation
    fn supports_streaming(&self) -> bool {
        false  // Default: no streaming (backward compatible)
    }
}
```

**Complete Example: Multi-Stage Search Agent**

```rust
struct SearchAgentNode;

#[async_trait]
impl Node<AnalysisState> for SearchAgentNode {
    fn supports_streaming(&self) -> bool { true }

    async fn execute_with_context(
        &self,
        mut state: AnalysisState,
        ctx: &NodeContext,
    ) -> Result<AnalysisState> {
        // Phase 1: Query analysis
        ctx.send_thinking("Analyzing query complexity", 1).await?;
        ctx.send_progress("Analyzing query", 0.2).await?;

        // Phase 2: Multi-source search
        let sources = vec!["Wikipedia", "ArXiv", "News"];
        for (i, source) in sources.iter().enumerate() {
            let call_id = format!("search_{}", i);

            // Tool start
            ctx.send_tool_event(&call_id, source, STAGE_STARTED, 0).await?;
            ctx.send_substep(&format!("search_{}", source), "started").await?;

            // Perform search
            let results = search(source, &state.query).await?;

            // Tool complete
            ctx.send_tool_event(&call_id, source, STAGE_COMPLETE, 500_000).await?;
            ctx.send_substep(&format!("search_{}", source), "complete").await?;

            // Progress update
            let progress = 0.2 + (0.6 * (i + 1) as f64 / sources.len() as f64);
            ctx.send_progress(&format!("Searched {}/{}", i + 1, sources.len()), progress).await?;

            state.results.extend(results);
        }

        // Phase 3: Result synthesis
        ctx.send_thinking("Synthesizing results from multiple sources", 2).await?;
        ctx.send_progress("Synthesizing results", 0.9).await?;

        let summary = synthesize(&state.results).await?;
        state.summary = summary;

        // Emit final metrics
        ctx.send_metric("sources_searched", sources.len() as f64, "sources").await?;
        ctx.send_metric("results_found", state.results.len() as f64, "results").await?;

        ctx.send_progress("Complete", 1.0).await?;
        Ok(state)
    }

    async fn execute(&self, state: AnalysisState) -> Result<AnalysisState> {
        self.execute_with_context(state, &NodeContext::empty()).await
    }
}
```

**Telemetry Events Emitted:**

When the above node executes with a DashStreamCallback attached, it emits:
- **1 NodeStart** event (executor)
- **2 NodeThinking** events (reasoning steps)
- **~8 NodeProgress** events (progress tracking)
- **6 NodeSubstep** events (3 start + 3 complete)
- **6 ToolExecution** events (3 start + 3 complete)
- **2 Metrics** events (sources_searched, results_found)
- **1 NodeEnd** event (executor)

**Integration with DashStream:**

NodeContext works seamlessly with DashStreamCallback:

```rust
// Create DashStream callback
let callback = DashStreamCallback::<State>::with_config(config).await?;

// Compile graph with callback
let compiled = graph.compile()?.with_callback(callback);

// Execute - streaming nodes automatically get context
let result = compiled.invoke(initial_state).await?;
```

**Executor Integration:**

The executor automatically:
1. Checks `node.supports_streaming()`
2. Creates `NodeContext` if true (extracts producer from DashStreamCallback)
3. Calls `execute_with_context()` instead of `execute()`
4. Falls back to `execute()` if `supports_streaming()` returns false

**Performance Characteristics:**

- **Context creation**: ~2-5μs (only for streaming nodes)
- **Send overhead**: <0.01% (fire-and-forget tokio::spawn)
- **No producer**: Zero overhead (no-op check is branch prediction friendly)
- **Feature gated**: Works without `dashstream` feature (conditional compilation)

**Backward Compatibility:**

```rust
// Old node (still works, no changes needed)
struct OldNode;

#[async_trait]
impl Node<State> for OldNode {
    async fn execute(&self, state: State) -> Result<State> {
        Ok(state)
    }
}

// Result: supports_streaming() = false (default)
// Executor calls execute() directly (zero overhead)
```

**Use Cases:**

1. **Long-Running Operations**: Document processing, batch inference, large-scale searches
2. **LLM Chain-of-Thought**: Capture reasoning steps for debugging and analysis
3. **Multi-Step Workflows**: Track substep completion (validation → API call → processing)
4. **Token Streaming**: Real-time LLM response generation for UI updates
5. **Tool Execution**: Track tool call lifecycle (requested → started → completed → failed)
6. **Performance Monitoring**: Emit custom metrics during execution
7. **Error Diagnosis**: Send warnings for degraded mode or partial failures

**Observability Benefits:**

- **Real-Time Monitoring**: See exactly what nodes are doing (not black boxes)
- **Debugging**: Identify where nodes get stuck or slow
- **User Experience**: Show progress indicators in UIs
- **Performance Analysis**: Track substep durations and bottlenecks
- **Cost Tracking**: Monitor LLM token usage in real-time
- **Error Diagnosis**: Understand failure context with warnings

**Code Pointers:**

- NodeContext struct: `crates/dashflow/src/node.rs` (feature gated)
- Node trait enhancements: `crates/dashflow/src/node.rs:execute_with_context(), supports_streaming()`
- Executor integration: `crates/dashflow/src/executor/mod.rs:create_node_context(), execute_node()`
- EventCallback trait: `crates/dashflow/src/event.rs:get_producer(), get_ids()`
- Protocol: `proto/dashstream.proto:EventType` (80-83: NODE_PROGRESS, NODE_THINKING, NODE_SUBSTEP, NODE_WARNING)
- Example: `crates/dashflow/examples/streaming_node.rs` (comprehensive demo)
- Tests: `crates/dashflow/src/node.rs` (69 tests for NodeContext)
- Documentation: `docs/DASHSTREAM_PROTOCOL.md` (Intra-Node Streaming Telemetry section)

**upstream Python DashFlow Compatibility:**

This feature is **Rust-specific innovation** (not in upstream Python DashFlow as of November 2025). upstream Python DashFlow nodes are still black boxes. This is a competitive advantage for Rust implementation:

| Feature | upstream Python DashFlow | DashFlow |
|---------|------------------|----------------|
| Node visibility | Black box (NodeStart/End only) | Full transparency (progress, thinking, substeps) |
| Token streaming | External (separate API) | Integrated (ctx.send_token()) |
| Tool tracking | Limited | Full lifecycle (requested → started → complete) |
| Progress updates | Manual (state updates) | Native (ctx.send_progress()) |
| Overhead | N/A | <0.01% (fire-and-forget) |

**Example: Running the Demo**

```bash
# Start Kafka
docker-compose -f docker-compose-kafka.yml up -d

# Run streaming example
cargo run --example streaming_node --features dashstream

# Monitor telemetry in real-time
docker-compose -f docker-compose-kafka.yml exec kafka \
    kafka-console-consumer --bootstrap-server localhost:9092 \
    --topic dashstream-streaming-demo --from-beginning
```

---

### Edge Types and Routing
**Location:** `crates/dashflow/src/edge.rs`

Edges define how nodes are connected in a DashFlow, controlling execution flow. While nodes perform computation (LLM calls, tool execution), edges determine which node executes next. DashFlow supports three edge types: **Simple** (unconditional transitions), **Conditional** (state-based routing), and **Parallel** (concurrent fan-out to multiple nodes).

**Core Concept:**

Graph execution is edge-driven routing: after a node completes, edges determine the next node(s) to execute. This separates **what to compute** (nodes) from **when to compute** (edges), enabling:
- **Declarative control flow:** Routing logic is explicit, not buried in node code
- **Dynamic branching:** Same node can route to different successors based on state
- **Parallel execution:** Multiple nodes execute concurrently when appropriate
- **Testable routing:** Edge conditions are independent functions, easily tested

**Edge Types:**

DashFlow provides three edge types, each optimized for specific routing patterns:

1. **Simple Edge:** Direct node-to-node transition (no conditions)
2. **Conditional Edge:** State-based routing to one of multiple successors
3. **Parallel Edge:** Fan-out to multiple nodes (all execute concurrently)

**Simple Edge (Unconditional Transition):**

Direct connection from one node to another. Always executes after source node completes.

```rust
use dashflow::edge::Edge;

// Create simple edge: node1 → node2
let edge = Edge::new("node1", "node2");

// Add to graph
graph.add_edge("node1", "node2");
```

**Structure:**
```rust
pub struct Edge {
    pub from: Arc<String>,  // Source node name
    pub to: Arc<String>,    // Destination node name
}
```

**When to Use:**
- **Sequential workflows:** node1 → node2 → node3 (no branching)
- **Mandatory transitions:** Always proceed to next node (no conditions)
- **Pipeline stages:** Data transformation pipelines (parse → validate → store)

**Example: Linear Pipeline**
```rust
// Simple 3-stage pipeline
graph.add_edge("parse", "validate");
graph.add_edge("validate", "store");
graph.add_edge("store", END);

// Execution: parse → validate → store → END
```

**Conditional Edge (State-Based Routing):**

Routes to different nodes based on state. Condition function examines state and returns next node name.

```rust
use dashflow::edge::ConditionalEdge;
use std::collections::HashMap;

// Define routing function
fn route_by_score(state: &AgentState) -> String {
    if state.score > 80 {
        "high_priority".to_string()
    } else {
        "normal_priority".to_string()
    }
}

// Create routes map (for validation)
let mut routes = HashMap::new();
routes.insert("high_priority".to_string(), "high_priority_node".to_string());
routes.insert("normal_priority".to_string(), "normal_priority_node".to_string());

// Create conditional edge
let edge = ConditionalEdge::new("evaluator", route_by_score, routes);

// Add to graph
graph.add_conditional_edges("evaluator", route_by_score, routes);
```

**Structure:**
```rust
pub struct ConditionalEdge<S: Send + Sync> {
    pub from: Arc<String>,                              // Source node
    pub condition: Arc<dyn Fn(&S) -> String + Send + Sync>,  // Routing function
    pub routes: HashMap<String, String>,                // Valid routes (for validation)
}
```

**Condition Function:**
- **Input:** Immutable reference to state (`&S`)
- **Output:** String (name of next node)
- **Constraints:** Must be `Send + Sync` (thread-safe)
- **Lifetime:** Stored as `Arc` (shared across invocations)

**When to Use:**
- **Branching logic:** Different paths based on state (if-then-else in graph form)
- **Tool calling:** Route to tools if LLM requests them, END if no tools
- **Error handling:** Route to error handler on failure, continue on success
- **Dynamic workflows:** Next node determined by runtime state (not static structure)

**Example: Tool Calling Pattern (ReAct Agent)**
```rust
use dashflow::integration::tools_condition;

// Route to tools if LLM made tool calls, END if none
graph.add_conditional_edges(
    "agent",
    |state: &AgentState| tools_condition(&state.messages).to_string(),
    {
        let mut routes = HashMap::new();
        routes.insert("tools".to_string(), "tools".to_string());
        routes.insert("__end__".to_string(), "__end__".to_string());
        routes
    },
);

// Execution:
// - If last message has tool_calls → execute "tools" node
// - If no tool_calls → END (agent finished)
```

**Example: Multi-Way Branching**
```rust
fn classify_intent(state: &AgentState) -> String {
    let last_message = state.messages.last().unwrap();
    if last_message.contains("search") {
        "search_handler".to_string()
    } else if last_message.contains("summarize") {
        "summarize_handler".to_string()
    } else {
        "general_handler".to_string()
    }
}

let mut routes = HashMap::new();
routes.insert("search_handler".to_string(), "search_handler".to_string());
routes.insert("summarize_handler".to_string(), "summarize_handler".to_string());
routes.insert("general_handler".to_string(), "general_handler".to_string());

graph.add_conditional_edges("classifier", classify_intent, routes);
```

**Parallel Edge (Concurrent Fan-Out):**

Routes to multiple nodes simultaneously. All target nodes execute in parallel, results merged.

```rust
use dashflow::edge::ParallelEdge;

// Create parallel edge: node1 → [node2, node3, node4] (all concurrent)
let edge = ParallelEdge::new(
    "broadcast",
    vec!["handler1".to_string(), "handler2".to_string(), "handler3".to_string()],
);

// Add to graph (not yet implemented in StateGraph API, but edge type exists)
```

**Structure:**
```rust
pub struct ParallelEdge {
    pub from: Arc<String>,       // Source node
    pub to: Arc<Vec<String>>,    // Target nodes (all execute in parallel)
}
```

**Execution Model:**
1. Source node completes
2. Current state cloned to each target node (independent copies)
3. All target nodes execute concurrently (Tokio spawn)
4. Results merged using state reducers (e.g., add_messages merges message lists)
5. Merged state passed to next node(s)

**When to Use:**
- **Independent operations:** Multiple tasks that don't depend on each other
- **Parallel I/O:** Fetch from multiple APIs concurrently (reduce latency)
- **Fan-out patterns:** One input, multiple processors (e.g., analyze with multiple models)
- **Performance optimization:** Exploit concurrency when operations are I/O-bound

**Example: Parallel API Calls**
```rust
// Fetch data from 3 APIs concurrently
let edge = ParallelEdge::new(
    "fetch_trigger",
    vec![
        "fetch_weather".to_string(),
        "fetch_news".to_string(),
        "fetch_stocks".to_string(),
    ],
);

// All 3 nodes execute in parallel (latency = max(weather, news, stocks), not sum)
```

**State Merging with Reducers:**

Parallel edges require state reducers to merge results. Without reducers, last result wins (data loss).

```rust
#[derive(GraphState)]
struct AgentState {
    #[add_messages]  // Reducer: merge message lists (append)
    messages: Vec<Message>,

    // Other fields use default reducer (last write wins)
    user_id: String,
}

// Parallel execution:
// - Node1 adds messages ["A", "B"]
// - Node2 adds messages ["C", "D"]
// - Node3 adds messages ["E", "F"]
// Result after merge: ["A", "B", "C", "D", "E", "F"] (all messages preserved)
```

**Edge Type Enum (Internal):**

Graphs store edges as `EdgeType<S>` enum:

```rust
pub enum EdgeType<S: Send + Sync> {
    Simple(Edge),                           // Unconditional edge
    Conditional(Arc<ConditionalEdge<S>>),   // State-based routing
    Parallel(ParallelEdge),                 // Concurrent fan-out
}
```

**Special Markers (START and END):**

DashFlow uses special node names for graph boundaries:

```rust
pub const START: &str = "__start__";  // Entry point (implicit)
pub const END: &str = "__end__";      // Exit point (terminal node)
```

**START:**
- Implicit entry node (no implementation needed)
- First edges connect START → first real node
- Used in routing: `graph.add_edge(START, "first_node")`

**END:**
- Terminal node (graph stops execution)
- Routes to END signal completion
- Used in routing: `graph.add_edge("final_node", END)`

**Example: START and END**
```rust
// Connect entry point to first node
graph.add_edge(START, "parse_input");

// Connect final node to exit
graph.add_edge("generate_response", END);

// Conditional exit (route to END when done)
graph.add_conditional_edges(
    "agent",
    |state: &AgentState| {
        if state.done {
            END.to_string()
        } else {
            "continue".to_string()
        }
    },
    routes,
);
```

**Why This Design:**

**Problem 1: Control Flow Embedded in Nodes**
Without edges, nodes must decide what to execute next (imperative control flow):
- Nodes have hardcoded next node references (tight coupling)
- Routing logic mixed with business logic (hard to test)
- Cannot change workflow without modifying node code (inflexible)
- Graph structure not visible (no declarative representation)

**Problem 2: No Parallel Execution**
Sequential node execution is inefficient for I/O-bound workloads:
- Fetch 3 APIs sequentially: 300ms + 400ms + 200ms = 900ms total
- Fetch 3 APIs in parallel: max(300ms, 400ms, 200ms) = 400ms total
- Without parallel edges, cannot express concurrent operations

**Solution: Declarative Edge System**
- **Simple edges:** Sequential workflows (explicit transitions)
- **Conditional edges:** Dynamic branching (routing as data)
- **Parallel edges:** Concurrent execution (exploit I/O parallelism)
- **Separation of concerns:** Nodes compute, edges route (testability)

**Performance Characteristics:**

- **Simple edge overhead:** ~1-2μs (check destination node exists)
- **Conditional edge overhead:** ~5-10μs (call condition function + validate route)
- **Parallel edge overhead:** ~50-100μs (spawn Tokio tasks + state cloning)
- **Condition function execution:** Depends on complexity (typically <1ms)

**Typical edge execution times:**
- Simple edge: ~1-2μs (negligible)
- Conditional edge (simple condition): ~10-20μs (negligible)
- Conditional edge (complex condition): ~1-10ms (depends on state inspection)
- Parallel edge (spawn 3 nodes): ~100μs + max(node1, node2, node3) execution time

**Edge overhead is <0.01% of typical graph execution time.**

**Use Cases:**

**Use Case 1: ReAct Agent (Tool Calling)**
```rust
// Agent → Tools (if tool_calls) → Agent (loop)
//      → END (if no tool_calls)

graph.add_edge(START, "agent");
graph.add_conditional_edges(
    "agent",
    |state: &AgentState| {
        if has_tool_calls(&state.messages) {
            "tools".to_string()
        } else {
            END.to_string()
        }
    },
    routes,
);
graph.add_edge("tools", "agent");  // Loop back to agent
```

**Use Case 2: Error Handling**
```rust
// Parser → Success → Store
//        → Failure → Error Handler → END

graph.add_conditional_edges(
    "parser",
    |state: &DataState| {
        if state.error.is_none() {
            "store".to_string()
        } else {
            "error_handler".to_string()
        }
    },
    routes,
);
graph.add_edge("store", END);
graph.add_edge("error_handler", END);
```

**Use Case 3: Parallel Data Fetching**
```rust
// Fetch → [Weather, News, Stocks] (parallel) → Aggregate → END

let parallel = ParallelEdge::new(
    "fetch_trigger",
    vec![
        "fetch_weather".to_string(),
        "fetch_news".to_string(),
        "fetch_stocks".to_string(),
    ],
);
// (Parallel edge API not yet exposed in StateGraph, but edge type exists)
```

**Use Case 4: Multi-Stage Pipeline**
```rust
// Scrape → Parse → Validate → Transform → Store → END

graph.add_edge(START, "scrape");
graph.add_edge("scrape", "parse");
graph.add_edge("parse", "validate");
graph.add_edge("validate", "transform");
graph.add_edge("transform", "store");
graph.add_edge("store", END);
```

**upstream Python DashFlow Compatibility:**

Rust edge types match upstream Python DashFlow edge semantics:

| Feature | upstream Python DashFlow | DashFlow | Notes |
|---------|------------------|----------------|-------|
| Simple edge | `graph.add_edge("a", "b")` | `graph.add_edge("a", "b")` | Identical API |
| Conditional edge | `graph.add_conditional_edges("a", func, routes)` | `graph.add_conditional_edges("a", func, routes)` | Identical API |
| Condition function | `def func(state): return "next"` | `\|state\| "next".to_string()` | Both accept functions |
| START marker | `START` | `START` | Same constant |
| END marker | `END` | `END` | Same constant |

**Python Example (Equivalent):**

```python
from dashflow.graph import StateGraph, START, END

# Python: Simple edge
graph.add_edge("node1", "node2")

# Python: Conditional edge
def route(state):
    return "tools" if state["tool_calls"] else END

graph.add_conditional_edges("agent", route, {"tools": "tools", END: END})

# Rust: Same semantics
graph.add_edge("node1", "node2");

graph.add_conditional_edges(
    "agent",
    |state: &AgentState| {
        if state.tool_calls { "tools".to_string() } else { END.to_string() }
    },
    routes,
);
```

**Edge Serialization:**

Simple and Parallel edges support serialization (checkpointing):

```rust
use serde_json;

// Serialize edge
let edge = Edge::new("node1", "node2");
let json = serde_json::to_string(&edge).unwrap();

// Deserialize edge
let deserialized: Edge = serde_json::from_str(&json).unwrap();
assert_eq!(deserialized.from.as_str(), "node1");
assert_eq!(deserialized.to.as_str(), "node2");
```

**Note:** ConditionalEdge cannot be serialized (contains function pointer). Conditional routing must be reconstructed on deserialization.

**Memory Efficiency (Arc Sharing):**

Edges use `Arc<String>` for node names (cheap cloning):

```rust
let edge1 = Edge::new("node", "target");
let edge2 = edge1.clone();  // Arc clone (pointer copy, not string copy)

// Same pointer (memory shared)
assert!(Arc::ptr_eq(&edge1.from, &edge2.from));
```

**Benefits:**
- **Cheap clones:** O(1) pointer copy, not O(n) string copy
- **Memory sharing:** Multiple edges referencing same node share one string
- **Thread-safe:** `Arc` enables sharing across threads (parallel execution)

**Best Practices:**

1. **Use simple edges for sequential workflows:** Unconditional transitions (no branching)
2. **Use conditional edges for dynamic routing:** State-based decisions (if-then-else)
3. **Keep condition functions pure:** No side effects (easier testing, no surprises)
4. **Route to END explicitly:** Don't rely on implicit termination (clear intent)
5. **Validate routes in condition function:** Ensure condition returns valid node name
6. **Use parallel edges for independent I/O:** Fetch multiple APIs concurrently
7. **Document routing logic:** Condition functions are code (need comments)
8. **Test condition functions independently:** Unit test routing logic outside graph

**Code Pointers:**

- Core module: `crates/dashflow/src/edge.rs`
- Edge struct: `crates/dashflow/src/edge.rs:100-113`
- ConditionalEdge struct: `crates/dashflow/src/edge.rs:171-182`
- ParallelEdge struct: `crates/dashflow/src/edge.rs:278-291`
- EdgeType enum: `crates/dashflow/src/edge.rs:305-315`
- START constant: `crates/dashflow/src/edge.rs:351`
- END constant: `crates/dashflow/src/edge.rs:331`
- Tests: `crates/dashflow/src/edge.rs:353-984` (38 test cases)
- Example: `crates/dashflow/examples/` (all examples use edges)
- Integration: `crates/dashflow/src/integration.rs:375-385` (tools_condition helper)

### Streaming (Enhanced in v1.7.0)
**Location:** `crates/dashflow/src/stream.rs`

Real-time streaming of intermediate results and events during graph execution.

```rust
use dashflow::stream::StreamMode;

// Stream state updates
let app = graph.compile()?
    .with_checkpointer(checkpointer);

let mut stream = app.stream(state, StreamMode::Values).await?;
while let Some(update) = stream.next().await {
    println!("State update: {:?}", update);
}

// Stream with updates mode (deltas only)
let mut stream = app.stream(state, StreamMode::Updates).await?;
while let Some(delta) = stream.next().await {
    println!("Delta: {:?}", delta);
}

// Stream events (debug mode)
let mut stream = app.stream(state, StreamMode::Debug).await?;
while let Some(event) = stream.next().await {
    println!("Event: {:?}", event);
}
```

**Stream Modes:**
- `Values` - Complete state after each node (full state snapshots)
- `Updates` - Incremental state changes only (deltas)
- `Debug` - All internal events including node start/end, errors (verbose)

**Features (v1.7.0):**
- Zero-copy streaming with async iterators
- Checkpoint compatibility (works with all checkpointer types)
- Error propagation during streaming
- Backpressure support for slow consumers

**Code Pointers:**
- Implementation: `crates/dashflow/src/stream.rs`
- Tests: `crates/dashflow/tests/dashstream_integration.rs`
- Example: `examples/apps/librarian/` (production streaming patterns)

### Event System
**Location:** `crates/dashflow/src/event.rs`

Custom callbacks for workflow lifecycle.

```rust
use dashflow::event::{GraphEvent, EventCallback};

struct MyCallback;

impl EventCallback for MyCallback {
    async fn on_node_start(&self, node: &str) {
        println!("Starting node: {}", node);
    }

    async fn on_node_end(&self, node: &str, duration: Duration) {
        println!("Node {} took {:?}", node, duration);
    }
}

let app = graph.compile()?.with_callback(MyCallback);
```

**Code Pointer:** `crates/dashflow/src/event.rs`

### WASM Executor
**Location:** `crates/dashflow-wasm-executor/src/executor.rs`

HIPAA/SOC2 compliant WebAssembly sandbox for executing untrusted code in AI agent workflows. Provides secure, isolated execution with comprehensive audit trails and compliance controls.

```rust
use dashflow_wasm_executor::{WasmExecutor, WasmExecutorConfig, WasmCodeExecutionTool};

// Configure with JWT secret for authentication
let config = WasmExecutorConfig::new(
    "your-jwt-secret-at-least-32-characters-long".to_string()
);

let executor = WasmExecutor::new(config)?;

// Execute WASM code directly
let wasm_bytes = &[/* WASM bytecode */];
let result = executor.execute(wasm_bytes, "function_name", &[arg1, arg2]).await?;

// Or use as a DashFlow tool
let tool = WasmCodeExecutionTool::new(executor);
agent.with_tool(tool);
```

**Security Features:**
- WebAssembly sandbox (memory isolation, no file/network access)
- CPU/memory limits with fuel metering
- JWT authentication with role-based access control
- Complete audit trail (who, what, when, result, resources)
- 98% security rating (1 in 5,000 breach probability)

**Compliance:**
- ✅ HIPAA §164.312 Technical Safeguards
- ✅ SOC 2 Trust Service Criteria (CC6-9)
- ✅ Audit logging with immutable trails
- ✅ Encryption at rest and in transit
- ✅ Access controls and authentication

**Configuration:**
- `jwt_secret` - Secret for JWT token verification (required, ≥32 chars)
- `max_fuel` - CPU execution limit (default: 1M instructions)
- `audit_log_path` - Audit log file path (optional)
- `enable_metrics` - Prometheus metrics (default: false)

**Use Cases:**
- Code execution for AI agents (E2B alternative)
- User-submitted code sandboxing
- Plugin systems with untrusted code
- Healthcare/financial AI compliance
- Educational code evaluation

**Code Pointers:**
- Executor: `crates/dashflow-wasm-executor/src/executor.rs`
- Tool integration: `crates/dashflow-wasm-executor/src/tool.rs`
- Auth: `crates/dashflow-wasm-executor/src/auth.rs`
- Audit: `crates/dashflow-wasm-executor/src/audit.rs`
- Compliance guide: `docs/WASM_HIPAA_SOC2_COMPLIANCE.md`

### Remote Nodes
**Location:** `crates/dashflow-remote-node/src/client.rs`

Execute compute-intensive nodes on remote servers via gRPC. Ideal for distributed workflows where certain operations require specialized hardware (GPUs) or need to scale independently.

```rust
use dashflow_remote_node::RemoteNode;
use std::time::Duration;

// Create remote node client
let remote_node = RemoteNode::new("heavy_computation")
    .with_endpoint("http://compute-server:50051")
    .with_timeout(Duration::from_secs(300))
    .with_retry_count(3)
    .with_thread_id("session-123");  // For checkpoint integration

// Add to graph like any other node
graph.add_node("compute", remote_node);
```

**Features:**
- Transparent gRPC communication (looks like local node)
- Multiple serialization formats (JSON, Bincode)
- Automatic retry with exponential backoff
- Health checks before execution
- Request tracing with IDs
- Thread ID propagation for checkpoints
- Streaming support via `execute_node_stream`

**Configuration Options:**
- `endpoint` - gRPC server URL
- `timeout` - Execution timeout (default: 60s)
- `retry_count` - Retry attempts (default: 3)
- `retry_delay` - Initial retry delay (default: 100ms)
- `format` - Serialization format (JSON or Bincode)
- `health_check` - Enable health checks (default: true)
- `thread_id` - Thread ID for checkpoint isolation (optional)

**Server Side:**
```rust
use dashflow_remote_node::{NodeRegistry, RemoteNodeServer};

let mut registry = NodeRegistry::new();
registry.register("heavy_computation", my_computation_node)?;

RemoteNodeServer::new(registry)
    .serve("0.0.0.0:50051")
    .await?;
```

**Use Cases:**
- GPU-accelerated computations (model inference, training)
- Long-running operations that need dedicated resources
- Scaling specific nodes independently
- Hybrid local/cloud workflows
- Checkpoint isolation across distributed executions

**Code Pointers:**
- Client: `crates/dashflow-remote-node/src/client.rs`
- Server: `crates/dashflow-remote-node/src/server.rs`
- Protocol: `crates/dashflow-remote-node/proto/remote_node.proto`
- Tests: `crates/dashflow-remote-node/src/client.rs:514-577` (4 tests)

### Work-Stealing Scheduler
**Location:** `crates/dashflow/src/scheduler/mod.rs`

Distributed parallel execution orchestrator for DashFlow workflows. Implements classic work-stealing algorithms (inspired by Cilk and Rayon) to automatically load-balance parallel node execution across multiple remote workers.

**Core Algorithm:**
1. **Locality-first**: Execute locally when task queue is small (< threshold)
2. **Distribution**: Distribute to remote workers when queue exceeds threshold
3. **Load balancing**: Select workers based on configured strategy (RoundRobin, LeastLoaded, Random)
4. **Fault tolerance**: Automatic fallback to local execution if workers unavailable

```rust
use dashflow::{StateGraph, scheduler::WorkStealingScheduler};
use dashflow::scheduler::SelectionStrategy;

// Create scheduler with remote workers
let scheduler = WorkStealingScheduler::new()
    .with_workers(vec![
        "worker1.example.com:50051",
        "worker2.example.com:50051",
        "worker3.example.com:50051",
    ])
    .with_threshold(10)  // Distribute when queue > 10 tasks
    .with_strategy(SelectionStrategy::LeastLoaded)
    .with_work_stealing(true)
    .with_steal_attempts(3);

// Attach scheduler to compiled graph
let app = graph.compile()?
    .with_scheduler(scheduler);

// Parallel edges automatically use scheduler
let result = app.invoke(state).await?;

// Get performance metrics
let metrics = app.scheduler_metrics().await;
println!("Remote execution ratio: {:.1}%",
         metrics.remote_execution_ratio() * 100.0);
println!("Avg remote execution time: {:?}",
         metrics.avg_remote_execution_time());
```

**Configuration Options:**

- **`with_workers(endpoints)`** - Add remote worker gRPC endpoints
  - Example: `vec!["worker1:50051", "worker2:50051"]`
  - Workers are RemoteNode servers running graph node executors

- **`with_threshold(size)`** - Local queue threshold (default: 10)
  - When local queue size exceeds this, tasks distribute to workers
  - Lower threshold = more aggressive distribution
  - Higher threshold = more local execution (lower latency)

- **`with_strategy(strategy)`** - Worker selection strategy
  - `SelectionStrategy::LeastLoaded` - Select worker with lowest current load (default)
  - `SelectionStrategy::RoundRobin` - Distribute tasks in round-robin order
  - `SelectionStrategy::Random` - Select worker randomly

- **`with_work_stealing(enabled)`** - Enable work stealing between workers (default: true)
  - Future feature: Workers can steal tasks from each other when idle
  - Currently: Workers receive tasks from scheduler only

- **`with_steal_attempts(attempts)`** - Steal attempts per cycle (default: 3)
  - Number of times a worker tries to steal tasks when idle

**Performance Metrics (SchedulerMetrics):**

```rust
pub struct SchedulerMetrics {
    pub tasks_submitted: u64,           // Total tasks submitted
    pub tasks_executed_local: u64,      // Tasks executed locally
    pub tasks_executed_remote: u64,     // Tasks executed on workers
    pub tasks_stolen: u64,              // Tasks stolen by workers (future)
    pub execution_time_local: Duration, // Total local execution time
    pub execution_time_remote: Duration,// Total remote execution time
    pub task_distribution_latency: Duration, // Time spent distributing
}

// Helper methods
metrics.avg_local_execution_time() -> Option<Duration>
metrics.avg_remote_execution_time() -> Option<Duration>
metrics.remote_execution_ratio() -> f64  // 0.0 to 1.0
```

**When to Use:**

- **Parallel execution bottleneck**: Graph has many parallel edges executing simultaneously
- **Compute-intensive nodes**: Individual nodes require significant CPU/GPU resources
- **Horizontal scaling**: Need to scale execution capacity by adding workers
- **Resource isolation**: Different nodes require different runtime environments

**Use Cases:**

1. **Multi-agent workflows**: Distribute agent reasoning across worker pool
2. **Batch processing**: Process large document collections in parallel
3. **Real-time analysis**: Distribute live data analysis tasks across workers
4. **Hybrid deployment**: Local orchestration, remote execution (on-prem + cloud)

**Architecture Notes:**

- Scheduler runs locally in the graph executor (no separate scheduler server)
- Workers are standard RemoteNode servers (reuses existing infrastructure)
- Tasks are graph node executions (name, state) sent via gRPC
- State serialization uses JSON or Bincode (configured per worker)
- Automatic retry and health checks inherit from RemoteNode configuration

**Comparison to Local Parallel Execution:**

| Feature | Local (tokio::spawn) | Distributed (WorkStealingScheduler) |
|---------|---------------------|-------------------------------------|
| Setup complexity | Zero | Medium (deploy workers) |
| Latency overhead | ~1µs | ~1-5ms (network) |
| Scalability | Limited to local cores | Unlimited (add workers) |
| Fault tolerance | Process-level | Worker-level isolation |
| Resource isolation | Shared process memory | Separate worker processes |
| Cost | Free (local CPU) | Cloud compute costs |

**Code Pointers:**
- Core scheduler: `crates/dashflow/src/scheduler/mod.rs`
- Configuration: `crates/dashflow/src/scheduler/config.rs`
- Metrics: `crates/dashflow/src/scheduler/metrics.rs`
- Worker pool: `crates/dashflow/src/scheduler/worker.rs`
- Task abstraction: `crates/dashflow/src/scheduler/task.rs`
- Example: `crates/dashflow/examples/work_stealing_scheduler.rs`
- Integration: `crates/dashflow/src/executor/mod.rs:136-350` (parallel execution config, limits)

### Error Types and Error Handling
**Location:** `crates/dashflow/src/error.rs`

Comprehensive error types for DashFlow operations covering graph construction, execution, checkpointing, and validation failures. All errors are Send + Sync (thread-safe) and provide detailed context to diagnose failures.

**Core Concept:**

DashFlow error handling follows Rust's Result<T, Error> pattern. All fallible operations return Result<T> (alias for std::result::Result<T, Error>). This enables idiomatic error propagation with the `?` operator and clear error boundaries.

Error design principles:
- **Specific variants:** Each error type represents a distinct failure mode (validation, execution, timeout, etc.)
- **Contextual information:** Errors include relevant context (node name, duration, thread_id)
- **Helpful messages:** Error Display implementations provide clear guidance for fixing issues
- **Type-safe propagation:** thiserror::Error trait enables `?` operator and From conversions
- **Thread-safe:** All errors are Send + Sync (safe to use in async/await and parallel execution)

**Error Enum:**

```rust
use dashflow::Error;

pub enum Error {
    /// Graph validation error (construction-time checks)
    Validation(String),

    /// Node execution error (runtime failure in node logic)
    NodeExecution {
        node: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Graph has no entry point defined
    NoEntryPoint,

    /// Node not found in graph
    NodeNotFound(String),

    /// Cycle detected (when cycles are not allowed)
    CycleDetected(String),

    /// Invalid edge configuration
    InvalidEdge(String),

    /// Execution timeout
    Timeout(std::time::Duration),

    /// Serialization error (JSON/bincode)
    Serialization(#[from] serde_json::Error),

    /// Interrupt without checkpointer configured
    InterruptWithoutCheckpointer(String),

    /// Interrupt without thread_id configured
    InterruptWithoutThreadId(String),

    /// Resume without checkpointer configured
    ResumeWithoutCheckpointer,

    /// Resume without thread_id configured
    ResumeWithoutThreadId,

    /// No checkpoint to resume from
    NoCheckpointToResume(String),

    /// Recursion limit exceeded (infinite loop detection)
    RecursionLimit { limit: u32 },

    /// Generic error (catch-all)
    Generic(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

**Error Categories:**

**Construction-Time Errors (Graph Building):**

These occur during graph construction (before compilation):

| Error Variant | When It Occurs | Example | How to Fix |
|---------------|----------------|---------|------------|
| `Validation(String)` | Invalid graph structure | Missing required nodes, disconnected subgraphs | Verify all nodes are reachable, check edge connectivity |
| `NoEntryPoint` | No entry node defined | `compile()` called without `set_entry_point()` | Call `graph.set_entry_point("start_node")` |
| `InvalidEdge(String)` | Edge references non-existent nodes | `add_edge("missing", "target")` | Ensure both edge endpoints are added as nodes first |
| `CycleDetected(String)` | Graph has cycles (when disallowed) | `A -> B -> C -> A` without loops enabled | Enable loops with `allow_loops()` or redesign graph structure |

**Runtime Errors (Execution):**

These occur during graph execution (after compilation):

| Error Variant | When It Occurs | Example | How to Fix |
|---------------|----------------|---------|------------|
| `NodeExecution` | Node function panics/returns error | HTTP request fails, parsing error | Implement error handling in node logic, use retries |
| `NodeNotFound(String)` | Conditional edge routes to missing node | Edge function returns non-existent node name | Verify all conditional edge targets are valid node names |
| `Timeout(Duration)` | Execution exceeds timeout | 30s timeout, LLM call takes 45s | Increase timeout or optimize slow nodes |
| `RecursionLimit` | Iteration count exceeds limit | ReAct loop doesn't terminate after 100 steps | Fix infinite loop logic or increase limit with `with_recursion_limit()` |

**Checkpointing Errors (Pause/Resume):**

These occur when using checkpoint features incorrectly:

| Error Variant | When It Occurs | Example | How to Fix |
|---------------|----------------|---------|------------|
| `InterruptWithoutCheckpointer` | Node interrupts without checkpointer | Interrupt at node but no checkpointer configured | Call `with_checkpointer(checkpointer)` before compiling graph |
| `InterruptWithoutThreadId` | Node interrupts without thread_id | Interrupt at node but no thread_id in invoke | Call `with_thread_id("thread_123")` before invoking graph |
| `ResumeWithoutCheckpointer` | Resume called without checkpointer | Resume after interrupt but no checkpointer configured | Call `with_checkpointer(checkpointer)` before compiling graph |
| `ResumeWithoutThreadId` | Resume called without thread_id | Resume after interrupt but no thread_id in invoke | Call `with_thread_id("thread_123")` before invoking graph |
| `NoCheckpointToResume(String)` | Resume called but no checkpoint exists | Resume thread_id that was never saved | Verify thread_id is correct, check checkpointer storage |

**Serialization Errors:**

| Error Variant | When It Occurs | Example | How to Fix |
|---------------|----------------|---------|------------|
| `Serialization(serde_json::Error)` | State serialization fails | State contains non-serializable field | Ensure all state fields implement Serialize + Deserialize, avoid raw pointers |

**Error Handling Patterns:**

**Pattern 1: Propagate with `?` Operator (Recommended)**

Most common pattern. Let errors bubble up to caller:

```rust
use dashflow::{StateGraph, Error, Result};

async fn build_and_run_graph() -> Result<MyState> {
    let mut graph = StateGraph::new();
    graph.add_node("start", my_node)?;
    graph.set_entry_point("start")?;

    let app = graph.compile()?;
    let result = app.invoke(initial_state).await?;  // ? propagates Error
    Ok(result)
}
```

**Pattern 2: Match on Error Variants (Conditional Recovery)**

Handle specific errors differently:

```rust
use dashflow::{StateGraph, Error};

async fn execute_with_retry(app: &CompiledGraph<State>) -> Result<State> {
    match app.invoke(state).await {
        Ok(result) => Ok(result),
        Err(Error::Timeout(duration)) => {
            log::warn!("Timeout after {:?}, retrying...", duration);
            app.invoke(state).await  // Retry once
        }
        Err(Error::NodeExecution { node, source }) => {
            log::error!("Node '{}' failed: {}", node, source);
            Err(Error::NodeExecution { node, source })  // Propagate
        }
        Err(e) => Err(e),  // Propagate all other errors
    }
}
```

**Pattern 3: Convert NodeExecution Errors (Add Context)**

Wrap underlying errors with node context:

```rust
use dashflow::{Node, Error};

struct MyNode;

#[async_trait::async_trait]
impl Node<State> for MyNode {
    async fn execute(&self, state: State) -> Result<State> {
        // Call function that might fail
        let data = fetch_data().await.map_err(|e| Error::NodeExecution {
            node: "my_node".to_string(),
            source: Box::new(e),  // Wrap underlying error
        })?;

        Ok(state.with_data(data))
    }
}
```

**Pattern 4: Validation Errors (Graph Construction)**

Return validation errors during graph building:

```rust
use dashflow::{StateGraph, Error, Result};

fn build_graph(config: &Config) -> Result<CompiledGraph<State>> {
    let mut graph = StateGraph::new();

    // Validate configuration
    if config.nodes.is_empty() {
        return Err(Error::Validation("Configuration must have at least one node".to_string()));
    }

    // Add nodes and edges...
    graph.compile()
}
```

**Pattern 5: Generic Errors (Custom Messages)**

Use Generic variant for application-specific errors:

```rust
use dashflow::Error;

fn validate_state(state: &State) -> Result<()> {
    if state.messages.is_empty() {
        return Err(Error::Generic("State must have at least one message".to_string()));
    }

    if state.iteration > 1000 {
        return Err(Error::Generic("Iteration count too high (> 1000)".to_string()));
    }

    Ok(())
}
```

**Error Messages:**

All errors provide helpful Display messages:

```rust
// Validation error
Error::Validation("missing nodes".to_string())
// Output: "Graph validation error: missing nodes"

// NodeExecution error
Error::NodeExecution {
    node: "llm_call".to_string(),
    source: Box::new(reqwest::Error::...),
}
// Output: "Node execution error in 'llm_call': <underlying error message>"

// NoEntryPoint error
Error::NoEntryPoint
// Output: "Graph has no entry point defined"

// NodeNotFound error
Error::NodeNotFound("missing_node".to_string())
// Output: "Node 'missing_node' not found in graph"

// CycleDetected error
Error::CycleDetected("A -> B -> C -> A".to_string())
// Output: "Cycle detected in graph: A -> B -> C -> A"

// InvalidEdge error
Error::InvalidEdge("edge from nonexistent node".to_string())
// Output: "Invalid edge: edge from nonexistent node"

// Timeout error
Error::Timeout(Duration::from_secs(30))
// Output: "Execution timeout after 30s"

// InterruptWithoutCheckpointer error
Error::InterruptWithoutCheckpointer("node_a".to_string())
// Output: "Cannot interrupt at node 'node_a' without a checkpointer configured. Use with_checkpointer() before compiling the graph."

// InterruptWithoutThreadId error
Error::InterruptWithoutThreadId("node_a".to_string())
// Output: "Cannot interrupt at node 'node_a' without a thread_id configured. Use with_thread_id() before invoking the graph."

// ResumeWithoutCheckpointer error
Error::ResumeWithoutCheckpointer
// Output: "Cannot resume without a checkpointer configured. Use with_checkpointer() before compiling the graph."

// ResumeWithoutThreadId error
Error::ResumeWithoutThreadId
// Output: "Cannot resume without a thread_id configured. Use with_thread_id() before invoking the graph."

// NoCheckpointToResume error
Error::NoCheckpointToResume("thread_123".to_string())
// Output: "No checkpoint found to resume from for thread_id: thread_123"

// RecursionLimit error
Error::RecursionLimit { limit: 100 }
// Output: "Recursion limit of 100 reached. Graph execution exceeded maximum number of steps. This may indicate an infinite loop. Use with_recursion_limit() to increase the limit if needed."

// Generic error
Error::Generic("custom error message".to_string())
// Output: "custom error message"
```

**Common Error Scenarios:**

**Scenario 1: Missing Entry Point**

```rust
// BAD: No entry point defined
let mut graph = StateGraph::new();
graph.add_node("start", start_node);
let app = graph.compile()?;  // ❌ Error: NoEntryPoint

// GOOD: Set entry point
let mut graph = StateGraph::new();
graph.add_node("start", start_node);
graph.set_entry_point("start")?;
let app = graph.compile()?;  // ✅ Success
```

**Scenario 2: Invalid Edge References**

```rust
// BAD: Edge to non-existent node
let mut graph = StateGraph::new();
graph.add_node("start", start_node);
graph.add_edge("start", "missing_node")?;  // ❌ Error: InvalidEdge

// GOOD: Add both nodes first
let mut graph = StateGraph::new();
graph.add_node("start", start_node);
graph.add_node("end", end_node);
graph.add_edge("start", "end")?;  // ✅ Success
```

**Scenario 3: Conditional Edge to Missing Node**

```rust
// BAD: Conditional edge returns non-existent node
graph.add_conditional_edge("router", |state: &State| {
    if state.score > 0.5 { "high".to_string() }  // Node "high" doesn't exist
    else { "low".to_string() }
})?;

let result = app.invoke(state).await?;  // ❌ Error: NodeNotFound("high")

// GOOD: Ensure all target nodes exist
graph.add_node("high", high_node);
graph.add_node("low", low_node);
graph.add_conditional_edge("router", |state: &State| {
    if state.score > 0.5 { "high".to_string() }
    else { "low".to_string() }
})?;

let result = app.invoke(state).await?;  // ✅ Success
```

**Scenario 4: Infinite Loop (Recursion Limit)**

```rust
// BAD: ReAct loop doesn't terminate
graph.add_node("agent", agent_node);
graph.add_conditional_edge("agent", |state: &State| {
    "agent".to_string()  // Always routes back to self (infinite loop)
})?;

let result = app.invoke(state).await?;  // ❌ Error: RecursionLimit { limit: 100 }

// GOOD: Add termination condition
graph.add_conditional_edge("agent", |state: &State| {
    if state.iteration > 10 || state.done {
        "end".to_string()  // Exit loop
    } else {
        "agent".to_string()  // Continue loop
    }
})?;

let result = app.invoke(state).await?;  // ✅ Success
```

**Scenario 5: Interrupt Without Checkpointer**

```rust
// BAD: Interrupt without checkpointer
graph.add_node("approval", approval_node);
graph.interrupt_before("approval")?;
let app = graph.compile()?;  // No checkpointer configured

let result = app.invoke(state).await?;  // ❌ Error: InterruptWithoutCheckpointer("approval")

// GOOD: Configure checkpointer before compiling
graph.add_node("approval", approval_node);
graph.interrupt_before("approval")?;
let app = graph.compile()?
    .with_checkpointer(MemoryCheckpointer::new());

let result = app
    .with_thread_id("thread_123")
    .invoke(state)
    .await?;  // ✅ Success (execution paused at approval)
```

**Scenario 6: Node Execution Error (HTTP Request Fails)**

```rust
// Node that makes HTTP request
async fn fetch_node(state: State) -> Result<State> {
    let response = reqwest::get("https://api.example.com/data").await
        .map_err(|e| Error::NodeExecution {
            node: "fetch_node".to_string(),
            source: Box::new(e),
        })?;

    let data = response.json().await
        .map_err(|e| Error::NodeExecution {
            node: "fetch_node".to_string(),
            source: Box::new(e),
        })?;

    Ok(state.with_data(data))
}

// Error propagates with node context
let result = app.invoke(state).await;  // Error: NodeExecution { node: "fetch_node", source: ... }
```

**Error Size and Performance:**

Error enum is compact (<128 bytes) for efficient stack allocation:

```rust
use std::mem::size_of;
assert!(size_of::<Error>() < 128);
```

Most variants are zero-cost (no heap allocation). NodeExecution uses Box<dyn Error> for source errors (one heap allocation per error).

**Thread Safety:**

All errors implement Send + Sync:

```rust
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

assert_send::<Error>();
assert_sync::<Error>();
```

Enables safe error propagation across thread boundaries in async/await and parallel execution.

**upstream Python DashFlow Compatibility:**

| Error Type | upstream Python DashFlow | DashFlow | Notes |
|------------|------------------|----------------|-------|
| Graph validation errors | `ValueError` | `Error::Validation` | Similar validation checks (missing entry, invalid edges) |
| Node execution errors | Underlying exception | `Error::NodeExecution` | Rust preserves source error with Box<dyn Error> |
| Missing entry point | `ValueError` | `Error::NoEntryPoint` | Both check entry point before execution |
| Node not found | `KeyError` | `Error::NodeNotFound` | Both handle missing node lookups |
| Cycle detection | `ValueError` | `Error::CycleDetected` | Both detect cycles (when disallowed) |
| Execution timeout | `TimeoutError` | `Error::Timeout` | Both support configurable timeouts |
| Recursion limit | `RecursionError` | `Error::RecursionLimit` | Both prevent infinite loops with iteration counts |
| Checkpointing errors | `ValueError` | `Error::InterruptWithout*` | Rust has more specific checkpoint error variants |
| Serialization errors | `TypeError`, `ValueError` | `Error::Serialization` | Rust uses serde, Python uses pickle |

**Key Difference:** Rust's type system enables compile-time error handling (Result<T> forces error checking), while Python uses runtime exceptions (can be missed). Rust errors provide stronger guarantees (exhaustive match ensures all variants handled).

**Best Practices:**

1. **Use `?` operator:** Idiomatic Rust error propagation (concise, readable)
2. **Match on variants:** Handle specific errors differently (retry on timeout, log validation errors)
3. **Preserve context:** Wrap underlying errors with NodeExecution (include node name for debugging)
4. **Validate early:** Return Validation errors during graph construction (fail fast, before execution)
5. **Helpful messages:** Include relevant context in error strings (node names, durations, thread_ids)
6. **Test error paths:** Write tests for error scenarios (missing entry point, invalid edges, timeouts)
7. **Don't panic:** Use Result<T> return types, not panic!() (recoverable errors)
8. **Document errors:** Note which errors functions can return (enables caller handling)

**Common Pitfalls:**

**Pitfall 1: Ignoring Errors (Using `unwrap()` or `expect()`)**

```rust
// BAD: Panic on error (non-recoverable)
let app = graph.compile().unwrap();  // ❌ Panics if validation fails

// GOOD: Propagate error (recoverable)
let app = graph.compile()?;  // ✅ Returns Error to caller
```

**Pitfall 2: Losing Error Context (Not Wrapping Underlying Errors)**

```rust
// BAD: Lose underlying error details
async fn node(state: State) -> Result<State> {
    let data = fetch_data().await
        .map_err(|_| Error::Generic("fetch failed".to_string()))?;  // ❌ Lost error details
    Ok(state)
}

// GOOD: Preserve error context
async fn node(state: State) -> Result<State> {
    let data = fetch_data().await
        .map_err(|e| Error::NodeExecution {
            node: "node".to_string(),
            source: Box::new(e),  // ✅ Preserved underlying error
        })?;
    Ok(state)
}
```

**Pitfall 3: Not Checking Checkpointer Configuration**

```rust
// BAD: Interrupt without checking if checkpointer exists
graph.interrupt_before("approval")?;
let app = graph.compile()?;  // No checkpointer configured
// Runtime error: InterruptWithoutCheckpointer

// GOOD: Configure checkpointer if using interrupts
if config.enable_interrupts {
    graph.interrupt_before("approval")?;
    app = app.with_checkpointer(MemoryCheckpointer::new());
}
```

**Pitfall 4: Conditional Edges to Missing Nodes**

```rust
// BAD: Conditional edge returns non-existent node
graph.add_conditional_edge("router", |state| {
    state.next.clone()  // User sets state.next to arbitrary string
})?;
// Runtime error: NodeNotFound if state.next is invalid

// GOOD: Validate node names
graph.add_conditional_edge("router", |state| {
    match state.next.as_str() {
        "node_a" | "node_b" | "node_c" => state.next.clone(),
        _ => "default".to_string(),  // Fallback to valid node
    }
})?;
```

**Code Pointers:**

- Error module: `crates/dashflow/src/error.rs`
- Error enum definition: `crates/dashflow/src/error.rs:323-438`
- Result type alias: `crates/dashflow/src/error.rs:441`
- Tests: `crates/dashflow/src/error.rs:919-1832` (66 tests)
- Error usage in executor: `crates/dashflow/src/executor/execution.rs` (NodeNotFound: 574, 690, 1127, 1521; RecursionLimit: 1115; Interrupt errors: 1145, 1149, 1291, 1297)
- Error usage in graph: `crates/dashflow/src/graph.rs` (validation errors)
- Error usage in nodes: `crates/dashflow/src/node.rs:839` (NodeExecution example)
- Error usage in subgraph: `crates/dashflow/src/subgraph.rs` (error propagation)

---

### DashStream Callback (Telemetry Integration)

**Location:** `crates/dashflow/src/dashstream_callback/mod.rs`

DashStreamCallback provides production-grade telemetry integration for DashFlow workflows by streaming graph execution events and state changes to Kafka using the DashFlow Streaming protocol. It enables real-time observability, debugging, and performance analysis of complex graph executions in production environments.

**Core Concept:**

Telemetry is essential for understanding graph behavior in production. DashStreamCallback automatically captures:
- **Graph lifecycle events:** Start, end, node execution, edge traversal, parallel execution
- **State changes:** Incremental state diffs (not full state) for efficiency
- **Performance metrics:** Execution duration for nodes, edges, and entire graph
- **Error tracking:** Node failures, validation errors, timeout events
- **Thread correlation:** All events tagged with unique thread_id for multi-tenant isolation

This data streams to Kafka in real-time, enabling:
- **Live debugging:** Watch graph execution as it happens
- **Performance analysis:** Identify slow nodes, optimization opportunities
- **Error diagnosis:** Trace failure paths, inspect state at failure point
- **Usage analytics:** Track patterns, popular paths, success rates
- **Compliance:** Audit trails for regulated industries

**DashFlow Streaming Protocol:**

DashStream is a binary streaming protocol designed for AI workflow telemetry (see `docs/DASHSTREAM_PROTOCOL.md` and `proto/dashstream.proto`):
- **Ultra-efficient:** Protobuf encoding + compression (10-100× smaller than JSON)
- **Diff-based:** Only transmits state changes, not full state (90%+ reduction)
- **Kafka-native:** Designed for high-throughput distributed streaming
- **Schema-versioned:** Protobuf enables schema evolution without breaking consumers
- **Multi-tenant:** Tenant ID and thread ID isolation for shared infrastructure

**Key Features:**

| Feature | Description | Benefit |
|---------|-------------|---------|
| **Event Streaming** | All GraphEvent types sent to Kafka | Complete visibility into graph execution |
| **State Diffing** | JSON patch diffs instead of full state | 90%+ bandwidth reduction for large states |
| **Async Publishing** | Non-blocking event sends (tokio::spawn) | Zero performance impact on graph execution |
| **Thread Isolation** | Unique thread_id per graph invocation | Multi-tenant observability (trace single conversation) |
| **Configurable** | Custom Kafka brokers, topics, compression | Flexible deployment (localhost, cloud, on-prem) |
| **Flush Guarantees** | Explicit flush() ensures delivery | Test verification (events actually reached Kafka) |

**Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│ DashFlow Executor                                          │
│                                                             │
│  ┌─────────────┐      ┌─────────────┐      ┌──────────┐   │
│  │   Node A    │ ───> │   Node B    │ ───> │  Node C  │   │
│  └─────────────┘      └─────────────┘      └──────────┘   │
│         │                    │                    │         │
│         │ GraphEvent         │ GraphEvent         │         │
│         ▼                    ▼                    ▼         │
│  ┌──────────────────────────────────────────────────────┐  │
│  │         DashStreamCallback (EventCallback)           │  │
│  │  - Sequence numbers (0, 1, 2, ...)                   │  │
│  │  - State diffing (old → new)                          │  │
│  │  - Event conversion (GraphEvent → DashStream Event)   │  │
│  └──────────────────────────────────────────────────────┘  │
│                            │                                │
└────────────────────────────┼────────────────────────────────┘
                             │ Protobuf Messages
                             ▼
                    ┌─────────────────────┐
                    │  Kafka Topic        │
                    │  dashstream-events  │
                    └─────────────────────┘
                             │
                             ▼
            ┌────────────────────────────────────┐
            │  Consumers (Parallel Processing)   │
            │  - Dashboard (real-time UI)        │
            │  - Analytics (aggregation)         │
            │  - Debugger (event replay)         │
            │  - Storage (long-term retention)   │
            └────────────────────────────────────┘
```

**DashStreamConfig:**

Configuration for DashStream callback connection and behavior:

```rust
pub struct DashStreamConfig {
    /// Kafka bootstrap servers (comma-separated)
    pub bootstrap_servers: String,  // "localhost:9092" or "kafka1:9092,kafka2:9092"

    /// Kafka topic name
    pub topic: String,  // "dashstream-events"

    /// Tenant ID for multi-tenancy
    pub tenant_id: String,  // "acme-corp" (organization identifier)

    /// Thread/session ID (unique per graph invocation)
    pub thread_id: String,  // "conv-123" or UUID

    /// Enable state diffing (default: true)
    pub enable_state_diff: bool,  // false = no state diffs sent (events only)

    /// Compression threshold in bytes (default: 512)
    pub compression_threshold: usize,  // Compress states larger than threshold
}
```

**Default Configuration:**
- `bootstrap_servers`: `"localhost:9092"` (local Kafka)
- `topic`: `"dashstream-events"` (standard topic name)
- `tenant_id`: `"default"` (single-tenant mode)
- `thread_id`: UUID v4 (random unique ID)
- `enable_state_diff`: `true` (diffing enabled)
- `compression_threshold`: `512` bytes

**DashStreamCallback:**

Main callback type implementing EventCallback trait:

```rust
pub struct DashStreamCallback<S>
where
    S: GraphState + Serialize,
{
    producer: Arc<DashStreamProducer>,  // Kafka producer (shared across threads)
    config: DashStreamConfig,           // Configuration
    sequence: Arc<Mutex<u64>>,          // Monotonic sequence counter (0, 1, 2, ...)
    previous_state: Arc<Mutex<Option<serde_json::Value>>>,  // Previous state for diffing
    _phantom: std::marker::PhantomData<S>,
}
```

**Public API:**

**1. Constructor (Simple):**

```rust
pub async fn new(
    bootstrap_servers: &str,
    topic: &str,
    tenant_id: &str,
    thread_id: &str,
) -> Result<Self, Box<dyn std::error::Error>>
```

Creates callback with default configuration (state diffing enabled, 512-byte compression threshold).

**Example:**
```rust
let callback = DashStreamCallback::<AgentState>::new(
    "localhost:9092",           // Kafka brokers
    "dashstream-events",        // Topic
    "acme-corp",                // Tenant
    "conversation-123"          // Thread
).await?;
```

**2. Constructor (Custom Config):**

```rust
pub async fn with_config(config: DashStreamConfig)
    -> Result<Self, Box<dyn std::error::Error>>
```

Creates callback with custom configuration (full control over behavior).

**Example:**
```rust
let config = DashStreamConfig {
    bootstrap_servers: "kafka.prod.example.com:9093".to_string(),
    topic: "prod-events".to_string(),
    tenant_id: "customer-42".to_string(),
    thread_id: format!("session-{}", uuid::Uuid::new_v4()),
    enable_state_diff: true,
    compression_threshold: 1024,  // Compress states >1KB
};

let callback = DashStreamCallback::<MyState>::with_config(config).await?;
```

**3. Flush (Delivery Guarantee):**

```rust
pub async fn flush(&self) -> Result<(), Box<dyn std::error::Error>>
```

Blocks until all buffered events are written to Kafka (5-second timeout). Essential for tests to verify event delivery.

**Example:**
```rust
// Run graph with callback
compiled_graph.invoke_with_callback(state, &callback).await?;

// Ensure all events written to Kafka
callback.flush().await?;
println!("All telemetry events delivered to Kafka");
```

**Event Types (GraphEvent → DashStream Event):**

DashStreamCallback converts GraphEvent to DashStream Event messages:

| GraphEvent | DashStream EventType | node_id | duration_us | State Diff |
|------------|---------------------|---------|-------------|------------|
| `GraphStart` | `GRAPH_START` | `""` (empty) | `0` | Initial state stored (baseline for diffs) |
| `GraphEnd` | `GRAPH_END` | `""` (empty) | Total duration | Final state diff (initial → final) |
| `NodeStart` | `NODE_START` | Node name | `0` | No diff |
| `NodeEnd` | `NODE_END` | Node name | Node duration | State diff (before → after node) |
| `NodeError` | `NODE_ERROR` | Node name | `0` | No diff |
| `EdgeTraversal` (simple) | `EDGE_TRAVERSAL` | From node | `0` | No diff |
| `EdgeTraversal` (conditional) | `CONDITIONAL_BRANCH` | From node | `0` | No diff |
| `ParallelStart` | `PARALLEL_START` | Comma-joined nodes | `0` | No diff |
| `ParallelEnd` | `PARALLEL_END` | Comma-joined nodes | Parallel duration | No diff |

**State Diffing (Incremental Updates):**

DashStreamCallback uses JSON Patch (RFC 6902) to send only state changes:

**Example:**
```rust
// Initial state
{"messages": ["Hello"], "iteration": 0}

// After node execution
{"messages": ["Hello", "Response"], "iteration": 1}

// State diff sent (JSON Patch):
[
  {"op": "add", "path": "/messages/1", "value": "Response"},
  {"op": "replace", "path": "/iteration", "value": 1}
]
```

**Benefits:**
- **Bandwidth reduction:** 90%+ for large states (only send changed fields)
- **Faster transmission:** Smaller messages reach Kafka faster
- **Incremental replay:** Consumers can reconstruct state from diffs
- **Compression-friendly:** JSON patches compress extremely well

**Configuration:**
- `enable_state_diff: true` → Send JSON patches (efficient)
- `enable_state_diff: false` → Send full state (simpler, larger)
- Automatic fallback: If diff generation fails, send full state

**When diffs are sent:**
- `GraphStart`: Initial state stored (no diff sent, baseline established)
- `NodeEnd`: Diff from state before node → state after node
- `GraphEnd`: Diff from initial state → final state

**Usage Patterns:**

**Pattern 1: Basic Integration (Development)**

```rust
use dashflow::dashstream_callback::DashStreamCallback;
use dashflow::StateGraph;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create callback (localhost Kafka)
    let callback = DashStreamCallback::<AgentState>::new(
        "localhost:9092",
        "dashstream-events",
        "dev",
        "test-session"
    ).await?;

    // Create and compile graph
    let mut graph = StateGraph::new();
    // ... add nodes and edges ...
    let compiled = graph.compile()?;

    // Invoke with telemetry
    let result = compiled.invoke_with_callback(initial_state, &callback).await?;

    // Ensure delivery
    callback.flush().await?;

    Ok(())
}
```

**Pattern 2: Production Integration (Multi-Tenant)**

```rust
use dashflow::dashstream_callback::{DashStreamCallback, DashStreamConfig};
use dashflow::StateGraph;

async fn handle_request(
    user_id: &str,
    session_id: &str,
    graph: &CompiledGraph<MyState>,
    state: MyState,
) -> Result<MyState, Box<dyn std::error::Error>> {
    // Create callback with tenant/thread isolation
    let config = DashStreamConfig {
        bootstrap_servers: std::env::var("KAFKA_BROKERS")?,
        topic: "production-events".to_string(),
        tenant_id: user_id.to_string(),  // Isolate by user
        thread_id: session_id.to_string(),  // Isolate by session
        enable_state_diff: true,
        compression_threshold: 1024,
    };

    let callback = DashStreamCallback::with_config(config).await?;

    // Invoke with telemetry
    let result = graph.invoke_with_callback(state, &callback).await?;

    // Flush (optional in production, adds latency)
    // callback.flush().await?;

    Ok(result)
}
```

**Pattern 3: Test Verification (CI/CD)**

```rust
#[tokio::test]
#[ignore]  // Requires: Kafka running
async fn test_graph_telemetry() -> Result<(), Box<dyn std::error::Error>> {
    // Start Kafka: docker-compose -f docker-compose-kafka.yml up -d

    // Create callback with unique thread ID
    let thread_id = format!("test-{}", uuid::Uuid::new_v4());
    let callback = DashStreamCallback::<AgentState>::new(
        "localhost:9092",
        "dashstream-events",
        "test-tenant",
        &thread_id
    ).await?;

    // Run graph
    let graph = create_test_graph();
    let result = graph.invoke_with_callback(initial_state, &callback).await?;

    // Verify delivery (CRITICAL: Without flush, events might buffer)
    callback.flush().await?;

    // Verify result
    assert_eq!(result.messages.len(), 3);

    // Optional: Consume Kafka events to verify content
    // (requires Kafka consumer implementation)

    Ok(())
}
```

**Pattern 4: Conditional Telemetry (Feature Flag)**

```rust
use dashflow::StateGraph;
use dashflow::dashstream_callback::DashStreamCallback;

async fn run_graph_with_optional_telemetry(
    state: MyState,
    enable_telemetry: bool,
) -> Result<MyState, Box<dyn std::error::Error>> {
    let graph = create_graph();

    if enable_telemetry {
        // With telemetry
        let callback = DashStreamCallback::new(
            "localhost:9092",
            "dashstream-events",
            "tenant-1",
            "session-abc"
        ).await?;

        graph.invoke_with_callback(state, &callback).await
    } else {
        // Without telemetry (no overhead)
        graph.invoke(state).await
    }
}
```

**Pattern 5: Error Handling (Kafka Unavailable)**

```rust
use dashflow::dashstream_callback::DashStreamCallback;
use dashflow::StateGraph;

async fn run_with_fallback_telemetry(
    state: MyState,
) -> Result<MyState, Box<dyn std::error::Error>> {
    let graph = create_graph();

    // Try to create callback (might fail if Kafka down)
    match DashStreamCallback::new(
        "localhost:9092",
        "dashstream-events",
        "tenant-1",
        "session-123"
    ).await {
        Ok(callback) => {
            // Kafka available: Use telemetry
            println!("Telemetry enabled");
            graph.invoke_with_callback(state, &callback).await
        }
        Err(e) => {
            // Kafka unavailable: Proceed without telemetry
            eprintln!("Warning: Telemetry unavailable ({}), proceeding without", e);
            graph.invoke(state).await
        }
    }
}
```

**EventCallback Implementation:**

DashStreamCallback implements the EventCallback trait:

```rust
impl<S> EventCallback<S> for DashStreamCallback<S>
where
    S: GraphState + Serialize,
{
    fn on_event(&self, event: &GraphEvent<S>) {
        self.send_graph_event(event);
    }
}
```

**Behavior:**
- **Non-blocking:** Events sent asynchronously (`tokio::spawn`)
- **Fire-and-forget:** No wait for Kafka acknowledgment (low latency)
- **Buffered:** Kafka producer buffers events (flush() to force write)
- **Error handling:** Kafka send errors logged but don't block graph execution

**Performance Characteristics:**

| Metric | Value | Notes |
|--------|-------|-------|
| **Overhead per event** | <1ms | Async send, no blocking |
| **State diff time** | 1-10ms | Depends on state size (O(n) where n=fields) |
| **Memory overhead** | ~500 bytes/event | Protobuf encoding |
| **Bandwidth (with diffs)** | 100-500 bytes/event | 90%+ reduction vs full state |
| **Bandwidth (without diffs)** | 1-50 KB/event | Full state serialization |
| **Flush latency** | 5-50ms | Depends on Kafka RTT |

**Best Practices:**

**1. Use Unique Thread IDs:**
```rust
// GOOD: Unique per invocation (traceable)
let thread_id = format!("conv-{}", uuid::Uuid::new_v4());

// BAD: Same thread_id for multiple invocations (events mixed)
let thread_id = "static-id";  // Multiple graphs share same ID
```

**2. Flush in Tests, Not Production:**
```rust
// Tests: Flush to verify delivery
callback.flush().await?;

// Production: Don't flush (adds latency, Kafka batching handles delivery)
// callback.flush().await?;  // <-- Remove in production
```

**3. Enable State Diffing for Large States:**
```rust
// Large states (>1 KB): Enable diffing
let config = DashStreamConfig {
    enable_state_diff: true,  // 90%+ reduction
    ..Default::default()
};

// Small states (<100 bytes): Disable diffing (overhead not worth it)
let config = DashStreamConfig {
    enable_state_diff: false,  // Full state simpler
    ..Default::default()
};
```

**4. Use Tenant IDs for Multi-Tenancy:**
```rust
// GOOD: Tenant isolation (users can't see each other's events)
let tenant_id = user.organization_id.clone();

// BAD: Shared tenant (no isolation, privacy risk)
let tenant_id = "default";
```

**5. Handle Kafka Failures Gracefully:**
```rust
// GOOD: Proceed without telemetry if Kafka down
let callback = match DashStreamCallback::new(...).await {
    Ok(cb) => Some(cb),
    Err(e) => {
        log::warn!("Telemetry unavailable: {}", e);
        None
    }
};

// Invoke with optional callback
if let Some(ref cb) = callback {
    graph.invoke_with_callback(state, cb).await?
} else {
    graph.invoke(state).await?
}
```

**Common Scenarios:**

**Scenario 1: Real-Time Debugging**

**Problem:** Graph fails in production, need to see execution path and state at failure.

**Solution:**
1. Enable DashFlow Streaming telemetry in production
2. Graph fails → events stream to Kafka
3. Consumer reads events, reconstructs execution:
   - NodeStart → NodeStart → NodeError (failure point)
   - State diffs show state at each step
4. Identify failure: Node X with state Y caused error Z

**Code:**
```rust
// Production graph with telemetry
let callback = DashStreamCallback::new(
    kafka_brokers,
    "production-events",
    customer_id,
    session_id
).await?;

// Graph executes, events stream to Kafka
graph.invoke_with_callback(state, &callback).await?;

// On failure: Consumer reads events from Kafka
// Reconstructs execution path, state at each step
// Identifies failure point: "Node X failed with error Y at state Z"
```

**Scenario 2: Performance Analysis**

**Problem:** Graph slow in production, need to identify bottleneck nodes.

**Solution:**
1. DashStream captures duration_us for each node
2. Consumer aggregates durations across invocations
3. Identify slow nodes: Node A (avg 2000ms), Node B (avg 50ms)
4. Optimize Node A (caching, parallelism, etc.)

**Code:**
```rust
// Events include duration_us
GraphEvent::NodeEnd { duration, .. } → Event { duration_us: 2_000_000 }

// Consumer aggregates:
// SELECT node_id, AVG(duration_us) FROM events GROUP BY node_id
// Result: Node A = 2000ms (bottleneck), Node B = 50ms (fast)
```

**Scenario 3: Multi-Turn Conversation Tracking**

**Problem:** Chatbot has multi-turn conversations, need to trace entire conversation history.

**Solution:**
1. Use same thread_id for all turns in conversation
2. Each turn generates events with same thread_id
3. Consumer filters by thread_id → full conversation trace

**Code:**
```rust
// Turn 1
let thread_id = format!("conv-{}", uuid::Uuid::new_v4());
let callback = DashStreamCallback::new(kafka, topic, tenant, &thread_id).await?;
graph.invoke_with_callback(turn1_state, &callback).await?;

// Turn 2 (same thread_id)
let callback = DashStreamCallback::new(kafka, topic, tenant, &thread_id).await?;
graph.invoke_with_callback(turn2_state, &callback).await?;

// Consumer: SELECT * FROM events WHERE thread_id = 'conv-123' ORDER BY sequence
// Result: Complete conversation trace (all turns, all events, in order)
```

**Comparison to upstream Python DashFlow:**

| Feature | Rust DashStreamCallback | upstream Python DashFlow | Notes |
|---------|------------------------|------------------|-------|
| **Telemetry Protocol** | DashStream (Protobuf) | None (custom callbacks) | Rust has production-grade protocol |
| **Kafka Integration** | Built-in (dashflow_streaming) | Manual (kafka-python) | Rust more turnkey |
| **State Diffing** | Automatic (JSON Patch) | Manual | Rust 90%+ bandwidth reduction |
| **Performance** | Async, non-blocking | Sync (blocking) | Rust zero overhead |
| **Type Safety** | Compile-time (S: GraphState) | Runtime | Rust catches errors at compile time |
| **Multi-Tenancy** | Built-in (tenant_id, thread_id) | Manual | Rust designed for multi-tenant |
| **Schema Evolution** | Protobuf schema versioning | N/A | Rust supports backward compatibility |
| **Compression** | Automatic (zstd, lz4, snappy) | Manual | Rust built-in compression |

**Python Equivalent:**

upstream Python DashFlow uses custom callbacks (no standard telemetry protocol):

```python
# Python: Custom callback (manual implementation)
class KafkaCallback:
    def on_graph_start(self, state):
        kafka_producer.send("events", {"type": "graph_start", "state": state})

    def on_node_end(self, node, state, duration):
        kafka_producer.send("events", {"type": "node_end", "node": node, "duration": duration})

# Usage
graph.invoke(state, callbacks=[KafkaCallback()])
```

**Rust Advantage:**
- **Standardized protocol:** DashFlow Streaming protocol (Protobuf) vs ad-hoc JSON
- **Built-in diffing:** Automatic state diffs (90%+ reduction) vs full state
- **Production-ready:** Multi-tenancy, compression, schema versioning built-in
- **Performance:** Async, non-blocking (zero overhead) vs sync (blocks graph)

**Common Pitfalls:**

**Pitfall 1: Forgetting to Flush in Tests**

**BAD:**
```rust
#[tokio::test]
async fn test_telemetry() -> Result<(), Box<dyn std::error::Error>> {
    let callback = DashStreamCallback::new(...).await?;
    graph.invoke_with_callback(state, &callback).await?;
    // Test passes, but events might still be buffered (not written to Kafka)
    Ok(())
}
```

**GOOD:**
```rust
#[tokio::test]
async fn test_telemetry() -> Result<(), Box<dyn std::error::Error>> {
    let callback = DashStreamCallback::new(...).await?;
    graph.invoke_with_callback(state, &callback).await?;
    callback.flush().await?;  // Ensure delivery before test ends
    Ok(())
}
```

**Pitfall 2: Reusing Thread IDs**

**BAD:**
```rust
// Same thread_id for multiple invocations (events mixed)
let callback = DashStreamCallback::new(kafka, topic, tenant, "static-id").await?;
graph.invoke_with_callback(state1, &callback).await?;  // thread_id = "static-id"
graph.invoke_with_callback(state2, &callback).await?;  // thread_id = "static-id"
// Consumer can't distinguish invocations (events mixed)
```

**GOOD:**
```rust
// Unique thread_id per invocation (events isolated)
let thread_id1 = format!("inv-{}", uuid::Uuid::new_v4());
let callback1 = DashStreamCallback::new(kafka, topic, tenant, &thread_id1).await?;
graph.invoke_with_callback(state1, &callback1).await?;  // thread_id = "inv-abc"

let thread_id2 = format!("inv-{}", uuid::Uuid::new_v4());
let callback2 = DashStreamCallback::new(kafka, topic, tenant, &thread_id2).await?;
graph.invoke_with_callback(state2, &callback2).await?;  // thread_id = "inv-def"
```

**Pitfall 3: Blocking on Flush in Production**

**BAD:**
```rust
// Production: Flush adds latency (5-50ms per request)
let callback = DashStreamCallback::new(...).await?;
let result = graph.invoke_with_callback(state, &callback).await?;
callback.flush().await?;  // Adds latency, not necessary (Kafka batching handles delivery)
return result;
```

**GOOD:**
```rust
// Production: Don't flush (Kafka batching handles delivery)
let callback = DashStreamCallback::new(...).await?;
let result = graph.invoke_with_callback(state, &callback).await?;
// No flush (events delivered asynchronously via Kafka batching)
return result;
```

**Pitfall 4: Not Handling Kafka Unavailability**

**BAD:**
```rust
// Kafka down → entire request fails
let callback = DashStreamCallback::new(...).await?;  // Panics if Kafka down
graph.invoke_with_callback(state, &callback).await?;
```

**GOOD:**
```rust
// Kafka down → proceed without telemetry
let result = match DashStreamCallback::new(...).await {
    Ok(callback) => graph.invoke_with_callback(state, &callback).await?,
    Err(e) => {
        log::warn!("Telemetry unavailable: {}", e);
        graph.invoke(state).await?  // Proceed without telemetry
    }
};
```

**Testing:**

DashStreamCallback has comprehensive tests (see `dashstream_callback/mod.rs:2756+`):

**Unit Tests:**
- Config defaults and customization
- Sequence number generation (monotonic, thread-safe)
- Header creation (message_id, timestamp, tenant_id, thread_id)
- State diff creation (JSON Patch generation)

**Integration Tests (Require Kafka):**
- Event publishing to Kafka
- State diff publishing
- Multi-event sequences
- Flush guarantee

**Run Tests:**
```bash
# Start Kafka
docker-compose -f docker-compose-kafka.yml up -d

# Run tests
cargo test --package dashflow --lib dashstream_callback --features dashstream -- --nocapture

# Run integration tests (require Kafka)
cargo test --package dashflow --lib dashstream_callback --features dashstream -- --ignored --nocapture
```

**Code Pointers:**

- DashStream Callback module: `crates/dashflow/src/dashstream_callback/mod.rs` (2757 lines)
- DashStreamConfig struct: `crates/dashflow/src/dashstream_callback/mod.rs:726-784`
- DashStreamCallback struct: `crates/dashflow/src/dashstream_callback/mod.rs:961-995`
- Constructor (simple): `crates/dashflow/src/dashstream_callback/mod.rs:1002-1030`
- Constructor (custom config): `crates/dashflow/src/dashstream_callback/mod.rs:1033-1050`
- Flush method: `crates/dashflow/src/dashstream_callback/mod.rs` (see `flush_pending_tasks`)
- EventCallback implementation: `crates/dashflow/src/dashstream_callback/mod.rs:2691-2720`
- Event conversion logic: `crates/dashflow/src/dashstream_callback/mod.rs:1150-1500` (emit_event, create_event)
- State diffing logic: `crates/dashflow/src/dashstream_callback/mod.rs:1550-1700` (create_state_diff)
- Tests: `crates/dashflow/src/dashstream_callback/mod.rs:2756+` (unit + integration)
- DashFlow Streaming protocol spec: `docs/DASHSTREAM_PROTOCOL.md`
- Protobuf schema: `proto/dashstream.proto`
- DashStream producer: `crates/dashflow-streaming/src/producer.rs`
- State diffing algorithm: `crates/dashflow-streaming/src/diff.rs`
- Multi-turn test examples: See `examples/apps/librarian/tests/` for production multi-turn conversation test patterns

---

### Metrics (Performance Tracking)

**Location:** `crates/dashflow/src/metrics.rs`

ExecutionMetrics provides detailed performance tracking for graph executions, capturing node durations, execution counts, checkpoint operations, state sizes, and concurrency patterns. Metrics are automatically collected during graph execution and enable performance analysis, bottleneck identification, and optimization.

**Core Concept:**

Metrics enable observability into graph execution performance. When a graph executes:
1. **Automatic collection:** Executor records metrics for every operation (node execution, edge traversal, checkpoint save/load, parallel execution)
2. **Zero overhead:** Metrics collection uses in-memory counters and timers (negligible performance impact)
3. **Post-execution analysis:** Access metrics after graph completion to understand performance characteristics
4. **Optimization insights:** Identify slow nodes, excessive checkpoint operations, underutilized parallelism

This design enables:
- **Performance analysis:** Measure node durations to identify bottlenecks (slowest node, average duration, execution percentage)
- **Debugging:** Understand execution patterns (execution counts, edge traversals, conditional branches)
- **Capacity planning:** Track concurrency patterns (parallel executions, peak concurrency) for resource allocation
- **Optimization:** Compare metrics before/after changes to validate improvements

**ExecutionMetrics Struct:**

```rust
#[derive(Debug, Clone, Default)]
pub struct ExecutionMetrics {
    /// Duration per node (node_name -> duration)
    pub node_durations: HashMap<String, Duration>,

    /// Number of times each node was executed
    pub node_execution_counts: HashMap<String, usize>,

    /// Total execution duration (wall clock time)
    pub total_duration: Duration,

    /// Number of checkpoints saved
    pub checkpoint_count: usize,

    /// Number of checkpoints loaded
    pub checkpoint_loads: usize,

    /// State size in bytes (if serializable)
    pub state_size_bytes: Option<usize>,

    /// Number of edges traversed
    pub edges_traversed: usize,

    /// Number of conditional branches evaluated
    pub conditional_branches: usize,

    /// Number of parallel executions
    pub parallel_executions: usize,

    /// Peak number of concurrent nodes
    pub peak_concurrency: usize,

    /// Total number of events emitted
    pub events_emitted: usize,
}
```

**Field Descriptions:**

| Field | Type | Purpose | Example Value |
|-------|------|---------|---------------|
| `node_durations` | HashMap<String, Duration> | Accumulated duration per node (if node executes multiple times, durations summed) | `{"search": 150ms, "llm": 2000ms}` |
| `node_execution_counts` | HashMap<String, usize> | Number of times each node executed (loops, retries) | `{"search": 1, "llm": 3}` |
| `total_duration` | Duration | Wall clock time from start to end (includes parallel execution) | `2500ms` |
| `checkpoint_count` | usize | Number of checkpoints saved (persistence operations) | `5` |
| `checkpoint_loads` | usize | Number of checkpoints loaded (resume operations) | `2` |
| `state_size_bytes` | Option<usize> | State size in bytes (serialized state size for memory analysis) | `Some(4096)` |
| `edges_traversed` | usize | Number of edges traversed (graph complexity indicator) | `10` |
| `conditional_branches` | usize | Number of conditional branches evaluated (routing complexity) | `3` |
| `parallel_executions` | usize | Number of parallel executions (concurrency events) | `2` |
| `peak_concurrency` | usize | Maximum concurrent nodes (resource utilization) | `4` |
| `events_emitted` | usize | Total events emitted (telemetry volume) | `25` |

**Public API:**

**1. Accessors (Read-Only):**
```rust
// Get average node execution time
fn average_node_duration(&self) -> Duration

// Get slowest node (bottleneck identification)
fn slowest_node(&self) -> Option<(&str, Duration)>

// Get node execution percentage (relative performance)
fn node_percentage(&self, node_name: &str) -> f64

// Format metrics as human-readable string
fn to_string_pretty(&self) -> String
```

**2. Usage Pattern (Automatic Collection):**
```rust
use dashflow::{StateGraph, GraphState};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct MyState {
    input: String,
    output: String,
}

let mut graph = StateGraph::<MyState>::new();
graph.add_node("process", |state: MyState| async move {
    // ... node logic ...
    Ok(state)
});

let app = graph.compile()?;

// Metrics automatically collected during execution
let result = app.invoke(state).await?;

// Access metrics after completion
let metrics = app.metrics();

println!("{}", metrics.to_string_pretty());
println!("Slowest node: {:?}", metrics.slowest_node());
println!("Average duration: {:?}", metrics.average_node_duration());
```

**Analysis Methods:**

**1. average_node_duration():**
Calculates average duration across all nodes. Useful for understanding typical node performance.

```rust
let avg = metrics.average_node_duration();
println!("Average node execution: {:?}", avg);
// Output: Average node execution: 75ms
```

**Formula:** `sum(node_durations) / count(nodes)`

**2. slowest_node():**
Identifies bottleneck node with longest cumulative duration. Critical for optimization.

```rust
if let Some((name, duration)) = metrics.slowest_node() {
    println!("Bottleneck: {} ({:?})", name, duration);
    // Output: Bottleneck: llm_call (2000ms)
}
```

**3. node_percentage():**
Calculates percentage of total execution time spent in specific node. Identifies relative impact.

```rust
let pct = metrics.node_percentage("llm_call");
println!("LLM time: {:.1}% of total", pct);
// Output: LLM time: 80.0% of total
```

**Formula:** `(node_duration / total_duration) * 100`

**4. to_string_pretty():**
Formats all metrics as human-readable report. Ideal for logging and debugging.

```rust
println!("{}", metrics.to_string_pretty());
```

**Output Example:**
```
Execution Metrics:
  Total Duration: 2500ms
  Edges Traversed: 10
  Checkpoints: 5 saved, 2 loaded
  State Size: 4096 bytes

  Node Durations:
    llm_call              2000ms ( 80.00%) [3 calls]
    search                 150ms (  6.00%) [1 calls]
    format                  50ms (  2.00%) [1 calls]

  Conditional Branches: 3
  Parallel Executions: 2 (peak concurrency: 4)
```

**Common Use Cases:**

**Use Case 1: Identify Performance Bottlenecks**

Find slowest node to optimize:

```rust
let result = app.invoke(state).await?;
let metrics = app.metrics();

// Find bottleneck
if let Some((name, duration)) = metrics.slowest_node() {
    let pct = metrics.node_percentage(name);
    println!("Bottleneck: {} takes {:.1}% of total time ({:?})", name, pct, duration);

    if pct > 50.0 {
        println!("⚠️  Critical: {} dominates execution. Consider optimization.", name);
    }
}

// Output:
// Bottleneck: llm_call takes 80.0% of total time (2000ms)
// ⚠️  Critical: llm_call dominates execution. Consider optimization.
```

**Use Case 2: Detect Inefficient Loops**

Identify nodes executing more times than expected:

```rust
let metrics = app.metrics();

for (node, count) in &metrics.node_execution_counts {
    if *count > 10 {
        let duration = metrics.node_durations.get(node).unwrap();
        println!("⚠️  {} executed {} times (total {:?})", node, count, duration);
    }
}

// Output:
// ⚠️  retry_node executed 15 times (total 1500ms)
// Suggests: Retry logic might be too aggressive or error condition not resolved
```

**Use Case 3: Monitor Checkpoint Overhead**

Track checkpoint save/load operations to assess persistence cost:

```rust
let metrics = app.metrics();

println!("Checkpoints: {} saved, {} loaded",
    metrics.checkpoint_count,
    metrics.checkpoint_loads);

if metrics.checkpoint_count > 100 {
    println!("⚠️  High checkpoint volume. Consider coarser checkpointing.");
}

// Output:
// Checkpoints: 150 saved, 0 loaded
// ⚠️  High checkpoint volume. Consider coarser checkpointing.
```

**Use Case 4: Analyze Parallelism Utilization**

Understand concurrency patterns to optimize resource allocation:

```rust
let metrics = app.metrics();

println!("Parallel executions: {}", metrics.parallel_executions);
println!("Peak concurrency: {}", metrics.peak_concurrency);

let avg_concurrency = metrics.peak_concurrency as f64 / metrics.parallel_executions.max(1) as f64;
println!("Average concurrency: {:.1}", avg_concurrency);

if avg_concurrency < 2.0 {
    println!("ℹ️  Low concurrency. Consider adding parallel edges for better throughput.");
}

// Output:
// Parallel executions: 5
// Peak concurrency: 4
// Average concurrency: 0.8
// ℹ️  Low concurrency. Consider adding parallel edges for better throughput.
```

**Use Case 5: Compare Before/After Optimization**

Validate optimization improvements with metrics comparison:

```rust
// Before optimization
let before = app.invoke(state.clone()).await?;
let metrics_before = app.metrics();

// Apply optimization (e.g., cache LLM responses)
app.enable_caching(true);

// After optimization
let after = app.invoke(state).await?;
let metrics_after = app.metrics();

// Compare
let improvement = metrics_before.total_duration.as_millis() as f64
    / metrics_after.total_duration.as_millis() as f64;

println!("Total duration: {:?} → {:?} ({:.1}× speedup)",
    metrics_before.total_duration,
    metrics_after.total_duration,
    improvement);

// Output:
// Total duration: 2500ms → 500ms (5.0× speedup)
```

**Best Practices:**

**1. Always Check Metrics After Optimization**

❌ **BAD: Optimize without measuring**
```rust
// Changed caching strategy
app.enable_caching(true);
// No metrics check - did it help?
```

✅ **GOOD: Measure before and after**
```rust
let metrics_before = app.invoke(state.clone()).await?.metrics();
app.enable_caching(true);
let metrics_after = app.invoke(state).await?.metrics();

let speedup = metrics_before.total_duration.as_secs_f64()
    / metrics_after.total_duration.as_secs_f64();
println!("Speedup: {:.2}×", speedup);
```

**2. Focus on Percentage, Not Absolute Duration**

❌ **BAD: Optimize low-impact node**
```rust
// Node takes 10ms but only 1% of total time
// Optimization effort not worth it
if metrics.node_durations.get("format") == Some(&Duration::from_millis(10)) {
    optimize_format_node(); // Low impact
}
```

✅ **GOOD: Optimize high-percentage nodes**
```rust
// Focus on nodes taking >20% of total time
for (node, duration) in &metrics.node_durations {
    let pct = metrics.node_percentage(node);
    if pct > 20.0 {
        println!("High-impact node: {} ({:.1}%)", node, pct);
        // Optimization here has significant impact
    }
}
```

**3. Use to_string_pretty() for Debugging**

❌ **BAD: Manual formatting**
```rust
println!("Total: {:?}", metrics.total_duration);
for (node, dur) in &metrics.node_durations {
    println!("{}: {:?}", node, dur);
}
// Tedious, inconsistent formatting
```

✅ **GOOD: Use built-in formatter**
```rust
println!("{}", metrics.to_string_pretty());
// Professional formatting with percentages, call counts, sorted by duration
```

**4. Monitor Metrics in Production**

❌ **BAD: Only collect in development**
```rust
#[cfg(debug_assertions)]
let metrics = app.metrics();
// No visibility in production
```

✅ **GOOD: Log metrics for production analysis**
```rust
let result = app.invoke(state).await?;
let metrics = app.metrics();

// Log to observability platform
tracing::info!(
    total_duration_ms = metrics.total_duration.as_millis(),
    edges_traversed = metrics.edges_traversed,
    checkpoint_count = metrics.checkpoint_count,
    "Graph execution complete"
);
```

**Comparison to upstream Python DashFlow:**

| Feature | Rust Metrics | upstream Python DashFlow |
|---------|--------------|------------------|
| Automatic collection | ✅ Zero-overhead in-memory | ❌ Manual instrumentation |
| Node durations | ✅ Per-node HashMap | ⚠️ Via callbacks (custom) |
| Execution counts | ✅ Built-in | ❌ Manual tracking |
| Checkpoint metrics | ✅ Save/load counts | ❌ Not tracked |
| Parallelism metrics | ✅ Peak concurrency, parallel executions | ❌ Not tracked |
| Pretty formatting | ✅ to_string_pretty() with percentages | ❌ Manual |
| Type safety | ✅ Compile-time Duration type | ⚠️ Runtime float (seconds) |
| Memory overhead | ✅ ~200 bytes per graph execution | ⚠️ Depends on callback implementation |

**Python Equivalent:**

upstream Python DashFlow doesn't have built-in metrics. Users implement via callbacks:

```python
from dashflow.callbacks.base import BaseCallbackHandler
from time import time

class MetricsCallback(BaseCallbackHandler):
    def __init__(self):
        self.node_durations = {}
        self.start_time = None

    def on_chain_start(self, serialized, inputs, **kwargs):
        self.start_time = time()

    def on_chain_end(self, outputs, **kwargs):
        node = kwargs.get("name", "unknown")
        duration = time() - self.start_time
        self.node_durations[node] = duration

# Manual setup required
callback = MetricsCallback()
result = app.invoke(state, callbacks=[callback])
print(callback.node_durations)  # Manual formatting
```

**Rust Advantage:**
- **Automatic:** No callback setup, metrics always collected
- **Comprehensive:** 13 metrics vs 1 (node durations only)
- **Type-safe:** Duration type vs float seconds (no unit confusion)
- **Zero-cost:** In-memory counters vs callback overhead
- **Pretty printing:** to_string_pretty() vs manual formatting

**Common Pitfalls:**

**Pitfall 1: Confusing Cumulative vs Average Duration**

❌ **BAD: Comparing cumulative to single execution**
```rust
// Node executed 10 times, cumulative duration is 1000ms
// Reporting "node takes 1000ms" misleading (actually 100ms per call)
let duration = metrics.node_durations.get("loop_node").unwrap();
println!("Node takes {:?}", duration); // 1000ms total, not per-call
```

✅ **GOOD: Report cumulative and average**
```rust
let duration = metrics.node_durations.get("loop_node").unwrap();
let count = metrics.node_execution_counts.get("loop_node").unwrap();
let avg = *duration / *count as u32;
println!("Node: {:?} total, {:?} per call ({} calls)", duration, avg, count);
// Output: Node: 1000ms total, 100ms per call (10 calls)
```

**Pitfall 2: Ignoring Execution Counts**

❌ **BAD: Only looking at duration**
```rust
// Node A: 500ms (1 execution)
// Node B: 500ms (100 executions, 5ms each)
// Node B is more efficient but looks equal
if metrics.node_durations.get("nodeA") == metrics.node_durations.get("nodeB") {
    println!("Nodes equally slow");
}
```

✅ **GOOD: Check execution counts**
```rust
for (node, duration) in &metrics.node_durations {
    let count = metrics.node_execution_counts.get(node).unwrap();
    let avg = *duration / *count as u32;
    println!("{}: {:?} average ({} calls)", node, avg, count);
}
// Output:
// nodeA: 500ms average (1 calls) ← Slow
// nodeB: 5ms average (100 calls) ← Fast
```

**Pitfall 3: Not Accounting for Parallelism**

❌ **BAD: Adding node durations to estimate total**
```rust
// Parallel nodes: A (100ms) + B (100ms) = 200ms expected
// Actual total: 100ms (executed concurrently)
let sum: Duration = metrics.node_durations.values().sum();
println!("Estimated total: {:?}", sum); // Wrong for parallel execution
```

✅ **GOOD: Use total_duration (wall clock time)**
```rust
let sum: Duration = metrics.node_durations.values().sum();
let actual = metrics.total_duration;
let parallelism_factor = sum.as_secs_f64() / actual.as_secs_f64();

println!("Total time: {:?} (parallelism: {:.2}×)", actual, parallelism_factor);
// Output: Total time: 100ms (parallelism: 2.00×)
```

**Pitfall 4: Comparing Metrics Across Different State Sizes**

❌ **BAD: Comparing absolute durations with different inputs**
```rust
// Small input: 100ms
// Large input: 2000ms
// Can't conclude which is "faster" without normalizing
if metrics_small.total_duration < metrics_large.total_duration {
    println!("Small input faster"); // Obvious, not insightful
}
```

✅ **GOOD: Normalize by input size or complexity**
```rust
let throughput_small = input_size_small as f64 / metrics_small.total_duration.as_secs_f64();
let throughput_large = input_size_large as f64 / metrics_large.total_duration.as_secs_f64();

println!("Throughput: {:.0} items/sec (small), {:.0} items/sec (large)",
    throughput_small, throughput_large);
// Output: Throughput: 100 items/sec (small), 50 items/sec (large)
// Conclusion: Performance degrades with input size
```

**Testing:**

Metrics module has comprehensive test coverage (31 tests):

**Unit Tests:**
- test_metrics_basic: Basic metric recording and retrieval
- test_metrics_node_percentage: Percentage calculation accuracy
- test_metrics_multiple_executions: Cumulative duration for repeated nodes
- test_record_checkpoint_save/load: Checkpoint counting
- test_record_event: Event counting
- test_set_state_size: State size tracking
- test_record_edge_traversal: Edge traversal counting
- test_record_conditional_branch: Conditional branch counting
- test_record_parallel_execution: Concurrency tracking
- test_average_node_duration_empty: Empty metrics handling
- test_node_percentage_zero_total: Zero total duration handling
- test_node_percentage_nonexistent_node: Missing node handling
- test_to_string_pretty_basic: Pretty printing basic metrics
- test_to_string_pretty_with_state_size: Pretty printing with state size
- test_to_string_pretty_with_conditional_branches: Pretty printing with branches
- test_to_string_pretty_with_parallel_executions: Pretty printing with parallelism
- test_metrics_default_trait: Default implementation
- test_metrics_clone: Clone implementation
- test_slowest_node_multiple_nodes: Slowest node identification
- test_slowest_node_empty: Empty metrics slowest node

**Run Tests:**
```bash
# All metrics tests
cargo test --package dashflow metrics

# Specific test
cargo test --package dashflow test_metrics_basic

# With coverage
cargo llvm-cov --package dashflow --summary-only
```

**Code Pointers:**

- **Module:** `crates/dashflow/src/metrics.rs` (834 lines)
- **ExecutionMetrics struct:** `metrics.rs:189-236` (18 fields)
- **Recording methods:** `metrics.rs:246-333` (11 internal methods: record_node_execution, record_node_execution_with_timestamp, record_checkpoint_save/load, record_edge_traversal, record_conditional_branch, record_parallel_execution, record_event, set_state_size, set_total_duration, record_node_tokens)
- **Analysis methods:** `metrics.rs:337-367` (3 public methods: average_node_duration, slowest_node, node_percentage)
- **Pretty formatter:** `metrics.rs:371-411` (to_string_pretty implementation with sorted output, percentages, call counts)
- **Tests:** `metrics.rs:415-834` (31 tests covering all functionality)
- **Usage in executor:** `crates/dashflow/src/executor.rs` (metrics collection during graph execution)
- **Integration with app:** `crates/dashflow/src/graph.rs` (app.metrics() accessor)

---

### Templates (Graph Workflow Patterns)

**Location:** `crates/dashflow/src/templates.rs` (2848 lines)

**What It Is:**

Graph Templates provide pre-built, production-ready workflow patterns that compile to standard StateGraph instances. Templates eliminate boilerplate code for common multi-agent architectures (Supervisor, MapReduce) while maintaining zero runtime overhead (templates are builder patterns that generate StateGraphs at compile time).

**Why It Matters:**

Multi-agent systems often follow common patterns: coordinator-worker architectures (Supervisor pattern), parallel processing with aggregation (MapReduce pattern), sequential pipelines, hierarchical delegation. Implementing these patterns from scratch requires repetitive graph construction (add nodes, add edges, set entry points, configure routing). Templates provide tested, reusable implementations that reduce development time (minutes vs hours), prevent common mistakes (missing edges, incorrect routing), and ensure consistent patterns across codebases.

**Core Concept:**

**Template = Builder Pattern + Graph Compilation**

1. **Builder API:** Fluent interface for configuring workflow components (supervisor, workers, mappers, reducers)
2. **Validation:** Compile-time validation of required components (supervisor + workers + router, or input + mappers + reducer)
3. **Graph Generation:** Automatic StateGraph construction with correct edges, entry points, and routing
4. **Zero Overhead:** Templates compile to standard StateGraphs (no runtime abstraction penalty)

**Available Templates (2):**

1. **Supervisor Pattern:** Coordinator agent manages multiple worker agents (iterative task delegation)
2. **MapReduce Pattern:** Parallel processing with aggregation (data transformation workflows)

---

#### Supervisor Template

**Pattern Structure:**

```text
START → supervisor → [router] → worker1 → supervisor
                             → worker2 → supervisor
                             → worker3 → supervisor
                             → END
```

**How It Works:**

1. **Supervisor analyzes state** and sets `next_action` field (which worker to call or END)
2. **Router reads `next_action`** and routes to worker or END
3. **Worker executes task** and returns control to supervisor
4. **Loop continues** until supervisor decides to END

**Builder API:**

```rust
use dashflow::templates::GraphTemplate;
use dashflow::{GraphState, END};

#[derive(Clone, Serialize, Deserialize)]
struct SupervisorState {
    task: String,
    next_action: String,  // Set by supervisor, read by router
    results: Vec<String>,
}

// Build supervisor graph
let graph = GraphTemplate::supervisor()
    // Add coordinator node
    .with_supervisor_node_fn("supervisor", |mut state: SupervisorState| {
        Box::pin(async move {
            // Analyze state and decide next action
            if state.results.len() < 3 {
                state.next_action = "researcher".to_string();
            } else if needs_analysis(&state) {
                state.next_action = "analyst".to_string();
            } else {
                state.next_action = END.to_string();
            }
            Ok(state)
        })
    })
    // Add worker nodes (can add multiple)
    .with_worker_fn("researcher", |mut state: SupervisorState| {
        Box::pin(async move {
            // Research task
            state.results.push("research_result".to_string());
            Ok(state)
        })
    })
    .with_worker_fn("analyst", |mut state: SupervisorState| {
        Box::pin(async move {
            // Analysis task
            state.results.push("analysis_result".to_string());
            Ok(state)
        })
    })
    // Configure routing logic
    .with_router(|state| state.next_action.clone())
    .build()?;

// Execute
let app = graph.compile()?;
let result = app.invoke(initial_state).await?;
```

**Builder Methods:**

| Method | Parameters | Purpose | Required? |
|--------|-----------|---------|-----------|
| `with_supervisor_node_fn` | `name: String`, `func: Fn(S) -> Result<S>` | Add coordinator node that decides next action | ✓ Yes |
| `with_worker_fn` | `name: String`, `func: Fn(S) -> Result<S>` | Add worker node (call multiple times for multiple workers) | ✓ Yes (≥1) |
| `with_router` | `func: Fn(&S) -> String` | Router function that reads state and returns worker name or END | ✓ Yes |
| `build` | - | Compile to StateGraph (validates configuration) | ✓ Yes |

**Generated Graph Structure:**

```rust
// Equivalent manual construction:
let mut graph = StateGraph::new();

// Add nodes
graph.add_node_from_fn("supervisor", supervisor_fn);
graph.add_node_from_fn("researcher", researcher_fn);
graph.add_node_from_fn("analyst", analyst_fn);

// Set entry point
graph.set_entry_point("supervisor");

// Workers return to supervisor
graph.add_edge("researcher", "supervisor");
graph.add_edge("analyst", "supervisor");

// Supervisor conditional routing
let mut routes = HashMap::new();
routes.insert("researcher".to_string(), "researcher".to_string());
routes.insert("analyst".to_string(), "analyst".to_string());
routes.insert(END.to_string(), END.to_string());
graph.add_conditional_edges("supervisor", router_fn, routes);
```

**Validation Rules:**

Template validates at build time (compile-time errors prevent invalid graphs):

1. **Supervisor node required:** Must call `with_supervisor_node_fn()` exactly once
2. **At least one worker:** Must call `with_worker_fn()` at least once (no upper limit)
3. **Router required:** Must call `with_router()` exactly once
4. **Automatic END route:** Template automatically adds END to routes map (router can return END without explicit configuration)

**Common Use Cases:**

**Use Case 1: Research Assistant with Multiple Specialists**

```rust
// Supervisor coordinates researcher, fact-checker, writer
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", |mut state| {
        Box::pin(async move {
            state.next_action = match state.phase.as_str() {
                "research" => "researcher".to_string(),
                "verify" => "fact_checker".to_string(),
                "write" => "writer".to_string(),
                _ => END.to_string(),
            };
            Ok(state)
        })
    })
    .with_worker_fn("researcher", research_fn)
    .with_worker_fn("fact_checker", verify_fn)
    .with_worker_fn("writer", write_fn)
    .with_router(|s| s.next_action.clone())
    .build()?;
```

**Use Case 2: Code Review System**

```rust
// Supervisor coordinates linter, security-scanner, reviewer
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", |mut state| {
        Box::pin(async move {
            if !state.linted {
                state.next_action = "linter".to_string();
            } else if !state.security_checked {
                state.next_action = "security".to_string();
            } else if !state.reviewed {
                state.next_action = "reviewer".to_string();
            } else {
                state.next_action = END.to_string();
            }
            Ok(state)
        })
    })
    .with_worker_fn("linter", lint_code_fn)
    .with_worker_fn("security", security_scan_fn)
    .with_worker_fn("reviewer", human_review_fn)
    .with_router(|s| s.next_action.clone())
    .build()?;
```

**Use Case 3: Customer Support Router**

```rust
// Supervisor routes to billing, technical, or sales based on query type
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", classify_query_fn)
    .with_worker_fn("billing", handle_billing_fn)
    .with_worker_fn("technical", handle_technical_fn)
    .with_worker_fn("sales", handle_sales_fn)
    .with_router(|s| s.department.clone())  // "billing", "technical", "sales", or END
    .build()?;
```

---

#### MapReduce Template

**Pattern Structure:**

```text
START → input → [parallel] → mapper1 → reduce → END
                          → mapper2 ↗
                          → mapper3 ↗
```

**How It Works:**

1. **Input node prepares data** (load documents, split tasks, initialize state)
2. **Mappers execute in parallel** (each processes subset of data)
3. **Reducer aggregates results** (combine mapper outputs into final result)

**Builder API:**

```rust
use dashflow::templates::GraphTemplate;

#[derive(Clone, Serialize, Deserialize)]
struct MapReduceState {
    documents: Vec<String>,
    summaries: Vec<String>,
    final_summary: String,
}

// Build MapReduce graph
let graph = GraphTemplate::map_reduce()
    // Prepare input
    .with_input_node_fn("load_docs", |mut state: MapReduceState| {
        Box::pin(async move {
            state.documents = load_documents().await?;
            Ok(state)
        })
    })
    // Add mapper nodes (process in parallel)
    .with_mapper_fn("summarize_1", |mut state: MapReduceState| {
        Box::pin(async move {
            let summary = summarize_chunk(&state.documents[0..100]).await?;
            state.summaries.push(summary);
            Ok(state)
        })
    })
    .with_mapper_fn("summarize_2", |mut state: MapReduceState| {
        Box::pin(async move {
            let summary = summarize_chunk(&state.documents[100..200]).await?;
            state.summaries.push(summary);
            Ok(state)
        })
    })
    .with_mapper_fn("summarize_3", |mut state: MapReduceState| {
        Box::pin(async move {
            let summary = summarize_chunk(&state.documents[200..]).await?;
            state.summaries.push(summary);
            Ok(state)
        })
    })
    // Aggregate results
    .with_reducer_node_fn("combine", |mut state: MapReduceState| {
        Box::pin(async move {
            state.final_summary = combine_summaries(&state.summaries).await?;
            Ok(state)
        })
    })
    .build()?;

// Execute
let app = graph.compile()?;
let result = app.invoke(initial_state).await?;
```

**Builder Methods:**

| Method | Parameters | Purpose | Required? |
|--------|-----------|---------|-----------|
| `with_input_node_fn` | `name: String`, `func: Fn(S) -> Result<S>` | Add input preparation node | ✓ Yes |
| `with_mapper_fn` | `name: String`, `func: Fn(S) -> Result<S>` | Add mapper node (call multiple times for multiple mappers) | ✓ Yes (≥1) |
| `with_reducer_node_fn` | `name: String`, `func: Fn(S) -> Result<S>` | Add reducer node that aggregates mapper results | ✓ Yes |
| `build` | - | Compile to StateGraph (validates configuration) | ✓ Yes |

**Generated Graph Structure:**

```rust
// Equivalent manual construction:
let mut graph = StateGraph::new();

// Add nodes
graph.add_node_from_fn("load_docs", input_fn);
graph.add_node_from_fn("summarize_1", mapper1_fn);
graph.add_node_from_fn("summarize_2", mapper2_fn);
graph.add_node_from_fn("summarize_3", mapper3_fn);
graph.add_node_from_fn("combine", reducer_fn);

// Set entry point
graph.set_entry_point("load_docs");

// Parallel edges: input → all mappers
graph.add_parallel_edges("load_docs", vec![
    "summarize_1".to_string(),
    "summarize_2".to_string(),
    "summarize_3".to_string(),
]);

// Mappers → reducer
graph.add_edge("summarize_1", "combine");
graph.add_edge("summarize_2", "combine");
graph.add_edge("summarize_3", "combine");

// Reducer → END
graph.add_edge("combine", END);
```

**Validation Rules:**

Template validates at build time:

1. **Input node required:** Must call `with_input_node_fn()` exactly once
2. **At least one mapper:** Must call `with_mapper_fn()` at least once (no upper limit)
3. **Reducer required:** Must call `with_reducer_node_fn()` exactly once

**Parallel Execution Note:**

DashFlow currently executes parallel edges concurrently but **only preserves the last node's state modifications** (not a true reduce with state merging from all branches). For true map-reduce with state aggregation:

**Option 1 (Recommended): Use shared state structure**

```rust
#[derive(Clone, Serialize, Deserialize)]
struct MapReduceState {
    documents: Vec<String>,
    summaries: Arc<Mutex<Vec<String>>>,  // Shared across mappers
    final_summary: String,
}

// Mappers write to shared Vec
.with_mapper_fn("mapper1", |state| {
    Box::pin(async move {
        let summary = process(&state.documents[0..100]).await?;
        state.summaries.lock().await.push(summary);
        Ok(state)
    })
})
```

**Option 2: Sequential execution (not parallel)**

```rust
// Don't use MapReduce template for sequential processing
// Use manual graph with sequential edges instead
```

**Common Use Cases:**

**Use Case 1: Document Summarization**

```rust
// Summarize large document by splitting into chunks
let graph = GraphTemplate::map_reduce()
    .with_input_node_fn("split", split_document_fn)
    .with_mapper_fn("summarize_chunk_1", summarize_fn)
    .with_mapper_fn("summarize_chunk_2", summarize_fn)
    .with_mapper_fn("summarize_chunk_3", summarize_fn)
    .with_reducer_node_fn("combine", combine_summaries_fn)
    .build()?;
```

**Use Case 2: Parallel Web Scraping**

```rust
// Scrape multiple websites in parallel, aggregate results
let graph = GraphTemplate::map_reduce()
    .with_input_node_fn("prepare_urls", load_urls_fn)
    .with_mapper_fn("scrape_site_1", scrape_fn)
    .with_mapper_fn("scrape_site_2", scrape_fn)
    .with_mapper_fn("scrape_site_3", scrape_fn)
    .with_reducer_node_fn("aggregate", merge_results_fn)
    .build()?;
```

**Use Case 3: Batch Translation**

```rust
// Translate document sections in parallel
let graph = GraphTemplate::map_reduce()
    .with_input_node_fn("split_sections", split_fn)
    .with_mapper_fn("translate_intro", translate_fn)
    .with_mapper_fn("translate_body", translate_fn)
    .with_mapper_fn("translate_conclusion", translate_fn)
    .with_reducer_node_fn("reassemble", join_sections_fn)
    .build()?;
```

---

#### Best Practices

**Best Practice 1: Use Templates for Standard Patterns, Custom Graphs for Complex Routing**

```rust
// ❌ BAD: Force custom routing into Supervisor template
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", |state| {
        // Complex routing logic with nested conditionals
        // Hard to understand, doesn't match pattern
    })
    .build()?;

// ✅ GOOD: Use template for standard supervisor pattern
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", simple_routing_fn)
    .build()?;

// ✅ GOOD: Use manual graph for complex custom routing
let mut graph = StateGraph::new();
graph.add_node_from_fn("coordinator", coordinator_fn);
// ... custom edge logic
```

**Best Practice 2: Supervisor State Should Have Explicit `next_action` Field**

```rust
// ❌ BAD: Implicit routing field
#[derive(Clone, Serialize, Deserialize)]
struct State {
    data: String,
    // Router has to guess which field contains routing info
}

// ✅ GOOD: Explicit routing field
#[derive(Clone, Serialize, Deserialize)]
struct State {
    data: String,
    next_action: String,  // Clear: supervisor sets this, router reads this
}
```

**Best Practice 3: Use Arc<Mutex<Vec>> for MapReduce State Aggregation**

```rust
// ❌ BAD: State modifications from parallel mappers get lost
#[derive(Clone)]
struct State {
    results: Vec<String>,  // Only last mapper's results preserved
}

// ✅ GOOD: Shared state structure for parallel aggregation
#[derive(Clone)]
struct State {
    results: Arc<Mutex<Vec<String>>>,  // All mappers write to shared Vec
}
```

**Best Practice 4: Validate Template Configuration Before Runtime**

```rust
// ❌ BAD: Runtime panic when build() fails
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", supervisor_fn)
    // Forgot to add workers!
    .build()
    .unwrap();  // Panics: "No workers configured"

// ✅ GOOD: Handle build errors explicitly
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", supervisor_fn)
    .with_worker_fn("worker1", worker_fn)
    .with_router(router_fn)
    .build()
    .map_err(|e| format!("Template build failed: {}", e))?;
```

---

#### Common Pitfalls

**Pitfall 1: Forgetting to Add END Route in Router**

```rust
// ❌ BAD: Router returns END but doesn't handle it
.with_router(|state| {
    if state.done {
        END.to_string()  // Template automatically adds END route
    } else {
        "worker".to_string()
    }
})

// ✅ GOOD: Template handles END automatically (no action needed)
// Just return END from router, template adds it to routes map
```

**Pitfall 2: Expecting State Merging from Parallel Mappers**

```rust
// ❌ BAD: Expecting all mapper results to merge automatically
#[derive(Clone)]
struct State {
    results: Vec<String>,  // Only last mapper's Vec is preserved
}

.with_mapper_fn("mapper1", |mut state| {
    Box::pin(async move {
        state.results.push("result1".to_string());
        Ok(state)  // This Vec gets overwritten by mapper2
    })
})

// ✅ GOOD: Use Arc<Mutex<>> for shared state
#[derive(Clone)]
struct State {
    results: Arc<Mutex<Vec<String>>>,  // All mappers write to same Vec
}

.with_mapper_fn("mapper1", |state| {
    Box::pin(async move {
        state.results.lock().await.push("result1".to_string());
        Ok(state)  // All results preserved
    })
})
```

**Pitfall 3: Not Returning Control to Supervisor**

```rust
// ❌ BAD: Worker tries to call another worker directly
.with_worker_fn("worker1", |mut state| {
    Box::pin(async move {
        state.next_action = "worker2".to_string();  // Wrong!
        Ok(state)  // Worker always returns to supervisor
    })
})

// ✅ GOOD: Worker finishes, supervisor decides next action
.with_worker_fn("worker1", |mut state| {
    Box::pin(async move {
        // Just do work, supervisor will decide what's next
        state.work_done = true;
        Ok(state)
    })
})
.with_supervisor_node_fn("supervisor", |mut state| {
    Box::pin(async move {
        if state.work_done {
            state.next_action = "worker2".to_string();
        }
        Ok(state)
    })
})
```

**Pitfall 4: Building Template Without Validating Configuration**

```rust
// ❌ BAD: Missing required components (runtime error)
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", supervisor_fn)
    // Missing: workers, router
    .build()?;  // Error: "No workers configured"

// ✅ GOOD: Complete configuration before build
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", supervisor_fn)
    .with_worker_fn("worker1", worker1_fn)  // ✓ At least one worker
    .with_router(router_fn)  // ✓ Router configured
    .build()?;  // Success
```

---

#### Comparison to upstream Python DashFlow

| Feature | Rust Templates | upstream Python DashFlow | Rust Advantage |
|---------|---------------|------------------|----------------|
| **Template patterns** | Supervisor, MapReduce (built-in builders) | No built-in templates (manual graph construction) | ✓ Faster development (minutes vs hours) |
| **Compile-time validation** | Build-time errors for missing components | Runtime errors | ✓ Catch mistakes before execution |
| **Type safety** | Generic over state type `<S: GraphState>` | Dynamic state (dict-based) | ✓ Compile-time state validation |
| **Zero overhead** | Templates compile to StateGraphs (no runtime abstraction) | N/A | ✓ No performance penalty |
| **Builder pattern** | Fluent API (`.with_supervisor_node_fn().with_worker_fn().build()`) | Manual graph construction | ✓ More readable |
| **Error messages** | Specific ("Reducer node not set. Call with_reducer_node_fn()") | Generic | ✓ Better DX |

**Python Equivalent (Manual Construction):**

```python
from dashflow.graph import StateGraph, END

# Python: Manual supervisor pattern (no template)
graph = StateGraph()
graph.add_node("supervisor", supervisor_fn)
graph.add_node("worker1", worker1_fn)
graph.add_node("worker2", worker2_fn)
graph.set_entry_point("supervisor")
graph.add_edge("worker1", "supervisor")
graph.add_edge("worker2", "supervisor")
graph.add_conditional_edges(
    "supervisor",
    router_fn,
    {"worker1": "worker1", "worker2": "worker2", END: END}
)
app = graph.compile()

# Rust: Template (less boilerplate)
let graph = GraphTemplate::supervisor()
    .with_supervisor_node_fn("supervisor", supervisor_fn)
    .with_worker_fn("worker1", worker1_fn)
    .with_worker_fn("worker2", worker2_fn)
    .with_router(router_fn)
    .build()?;
let app = graph.compile()?;
```

**Rust Advantages:**

1. **Less boilerplate:** Template handles edge configuration (workers → supervisor, supervisor → workers/END)
2. **Validation:** Build-time errors if missing components (Python fails at runtime)
3. **Type safety:** Generic over state type (Python uses dict)
4. **Intent-revealing:** `GraphTemplate::supervisor()` makes pattern obvious (Python looks like generic graph)

---

#### Testing

**Test Coverage: 97 tests**

**Template Creation Tests (4 tests):**
- `test_graph_template_supervisor_creation` - Create supervisor template
- `test_graph_template_map_reduce_creation` - Create MapReduce template
- `test_supervisor_template_basic` - Execute supervisor workflow
- `test_mapreduce_template_basic` - Execute MapReduce workflow

**Supervisor Builder Tests (15 tests):**
- `test_supervisor_builder_new` - Builder initialization
- `test_supervisor_builder_default` - Default trait implementation
- `test_supervisor_builder_chaining` - Fluent API chaining
- `test_supervisor_builder_with_supervisor_only` - Partial configuration validation
- `test_supervisor_builder_with_workers_only` - Missing supervisor validation
- `test_supervisor_builder_with_router_only` - Missing nodes validation
- `test_supervisor_builder_missing_supervisor_name_only` - Name validation
- `test_supervisor_builder_missing_router_specific` - Router validation
- `test_supervisor_error_messages_specific` - Error message quality
- `test_supervisor_builder_end_route_added` - END route auto-configuration
- `test_supervisor_immediate_end` - Supervisor can immediately route to END
- `test_supervisor_many_workers` - Support for large worker sets (10 workers)
- `test_supervisor_template_validation` - Configuration validation
- `test_supervisor_loop_detection` - Detect infinite supervisor loops
- `test_supervisor_worker_communication` - State passing between workers

**MapReduce Builder Tests (16 tests):**
- `test_mapreduce_builder_new` - Builder initialization
- `test_mapreduce_builder_default` - Default trait implementation
- `test_mapreduce_builder_chaining` - Fluent API chaining
- `test_mapreduce_builder_with_input_only` - Partial configuration validation
- `test_mapreduce_builder_with_mappers_only` - Missing input validation
- `test_mapreduce_builder_with_reducer_only` - Missing mappers validation
- `test_mapreduce_builder_missing_reducer_specific` - Reducer validation
- `test_mapreduce_error_messages_specific` - Error message quality
- `test_mapreduce_builder_mappers_to_reducer_edges` - Edge configuration
- `test_mapreduce_parallel_edges` - Parallel execution validation
- `test_mapreduce_template_validation` - Configuration validation
- `test_mapreduce_empty_input` - Handle empty input
- `test_mapreduce_single_mapper` - Single mapper support
- `test_mapreduce_many_mappers` - Support for large mapper sets (100 mappers)
- `test_mapreduce_state_aggregation` - Arc<Mutex<>> state aggregation
- `test_mapreduce_reducer_aggregates_results` - Verify reducer receives all mapper results

**Run Tests:**

```bash
# All template tests
cargo test --package dashflow --lib templates

# Specific test
cargo test --package dashflow --lib templates::tests::test_supervisor_template_basic

# With output
cargo test --package dashflow --lib templates -- --nocapture
```

---

#### Code Pointers

- **Module:** `crates/dashflow/src/templates.rs` (2848 lines)
- **GraphTemplate enum:** `templates.rs:56-61` (Supervisor, MapReduce variants)
- **SupervisorBuilder:** `templates.rs:104-254` (Builder pattern, build() method with validation)
- **MapReduceBuilder:** `templates.rs:277-396` (Builder pattern, build() method with validation)
- **Tests:** `templates.rs:405-2848` (97 tests covering all functionality)
- **Usage examples:** See tests for complete working examples (test_supervisor_template_basic, test_mapreduce_template_basic)

---

### Graph Executor (CompiledGraph)

**Location:** `crates/dashflow/src/executor/` (13,089 lines across mod.rs, execution.rs, tests.rs, trace.rs, and helper modules)

**What It Is:**

CompiledGraph is the runtime execution engine for StateGraphs. After building a graph with nodes, edges, and routing logic, calling `.compile()` produces a CompiledGraph ready for execution. The executor handles node traversal, edge routing, state management, checkpoint persistence, event emission, metrics collection, parallel execution, timeout enforcement, and human-in-the-loop interrupts.

**Why It Matters:**

StateGraph is the blueprint; CompiledGraph is the runtime. The executor transforms declarative graph definitions into production execution with: (1) Streaming execution for real-time results, (2) Checkpoint persistence for fault tolerance and resume capability, (3) Observability via callbacks and metrics, (4) Distributed execution via work-stealing scheduler, (5) Safety features (timeouts, recursion limits, interrupts). Without the executor, graphs are static data structures. With the executor, graphs become interactive, observable, fault-tolerant production workflows.

**Core Concept:**

**Execution = Graph Traversal + State Management + Observability**

1. **Graph Traversal:** Navigate nodes/edges from entry point to END (sequential, parallel, conditional routing)
2. **State Management:** Thread state through nodes with checkpointing for resume after interrupts/failures
3. **Observability:** Emit events, collect metrics, invoke callbacks for monitoring and debugging

**CompiledGraph Struct (Key Fields):**

```rust
pub struct CompiledGraph<S: GraphState> {
    name: Option<String>,                        // Graph name (tracing spans)
    nodes: HashMap<String, BoxedNode<S>>,        // Node registry
    edges: Vec<Edge>,                            // Sequential edges (A → B)
    conditional_edges: Vec<ConditionalEdge<S>>,  // Conditional routing (A → B|C based on state)
    parallel_edges: Vec<ParallelEdge>,           // Parallel execution (A → [B,C,D])
    entry_point: String,                         // Starting node name
    graph_timeout: Option<Duration>,             // Total execution timeout
    node_timeout: Option<Duration>,              // Per-node timeout
    callbacks: Vec<Arc<dyn EventCallback<S>>>,   // Event listeners (DashStream, custom)
    checkpointer: Option<Arc<dyn Checkpointer<S>>>, // State persistence (resume after interrupt/failure)
    thread_id: Option<ThreadId>,                 // Checkpoint isolation (multi-conversation)
    metrics: Arc<Mutex<ExecutionMetrics>>,       // Performance tracking
    scheduler: Option<Arc<WorkStealingScheduler<S>>>, // Distributed execution
    interrupt_before: Vec<String>,               // Pause before these nodes (human-in-the-loop)
    interrupt_after: Vec<String>,                // Pause after these nodes (human-in-the-loop)
    recursion_limit: u32,                        // Max steps before RecursionLimit error (default: 25)
}
```

---

#### Configuration API (Builder Pattern)

CompiledGraph uses builder pattern for optional configuration:

**Basic Configuration:**

```rust
use dashflow::StateGraph;

let graph = StateGraph::new()
    .add_node("start", start_fn)
    .add_node("process", process_fn)
    .add_edge("start", "process")
    .add_edge("process", END)
    .set_entry_point("start")
    .compile()?;

// Execute immediately (minimal configuration)
let result = graph.invoke(initial_state).await?;
```

**Production Configuration:**

```rust
use dashflow::checkpoint::SqliteCheckpointer;
use dashflow::dashstream_callback::DashStreamCallback;
use std::time::Duration;

let checkpointer = SqliteCheckpointer::new("checkpoints.db").await?;
let telemetry = DashStreamCallback::new("kafka:9092", "events")?;

let graph = StateGraph::new()
    .add_node("agent", agent_fn)
    // ... add more nodes/edges
    .compile()?
    .with_name("customer-support-agent")           // Tracing span name
    .with_checkpointer(checkpointer)               // Fault tolerance
    .with_thread_id("conversation-123")            // Checkpoint isolation
    .with_callback(telemetry)                      // Telemetry
    .with_timeout(Duration::from_secs(300))        // 5min graph timeout
    .with_node_timeout(Duration::from_secs(30))    // 30sec per-node timeout
    .with_recursion_limit(100)                     // Allow up to 100 steps
    .with_interrupt_before(vec!["human_review"])   // Pause before human review
    .with_interrupt_after(vec!["sensitive_action"]);// Pause after sensitive action

// Execute with full observability and fault tolerance
let result = graph.invoke(initial_state).await?;
```

**Configuration Methods:**

| Method | Purpose | Default | Use When |
|--------|---------|---------|----------|
| `with_name(name)` | Set graph name for tracing spans | None | Production logging/monitoring |
| `with_callback(callback)` | Add event listener | Empty | Telemetry, logging, debugging |
| `with_checkpointer(checkpointer)` | Enable state persistence | None | Long-running workflows, fault tolerance |
| `with_thread_id(thread_id)` | Isolate checkpoints per thread | None | Multi-conversation systems |
| `with_scheduler(scheduler)` | Enable distributed execution | None | Scale-out workloads |
| `with_timeout(duration)` | Total execution timeout | None | Prevent runaway executions |
| `with_node_timeout(duration)` | Per-node timeout | None | Prevent slow nodes blocking graph |
| `with_recursion_limit(limit)` | Max execution steps | 25 | Graphs with cycles/loops |
| `with_interrupt_before(nodes)` | Pause before nodes | Empty | Human-in-the-loop (approval before action) |
| `with_interrupt_after(nodes)` | Pause after nodes | Empty | Human-in-the-loop (review after action) |

---

#### Execution Methods

**1. invoke() - Run to Completion**

Execute graph from initial state to END or interrupt:

```rust
let result = graph.invoke(initial_state).await?;

// ExecutionResult contains:
// - state: Final state after execution
// - execution_path: Vec<String> of nodes executed in order
// - interrupted: bool (true if hit interrupt_before/after)
// - next_node: Option<String> (node to resume from if interrupted)

println!("Final state: {:?}", result.state);
println!("Path: {:?}", result.execution_path);  // ["start", "process", "llm", "tools", "llm"]
println!("Interrupted: {}", result.interrupted);
```

**2. resume() - Continue After Interrupt**

Resume execution from last checkpoint (requires checkpointer + thread_id):

```rust
// First execution: runs until interrupt
let result = graph.invoke(initial_state).await?;
assert!(result.interrupted);
assert_eq!(result.next_node, Some("human_review".to_string()));

// Human reviews and approves (application logic, not shown)

// Resume from checkpoint
let final_result = graph.resume().await?;
assert!(!final_result.interrupted);  // Completed to END
```

**3. stream() - Streaming Execution**

Stream execution events in real-time (values, updates, debug, or messages):

```rust
use dashflow::stream::{StreamMode, StreamEvent};
use futures::StreamExt;

let mut stream = graph.stream(initial_state, StreamMode::Values).boxed();

while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::Value(state) => {
            // State after each node
            println!("Node completed. Current state: {:?}", state);
        }
        StreamEvent::End(final_state) => {
            // Graph reached END
            println!("Graph completed. Final state: {:?}", final_state);
        }
    }
}
```

**StreamMode Options:**

- **Values:** Emit state after each node completion (final state values)
- **Updates:** Emit state diffs/updates per node (what changed, not full state)
- **Debug:** Emit detailed debug events (node start/end, edge traversal, routing decisions)
- **Messages:** Emit message updates only (for chat/LLM applications, filters to message field)

**4. get_current_state() - Read Checkpoint**

Read current state from checkpointer without executing:

```rust
// Read state from database (doesn't execute graph)
let current_state = graph.get_current_state().await?;
println!("Conversation state: {:?}", current_state);
```

**5. update_state() - Modify Checkpoint**

Update state in checkpointer without executing (manual state modification):

```rust
// Manually update state (e.g., user correction, admin override)
graph.update_state(|mut state| {
    state.messages.push(user_correction_message);
    state.retry_count = 0;  // Reset retry counter
    Ok(state)
}).await?;

// Resume with updated state
let result = graph.resume().await?;
```

---

#### Execution Flow

**Sequential Execution:**

```text
invoke(state) → entry_point → node1 → node2 → node3 → END
                  ↓            ↓        ↓        ↓        ↓
               metrics      callback  state   metrics  result
               events       events    update  events
```

**Parallel Execution:**

```text
invoke(state) → entry_point → [parallel] → node1 → reducer → END
                                         → node2 ↗
                                         → node3 ↗
                  (concurrent execution, last node's state preserved)
```

**Conditional Routing:**

```text
invoke(state) → entry_point → router → node_A → END
                                     → node_B → END
                                     → END (direct)
                  (router function evaluates state, returns next node)
```

**Interrupt Flow:**

```text
invoke(state) → node1 → node2 → [interrupt_before="node3"] → PAUSE
                                                               ↓ save checkpoint
                                                               return ExecutionResult{interrupted=true}

resume() → load checkpoint → node3 → node4 → END
           ↓
           continue from saved state
```

---

#### Error Handling

**Timeout Errors:**

```rust
// Graph-level timeout
let result = graph
    .with_timeout(Duration::from_secs(60))
    .invoke(state)
    .await;

match result {
    Err(Error::Timeout) => println!("Graph exceeded 60s timeout"),
    Ok(result) => println!("Completed: {:?}", result),
}

// Node-level timeout
let result = graph
    .with_node_timeout(Duration::from_secs(10))
    .invoke(state)
    .await;

match result {
    Err(Error::NodeTimeout { node_name }) => {
        println!("Node '{}' exceeded 10s timeout", node_name);
    }
    Ok(result) => println!("Completed: {:?}", result),
}
```

**Recursion Limit:**

```rust
// Prevent infinite loops
let result = graph
    .with_recursion_limit(50)  // Allow up to 50 steps
    .invoke(state)
    .await;

match result {
    Err(Error::RecursionLimit { limit, steps }) => {
        println!("Exceeded recursion limit: {} steps (max: {})", steps, limit);
    }
    Ok(result) => println!("Completed in {} steps", result.execution_path.len()),
}
```

**Checkpoint Errors:**

```rust
// Missing checkpointer for resume
let result = graph.resume().await;

match result {
    Err(Error::NoCheckpointer) => {
        println!("resume() requires checkpointer. Use .with_checkpointer(...)");
    }
    Ok(result) => println!("Resumed: {:?}", result),
}
```

---

#### Performance Characteristics

**Execution Overhead:**

- **Node dispatch:** ~10-20μs per node (lookup + async call)
- **Edge traversal:** ~5μs per edge (routing logic)
- **Checkpoint save:** ~1-5ms per checkpoint (database write, depends on checkpointer)
- **Event emission:** ~1-2μs per event (callback invocation)
- **Metrics collection:** <1μs per metric (in-memory counter/timer)

**Streaming Performance:**

- **Values mode:** Emits state after each node (~10-20μs overhead per event)
- **Updates mode:** Emits state diffs (~50-100μs overhead for diff calculation)
- **Debug mode:** Emits detailed events (~20-30μs overhead per event)
- **Messages mode:** Filters to message field (~30-40μs overhead for filtering + cloning)

**Parallelism:**

- Parallel edges execute concurrently (tokio::spawn per node)
- Spawning overhead: ~20-50μs per parallel node
- Last node's state modifications preserved (not a true reduce/merge)
- Use `Arc<Mutex<Vec>>` in state for parallel aggregation

---

#### Common Use Cases

**Use Case 1: Long-Running Agent with Checkpointing**

```rust
// Customer support agent that can resume conversations
let checkpointer = SqliteCheckpointer::new("conversations.db").await?;

let graph = StateGraph::new()
    .add_node("classify", classify_fn)
    .add_node("search_docs", search_fn)
    .add_node("answer", answer_fn)
    // ... edges
    .compile()?
    .with_checkpointer(checkpointer)
    .with_thread_id(format!("user-{}", user_id))
    .with_timeout(Duration::from_secs(300));

// First message
let result = graph.invoke(initial_state).await?;

// Later: user sends follow-up (different process/server)
// Automatically loads checkpoint for this thread_id
let result = graph.invoke(followup_state).await?;
```

**Use Case 2: Human-in-the-Loop Workflow**

```rust
// Require human approval before executing sensitive actions
let graph = StateGraph::new()
    .add_node("analyze", analyze_fn)
    .add_node("generate_sql", generate_sql_fn)
    .add_node("execute_sql", execute_sql_fn)  // Sensitive!
    // ... edges
    .compile()?
    .with_checkpointer(checkpointer)
    .with_thread_id("workflow-123")
    .with_interrupt_before(vec!["execute_sql"]);

// Execute: pauses before execute_sql
let result = graph.invoke(state).await?;
assert!(result.interrupted);
assert_eq!(result.next_node, Some("execute_sql"));

// Human reviews generated SQL, approves
if human_approves(&result.state.generated_sql) {
    let final_result = graph.resume().await?;  // Executes SQL, completes
}
```

**Use Case 3: Streaming Agent with Real-Time Updates**

```rust
// Stream agent execution to frontend (SSE, WebSocket, etc.)
use futures::StreamExt;

let mut stream = graph.stream(initial_state, StreamMode::Values).boxed();

while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::Value(state) => {
            // Send update to frontend
            websocket.send_json(&state).await?;
        }
        StreamEvent::End(final_state) => {
            websocket.send_json(&json!({"done": true, "state": final_state})).await?;
        }
    }
}
```

**Use Case 4: Distributed Execution with Work-Stealing**

```rust
// Scale graph execution across multiple workers
let scheduler = WorkStealingScheduler::new(redis_url).await?;

let graph = StateGraph::new()
    .add_node("expensive_node_1", expensive_fn_1)
    .add_node("expensive_node_2", expensive_fn_2)
    // ... many expensive nodes
    .compile()?
    .with_scheduler(scheduler);  // Nodes execute on available workers

// Execution automatically distributed
let result = graph.invoke(state).await?;
```

---

#### Code Pointers

- **Module:** `crates/dashflow/src/executor/mod.rs` (2835 lines)
- **CompiledGraph struct:** `executor/mod.rs:214-290` (15+ fields for configuration/state)
- **Configuration methods:** `executor/mod.rs:359-1410` (15+ builder methods: with_name, with_callback, with_checkpointer, with_thread_id, with_scheduler, with_interrupt_before, with_interrupt_after, with_recursion_limit, with_timeout, with_node_timeout, with_execution_tracker, with_observability, with_checkpointing, metrics accessor)
- **Execution methods:** `executor/execution.rs:180-550` (invoke, resume, get_current_state, update_state, stream with 4 modes)
- **Internal traversal:** `executor/execution.rs:831-2100` (execute_node, invoke_internal, parallel execution, emit_events)
- **ExecutionResult struct:** `executor/mod.rs:2793-2835` (state, execution_path, interrupted, next_node)
- **Tests:** `executor/mod.rs:2835+` and `executor/tests.rs` (100+ tests covering all execution modes, timeouts, interrupts, checkpointing, parallel execution)

---
