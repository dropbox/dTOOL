# Graph View + Graph State: Audit & Better Design

**Last Updated:** 2025-12-30 (Worker #2175 - M-2047 Fix stale file:line refs)
**Scope:** Observability UI graph tab + underlying event→state projection.

This document audits the current “live graph” view and proposes a design that:
1) cleanly separates **graph schema**, **execution/run state**, and **business state**,
2) supports **time travel** (state at any event),
3) supports **graph evolution** (schema/config/version changes over time).

---

## 1) What exists today (high level)

**UI surface (web):**
- `observability-ui/src/components/GraphCanvas.tsx`: graph rendering (ReactFlow + dagre layout) + node/edge styling.
- `observability-ui/src/components/NodeDetailsPanel.tsx`: selected node details (metadata + execution metrics + “state after node”).
- `observability-ui/src/components/ExecutionTimeline.tsx`: scrolling event list.
- `observability-ui/src/components/StateDiffViewer.tsx`: top-level key diff between current and previous state.
- `observability-ui/src/hooks/useGraphEvents.ts`: converts decoded DashStream protobuf messages into a `GraphExecution` object.
- `observability-ui/src/App.tsx`: owns WebSocket + decoder + “graph tab” state.

**Protocol:**
- `proto/dashstream.proto`: `Event` + `StateDiff` messages (RFC6902-ish operations or full snapshots).

**Backend producer (graph telemetry):**
- `crates/dashflow/src/dashstream_callback/mod.rs`: emits `EventType::GraphStart/NodeStart/NodeEnd/...` and optional `StateDiff` messages.
- `crates/dashflow/src/introspection/graph_manifest.rs`: `GraphManifest` (AI introspection; JSON contains node/edge maps).
- `crates/dashflow/src/schema.rs`: `GraphSchema` (UI-oriented; arrays, edge types, node metadata, conditional targets).

---

## 2) Audit findings (correctness + UX + scalability)

### Clarification: “JSON Patch” even though we use Protobuf

We *do* use Protobuf on the wire (`proto/dashstream.proto`). However the `StateDiff` message intentionally encodes **JSON Patch-style operations**:
- Each operation has an `op` + JSON-pointer `path` (RFC 6901) + `value` bytes.
- In practice today, those `value` bytes are usually **JSON-encoded** (`ValueEncoding::JSON` in Rust).

So: **protobuf is the envelope**, and **JSON Patch is the diff format** for state updates inside that envelope. Supporting time travel requires applying these patch operations, not only full snapshots.

### A. Schema mismatch: UI expects “GraphSchema-ish”, backend sends “GraphManifest”

The Web UI attempts to parse `attributes["graph_manifest"]` into `GraphSchema`:
- Nodes are parsed as a map (compatible with `GraphManifest.nodes`).
- **Edges are parsed as an array**, but `GraphManifest.edges` serializes as a **map** (`from -> [EdgeManifest...]`).

Effect: live runs can render nodes but silently drop edges; conditional/parallel structure and edge labels are lost.

Related: `GraphManifest` currently assigns all nodes `NodeType::Function` in `StateGraph::manifest()` (`crates/dashflow/src/graph.rs`), so “node type” and “node description” are mostly absent for live visualizations.

### B. State reconstruction ignores `StateDiff.operations`

`proto/dashstream.proto` defines `StateDiff` with either:
- `full_state` snapshot bytes, OR
- `operations` (RFC 6902 JSON Patch operations), plus `state_hash`.

The UI (`useGraphEvents.ts`) only consumes `fullState` and ignores `operations`.

Effect:
- incremental diffs won’t update the UI unless the backend falls back to full snapshots.
- “removed keys” can’t be represented via the current shallow-merge approach.

### C. Topology evolution isn’t modeled (sessions vs within-run)

You noted topology *can* change within a run (rare) and often changes between sessions (important).

Today the UI model effectively assumes:
- one schema per run,
- schema never changes after GraphStart.

That blocks a clean “topology slider with session markers” because the slider needs to move through **(run, time)** while also choosing the **schema version active at that time**.

