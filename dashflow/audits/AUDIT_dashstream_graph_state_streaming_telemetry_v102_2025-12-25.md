# DashFlow v102 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1788
**Scope:** DashStream UI telemetry + graph state pipeline (`observability-ui/`), websocket-server forward/replay/resume (`crates/dashflow-observability/.../websocket_server/`), replay buffer persistence/metrics (`replay_buffer.rs`)
**Prior:** v101 (M-993..M-1002) — fixed in #1784/#1785/#1786/#1787 (re-verified)

This pass is intentionally skeptical: it assumes “FIXED” claims may hide partial fixes, stale docs, or new regressions, and it looks for correctness gaps in **measurement**, **resume/replay protocol**, and **operator-visible telemetry**.

---

## Executive Summary

Found **10 NEW issues** (M-1003..M-1012). Highest risk:

- **M-1003 (P1)**: websocket-server “old data” classification is technically offset-based but (as implemented) never classifies the initial catch-up as old → expected “historic schema break” decode errors are treated as **new-data** failures, distorting alerting and DLQ volume.
- **M-1007 (P1)**: UI decode/apply pipeline is serialized but **unbounded**; if decode/apply falls behind, the promise chain can grow without a hard cap, causing memory growth and long-tail UI stalls without a deterministic self-protection mechanism.
- **M-1008 (P2)**: websocket-server resume parsing lacks basic validation (partition bounds, map size, allowed `from` values), creating correctness and DoS footguns.

---

## Re-Verification of v101 Claims (spot checks)

- ✅ **M-998** off-main-thread decode exists (Web Worker + pool): `observability-ui/src/workers/decode.worker.ts`, `observability-ui/src/workers/DecodeWorkerPool.ts`, used by `observability-ui/src/App.tsx`.
- ✅ **M-999** apply-lag exported to health panel (state + cards): `observability-ui/src/App.tsx`.
- ✅ **M-995/M-996** server now stores broadcast/replay payloads as `Bytes` (O(1) clone); last-mile `Vec<u8>` conversion remains due to axum 0.7.

---

## New Issues (M-1003 to M-1012)

