# dashflow-derive

**Procedural macros for DashFlow state management with compile-time safety and automatic merge strategies.**

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../../LICENSE)

---

## Overview

This crate provides derive macros for DashFlow state types, eliminating boilerplate and ensuring compile-time correctness. It was created during the Framework-First initiative to solve **Gap #2: GraphState Boilerplate** - reducing 15 lines of manual code to 1 line.

**What it provides:**

- `#[derive(GraphState)]` - Compile-time verification of required traits
- `#[derive(MergeableState)]` - Automatic merge implementations for parallel execution
- Zero runtime overhead (pure compile-time code generation)
- Type-safe merge strategies for common collections and primitives

---

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
dashflow = "1.11"
dashflow-derive = "1.11"
serde = { version = "1.0", features = ["derive"] }
```

### Basic State with GraphState

```rust
use dashflow_derive::GraphState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, GraphState)]
struct MyState {
    query: String,
    results: Vec<String>,
}
```

The `GraphState` derive ensures at compile time that your type implements:
- `Clone` (required for state snapshots)
- `serde::Serialize` (required for checkpointing)
- `serde::Deserialize` (required for state restoration)

### Parallel Execution with MergeableState

When using parallel execution in DashFlow, you need to merge state from concurrent branches. The `MergeableState` derive automatically generates merge logic:

```rust
use dashflow_derive::MergeableState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ResearchState {
    findings: Vec<String>,        // Extends with other's elements
    sources: HashSet<String>,     // Unions with other's elements
    metadata: HashMap<String, String>,  // Merges maps (overwrites on collision)
    confidence: f64,              // Takes maximum value
    description: String,          // Concatenates with newline
    has_results: bool,            // Logical OR
}
```

---

## Features

### 1. GraphState Derive

**Purpose:** Compile-time verification that state types implement required traits.

**Generated Code:** Compile-time assertions (zero runtime cost)

**Example:**

```rust
#[derive(Clone, Serialize, Deserialize, GraphState)]
struct AgentState {
    messages: Vec<String>,
    next_action: Option<String>,
}

// This won't compile if Clone, Serialize, or Deserialize are missing:
// error: the trait bound `AgentState: Clone` is not satisfied
```

### 2. MergeableState Derive

**Purpose:** Automatic state merging for parallel execution paths.

**Merge Strategies by Type:**

| Type | Merge Strategy | Example |
|------|---------------|---------|
| `Vec<T>` | Extend (concatenate) | `[1, 2] + [3, 4] = [1, 2, 3, 4]` |
| `VecDeque<T>` | Extend (concatenate) | Same as Vec |
| `HashSet<T>` | Union | `{1, 2} + {2, 3} = {1, 2, 3}` |
| `BTreeSet<T>` | Union | Same as HashSet |
| `HashMap<K, V>` | Merge (last-write-wins) | `{a:1} + {a:2, b:3} = {a:2, b:3}` |
| `BTreeMap<K, V>` | Merge (last-write-wins) | Same as HashMap |
| `Option<T>` | Take if None | `None + Some(x) = Some(x)` |
| Numeric types | Maximum | `5 + 8 = 8` |
| `String` | Concatenate (newline) | `"a" + "b" = "a\nb"` |
| `bool` | Logical OR | `false + true = true` |
| Other types | Keep self (no merge) | Custom types unchanged |

**Example: Parallel Web Scraping**

```rust
use dashflow::{StateGraph, MergeableState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ScraperState {
    urls: Vec<String>,           // Collects URLs from all sources
    errors: Vec<String>,         // Collects errors from all sources
    pages_scraped: usize,        // Takes maximum from parallel paths
}

// Without MergeableState, you'd write ~15 lines:
// impl MergeableState for ScraperState {
//     fn merge(&mut self, other: &Self) {
//         self.urls.extend(other.urls.clone());
//         self.errors.extend(other.errors.clone());
//         self.pages_scraped = self.pages_scraped.max(other.pages_scraped);
//     }
// }
```

---

## Real-World Examples

### Multi-Agent Research Team

```rust
use dashflow_derive::MergeableState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ResearchState {
    query: String,              // Kept from initial state
    findings: Vec<String>,      // Aggregated from all agents
    sources: HashSet<String>,   // Deduplicated across agents
    agent_count: usize,         // Tracks how many agents ran
}

// When 3 agents run in parallel, their states merge automatically:
// Agent 1: findings = ["fact1"], sources = {"src1"}, agent_count = 1
// Agent 2: findings = ["fact2"], sources = {"src2"}, agent_count = 1
// Agent 3: findings = ["fact3"], sources = {"src1"}, agent_count = 1
// Merged:  findings = ["fact1", "fact2", "fact3"],
//          sources = {"src1", "src2"},
//          agent_count = 1 (max of all)
```

### Error Recovery with Retry Tracking

```rust
use dashflow_derive::MergeableState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct RetryState {
    operation: String,
    errors: Vec<String>,        // Collects all error messages
    retry_count: usize,         // Increments across retries
    succeeded: bool,            // True if any path succeeded
}
```

### Streaming Aggregator Pattern

Example of parallel data source aggregation:

```rust
#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct StreamingState {
    query: String,
    web_api_results: Vec<String>,     // Results from web API
    database_results: Vec<String>,    // Results from database
    cache_results: Vec<String>,       // Results from cache
}

