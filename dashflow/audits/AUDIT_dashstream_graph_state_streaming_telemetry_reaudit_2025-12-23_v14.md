# Re-Audit v14: DashStream Telemetry + Graph State Correctness — 2025-12-23

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

> **⚠️ STALE FILE REFERENCES:** This audit references `dashstream_callback.rs` as a single file. The file was split into `dashstream_callback/` directory (Dec 2024) with: `mod.rs` (118,680 bytes), `tests.rs` (67,261 bytes). Line numbers are historical and do not match current code.

This extends:
- `audits/AUDIT_dashstream_streaming_metrics_telemetry_reaudit_2025-12-23_v13.md`
- `audits/AUDIT_kafka_streaming_metrics_reaudit_2025-12-23_v12.md`

Re-checked against `main` HEAD through worker commits `#1565`–`#1568`.

Intent: skeptical, correctness-first. This pass focuses specifically on **graph state streaming (StateDiff / initial_state) and end-to-end replay correctness**, because “metrics exist” is irrelevant if the telemetry stream is non-replayable, non-resumable, or silently drops state updates.

---

## 0) What Is Already Fixed Since v13 (Verified)

Kafka/metrics audit follow-ups are now fixed and tracked in `ROADMAP_CURRENT.md`:
- **M-642** ✅ FIXED `#1567` (assignment-aware lag monitor)
- **M-643** ✅ FIXED `#1568` (tiered infra error alerts)
- **M-644** ✅ FIXED `#1565` (clock skew guard for E2E latency)
- **M-645** ✅ FIXED `#1567` (old data errors included in `websocket_kafka_messages_total{status="old_data_error"}`)

This audit does **not** re-litigate those; it looks for the next correctness cliffs.

---

## 1) High-Risk Graph-State Telemetry Findings (New)

### 1.1 The WebSocket replay/resume contract is incompatible with DashStream’s sequence semantics

**Observed behavior (websocket-server):**
- Client resume uses a single scalar `lastSequence` (`/ws` JSON message type `"resume"`), and server replays via `ReplayBuffer::get_messages_after(last_sequence)`:
  - `crates/dashflow-observability/src/bin/websocket_server.rs:3475`
- Replay buffer storage in Redis is keyed by `seq` alone (`{prefix}:seq:{seq}`), and range queries are also by `seq`:
  - `crates/dashflow-observability/src/bin/websocket_server.rs:328`
  - `crates/dashflow-observability/src/bin/websocket_server.rs:493`

**But the DashStream protocol defines `Header.sequence` as “ordered within thread”** (not globally ordered across the topic):
- `docs/DASHSTREAM_PROTOCOL.md:40`
- `proto/dashstream.proto:45`

**Correctness impact:**
- If multiple graph runs / thread_ids are present on the topic (expected), `sequence` collisions are guaranteed (each thread starts at 1).
- The replay buffer will overwrite colliding sequences in Redis, and “resume from lastSequence=N” is meaningless globally.
- Even if you *intend* a single-thread stream, nothing enforces it today; so the system can fail silently and the UI will show incorrect or incomplete graph state.

**This is not a “minor drift”**. It is a foundational mismatch between:
- protocol semantics (sequence scoped to thread_id), and
- websocket resume contract (single global scalar cursor).

---

### 1.2 WebSocket server’s sequencing + validation only works for `Event`, not `StateDiff` (or anything else)

**Observed behavior:**
- The websocket consumer extracts sequence and validates only when the decoded message is `Message::Event`:
  - `crates/dashflow-observability/src/bin/websocket_server.rs:2489`
- For non-Event message types (StateDiff, TokenChunk, ToolExecution, Metrics, EventBatch), `sequence` is not extracted; they become effectively “unsequenced” in the replay buffer.

**Replay correctness impact (subtle but fatal):**
- Replay memory logic only starts replay after encountering the first *sequenced* message with `seq > last_sequence`; it then includes all subsequent messages (including those without sequence):
  - `crates/dashflow-observability/src/bin/websocket_server.rs:401`
- If a `StateDiff` arrives before the next `Event` (very plausible), it is skipped because replay hasn’t “started” yet.
  - This breaks state reconstruction: you can replay the NodeEnd event but miss the diff that updates state.

**Persistence impact:**
- Only sequenced messages are persisted to Redis:
  - `crates/dashflow-observability/src/bin/websocket_server.rs:320`