### M-1003 (P1): “Old data” decode classification is offset-based but doesn’t suppress initial catch-up
**Category:** Metrics/Correctness + Ops Noise
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- The logic defines “old data” as `offset < first_seen_offset` per partition (rewind-only), which will not trigger during the common “first start reads from earliest” catch-up:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2193` (comment + intent)
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2236` (first-seen tracking)
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2255` (old-by-offset condition)

**Impact**
- On first deploy (or when offsets reset and the consumer replays history), decode failures from historic schema changes will be treated as **new-data** failures:
  - `decode_errors` and `websocket_kafka_messages_total{status="error"}` inflate,
  - DLQ volume spikes (best-effort but still real load),
  - alerts fire for “current” failures that are actually old history.

**Fix direction**
- Make offset-based classification actually represent “caught up to head at session start”, not “rewind”:
  1. At partition assignment (or first message per partition), fetch the partition high-watermark (head offset) and store it as `session_head_offset[partition]`.
  2. While `offset < session_head_offset` treat decode failures as “old data” (optionally with a bounded grace window), and once `offset >= session_head_offset` classify as new.
  3. Export a gauge/counter for `catchup_phase{partition}` to make this visible.

**Acceptance**
- With `auto.offset.reset=earliest` and a topic containing historic schema breaks, old decode failures increment `old_data_decode_errors` until catch-up completes, then new decode failures increment `kafka_messages_error` only after reaching the session-start head.

---

### M-1004 (P2): Apply-lag health card severity colors use inverted threshold ordering
**Category:** UI/Telemetry Correctness (operator signal)
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- The ternary checks are ordered such that the “red” condition is unreachable whenever the “yellow” threshold is lower:
  - `observability-ui/src/App.tsx:2084` (`avgLatencyMs > 1000 ? yellow : avgLatencyMs > 5000 ? red : green`)
  - `observability-ui/src/App.tsx:2090` (`pendingCount > 100 ? yellow : pendingCount > 500 ? red : green`)

**Impact**
- The UI under-reports severity (red never shown), reducing operator trust in telemetry and making it harder to notice “UI is falling behind” incidents quickly.

**Fix direction**
- Reverse the ordering: check the most severe threshold first (e.g., `> 5000` before `> 1000`, `> 500` before `> 100`).

**Acceptance**
- When `avgLatencyMs=6000` the card is red; when `avgLatencyMs=1500` the card is yellow.

---

### M-1005 (P2): Schema mismatch state isn’t reset on reconnect (can remain wedged after rollback)
**Category:** UI/Correctness + Recovery
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- WebSocket reconnect resets protocol/timeout refs, but does not reset `schemaVersionMismatchActiveRef` or `schemaVersionMismatchWarnedRef`:
  - Reset block: `observability-ui/src/App.tsx:963`
  - Schema mismatch refs exist and are sticky: `observability-ui/src/App.tsx:223`, `observability-ui/src/App.tsx:222`

**Impact**
- If a mismatch was triggered by a transient deployment (e.g., server rolled back to compatible schema), a reconnect may still stay stuck in “mismatch active” mode (skipping applies and gating cursor commits) until a full page refresh.

**Fix direction**
- On connect (or after receiving a server “schema_ok” control message), reset mismatch refs and clear the banner state.
- Add an explicit “Reset mismatch state” button to unblock operators during incident response.

**Acceptance**
- After a mismatch-triggering deploy is rolled back, a reconnect resumes normal apply/commit without requiring a page refresh.

---

### M-1006 (P2): Worker decode duplicates schema constants/logic, risking drift vs main-thread decoder
**Category:** UI/Protocol Maintainability → Correctness risk
**Where:** `observability-ui/src/workers/decode.worker.ts`, `observability-ui/src/proto/dashstream.ts`

**Evidence**
- Both files define an `EXPECTED_SCHEMA_VERSION` constant independently:
  - `observability-ui/src/workers/decode.worker.ts:13`
  - `observability-ui/src/proto/dashstream.ts:16`
- The decode logic is effectively forked (header detection, decompression, schema mismatch extraction).

**Impact**
- The worker decode path is the live path. If someone updates `dashstream.ts` but forgets to update the worker, the UI will silently diverge (e.g., mismatch detection wrong, decode compatibility behavior differs).

**Fix direction**
- Move shared constants and decoding logic into a shared, dependency-minimal module (e.g. `observability-ui/src/proto/dashstreamDecode.ts`) that is imported by both `dashstream.ts` and `decode.worker.ts`.
- Keep the worker surface to: receive bytes → call shared decode → post result.

**Acceptance**
- There is exactly one definition of `EXPECTED_SCHEMA_VERSION` and one authoritative decode implementation used by both worker and non-worker paths.

---

### M-1007 (P1): UI decode/apply queue is unbounded (no client-side backpressure / self-protection)
**Category:** UI/Performance + Correctness (liveness)
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- Every binary message appends onto a serialized promise chain and increments `pendingCount`, but there is no hard cap that forces a reconnect/reset when the backlog grows:
  - `observability-ui/src/App.tsx:1377` (pendingCount++)
  - `observability-ui/src/App.tsx:1379` (promise chain append)

**Impact**
- If decode/apply throughput < inbound stream throughput, backlog grows without bound:
  - memory grows (queued closures/buffers),
  - UI becomes increasingly stale,
  - operators see “connected” but the timeline/graph state may be minutes behind.

**Fix direction**
- Add a hard cap such as:
  - `MAX_PENDING_BINARY_MESSAGES` (e.g. 500), and/or
  - `MAX_APPLY_LAG_MS` (e.g. 30s between receipt and apply).
- When exceeded: set a “falling behind” banner, force reconnect, and mark active runs needing resync.

**Acceptance**
- Under an artificial decode slowdown, the UI disconnects/reconnects once backlog exceeds the cap and does not grow unbounded.

---

### M-1008 (P2): websocket-server resume input parsing lacks bounds/validation (DoS + correctness footguns)
**Category:** Server/Protocol Robustness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs`

**Evidence**
- Resume parses `lastOffsetsByPartition` into a `HashMap` with no cap on entries and no partition bounds checks:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:681`
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:688` (unbounded loop)
- Resume strategy `from` accepts any string (no allowlist):
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:730`

**Impact**
- Malformed/hostile clients can force large JSON parsing work and large maps per resume call.
- Incorrect `from` values can lead to confusing `replay_complete.mode` values and inconsistent operator debugging.

**Fix direction**
- Enforce strict validation:
  - cap max partitions (e.g. 512) and reject/close on exceed,
  - require `partition >= 0`,
  - require allowed `from ∈ {"cursor","earliest","latest"}`; default to `cursor` on invalid,
  - return a structured error frame on invalid input (don’t silently ignore).

**Acceptance**
- A resume with >N partitions is rejected with a clear error; `from` values are validated and normalized.

---

### M-1009 (P2): websocket-server sends “cursor” then “binary” with no per-send timeout (can hang tasks)
**Category:** Server/Liveness + Resource Safety
**Where:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs`

