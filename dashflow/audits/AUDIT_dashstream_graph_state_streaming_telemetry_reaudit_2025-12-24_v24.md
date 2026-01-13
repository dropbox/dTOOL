# v24 Skeptical Source Code Audit - DashStream Graph State / Streaming / Telemetry

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

**Date:** 2025-12-24
**Auditor:** Worker #1655 (v24 deep source code audit)
**Prior Audits:** v23 (M-749..M-760), v22 (M-739..M-748), v21 (M-719..M-738), v20 (M-707..M-716), v19, v18, v15, v14, v13, v12, v11
**Scope:** `observability-ui/`, `crates/dashflow-observability/`, WebSocket/Kafka streaming pipeline, graph-state correctness + telemetry
**Method:** Deep skeptical source code analysis — actual source, not docs

---

## Executive Summary

This v24 reaudit focuses on **DashStream graph-state correctness**, **resume/replay protocol**, and **telemetry/measurement failure modes**. I found **12 NEW issues** (M-762 to M-773), including **3 P1s**.

**Top risks (P1):**
- **UI can crash WebSocket connect** if `localStorage` contains non-numeric sequence strings (BigInt throws) — `observability-ui/src/App.tsx` (M-763).
- **FullState snapshot decode/parse failures are silent** (no `needsResync`/`corrupted`), allowing the UI to continue with stale/partial state — `observability-ui/src/hooks/useRunStateStore.ts` (M-770).
- **Checkpoint parse failures are treated as “checkpoint exists”** and can suppress `base_checkpoint_id` resync detection even when the base state is unusable — `observability-ui/src/hooks/useRunStateStore.ts` (M-771).

**Protocol / operability gaps:**
- WebSocket server **documents** `from:"earliest"` but does not implement it (M-764).
- Resume mode selection is derived from field presence, making “thread mode” effectively non-addressable by explicit client intent (M-765).
- “Thread-mode” replay sends binary frames without cursor metadata; current UI treats that as a protocol violation (M-766).

---

## Skeptical Verification of v23 “FIXED” Claims (spot-check)

I re-verified a small sample of v23 P1 fixes in current HEAD:
- **M-749:** `cursorSeq?: string` is now correct in `observability-ui/src/components/StateDiffViewer.tsx:15-17`.
- **M-750:** `canonicalJsonString()` now handles `bigint` as a string in `observability-ui/src/utils/stateHash.ts:44-49`.
- **M-751:** EventBatch max inner sequence includes `seq=0` correctly in `observability-ui/src/proto/dashstream.ts:388-410`.

No regressions found in those specific fixes. The new issues below are **new gaps**, not re-openings of those items.

---

## New Issues (M-762 to M-773)

### M-762 (P2): websocket-server thread-mode resume ignores string sequences (and filters out seq=0)
**Category:** Resume/Protocol Correctness
**Severity:** P2 (latent, but breaks documented/legacy thread-mode resume)

