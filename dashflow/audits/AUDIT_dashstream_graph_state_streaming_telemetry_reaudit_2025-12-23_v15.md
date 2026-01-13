# Re-Audit v15: DashStream Telemetry + Graph State + UI Resume Correctness — 2025-12-23

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

> **⚠️ STALE FILE REFERENCES:** This audit references `dashstream_callback.rs` as a single file. The file was split into `dashstream_callback/` directory (Dec 2024) with: `mod.rs` (118,680 bytes), `tests.rs` (67,261 bytes). Line numbers are historical and do not match current code.

This extends:
- `audits/AUDIT_dashstream_graph_state_streaming_telemetry_reaudit_2025-12-23_v14.md`
- `audits/AUDIT_dashstream_streaming_metrics_telemetry_reaudit_2025-12-23_v13.md`

Re-checked against `main` HEAD through worker commits `#1565`–`#1569`.

Intent: skeptical, correctness-first. This pass focuses on **graph state reconstruction correctness** and **resume/replay correctness across websocket-server + observability-ui**, because the system currently has multiple independent “sequence/cursor” notions that do not match the DashStream protocol.

---

## 0) The Critical Theme: We Don’t Have a Single Correct Cursor

Right now we have at least four “order/cursor” concepts:

1) **DashStream Header.sequence**: protocol-defined “ordered within thread_id” (`proto/dashstream.proto`).
2) **Kafka offset**: the only globally ordered cursor per (partition, offset).
3) **websocket-server replay key**: Redis/memory keyed by `seq` only, derived only for `Event` messages.
4) **observability-ui resume cursor**: a single global `lastSequenceRef`, updated from whatever message arrived last (possibly other thread, possibly EventBatch sequence=0), sometimes synthesized.

These cannot all be correct at the same time. This audit identifies the concrete breakpoints and proposes a prioritized fix plan.

---

## 1) New High-Risk Findings (Confirmed in Code)

### 1.1 UI resume is fundamentally wrong for multi-thread streams (and can go backwards)

In `observability-ui/src/App.tsx`:
- Resume request is `{ type: "resume", lastSequence: lastSequenceRef.current }`.
- `lastSequenceRef.current` is set to `decoded.sequence` for every decoded message:
  - `observability-ui/src/App.tsx:772`

Problems:
- `decoded.sequence` is **per-thread_id**, but the UI stores it as a single global scalar.
- In a mixed topic (multiple thread_ids), sequences interleave and can be smaller than the last seen value from another thread → UI cursor can go **backwards**, triggering duplicate replays or failing to resume.
- This is not theoretical: websocket-server is a topic-wide consumer and forwards all bytes; nothing pins it to a single thread.

**Impact:** reconnect/resume is not correct as soon as multiple runs appear on the topic.

---

### 1.2 EventBatch breaks UI resume immediately (sequence=0 resets cursor)

DashStreamCallback can emit `EventBatch` when `telemetry_batch_size > 1` and its batch header uses `sequence=0` by design:
- `crates/dashflow/src/dashstream_callback.rs:577`

UI decoder surfaces EventBatch header sequence (`0`) as `decoded.sequence`:
- `observability-ui/src/proto/dashstream.ts:362`

UI then sets `lastSequenceRef.current = decoded.sequence` (0), which disables resume (`> 0` check fails) and loses cursor continuity.

**Impact:** enabling batching can silently disable resume for the UI even if websocket-server replay were otherwise correct.

---

### 1.3 UI assigns synthetic sequences that can collide with real sequences (dedup drops real data)

`useRunStateStore` uses:
- `const seq = decoded.sequence ?? nextSyntheticSeqRef.current++`
  - `observability-ui/src/hooks/useRunStateStore.ts:470`
- Events are deduplicated per-run by `seq`:
  - `observability-ui/src/hooks/useRunStateStore.ts:371`

EventBatch handling intentionally omits `sequence` for inner events:
- `observability-ui/src/hooks/useRunStateStore.ts:656`

So inner events get synthetic seqs starting at 1, which can collide with real header sequences that also start at 1 for a thread.

**Impact:** early-run EventBatch can cause the run store to drop messages as “duplicate seq” and permanently corrupt the run timeline/state.

---

### 1.4 websocket-server replay is still “Event-only” (StateDiff/TokenChunk/etc aren’t resumable)

Even after M-642/M-645 fixes, websocket-server’s replay metadata extraction still only looks at `Message::Event`:
- `crates/dashflow-observability/src/bin/websocket_server.rs:2490`

Consequences already described in v14 still apply, but now the UI makes it worse:
- UI resume cursor can be derived from a `StateDiff` seq, but websocket-server replay buffer doesn’t index state diffs by seq at all.
- Result: “resume from seq N” may replay nothing or replay the wrong set of messages.

---

### 1.5 Checkpoint messages exist in the protocol but are effectively unusable in the UI today

Protocol defines `Checkpoint { Header header, bytes state (compressed), CompressionInfo ... }`:
- `proto/dashstream.proto:313`

But the UI decoder does **not** extract header/threadId/sequence for checkpoint messages:
- `observability-ui/src/proto/dashstream.ts:434` returns `{ type: 'checkpoint', ... timestamp: now }` without `threadId` or `sequence`.

And `useRunStateStore` does not process `decoded.type === 'checkpoint'` at all (it quarantines messages without threadId).