**Evidence**
- Broadcast path does `await socket.send(Text)` then `await socket.send(Binary)` without any timeout wrapper:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:543`
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:551`

**Impact**
- A wedged TCP connection or pathological client can stall a task inside `.await` for long periods, delaying detection of lag and consuming server resources.

**Fix direction**
- Add a bounded `tokio::time::timeout` around each send (or around the paired sends) and disconnect clients that exceed the bound.
- Consider sending a single framed binary message containing both cursor metadata + payload (eliminates sideband ordering and halves sends).

**Acceptance**
- A client that stops reading is disconnected within the configured send timeout; the server does not accumulate stuck send tasks.

---

### M-1010 (P2): ReplayBuffer uses raw `thread_id` in Redis keys (keyspace injection / cardinality hazards)
**Category:** Server/Persistence Robustness + Security hygiene
**Where:** `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs`

**Evidence**
- Redis keys embed `tid` directly:
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:359`
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:360`

**Impact**
- If `thread_id` contains `:` or very long strings, this inflates key length and can cause:
  - storage blowups (high cardinality),
  - awkward operator/debug tooling (keys hard to reason about),
  - potential key-pattern surprises if any code ever uses pattern matches by `thread_id`.

**Fix direction**
- Encode `thread_id` into a URL-safe key component (e.g. base64url) or use a stable hash (e.g. SHA-256 hex) and store the original thread_id as a value/field when needed.
- Enforce a maximum `thread_id` length for indexing; drop/metric if exceeded.

**Acceptance**
- Redis keys never include raw `thread_id`; long/malformed thread_ids cannot explode key size.

---

### M-1011 (P2): ReplayBuffer has precision-risk path for per-thread sequences (`f64` score) with no warning/guard
**Category:** Server/Correctness (edge)
**Where:** `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs`

**Evidence**
- Per-thread replay uses sorted-set score math (`last_sequence as f64 + 1.0`) with no analogous warning/guard to the offset path:
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:943`
  - Thread keys are scored by `seq` in `thread_entries`: `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:355`

**Impact**
- If sequence values ever approach >2^53 (unlikely but not impossible long-term), the replay ordering and range queries can become incorrect without any diagnostic.

**Fix direction**
- Mirror the offset path behavior:
  - warn (or hard-fail) when `seq > MAX_SAFE_REDIS_SCORE`,
  - document the limitation explicitly in operator docs,
  - optionally implement lexicographic ZSET member ordering for correctness (longer-term).

**Acceptance**
- When sequences exceed the safe threshold, the system produces a clear warning and does not silently mis-order replay.

---

### M-1012 (P2): Resume protocol allows silent “partial parse” of client offsets (ignores invalid keys/values)
**Category:** Server/Protocol Correctness + Operator Debuggability
**Where:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs`

**Evidence**
- Invalid partitions or offsets are silently skipped (`continue`) rather than rejected:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:689`
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:696`

**Impact**
- A UI bug or misconfigured client can send malformed offsets and still receive “successful” resume behavior, but replay correctness is degraded in ways that are hard to diagnose.

**Fix direction**
- Track parse failures explicitly:
  - return a `resume_error` control frame with counts of invalid partitions/offsets,
  - increment a Prometheus counter for “invalid resume cursor payloads”,
  - optionally refuse resume when any invalid fields are present (strict mode).

**Acceptance**
- Malformed resume payloads produce an explicit error signal in both logs and metrics; “silent degrade” is eliminated.

---

## Worker Priority (recommended order)

1. **M-1003 (P1)**: Fix old-data catch-up classification semantics (metrics + DLQ sanity).
2. **M-1007 (P1)**: Add client-side backlog cap + reconnect/resync behavior.
3. **M-1008/M-1012 (P2)**: Harden resume parsing (bounds + explicit error telemetry).
4. **M-1009 (P2)**: Add per-send timeouts and consider cursor+payload framing improvements.
5. **M-1004/M-1005/M-1006 (P2)**: UI telemetry correctness + mismatch recovery + dedupe shared decode logic.
6. **M-1010/M-1011 (P2)**: Redis key hygiene + sequence precision diagnostics (long-horizon correctness).
