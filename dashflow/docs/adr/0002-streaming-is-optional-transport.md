# ADR-0002: Streaming is Optional Transport

**Status:** accepted
**Date:** 2025-12-22
**Author:** DashFlow Architecture Team
**Last Updated:** 2025-12-22 (Worker #1414 - Initial ADR creation)

## Context

DashFlow includes a streaming subsystem (`dashflow-streaming`) for real-time observability via Kafka, WebSocket, and Redis. This enables:
- Distributed tracing across multiple executors
- Real-time dashboards and monitoring
- External observability integrations

There was a risk of coupling local functionality to streaming infrastructure:
- Tests requiring Kafka to run
- Local debugging needing network connectivity
- Development environments becoming complex

## Decision

**Local analysis NEVER requires external infrastructure.**

DashFlow Streaming is for:
- Communicating execution state to external listeners
- Distributed observability across multiple executors
- Real-time dashboards and monitoring

DashFlow Streaming is NOT for:
- Local execution analysis
- Running optimizers on local traces
- Debugging local executions

### Correct Patterns

```rust
// RIGHT: Local traces are always available
let trace = compiled.get_execution_trace(thread_id);
let analysis = analyze(&trace);

// RIGHT: Optional streaming when infrastructure is available
if let Some(producer) = config.stream_producer() {
    producer.send(&trace).await?;
}
```

### Anti-Patterns

```rust
// WRONG: Requiring Kafka for local analysis
let traces = TraceCollector::new("localhost:9092", topic).await?;
let analysis = analyze(traces);

// WRONG: Feature-gating local functionality
#[cfg(feature = "dashstream")]
fn collect_traces() { ... }
```

## Consequences

### Positive
- Local development requires only `cargo run`
- Tests don't need Docker/Kafka infrastructure
- Clear separation: local logic vs distributed transport
- Easier onboarding for new developers

### Negative
- Two code paths: local and distributed
- Must ensure parity between local and streamed data
- Some features (distributed coordination) genuinely need infrastructure

### Neutral
- Streaming remains available for production deployments
- Integration tests can use infrastructure when needed

## Alternatives Considered

### Alternative 1: Always Stream
- Every execution streams to infrastructure
- Rejected: Too heavy for development, testing, debugging

### Alternative 2: Local Kafka (In-Memory)
- Use embedded Kafka for local development
- Rejected: Adds complexity, doesn't solve test isolation

## Related Documents

- `DESIGN_INVARIANTS.md` Invariant 2
- `crates/dashflow-streaming/README.md`