- So today, StateDiff is not durably replayable across websocket-server restarts even if the in-memory replay happens to include it.

---

### 1.3 Event batching (EventBatch) breaks resume/replay unless the websocket-server understands it

**Observed behavior (producer):**
- DashStreamCallback may emit `EventBatch` messages. Batch headers use `sequence=0` by design; individual events have sequences:
  - `crates/dashflow/src/dashstream_callback.rs:577`

**Observed behavior (websocket-server):**
- It does not parse batch contents for replay sequencing or validation (same “Event-only” extraction).

**Correctness impact:**
- If batching is ever enabled (`telemetry_batch_size > 1`), most Events can become “unsequenced” at the message layer and the current replay cursor becomes meaningless.
- This can silently regress resume correctness without any compile-time signal.

---

### 1.4 DashStreamCallback can drop or reorder graph-state messages (best-effort semantics conflict with state reconstruction)

**Drop behavior is explicit:**
- Telemetry tasks use `try_acquire_owned`; at capacity they are dropped:
  - `crates/dashflow/src/dashstream_callback.rs:620`
- Batch queue `try_send` drops as well:
  - `crates/dashflow/src/dashstream_callback.rs:1240`

**State reconstruction requires completeness or checkpoints.**
Right now:
- GraphStart embeds the only baseline state (`initial_state_json`) as an Event attribute:
  - `crates/dashflow/src/dashstream_callback.rs:768`
- Subsequent updates are StateDiff messages.
- If GraphStart or any StateDiff is dropped, the UI cannot deterministically reconstruct state (there is no resync protocol implemented via Checkpoint / base_checkpoint_id).

**Ordering is also not guaranteed:**
- NodeEnd schedules sending StateDiff via `spawn_tracked` *before* sending the NodeEnd Event, but both are dispatched asynchronously and can enqueue out-of-order under scheduler variance.
- If receiver expects state diffs to correspond to specific node boundaries, out-of-order delivery can produce misleading UI states even without drops.

---

### 1.5 `enable_state_diff=false` does not actually stop state from being streamed

`DashStreamConfig.enable_state_diff` disables diffing/storage, but GraphStart still emits `initial_state_json` when it can serialize:
- `crates/dashflow/src/dashstream_callback.rs:768`

So “disable state diffs” currently still leaks baseline graph state into telemetry.

---

## 2) Secondary Graph-State Risks (Likely Bugs / Gaps)

### 2.1 Size limits are not end-to-end safe (Kafka max message size / attribute bloat)

`serialize_state_with_limit` uses `bincode::serialized_size` to estimate and then serializes to JSON:
- `crates/dashflow/src/dashstream_callback.rs:141`

Gaps:
- bincode can under-estimate JSON size significantly; a state can pass the check but still produce huge JSON.
- GraphStart schema/manifest attributes (`graph_manifest`, `graph_schema_json`, `initial_state_json`) have no explicit byte-size cap before being inserted into a protobuf string field.

Risk: telemetry becomes “send-failures under load” (or broker rejects) with no single clear symptom besides background warnings.

### 2.2 Sensitive data / secrets risk in state diffs and attributes

Metrics label redaction exists in `dashflow-observability` for Prometheus text output, but DashStream telemetry is separate:
- `crates/dashflow-observability/src/metrics.rs:96`

Graph state diffs and GraphEvent attributes can carry raw state and error strings. There is no redaction/allowlist for:
- `initial_state_json`
- `StateDiff.full_state`
- patch values
- `GraphEvent::NodeError{error}` attributes

This is a real data-exfiltration risk if user state contains prompts, secrets, or PII.

---

## 3) Worker Priority Fix Plan (Add These As New Roadmap Items)

The existing top priority is still **M-646 (P0)** “FAKE METRICS: registry split drops DashStream metrics from scrape”.
This audit adds **graph-state correctness issues** that should likely be P1 (or P0 if UI correctness is mission-critical).

### P0
1) **M-646**: Unify Prometheus registry usage so DashStream metrics are actually scraped (see v13).

### P1 (Graph state correctness)

#### M-651: Define a correct resume cursor (stop using Header.sequence as a global cursor)
**Problem:** websocket resume uses a single `lastSequence`, but DashStream `sequence` is scoped to thread_id; collisions corrupt replay.

