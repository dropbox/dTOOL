# DashStream Graph State + Streaming Telemetry Re-Audit (v17) — 2025-12-24

This is an “AGAIN” pass extending:
- `audits/AUDIT_dashstream_graph_state_streaming_telemetry_reaudit_2025-12-24_v16.md`

Scope: DashStream (producer → Kafka → websocket-server → observability-ui) correctness, replay/resume semantics, and measurement/telemetry trustworthiness.

---

## Executive Summary (skeptical / correctness-first)

v16 established that DashStream resume/replay had structural correctness failures (global cursor misuse, synthetic seq collisions, EventBatch handling). Those core problems were addressed in `#1579` and `#1583`, and `#1580` eliminated a real UI state-hash race.

This v17 pass finds a **new high-impact follow-up correctness bug** in the `#1583` M-674 “partition offset resume” implementation:

- The UI persisted Kafka offsets when it received a `cursor` text frame, **before** it successfully decoded/applied the corresponding binary protobuf message. A reload/crash mid-replay could therefore permanently skip messages on the next resume.

v17 also finds “still not truly global catch-up” gaps:
- Resume-by-partition still fails to catch up partitions the UI has never seen (no partition discovery / “missing partition means -1” contract).
- Replay is effectively **paged at 1000** items per partition (Redis `ZRANGEBYSCORE ... LIMIT 0 1000`), but the resume protocol is one-shot; a long disconnect can silently remain incomplete.

This report includes worker-grade fix directions + acceptance criteria to close these gaps and to harden streaming metrics so dashboards and alerts are *actually true*.

---

## What v17 Verified as Truly Fixed (with concrete evidence)

### ✅ M-670 (P1): UI state_hash verification race
`observability-ui/src/hooks/useRunStateStore.ts` now deep-clones state before async hash computation (`stateSnapshotForHash = deepCloneJson(store.latestState)`), eliminating false mismatches under concurrent StateDiff apply.

### ✅ M-666 (P1): Producer ordering (Events vs StateDiff)
`crates/dashflow/src/dashstream_callback.rs` now routes Events and StateDiff through a single ordered message queue (`message_sender` / `BatchMessage`), eliminating scheduler-driven reorder races (FIXED `#1584`).

### ✅ M-674 (P1): Global catch-up cursor exists (partition+offset)
`crates/dashflow-observability/src/bin/websocket_server.rs` now:
- attaches `(partition, offset)` to every forwarded message,
- replays by `lastOffsetsByPartition` via `ReplayBuffer::get_messages_after_by_partition`,
- emits sideband JSON `{"type":"cursor","partition":...,"offset":...}` text frames before each binary frame.

However, v17 marks M-674 as “needs follow-ups” because of protocol/consumer gaps described below (paging + partition discovery + cursor commit semantics).

---

## New v17 Finding: Cursor Persistence Was Wrong (causes permanent skip)

### **M-675 (P0/P1): UI must commit Kafka offsets only after decode+apply**

**Problem**
- The websocket-server sends cursor metadata in a text frame before each binary protobuf frame.
- The UI stored offsets on receipt of the cursor frame, while binary decode/apply was asynchronous (`Blob.arrayBuffer().then(...)`) and not serialized.
- A reload/crash mid-replay (or decode failure) can result in `localStorage` offsets moving ahead of applied state → on reconnect, the UI requests resume from too-far-forward offsets → **permanent loss**.

**Evidence**
- `observability-ui/src/App.tsx` previously persisted offsets inside the `data.type === "cursor"` branch.
- Binary decode/apply was Promise-based without ordering guarantees.

**Fix (implemented in this iteration)**
- `observability-ui/src/App.tsx` now:
  - queues cursor frames (`pendingKafkaCursorsRef`),
  - serializes binary decode/apply (`binaryProcessingChainRef`),
  - commits offsets only after successful decode+apply for the paired binary message,
  - drops the newest cursor on overflow (preserves FIFO alignment),
  - uses a connection epoch (`wsEpochRef`) so old async work can’t corrupt new sessions.

**Acceptance**
1) Start UI, produce >5k messages quickly, and disconnect/reconnect repeatedly.
2) Kill/reload browser tab mid-replay.
3) On restart, resume must not skip messages that were not applied before the reload (no “cursor ran ahead” permanent gap).

