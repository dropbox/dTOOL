# DashFlow v109 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1826
**Scope:** DashStream UI decode + graph-state reconstruction (`observability-ui/`), websocket-server streaming metrics + health (`crates/dashflow-observability/.../websocket_server/`), dashboards/operator truth (`grafana/`, `/health` consumers).
**Prior:** v106/v107 — ✅ fixed via #1812, #1815-#1818. v108 — **OPEN** (see `audits/AUDIT_dashstream_graph_state_streaming_telemetry_v108_2025-12-25.md`).

This re-audit focuses on **operator truth** and “failure-mode correctness”: situations where the system is *actively failing* but telemetry/behavior makes it look “fine”, or where malformed streaming input can wedge/crash the UI.

---

## Executive Summary (What’s new vs v108)

New gaps found in v109 are primarily:

1. **Kafka decode error policy is not consistently enforced**: `KAFKA_ON_DECODE_ERROR=pause` is ignored for `payload_too_large` and `payload_missing` branches, which undermines the durability/triage model.
2. **UI decompression safety is not real**: both main-thread and worker decoders call `fzstd.decompress()` *before* checking `MAX_DECOMPRESSED_SIZE`, so a zstd “decompression bomb” can still OOM/freeze the tab/worker.
3. **Graph-state reconstruction can become O(N²)**: JSON Patch application clones state for every op; combined with unbounded patch op count/path sizes, this can freeze even for “valid” streams.
4. **Telemetry consumers are stale**: the UI `/health` model and calculations use lifetime counters and omit newer health fields (windowed rates, replay buffer metrics, send failure counters), which leads to wrong operator decisions.

---

## Status (v109)

**2 P0 | 4 P1 | 3 P2 | 3 P3** open (new)

| ID | Priority | Category | Summary |
|----|----------|----------|---------|
| **M-1096** | **P0** | Server/Correctness + Config | `KAFKA_ON_DECODE_ERROR=pause` is ignored for `payload_too_large` and `payload_missing` branches (policy bypass) |
| **M-1097** | **P0** | UI/Resource Safety | zstd size guard runs **after** `fzstd.decompress()` allocation (main decoder + worker) → decompression bomb can still OOM/freeze |
| **M-1098** | **P1** | UI/Resource Safety | `extractState()` “size check” allocates full UTF-8 buffer (`TextEncoder().encode`) → large strings can still OOM/freeze before JSON.parse guard |
| **M-1099** | **P1** | UI/Operator Truth | UI `/health` model + calculations use lifetime totals and omit newer fields (windowed rates, replay_buffer, send failures/timeouts) |
| **M-1100** | **P1** | UI/Correctness | UI “Error Distribution” double-counts decode errors (Kafka Errors already includes Decode Errors) |
| **M-1101** | **P1** | UI/Performance + Correctness | JSON Patch apply clones full state per operation; plus no cap on op count/path sizes → O(N²) freeze risk |
| **M-1102** | P2 | UI/Telemetry Correctness | `getNumberAttribute()` uses `parseFloat` (accepts junk) and ignores `floatValue` wrapper; numeric telemetry can be wrong/missing |
| **M-1103** | P2 | Server/Metrics Correctness | Prometheus metric registration failures are inconsistently handled (`decode_errors` returns `Some` even if not registered) → silent missing metrics |
| **M-1104** | P2 | Server/Metrics Correctness | `websocket_dlq_send_failures_total{reason=...}` is mis-labeled in the oversized-payload DLQ send path (uses `error_type`) |
| **M-1105** | P3 | Server/Performance | Sliding-window tracking uses a `Mutex` lock per message (hot path) → throughput contention under load |
| **M-1106** | P3 | Dashboards/Operator Truth | Grafana queries frequently use `… or vector(0)` which masks missing series/registration failures; add explicit “absent metric” detection |
| **M-1107** | P3 | Server/Health Design | `websocket_kafka_payload_missing_total` is not surfaced in `/health` or degraded predicates despite indicating data loss/corruption |

---

## New Issues (M-1096 to M-1107)

### M-1096 (P0): `KAFKA_ON_DECODE_ERROR=pause` is ignored for `payload_too_large` + `payload_missing` paths

