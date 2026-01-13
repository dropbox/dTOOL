# DashStream Graph State + Streaming Telemetry Re-Audit (v20) — 2025-12-24

This is an “AGAIN” pass extending:
- `audits/AUDIT_dashstream_graph_state_streaming_telemetry_reaudit_2025-12-23_v19.md`

Scope: DashStream streaming telemetry (DashStreamCallback → Kafka → websocket-server → observability-ui), with emphasis on correctness, bugs, and configuration/operability.

---

## Executive Summary (skeptical)

v19 identified 10 “second-order” issues (M-697..M-706). Two P1 items from v19 were fixed in `#1609`:
- M-698: replay paging completion semantics (per-partition truncation tracking)
- M-704: patch apply failures mark runs corrupted + wait for recovery snapshot

This v20 pass looks for *more of the same class* of correctness failures: numeric precision at boundaries, protocol compatibility edge cases, and silent corruption when telemetry is malformed or partially missing.

This pass found **10 more concrete gaps** (M-707..M-716). The highest-risk class is **UI state correctness**:
- Kafka offset precision is still lossy in the browser (Number),
- the JSON Patch implementation is not RFC6902-correct for arrays and has invalid-index hazards,
- “gap”/“stale cursor” signals aren’t used to quarantine/trigger resync (UI continues showing an invalid state as if it were trustworthy).

---

## New v20 Findings (10 more actionable gaps)

### M-707 (P1): UI still stores Kafka offsets as JS `number` (precision loss > 2^53)
**Problem**
- Server-side JSON now encodes offsets as strings (M-690), but the UI parses them with `parseInt()` and stores them as `number`.
- This means offsets above `2^53-1` are still lossy end-to-end in the browser, including:
  - resume cursor persistence (`localStorage`),
  - monotonicity checks,
  - “offset went backwards” detection.

Evidence:
- UI cursor parsing: `observability-ui/src/App.tsx` (cursor handler uses `parseInt()` and stores `{partition, offset:number}`)
- Offset storage types: `observability-ui/src/App.tsx` (`lastOffsetsByPartitionRef` is `Record<string, number>`)

Fix direction:
- Represent offsets as strings (or `bigint`) throughout the UI:
  - `lastOffsetsByPartitionRef: Record<string, string>`
  - `pendingKafkaCursorRef: {partition:number; offset:string}`
  - comparisons via `BigInt(offset)`
- Persist strings to `localStorage`.
- When sending `resume`, send offset strings (server already supports string decoding for JSON control messages).

Acceptance:
- Round-trip an offset > `9_007_199_254_740_991` through: server cursor → UI store → localStorage → reload → resume, with exact equality.

---

### M-708 (P1): `jsonPatch.ts` is not RFC6902-correct for arrays (silent state corruption risk)
**Problem**
- The UI’s JSON Patch implementation treats array `add`/`replace` as index assignment rather than RFC6902 insertion semantics.
- For `add` on arrays, RFC6902 requires inserting at the specified index (shifting subsequent elements). Index assignment changes meaning and can silently corrupt state.
- This is particularly dangerous because it can appear “fine” until later state hash mismatch (and state hashes may be omitted).

Evidence:
- `setLocation()` uses `current[index] = value` for arrays: `observability-ui/src/utils/jsonPatch.ts`

Fix direction:
- Use a well-tested RFC6902 implementation (preferred) OR implement correct array behavior:
  - `add` on arrays: `splice(index, 0, value)` (and support `-` append semantics for add only)
  - `replace` on arrays: assignment at index (but validate index exists)
  - `move`/`copy` should preserve RFC6902 semantics for arrays too
- Add unit tests covering:
  - `add` into middle of array,
  - `remove` from array,
  - `move` and `copy` involving arrays,
  - invalid index behavior (see M-709).

Acceptance:
- RFC6902 test vectors for array ops pass, and the UI state after applying server-produced patches matches a full snapshot.

---

### M-709 (P1): Array index parsing can produce wrong mutations (`NaN` → remove index 0)
**Problem**
- `removeLocation()` and other helpers use `parseInt()` for array indices with no validation.
- If `parseInt()` returns `NaN`, JS array operations can do surprising things:
  - `splice(NaN, 1)` treats `NaN` like `0` → removes the first element.
- A malformed patch (producer bug, schema mismatch, corrupted bytes) can mutate the wrong data without throwing.

Evidence:
- `removeLocation()` uses `parseInt(key, 10)` then `splice(index, 1)`: `observability-ui/src/utils/jsonPatch.ts`

Fix direction:
- Strictly validate array index segments:
  - Only digits are allowed for array indices in non-`add` contexts.
  - For `add`, allow `-` (append) and digits.
  - Reject anything else by throwing (then the UI will mark patch failure/corruption per M-704).

Acceptance:
- Invalid array paths never mutate state (must throw), and patch apply failure marks run corrupted.

---

### M-710 (P2): JSON Patch path segments permit prototype pollution in the browser
**Problem**
- Patch paths are applied directly to JS objects with `obj[segment] = ...`.
- A patch containing `__proto__`, `constructor`, or `prototype` segments can cause prototype pollution in the browser context.
- Even if producers are “trusted”, this is still a multi-tenant safety boundary risk (and an avoidable footgun).

Evidence:
- Direct dynamic key writes: `observability-ui/src/utils/jsonPatch.ts` (`setLocation()` assigns `obj[segment]`)

Fix direction:
- Reject dangerous path segments in `parseJsonPointer()` (or before mutation) for all operations.
- Prefer a patch library with built-in prototype pollution hardening, if available.

Acceptance:
- A patch containing `/__proto__/polluted=true` does not mutate global object prototypes; UI flags corruption/quarantine.

