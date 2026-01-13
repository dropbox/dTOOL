# DashFlow v107 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1813
**Scope:** DashStream UI telemetry (`observability-ui/`), websocket-server health/metrics/circuit-breaker (`crates/dashflow-observability/.../websocket_server/`), producer-side DashStream callback (`crates/dashflow/src/dashstream_callback/`), and Kafka→Prometheus bridge for DashStream metrics (`crates/dashflow-prometheus-exporter/`).
**Prior:** v106 (M-1058..M-1067) — P2 OPEN (P1 FIXED #1812).

This audit focuses on **operator-truth metrics**, **correctness under edge cases**, and **config drift** across UI/server/exporter.

---

## Status (v107)

**0 P0 | 0 P1 | 2 P2 | 4 P3** open (4 P2 FIXED: M-1069, M-1070 #1815; M-1073, M-1074 #1816)

| ID | Priority | Category | Summary |
|----|----------|----------|---------|
| **M-1068** | P2 | Protocol/Correctness | `sequence==0` semantics are inconsistent across producer/server/UI; UI treats 0 as “real” but server drops it |
| ~~**M-1069**~~ | ~~P2~~ | ~~Server/Health + Metrics Correctness~~ | ✅ FIXED #1815 - /health now uses 120s sliding window decode error rate |
| ~~**M-1070**~~ | ~~P2~~ | ~~Server/Correctness~~ | ✅ FIXED #1815 - Circuit breaker uses same windowed rate as /health |
| ~~**M-1073**~~ | ~~P2~~ | ~~Producer/Metrics Correctness~~ | ✅ FIXED #1816 - `queue_depth` now uses `fetch_update` with saturating subtraction (prevents u64::MAX corruption) |
| ~~**M-1074**~~ | ~~P2~~ | ~~Exporter/Metrics Correctness~~ | ✅ FIXED #1816 - Session cleanup now runs every 100 events OR when tracker is large (fixes low-traffic under-reporting) |
| **M-1075** | P2 | Exporter/Config Drift + Reliability | prometheus-exporter decode limit is hard-coded (`DEFAULT_MAX_PAYLOAD_SIZE`), not aligned to deployment config |
| **M-1071** | P3 | Server/Performance | `get_send_timeout_secs()` parses env var on every WebSocket send (hot path) |
| **M-1072** | P3 | Server/Operator Truth | `/health` omits key streaming failure signals (send timeouts/failures, DLQ failures), forcing operators to scrape `/metrics` |
| **M-1076** | P3 | Exporter/Telemetry Gap | prometheus-exporter has no explicit “payload missing” counter (unlike websocket-server) |
| **M-1077** | P3 | UI/Maintainability → Correctness | Sequence parsing/semantics duplicated across `proto/dashstream.ts` and `decode.worker.ts` (drift risk) |

---

## New Issues (M-1068 to M-1077)

### M-1068 (P2): `sequence==0` semantics are inconsistent across producer/server/UI
**Where**
- Server treats `sequence==0` as “missing”:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:298` (`let sequence = (header.sequence > 0).then_some(header.sequence);`)
- Producer intentionally emits `sequence=0` for EventBatch headers:
  - `crates/dashflow/src/dashstream_callback/mod.rs:1160-1166`
- UI treats `seq >= 0` as “real”:
  - `observability-ui/src/hooks/useRunStateStore.ts:105-110`
- UI decoder emits `"0"` for `0` sequences:
  - `observability-ui/src/proto/dashstream.ts:40-47`
  - `observability-ui/src/workers/decode.worker.ts:43-48`

**Why it matters**
- This is a correctness footgun: any producer that omits sequence (proto3 default `0`) may be interpreted as “real seq=0” by the UI, while the server excludes it from gap detection and related metrics.
- It also makes protocol evolution brittle: “is 0 valid?” differs by component.

**Fix direction**
- Establish a single invariant: **“real producer sequences are strictly `>= 1`”**.
- Update UI helpers to treat `0` as “missing” (i.e., `isRealSeq(seq) => BigInt(seq) > 0n`), and ensure decoders return `undefined` for `0`.
- Keep the EventBatch header `sequence=0` design (it’s a batch envelope), but ensure it is never treated as a “resume cursor” / “lastAppliedSeq”.

**Acceptance**
- No component treats `sequence=0` as a resumable ordering signal.
- Any missing sequence (`0`) yields synthetic negative seqs in UI store and is excluded from “real sequence” comparisons.

---

### M-1069 (P2): `/health` uses lifetime decode error rate (not recency) and a mismatched denominator
**Where**
- `/health` computes:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:123-127`

**Why it matters**
- Lifetime ratios are not “operator truth” for streaming:
  - A recent spike can be hidden by a long clean history (false healthy).
  - A past spike can keep the system looking worse than current reality (false degraded), depending on thresholds.
- The numerator/denominator mismatch is subtle:
  - `decode_errors` increments only for “new data” decode failures, but the denominator uses `kafka_messages_received` (all messages, including catch-up), diluting the signal during startup/catch-up.

**Fix direction**
- Add a sliding window for decode errors (mirror `dropped_messages_last_120s`) and compute:
  - `decode_errors_last_120s / messages_received_last_120s` (or success+error in-window, excluding old-data phase if appropriate).
- Optionally expose both:
  - `decode_error_rate_lifetime` (for baseline) and
  - `decode_error_rate_120s` (for alerting/health).

**Acceptance**
- `/health` reports a recency-based decode error rate and uses a consistent in-window denominator.
- Health status uses the recency-based rate for “degraded” gating.

---

### M-1070 (P2): Circuit breaker uses lifetime decode error rate; restart decisions can be wrong
**Where**
- Circuit breaker monitor uses the same lifetime ratio:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:3404-3413`

**Why it matters**
- This monitor is supposed to act on **current** degraded conditions.
- Using lifetime ratios can:
  - delay restarts during a fresh corruption burst (ratio still low),
  - or prolong restarts because the system “remembers” old failures.

**Fix direction**
- Reuse the same windowed “degraded” predicate as `/health` (single source of truth).
- If keeping lifetime counters, treat them as “context”, not as restart inputs.

**Acceptance**
- Circuit breaker degraded decision is driven by recency-window metrics and aligns with `/health` status.

---

### M-1071 (P3): `get_send_timeout_secs()` parses env var on every WebSocket send (hot path)
**Where**
- `send_with_timeout()` reads timeout via env lookup every call:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:47-58`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:649-659`

**Why it matters**
- WebSocket sends are the hot path; repeated env parsing is unnecessary overhead and risks log spam if misconfigured.

**Fix direction**
- Parse once at startup and store in `ServerState` (or pass as parameter).

**Acceptance**
- No per-send env var parsing; timeout is a cached value.

---

### M-1072 (P3): `/health` omits key streaming failure signals (send + DLQ)
**Where**
- `ServerMetricsSnapshot` omits send failure/timeout counters and DLQ metrics:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs` (snapshot fields)
- `/health` only surfaces dropped messages and decode errors:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:171-199`

**Why it matters**
- Operators reading `/health` miss common “why is UI stale?” causes:
  - repeated `send_timeout` / `send_failed`,
  - DLQ failure/backpressure (forensics degraded),
  - replay-buffer Redis failures (some are shown, others aren’t).

**Fix direction**
- Extend `/health.metrics` with:
  - `send_failed_total`, `send_timeout_total`,
  - (optional) DLQ send failures in last window and total.
- Add alert strings when these are **recent**, not lifetime.

**Acceptance**
- `/health` highlights recent send/DLQ failure conditions without requiring `/metrics`.

---

### M-1073 (P2): ✅ FIXED #1816 - DashStreamCallback `queue_depth` underflow prevention
**Where**
- All 6 instances of `queue_depth.fetch_sub(1, ...)` replaced with `decrement_queue_depth_saturating()`:
  - `crates/dashflow/src/dashstream_callback/mod.rs` (batching path: 3 instances, non-batching path: 3 instances)

**Fix applied**
- Added `decrement_queue_depth_saturating()` helper function that uses `fetch_update` with `saturating_sub(1)`:
  - The atomic value itself is now updated with saturation, not just the returned value
  - If counter is 0, it stays 0 (no wrap to `u64::MAX`)
  - Debug logging added when decrement is called on zero (indicates internal invariant violation)
- All 6 `fetch_sub` call sites replaced with the helper function

**Acceptance criteria met**
- `queue_depth` atomic cannot wrap underflow
- Gauge and atomic remain consistent
- Invariant violation is logged (debug level) for future debugging

---

### M-1074 (P2): ✅ FIXED #1816 - prometheus-exporter session cleanup now event-driven
**Where**
- `crates/dashflow-prometheus-exporter/src/main.rs:777-789` (cleanup logic)

**Fix applied**
- Added `session_event_counter: Arc<AtomicU64>` field to track events processed.
- Added `SESSION_CLEANUP_INTERVAL` constant (100 events).
- Changed cleanup condition from `tracker.len() > 100` to:
  `event_count % SESSION_CLEANUP_INTERVAL == 0 || tracker.len() > 100`
- This ensures cleanup runs every 100 events regardless of tracker size, while still
  running more frequently when tracker is large.

**Acceptance criteria met**
- Session completion observations occur within `session_timeout_secs()` + bounded delay (max 100 events)
- Low-traffic scenarios no longer wait until shutdown to observe session completions
- Overhead intent preserved: cleanup still doesn't run on every single event

---

### M-1075 (P2): prometheus-exporter decode size limit is hard-coded (config drift risk)
**Where**
- Decode uses `DEFAULT_MAX_PAYLOAD_SIZE` directly:
  - `crates/dashflow-prometheus-exporter/src/main.rs:1042-1044`

**Why it matters**
- Deployments can (and do) tune payload limits (server/UI already do).
- If producer/server accept payloads that exporter rejects, dashboards silently lose metrics (and the exporter just counts generic “decode” failures).

**Fix direction**
- Add an exporter env var (shared naming with other components) to set max decode size:
  - e.g., `DASHSTREAM_MAX_PAYLOAD_BYTES` (preferred shared) or reuse `WEBSOCKET_MAX_PAYLOAD_BYTES` if policy is “one limit everywhere”.
- Emit startup log showing the configured limit (and whether it matches server’s `/version`).

**Acceptance**
- Exporter max decode size is configurable, and mismatches are observable.

---

### M-1076 (P3): prometheus-exporter lacks “payload missing” visibility
**Where**
- Messages with `payload=None` are silently skipped (but offsets are still advanced):
  - `crates/dashflow-prometheus-exporter/src/main.rs:995`

**Why it matters**
- Payload-missing is data loss / corruption and should be visible (websocket-server has `websocket_kafka_payload_missing_total`).

**Fix direction**
- Add `dashstream_kafka_payload_missing_total` (or similar) and increment it on `payload=None`.

**Acceptance**
- Operators can alert on payload-missing occurrences in the exporter.

---

### M-1077 (P3): Sequence conversion logic is duplicated across UI decoder and worker (drift risk)
**Where**
- `safeNonNegativeSequenceString` exists in both:
  - `observability-ui/src/proto/dashstream.ts:40-47`
  - `observability-ui/src/workers/decode.worker.ts:43-48`

**Why it matters**
- Protocol semantics changes (like treating `0` as missing) require coordinated edits; duplication increases the chance that one path diverges and silently corrupts graph ordering/metrics.

**Fix direction**
- Export a single shared helper from `observability-ui/src/proto/dashstream.ts` and import it in the worker (same pattern as `MAX_DECOMPRESSED_SIZE`).

**Acceptance**
- Only one implementation of sequence conversion exists in the UI codebase; worker and main-thread decoding match exactly.

---

## Worker Priority (v107)

1. ~~**M-1069 + M-1070 (P2)**~~: ✅ FIXED #1815 - Degraded/auto-restart decisions now use 120s windowed decode error rate.
2. ~~**M-1068 + M-1077 (P2/P3)**~~: ✅ FIXED #1817 - Normalize `sequence==0` semantics (> 0 means real), deduplicated UI helpers.
3. ~~**M-1073 (P2)**~~: ✅ FIXED #1816 - Producer queue depth metrics now use `fetch_update` with saturating subtraction.
4. ~~**M-1074 + M-1075 (P2)**~~: ✅ FIXED #1816/#1817 - Session cleanup event-driven; exporter max_payload_bytes configurable.
5. ~~**M-1071 + M-1072 + M-1076 (P3)**~~: ✅ FIXED #1817 - Cached send timeout via OnceLock; send counters in /health; payload_missing counter.