**Where**
- Oversized payload path increments error counters and then `continue`s without applying `decode_error_policy`:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2211` … `:2340`
- Payload-missing path always advances offsets (“skip policy”) without consulting `decode_error_policy`:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2808` … `:2834`

**Why it matters**
- In `pause` mode, operators expect the consumer to stop **on any unprocessable message**, not just protobuf decode failures.
- As written, “pause mode” can still advance offsets past corrupted/oversized/null payloads, undermining durability and forensic replay expectations.

**Fix direction**
- Centralize “unprocessable message handling” into a shared helper that:
  - increments the right counters (including windowed denominators — see v108/M-1085),
  - emits DLQ records as appropriate,
  - then applies `DecodeErrorPolicy::{Skip|Pause}` consistently:
    - **Pause:** do not store offsets; enter paused loop (same behavior as current decode-error pause path).
    - **Skip:** store offset and update lag-monitor state, then continue.

**Acceptance**
- With `KAFKA_ON_DECODE_ERROR=pause`, an oversized payload and a payload-missing message both cause the consumer to stop consuming and **do not** advance stored offsets.
- With `KAFKA_ON_DECODE_ERROR=skip`, both cases advance offsets and do not re-trigger on restart.

---

### M-1097 (P0): zstd decompression cap is enforced after allocation (UI main decoder + worker)

**Where**
- Main decoder decompresses before checking `MAX_DECOMPRESSED_SIZE`:
  - `observability-ui/src/proto/dashstream.ts:398-407`
- Worker decoder does the same:
  - `observability-ui/src/workers/decode.worker.ts:167-179`

**Why it matters**
- `fzstd.decompress()` must allocate the output buffer; a zstd “decompression bomb” can exhaust memory **before** we check size.
- This is attacker-controlled/untrusted input (Kafka → WebSocket → UI). The current guard does not actually prevent OOM/freeze.

**Fix direction**
- Enforce an *allocation-safe* limit before calling `decompress()`:
  - Parse zstd frame header and reject frames whose declared content size exceeds `MAX_DECOMPRESSED_SIZE`.
  - If content size is “unknown” in the frame, either:
    - reject (safer default), or
    - use a decompressor implementation that supports bounded output / streaming with explicit max output.
- Ensure both main-thread decoder and worker use the same helper (single source of truth).

**Acceptance**
- A compressed message that would decompress to > `MAX_DECOMPRESSED_SIZE` is rejected without allocating the full output buffer (no tab crash).
- Worker and main decoder share identical behavior (no drift).

---

### M-1098 (P1): `extractState()` size check allocates a full byte buffer (DoS)

**Where**
- `extractState()` computes size via `new TextEncoder().encode(stateJson).length`:
  - `observability-ui/src/hooks/useRunStateStore.ts:476`

**Why it matters**
- For a very large `stateJson` string, `TextEncoder().encode(...)` allocates a full `Uint8Array` of the encoded size.
- This can OOM/freeze the UI **before** the guard can skip JSON.parse.

**Fix direction**
- Replace the “allocate-to-measure” byte sizing with a non-allocating approximation:
  - e.g., use `stateJson.length` with a conservative multiplier (UTF-16 to UTF-8 worst-case),
  - or implement a bounded byte-length estimator that walks the string but stops early once the cap is exceeded.
- Add a dedicated counter/log when state is skipped for size.

**Acceptance**
- Oversized `stateJson` strings do not cause a second large allocation during size checking.
- Size guard remains conservative (never allows > max through).

---

### M-1099 (P1): UI `/health` model and calculations are stale (windowed rates + replay buffer + send failures ignored)

**Where**
- UI HealthResponse type omits fields present in server health response:
  - UI: `observability-ui/src/App.tsx:63-83`
  - Server: `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:67-81`
  - Server metrics include windowed and send failure/timeouts fields:
    - `crates/dashflow-observability/src/bin/websocket_server/state.rs:536-545`
- UI computes error rate using lifetime totals:
  - `observability-ui/src/App.tsx:853-857`

