# Introspection Module Architecture

**Last Updated:** 2025-12-15

## Overview

The introspection module provides AI self-awareness capabilities for DashFlow agents. It enables agents to query their own structure, execution state, and performance characteristics. This supports intelligent decision-making during runtime (e.g., "Am I looping?", "Where did I spend most time?").

## Module Summary

Total: 15 modules, ~19,900 lines of code (excludes tests.rs)

### Core Types

| Module | Lines | Purpose |
|--------|-------|---------|
| `trace.rs` | 1,022 | **Central type**: `ExecutionTrace` - execution history for analysis |
| `context.rs` | 305 | `ExecutionContext` - runtime state awareness (iteration, limits) |
| `state.rs` | 246 | `StateIntrospection` - JSON state inspection utilities |

### Graph Structure

| Module | Lines | Purpose |
|--------|-------|---------|
| `graph_manifest.rs` | 814 | `GraphManifest` - static graph structure (nodes, edges, metadata) |
| `capability.rs` | 638 | `CapabilityManifest` - model/tool/storage capabilities |

### Analysis Modules

| Module | Lines | Purpose |
|--------|-------|---------|
| `bottleneck.rs` | 1,105 | `BottleneckAnalysis` - identify performance bottlenecks |
| `pattern.rs` | 1,468 | `PatternAnalysis` - detect execution patterns (loops, branching) |
| `optimization.rs` | 982 | `OptimizationSuggestion` - generate improvement recommendations |
| `performance.rs` | 936 | `PerformanceMetrics` - latency, throughput, error rate tracking |

### Configuration & History

| Module | Lines | Purpose |
|--------|-------|---------|
| `config.rs` | 1,284 | `ConfigurationRecommendations` - dynamic reconfiguration |
| `decision.rs` | 656 | `DecisionHistory` - logged decisions for review |
| `resource.rs` | 1,065 | `ResourceUsage` - token/cost budget tracking |

### Integration

| Module | Lines | Purpose |
|--------|-------|---------|
| `integration.rs` | 817 | Orchestrates analysis across modules |
| `telemetry.rs` | 416 | `OptimizationTrace` - A/B testing and variant tracking |

### Tests

| Module | Lines | Purpose |
|--------|-------|---------|
| `tests.rs` | 7,991 | Unit and integration tests |

## Module Dependencies

```
                          ┌─────────────────────────────────────────────────┐
                          │                  External                       │
                          │  crate::metrics::ExecutionMetrics               │
                          └─────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Core Types Layer                                  │
│                                                                             │
│    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                │
│    │   trace.rs   │    │  context.rs  │    │  state.rs    │                │
│    │ Execution    │    │ Runtime      │    │ JSON state   │                │
│    │ Trace        │    │ Context      │    │ inspection   │                │
│    └──────────────┘    └──────────────┘    └──────────────┘                │
│            │                                                                │
└────────────┼────────────────────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Analysis Layer                                     │
│                                                                             │
│    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                │
│    │bottleneck.rs │    │ pattern.rs   │    │optimization.rs│               │
│    │ Bottleneck   │◄───│ Pattern      │───►│ Optimization  │               │
│    │ Analysis     │    │ Analysis     │    │ Suggestion    │               │
│    └──────────────┘    └──────────────┘    └──────────────┘                │
│                               │                                             │
│    ┌──────────────┐          │           ┌──────────────┐                  │
│    │performance.rs│          │           │  resource.rs │                  │
│    │ Performance  │◄─────────┘           │ Resource     │                  │
│    │ Metrics      │                      │ Usage        │                  │
│    └──────────────┘                      └──────────────┘                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Configuration Layer                                   │
│                                                                             │
│    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                │
│    │  config.rs   │    │ decision.rs  │    │ telemetry.rs │                │
│    │ Reconfig     │    │ Decision     │    │ Optimization │                │
│    │ Recommend    │    │ History      │    │ Trace        │                │
│    └──────────────┘    └──────────────┘    └──────────────┘                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Integration Layer                                    │
│                                                                             │
│    ┌──────────────┐    ┌──────────────────┐                                │
│    │integration.rs│    │ graph_manifest.rs │                               │
│    │ Orchestrator │    │ Static graph      │                               │
│    │              │    │ structure         │                               │
│    └──────────────┘    └──────────────────┘                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Runtime Trace Collection

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  StateGraph  │────►│ NodeExecution│────►│ExecutionTrace│
│  .invoke()   │     │  per node    │     │  complete    │
└──────────────┘     └──────────────┘     └──────────────┘
                                                  │
                                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Analysis Pipeline                             │
│                                                                 │
│  ExecutionTrace ──► BottleneckAnalysis ──► Bottleneck[]         │
│                 ──► PatternAnalysis    ──► Pattern[]            │
│                 ──► PerformanceMetrics ──► PerformanceAlert[]   │
│                 ──► OptimizationAnalysis ──► OptimizationSugg[] │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                                                  │
                                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Output / Actions                              │
│                                                                 │
│  ConfigurationRecommendations ──► Dynamic graph reconfiguration │
│  DecisionLog ──► Audit trail for debugging                      │
│  ResourceUsage ──► Budget alerts and termination                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Static Manifest Generation

```
┌──────────────┐                    ┌──────────────────┐
│  StateGraph  │────graph.manifest()────►│  GraphManifest   │
│              │                    │  - nodes[]       │
│              │                    │  - edges[]       │
│              │                    │  - metadata      │
└──────────────┘                    └──────────────────┘
                                              │
                                              ▼
                                    ┌──────────────────┐
                                    │ AI Self-Query    │
                                    │ "What nodes do I │
                                    │  have?"          │
                                    └──────────────────┘