### D. Execution timeline semantics are not anchored to a run start

There’s no run-scoped “clock” derived from `GraphStart` (proto timestamp) that all timeline items use; the timeline is effectively “best effort”.

### E. “Graph state” vs “node input/output state” is conflated

The UI shows “State After Node” in `NodeDetailsPanel`, but it is the **global** graph state, not:
- input state *at node start*,
- output state *at node end*,
- or a diff attributable to that node.

The underlying types (`NodeExecution`) already include optional `input_state`/`output_state`, but the projection does not populate `input_state`, and `output_state` is only partially populated.

### F. Not designed for multi-run (multiple `thread_id`s)

`useGraphEvents` supports multiple executions internally (`Map<threadId, GraphExecution>`), but `App.tsx` stores a single set of “current” graph variables. If multiple threads stream concurrently, the UI will interleave state.

### G. Performance risks

Heavy use of `JSON.stringify` for equality/diffing:
- O(size_of_state) per update, repeated.
- brittle with non-JSON values (Long objects, cyclic structures, large blobs).

---

## 3) Better design: three explicit layers

### Layer 1: Graph definition (schema)

**Definition:** a versioned, immutable (content-addressed) graph structure.

**Preferred UI contract:** `crates/dashflow/src/schema.rs::GraphSchema` (arrays, typed edges, conditional targets, optional positions).

**Identity:**
- `schema_id`: stable hash of the schema content (content-addressed).
- `graph_id`, `graph_name`, `schema_version` (semantic version or monotonically increasing).

**Evolution:** schema changes create a new `schema_id`. UI can diff schemas.

### Layer 2: Execution/run (per `thread_id`)

**Definition:** the event stream for one run; includes node statuses, durations, errors, and causal ordering.

**Key UI entities:**
- `RunId = thread_id`
- `RunStatus = running|completed|error`
- `EventLog[]` (ordered by `sequence` when possible)
- `NodeRunState` (status, start/end, duration, retries, etc.)

**Evolution:** runs accumulate; the UI should support selecting runs and comparing them.

### Layer 3: Business state over time (time-travelable)

**Definition:** the JSON state the graph manipulates, represented as:
- checkpoints (full snapshots), plus
- patches (JSON Patch operations), with hashes.

**Core property:** state is reconstructable at any event index (“time travel”).

**Store shape (conceptual):**
```ts
type Seq = number;

interface RunStateStore {
  latest: unknown;                     // latest reconstructed JSON state
  checkpoints: Map<Seq, unknown>;      // periodic snapshots for fast seeking
  patches: Map<Seq, JsonPatchOp[]>;    // StateDiff.operations
  changedPaths: Map<Seq, string[]>;    // derived from ops; used by UI
  provenance: Map<string, { seq: Seq; node?: string }>; // json-pointer -> last writer
}
```

**Evolution:** because state is patch-based, the UI can show:
- “what changed”, by JSON pointer,
- “who changed it”, by node context,
- “how it changed over time”, via per-key history.

---

## 4) UI proposal: “Time-travel debugger” for graphs

### Layout

**Top bar**
- Run selector (thread_id) + status
- Schema/version badge (schema_id/hash)
- Expected graph selector (pin) + mismatch banner
- Time controls: “Live” toggle, scrubber over event sequence/time, step forward/back, “jump to node start/end”
- Search: node name, state key/path

**Main split**
- Center: Graph canvas (stable layout; status overlays)
- Right: Inspector (tabs)
  - Node (metadata, timings, retries, errors)
  - State Lens (inputs/outputs + changed paths)
  - Config (node_config version/hash; if available)

**Bottom**
- Event timeline (filterable; click selects time index)
- Diff panel (changed paths for selected step)

### Key behaviors

1) **Live mode:** auto-advances time index to the latest event.
2) **Scrub mode:** selecting a timeline event reconstructs state at that event and renders:
   - node statuses up to that point,
   - state snapshot at that point,
   - diff from prior point,
   - edge traversal highlights.
