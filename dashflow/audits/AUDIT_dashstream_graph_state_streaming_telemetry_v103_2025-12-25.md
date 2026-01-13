# DashFlow v103 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1792
**Scope:** DashStream UI streaming/telemetry (`observability-ui/`), websocket-server forward/replay/resume (`crates/dashflow-observability/.../websocket_server/`), replay buffer persistence/metrics (`replay_buffer.rs`)
**Prior:** v102 (M-1003..M-1012) — claimed fixed in #1789-#1791; this pass assumes regressions/partial fixes are possible.

This audit focuses on **telemetry truth**, **resume/replay correctness**, and **configuration drift** between server ↔ UI.

---

## Executive Summary

Found **10 NEW issues** (M-1013..M-1022). Highest risk:

- ~~**M-1017 (P1)**~~: ✅ FIXED #1793 - UI reconnect resets `applyLagMetricsRef.current` while prior-epoch decode/apply tasks can still run → pending counts can go **negative**, backlog cap becomes unreliable, and operator telemetry becomes untrustworthy.
- ~~**M-1016 (P1)**~~: ✅ FIXED #1793 - Non-timeout decode/worker errors do **not** force reconnect; the UI can continue and commit later cursors, risking **permanent message skips** after reload.
- **M-1013 (P1)**: Redis `thread_id` "sanitization" hashes IDs (with a doc/code mismatch) and breaks backward compatibility with existing Redis keys; collisions or hash algorithm changes can corrupt replay correctness.

---

## New Issues (M-1013 to M-1022)

### ~~M-1013 (P1)~~: Redis thread_id hashing breaks replay compatibility and risks collisions; doc/code mismatch — ✅ FIXED #1794
**Category:** Replay/Correctness + Persistence Migration
**Where:** `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs`

**Evidence**
- Thread IDs are hashed when they contain `:` or exceed 128 chars:
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:21`
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:43`
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:48`
- The comment claims "truncated SHA-256", but the implementation uses `DefaultHasher` (SipHash-ish 64-bit):
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:41`
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:44`

**Impact**
- **Backward compatibility:** Existing Redis keys written before this change used raw `thread_id`. After this change, reads/writes use sanitized IDs, so older persisted history becomes unreachable for threads requiring hashing.
- **Collision risk:** 64-bit hashing is not collision-free; a collision would mix two threads' sequences in one ZSET/keyspace (catastrophic replay correctness).
- **Upgrade risk:** If the hashing algorithm changes across Rust versions, replay becomes non-deterministically broken across deploys.

**Fix summary (Worker #1794)**
1. Replaced 64-bit DefaultHasher with **URL-safe base64 encoding (RFC 4648 §5)** for key components:
   - New keys use `b64_` prefix (e.g., `b64_dGhyZWFkOndpdGg6Y29sb25z`)
   - Encoding is **collision-free** (bijective) and **reversible** (can decode for debugging)
   - Encoding is **stable** across Rust versions (unlike DefaultHasher)
2. Added **backward-compatible read fallback**:
   - `fetch_from_redis_for_thread()` first tries new base64-encoded key
   - If empty and thread_id requires encoding, falls back to legacy hash-based key (`h_` prefix)
   - Logs `info` when legacy data is used for operator visibility
3. Fixed comment to accurately describe encoding behavior
4. Preserved legacy hash function (`legacy_hash_thread_id_for_redis`) for fallback reads
5. Added 7 unit tests verifying encoding properties and backward compatibility

**Files:**
- `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs` (sanitize_thread_id_for_redis, legacy_hash_thread_id_for_redis, fetch_from_redis_for_thread, tests)

**Verification:** `cargo check` + `cargo test sanitize_thread_id` + `cargo test legacy_hash` all pass

---

### M-1014 (P2): Old-data catch-up classification has weak fallback for partitions missing head offsets
**Category:** Metrics/Correctness + Config Robustness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Session head offsets are fetched once at startup from metadata; missing partitions fall back to a first-seen offset heuristic:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1047`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1122`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2298` (fallback described)

**Impact**
- If startup watermark fetch fails for some partitions, those partitions revert to “rewind-only” old-data classification, and initial catch-up decode errors can again be misclassified as **new-data** failures (alert/DLQ noise).

**Fix direction**
- Add **lazy per-partition watermark fetch** when encountering a partition missing a session head:
  - On first message for such partition, fetch high watermark and store it in `session_head_offsets`.
  - Track failures and retry with backoff.

