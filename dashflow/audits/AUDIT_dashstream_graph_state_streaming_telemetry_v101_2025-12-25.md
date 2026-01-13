# DashFlow v101 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1781
**Scope:** DashStream UI decode + graph-state timeline (`observability-ui/`), WebSocket server forward/replay path + replay buffer (`crates/dashflow-observability/.../websocket_server/`), DashStreamCallback telemetry emission (`crates/dashflow/src/dashstream_callback/`)
**Prior:** v95 (M-973..M-982) — verified fixed in #1771-#1773

This pass assumes “fixed” claims are not proof; it re-checks the pipeline for **telemetry truth**, **resume/replay correctness**, and **configuration footguns**.

---

## Executive Summary

Found **10 NEW issues** (M-993..M-1002). Highest risk:

- **M-997 (P1)**: UI keeps applying messages under `schemaVersionMismatch`, only gating cursor commits → UI can display/derive **wrong state** while looking connected.
- **M-994 (P1)**: websocket-server old-data decode suppression is timestamp-based → stale producer timestamps can **hide real decode failures** from alerting.
- **M-995/M-996 (P2)**: websocket-server uses `Vec<u8>` broadcast/replay payloads → **payload cloning / allocation amplification** with multiple clients and replay pages.

---

## New Issues (M-993 to M-1002)

### M-993 (P3): `schema_id` extraction in `App.tsx` is wrong for protobuf AttributeValue
**Category:** UI/Telemetry Correctness
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- `App.tsx` treats `attributes['schema_id']` as a direct string:
  - `observability-ui/src/App.tsx:1437`
  - `observability-ui/src/App.tsx:1439`
- But the codebase already handles protobufjs AttributeValue wrapper (`{ stringValue: ... }`) elsewhere:
  - `observability-ui/src/hooks/useRunStateStore.ts:10`

**Impact**
- Schema highlighting / schema_id-based diagnostics silently fail (schema_id never detected even when present).

**Fix direction**
- Reuse the same extraction logic as `getStringAttribute()` or move it into a shared utility and use it in both `App.tsx` and `useRunStateStore.ts`.

**Acceptance**
- With an event attribute `schema_id` encoded as `{stringValue:"..."}`, the UI extracts and displays it.

---

