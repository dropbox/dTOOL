# DashFlow Internal Architecture (Contributor Guide)

**Last Updated:** 2026-01-03 (Worker #2361 - Document core execution internals)

This document explains how DashFlow’s *core runtime* is structured internally, with pointers to the key modules and the end-to-end execution path from `StateGraph` construction to `CompiledGraph` execution, telemetry, and persistence.

If you’re looking for API-level concepts and patterns (LCEL, callbacks, agents), start with `docs/ARCHITECTURE.md` and `docs/GOLDEN_PATH.md`. If you’re looking for crate-level organization across the workspace, start with `docs/CRATE_ARCHITECTURE.md`.

---

## Mental Model

At the center of DashFlow is a “compile then execute” workflow:

1. Build a graph (`StateGraph<S>`) out of nodes and edges.
2. Compile it into an execution-ready plan (`CompiledGraph<S>`), capturing configuration.
3. Execute it (`invoke*` / `stream*`) using the executor loop and scheduler.
4. Emit telemetry during execution, and persist traces/events (default ON; opt-out via env vars).

Key types:
- Graph builder: `crates/dashflow/src/graph/mod.rs` (`StateGraph<S>`)
- Execution plan: `crates/dashflow/src/executor/mod.rs` (`CompiledGraph<S>`)
- Execution loop: `crates/dashflow/src/executor/execution.rs` (`invoke()` and friends)

---

## Module Map (Core Crate)

This is the “where do I find X?” map for the `dashflow` crate:

| Concern | Primary Modules | Notes |
|--------|------------------|------|
| Graph definition | `crates/dashflow/src/graph/`, `crates/dashflow/src/node.rs`, `crates/dashflow/src/edge.rs` | Nodes + edges + compile-time constraints. |
| Execution engine | `crates/dashflow/src/executor/` | `CompiledGraph<S>` + execution loop + tracing integration. |
| Scheduling | `crates/dashflow/src/scheduler/` | Work-stealing scheduler hooks used for distributed execution. |
| State model | `crates/dashflow/src/state.rs` | `GraphState`, `MergeableState` (parallel merge), size/serialization helpers. |
| Streaming | `crates/dashflow/src/stream.rs` | `StreamEvent`, `StreamMode`, channel capacities, streaming invoke variants. |
| Telemetry primitives | `crates/dashflow/src/telemetry.rs`, `crates/dashflow/src/metrics.rs`, `crates/dashflow/src/event.rs` | Events + metrics + sinks. |
| Trace persistence | `crates/dashflow/src/executor/trace.rs` | Writes project-local traces; integrates with WAL when enabled. |
| WAL / event store | `crates/dashflow/src/wal/` | Persistent observability (segments + SQLite index + event store). |
| Introspection | `crates/dashflow/src/introspection/`, `crates/dashflow/src/unified_introspection.rs`, `crates/dashflow/src/live_introspection.rs` | Execution traces, live tracking, “automatic capabilities” views. |
| “Golden path” API | `crates/dashflow/src/api.rs` | Convenience APIs intended as the default public surface. |

`crates/dashflow/src/lib.rs` is the top-level re-export and is a good orientation pass.

---

## Execution Lifecycle (End-to-End)

### 1) Build a `StateGraph<S>`

`StateGraph<S>` is a builder holding:
- Named nodes (`HashMap<String, BoxedNode<S>>`)
- Edge lists (simple edges, conditional edges, parallel edges)
- Graph metadata and node configs for visualization/introspection

See `crates/dashflow/src/graph/mod.rs`.

### 2) Compile into a `CompiledGraph<S>`

`CompiledGraph<S>` is the “execution plan” holding:
- Graph structure: nodes + edges (Arc-wrapped to avoid cloning in streaming/multi execution)
- Execution configuration: timeouts, recursion limit, retry policy, channel capacities
- Persistence/telemetry hooks: callbacks, checkpointer, trace base dir, live introspection tracker
- Optional distributed scheduling: `WorkStealingScheduler<S>`

See `crates/dashflow/src/executor/mod.rs` (`CompiledGraph<S>` fields and configuration methods).

### 3) Run the executor loop

The main entrypoint is:
- Parallel-capable graphs: `CompiledGraph<S>::invoke()` in `crates/dashflow/src/executor/execution.rs` (requires `S: MergeableState`)
- Sequential-only graphs: `invoke_sequential()` (does not require merge support)

Conceptually, the loop:
1. Sets up execution context (execution stack + decision tracking context).
2. Applies a *graph timeout* (defaulted if not explicitly configured).
3. Repeatedly picks the next node(s) based on edge types:
   - simple edge → one next node
   - conditional edge → one chosen alternative based on state
   - parallel edge → multiple nodes, executed concurrently, then merged via `MergeableState`
4. Executes node(s), enforcing per-node timeout and retry policy if configured.
5. Emits events and metrics (and streaming events if using `stream()` variants).
6. Produces an `ExecutionResult<S>` containing final state + metadata.

See `crates/dashflow/src/executor/execution.rs` for `invoke()` and helper logic (e.g., computing state changes, selecting next nodes, emitting observability events).

---

## Telemetry, Traces, and Persistence

DashFlow’s default is “full opt-in by default, opt-out only” (see Invariant 6 in `DESIGN_INVARIANTS.md`). The internal architecture reflects that:

### Trace persistence (project-local + WAL integration)

When enabled, graph execution builds an `ExecutionTrace` and persists it:
- Project-local traces: `{trace_base_dir}/.dashflow/traces/…`
- WAL integration (user-global): `~/.dashflow/wal/…` via the event store

Key implementation:
- `crates/dashflow/src/executor/trace.rs` (trace construction and persistence)
- `crates/dashflow/src/wal/mod.rs` (WAL + global EventStore singleton)

Performance notes (recent fixes):
- PERF-002: global `EventStore` singleton avoids per-invoke SQLite setup overhead.
- PERF-003: trace persistence is spawned in a background blocking task to avoid synchronous file I/O in the hot path.

### WAL (Write-Ahead Log)

The WAL system provides persistent, queryable telemetry:
- Append-only segments in `~/.dashflow/wal/`
- SQLite index at `~/.dashflow/index.db`

See `crates/dashflow/src/wal/mod.rs` for a high-level architecture overview and configuration env vars.

---

## Extension Points (What You Modify for Features)

### Nodes

Most feature work lands in node implementations or helpers around nodes:
- Node trait/types: `crates/dashflow/src/node.rs`
- Common patterns: `crates/dashflow/src/api.rs` (preferred public surface)

### Callbacks and event sinks

Execution emits events through `EventCallback<S>` and related event plumbing:
- Event types: `crates/dashflow/src/event.rs`
- Decision tracking helper: `crates/dashflow/src/decision_tracking.rs`
- WAL callback: `crates/dashflow/src/wal/callback.rs`
- DashStream callback (feature-gated): `crates/dashflow/src/dashstream_callback/`

### State persistence (checkpointers)

`CompiledGraph` can be configured with a `Checkpointer<S>`:
- Trait and core logic live in `crates/dashflow/src/checkpoint.rs`
- Backend crates live elsewhere in the workspace (see `docs/CRATE_ARCHITECTURE.md`).

### Scheduling / distributed execution

Distributed execution and work-stealing scheduling hooks live in:
- `crates/dashflow/src/scheduler/`

---

## Recommended Reading Order (Contributors)

1. `crates/dashflow/src/lib.rs` (module overview + re-exports)
2. `crates/dashflow/src/graph/mod.rs` (`StateGraph<S>` builder)
3. `crates/dashflow/src/executor/mod.rs` (`CompiledGraph<S>` structure + configuration)
4. `crates/dashflow/src/executor/execution.rs` (execution loop and control flow)
5. `crates/dashflow/src/state.rs` (`GraphState`, `MergeableState`, serialization/size constraints)
6. `crates/dashflow/src/stream.rs` (streaming surface + event types)
7. `crates/dashflow/src/executor/trace.rs` and `crates/dashflow/src/wal/mod.rs` (persistence + performance constraints)
8. `DESIGN_INVARIANTS.md` (especially Invariant 6: defaults-on behavior)