**Impact:** any attempt to implement “drop-tolerant state resync via Checkpoint” will fail unless UI decoding + store logic is fixed too.

---

### 1.6 UI state_hash verification is racy and likely produces false corruption flags

State diff processing does:
- apply patch into `store.latestState`, then
- asynchronously `computeStateHash(store.latestState)` and compares to `stateDiff.stateHash`:
  - `observability-ui/src/hooks/useRunStateStore.ts:610`

But JSON Patch application is largely **mutating**, and `store.latestState` can be mutated again by the time the async digest runs.

Result:
- hash may be computed over “future state” rather than the state for seq N, yielding false mismatch.
- this can also mask real mismatch if later diffs happen to “fix” the state before hashing.

**Impact:** the UI’s corruption detection is not reliable today; it can page operators (or mislead debugging) with false positives.

---

## 2) Graph-State Producer Semantics Still Conflict With Best-Effort Drop/Ordering

These remain true from v14, but now verified against UI expectations:

- `enable_state_diff=false` does not stop baseline state emission (`initial_state_json`) on GraphStart:
  - `crates/dashflow/src/dashstream_callback.rs:768`
  - This contradicts docs/expectations (several docs claim disable means “send full state”).

- If `previous_state` is missing (e.g., initial_state_json serialization skipped due to size), the first NodeEnd will update previous_state but send **no diff** (old_state was None), causing early state changes to be lost for the UI.

---

## 3) Worker Priority Fix Plan (Update/Add Roadmap Items)

The repo already has:
- **M-646 (P0)** FAKE METRICS: registry split drops DashStream metrics from scrape (still top priority for metrics correctness).

This audit adds/extends graph-state correctness items with explicit UI+server coupling.

### P0 (if time-travel/resume correctness matters in production)

#### M-651 (revise): Define and implement a single correct resume cursor (UI + websocket-server + replay buffer)
**Non-negotiable:** `Header.sequence` is per-thread and cannot be used as a global cursor.

**Preferred solution:** Use Kafka `(partition, offset)` as the cursor.
- websocket-server already knows `partition` and `offset` per message (`rdkafka`).
- You need a way to communicate the cursor to the UI:
  - Option A: wrap outgoing websocket bytes with a small framing header that includes (partition, offset).
  - Option B: send a paired JSON control message with cursor metadata for each binary message (higher overhead, but simplest to implement).

**Acceptance:**
- With multiple thread_ids interleaved, reconnect resumes without duplicates or gaps.
- Cursor never goes backwards.
- Cursor works across websocket-server restarts (Redis-backed replay).

### P1 (must-fix to make state reconstruction sane)

#### M-652 (revise): Extract header/sequence for all message variants in websocket-server replay tracking
websocket-server must extract `Header` from:
- Event, StateDiff, TokenChunk, ToolExecution, Metrics, Error, ExecutionTrace, EventBatch (inner headers), Checkpoint.

**Acceptance:** replay/resume replays a consistent prefix of *all* message types, not just Events.

#### M-653 (revise): Make EventBatch end-to-end correct or forbid it on the websocket topic
If you allow batching:
- websocket-server must handle EventBatch for replay cursoring (either unpack or frame it).
- UI must not treat EventBatch header seq=0 as “cursor=0”.

**Immediate UI fix if batching is allowed:**
- In `useRunStateStore`, when processing EventBatch, pass per-event `sequence` from `event.header.sequence` instead of omitting it.

#### M-656 (revise): Checkpoint/resync strategy must include UI decoding + decompression
If we ever want drop-tolerance:
- Fix UI decoder to extract checkpoint header/threadId/sequence.
- Implement checkpoint message handling in `useRunStateStore`.
- Define encoding/compression for checkpoint.state (it is “compressed” per proto, so the UI needs zstd or base64 decode rules).

### P2 (integrity + docs hardening)

#### M-660: Fix UI hash verification race (compute on a stable snapshot)
**Fix direction:**
- Take a deep clone of `store.latestState` (or canonical string) immediately after applying the diff and before scheduling hash compute.
- Compute hash over the snapshot and compare to expected.

**Acceptance:** corruption flag changes only when stream truly diverges, not due to async timing.

#### M-661: Make synthetic sequences impossible to collide
If the UI must synthesize seq:
- use a dedicated namespace (e.g., negative numbers, or start at a high sentinel range),
- and never synthesize for EventBatch events when per-event sequence exists.

#### M-662: Align docs with actual behavior for `enable_state_diff=false`
Docs currently imply disable sends full state; code currently still emits `initial_state_json` and sends no diffs.

---

## 4) Skeptical Verification Checklist (Worker)

These should be automated as much as possible:

1) **Multi-thread replay test**:
   - Produce two thread_ids concurrently to the same topic.
   - Disconnect UI; reconnect; validate no duplicates/gaps and both runs are reconstructable.
2) **Batching test**:
   - Enable `telemetry_batch_size > 1`.
   - Ensure UI cursor does not reset to 0 and run store does not drop events as duplicate seq.
3) **Hash stability test**:
   - Add a test vector where a diff is applied, then another diff arrives before async hash resolves.
   - Ensure hash is computed over the correct snapshot and mismatches are deterministic.
4) **Checkpoint decode test**:
   - Send a Checkpoint message and ensure UI assigns threadId/seq and uses it as a resync checkpoint.