3) **Node-centric view:** selecting a node shows:
   - input state at `NodeStart` (snapshot),
   - output state at `NodeEnd` (snapshot),
   - diff attributable to the node (patch subset).

### Expected graph (baseline) + mismatch highlighting

This visualization needs a first-class concept of an **expected graph** so the UI can warn when the telemetry stream does not match what the operator believes is running.

**Definitions**
- **Expected graph**: a schema/version the operator pins as the baseline (by `graph_id` + `schema_id`, or just `schema_id` hash).
- **Declared graph**: the schema/version announced by the run (typically at `GraphStart`).
- **Observed graph**: what we can infer from runtime events (nodes referenced, edges traversed, schema updates).

**Mismatch cases to highlight**
- **Schema mismatch**: `declared.schema_id != expected.schema_id` (most common between sessions).
- **Topology drift within a run** (rare): schema changes mid-run; show a timeline marker “Schema updated” and switch schema segments.
- **Out-of-schema events**: an event references a node/edge not present in the active schema at the cursor (render as dashed red “unknown node”, and raise a warning).
- **Unattributed diffs/events**: state diffs or events arrive without a usable `thread_id` (or before we have any declared schema); keep them quarantined and show “unbound telemetry” rather than merging into the selected run.

**UX**
- Top bar shows: `Expected: <name/hash>` and `Actual: <name/hash>`.
- When mismatch exists: show a prominent banner (and optionally color the schema segment red in the slider).
- Provide a one-click action: “Set expected = this run” (pin the current run’s declared schema as baseline).

---

## 5) Showing “how the graph may evolve”

### A. Schema evolution (between versions)

Add a “Schema History” panel:
- group runs by `graph_id`/`schema_id`
- show diffs: nodes added/removed, edges changed, node type changes, config schema changes
- allow “compare schema A vs B” and “compare run A vs B”

This aligns with the versioning/registry roadmap (`ROADMAP_GRAPH_VERSIONING_AND_REGISTRY.md`).

### B. Runtime evolution (during a run)

Use existing event types to communicate execution path:
- `edge_traversal`, `conditional_branch`, `parallel_start/end`

UI should:
- animate the traversed edge(s) at that time index,
- pin the chosen conditional branch,
- show fan-out/fan-in boundaries for parallel blocks.

### C. Config evolution (node configs/prompts)

`GraphManifest` already has `node_configs` (with version + hash + updated_by). To visualize evolution:
- display node config version/hash in node inspector,
- add an event type or attribute for config updates, so config evolution can be shown in the timeline.

---

## 6) Decouple rendering: multiple views, one model

This is the “separate rendering layer” you asked for.

### A. Canonical intermediate model (renderer-agnostic)

Define a **single**, normalized model that every renderer consumes:

- `SchemaTimeline`: a time-indexed sequence of schema versions.
- `RunTimeline`: an event log + derived node execution overlay for one `thread_id`.
- `StateTimeline`: checkpoints + patches for time travel.
- `Cursor`: `(thread_id, seq)` (or `(thread_id, timestamp)` mapped to nearest seq).

Then renderers are pure functions:
```ts
renderReactFlow(viewModel)   // canvas
renderMermaid(viewModel)     // text mode
renderTable(viewModel)       // optional: nodes list / heatmap
```

### B. Mermaid “text mode”

Mermaid becomes just another renderer for the same `viewModel`:
- Generate `graph TD` (or `flowchart TD`) from the *schema at the cursor*.
- Overlay execution using node labels/classes (e.g., `[✓]`, `[⚡]`, `[✗]`) and optional edge styling.

Snap-to-start is fine: the first version can simply regenerate the Mermaid text when the cursor changes.

---

## 7) Practical implementation path (incremental)

1) **Make schema ingestion correct**
   - Option A (recommended): emit `GraphSchema` JSON on `GraphStart` (e.g., `attributes["graph_schema_json"]`).
   - Option B: update UI to parse `GraphManifest` shape (edge map) and degrade gracefully when metadata is absent.