---

### M-711 (P1): UI does not treat `gap` / `cursor_stale` signals as resync/corruption triggers
**Problem**
- Server sends explicit control messages indicating lossiness:
  - `gap` (missing messages during replay)
  - `cursor_stale` (requested offset older than retained data)
- The UI currently logs/writes a small indicator list and continues to present state as if it were trustworthy.

Evidence:
- Server `gap` message includes `partition` and severity: `crates/dashflow-observability/src/bin/websocket_server.rs`
- UI ignores `partition`/`severity` and does not mark any run `needsResync`: `observability-ui/src/App.tsx`

Fix direction:
- Treat `gap` and `cursor_stale` as correctness events, not just UI decorations:
  - surface a prominent banner,
  - mark affected runs as `needsResync` and/or `corrupted`,
  - optionally pause applying further diffs until a checkpoint/full snapshot arrives,
  - add explicit “request resync” action if supported.

Acceptance:
- On any `gap`/`cursor_stale`, UI immediately marks the state as untrusted and recovers after a checkpoint/full snapshot.

---

### M-712 (P2): Redis replay index keys (`*:offsets`, `*:sequences`) do not expire and accumulate
**Problem**
- Payload keys use `SETEX`, but the ZSET index keys do not have a TTL.
- This can accumulate large numbers of keys over time, especially per-thread sequence keys.
- Expired payload keys are only removed from ZSETs when a replay read happens; otherwise stale index entries persist indefinitely.

Evidence:
- Payload keys use `set_ex`, but no `expire` on `offsets_set_key` / `thread_set_key`: `crates/dashflow-observability/src/bin/websocket_server.rs`

Fix direction:
- Apply TTL to index keys:
  - after `ZADD`, `EXPIRE offsets_set_key <ttl>`
  - after `ZADD`, `EXPIRE thread_set_key <ttl>`
- Consider a periodic maintenance/GC (or an “expire-on-write” policy) to prevent key growth in idle threads.

Acceptance:
- After TTL, both payload keys and index keys disappear without requiring a replay read to clean them up.

---

### M-713 (P2): DashStreamCallback does not count Kafka send failures in drop/degraded metrics
**Problem**
- `dashstream_telemetry_dropped_total` is used for backpressure/queue drops, but Kafka send errors in the message worker are only logged.
- During Kafka outages, dashboards can misleadingly show “no drops” while telemetry is silently not delivered.

Evidence:
- Message worker logs `Failed to send event/state diff/checkpoint telemetry` without incrementing a counter: `crates/dashflow/src/dashstream_callback.rs`

Fix direction:
- Add a producer-side counter for send failures (and/or extend dropped_total):
  - `dashstream_telemetry_send_failures_total{message_type, reason="kafka_send_failed"}`
  - optionally also increment `dashstream_telemetry_dropped_total{..., reason="kafka_send_failed"}`

Acceptance:
- Induce Kafka send failure: counter increments and alerting can catch it.

---

### M-714 (P2): Redis write path is multiple round-trips per message (high load / jitter risk)
**Problem**
- Each message write performs multiple Redis operations (SETEX + ZADD + ZCARD + ZREMRANGEBYRANK, plus thread variants).
- This is expensive at throughput and can increase latency, timeouts, and dropped redis writes.

Evidence:
- `add_message()` runs multiple sequential commands per message: `crates/dashflow-observability/src/bin/websocket_server.rs`

Fix direction:
- Pipeline related commands:
  - Use pipelining (or Lua) to SETEX + ZADD + trim in a single round-trip.
  - Avoid `ZCARD` on every write; trim opportunistically or with a probabilistic cadence.

Acceptance:
- Under load, Redis latency decreases and `replay_buffer_redis_write_dropped_total` stays near 0.

---

### M-715 (P2): UI checkpoint stores are unbounded (memory growth on long sessions)
**Problem**
- The UI stores checkpoints both by sequence and by checkpoint_id with no eviction policy.
- Long-running dashboards will accumulate checkpoints indefinitely, increasing memory usage and slowing time-travel operations.

Evidence:
- `checkpoints: new Map()` and `checkpointsById: new Map()` with no eviction: `observability-ui/src/hooks/useRunStateStore.ts`

Fix direction:
- Add a bounded retention policy:
  - keep last N checkpoints per run (and/or last T minutes),
  - evict on insert,
  - expose config knobs for operators.

Acceptance:
- After a long stream, checkpoint count stays bounded and UI memory usage remains stable.

---

### M-716 (P2): Legacy `lastSequence` field is lossy after BigInt sequences (rolling upgrade hazard)
**Problem**
- The UI still sends legacy `lastSequence` as a JS `number` for older websocket-server compatibility.
- When sequences exceed `2^53-1`, this legacy field becomes lossy, potentially causing incorrect legacy replay during rolling upgrades or mixed-version deployments.

Evidence:
- Legacy conversion uses `Number(...)` on a potentially large BigInt-backed string: `observability-ui/src/App.tsx`

Fix direction:
- If the max sequence exceeds `Number.MAX_SAFE_INTEGER`, omit `lastSequence` entirely (force modern resume paths).
- Alternatively, send legacy fields as strings and make legacy servers ignore unknown fields (or add `lastSequenceStr`).

Acceptance:
- With sequences > `2^53-1`, mixed-version deployments do not regress to incorrect legacy replay.

---

## Worker Priority (v20 additive)

1) **P1 correctness:** M-707, M-708, M-709, M-711.
2) **P2 safety/operability:** M-712, M-713, M-714, M-715, M-716.
3) **P2 security hardening:** M-710.
