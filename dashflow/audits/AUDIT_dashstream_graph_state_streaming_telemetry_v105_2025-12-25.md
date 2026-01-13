# DashFlow v105 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-25
**Auditor:** Worker #1799
**Scope:** DashStream UI streaming/telemetry (`observability-ui/`), websocket-server forward/replay/resume and health/metrics (`crates/dashflow-observability/.../websocket_server/`), producer-side streaming callback telemetry (`crates/dashflow/src/dashstream_callback/`)
**Prior:** v104 (M-1023..M-1032) — ✅ ALL FIXED (#1795-#1797), v103 still has 5 P2 OPEN (M-1018..M-1022).

This audit is intentionally skeptical. It looks for *new* correctness/telemetry/config gaps that remain even after the recent flurry of fixes.

---

## Status (v105)

**0 P0 | 3 P1 | 7 P2** open

| ID | Priority | Category | Summary |
|----|----------|----------|---------|
| **M-1037** | **P1** | UI/Correctness | `useRunStateStore` applies event-derived state mutations without out-of-order guard; can corrupt `latestState` |
| **M-1038** | **P1** | UI/Resource Safety | Unbounded `JSON.parse` of event attribute `state_json` / `initial_state_json`; can freeze/OOM browser |
| **M-1034** | **P1** | Server/Liveness | Control-plane `socket.send(...).await` calls are unbounded (replay timeout error, backpressure disconnect, ping/pong); can wedge handler tasks |
| **M-1033** | P2 | Server/Config + Metrics Gap | `SEND_TIMEOUT_SECS=5` is hardcoded and send failures/timeouts have no Prometheus telemetry |
| **M-1035** | P2 | Metrics/Correctness | `/health` uses `tx.receiver_count()` for connected clients but `/metrics` exports atomic `connected_clients`; mismatch/confusion |
| **M-1036** | P2 | UI/Telemetry Gap | Decode worker returns `null` on failure without structured error; UI can’t classify root cause or record reasoned metrics |
| **M-1039** | P2 | UI/Telemetry Hygiene | UI stores `metrics.tags` verbatim in run timeline; unbounded size + possible PII/secret leakage |
| **M-1040** | P2 | Producer Observability Gap | `DashStreamCallback` lacks self-observability metrics (queue depth/in-flight sends/send latency); drops become “surprise” |
| **M-1041** | P2 | Health/Telemetry Truth | `/health` alert uses lifetime `dropped_messages > 10`, so a long-lived server can be “WARNING” forever |
| **M-1042** | P2 | Metrics/Gap | Prometheus lacks gauges for `last_kafka_message_ago_seconds` / `last_infrastructure_error_ago_seconds` used by `/health` |

---

## Executive Summary

Even after v104, there are still correctness and telemetry blind spots in the “last mile”:

- **UI graph state can still corrupt** if event-based state mutations arrive out-of-order (state-diff path is guarded; event-derived state mutations are not).
- **Browser resource safety is incomplete**: event attribute `state_json` parsing has no byte cap (fullState snapshots do).
- **Server liveness has a remaining wedge class**: several non-broadcast sends are still unbounded (especially the replay timeout error send).
- **Operator truth gaps remain**: `/health` warnings are based on lifetime totals; Prometheus can’t scrape “time since last Kafka msg/infra error”.

---

## New Issues (M-1033 to M-1042)

### M-1033 (P2): WebSocket send timeout is hardcoded and has no metrics
**Where**
- `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:36-60`

**Evidence**
- Hardcoded timeout: `const SEND_TIMEOUT_SECS: u64 = 5;`
- `send_with_timeout` returns `"send_failed"` / `"send_timeout"` but only logs a warn on timeout; no counter/histogram is updated.

**Why it matters**
- 5 seconds is not universally correct (LAN vs WAN vs mobile; replay vs control traffic).
- When sends start timing out, operators have no scrapeable signal beyond logs.

**Fix direction**
- Make timeout configurable (e.g., `WEBSOCKET_SEND_TIMEOUT_SECS`, clamp to `[1..60]`).
- Add Prometheus counter: `websocket_ws_send_failures_total{reason="send_failed|send_timeout", path="broadcast|control|resume"}` (keep label cardinality bounded).
- Consider a histogram for send duration for *control* sends only (avoid high-volume overhead).

**Acceptance**
- Timeout is tunable via env var and reflected in `/version`.
- Timeouts and send failures show up in Prometheus without log scraping.

---

### M-1034 (P1): Control-plane sends are unbounded and can wedge handler tasks
**Where**
- Replay timeout path sends an error without `send_with_timeout`:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:476-496`
- Backpressure disconnect notification uses unbounded send:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:696-709`
- Ping/pong responses use unbounded send:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:455-462`

**Evidence**
- `let _ = socket.send(...).await;` appears in operationally critical recovery/control paths.

**Why it matters**
- This defeats the purpose of timeouts: the system detects a slow/hung client, then can hang trying to tell it about the timeout/disconnect.
- A stuck handler task leaks capacity and can distort connected-client metrics.

**Fix direction**
- Use `send_with_timeout` for all non-trivial outgoing messages (control plane and recovery plane), including replay timeout error and disconnect notifications.
- If “best effort” is desired, still bound it: `send_with_timeout(...).await.ok();` but don’t `await` unbounded.

**Acceptance**
- No `socket.send(...).await` remains in replay timeout handling or disconnect notification paths (except Close frames if intentionally best-effort with explicit justification).

---

### M-1035 (P2): `/health` connected_clients differs from `/metrics`
**Where**
- `/health` overrides connected_clients:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:111-116`
- Prometheus exports atomic connected_clients:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:534-563`

**Evidence**
- `/health` uses `state.tx.receiver_count()` while `/metrics` uses `metrics.connected_clients`.

**Why it matters**
- Operators can see contradictory values depending on which endpoint they’re looking at.
- This hides failure modes where the handler task didn’t decrement the atomic but the receiver dropped (or vice versa).

**Fix direction**
- Pick one source of truth and export it consistently:
  - Preferred: track atomic accurately and remove override, or
  - Export `receiver_count` as its own gauge and name it clearly.

**Acceptance**
- `/health.metrics.connected_clients` matches `websocket_connected_clients` within a scrape interval (or both are documented as intentionally different, with distinct metric names).

---

### M-1036 (P2): Decode worker returns `null` without structured error details
**Where**
- Decode response supports `error?: string` but is never populated:
  - `observability-ui/src/workers/decode.worker.ts:92-97`
  - `observability-ui/src/workers/decode.worker.ts:432-440`
- Worker returns `null` for decode failure:
  - `observability-ui/src/workers/decode.worker.ts:413-416`

**Why it matters**
- The UI can’t distinguish “payload too large”, “zstd decompress failed”, “protobuf decode failed”, “schema mismatch”, etc.
- Telemetry/alerts become coarse, and operators lose the ability to attribute failures to configuration drift vs corruption.

**Fix direction**
- Make decode return `{ ok: false, reason: "decompress_failed|decompressed_too_large|protobuf_decode_failed|..." }` instead of `null`.
- Plumb that through `DecodeWorkerPool` and surface:
  - a counter by reason (in UI state, and/or a server-side DLQ metric if relevant),
  - a reconnect decision (some reasons should force `cursor_reset` guidance).

**Acceptance**
- UI logs and state show a reasoned decode failure classification; not just “decode failed”.

---

### M-1037 (P1): Event-derived state mutations lack out-of-order protection
**Where**
- `EVENT_TYPE_GRAPH_START` sets `store.latestState = extractState(...)` without comparing to `lastAppliedSeq`:
  - `observability-ui/src/hooks/useRunStateStore.ts:761-804`
- `NODE_START` / `NODE_END` merge `newState` into `latestState` without an OOO check:
  - `observability-ui/src/hooks/useRunStateStore.ts:818-858`

**Evidence**
- StateDiff and checkpoints have explicit out-of-order guards (M-787), but the event-based state updates do not.

**Why it matters**
- If an event arrives late (or is replayed late) after a newer state snapshot/patch was applied, `latestState` can be overwritten with older state.
- This is silent corruption: it can look “healthy” while being wrong.

**Fix direction**
- Apply the same OOO rule used for StateDiff/checkpoints to *any* state mutation path:
  - If `seq < lastAppliedSeq`, skip mutation, set `needsResync=true`, `corrupted=true`.
- Decide policy: either “events never mutate authoritative state” (preferred) or “events mutate state only when they are monotonic”.

**Acceptance**
- There is a single monotonic guard used for every path that mutates `latestState`.

---

### M-1038 (P1): Unbounded JSON.parse for event attribute state payloads
**Where**
- `extractState` parses `initial_state_json` / `state_json` / `state` with no size limit:
  - `observability-ui/src/hooks/useRunStateStore.ts:461-475`

**Why it matters**
- A single large attribute string can freeze the UI thread or blow memory.
- This bypasses the `maxFullStateSizeBytes` protection that exists for `StateDiff.fullState`.

**Fix direction**
- Enforce a byte cap before parsing (e.g., reuse `maxFullStateSizeBytes` or add `maxEventStateJsonBytes`).
- On rejection: flag run as `needsResync`, and record an explicit timeline event like `{skipped:"state_json_too_large"}`.

**Acceptance**
- Large `state_json` does not block the main thread; it is rejected with explicit operator-visible reason.

---

### M-1039 (P2): UI stores `metrics.tags` verbatim (unbounded and potentially sensitive)
**Where**
- `metrics` timeline event stores `tags: metrics.tags || {}`:
  - `observability-ui/src/hooks/useRunStateStore.ts:1523-1532`

**Why it matters**
- Tags can be arbitrarily large and may contain PII/secrets depending on producers.
- Even with `maxEventsPerRun`, this can balloon memory usage and increase render costs.

**Fix direction**
- Store only bounded metadata:
  - `tagKeys`, `tagCount`, and maybe a sanitized allowlist (e.g., `tenant_id`, `model`, `graph_name`).
- If full tags are needed, cap total serialized size and redact known-sensitive keys.

**Acceptance**
- Tags can’t cause large memory growth; sensitive keys are not rendered/stored by default.

---

### M-1040 (P2): `DashStreamCallback` lacks self-observability metrics (queue depth/in-flight)
**Where**
- Only drop and send-failure counters exist:
  - `crates/dashflow/src/dashstream_callback/mod.rs:110-169`

**Why it matters**
- Drops are a lagging indicator; you want early warning (queue depth rising, semaphore saturated, batch flush delay).

**Fix direction**
- Add bounded self-metrics:
  - Prometheus gauges: in-flight permits used, queue length (approx), pending_tasks length.
  - Optional periodic DashStream `Metrics` message (scope `dashstream_callback`) if Prometheus is not available.
- Add a histogram for `producer.send_*` latency if feasible.

**Acceptance**
- Operators can see “approaching saturation” before drops occur.

---

### M-1041 (P2): `/health` warns forever after 11 total drops (lifetime counter misuse)
**Where**
- `if metrics.dropped_messages > 10 { ... }`:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:172-178`

**Why it matters**
- A server that has been up for days can show WARNING forever from one transient incident.
- Operators lose the ability to interpret current health vs historical health.

**Fix direction**
- Make alerts based on **rates or recency**, not lifetime totals:
  - track `last_drop_at` timestamp and/or a short rolling window counter,
  - include a “drops in last 2m” value in health response.

**Acceptance**
- Health warnings reflect recent behavior, not “ever since boot”.

---

### M-1042 (P2): Prometheus missing “time since last Kafka msg / infra error” gauges
**Where**
- `/health` relies on `last_kafka_message_ago_seconds` and `last_infrastructure_error_ago_seconds`, but `/metrics` does not export them:
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:300-323` (snapshot fields)
  - `crates/dashflow-observability/src/bin/websocket_server/state.rs:529-610` (collector; missing these)

**Why it matters**
- These are core alerting signals; without Prometheus gauges, operators must poll `/health` or parse logs.

**Fix direction**
- Export gauges:
  - `websocket_last_kafka_message_age_seconds`
  - `websocket_last_infrastructure_error_age_seconds`
- Decide sentinel for “never”: `-1` or `NaN` (document).

**Acceptance**
- Prometheus scrape contains these ages, enabling alerts like `> 60` seconds stale.

---

## Worker Priority (v105)

Fix in this order (highest correctness risk first):
1. **M-1037 (P1):** unify monotonic state-mutation guard across event/state_diff/checkpoint.
2. **M-1038 (P1):** add size cap for event attribute state JSON before parsing.
3. **M-1034 (P1):** remove unbounded control-plane sends; use `send_with_timeout`.
4. **M-1042 + M-1041 (P2):** make health/Prometheus reflect *recent* behavior and expose recency gauges.
5. **M-1036 + M-1033 + M-1035 + M-1039 + M-1040 (P2):** improve diagnosability and telemetry hygiene.