2) **Make state reconstruction correct**
   - Apply `StateDiff.operations` (JSON Patch) in the UI.
   - Maintain periodic checkpoints for fast seeking and hashing for integrity.

3) **Model topology evolution**
   - Support “schema segments”:
     - Between sessions: each run gets its own `schema_id` at GraphStart.
     - Within a run (rare): accept a `SchemaUpdate` event/attribute that swaps the active schema at a specific `seq`.
   - UI slider shows:
     - session markers (GraphStart per `thread_id`)
     - schema segments (by `schema_id`) within the selected run or across runs

4) **Refactor UI state into a run-scoped store**
   - Normalize by `thread_id`.
   - Keep a bounded event log and state history per run.

5) **Add time travel + node-attributed diffs**
   - Drive graph + state panels from a selected time index.
   - Capture node input/output snapshots and per-node diffs.

6) **Add schema/run comparison**
   - Compare versions and runs to show how behavior evolves.

7) **Add Mermaid renderer**
   - Use the same `viewModel` as the canvas.
   - Provide a toggle: `Canvas | Text (Mermaid)`.

---

## 8) Implementation Status and Correctness Notes

### Phase 0.6 Implementation (N=500-502)

✅ **Completed:**
- Backend emits `graph_schema_json` and `schema_id` on GraphStart
- UI prefers `graph_schema_json` over `graph_manifest`
- Run-scoped store (`useRunStateStore`) with cursor (thread_id, seq)
- JSON Patch application (`jsonPatch.ts` implements RFC 6902)
- Timeline slider with run selector and live toggle
- Mermaid text mode renderer
- 44 unit tests (20 jsonPatch + 24 mermaid)
- View toggle (Canvas/Mermaid) integrated into App.tsx

### Phase 0.6.1 Correctness Fixes (N=532)

#### Bug 1: Baseline State Missing for Patches (FIXED)

**Problem:** The first `StateDiff.operations` patch was being applied onto `{}` (empty object) instead of the actual initial state. This silently lost any unchanged keys from the initial state.

**Root Cause:** `GraphStart` event stored the initial state for internal diff computation but did NOT emit it as an attribute for the UI to consume.

**Fix (N=532):**
- Backend now emits `initial_state_json` attribute on GraphStart events
- UI's `extractState()` checks for `initial_state_json` first, then `state_json`, then `state`
- Time-travel to the first event now correctly shows the full initial state

**Relevant Files:**
- `crates/dashflow/src/dashstream_callback/mod.rs:649-672` - Emits `initial_state_json`
- `observability-ui/src/hooks/useRunStateStore.ts:738-767` - Extracts initial state (`extractState` function)

#### CRITICAL INVARIANT: GraphStart Must Emit Initial State

**For time-travel state reconstruction to work correctly:**
1. `GraphStart` MUST emit `initial_state_json` attribute containing the full initial state
2. All subsequent `StateDiff` patches are applied relative to this baseline
3. Without the initial state, the first patch appears to create all keys from nothing

**Protocol Contract:**
```
GraphStart {
  attributes: {
    graph_schema_json: string,    // UI-friendly schema
    schema_id: string,            // Content-addressed hash
    initial_state_json: string,   // REQUIRED for time-travel
    graph_name: string,           // Optional
    graph_entry_point: string,    // Optional
  }
}
```

#### Bug 2: Graph Tab State Management (DOCUMENTED)

**Concern:** Graph tab used both `useRunStateStore()` AND `useGraphEvents()`, potentially causing split-brain between two state sources.

**Current Architecture (Staged Migration):**
- Both stores receive WebSocket messages for backwards compatibility
- `effective*` variables (lines 232-235 of App.tsx) **always prefer** `useRunStateStore` data
- `useGraphEvents` serves as fallback when no time-travel data is available

**Resolution:** The current architecture correctly prioritizes `useRunStateStore`. The split-brain concern is mitigated by the `effective*` priority logic.

**DONE (Phase 755, #968):** `useGraphEvents` was removed and all state migrated to `useRunStateStore`. See `observability-ui/ARCHITECTURE.md:154` for details.
