# DashFlow v95 Skeptical Audit: DashStream Streaming Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1770
**Scope:** DashStream UI decode + graph-state timeline/cursors (`observability-ui/`), WebSocket server forward path (`crates/dashflow-observability/.../websocket_server/`), producer backpressure counters (`crates/dashflow/src/dashstream_callback/`)
**Prior context:** v30 (M-806..M-808), v33 “App.tsx/dashstream.ts” skim, v34 “useRunStateStore.ts” skim.

This is an intentionally skeptical re-audit focused on: **timestamp correctness**, **resume/replay cursor correctness**, **decoder correctness**, and **telemetry trustworthiness** (metrics/logging that operators rely on).

---

## Executive Summary

Found **10 NEW gaps** (M-973..M-982). Two are correctness-critical because they can cause **permanent message skips** or **nonsensical latency/timeline math**:

- **M-975 (P1)**: UI treats `decoder.decode(...) === null` as a soft drop and continues streaming, allowing later Kafka cursors to be committed past an unapplied message → **permanent skip** after reload/crash.
- **M-973 (P2)**: “M-807 fixed” inner-event timestamp extraction is still unit-wrong: `timestampUs` is treated as milliseconds, which can invert/ruin ordering and latency.

Several other issues are “measurement correctness” failures: we *collect* metrics/logs but they can be misleading or incomplete (schema version not validated, producer drop metrics mis-labeled, websocket-server hot-path logging).

---

## New Issues (M-973 to M-982)

### M-973 (P2): `timestampUs` treated as milliseconds for `event_batch` inner events
**Category:** Telemetry/UI Correctness
**Where:** `observability-ui/src/hooks/useRunStateStore.ts` (event_batch handling)

**Evidence**
- `innerTimestampUs` is read from `event.header.timestampUs`, but the value is assigned directly to `timestamp` without converting microseconds → milliseconds:
  - `observability-ui/src/hooks/useRunStateStore.ts:1275-1293`

This is a follow-up to v30’s M-807: the “use inner timestampUs” change exists, but the unit conversion is wrong.

**Impact**
- UI latency (`now - decoded.timestamp`) and ordering can become wildly wrong when an inner timestamp is chosen.
- Graph timelines become inconsistent because other message types are `tsUs / 1000`, but this path uses raw `timestampUs`.

**Fix direction**
- Convert inner timestamps from microseconds to milliseconds consistently:
  - Accept `bigint | number | Long-like` for `timestampUs`.
  - Convert to an integer milliseconds timestamp: `ms = Math.floor(us / 1000)`.
- Add a defensive sanity check: if derived ms is non-finite or absurd (e.g., `< 0` or `> Date.now() + 365d`), fall back to batch timestamp and log a single warning per run.

**Acceptance**
- For a batch with inner timestamps, all inner events’ `timestamp` values are in the same unit as other decoded messages (ms since epoch).
- Latency graph no longer shows massive negative/huge values from mixed units.

---

### M-974 (P4): UI uses truthiness (`||`, `if (decoded.timestamp)`) for timestamps
**Category:** Telemetry/UI Edge Correctness
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- `timestamp: decoded.timestamp || now` and `if (decoded.timestamp) { ... }`:
  - `observability-ui/src/App.tsx:1410`
  - `observability-ui/src/App.tsx:1455`
  - `observability-ui/src/App.tsx:1472`

**Impact**
- `timestamp=0` (valid epoch) is treated as missing.
- `NaN`/non-finite timestamps can silently enter event/timeline state and break derived calculations.

**Fix direction**
- Replace truthiness with explicit finiteness checks:
  - `const ts = Number.isFinite(decoded.timestamp) ? decoded.timestamp : now;`
  - `if (Number.isFinite(decoded.timestamp)) { ... }`

**Acceptance**
- `timestamp=0` is preserved.
- `NaN` doesn’t pollute event/timeline arrays.

---

### M-975 (P1): UI decode failure can permanently skip messages (cursor commits can move past an unapplied frame)
**Category:** Resume/Replay Correctness
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- On decode failure, the UI logs and returns, but **does not force reconnect**:
  - `observability-ui/src/App.tsx:1376-1382`

**Why this is correctness-critical**
- The cursor was already consumed/cleared (`pendingKafkaCursorRef.current = null` earlier).
- Later messages can still decode+apply, and **later cursors will be committed**.
- Result: the failed message is now behind the committed offset → after reload/reconnect, replay resumes after it → **permanent skip**.