**Fix options (pick one; document it; implement end-to-end):**
- Option A (recommended): Use Kafka `(partition, offset)` as the resume cursor.
  - Server must send clients the `(partition, offset)` of each message (envelope or side channel).
  - Replay buffer key becomes `{prefix}:{partition}:{offset}` and resume request includes both.
- Option B: Use a server-assigned monotonic “ingest sequence” stored in Redis (`INCR`) and attach that as the resume cursor.
  - Decouples from protocol semantics; works across thread_ids and message types.
- Option C: Make resume per-thread (`thread_id`, `sequence`) and require UI to track per-thread cursors.
  - This matches protocol semantics but is a larger UI + server protocol change.

**Acceptance:**
- With two different thread_ids producing concurrently, resume must replay the exact missing messages without overwrites or “false gaps”.
- Restart websocket-server: resume still works using Redis-backed replay.

#### M-652: Extract header/sequence for all message variants (Event, StateDiff, TokenChunk, ToolExecution, Metrics, EventBatch, Checkpoint, Error)
**Problem:** websocket-server sequence validation + replay metadata is Event-only.

**Fix direction:**
- Implement a helper like the library consumer’s `validate_sequences()` that extracts `Header` from any message type.
- Apply sequence validation on all messages with `sequence != 0` (and non-empty thread_id).

**Acceptance:**
- StateDiff messages participate in replay (not skipped), and sequence-gap alerts only reflect real stream gaps, not “we ignored message types”.

#### M-653: Make EventBatch replayable (or prohibit it on the websocket topic)
**Problem:** EventBatch header uses `sequence=0` by design; websocket replay breaks unless batches are unpacked.

**Fix direction:**
- Either: unpack EventBatch in websocket-server, and treat inner events as replay units, OR
- Prohibit EventBatch on the websocket server’s topic (enforce via config + docs; fail fast if seen).

**Acceptance:**
- Enabling `telemetry_batch_size > 1` does not silently break resume/replay.

#### M-654: Guarantee in-order emission semantics across Event and StateDiff (per thread)
**Problem:** DashStreamCallback sends Event and StateDiff in separate async tasks; ordering vs sequence is not guaranteed.

**Fix direction:**
- Route *all* outgoing messages through a single per-callback queue/worker that serializes sends in sequence order, OR
- Redefine semantics: sequence validates Events only, and StateDiff is explicitly unsequenced (sequence=0) and/or embedded in Event attributes.

**Acceptance:**
- A consumer validating sequences across all message types sees no reorders under load.

#### M-655: `enable_state_diff=false` must disable baseline state streaming too
**Problem:** GraphStart always emits `initial_state_json` when serializable.

**Fix direction:**
- Gate `initial_state_json` emission behind `enable_state_diff`, or introduce an explicit `include_state_in_telemetry` config with safe default.

**Acceptance:**
- With state streaming disabled, neither initial state nor diffs are emitted.

#### M-656: Implement a resync/checkpoint strategy for graph state (drop-tolerant)
**Problem:** current graph-state model requires full fidelity, but telemetry is explicitly best-effort.

**Fix direction:**
- Implement periodic `Checkpoint` messages (full state) and/or populate `base_checkpoint_id` in StateDiff, plus a resync behavior in UI/server on mismatch.

**Acceptance:**
- Induced drop of a StateDiff does not permanently corrupt UI state; system self-heals within a bounded time.

### P2 (hardening + security)

#### M-657: Enforce byte-size caps on schema/manifest/state attributes and state JSON
- Add explicit maximum bytes for `graph_manifest`, `graph_schema_json`, `initial_state_json`.
- Treat `bincode::serialized_size` errors as “skip state emission” (fail closed).

#### M-658: Add telemetry redaction/allowlist for graph state and event attributes
- Provide a safe-by-default policy for state keys or value patterns.
- Add tests demonstrating secrets do not appear in telemetry outputs.

---

## 4) “Skeptical” Verification Checklist (Worker)

After implementing M-651..M-656:
- Multi-thread correctness: run two concurrent graphs with distinct `thread_id`s; ensure resume replays both without collisions/overwrites.
- State correctness: intentionally drop a message (simulate websocket broadcast lag drop); verify state reconstruction self-heals via checkpoint/resync.
- Batch correctness: enable batching (`telemetry_batch_size > 1`) and ensure resume still functions.