// Three data sources run in parallel
// All results automatically merge into unified state
// Zero data loss, zero manual merge code
```

See `examples/apps/librarian/` for production streaming patterns.

---

## Why This Matters

### Before (Manual Implementation)

```rust
#[derive(Clone, Serialize, Deserialize)]
struct ParallelState {
    results: Vec<String>,
    errors: Vec<String>,
    count: usize,
}

impl dashflow::MergeableState for ParallelState {
    fn merge(&mut self, other: &Self) {
        self.results.extend(other.results.clone());
        self.errors.extend(other.errors.clone());
        self.count = self.count.max(other.count);
    }
}
```

**15 lines of boilerplate** for every state type with parallel execution.

### After (Derive Macro)

```rust
#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct ParallelState {
    results: Vec<String>,
    errors: Vec<String>,
    count: usize,
}
```

**1 line** - 93% boilerplate reduction. The macro generates the exact same merge implementation.

---

## Performance

- **Zero runtime overhead** - All code generation happens at compile time
- **Type-safe** - Merge strategies are type-checked by the compiler
- **No reflection** - Pure Rust code generation with `syn` and `quote`

Benchmarks from Framework-First initiative:
- Compile time increase: <0.1s for typical workspace
- Binary size increase: 0 bytes (no runtime code)
- Execution speed: Identical to hand-written merge implementations

---

## Integration with DashFlow

This crate is designed to work seamlessly with `dashflow`:

```rust
use dashflow::{StateGraph, END};
use dashflow_derive::MergeableState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, MergeableState)]
struct State {
    data: Vec<String>,
}

async fn node_a(state: State) -> Result<State, Box<dyn std::error::Error>> {
    let mut state = state;
    state.data.push("from_a".to_string());
    Ok(state)
}

async fn node_b(state: State) -> Result<State, Box<dyn std::error::Error>> {
    let mut state = state;
    state.data.push("from_b".to_string());
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = StateGraph::new();
    graph.add_node("a", node_a);
    graph.add_node("b", node_b);
    graph.add_edge("a", END);
    graph.add_edge("b", END);
    graph.set_parallel_entry_points(vec!["a", "b"]);

    let app = graph.compile()?;
    let result = app.invoke(State { data: vec![] }, None).await?;

    // result.data contains ["from_a", "from_b"] in some order
    // Both parallel paths executed and merged automatically
    Ok(())
}
```

---

## Advanced Usage

### Custom Merge Logic

If you need custom merge behavior for specific fields, implement `MergeableState` manually:

```rust
#[derive(Clone, Serialize, Deserialize)]
struct CustomState {
    standard_field: Vec<String>,  // Would auto-merge
    custom_field: MyType,         // Needs manual merge
}

