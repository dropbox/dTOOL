# ADR-0001: Single Telemetry System

**Status:** accepted
**Date:** 2025-12-22
**Author:** DashFlow Architecture Team
**Last Updated:** 2026-01-02 (Worker #2288 - Fixed stale crate count reference)

## Context

DashFlow has grown to 108 crates with multiple subsystems (optimization, self-improvement, debugging, visualization). Each subsystem needs access to execution telemetry data: timing, token usage, state snapshots, errors, etc.

Early in development, multiple parallel trace types emerged:
- `TraceEntry` in optimize/trace.rs
- `ExecutionTracer` in debug.rs
- Custom types in various crates

This caused:
- Data format incompatibility between subsystems
- Duplicate implementation effort
- Confusion about which trace format to use
- Difficulty building cross-cutting tools (visualizers, optimizers)

## Decision

**All execution telemetry flows through a single canonical type: `ExecutionTrace`.**

Location: `crates/dashflow/src/introspection/trace.rs`

`ExecutionTrace` tracks:
- Node executions with timing (`NodeExecution`)
- State snapshots before/after nodes
- Token usage per node
- Errors and failures
- Total duration and completion status

### Correct Patterns

```rust
// Extend ExecutionTrace when needed
impl ExecutionTrace {
    pub fn to_training_examples(&self) -> Vec<Example> { ... }
    pub fn to_dashstream(&self) -> Vec<DashStreamMessage> { ... }
}

// Consume ExecutionTrace
fn analyze(trace: &ExecutionTrace) -> Analysis { ... }
```

### Anti-Patterns (Do Not Create)

```rust
// WRONG: Creating parallel trace types
pub struct TraceEntry { ... }        // DEPRECATED
pub struct ExecutionTracer { ... }   // DEPRECATED
pub struct MyNewTraceType { ... }    // DON'T DO THIS
```

## Consequences

### Positive
- Single format enables cross-cutting tools (optimizer reads same data as debugger)
- Clear ownership: introspection module owns trace format
- New subsystems automatically interoperate
- Reduced code duplication

### Negative
- `ExecutionTrace` may grow large with many fields
- Changes to `ExecutionTrace` affect all consumers
- Must coordinate additions to avoid bloat

### Neutral
- Existing code using deprecated trace types needs migration

## Alternatives Considered

### Alternative 1: Trait-based Abstraction
- Define `Trace` trait, let each subsystem implement
- Rejected: Still leads to N implementations, conversion overhead, coordination cost

### Alternative 2: Generic Trace<T>
- Parameterize trace type per use case
- Rejected: Complexity without clear benefit; most consumers need full data

## Related Documents

- `DESIGN_INVARIANTS.md` Invariant 1
- `crates/dashflow/src/introspection/trace.rs` - Implementation