**Status**
- ✅ Implemented in `#1585` (see `observability-ui/src/App.tsx`).

---

## Still Open / Not Actually Resolved (correctness gaps)

### 🔴 M-676 (P1): Partition-offset resume still incomplete (paging + partitions)

**Issue A: Missing partitions are not replayed**
- The UI only sends offsets for partitions it has seen. If a partition becomes active while the UI is offline *and the UI never observed it before*, the UI won’t request replay for it.

**Fix direction**
- Server should treat a missing partition key as `-1` (replay from earliest retained in replay buffer) *or* send a partition inventory/hello message so the UI can include every partition in its resume payload.

**Issue B: Replay is paged but resume is one-shot**
- Redis fetch uses a hard limit of 1000 keys per partition per resume.
- There is no “continue replay” pagination protocol (no cursor token, no repeated resume loop).

**Fix direction**
- Implement replay paging:
  - server: loop pages until “up-to-date” or until a safe cap (e.g. `REPLAY_MAX_TOTAL_MESSAGES`) and send a `{"type":"replay_complete","lastOffsetsByPartition":...}` summary.
  - UI: if `replay_complete` not received, re-issue resume (or request next page) until complete.

**Acceptance**
1) Disconnect for long enough to exceed 1000 missed messages per partition.
2) Reconnect and confirm UI reaches *true* catch-up (server sends replay_complete; UI is up to date).

### 🔴 Metrics and measurement quality gaps (dashboards can lie)

**M-647 (P1): Metric contract violations**
Same metric name, different buckets/labels across components, which corrupts `histogram_quantile` and alert semantics:
- `dashstream_redis_connection_errors_total` is `CounterVec{operation}` in `crates/dashflow-streaming/src/rate_limiter.rs` but a plain `IntCounter` in `crates/dashflow-observability/src/bin/websocket_server.rs`.
- `dashstream_redis_operation_latency_ms` buckets differ between those same sites.

**Fix direction**
- Centralize metric definitions or rename metrics to be component-scoped (`dashstream_rate_limiter_redis_*` vs `websocket_replay_redis_*`).

**Missing: Replay correctness observability**
Resume/replay is correctness-critical, but there is no authoritative instrumentation for:
- resume requests,
- replayed messages per resume,
- replay paging progress,
- cursor-gap detection with offsets/ranges.

**Fix direction**
- Add websocket-server metrics:
  - `websocket_resume_requests_total{mode="partition|thread|legacy"}`
  - `websocket_replay_messages_total{mode="partition|thread|legacy"}`
  - `websocket_replay_gaps_total{mode="partition|thread"}`
  - `websocket_replay_page_total` (if paging added)

**M-649 (P2): dashstream_message_loss_rate is process-local and misleading**
`crates/dashflow-streaming/src/metrics_monitor.rs` computes “loss rate” from whatever counters happen to exist in the current process, which is not end-to-end in distributed deployments.

Fix direction:
- Deprecate or redesign as consumer-observed sequence gap rates (per topic/partition) rather than “sent vs received” across processes.

---

## Worker Priority List (do these in order)

### P0 / P1: correctness before dashboards
1) **Close M-676**:
   - partition discovery / missing partition semantics,
   - replay paging protocol + completion signal,
   - UI resume loop until complete.
2) **Fix M-647 metric contract violations**: rename or centralize definitions (do not ship dashboards with ambiguous metrics).

### P2: hardening
4) **M-671 checkpoint/resync**: define and implement a real recovery contract (UI uses checkpoints; server can serve checkpoints).
5) **M-673 redaction**: ensure state diffs don’t leak secrets into Kafka/Redis/UI logs.
6) **M-672 size limits**: enforce end-to-end payload caps so “big state” cannot break streaming or OOM consumers.

---

## Minimal Skeptical Repro Scenarios (repeat until you can’t break it)

1) **Crash mid-replay**
   - Start replay (disconnect UI then reconnect), reload tab mid-backlog.
   - Confirm no permanent skip on next resume.

2) **Long offline, >1000 missed**
   - Ensure disconnect creates >1000 missed messages per partition.
   - Confirm replay pages to completion (no silent partial catch-up).

3) **Multi-partition topic**
   - Produce runs across multiple partitions.
   - Disconnect UI, start a run on a partition the UI hasn’t seen recently.
   - Reconnect: UI must still catch up those runs.