**Why it matters**
- Operators use the UI dashboard to answer “is this currently failing?”.
- Lifetime rates dilute current spikes; ignoring windowed fields defeats the server-side v107/v106 work and can mask active breakage.
- Replay buffer health (Redis drops/failures) and send failures/timeouts are invisible in UI despite being present in `/health`.

**Fix direction**
- Update UI `HealthResponse` typing and rendering to include:
  - `decode_errors_last_120s`, `messages_last_120s`, `send_failed`, `send_timeout`,
  - `replay_buffer` metrics.
- Compute “error rate” using windowed counters when available, and fall back to lifetime only if missing.

**Acceptance**
- UI shows windowed decode error rate as primary health signal.
- UI surfaces replay buffer drop/failure warnings and send_failed/send_timeout visibility.

---

### M-1100 (P1): UI “Error Distribution” double-counts decode errors

**Where**
- UI pie chart uses:
  - `Kafka Errors: health.metrics.kafka_errors`
  - `Decode Errors: health.metrics.decode_errors`
  - `Infrastructure: health.metrics.infrastructure_errors`
  - `observability-ui/src/App.tsx:2481-2485`

**Why it matters**
- In the server, decode errors increment `kafka_errors` and `decode_errors` (decode errors are a subset).
- The chart implies these are disjoint categories, which is false and misleads operators.

**Fix direction**
- Render disjoint buckets:
  - `decode_errors`
  - `other_kafka_errors = kafka_errors - decode_errors` (clamped at 0)
  - `infrastructure_errors`
- Alternatively, rename chart to explicitly show overlap (but disjoint is better for operator intuition).

**Acceptance**
- Chart categories are disjoint and sum correctly; values reflect actual semantics.

---

### M-1101 (P1): JSON Patch apply is O(N²) and lacks bounds on patch size/complexity

**Where**
- Each patch op deep-clones full state (`safeClone`) before applying:
  - `observability-ui/src/utils/jsonPatch.ts:522-534` (applyPatch → applyPatchOp clone per op)

**Why it matters**
- A StateDiff with many operations against a large state causes:
  - repeated deep clones,
  - quadratic runtime/memory amplification,
  - UI freezes even for “valid” streams under load.

**Fix direction**
- Change patch application to:
  - clone once per StateDiff, then apply ops in-place on that clone, or
  - implement structural sharing with bounded cloning of touched subtrees only.
- Add explicit caps:
  - max operations per StateDiff (count),
  - max total path bytes,
  - max value bytes (already bounded by message payload, but still worth explicit).
- On cap breach: mark run as `needsResync` and skip applying diffs until a full snapshot arrives.

**Acceptance**
- Applying a StateDiff with N ops performs O(state_size + N) work, not O(N × state_size).
- Unreasonable patch sizes/paths are rejected without freezing; UI indicates “needs resync”.

---

### M-1102 (P2): Numeric telemetry parsing is permissive and incomplete (`parseFloat`, ignores `floatValue`)

**Where**
- `getNumberAttribute()` uses `parseFloat` and only checks `intValue` wrapper:
  - `observability-ui/src/utils/attributes.ts:47-60`

**Why it matters**
- `parseFloat("123abc")` yields `123`, silently accepting garbage.
- Producer attributes may legitimately use `floatValue`; these will currently be dropped, causing missing duration/metrics fields and confusing graphs.

**Fix direction**
- Support both `intValue` and `floatValue` wrappers.
- Parse strings strictly:
  - accept only canonical numeric strings (`/^-?\d+(\.\d+)?$/` or more strict per expected field),
  - or use `Number(s)` and require exact round-trip string match after trimming.

**Acceptance**
- Invalid numeric strings are rejected (no partial parse).
- Float-valued attributes are correctly read and displayed where used.

---

### M-1103 (P2): Prometheus metric registration failures are inconsistently handled (silent missing metrics)

**Where**
- `decode_errors` returns `Some(m)` even if registry registration fails:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:765-778`
- Other metrics correctly return `None` on registration failure (example):
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:794-802`

**Why it matters**
- Registration failures (duplicate metric names, registry misuse) produce a metric object that the code keeps incrementing, but Prometheus never scrapes it.
- Combined with dashboard expressions like `… or vector(0)`, this can fully mask missing instrumentation.

