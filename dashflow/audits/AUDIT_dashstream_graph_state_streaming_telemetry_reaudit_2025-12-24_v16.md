# DashStream Graph State + Streaming Telemetry Re-Audit (v16) — 2025-12-24

This is a follow-up to:
- `audits/AUDIT_dashstream_streaming_metrics_telemetry_reaudit_2025-12-23_v13.md`
- `audits/AUDIT_dashstream_graph_state_streaming_telemetry_reaudit_2025-12-23_v14.md`
- `audits/AUDIT_dashstream_graph_state_streaming_telemetry_reaudit_2025-12-23_v15.md`

Scope: DashStream (Kafka → websocket-server → observability-ui) telemetry correctness, replay/resume semantics, and graph-state reconstruction safety.

---

## Executive Summary (skeptical)

v15 identified two P0s (M-663, M-669) showing that resume/replay was *structurally incorrect*:
- UI treated `Header.sequence` as a global cursor, but it is per-`thread_id`.
- UI synthetic sequences could collide with real sequences, causing dedup to drop real telemetry.

This pass confirms those failures were real and broad (they affected normal multi-run topics and batching).

This repo now contains an implementation that addresses the core correctness mismatch by:
1) Making resume cursor **per-thread**, and
2) Ensuring sequences are **never 0** and **never collide** with synthetic values,
3) Making replay persistence cover **more message types**, including EventBatch unbatching.

However, even with a per-thread cursor, there are still correctness gaps that v16 elevates:
- “Catch-up” cannot include *threads the client never saw* during disconnect; a true global cursor still does not exist (new v16 item).
- Producer-side ordering/dropping can still permanently corrupt UI state without checkpoints/resync (M-666/M-671 remain).
- UI state hash verification remains racy (M-670 remains).

---

## Verified Fixes / Addressed Issues (with evidence)

### ✅ M-663 (P0): Resume cursor wrong (global vs per-thread)
**Fix direction implemented:** UI stores `lastSequencesByThread` and requests resume with a map keyed by thread_id, server replays by per-thread cursors.

Evidence:
- UI sends `lastSequencesByThread` (and still includes best-effort `lastSequence` for rollout compatibility): `observability-ui/src/App.tsx`
- Server parses `lastSequencesByThread` and replays per thread: `crates/dashflow-observability/src/bin/websocket_server.rs`
- ReplayBuffer storage and Redis keys are thread-scoped: `crates/dashflow-observability/src/bin/websocket_server.rs`

Residual risk:
- This is “correct per-thread resume”, not “global catch-up”. See v16 new item M-674.

### ✅ M-669 (P0): Synthetic sequences collide with real → dedup drops real telemetry
**Fix direction implemented:** synthetic sequences are negative; inner EventBatch events use real header sequence when available.

Evidence:
- Synthetic cursor is negative and decrements: `observability-ui/src/hooks/useRunStateStore.ts`
- EventBatch inner events pass through real header `sequence` when present: `observability-ui/src/hooks/useRunStateStore.ts`
- Decoder treats `sequence=0` as “missing”, not as a real cursor: `observability-ui/src/proto/dashstream.ts`

### ✅ M-664 (P1): websocket-server sequence extraction was Event-only
**Fix direction implemented:** websocket-server extracts `Header` for multiple message types and validates/stores per-thread sequence when present.

Evidence:
- Header extraction includes Event/TokenChunk/StateDiff/ToolExecution/Checkpoint/Metrics/Error/ExecutionTrace: `crates/dashflow-observability/src/bin/websocket_server.rs`

### ✅ M-665 (P1): EventBatch breaks resume/replay (header sequence=0)
**Fix direction implemented:** websocket-server unbatches EventBatch into per-Event messages, re-encodes and replays as normal sequenced events.

Evidence:
- EventBatch unbatching + re-encode path: `crates/dashflow-observability/src/bin/websocket_server.rs`
- UI decoder computes effective sequence using max inner event sequence (best-effort): `observability-ui/src/proto/dashstream.ts`

### ✅ M-667 (P1): `enable_state_diff=false` still emits `initial_state_json`
**Fix direction implemented:** `initial_state_json` emission is gated by `enable_state_diff`.

Evidence:
- Gating helper in callback: `crates/dashflow/src/dashstream_callback.rs`

### ✅ M-668 (P1): Checkpoint messages unusable in UI (decoder missing header/threadId/seq)
**Fix direction implemented:** decoder extracts checkpoint header fields and timestamp.

Evidence:
- `DecodedMessage` for checkpoints now includes `threadId`, `sequence`, `timestamp`: `observability-ui/src/proto/dashstream.ts`

Residual risk:
- UI store still does not use checkpoints as a recovery primitive (M-671 remains).

---

## Still Open (high-impact correctness issues)

### 🔴 M-666 (P1): Producer can reorder events per-thread
Even with per-thread resume, the producer (`DashStreamCallback`) can emit messages via multiple async paths; Kafka preserves per-partition order, but the producer’s own scheduling can reorder *before* publish.

Impact:
- UI can see GraphStart/StateDiff/Event ordering anomalies.
- Sequence gaps/reorders become “normal” noise, which destroys the value of sequence-based correctness signals.

