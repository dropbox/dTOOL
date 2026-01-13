# dashflow-cli

Unified DashFlow CLI for streaming telemetry, prompt optimization, evaluation, and training.

## Installation

```bash
cargo install --path crates/dashflow-cli
```

## Commands

Commands are organized into categories:

**Timeline (recommended):**
- **timeline live** - Live graph execution TUI (replaces `watch`)
- **timeline replay** - Time-travel debugging (replaces `replay`)
- **timeline view** - Static graph visualization (replaces `visualize view`)
- **timeline export** - Export visualization to HTML (replaces `visualize export`)

**Streaming Telemetry:**
- **tail** - Stream live events from Kafka
- **inspect** - Thread details and execution history
- **diff** - Compare state between checkpoints
- **export** - Export thread data to JSON
- **flamegraph** - Performance visualization
- **costs** - Token usage and cost analysis
- **profile** - Detailed performance profiling
- **analyze** - Analyze exported JSON files offline

**Prompt Optimization:**
- **optimize** - Run prompt optimization on a graph
- **eval** - Evaluate graph performance on test data
- **train** - Train or fine-tune models (distillation, RL)
- **dataset** - Dataset utilities (generate, validate, inspect)

**Developer Tools:**
- **debug** - Step-through graph execution debugger
- **introspect** - Platform and runtime introspection
- **patterns** - Pattern detection in execution traces

**Infrastructure:**
- **status** - Infrastructure health checks
- **locks** - Parallel AI development coordination
- **new** - Project scaffolding

## Usage

```bash
# Stream all events
dashflow tail

# Watch live execution with TUI (recommended)
dashflow timeline live --thread session-abc123

# Analyze specific thread
dashflow inspect --thread session-abc123 --stats

# Replay execution (recommended)
dashflow timeline replay --thread session-abc123 --from-checkpoint checkpoint-abc

# Token cost analysis
dashflow costs --by-node

# Performance profiling
dashflow profile --thread session-abc123

# Generate flamegraph
dashflow flamegraph --thread session-abc123 -o perf.svg

# Run optimization
dashflow optimize --graph ./my_graph.yaml --dataset ./train.jsonl

# Evaluate performance
dashflow eval --graph ./my_graph.yaml --dataset ./test.jsonl
```

## Documentation

- **[DashFlow Streaming Protocol](../../docs/DASHSTREAM_PROTOCOL.md)** - Protocol specification
- **[Main Repository](../../README.md)** - Full project documentation