**Acceptance**
- Partitions without startup watermarks still get correct catch-up classification after first message is seen.

---

### M-1015 (P2): Broadcast path still uses unbounded `await socket.send(...)` (send timeouts not applied)
**Category:** Server/Liveness + Telemetry Truth
**Where:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs`

**Evidence**
- Broadcast send loop sends cursor + binary without `send_with_timeout`:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:576`
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:584`
- Gap indicator send also lacks timeout:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:629`

**Impact**
- A wedged client connection can stall the handler task inside `.await`, consuming resources and delaying disconnect/backpressure behavior.

**Fix direction**
- Apply `send_with_timeout(...)` to **broadcast** sends too (cursor, binary, gap indicators).
- Export counters for send timeouts and disconnect reasons (timeout vs disconnect vs backpressure).

**Acceptance**
- A client that stops reading is disconnected within the configured timeout even in the broadcast path, and timeouts are visible in metrics.

---

### ~~M-1016 (P1)~~: Non-timeout decode/worker failures don't force reconnect; can skip messages and corrupt resume — ✅ FIXED #1793
**Category:** UI/Correctness (resume safety)
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- The binary processing chain only forces reconnect on timeout errors; other errors just log and decrement pending:
  - `observability-ui/src/App.tsx:1659`
  - `observability-ui/src/App.tsx:1675`

**Impact**
- If the worker throws or the decode pipeline errors (non-timeout), the UI can continue receiving later messages and eventually commit later cursors, meaning the failed message can be **skipped permanently** after reload/crash (same failure mode as pre-M-975 but via a different error path).