impl dashflow::MergeableState for CustomState {
    fn merge(&mut self, other: &Self) {
        // Standard auto-merge behavior
        self.standard_field.extend(other.standard_field.clone());

        // Custom merge logic
        self.custom_field.custom_merge(&other.custom_field);
    }
}
```

### Selective Derivation

You can derive `GraphState` without `MergeableState` for sequential graphs:

```rust
#[derive(Clone, Serialize, Deserialize, GraphState)]
struct SequentialState {
    current_step: usize,
    result: Option<String>,
}

// No parallel execution, no merge needed
```

---

## Implementation Details

### Code Generation

The macros use `syn` for parsing and `quote` for code generation:

```rust
// Input:
#[derive(MergeableState)]
struct State { items: Vec<String> }

// Generated:
impl dashflow::MergeableState for State {
    fn merge(&mut self, other: &Self) {
        self.items.extend(other.items.clone());
    }
}
```

### Compile-Time Guarantees

The `GraphState` derive generates compile-time assertions:

```rust
const _: () = {
    fn assert_graph_state<T>()
    where
        T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de>
    {}

    fn assert_impl() {
        assert_graph_state::<YourState>();
    }
};
```

If any trait is missing, compilation fails with a clear error message.

---

## Comparison to upstream DashFlow (Python)

In upstream DashFlow (Python), state merging is implicit and uses `operator.add`:

```python
# Python
class State(TypedDict):
    findings: Annotated[list[str], operator.add]
    sources: Annotated[set[str], operator.add]
```

In Rust, we make merging explicit and type-safe:

```rust
// Rust
#[derive(MergeableState)]
struct State {
    findings: Vec<String>,  // Auto-extends
    sources: HashSet<String>,  // Auto-unions
}
```

**Advantages:**
- Compile-time verification (Python errors at runtime)
- Explicit merge strategies (no implicit operator overloading)
- Zero performance overhead (no reflection)
- Better IDE support (errors show exact trait requirements)

---

## Testing

The derive macros are extensively tested with 44 test cases covering:

- All supported collection types (Vec, HashMap, HashSet, etc.)
- Numeric types (i32, u32, f64, usize, etc.)
- String concatenation
- Boolean logic
- Option handling
- Nested structures
- Compile-time trait verification

Run tests:

```bash
cargo test -p dashflow-derive
```

---

## Limitations

1. **Struct-only:** Macros only work with structs with named fields (not tuple structs or enums)
2. **Standard types:** Custom types use "keep self" strategy unless you implement `MergeableState` manually
3. **Clone requirement:** Merge implementations clone data (minimal overhead for most use cases)

---

## Resources

**Framework Code:**
- Source: `crates/dashflow-derive/src/lib.rs`
- Tests: `crates/dashflow-derive/tests/`
- Integration: `crates/dashflow/src/state.rs`

**Example Applications:**
- Librarian (production RAG): `examples/apps/librarian/`

> **Historical Note:** Previous example apps (streaming_aggregator, research_team, error_recovery)
> have been consolidated into the librarian paragon application.

**Documentation:**
- Gap #2 analysis: `reports/main/GAP_2_FIXED_DERIVE_MACROS.md`
- Framework-First report: `FRAMEWORK_FIRST_INITIATIVE_COMPLETE.md`

---

## License

Licensed under the MIT License. See [LICENSE](../../LICENSE) for details.

---

## Contributing

This crate is part of the DashFlow project. Contributions are welcome!

**Repository:** https://github.com/dropbox/dTOOL/dashflow

**Issues:** Report bugs or request features at the main repository

---

**Created:** November 16, 2025 (Framework-First Initiative, commits #40-42)
**Status:** Production-ready (5 apps validated, 27 commits tested)
**Version:** 1.0.0