```

## CLI Integration

The introspection module powers the `dashflow introspect` CLI:

```
dashflow introspect
├── search <keyword>     # Search modules by name/description
├── show <module>        # Show module details
├── list                 # List all known modules
├── cli                  # List all CLI commands
│   └── --stubs-only     # Show only unwired CLI stubs
├── health               # Run health checks
│   ├── --skip-infra     # Skip Docker/Grafana checks
│   ├── --skip-llm       # Skip LLM connectivity
│   └── --format json    # JSON output
└── ask <question>       # Natural language query (requires traces)
```

**Note:** The `introspect ask` command uses the unified four-level introspection API from `unified_introspection.rs`.

## Extension Points

### Custom Analysis

Implement analysis on `ExecutionTrace`:

```rust
use dashflow::introspection::{ExecutionTrace, NodeExecution};

fn analyze_token_efficiency(trace: &ExecutionTrace) -> f64 {
    let total_tokens: u64 = trace.nodes_executed.iter()
        .map(|n| n.tokens_used)
        .sum();
    let successful_nodes = trace.nodes_executed.iter()
        .filter(|n| n.error.is_none())
        .count();

    if total_tokens > 0 {
        successful_nodes as f64 / total_tokens as f64 * 1000.0
    } else {
        0.0
    }
}
```

### Custom Bottleneck Detection

Extend `BottleneckAnalysis` with domain-specific thresholds:

```rust
use dashflow::introspection::{
    BottleneckAnalysis, BottleneckThresholds, ExecutionTrace
};

let custom_thresholds = BottleneckThresholds {
    slow_node_ms: 500,  // Flag nodes >500ms
    high_token_usage: 2000,  // Flag nodes >2000 tokens
    error_rate_threshold: 0.1,  // Flag >10% error rate
};

let analysis = BottleneckAnalysis::with_thresholds(&trace, custom_thresholds);
for bottleneck in analysis.bottlenecks {
    println!("Found: {:?}", bottleneck);
}
```

### Custom Pattern Rules

Define domain-specific patterns:

```rust
use dashflow::introspection::{
    Pattern, PatternBuilder, PatternCondition, PatternOperator, PatternType
};

let api_loop_pattern = PatternBuilder::new("api_retry_loop")
    .pattern_type(PatternType::Loop)
    .add_condition(PatternCondition {
        field: "node_name".into(),
        operator: PatternOperator::Contains,
        value: "api_call".into(),
    })
    .threshold(3)  // Trigger after 3 consecutive API calls
    .build();
```

## Key Types Reference

### ExecutionTrace (trace.rs)

Central type for all analysis. Contains:
- `nodes_executed: Vec<NodeExecution>` - ordered execution history
- `total_duration_ms: u64` - total execution time
- `total_tokens: u64` - total token consumption
- `errors: Vec<ErrorTrace>` - any errors encountered
- `execution_metrics: Option<ExecutionMetrics>` - rich metrics from LocalMetricsBatch
- `performance_metrics: Option<PerformanceMetrics>` - real-time performance snapshot

### ExecutionContext (context.rs)

Runtime context available to nodes:
- `iteration: usize` - current iteration count
- `has_executed(node: &str) -> bool` - check execution history
- `is_near_limit() -> bool` - approaching resource limits
- `detect_loop(threshold: usize) -> Option<String>` - loop detection

### GraphManifest (graph_manifest.rs)

Static graph structure:
- `nodes: HashMap<String, NodeManifest>` - node definitions
- `edges: HashMap<String, Vec<EdgeManifest>>` - edge connections
- `metadata: GraphMetadata` - graph-level metadata

## Related Modules

- `dashflow::trace_analysis` - Shared trace analysis primitives
- `dashflow::registry_trait` - Registry trait hierarchy
- `dashflow::self_improvement` - Uses introspection data for improvement plans
- `dashflow::unified_introspection` - Four-level introspection API

## Design Decisions

1. **ExecutionTrace as central type**: All analysis modules consume `ExecutionTrace`, enabling consistent analysis across different aspects (bottlenecks, patterns, optimization).

2. **Builder patterns**: Most types use builders (`ExecutionTraceBuilder`, `PatternBuilder`) for ergonomic construction.

3. **Serialization**: All types implement `Serialize`/`Deserialize` for JSON export and persistence.

4. **Optional metrics**: `execution_metrics` and `performance_metrics` are optional fields to support both basic and rich trace data.

5. **Tests in separate file**: `tests.rs` (7,991 lines) keeps test code separate from implementation.
