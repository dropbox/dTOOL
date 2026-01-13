# DashFlow v108 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1825
**Scope:** DashStream UI graph-state reconstruction (`observability-ui/`), websocket-server streaming metrics + health (`crates/dashflow-observability/.../websocket_server/`), producer-side DashStreamCallback telemetry (`crates/dashflow/src/dashstream_callback/`).
**Prior:** v106/v107 audits — ✅ fixed via #1812, #1815-#1818.

This audit looks for **new** correctness/config/telemetry gaps after the v106/v107 fix wave, with emphasis on “operator truth” and avoiding silent metric distortion.

---

## Status (v108)

**0 P0 | 1 P1 | 4 P2 | 5 P3 | 1 P4** open (new)

| ID | Priority | Category | Summary |
|----|----------|----------|---------|
| **M-1087** | **P1** | UI/Resource Safety | `extractSchema()` parses unbounded JSON from raw attributes (bypasses attribute caps); can freeze/OOM UI |
| **M-1085** | P2 | Server/Metrics Correctness | Windowed “messages received” is not recorded on `payload_too_large` and `payload_missing` paths; decode error rate and staleness can be wrong |
| **M-1088** | P2 | UI/Resource Safety | `boundAttributes()` uses `JSON.stringify` on objects to estimate size; can allocate huge strings (DoS) |
| **M-1089** | P2 | Producer/Config + Reliability | Producer emits unbounded `graph_manifest` / `graph_schema_json`; can exceed payload limits and cause decode failures/telemetry drops |
| **M-1091** | P2 | UI/Correctness + Safety | NodeError stores `attributes['error'] as string` from raw attributes (unbounded + wrapper-unsafe); can leak/overflow UI |
| **M-1086** | P3 | UI/Correctness | Run eviction + run sorting use producer timestamp (`startTime`); clock skew / timestamp=0 can evict/sort incorrectly |
| **M-1090** | P3 | UI/Correctness | Live cursor can move backwards on out-of-order events (cursor set to seq unconditionally) |
| **M-1092** | P3 | UI/Correctness | GraphError does not clear `currentNode`; UI can show stale “active” node after graph failure |
| **M-1093** | P3 | Server/Operability | Old-data decode errors are printed per message (`println!`), causing log spam during catch-up |
| **M-1094** | P3 | Server/Health Design | Degraded threshold can trip on tiny sample sizes and denominator still includes catch-up traffic; needs “new-data-only” window and/or min-sample gating |
| **M-1095** | P4 | Server/Operator Truth | `/health` exposes lifetime send failures only; add windowed send failure/timeouts to detect “currently stuck” clients quickly |

---

## New Issues (M-1085 to M-1095)