Direction:
- Single per-callback queue that serializes all outbound telemetry for a thread_id, or a single batching worker that preserves producer order.
- Concretely: refactor `DashStreamCallback` to have **one** `mpsc::Sender<DashStreamMessage>` per callback and **one** background task that assigns sequence and publishes to Kafka in-order.
  - Do not spawn separate tasks for Event vs StateDiff paths; everything must go through the same queue.
  - File: `crates/dashflow/src/dashstream_callback.rs`

Acceptance:
- Force scheduler variance (artificial sleeps) and confirm observed sequence order is monotonic for a single thread_id.
- UI does not log out-of-order insertions for a single run under normal load.

### 🔴 M-670 (P1): UI `state_hash` verification is racy (false corruption)
Hash verification currently runs async and can hash a mutating reference. This produces false mismatches and makes “corruption” flags non-actionable.

Direction:
- Hash a deep-cloned snapshot of state *at the exact seq* (or serialize an immutable snapshot) before async hash computation.
  - If hashing is expensive, snapshot synchronously and hash asynchronously.
  - File: `observability-ui/src/hooks/useRunStateStore.ts`

Acceptance:
- Run with high throughput state diffs; no intermittent hash mismatches unless the backend actually sent a mismatching `state_hash`.

### 🔴 M-671 (P2): No checkpoint/resync strategy
Even after v16 decoding support, the system still treats dropped StateDiff as permanent corruption.

Direction:
- Define a real recovery contract: checkpoints at intervals, resync request/response, and UI application logic.

### 🔴 M-647 (P1): Metric contract violations (cross-target name reuse)
This audit pass did not observe a completed fix for the “same metric name, different buckets/labels” problem. This corrupts alert math and makes dashboards lie.

Direction:
- Centralize metric definitions (name/labels/buckets) or rename to component-scoped names.

---

## Telemetry + Metrics Gaps (measurement quality issues)

### Missing “resume/replay” observability
After v15, replay/resume was a correctness-critical mechanism but it is still under-instrumented. Operators need to know:
- how often clients resume,
- how many messages were replayed,
- how often replay gaps occur,
- whether legacy mode is being used.

Fix direction:
- Add websocket-server Prometheus metrics (component-scoped) such as:
  - `websocket_resume_requests_total{mode="per_thread|legacy"}`
  - `websocket_replay_messages_total{mode="per_thread|legacy"}`
  - `websocket_replay_gap_total`
  - `websocket_replay_thread_count_histogram` (or a gauge at resume time)
- File: `crates/dashflow-observability/src/bin/websocket_server.rs`

### “Message loss rate” remains suspect in distributed setups
If any “loss rate” metric is computed process-locally (producer-side), it will not represent end-to-end loss in multi-replica deployments and will be easy to misinterpret.

Fix direction:
- Either deprecate such metrics or redesign them as **consumer-observed** gaps (sequence gaps per thread_id) and replay gaps.

---

## New v16 Findings (similar error pattern, not previously itemized)

### **M-674 (P1): Per-thread resume cannot catch up unknown threads**
Current per-thread resume only replays threads the UI already has cursors for. If the UI disconnects and *new runs start while it’s offline*, the UI cannot request those missed “initial” messages because it does not know the thread_ids.

Why this matters:
- Operators expect “resume” to mean “catch up to present”.
- Without a global cursor (Kafka offsets or server-issued ingest cursor), the UI will miss the first N events of runs that started during disconnect.

Fix options (pick one):
1) **Kafka cursor (preferred for correctness):** server sends `(partition, offset)` metadata to UI and accepts a per-partition offset map on resume.
2) **Server-issued ingest cursor:** server assigns a monotonic `ingest_seq` for every forwarded message and includes it in a text-sideband frame paired with the binary frame.
3) **Run index listing endpoint:** on reconnect, UI asks server for “threads active since T” and then resumes each thread (still weaker than 1/2).

Acceptance:
- Disconnect UI for 60s while new runs start; on reconnect, UI sees GraphStart + early events for those runs without manual refresh.

---

## Worker Priorities (what to do next)

1) **Land and verify the P0/P1 fixes above in a single deployable unit**
   - Ensure websocket-server supports both new resume shape and legacy scalar resume (rolling upgrade safe).
   - Ensure `sequence=0` never becomes a stored cursor anywhere.

2) **Fix M-666 (ordering)**
   - Establish and enforce a producer ordering contract (single queue).

3) **Fix M-670 (hash race)**
   - Hash immutable snapshots, not a mutating object graph.

4) **Design and implement M-674 (true global catch-up)**
   - Pick Kafka offsets or server-issued cursor; write it down as a protocol contract.

5) **Finish M-671 (checkpoint/resync)**
   - After M-674, add checkpoint usage so the UI can recover after drops.

---

## Minimal Repro / Verification Scenarios (for skepticism)

1) **Two concurrent runs + batching**
   - Enable batching (`telemetry_batch_size > 1`).
   - Start two thread_ids concurrently.
   - Confirm UI shows both runs without dedup drops; confirm resume does not reset cursor to 0.

2) **Disconnect + new run starts while offline (M-674)**
   - Disconnect UI for 60s.
   - Start a run during downtime.
   - Reconnect and confirm you can see GraphStart + earliest events for that run.

3) **Restart websocket-server**
   - With Redis enabled, restart websocket-server.
   - Confirm replay works across restart (per-thread cursor), and legacy mode degrades gracefully.