**Fix direction**
- Treat `decoded === null` as a protocol/data error:
  1) mark active runs as needing resync (`markActiveRunsNeedResync(...)`)
  2) bump epoch and close WS (`close(1002, 'decode_failure')`) so the server replays from the last committed cursor
  3) optionally count this in a UI-side counter displayed in the health panel (“UI decode failures”)

**Acceptance**
- If a single message fails decode, the UI reconnects immediately and does not commit offsets beyond the failed message without a resync.

---

### M-976 (P2): UI ignores `schemaVersion` in DashStream headers (silent incompatibility risk)
**Category:** Protocol/Compatibility
**Where:** `observability-ui/src/proto/dashstream.ts`, `observability-ui/src/App.tsx`

**Problem**
- DashStream headers carry `schemaVersion`, but the UI does not validate it before trusting decoded content and committing cursors.

**Impact**
- During rolling upgrades, UI can silently misinterpret fields, drop messages, or apply incorrect state diffs.
- This becomes a telemetry trust issue: the UI may appear “healthy” while actually skipping or mis-decoding.

**Fix direction**
- Plumb `schemaVersion` into `DecodedMessage` (or expose it via a helper).
- Maintain a UI-expected schema version constant (generated alongside `dashstream.schema.json`), and:
  - if `schemaVersion > expected`: show a prominent banner “UI schema out of date; run proto:gen” and force resync/reconnect behavior (do not commit cursors).
  - if `schemaVersion < expected`: allow but warn once (backwards compatibility).

**Acceptance**
- Schema mismatches are visible and prevent “silent success”.

---

### M-977 (P2): EventBatch max sequence selection can lose precision due to protobufjs `Long` handling
**Category:** Resume/Replay Correctness
**Where:** `observability-ui/src/proto/dashstream.ts`

**Evidence**
- Max inner sequence is computed via direct `>` comparisons on `eventHeader.sequence`:
  - `observability-ui/src/proto/dashstream.ts:389-418`

In protobufjs, `uint64` typically decodes as a `Long` object (not a JS bigint). JS relational comparisons on `Long` can devolve to `number` coercions and lose ordering > 2^53, defeating the entire “sequence as string” correctness story.

**Impact**
- Wrong `DecodedMessage.sequence` for `event_batch` → wrong `lastSequencesByThread` persistence → resume/replay gaps/skips.

**Fix direction**
- Use a single canonical conversion for sequences:
  - `const seqStr = coerceU64ToString(eventHeader?.sequence);`
  - Compare using `BigInt(seqStr)` (guarded) or compare strings via an existing helper.
  - Store `sequence` as the max string, never by comparing `Long` directly.

**Acceptance**
- For sequences > 2^53, `event_batch` resumes still advance monotonically and correctly.

---

### M-978 (P2): UI zstd decompression has no explicit output-size cap (OOM/DoS risk, even with server-side limits)
**Category:** Robustness/Perf
**Where:** `observability-ui/src/proto/dashstream.ts`

**Evidence**
- `messageBuffer = decompress(compressedData);` with no cap:
  - `observability-ui/src/proto/dashstream.ts:313-333`

**Impact**
- Large frames can block the main thread and/or cause high memory pressure.
- Even if the server caps at 10MB today, this is brittle: any server config change (or alternate ingestion path) turns this into a client-side DoS.

**Fix direction**
- Enforce a UI-side max decompressed size aligned with server config (and make it explicit):
  - reject if decompressed length exceeds max; treat as decode failure (M-975 behavior).
- Prefer `subarray(1)` over `slice(1)` to avoid needless copies where possible.

**Acceptance**
- Oversized compressed frames fail fast and do not freeze the UI.

---

### M-979 (P3): WebSocket server hard-codes `DEFAULT_MAX_PAYLOAD_SIZE` and doesn’t surface “dropped for size” as a first-class metric
**Category:** Configuration/Telemetry
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Decode uses `dashflow_streaming::codec::DEFAULT_MAX_PAYLOAD_SIZE` (10MB) directly:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2223-2226`

**Impact**
- Operators can’t tune for real workloads (large state snapshots/diffs) without code changes.
- Decode failures from “too large” look like generic decode errors; there’s no “payload_too_large” counter/alert.

**Fix direction**
- Add env var (e.g., `WEBSOCKET_MAX_PAYLOAD_BYTES`) and pass it into `decode_message_compatible`.
- Add a labeled counter for decode failures with `error_type="payload_too_large"` (or a dedicated counter).
- Document the knob in `monitoring/PROMETHEUS_METRICS.md` and `docs/OBSERVABILITY_INFRASTRUCTURE.md`.

**Acceptance**
- Deployments can set max payload size without recompiling.
- Payload-too-large becomes visible as its own metric trendline.

---

### M-980 (P3): WebSocket server logs per message in the hot path (throughput collapse risk; distorts streaming metrics)
**Category:** Performance/Telemetry Trustworthiness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Per-message `println!("📨 ... Forwarding ...")` on every successful decode:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2231-2235`