### M-1085 (P2): Windowed “messages received” not recorded on `payload_too_large` + `payload_missing`
**Where**
- Oversized payload path records a decode error but does **not** record a received message:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2705-2835` (early `continue`)
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2721-2727` (`record_decode_error()` called)
- Normal message path records message received:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2843-2850` (`record_message_received()` called)
- Payload-missing path also does not record a received message:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2814-2830` (warn + offset store, but no `record_message_received`)
- Windowed API intent:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:413-422` (`record_message_received` doc: “success or error”)

**Why it matters**
- `decode_errors_last_120s / messages_last_120s` can exceed 100% during bursts of oversized new-data payloads.
- `/health` “staleness” can be wrong: `note_kafka_message()` isn’t updated for payload-missing, so the server can be actively consuming while reporting stale.
- Operator-facing metrics become inconsistent exactly during failure modes.

**Fix direction**
- On any message that advances offsets (including `payload_missing` and oversized payload skip), call:
  - `record_message_received()`, and
  - update “last message” timestamps consistently (or add a separate `note_kafka_poll()` to represent “consumer is alive”).
- Decide and document whether old-data catch-up should be included in denominators; if excluded, track separate windows (see M-1094).

**Acceptance**
- Windowed decode error rate never exceeds 100% due to missing denominator updates.
- `/health` staleness reflects actual Kafka consumption activity even for payload-missing/oversize paths.

---

### M-1086 (P3): Run eviction + ordering use producer timestamps (clock skew / timestamp=0 hazards)
**Where**
- Run creation uses `timestamp` (producer timestamp) as `startTime`:
  - `observability-ui/src/hooks/useRunStateStore.ts:521-552`
- Run eviction sorts by `startTime`:
  - `observability-ui/src/hooks/useRunStateStore.ts:555-561`
- Run list sorting uses `startTime`:
  - `observability-ui/src/hooks/useRunStateStore.ts:2034-2036`

**Why it matters**
- Producer timestamps can be skewed (future, past, or `0`), which can:
  - evict the wrong run when `maxRuns` is exceeded,
  - show the wrong “most recent run” in the UI.

**Fix direction**
- Store a separate local `createdAtLocalMs` (e.g., `Date.now()` or `performance.now()` + boot offset) and use that for eviction/sorting.
- Keep producer timestamp for display only.

**Acceptance**
- Run eviction and “recency” ordering are stable under producer clock skew.

---

### M-1087 (P1): `extractSchema()` parses unbounded JSON from raw attributes (bypasses caps)
**Where**
- `extractSchema()` is called on **raw** (unbounded) attributes:
  - `observability-ui/src/hooks/useRunStateStore.ts:788-790`
- JSON parsing is unbounded:
  - `observability-ui/src/utils/attributes.ts:17-35` (`JSON.parse(jsonStr)` with no size check)
- Producer emits large schema/manifest JSON strings:
  - `crates/dashflow/src/dashstream_callback/mod.rs:1408-1431` (`graph_manifest`, `graph_schema_json`)

**Why it matters**
- A single large schema/manifest string can freeze/OOM the tab *before* `boundAttributes()` is applied.
- This is an untrusted-input parsing path (Kafka → WebSocket → UI) and should have explicit size guards.

**Fix direction**
- Parse schema only from a bounded/truncated value:
  - call `extractSchema(storedEvent.attributes)` (bounded) rather than `attributes` (raw), or
  - implement `getJsonAttributeBounded(maxBytes)` that rejects/parses-with-cap.
- Add telemetry (console + optional UI counter) when schema parsing is skipped due to size.

**Acceptance**
- Schema/manifest JSON parsing cannot allocate unbounded memory; oversized schema attributes are safely ignored with visibility.

---

### M-1088 (P2): `boundAttributes()` uses `JSON.stringify` and can allocate huge strings
**Where**
- Object sizing uses `JSON.stringify(value)`:
  - `observability-ui/src/utils/attributes.ts:94-112`

**Why it matters**
- `JSON.stringify` must materialize the full string; a huge object can still allocate/GC-thrash and freeze the UI.
- This defeats the purpose of “bounded attribute storage”.

**Fix direction**
- Replace `JSON.stringify`-based sizing with a bounded traversal estimator:
  - cap depth, cap total visited keys, cap string length, cap array lengths,
  - stop early once the byte budget is exceeded.
- For unknown objects, store `{_truncated: true, _note: "size_estimate_exceeded"}` without full serialization.

**Acceptance**
- Bounding attributes never requires full serialization of attacker-controlled objects.

---

### M-1089 (P2): Producer emits unbounded schema/manifest telemetry fields
**Where**
- Producer inserts `graph_manifest` + `graph_schema_json` as full JSON strings:
  - `crates/dashflow/src/dashstream_callback/mod.rs:1408-1431`

**Why it matters**
- Large graphs/manifests can produce very large telemetry messages.
- This can cascade into:
  - `payload_too_large` rejects on the websocket-server (and DLQ noise),
  - silent drops/backpressure on the producer side.

**Fix direction**
- Add producer-side caps for all event attribute strings:
  - truncate with `{sha256, preview, original_len}` for oversized values,
  - track a counter like `dashstream_telemetry_attribute_truncated_total{key=...}` (or coarse label to avoid cardinality).
- Prefer emitting a content-addressed schema ID (already exists) and a compact schema summary rather than full JSON by default.

**Acceptance**
- Telemetry payload size is bounded at the producer; oversize schema/manifest payloads cannot cause downstream decode failures.

---

### M-1090 (P3): Live cursor can move backwards on out-of-order events
**Where**
- Live cursor updates unconditionally to the incoming event seq:
  - `observability-ui/src/hooks/useRunStateStore.ts:963-967`

**Why it matters**
- When an out-of-order event is inserted (the store supports this), live mode can “jump backwards” and show an older state, breaking operator trust.

**Fix direction**
- In live mode, only advance the cursor if `seq > currentCursor.seq` (BigInt compare) for the same thread.
- Alternatively, always set cursor to the latest event in the store after insertion.

**Acceptance**
- Live cursor is monotonic and never regresses due to late/out-of-order events.

---

### M-1091 (P2): NodeError stores raw `attributes['error'] as string` (unbounded + wrapper-unsafe)
**Where**
- NodeError error string assignment:
  - `observability-ui/src/hooks/useRunStateStore.ts:950-958` (esp. `:956`)

**Why it matters**
- `attributes['error']` may be:
  - a protobuf wrapper object (`{stringValue: ...}`),
  - a huge string (stack traces, tool outputs),
  - sensitive data (tokens/keys embedded in errors).
- Assigning it directly bypasses `boundAttributes()` and can render/retain large or sensitive strings.

**Fix direction**
- Use `getStringAttribute(attributes, 'error')` and apply truncation/redaction consistent with other attribute handling.
- Consider storing `error_preview` and `error_len` instead of the full string.

**Acceptance**
- NodeError error strings cannot exceed a bounded size and respect wrapper handling; sensitive content is not retained by default.

---

### M-1092 (P3): GraphError does not clear `currentNode` (stale active node after failure)
**Where**
- GraphError sets status/endTime but does not clear `currentNode`:
  - `observability-ui/src/hooks/useRunStateStore.ts:841-845`

**Why it matters**
- After a graph fails, UI can continue showing a node as “active” indefinitely.

**Fix direction**
- On GraphError, clear `currentNode` and consider marking the active node state as `error` if known.

**Acceptance**
- GraphError transitions always leave the UI in a consistent “no active node” state.

---

### M-1093 (P3): Old-data decode errors use `println!` per message (log spam)
**Where**
- Old-data decode errors print per message:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2591-2594`

