# DashFlow v104 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1794
**Scope:** DashStream UI streaming/telemetry (`observability-ui/`), websocket-server forward/replay/resume (`crates/dashflow-observability/.../websocket_server/`), replay buffer persistence/metrics (`replay_buffer.rs`)
**Prior:** v103 (M-1013..M-1022) — still OPEN; this pass looks for *additional* correctness/telemetry/config gaps.

This audit is intentionally skeptical: it assumes "mostly correct" systems fail at edges (timeouts, partial failures, config drift, multi-client semantics).

---

## Status (v104)

**0 P0 | 0 P1 | 0 P2** open — **ALL ISSUES FIXED**

| ID | Priority | Status | Summary |
|----|----------|--------|---------|
| ~~**M-1031**~~ | ~~P1~~ | ✅ FIXED #1795 | UI epoch re-check at apply/commit boundaries |
| ~~**M-1024**~~ | ~~P1~~ | ✅ FIXED #1795 | Payload size check before allocation |
| ~~**M-1025**~~ | ~~P1~~ | ✅ FIXED #1795 | Payload-missing metric + explicit skip policy |
| ~~**M-1023**~~ | ~~P2~~ | ✅ FIXED #1796 | DecodeWorkerPool timeout classification |
| ~~**M-1026**~~ | ~~P2~~ | ✅ FIXED #1796 | dropped_messages per-client semantics |
| ~~**M-1027**~~ | ~~P2~~ | ✅ FIXED #1796 | Backpressure multi-client amplification |
| ~~**M-1028**~~ | ~~P2~~ | ✅ FIXED #1797 | Startup watermark fetch bounded (15s budget + 2s/partition) |
| ~~**M-1029**~~ | ~~P2~~ | ✅ FIXED #1797 | cursor_reset_complete uses send_with_timeout |
| ~~**M-1030**~~ | ~~P2~~ | ✅ FIXED #1797 | Apply-lag UI updates periodically even when totalApplied==0 |
| ~~**M-1032**~~ | ~~P2~~ | ✅ VERIFIED #1797 | sanitize_thread_id_for_redis already handles all unsafe chars |

---

## Executive Summary

Found **10 NEW issues** (M-1023..M-1032). Highest risk (ALL NOW FIXED):

- ~~**M-1024 (P1):**~~ ✅ FIXED #1795 - websocket-server now checks payload size BEFORE `payload.to_vec()` allocation; oversized payloads are rejected early without memory amplification.
- ~~**M-1031 (P1):**~~ ✅ FIXED #1795 - UI now re-checks `wsEpoch` immediately before state mutation and cursor commit; stale-epoch messages are discarded with debug log.
- ~~**M-1025 (P1):**~~ ✅ FIXED #1795 - Payload-missing messages are now tracked via `websocket_kafka_payload_missing_total` metric; explicit skip policy documented in comments.