**Problem:** The server parses `lastSequencesByThread` using `v.as_u64()` only and ignores numeric strings. It also drops `seq=0` via `if seq > 0`, despite `seq=0` being valid for some event types (see prior M-751 logic in UI).

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:5475`
```rs
if let Some(seq) = v.as_u64() {
    if seq > 0 {
        last_sequences_by_thread.insert(thread_id.clone(), seq);
    }
}
```

**Impact:**
- Any client sending sequences as strings (current UI stores sequences as strings post M-693) cannot use thread-mode resume.
- Thread-mode resume from `seq=0` becomes a no-op.

**Fix direction:**
- Accept both JSON numbers and numeric strings, mirroring the offset parsing pattern:
  - `v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))`
- Treat `seq=0` as valid cursor input (store it; replay “after 0” is meaningful).
- Add a small unit test for parsing logic (preferably extracted helper).

**Acceptance criteria:**
- A `resume` with `lastSequencesByThread: {"t1":"0","t2":"123"}` is accepted and produces non-empty `last_sequences_by_thread`.

---

### M-763 (P1): UI localStorage restore accepts non-numeric sequence strings; BigInt() can throw and break reconnect
**Category:** Resume/Cursor Persistence
**Severity:** P1 (can break WebSocket connect and/or resume)

**Problem:** When restoring offsets/sequences from localStorage, any `string` is accepted without numeric validation. Later, `BigInt(v)` is called during resume construction and will throw on corrupted strings.

**Files:**
- `observability-ui/src/App.tsx:771-815` (restore accepts any string)
- `observability-ui/src/App.tsx:856-863` (BigInt conversion during resume)

**Impact:**
- A single corrupted value in `dashstream_lastSequencesByThread_v1:*` can hard-fail `ws.onopen` and prevent the client from reconnecting.
- Similar risk exists for offsets (bad strings can propagate and create undefined resume behavior).

**Fix direction:**
- Validate restored values:
  - For offsets/sequences: `if typeof value === 'string' && /^\d+$/.test(value) { ... } else drop + warn`
- Wrap BigInt comparisons in a defensive `try/catch` and fall back to `'0'` (and clear invalid entries) rather than crashing.
- Consider auto-cleaning corrupted keys by rewriting sanitized maps to localStorage.

**Acceptance criteria:**
- With `localStorage.setItem(sequencesStorageKey, '{"t1":"not-a-number"}')`, the UI connects and logs a warning; it does not crash or permanently fail reconnect.

---

### M-764 (P2): websocket-server documents `from:"earliest"` but does not implement it
**Category:** Protocol/Configuration Correctness
**Severity:** P2 (documented behavior mismatch)

**Problem:** The server documents `"earliest"` as a resume strategy, but only has special handling for `"latest"`. `"earliest"` currently behaves like `"cursor"` with whatever offsets were provided (often none), which is not “earliest retained”.

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server.rs:30-34` (docs promise earliest)
- `crates/dashflow-observability/src/bin/websocket_server.rs:5205-5212` (parsed, but no handling)

**Impact:**
- Operators/users cannot rely on `from:"earliest"` to force a full replay from retained history (useful for recovery / forensic analysis).

**Fix direction:**
- Implement `"earliest"` in partition mode:
  - Compute oldest retained offsets per partition (`ReplayBuffer` already has `get_oldest_offset_for_partition()`).
  - Seed `current_offsets` with those values (or `-1` semantics if your replay query expects “after X”).
- Add an integration-ish unit test around the replay-buffer oldest-offset behavior + resume path selection.

**Acceptance criteria:**
- With `{"type":"resume","lastOffsetsByPartition":{},"from":"earliest"}`, server replays from the oldest retained offsets and sends `replay_complete` with `mode:"earliest"` (or equivalent explicit field).

---

### M-765 (P2): Resume “mode” selection is implicit (field presence), preventing explicit thread-mode intent
**Category:** Protocol Design/Correctness
**Severity:** P2 (design trap; hard to reason about/extend)

**Problem:** The server selects partition-mode solely via presence of `lastOffsetsByPartition`:

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:5201-5204`
```rs
let use_partition_mode = msg.get("lastOffsetsByPartition").is_some();
```

The UI always sends `lastOffsetsByPartition` (even empty) to “enable partition discovery mode”:

**File:** `observability-ui/src/App.tsx:846-888`

**Impact:**
- Thread-mode resume/replay is effectively unreachable by explicit client intent as long as `lastOffsetsByPartition` exists.
- Makes protocol evolution brittle: adding new fields changes behavior unintentionally.

**Fix direction:**
- Add explicit `mode: "partition" | "thread"` in resume payload; keep field-presence fallback only for backwards compatibility.
- Update server to validate mode/fields coherence (e.g., `mode=thread` requires non-empty `lastSequencesByThread`).

**Acceptance criteria:**
- A client can force `mode:"thread"` even if `lastOffsetsByPartition` is present (and the server behaves predictably).

---

### M-766 (P2): Thread-mode replay sends binary frames without cursor metadata; current UI treats this as a protocol error
**Category:** Resume/Replay Correctness
**Severity:** P2 (only affects thread-mode path, but path is currently broken if used)

**Problem:** In thread-mode replay, the server sends `Message::Binary(...)` frames without first sending a cursor frame. The UI requires a cursor to be pending for every binary frame and will close the socket on “missing_cursor”.

**Server file:** `crates/dashflow-observability/src/bin/websocket_server.rs:5550-5560`
**UI file:** `observability-ui/src/App.tsx:1159-1210` (binary-without-cursor triggers protocol error)

**Impact:**
- If thread-mode replay is ever used, it will immediately disconnect clients and fail to replay.

**Fix direction:**
- Decide and document thread-mode cursor semantics:
  - Either send standard `"cursor"` messages (partition/offset) for every replayed binary message, or
  - Introduce a distinct `"thread_cursor"` message and update UI to pair binary frames with thread cursors in that mode, or
  - Deprecate/remove thread-mode replay entirely and stop advertising it.

**Acceptance criteria:**
- In thread-mode replay, the UI no longer emits `missing_cursor` protocol errors and can successfully apply replayed messages.

---

### M-767 (P2): ReplayBuffer has several hard-coded capacity/concurrency limits (not configurable)
**Category:** Operability/Configuration
**Severity:** P2 (production tuning + incident response)

**Problem:** Several key limits are compile-time constants with no env wiring:

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:749-772`
- `REDIS_MAX_SEQUENCES` (10,000)
- `DEFAULT_MAX_CONCURRENT_REDIS_WRITES` (100)
- `REDIS_PARTITION_PAGE_LIMIT` (1,000)