**Fix summary (Worker #1793)**
- Modified `.catch()` handler to treat ALL errors (not just timeouts) as fatal to resume correctness
- Non-timeout errors now call `markActiveRunsNeedResync()` and force reconnect via `wsProtocolErrorRef`
- Uses same `wsProtocolErrorRef.current` guard as timeout errors to prevent redundant closes
- Both timeout (close code 1011) and non-timeout (close code 1002) errors trigger reconnect

**Files:** `observability-ui/src/App.tsx:1674-1714`

**Verification:** `npx tsc --noEmit` passes

---

### ~~M-1017 (P1)~~: Reconnect resets apply-lag metrics while prior-epoch tasks can still mutate them (negative pendingCount) — ✅ FIXED #1793
**Category:** UI/Telemetry Correctness + Backpressure Safety
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- On reconnect, apply-lag metrics are replaced with a new object:
  - `observability-ui/src/App.tsx:984`
- In-flight chain tasks from the prior epoch decrement `applyLagMetricsRef.current.pendingCount` without epoch-guarding:
  - `observability-ui/src/App.tsx:1419`
  - `observability-ui/src/App.tsx:1631`
  - `observability-ui/src/App.tsx:1677`

**Impact**
- If any prior-epoch task completes after reconnect, it will decrement the **new** metrics object (which did not see the corresponding increments), producing:
  - negative pending counts,
  - broken `MAX_PENDING_BINARY_MESSAGES` logic (M-1007),
  - misleading health panel telemetry.

**Fix summary (Worker #1793)**
- Capture `metricsForThisBatch = applyLagMetricsRef.current` at increment time (line 1420)
- ALL decrement operations and metric updates use `metricsForThisBatch` instead of `applyLagMetricsRef.current`
- This ensures each message's increment/decrement operates on the SAME metrics object
- Prior-epoch tasks now decrement their own epoch's metrics, not the new epoch's
- UI state updates (`setApplyLagInfo`) are now gated by `wsEpoch === wsEpochRef.current` to avoid showing stale data

**Files:** `observability-ui/src/App.tsx:1414-1421,1426,1439,1462,1506,1642-1671,1693`

**Verification:** `npx tsc --noEmit` passes

---

### M-1018 (P2): Apply-lag “avg” is lifetime-average, not windowed (hides spikes and regressions)
**Category:** Telemetry Design/Truthfulness
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- Average latency is computed as `totalLatencyMs / totalApplied` since connection start:
  - `observability-ui/src/App.tsx:1643`

**Impact**
- A severe spike (e.g., 10s lag for 30 seconds) can be hidden by hours of prior low-latency data; operators cannot detect recent regressions reliably from the UI telemetry.

**Fix direction**
- Compute windowed metrics (e.g., 30s rolling window) and display both:
  - `avg_latency_ms_30s`, `p95_latency_ms_30s`, `max_latency_ms_30s`, `pending_now`.

**Acceptance**
- A short spike clearly reflects in the displayed lag metrics within the next reporting interval.

---

### M-1019 (P2): UI decode size limit is fixed; server max payload is configurable (config drift can break streaming)
**Category:** Configuration/Compatibility
**Where:** `observability-ui/src/proto/dashstream.ts`, `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- UI hard limits decompression size (10MB):
  - `observability-ui/src/proto/dashstream.ts:21`
- Server max payload is configurable via `WEBSOCKET_MAX_PAYLOAD_BYTES`:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1155`

**Impact**
- If operators raise server max payload (to allow larger snapshots/diffs), the UI can still fail decode due to its fixed limit, causing reconnect loops and apparent “instability” that is actually config mismatch.

**Fix direction**
- Negotiate limits at runtime:
  - expose `max_payload_bytes` in `/version` or a `/config` endpoint,
  - have UI read it and enforce `min(server_max, ui_safe_cap)` with a clear warning banner on mismatch.

**Acceptance**
- UI clearly reports config mismatch and does not silently fail when server payload limit changes.

---

### M-1020 (P2): Consumer advances offsets even on decode failures (data loss tradeoff not configurable)
**Category:** Kafka/Correctness + Ops Control
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Offsets are stored after processing regardless of decode success:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2689`

**Impact**
- When a decode failure is due to a transient bug or mismatch (not true data corruption), the server will still advance offsets and never re-attempt those messages after a fix, causing permanent gaps for clients relying on this stream.

**Fix direction**
- Make policy explicit and configurable:
  - `KAFKA_ON_DECODE_ERROR={skip|pause|halt}` defaulting to current behavior for availability,
  - when not skipping, stop consuming and alert loudly (so operators know they’re “stuck on a bad message”).

**Acceptance**
- Operators can choose between “never block” and “never skip” modes and understand the tradeoffs via metrics + logs.

---

### M-1021 (P2): Catch-up completion is only logged; no metric to alert on “still catching up”
**Category:** Metrics/Observability Gap
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Catch-up completion is tracked in-process and logged, but not exported as Prometheus metrics:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2310`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2372`

**Impact**
- Operators can’t alert on “server has been in catch-up for >N minutes” (which would explain old-data decode errors and lag) without scraping logs.

**Fix direction**
- Export a gauge per partition (or summary) indicating catch-up state:
  - `websocket_catchup_phase{partition} 1/0` and optionally `websocket_session_head_offset{partition}`.

**Acceptance**
- Catch-up status is visible and alertable in Prometheus/Grafana without log access.

---

### M-1022 (P2): ReplayBuffer metrics omit critical operational signals (buffer size/retention/oldest offsets)
**Category:** Metrics/Observability Gap
**Where:** `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs`, `state.rs`

**Evidence**
- Exported replay buffer metrics are only hit/miss and write-drop/failure counters:
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:1275`
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:468`

**Impact**
- Operators can’t answer “how close are we to cursor_stale?” or “is Redis retention collapsing?” from metrics alone.

**Fix direction**
- Export at least:
  - memory buffer length gauge,
  - oldest/latest offset per partition (or a bounded sample),
  - last Redis write timestamp (recency),
  - counts of known partitions/threads indexed.

**Acceptance**
- Dashboards can predict staleness/eviction risk and distinguish “no traffic” vs “buffer unhealthy”.

---

## Worker Priority (recommended order)

1. ~~**M-1017 (P1)**~~: ✅ FIXED #1793 - Fix apply-lag metrics epoch race (prevents broken backlog cap + misleading telemetry).
2. ~~**M-1016 (P1)**~~: ✅ FIXED #1793 - Force reconnect on any decode/worker failure (resume correctness).
3. **M-1013 (P1)**: Fix Redis thread_id key strategy + migration compatibility.
4. **M-1015/M-1014 (P2)**: Apply send timeouts to broadcast; strengthen watermark fallback.
5. **M-1019/M-1020 (P2)**: Make server↔UI config/skip policies explicit and observable.
6. **M-1021/M-1022/M-1018 (P2)**: Fill telemetry gaps so operators can trust and alert on streaming health.