Note: v103 P1 fixes (#1793) now force reconnect on **any** decode/apply error, but several issues below still affect **classification**, **telemetry truth**, and **cross-epoch correctness**.

---

## New Issues (M-1023 to M-1032)

### ✅ M-1023 (P2): DecodeWorkerPool timeouts don't look like `TimeoutError` — FIXED #1796
**Category:** UI/Correctness + Recovery
**Where:** `observability-ui/src/workers/DecodeWorkerPool.ts`, `observability-ui/src/App.tsx`

**Evidence**
- Worker-pool timeout rejects with a plain `Error` (name `"Error"`):
  - `observability-ui/src/workers/DecodeWorkerPool.ts:78` (`pending.reject(new Error('Decode operation timed out'))`)
- UI considers a timeout only when `err.name === 'TimeoutError'`:
  - `observability-ui/src/App.tsx:200` (`isTimeoutError`)
  - `observability-ui/src/App.tsx:1660` (timeout triggers reconnect)

**Fix summary:**
- Modified `handleTimeout()` in `DecodeWorkerPool.ts` to set `err.name = 'TimeoutError'`
- Timeout errors now correctly trigger the timeout-specific reconnect path
- Consistent with the `makeTimeoutError()` helper pattern used elsewhere in App.tsx

**Files:**
- `observability-ui/src/workers/DecodeWorkerPool.ts` (MODIFIED: line 83-86)

**Verification:** `npx tsc --noEmit` passes

---

### M-1024 (P1): websocket-server clones payload bytes before size validation (memory/DoS risk)
**Category:** Server/Correctness + Resource Safety
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Payload is cloned into `Bytes` **before** `decode_message_compatible(...)` enforces `max_payload_bytes`:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2418` (`Bytes::from(payload.to_vec())`)
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1155` (`WEBSOCKET_MAX_PAYLOAD_BYTES`)
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2430` (decode/limit enforcement happens later)

**Impact**
- Oversized payloads can allocate large `Vec<u8>` even though they will be rejected.
- Downstream paths can further amplify cost (e.g., base64 encoding on decode error paths) even when the message is invalid or malicious.

**Fix direction**
- Check raw `payload.len()` against `max_payload_bytes` **before cloning**:
  - If too large, classify as payload-too-large and skip cloning/base64.
- Consider a “DLQ payload size cap”: never base64 encode payloads beyond a safe maximum.

**Acceptance**
- A payload larger than `WEBSOCKET_MAX_PAYLOAD_BYTES` cannot cause `payload.to_vec()` allocation in the hot path.

---

### M-1025 (P1): Offsets are advanced even when Kafka message has no payload (silent drop; no metrics)
**Category:** Server/Correctness + Telemetry Truth
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Message processing is gated by `if let Some(payload) = msg.payload() { ... }`:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2334`
- Offset is stored unconditionally after the branch (even if payload is `None`):
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2819`

**Impact**
- Messages without payload are silently skipped, but offsets still advance → “at-least-once” semantics are violated for the stream.
- There is no explicit counter for “payload missing” events, so operators can’t detect this as data loss.

**Fix direction**
- If `payload.is_none()`:
  - increment a counter (`websocket_kafka_payload_missing_total`),
  - optionally treat as a “message error” (don’t store offset unless policy says skip),
  - or publish a DLQ record with metadata (no payload) for forensic analysis.

**Acceptance**
- Payload-missing messages are visible via metrics and do not silently advance offsets without an explicit configured policy.

---

### ✅ M-1026 (P2): `dropped_messages` is per-client — FIXED #1796
**Category:** Metrics/Correctness (operator interpretation)
**Where:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs`, `state.rs`

**Fix summary:**
- Updated `websocket_dropped_messages_total` help text to clarify per-client semantics
- Help text now reads: "Total messages dropped across all clients (N clients × M drops = N×M counted)"
- Documented `dropped_messages` struct field in state.rs with per-client counting explanation
- Directs operators to use `websocket_client_lag_events_total` for event-based alerting

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server/state.rs` (MODIFIED: help text, comments)

**Verification:** `cargo check -p dashflow-observability --features="websocket-server"` passes

---

### ✅ M-1027 (P2): Server backpressure multi-client amplification — FIXED #1796
**Category:** Server/Liveness + Config Correctness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Fix summary:**
- Added new `lag_events` atomic counter to `ServerMetrics` struct
- Each lag event increments by 1, regardless of how many messages were dropped
- Changed backpressure logic to use `lag_events` instead of `dropped_messages`
- Threshold set to 10 lag events/sec (stable regardless of client count)
- Updated backpressure log message to reflect lag events count

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server/state.rs` (MODIFIED: added `lag_events` counter)
- `crates/dashflow-observability/src/bin/websocket_server/handlers.rs` (MODIFIED: increment `lag_events`)
- `crates/dashflow-observability/src/bin/websocket_server/main.rs` (MODIFIED: backpressure uses `lag_events`)

**Verification:** `cargo check -p dashflow-observability --features="websocket-server"` passes

---

### M-1028 (P2): Startup watermark fetch can block server start O(partitions × timeout)
**Category:** Server/Config + Availability
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- At startup, code fetches metadata then fetches watermarks sequentially per partition with a per-partition timeout:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1059`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1092`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1098` (5s each)

**Impact**
- Large partition counts + transient Kafka issues can delay server readiness dramatically.
- This can create cascading failures (no websocket-server available) that look like “infra outage” but are really a startup-time serial fetch.

**Fix direction**
- Add an overall time budget (e.g., 10s total) for the startup watermark phase; stop early and fall back.
- Parallelize watermark fetches with bounded concurrency.
- Export a metric for “startup watermark fetch duration” and “partitions missing session_head_offset”.

**Acceptance**
- Server becomes ready within a predictable bound even when some partitions are slow to fetch watermarks.

---

### M-1029 (P2): `cursor_reset_complete` send is unbounded (no send timeout) and can hang on stuck clients
**Category:** Server/Liveness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs`

**Evidence**
- cursor_reset_complete uses `await socket.send(...)` directly:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:527`

**Impact**
- A client that stops reading can stall the handler task during an operational recovery action (cursor reset), making recovery appear unreliable.

**Fix direction**
- Apply the same `send_with_timeout` helper to cursor_reset_complete (and any other control sends).

**Acceptance**
- cursor_reset completes or fails fast with a bounded timeout, and timeouts are visible in logs/metrics.

---

### M-1030 (P2): Apply-lag UI telemetry can stay “—” under wedge conditions (no applied messages)
**Category:** UI/Telemetry Truthfulness
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- UI only updates apply-lag state every 10s **and only if `totalApplied > 0`**:
  - `observability-ui/src/App.tsx:1641`

**Impact**
- If the decode/apply pipeline is failing early (or wedged), the health panel shows no apply-lag info even as queue depth grows (operators get “no data” instead of “bad”).

**Fix direction**
- Always update apply-lag UI state periodically, even when `totalApplied == 0`:
  - show `pendingCount` and “no applies yet” explicitly,
  - add a “stalled apply pipeline” banner when pendingCount > 0 for >N seconds.

**Acceptance**
- The health panel reflects “stalled” vs “healthy but idle” without requiring devtools.

---

### M-1031 (P1): UI `wsEpoch` is only checked once per queued step; epoch can flip mid-decode and still apply old message
**Category:** UI/Correctness (cross-epoch contamination)
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- The queued decode/apply step checks epoch at the start:
  - `observability-ui/src/App.tsx:1418`
- After that, it performs async work (arrayBuffer/decode) and then applies state and commits cursor without re-checking epoch near the apply/commit boundary:
  - `observability-ui/src/App.tsx:1459` (schema mismatch early return)
  - `observability-ui/src/App.tsx:1618` (apply pipeline continues)
  - `observability-ui/src/App.tsx:1623` (cursor commit)

**Impact**
- If the socket epoch changes while decode is in-flight, a decoded message from the old epoch can be applied into the new epoch’s state, corrupting graph state/timeline and potentially committing a cursor for the wrong connection lifecycle.

**Fix direction**
- Add an epoch check immediately before:
  - `processRunStateMessageRef.current(decoded)`, and
  - `commitKafkaCursor(cursor)`.
- Alternatively, capture `wsEpoch` in the decoded payload and validate it at apply time.

**Acceptance**
- No message from an old websocket epoch can mutate state or advance persisted cursors after a reconnect.

---

### M-1032 (P2): Redis `thread_id` sanitization only handles `:` and length; other unsafe characters remain
**Category:** Server/Security Hygiene + Robustness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs`

**Evidence**
- Comments claim “special characters” are handled, but logic only checks `:` and length:
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:31`
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:48`

**Impact**
- Thread IDs containing whitespace/control characters/unicode may still be embedded in keys, potentially causing:
  - operator tooling surprises,
  - awkward key parsing/SCAN patterns,
  - hard-to-debug retention behavior.

**Fix direction**
- Define a strict allowed character set for key components (e.g. `[A-Za-z0-9._-]`) and:
  - base64url-encode anything else (preferred; reversible), or
  - percent-encode unsafe bytes.
- Enforce a hard max length at ingest and count drops.

**Acceptance**
- Redis keys never contain raw unsafe characters from thread_id, and replay remains deterministic.

---

## Worker Priority (ALL COMPLETE)

1. ~~**M-1031 (P1)**~~: ✅ FIXED #1795 - Add epoch re-check at apply/commit boundaries.
2. ~~**M-1024 (P1)**~~: ✅ FIXED #1795 - Avoid pre-limit payload cloning; add DLQ payload caps.
3. ~~**M-1025 (P1)**~~: ✅ FIXED #1795 - Make payload-missing behavior explicit.
4. ~~**M-1023 (P2)**~~: ✅ FIXED #1796 - Normalize timeout typing/classification.
5. ~~**M-1026/M-1027**~~: ✅ FIXED #1796 - lag_events-based backpressure + clarified docs.
6. ~~**M-1028/M-1029**~~: ✅ FIXED #1797 - Bound startup (15s budget) and control-path sends (5s timeout).
7. ~~**M-1030/M-1032**~~: ✅ FIXED #1797 - UI telemetry truthfulness + key hygiene verified.

---

## M-1028 (P2): Startup watermark fetch bounded — FIXED #1797
**Category:** Server/Availability

**Problem:** Sequential watermark fetch with 5s per-partition timeout could block server start O(partitions × 5s) for large partition counts or slow Kafka.

**Fix summary:**
- Added overall time budget (15s) for all partition watermark fetches
- Reduced per-partition timeout from 5s to 2s to allow more partitions within budget
- Check budget before each partition fetch; skip remaining if exhausted
- Log summary with timing: partitions fetched, partitions skipped, elapsed time
- Skipped partitions fall back to first-seen offset detection (graceful degradation)

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server/main.rs` (MODIFIED: lines 1080-1166)

**Verification:** `cargo check -p dashflow-observability --features="websocket-server"` passes

---

## M-1029 (P2): cursor_reset_complete send timeout — FIXED #1797
**Category:** Server/Liveness

**Problem:** `cursor_reset_complete` used direct `socket.send().await` without timeout, allowing stuck clients to hang the control-path handler indefinitely.

**Fix summary:**
- Replaced direct `socket.send()` with `send_with_timeout()` helper (5s timeout)
- On timeout/failure, handler breaks out of loop (disconnects client)
- Consistent with all other control sends in handlers.rs

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server/handlers.rs` (MODIFIED: lines 527-534)

**Verification:** `cargo check -p dashflow-observability --features="websocket-server"` passes

---

## M-1030 (P2): Apply-lag UI updates even when totalApplied==0 — FIXED #1797
**Category:** UI/Telemetry Truthfulness

**Problem:** UI only updated apply-lag state if `totalApplied > 0`. When the decode/apply pipeline was wedged (no successful applies), the health panel showed "—" even as pending count grew.

**Fix summary:**
- Removed `totalApplied > 0` condition from the periodic update check
- Added safe division to avoid NaN when totalApplied is 0
- Added separate 5-second interval (`useEffect`) that updates applyLagInfo independently of message apply success
- UI now shows pending count even when no messages have been applied yet

**Files:**
- `observability-ui/src/App.tsx` (MODIFIED: lines 1675-1697, 1798-1821)

**Verification:** `npx tsc --noEmit` passes

---

## M-1032 (P2): thread_id sanitization — VERIFIED #1797
**Category:** Redis/Robustness

**Problem (audit claim):** Comments claim "special characters" are handled, but logic only checks `:` and length.

**Verification:** The audit finding was based on the **legacy** function `legacy_hash_thread_id_for_redis()` which intentionally only checks `:` and length for backward-compatible reads.

The **current** `sanitize_thread_id_for_redis()` function (lines 54-58) already correctly handles ALL unsafe characters via a strict character allowlist:
```rust
|| !thread_id
    .chars()
    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
```

Any character not in `[A-Za-z0-9_-]` triggers base64url encoding.

**Fix summary:**
- Added clarifying documentation to `legacy_hash_thread_id_for_redis()` explaining that it intentionally uses the limited check for backward compatibility only
- Confirmed the main `sanitize_thread_id_for_redis()` correctly handles all unsafe chars

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs` (MODIFIED: documentation at lines 71-78)

**Verification:** `cargo check -p dashflow-observability --features="websocket-server"` passes