**Impact:**
- Operators can’t tune retention, Redis pressure, or replay behavior without code changes.
- Makes incident response harder (e.g., reduce replay pressure during Redis degradation).

**Fix direction:**
- Add env vars with validation/clamping (mirroring existing patterns like `REDIS_MESSAGE_TTL_SECS`, `REPLAY_MAX_TOTAL`).
- Log effective values at startup.

**Acceptance criteria:**
- Setting env vars changes the effective runtime limits (verified by startup logs and behavior).

---

### M-768 (P2): ReplayBuffer::clear() is not synchronized with in-flight Redis writes; cursor_reset can “clear” then immediately repopulate
**Category:** Cursor Reset / Data Integrity
**Severity:** P2 (race makes cursor_reset less trustworthy)

**Problem:** `ReplayBuffer::clear()` clears memory and deletes Redis keys, but does not coordinate with background Redis write tasks (which can still be running and can re-create keys after the scan).

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:1768-1823`

**Impact:**
- `cursor_reset_complete` can claim “bufferCleared: true” while old buffered history reappears on subsequent resumes.

**Fix direction:**
- Prefer an **epoch/key-prefix rotation** strategy for correctness:
  - Maintain `replay_epoch` in-memory; include it in `redis_key_prefix`.
  - `clear()` increments epoch immediately; old keys become unreachable; cleanup can run async.
- Alternatively (less ideal): acquire all semaphore permits before clear and block new writes while scanning/deleting.

**Acceptance criteria:**
- After cursor_reset, no pre-reset messages are returned by replay queries, even under concurrent load.

---

### M-769 (P3): ReplayBuffer::clear() uses SCAN+DEL without timeouts and may block Redis; should prefer UNLINK/async cleanup
**Category:** Operability/Perf
**Severity:** P3 (can become P2 under large keyspaces)

**Problem:** `clear()` loops SCAN and uses `DEL` (blocking deletion), with no overall timeout and no attempt to use `UNLINK`.

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:1784-1804`

**Impact:**
- Large keyspaces can cause Redis latency spikes and block the websocket handler while clearing.

**Fix direction:**
- Replace `DEL` with `UNLINK` where supported, or implement background deletion with an overall timeout.
- If adopting epoch/key-prefix rotation (M-768), move deletion fully off the request path.

**Acceptance criteria:**
- `cursor_reset` completes quickly even with many Redis keys; no long handler stalls.

---

### M-770 (P1): FullState snapshot JSON parse failures do not mark run as corrupted/needsResync (silent stale state)
**Category:** Graph State Correctness
**Severity:** P1 (silent data corruption / misleading UI)

