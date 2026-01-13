# DashStream Graph State + Streaming Telemetry Re-Audit (v18) — 2025-12-23

This is an “AGAIN” pass extending:
- `audits/AUDIT_dashstream_graph_state_streaming_telemetry_reaudit_2025-12-24_v17.md`

Scope: DashStream (producer → Kafka → websocket-server → observability-ui) correctness, replay/resume semantics, and streaming metrics/measurement trustworthiness.

---

## Executive Summary (skeptical)

Recent work fixed many structural correctness problems (resume cursor mismatch, ordering, batching, replay paging, checkpoint emission, redaction, size limits). This pass focuses on “still can’t break it?” and on measurement correctness.

This v18 pass found **multiple new and/or regressed correctness hazards** that are easy to miss because the system “usually works”:

1) UI offset persistence can still jump ahead of applied state via `replay_complete`, reintroducing the “permanent skip” failure mode (M-675-class bug, but in a different code path).
2) UI cursor pairing was vulnerable to misalignment under backlog pressure because cursor frames were buffered separately from the binary frames.
3) The system is still not safe for **large Kafka offsets / long-running sequences** because Redis and the UI use floating/JS-number representations that lose integer precision beyond 2^53.
4) Some metrics that appear “implemented” are actually dead/unobserved, and some drop paths aren’t counted → dashboards/alerts can lie.

This document enumerates **10 new concrete gaps** (M-687..M-696) with fix directions and acceptance criteria.

---

## Verified Fixes (skeptical re-check)

- M-647 is truly fixed: component-scoped Redis metrics exist in both rate limiter and websocket replay (`dashstream_rate_limiter_redis_*`, `dashstream_websocket_redis_*`).
- M-676 exists and is materially improved: websocket-server does paging + partition discovery and emits `replay_complete`.

However: several follow-ups below show M-676’s current implementation still has correctness/operability hazards (KEYS usage, “complete” semantics under cap, numeric precision).

---

## New v18 Findings (10+ actionable gaps)

### M-687 (P0): UI replay_complete must not advance offsets ahead of apply
**Problem**
- `observability-ui/src/App.tsx` handled `type === "replay_complete"` by persisting `lastOffsetsByPartition` immediately.
- `replay_complete` is emitted after the server finished *sending* replay frames; it does not guarantee the client has decoded/applied them.
- If the tab reloads/crashes while backlog is still being applied, persisted offsets can jump ahead → next resume permanently skips messages.

**Fix direction**
- Treat `replay_complete` offsets as debug-only, or persist them only after the binary processing queue is fully drained.
- Prefer “commit only after apply” as the single source of truth (cursor+binary pairing).

**Acceptance**
- Kill/reload the tab during a large replay; on reconnect, it must not permanently skip unapplied messages.

---

### M-688 (P0): Cursor pairing must be message-pair based, not cursor-queue based
**Problem**
- The UI previously buffered cursors separately from binary messages, then shifted one cursor during async processing.
- Any cursor drop/backpressure “safety” (or any ordering anomaly) risks cursor↔binary misalignment → wrong partition offsets persisted.

**Fix direction**
- Pair cursor↔binary at receipt time: store cursor as “next cursor”, capture it when the next binary frame arrives, then enqueue the pair for async decode/apply.
- If a binary arrives without a cursor, log and skip offset commit (never guess).

**Acceptance**
- Under deliberate decode slowdowns / large replays, offsets persisted must always monotonically match the actually processed message stream.

---

### M-689 (P1): Redis replay indexes use floating scores (precision loss > 2^53)
**Problem**
- websocket-server stores Kafka offsets and per-thread sequences in Redis ZSET scores as `f64` (`offset as f64`, `seq as f64`).
- Redis sorted-set scores are IEEE doubles; integers beyond 2^53 lose precision.
- Kafka offsets can exceed 2^53 in long-lived/high-throughput topics; at that point replay ordering/range queries become incorrect.

**Fix direction**
- Stop using ZSET scores as the integer offset/sequence.
- Options:
  - Use a Redis STREAM (native integer IDs), or
  - Use ZSET score as timestamp and keep offset as part of key, then range by lex (or maintain a separate ordered list), or
  - Use `ZRANGEBYLEX` with fixed-width zero-padded offsets in the member string (no float).

**Acceptance**
- Simulate offsets near/above 2^53 in unit tests; replay ordering and resume range queries must remain correct.

---

### M-690 (P1): Cursor protocol uses JSON numbers (precision loss in browser)
**Problem**
- websocket-server sends `{"type":"cursor","offset": <i64>}` as a JSON number.
- JS numbers lose integer precision beyond 2^53; `localStorage` persists these imprecise values.
- Resume payload returns those values to the server, which parses them as i64 → undefined/incorrect replay positions on large offsets.

**Fix direction**
- Send offsets and sequences as strings in control messages (`"offset":"123456..."`), and parse as `i64`/`u64` server-side.
- UI stores offsets as strings (or BigInt), and only converts to number for display (never for correctness).

**Acceptance**
- With offsets > 2^53, resume must still replay exactly-once (idempotent UI) and not drift.

---