### M-994 (P1): websocket-server “old data” decode suppression is timestamp-based and can hide real failures
**Category:** Metrics/Correctness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- Old-data classification uses Kafka timestamps vs server start time:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2165`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2195`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2199`

**Failure mode**
- If producers emit `CreateTime` far in the past (clock skew, backfilled topics, replayed data), **new** messages can be misclassified as old; decode failures get counted as old-data and do not flow through the main error path.

**Fix direction**
- Switch old-data classification to **offset-based** per partition (session start offset / first seen offset), not timestamps.
- Or require an explicit “I am replaying old history” config flag before suppressing decode errors.
- Add a “suspicious old-data classification” counter when `is_old_data==true` but offset is close to head/lag is low.

**Acceptance**
- Stale producer timestamps cannot mask decode errors as “old data”.

---

### M-995 (P2): Broadcast payload cloning is O(message_bytes × clients) due to `Vec<u8>` message type
**Category:** Server/Performance + Telemetry Trust
**Where:** `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs`, `handlers.rs`

**Evidence**
- `OutboundBinaryMessage` contains an owned `Vec<u8>` and is `Clone`:
  - `crates/dashflow-observability/src/bin/websocket_server/replay_buffer.rs:46`
- Replay paths clone the entire payload for send:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:922`
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:1134`

**Impact**
- With multiple clients and replay pages, full-payload cloning amplifies CPU + memory bandwidth and can turn into self-inflicted drops/latency (metrics become “true” but caused by the telemetry system).

**Fix direction**
- Replace payload storage with `bytes::Bytes` or `Arc<[u8]>` so cloning is cheap.
- Ensure replay sends don’t deep-clone payload bytes.

**Acceptance**
- Increasing client count does not multiply payload-copy CPU linearly with message size.

---

### M-996 (P2): websocket-server duplicates payload allocations across broadcast and replay buffer storage
**Category:** Server/Performance
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- The server materializes `payload.to_vec()` then uses copies across broadcast + replay buffer persistence:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2238`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2252`

**Impact**
- Even after switching to `Bytes`, replay buffer storage still duplicates memory unless the same ref-counted buffer is shared.

**Fix direction**
- Thread a single shared buffer instance (Bytes/Arc) through:
  - broadcast send
  - replay buffer memory store
  - Redis write task payload

**Acceptance**
- One Kafka payload results in one shared buffer on the hot path.

---

### M-997 (P1): `schemaVersionMismatch` only gates cursor commits; UI still applies potentially incompatible messages
**Category:** Protocol/UI Correctness
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- Mismatch detection sets a flag, but `processRunStateMessageRef.current(decoded)` still runs:
  - `observability-ui/src/App.tsx:1406`
  - `observability-ui/src/App.tsx:1423`
- Cursor commits are gated, not apply:
  - `observability-ui/src/App.tsx:1540`
  - `observability-ui/src/App.tsx:1542`

**Impact**
- The UI can continue producing state/timeline based on messages it explicitly warns may be misinterpreted.
- This is a correctness/incident-response failure mode: “UI is connected” does not imply “UI is correct”.

**Fix direction**
- When mismatch becomes active:
  - stop applying messages (skip `processRunStateMessageRef.current(decoded)`),
  - show a blocking banner until UI is updated,
  - optionally reconnect in a paused state after update.

**Acceptance**
- Under schema mismatch, the UI does not mutate graph state and clearly indicates incompatibility.

---

### M-998 (P2): UI decode/decompress is synchronous on main thread; timeouts don’t prevent freezes
**Category:** UI/Performance + Correctness
**Where:** `observability-ui/src/App.tsx`, `observability-ui/src/proto/dashstream.ts`

**Evidence**
- Decode is called synchronously:
  - `observability-ui/src/App.tsx:1380`
- Decoder does synchronous zstd decompression + protobuf decode:
  - `observability-ui/src/proto/dashstream.ts:390`
  - `observability-ui/src/proto/dashstream.ts:418`

**Impact**
- Large-but-allowed frames can freeze the UI. Timeouts don’t preempt synchronous CPU work.
- Apply-lag and cursor correctness are compromised because the UI is no longer responsive.

**Fix direction**
- Move decode/decompress into a Web Worker so it’s cancellable; terminate worker on timeout.
- If worker is too heavy, reduce payload sizes aggressively for UI consumption.

**Acceptance**
- A max-sized frame does not freeze the UI; decode work is off-thread.

---

### M-999 (P3): Client-side apply lag is only console-logged, not exported as telemetry
**Category:** Metrics/Observability Gap
**Where:** `observability-ui/src/App.tsx`

**Evidence**
- Apply lag is tracked and printed to console:
  - `observability-ui/src/App.tsx:1546`
  - `observability-ui/src/App.tsx:1557`

**Impact**
- Operators cannot alert on “UI is behind” without opening devtools.

**Fix direction**
- Expose apply-lag/backlog in the UI health panel.
- Optionally report to server via control frames to export as Prometheus gauge/counter.

**Acceptance**
- Apply lag is visible in UI without devtools; optional metric exists for alerting.

---

### M-1000 (P3): DashStreamCallback does not count Metrics send failures in `dashstream_telemetry_send_failures_total`
**Category:** Producer/Telemetry
**Where:** `crates/dashflow/src/dashstream_callback/mod.rs`

**Evidence**
- Send failure counter exists for other message types, but metrics path only logs:
  - `crates/dashflow/src/dashstream_callback/mod.rs:2035`

**Impact**
- Metrics pipeline failures can be invisible to alerting, even though dashboards depend on them.

**Fix direction**
- Increment `dashstream_telemetry_send_failures_total{message_type="metrics"}` in both sync and async metrics send paths.

**Acceptance**
- A failed metrics send increments the same send-failure counter family as events/diffs.

---

### M-1001 (P2): websocket-server EventBatch indexing assumes single thread_id; thread replay can miss data if batches contain multiple threads
**Category:** Resume/Replay Correctness
**Where:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`

**Evidence**
- For EventBatch, server picks first `thread_id` and a single max sequence:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2295`
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:2303`

**Impact**
- If EventBatch ever includes multiple threads, replay buffer’s per-thread indexing becomes incomplete and thread-mode replay can silently miss events.

**Fix direction**
- Detect multi-thread batches and count them (metric + warning).
- Preferably index per-thread max sequence for EventBatch (similar to UI `sequencesByThread` concept).

**Acceptance**
- Mixed-thread batches are detected and do not silently degrade thread replay correctness.

---

### M-1002 (P2): websocket-server handler still uses hot-path `println!` logging in per-client send loop
**Category:** Server/Performance
**Where:** `crates/dashflow-observability/src/bin/websocket_server/handlers.rs`

**Evidence**
- Logs for first 3 and every 10th message per client:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:522`
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:524`

**Impact**
- With high throughput and multiple clients, this can be a major bottleneck and distort streaming metrics (self-inflicted latency/drops).

**Fix direction**
- Replace with `tracing` and make the rate far less frequent (or time-based).

**Acceptance**
- Handler send path does not emit logs at 10% message frequency in normal operation.

---

## Worker Priority (recommended order)

1. ~~**M-997 (P1)**~~: ✅ FIXED #1784 - On schema mismatch, stop applying messages (not just gating cursor commits).
2. ~~**M-994 (P1)**~~: ✅ FIXED #1784 - Fix old-data error suppression (offset-based or explicit mode).
3. ~~**M-995/M-996 (P2)**~~: ✅ FIXED #1786 - Switch payloads to `Bytes`/`Arc<[u8]>` and remove full-payload cloning.
4. ~~**M-998 (P2)**~~: ✅ FIXED #1787 - Move decode/decompress off UI main thread (Web Worker with timeout).
5. ~~**M-1001/M-1002 (P2)**~~: ✅ FIXED #1785 - Batch thread invariants + remove hot-path println logging.
6. ~~**M-993**~~/~~M-999~~/~~M-1000~~ (P3): ✅ ALL FIXED - M-993 #1784, M-1000 #1785, M-999 #1787.

**AUDIT COMPLETE:** All 10 issues fixed across commits #1784, #1785, #1786, #1787.