**Problem:** When a `stateDiff.fullState` snapshot cannot be decoded/parsed, the code logs an error but does not set any “corrupted” or “needsResync” flags.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:830-873`

**Impact:**
- UI can keep showing stale/partial `latestState` while appearing healthy.
- Subsequent patch diffs may apply to an old state without explicit resync signaling.

**Fix direction:**
- On snapshot parse failure:
  - Set `store.corrupted = true` and `store.needsResync = true`
  - Record an error field (similar to `patchApplyError`) for debugging
  - Ensure UI surfaces a visible warning banner/state.
- Consider using `TextDecoder('utf-8', { fatal: true })` (see M-772).

**Acceptance criteria:**
- A malformed `fullState` causes the run to be visibly marked as `needsResync/corrupted` and stops pretending the state is trustworthy.

---

### M-771 (P1): Checkpoint parse failures store placeholder state and can suppress base_checkpoint_id resync detection
**Category:** Graph State Correctness / Resync Detection
**Severity:** P1 (false “diff chain is valid”)

**Problem:** If checkpoint state parsing fails, the UI stores a placeholder `{}` in `checkpointsById`. Later, base checkpoint validation checks only `has(id)`, so a diff referencing an unparseable checkpoint can incorrectly be treated as valid.

**Files:**
- `observability-ui/src/hooks/useRunStateStore.ts:805-819` (base checkpoint validation uses `.has()`)
- `observability-ui/src/hooks/useRunStateStore.ts:1076-1085` (on failure, store placeholder `{}`)

**Impact:**
- Resync detection can be suppressed, and diffs can be applied against an incorrect base without forcing a resync.

**Fix direction:**
- Track checkpoint validity explicitly:
  - `checkpointsById: Map<string, { seq: string; state?: object; stateValid: boolean }>`
  - Treat `stateValid=false` as “missing base” for purposes of base_checkpoint_id verification.
- For oversize checkpoints (M-738 path), also mark `stateValid=false` rather than `{}`.

**Acceptance criteria:**
- A StateDiff referencing a checkpoint whose state is unparseable/oversize sets `store.needsResync = true`.

---

### M-772 (P3): TextDecoder is used in non-fatal mode for JSON bytes; invalid UTF-8 can silently corrupt state/patch values
**Category:** Data Integrity
**Severity:** P3 (hardening; matters under data corruption)

**Problem:** `TextDecoder()` defaults to `fatal:false`, which replaces invalid sequences with U+FFFD. JSON.parse can succeed on corrupted strings, causing silent corruption.

**Files:**
- `observability-ui/src/utils/jsonPatch.ts:365-389`
- `observability-ui/src/hooks/useRunStateStore.ts:833-835` (fullState)
- `observability-ui/src/hooks/useRunStateStore.ts:1036-1038` (checkpoint)

**Fix direction:**
- Use `new TextDecoder('utf-8', { fatal: true })` for JSON decode paths and treat decode errors as hard corruption (set needsResync/corrupted, include debug context).

**Acceptance criteria:**
- Invalid UTF-8 in JSON payload results in an explicit error path (not a silently altered string).

---

### M-773 (P2): Backpressure disconnect uses lifetime cumulative lag; threshold becomes “eventually disconnect everyone”
**Category:** Telemetry/Backpressure Semantics
**Severity:** P2 (operational correctness + surprising behavior)

**Problem:** The server’s slow-client disconnect feature uses a monotonic `cumulative_lag` counter. Any long-lived client with intermittent lag will eventually cross the threshold, even if it is otherwise keeping up.

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server.rs:5121-5123` (init)
- `crates/dashflow-observability/src/bin/websocket_server.rs:5748-5779` (increment + disconnect)

**Fix direction:**
- Replace lifetime cumulative lag with a **windowed** metric:
  - Track `lag_in_window` + `window_start`, reset after `SLOW_CLIENT_LAG_WINDOW_SECS`, or
  - Implement a leaky bucket (budget replenishes over time).
- Rename env var / messaging so semantics are explicit (e.g., `SLOW_CLIENT_DROPPED_MESSAGES_THRESHOLD_PER_WINDOW`).

**Acceptance criteria:**
- A client that occasionally drops a few messages but recovers is not guaranteed to be disconnected over time solely due to longevity.

---

## Worker Priority (recommended order)

1. **M-763 (P1)** localStorage validation + BigInt safety (prevents hard reconnect failures)
2. **M-770 (P1)** snapshot parse failure → mark needsResync/corrupted (prevents silent stale state)
3. **M-771 (P1)** checkpoint validity tracking (prevents false “diff chain valid”)
4. **M-764/M-765 (P2)** make resume protocol explicit and implement documented behavior
5. **M-768/M-769 (P2/P3)** harden cursor_reset correctness/perf (prefer key-prefix epoch)
6. **M-773 (P2)** window/decay backpressure semantics
7. **M-762/M-766 (P2)** only if you intend to keep thread-mode resume/replay supported
8. **M-767/M-772 (P2/P3)** operability + hardening improvements