**Why it matters**
- During catch-up, this can flood logs and hide actionable events.

**Fix direction**
- Replace `println!` with `tracing::debug!` and rate-limit (e.g., “log every N errors” or “once per partition per window”).
- Keep metrics as the primary signal (`old_data_decode_errors` already exists).

**Acceptance**
- Catch-up phases do not spam logs; operators still have visibility via metrics and occasional summary logs.

---

### M-1094 (P3): Degraded threshold needs min-sample gating and “new-data-only” denominator
**Where**
- `/health` degraded status uses `decode_errors_last_120s / messages_last_120s` with a fixed threshold:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:125-150`
- Current “messages_last_120s” includes catch-up traffic (and misses some early-continue paths; see M-1085):
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2848-2850`
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:413-422`

**Why it matters**
- 1 decode error / 1 message in 2 minutes marks the server “degraded” even if the sample is too small to be meaningful.
- Including catch-up messages in the denominator can dilute true new-data corruption during startup.

**Fix direction**
- Track `new_data_messages_last_120s` and compute:
  - `decode_errors_last_120s / new_data_messages_last_120s` for health gating.
- Add min-sample gating:
  - only apply “>1%” threshold when `new_data_messages_last_120s >= N` (e.g., 100),
  - otherwise surface as a warning with counts.

**Acceptance**
- Health status reflects meaningful rates and does not flap on tiny sample sizes; new-data corruption is not diluted by catch-up.

---

### M-1095 (P4): `/health` lacks windowed send failure/timeouts (current stuckness invisible)
**Where**
- Snapshot includes lifetime `send_failed` / `send_timeout`:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:500-503`
- No windowed send-failure fields analogous to `dropped_messages_last_120s`:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs` (no send window)

**Why it matters**
- A server can be “healthy” by decode metrics but currently unable to deliver to clients (send timeouts spiking).
- Operators have to scrape `/metrics` and compute rates instead of using `/health`.

**Fix direction**
- Add `send_failed_last_120s` and `send_timeout_last_120s` windows (same sliding window helper).
- Add `/health` alerts when these exceed thresholds.

**Acceptance**
- `/health` surfaces “currently stuck clients” via recency-window send failure metrics.

---

## Worker Priority (v108)

1. **M-1087 (P1)**: Bound schema parsing in UI (untrusted JSON.parse).
2. **M-1085 (P2)**: Fix server window denominator and staleness updates for oversize/payload-missing paths.
3. **M-1088 (P2)**: Fix `boundAttributes()` to avoid JSON.stringify allocation bombs.
4. **M-1089 + M-1091 (P2)**: Producer + UI error field size caps/redaction (prevent huge/secret payloads).
5. **M-1086 + M-1090 + M-1092 (P3)**: UI correctness polish (recency ordering, cursor monotonicity, graph-error state).
6. **M-1093 + M-1094 + M-1095 (P3/P4)**: Operability polish (log spam, health gating, windowed send failures).