**Impact**
- High-volume stdout logging can become the bottleneck, causing:
  - backpressure and dropped messages,
  - inflated end-to-end latency,
  - misleading conclusions about Kafka/WebSocket health.

**Fix direction**
- Replace with either:
  - `tracing::debug!` behind a log level, and/or
  - rate-limited logging (e.g., every N messages or every T seconds), and
  - rely on Prometheus counters/histograms for throughput.

**Acceptance**
- Under load, CPU time is not dominated by stdout.
- Operators still have visibility via metrics.

---

### M-981 (P3): Producer backpressure drop metric is mislabeled for `spawn_tracked` (always `message_type="event"`)
**Category:** Telemetry/Correctness
**Where:** `crates/dashflow/src/dashstream_callback/mod.rs`

**Evidence**
- `spawn_tracked` increments `TELEMETRY_DROPPED_TOTAL` with `["event","capacity_limit"]` regardless of what task was dropped:
  - `crates/dashflow/src/dashstream_callback/mod.rs:1069-1106`
- `emit_quality_metrics_async()` uses `spawn_tracked(...)`, so **metrics drops are counted as event drops**.

**Impact**
- `dashstream_telemetry_dropped_total{message_type="event"}` is inflated and misleading.
- You cannot alert correctly on dropped metrics vs dropped events.

**Fix direction**
- Change `spawn_tracked` signature to accept `message_type: &'static str` and use it in:
  - `TELEMETRY_DROPPED_TOTAL.with_label_values(&[message_type, "capacity_limit"])`
- Ensure callers pass `"metrics"` when spawning metrics sends.

**Acceptance**
- Dropped metrics increment `message_type="metrics"` and do not contaminate `"event"`.

---

### M-982 (P3): UI `lastSequencesByThread` persistence relies on outer EventBatch “effective” thread/sequence only
**Category:** Resume/Correctness
**Where:** `observability-ui/src/App.tsx`, `observability-ui/src/proto/dashstream.ts`

**Evidence**
- UI only persists `lastSequencesByThreadRef` based on the outer decoded message:
  - `observability-ui/src/App.tsx:1422-1452`
- For `event_batch`, the decoder picks `effectiveThreadId` and a single `sequence` (max inner seq):
  - `observability-ui/src/proto/dashstream.ts:383-419`

**Impact**
- If an EventBatch ever contains mixed thread_ids (bug, future feature, or malformed data), persisted sequences can become wrong for some threads.
- This is a “works until it doesn’t” correctness hazard: resume depends on persisted sequences being authoritative.

**Fix direction**
- Enforce invariants explicitly:
  - In decoder: assert/validate all inner events share thread_id; if not, log and set `threadId` undefined for outer batch (forcing per-event handling only).
  - Better: have the UI update `lastSequencesByThreadRef` based on inner events (max per thread), not the outer synthetic summary.

**Acceptance**
- Persisted thread cursors are correct even if EventBatch contains multiple thread IDs.

---

## Worker Priority (recommended order)

1. **M-975 (P1)**: Close/reconnect on UI decode null; never commit offsets past an unapplied message.
2. **M-973 (P2)**: Fix inner `timestampUs` units + Long handling; align ms everywhere.
3. **M-977 (P2)**: Fix EventBatch max sequence selection to be truly precision-safe.
4. **M-976 (P2)** + **M-978 (P2)**: Add schemaVersion + explicit decompression limits to prevent silent/DoS failure modes.
5. **M-979/M-980/M-981/M-982**: Config + telemetry hygiene (trustworthy metrics, no hot-path stdout).

---

## Notes / Why I believe these are real (skeptical posture)

- All issues above are rooted in **concrete code paths** and *not* “style preferences”.
- Several are “telemetry trust” issues: if the UI can silently drop frames or accept incompatible schemas, dashboards and operator intuition become unreliable.
- Some prior audits marked these areas as “no significant issues”, but the current code still contains these gaps; this report treats “absence of an alert” as **not proof** of correctness.