**Fix direction**
- Make a consistent rule: if `register()` fails, treat the metric as unavailable (`None`), and log once.
- Consider a startup “metrics registry integrity” self-check that fails fast in non-prod, warns in prod.

**Acceptance**
- No metric is returned as `Some` unless it is successfully registered.
- Missing metric series are detectable (not silently replaced with zeros in dashboards).

---

### M-1104 (P2): DLQ send failure metric label misuse on oversized payload DLQ send

**Where**
- On DLQ send error, code increments `websocket_dlq_send_failures_total{reason=...}` using `error_type` value:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2328-2332`

**Why it matters**
- This pollutes the `reason` label space with values like `payload_too_large` (not a failure reason).
- Alerting and dashboards that expect stable reasons (`timeout`, `kafka_error`, `backpressure`) become misleading.

**Fix direction**
- Mirror the normal decode-error DLQ send path’s reason classification:
  - map errors to `timeout` vs `kafka_error`,
  - keep `backpressure` for semaphore refusal.

**Acceptance**
- `websocket_dlq_send_failures_total` label values are limited to the documented reason set.

---

### M-1105 (P3): Windowed metric tracking locks a `Mutex` per message on the hot path

**Where**
- Per-message calls lock a `Mutex`:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:399-422` (`record_message_received`)
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:386-398` (`record_decode_error`)

**Why it matters**
- Under high throughput, mutex contention can become a bottleneck and distort timing (especially when combined with many clients).

**Fix direction**
- Consider:
  - lock-free ring buffer (fixed buckets per second),
  - atomic counters per time bucket with periodic rotation,
  - or sampling (record 1/N messages) with scaling.

**Acceptance**
- Windowed metrics collection does not materially reduce throughput under load (measured in a perf test or load sim).

---

### M-1106 (P3): Grafana “or vector(0)” masks missing metrics/registration failures

**Where**
- Example panels use fallback-to-zero:
  - `grafana/dashboards/streaming_metrics_dashboard.json:422-431`

**Why it matters**
- When a metric disappears due to registration/config issues, dashboards still show “0” instead of “missing”, creating a false sense of health.

**Fix direction**
- Add explicit “instrumentation missing” panels/alerts:
  - use `absent(<metric>)`,
  - or show a “scrape present” metric per component.
- Use `or vector(0)` sparingly (only where “0 is a safe default”).

**Acceptance**
- If `websocket_decode_errors_total` or DLQ metrics disappear, dashboards show a clear “missing instrumentation” signal within minutes.

---

### M-1107 (P3): Payload-missing is not surfaced in `/health` despite indicating data loss/corruption

**Where**
- Payload missing counter exists:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:942-957` (Prometheus `websocket_kafka_payload_missing_total`)
- `/health` response and status predicates do not include payload-missing rate/count:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:113+` (status uses decode error rate + staleness, but not payload-missing)

**Why it matters**
- Null payloads can mean tombstones/compaction or producer bugs; skipping them is an availability tradeoff, but operators need visibility in `/health`.

**Fix direction**
- Add payload-missing counters (ideally windowed) into `ServerMetricsSnapshot` and `/health` alert/status rules.
- Consider counting payload-missing as “decode error” for windowed error rate, or track a separate “data loss” window.

**Acceptance**
- `/health` includes payload-missing visibility and triggers a clear warning/degraded state when payload-missing rate is high.

---

## Worker Priority (after Part 36 Paragon Apps)

1. **M-1096** (P0): policy correctness (pause/skip consistency across all unprocessable-message paths)
2. **M-1097** (P0): real decompression safety (no post-allocation caps)
3. **M-1101 + M-1098** (P1): prevent UI freezes from large state/patches (make guards allocation-safe + avoid O(N²))
4. **M-1099 + M-1100** (P1): fix UI operator truth (/health model + disjoint error distribution)
5. **M-1103 + M-1104 + M-1107** (P2/P3): metrics/schema correctness and health completeness
6. **M-1105 + M-1106** (P3): performance/observability hardening

**Note:** Several v108 items can be fixed opportunistically while touching these areas (especially v108/M-1085 and v108/M-1095).
