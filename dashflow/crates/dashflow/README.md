# dashflow

Graph-based multi-agent workflows for DashFlow - build stateful applications with cycles, conditional routing, and checkpointing.

## Usage

```rust
use dashflow::StateGraph;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<String>,
    iteration: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = StateGraph::new();

    // Add nodes
    graph.add_node_from_fn("researcher", |mut state| {
        Box::pin(async move {
            state.messages.push("Research complete".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("writer", |mut state| {
        Box::pin(async move {
            state.messages.push("Draft complete".to_string());
            Ok(state)
        })
    });

    // Add edges
    graph.add_edge("researcher", "writer");
    graph.add_edge("writer", "__end__");

    // Set entry point and compile
    graph.set_entry_point("researcher");
    let app = graph.compile()?;

    // Run
    let initial_state = AgentState {
        messages: Vec::new(),
        iteration: 0,
    };
    let result = app.invoke(initial_state).await?;

    println!("Messages: {:?}", result.state().messages);
    Ok(())
}
```

## Key Features

- **StateGraph**: Build workflows as directed graphs with cycles
- **Conditional Routing**: Dynamic next-step determination based on state
- **Parallel Execution**: Fan-out to multiple nodes, then aggregate
- **Checkpointing**: Save and restore workflow state (memory and file-based)
- **Streaming**: Stream intermediate results and events
- **Type Safety**: Generic state with full compile-time checking
- **DashOptimize**: Automatic prompt optimization using data-driven algorithms (BootstrapFewShot, MIPROv2, KNNFewShot, COPRO, Ensemble)
- **Production Features**: A/B testing, cost monitoring, data collection for continuous optimization

## Examples

Run examples to see patterns in action:
```bash
cargo run --package dashflow --example basic_graph
cargo run --package dashflow --example checkpointing_workflow
cargo run --package dashflow --example financial_analysis_agent
```

See [examples/](examples/) for 60+ comprehensive examples.

## Documentation

- **[DashOptimize Guide](../../docs/DASHOPTIMIZE_GUIDE.md)** - Comprehensive prompt optimization documentation
- **API Reference** - Generate with `cargo doc --package dashflow --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow = "1.11"
```

## Feature Flags

DashFlow uses feature flags to enable optional functionality. By default, SIMD acceleration is enabled.

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `default` | Default features (includes `simd`) | `simd` |
| `simd` | SIMD acceleration for vector operations | `simsimd` |
| `dashstream` | Streaming support for real-time events | `dashflow-streaming` |
| `observability` | Metrics and telemetry integration | `dashflow-observability` |
| `tracing` | LangSmith tracing integration | `dashflow-langsmith` |
| `network` | Distributed graph networking | `axum`, `mdns`, etc. |
| `mcp-server` | Model Context Protocol server | `axum`, `dashflow-module-discovery` |
| `dhat-heap` | Heap profiling for development | `dhat` |

### Example Usage

```toml
# Minimal (no default features)
[dependencies]
dashflow = { version = "1.11", default-features = false }

# With specific features
[dependencies]
dashflow = { version = "1.11", features = ["dashstream", "observability"] }

# All features
[dependencies]
dashflow = { version = "1.11", features = ["dashstream", "observability", "tracing", "network", "mcp-server"] }
```
