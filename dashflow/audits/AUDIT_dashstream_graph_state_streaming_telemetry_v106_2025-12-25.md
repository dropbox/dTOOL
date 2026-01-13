# DashFlow v106 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1811
**Scope:** DashStream UI telemetry (`observability-ui/`), websocket-server control/replay/DLQ/metrics (`crates/dashflow-observability/.../websocket_server/`), producer-side DashStream callback (`crates/dashflow/src/dashstream_callback/`)
**Prior:** v105 (M-1033..M-1042) — ✅ ALL FIXED (#1803-#1805), v103 P2 apply-lag window metrics ✅ FIXED (#1806). Recent module audits: v106-109 in `audits/` are largely CLEAN and/or P4 notes.

This audit looks for **additional** correctness/config/telemetry gaps that remain after the v105 fix wave, focusing on “edge correctness” and “operator truth”.

---

## Status (v106)

**0 P0 | 3 P1 | 7 P2** open

| ID | Priority | Category | Summary |
|----|----------|----------|---------|
| **M-1064** | **P1** | Server/Security + Reliability | DLQ includes full payload base64 on decode errors; can leak secrets and exceed Kafka message limits |
| **M-1065** | **P1** | Producer/Security | Redaction only applies to state diffs; event attributes and other telemetry can still leak secrets |
| **M-1061** | **P1** | Server/Security | WebSocket control JSON (`Message::Text`) has no max size guard; `serde_json::from_str` can DoS |
| **M-1058** | P2 | UI/Correctness | localStorage eviction uses highest offsets/sequences, not true recency; can evict active low-offset partitions/threads |
| **M-1059** | P2 | UI/Performance → Correctness | `useRunStateStore` dedupe uses `findIndex` per event (O(n²)); can wedge UI and trigger false reconnects |
| **M-1060** | P2 | UI/Correctness | Node durations derived from header timestamps; should prefer producer `duration_us` and clamp negative durations |
| **M-1062** | P2 | Server/Telemetry | Invalid client JSON is silently ignored; needs metric + policy (warn/disconnect) |
| **M-1063** | P2 | Server/Liveness | `cursor_reset_complete` can return offsets for all partitions (huge JSON) without cap/pagination |
| **M-1066** | P2 | UI/Metrics Correctness | Apply-lag / rate computations assume `Date.now()` “monotonic”; clock changes can skew metrics |
| **M-1067** | P2 | UI/Resource Safety | Event attributes stored unbounded (strings/objects); need size caps + redaction/preview strategy |

---

## New Issues (M-1058 to M-1067)

### M-1058 (P2): localStorage eviction uses “highest offset/seq”, not true recency
**Where**
- `observability-ui/src/App.tsx:139` (`evictOldestEntries`)
- Used for partition offsets and per-thread sequences persistence:
  - `observability-ui/src/App.tsx:1355` (offsets)
  - `observability-ui/src/App.tsx:1631` (sequences)

**Evidence**
- The helper claims “LRU eviction” but sorts by value (offset/seq) descending:
  - `sort((a,b) => compareFn(entries[b], entries[a]))`

**Why it matters**
- “Highest offset” ≠ “most recently updated” across partitions/threads:
  - A hot new partition/thread with low offsets can be evicted in favor of an old partition/thread with very high offsets.
- This is correctness-affecting: eviction chooses which partitions/threads can be resumed after reload.

**Fix direction**
- Track per-key last-updated timestamp and evict by **recency**, not offset magnitude.
- Keep current “protect partition 0” behavior, but generalize to protect “recently updated keys”.

**Acceptance**
- Eviction keeps the most recently updated keys, regardless of absolute offset/sequence magnitude.

---

### M-1059 (P2): `useRunStateStore` dedupe is O(n²) and can wedge UI
**Where**
- `observability-ui/src/hooks/useRunStateStore.ts:559` (`store.events.findIndex(...)`)

**Evidence**
- For every event, dedupe scans the entire `store.events` array.

**Why it matters**
- With `maxEventsPerRun=10000`, this becomes expensive and can cause:
  - backlog growth → forced reconnects (backpressure path),
  - misleading apply-lag/window metrics (UI slowed by its own bookkeeping).

**Fix direction**
- Maintain a bounded `Set` or `Map` of dedupe keys per run:
  - insert key when event added,
  - evict keys when events are trimmed,
  - avoid linear scans.

**Acceptance**
- Adding events remains ~O(log n) (insertion) rather than O(n) per message.

---

### M-1060 (P2): Node durations use timestamps; prefer `duration_us` and guard negatives
**Where**
- UI derives duration from timestamps:
  - `observability-ui/src/hooks/useRunStateStore.ts:888` (NodeEnd)
- Producer already has a duration field:
  - `crates/dashflow/src/dashstream_callback/mod.rs:1292` (duration_us computed)

**Why it matters**
- Header timestamps are wall-clock and can reflect cross-host clock skew.
- Even without skew, “duration = end_ts - start_ts” is less reliable than producer-calculated duration.

**Fix direction**
- If attribute `duration_us` (or equivalent) exists, use it as the source of truth for `NodeState.durationMs`.
- Clamp negative durations to 0 and record a “clock_skew_duration” diagnostic flag.

**Acceptance**
- Node durations are stable under clock skew and do not go negative.

---

### M-1061 (P1): WebSocket control JSON has no max size guard (DoS risk)
**Where**
- `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:451-456`

**Evidence**
- The server does `serde_json::from_str::<serde_json::Value>(&text)` with no `text.len()` cap.

**Why it matters**
- A single huge Text frame can allocate large memory and burn CPU, even though later parsing applies bounds to some fields.
- This is pre-validation DoS.

**Fix direction**
- Add a hard cap (e.g., `WEBSOCKET_MAX_CONTROL_BYTES`, default 1MB) and reject/close on oversize.
- Add a Prometheus counter for rejected control messages.

**Acceptance**
- Oversized control frames are rejected before JSON parsing.

---

### M-1062 (P2): Invalid JSON from clients is silently ignored (no metrics, no policy)
**Where**
- `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:451-455`

**Evidence**
- Invalid JSON falls through silently because parsing is inside `if let Ok(msg) = ...`.

**Why it matters**
- Operators can’t diagnose client/server protocol drift.
- Attackers can spam invalid JSON without any visibility.

**Fix direction**
- Add a policy:
  - increment `websocket_control_parse_failures_total`,
  - after N failures in a window, disconnect.

**Acceptance**
- Invalid JSON is visible via metric, and repeated failures trigger disconnect.

---

### M-1063 (P2): `cursor_reset_complete` can generate huge JSON responses
**Where**
- `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:499-545`

**Evidence**
- Returns offsets for “all known partitions” as a JSON map.

**Why it matters**
- On topics with many partitions, this can exceed:
  - WebSocket message size limits,
  - client decode limits,
  - send timeout budgets (even if bounded, it can thrash).

**Fix direction**
- Cap/ paginate:
  - return up to N partitions + `truncated=true`,
  - add an endpoint to fetch offsets in pages.

**Acceptance**
- cursor_reset is reliable even on high-partition topics; payload size bounded.

---

### M-1064 (P1): DLQ includes full payload base64 on decode errors (secret leak + size risk)
**Where**
- `crates/dashflow-observability/src/bin/websocket_server/main.rs:3051-3088`

**Evidence**
- `let original_payload_base64 = BASE64.encode(&binary_data);`
- Inserted into DLQ JSON as `"original_payload_base64": original_payload_base64`.

**Why it matters**
- Payload can contain sensitive data (prompts, tool args, user content).
- Base64 expands size; DLQ records can exceed Kafka limits and fail to publish, losing forensic signal.

**Fix direction**
- Default to truncation:
  - include SHA256 + first/last N bytes, and only include full base64 behind an explicit env var (disabled by default).
- Apply the same approach used for oversized payloads (M-1024 style) to *all* decode errors.

**Acceptance**
- DLQ payload size is bounded and secrets are not shipped by default.

---

### M-1065 (P1): Redaction only covers state diffs; event attributes can still leak secrets
**Where**
- Redaction helpers are scoped to state diff serialization:
  - `crates/dashflow/src/dashstream_callback/mod.rs:393-535` (`redact_json_value`)

**Why it matters**
- Graph events, tool execution telemetry, and other attributes can contain secrets (API keys in args, bearer tokens in error messages, etc.).
- The UI stores and renders event attributes; Kafka retains them.

**Fix direction**
- Apply redaction to *all* telemetry string fields (event attributes and error/tool fields), or:
  - switch to a strict allowlist of attribute keys that can be emitted to DashStream.

**Acceptance**
- A secret string present in event attributes is redacted before producing to Kafka.

---

### M-1066 (P2): UI metric timing assumes `Date.now()` is monotonic
**Where**
- Example comment indicates “monotonic timestamp (Date.now())”:
  - `observability-ui/src/App.tsx:104`
- `Date.now()` used in apply-lag and other timing paths:
  - `observability-ui/src/App.tsx:1703` (sampleTimestamp), plus other health timing uses.

**Why it matters**
- `Date.now()` can jump backward/forward due to NTP/clock changes.
- Windowed metrics and health “stale” logic can be skewed.

**Fix direction**
- Use `performance.now()` for durations/rates (monotonic), and `Date.now()` only for wall-clock display.

**Acceptance**
- Apply-lag and rate computations remain correct under system clock adjustments.

---

### M-1067 (P2): Event attributes are stored unbounded; need caps and redaction/preview
**Where**
- UI stores full attributes for events:
  - `observability-ui/src/hooks/useRunStateStore.ts:750-759`

**Why it matters**
- Even with event count caps, a few huge attributes (prompt dumps, stack traces, tool payloads) can OOM the tab.
- This also increases risk of secret exfiltration into operator UIs.

**Fix direction**
- Implement attribute size caps and summarization:
  - store `attributeBytes`, `attributeKeys`, and a truncated preview of large strings,
  - optionally drop known-large keys by default (allowlist).

**Acceptance**
- A single large attribute cannot freeze the UI; sensitive keys are not stored/rendered by default.

---

## Worker Priority (v106)

Fix order:
1. **M-1064 (P1)**: DLQ bounded payload + secret-safe defaults.
2. **M-1065 (P1)**: extend redaction to event/tool telemetry (or enforce allowlist).
3. **M-1061 (P1)**: max control frame size guard + metrics.
4. **M-1058/M-1059/M-1067 (P2)**: UI correctness/perf/resource safety (eviction, dedupe, attribute caps).
5. **M-1060/M-1066 (P2)**: timing/duration correctness.
6. **M-1062/M-1063 (P2)**: control-plane robustness (bad JSON policy, cursor_reset paging).