### M-691 (P2): Resume cursor storage is not namespaced (topic/cluster collisions)
**Problem**
- UI persists offsets under a single key (`dashstream_lastOffsetsByPartition_v1`) without scoping by Kafka topic/cluster.
- websocket-server replay keys are also only scoped by `redis_key_prefix` and do not prove they match the active Kafka topic/cluster.
- Switching topics/clusters (or pointing the same UI origin to multiple websocket servers) can cause resumes to start from incorrect cursors.

**Fix direction**
- Include a stable `resume_namespace` in the websocket hello/version payload (topic, cluster id, consumer group, etc.).
- Store offsets under `dashstream_lastOffsetsByPartition_v1:{namespace}`.
- Scope Redis replay prefix by topic/cluster (or store topic metadata and reject mismatches).

**Acceptance**
- Switching websocket-server topic/cluster must not reuse offsets from prior environments.

---

### M-692 (P1): replay_complete semantics are wrong when replay is capped
**Problem**
- websocket-server caps replay at `REPLAY_MAX_TOTAL` (10,000) for safety but still sends `type="replay_complete"`.
- This can incorrectly imply “up to date” even when the client is still behind.

**Fix direction**
- If capped, send `type="replay_capped"` (or `replay_complete{capped:true}`) and instruct the UI to re-issue resume (or to show a “cannot catch up” error).
- Alternatively: do not send replay_complete unless truly complete.

**Acceptance**
- When backlog > cap, UI must not believe it is fully caught up; behavior must be explicit and observable.

---

### M-693 (P1): UI drops sequences above MAX_SAFE_INTEGER (long runs degrade silently)
**Problem**
- `observability-ui/src/proto/dashstream.ts` converts u64 fields to JS numbers with overflow checks; values > 2^53 are dropped.
- When sequence drops, the UI falls back to synthetic negative sequences, breaking ordering, seek, and corruption detection semantics on long runs.

**Fix direction**
- Introduce BigInt-backed sequence handling in UI store:
  - keep `sequence` as string/BigInt internally,
  - adjust maps/sorting, cursor storage, and display.

**Acceptance**
- With sequences > 2^53, time-travel, dedup, and resume must remain correct.

---

### M-694 (P2): Drop metrics are incomplete (StateDiff/Checkpoint queue drops not counted)
**Problem**
- `dashflow/src/dashstream_callback.rs` increments `dashstream_telemetry_dropped_total` for some drop paths, but not for:
  - `BatchMessage::StateDiff` queue failures
  - `BatchMessage::Checkpoint` queue failures
- This makes “drop rate” appear healthy even when state diffs/checkpoints are being dropped (worst-case correctness impact).

**Fix direction**
- Add a `CounterVec` labeled by `{message_type, reason}` and increment on *all* drop paths.
- Keep `dashstream_telemetry_dropped_total` as a backward-compatible sum if desired.

**Acceptance**
- Under forced queue saturation, drop metrics must reflect drops for Events, StateDiff, and Checkpoint distinctly.

---

### M-695 (P2): dashstream_ws_retry_count is defined but never observed
**Problem**
- websocket-server creates `dashstream_ws_retry_count` histogram but never calls `observe(...)`.
- This is “fake observability”: dashboards can show a flatline and mislead on incident response.

**Fix direction**
- Either remove the metric or wire it to actual retry loops (Kafka connect retries, DLQ send retries, Redis retry, etc.).

**Acceptance**
- Metric changes under induced retry conditions; otherwise removed.

---

### M-696 (P1): Checkpoints emitted but not applied/used in UI resync
**Problem**
- Producer emits Checkpoint messages and StateDiff includes `base_checkpoint_id`, but `useRunStateStore` does not handle `decoded.type === 'checkpoint'`.
- UI still cannot resync after drops/patch failures; “checkpoint” exists only as a protobuf variant, not a recovery primitive.

**Fix direction**
- Implement checkpoint handling:
  - decode checkpoint `state` bytes (including `compression_info`),
  - install snapshot into the run store as an authoritative baseline,
  - use `base_checkpoint_id` to reject patches that don’t chain correctly and request/await checkpoint resync.

**Acceptance**
- Induce drop of StateDiff; UI must recover via checkpoint (no permanent corruption).

---

## Worker Priority (recommended order)

1) P0: Fix M-687/M-688 regressions (never persist offsets ahead of apply; cursor↔binary pairing).
2) P1: Fix numeric correctness end-to-end (M-689/M-690/M-693): no f64/JS-number for correctness-critical offsets/sequences.
3) P1: Remove Redis KEYS and fix replay_complete semantics under cap (M-691/M-692).
4) P1: Make checkpoints real in the UI (M-696).
5) P2: Fix measurement correctness gaps (M-694/M-695) so dashboards reflect reality.

---

## Minimal Skeptical Repro Scenarios

1) **Crash mid-replay**
   - Disconnect UI, reconnect, get a replay backlog; reload the tab mid-apply.
   - Confirm resume does not skip unapplied messages.

2) **Huge offsets / long runs**
   - Force offsets/sequences beyond 2^53 in synthetic tests.
   - Confirm replay ordering and resume remain correct end-to-end.

3) **Redis load**
   - Populate Redis with large keyspace; ensure partition discovery does not block.
