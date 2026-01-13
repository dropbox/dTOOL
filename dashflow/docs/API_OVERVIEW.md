# DashFlow API Overview

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

Quick reference for key APIs. For complete documentation, generate rustdoc with `./scripts/generate_docs.sh`.

---

## Core Types

### StateGraph

The main entry point for building agent workflows.

```rust
use dashflow::StateGraph;

let graph = StateGraph::<MyState>::new("my_workflow")
    .add_node("agent", agent_fn)
    .add_edge("__start__", "agent")
    .add_edge("agent", "__end__");
```

### CompiledGraph

A compiled, executable graph.

```rust
let compiled = graph.compile()?;
let result = compiled.invoke(initial_state).await?;
```

### ExecutionTrace

Captures execution telemetry. See [DESIGN_INVARIANTS.md](../DESIGN_INVARIANTS.md) for details.

```rust
use dashflow::introspection::ExecutionTrace;

let trace = compiled.get_execution_trace(thread_id);
```

---

## Key Crates

| Crate | Purpose |
|-------|---------|
| `dashflow` | Core framework, graph types, introspection |
| `dashflow-cli` | Command-line interface |
| `dashflow-streaming` | Streaming telemetry (Kafka, WebSocket) |
| `dashflow-openai` | OpenAI integration |
| `dashflow-anthropic` | Anthropic/Claude integration |
| `dashflow-evals` | Evaluation framework |
| `dashflow-prometheus-exporter` | Metrics export |

---

## Error Handling

DashFlow uses `Result<T, dashflow::Error>` for fallible operations.

```rust
use dashflow::Error;

match compiled.invoke(state).await {
    Ok(result) => println!("Success: {:?}", result),
    Err(Error::Timeout { node, duration }) => println!("Node {} timed out", node),
    Err(Error::NodeError { node, source }) => println!("Node {} failed: {}", node, source),
    Err(e) => println!("Other error: {}", e),
}
```

---

## Introspection APIs

### Platform Level

```rust
let info = compiled.platform_introspection();
```

### App Level

```rust
let manifest = compiled.manifest();
let introspection = compiled.introspect();
```

### Live Level

```rust
let executions = compiled.live_executions();
let tracker = compiled.execution_tracker();
```

---

## Generating Full Documentation

```bash
./scripts/generate_docs.sh
open target/doc/dashflow/index.html
```

---

## See Also

- [API_STABILITY.md](API_STABILITY.md) - Version guarantees
- [DESIGN_INVARIANTS.md](../DESIGN_INVARIANTS.md) - Architectural rules
- [CLI_REFERENCE.md](CLI_REFERENCE.md) - Command-line documentation
