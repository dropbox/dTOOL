# DashStream Graph State + Streaming Telemetry Re-Audit (v19) — 2025-12-23

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

> **⚠️ STALE FILE REFERENCES:** This audit references `dashstream_callback.rs` as a single file. The file was split into `dashstream_callback/` directory (Dec 2024) with: `mod.rs` (118,680 bytes), `tests.rs` (67,261 bytes). Line numbers are historical and do not match current code.

This is an "AGAIN" pass extending:
- `audits/AUDIT_dashstream_graph_state_streaming_telemetry_reaudit_2025-12-23_v18.md`

Scope: DashStream streaming telemetry (DashStreamCallback → Kafka → websocket-server → observability-ui), with emphasis on correctness, bugs, and configuration/operability.

---

## Executive Summary (skeptical)

v18 added a large set of new issues (M-677..M-696) and fixed two P0 cursor regressions (M-687, M-688). This v19 pass intentionally looks for “second-order” correctness failures and measurement gaps that still make dashboards or replay semantics unreliable.

This pass found **10 more concrete gaps** (M-697..M-706). The highest-risk class is **numeric correctness** (offsets/sequences > 2^53) and **replay paging semantics** (multi-partition behavior vs per-partition limits). The next highest class is “fake metrics”: metric names exist, but no code ever sets them.

---

## New v19 Findings (10 more actionable gaps)

### M-697 (P2): `dashstream_queue_depth` / `dashstream_batch_size` / `dashstream_consumer_lag` are effectively fake
**Problem**
- The metric names exist in `crates/dashflow-streaming/src/metrics_constants.rs`, but there is no code that sets these gauges anywhere.
- This leads to dashboards/alerts that either flatline or are missing entirely.

Evidence:
- Constants exist: `crates/dashflow-streaming/src/metrics_constants.rs`
- No usage sites: `rg -n "dashstream_queue_depth|dashstream_batch_size|dashstream_consumer_lag" crates/dashflow-streaming/src` returns only constants/tests.

Fix direction:
- Producer (`DashStreamCallback`): set `dashstream_batch_size` to configured batch size; export a real `dashstream_queue_depth` gauge based on `mpsc` queue length (track enqueue/dequeue counters to derive depth).
- Consumer(s): either implement `dashstream_consumer_lag` or remove the constants/docs to avoid fake metrics.

Acceptance:
- `/metrics` contains these gauges and they change under load/config changes.

---

### M-698 (P1): Replay paging logic mixes per-partition limits with global page semantics
**Problem**
- Redis fetch is limited per partition (`ZRANGEBYSCORE ... LIMIT 0 1000`), but the replay loop treats 1000 as a *global* page limit (`page_count = replay_messages.len()`).
- With multiple partitions, a single “page” can be far larger than intended (up to 1000 × partitions), causing:
  - premature hitting of `REPLAY_MAX_TOTAL`,
  - misleading `replay_complete` behavior (even if work is capped),
  - non-obvious replay latency spikes.

Evidence:
- Per-partition limit: `crates/dashflow-observability/src/bin/websocket_server.rs:849`
- Global page logic: `crates/dashflow-observability/src/bin/websocket_server.rs:4154`

Fix direction:
- Make paging explicit:
  - return per-partition “truncated” indicators from replay buffer reads, or
  - enforce a true global page limit by interleaving partitions and stopping at N total items,
  - and only send `replay_complete` when truly complete (or include `capped:true`).

Acceptance:
- In a 10-partition topic, a replay “page” never sends more than the configured global cap, and the client gets an explicit “capped vs complete” signal.

---

### M-699 (P2): Missing producer-side metrics for “state diff degraded mode”
**Problem**
Several correctness-degrading paths only log, with no metrics:
- patch serialization fails → fallback to full state,
- full state serialization fails → silent data loss (empty bytes),
- checkpoint serialization/compression failures → resync primitive silently absent.

Evidence:
- Only logs for diff serialization fallback: `crates/dashflow/src/dashstream_callback.rs:1460`
- Full-state serialization failure logs “telemetry data loss”: `crates/dashflow/src/dashstream_callback.rs:1479`
- Checkpoint serialization warnings: `crates/dashflow/src/dashstream_callback.rs:1531`

Fix direction:
- Add counters (component-scoped) for:
  - `dashstream_state_diff_fallback_full_state_total`
  - `dashstream_state_full_state_serialize_failures_total`
  - `dashstream_checkpoint_serialize_failures_total`
  - `dashstream_checkpoint_compression_failures_total`

Acceptance:
- Under induced failures (invalid patch encoding / oversized state), counters increment and operators can alert on degraded mode.

---

### M-700 (P2): Redis replay reads are N+1 GETs (can trigger replay timeouts)
**Problem**
- Replay reads fetch N keys from a ZSET and then do a `GET` per key, sequentially.
- Under load or large pages, this pattern is slow and increases the odds of replay timing out / disconnecting clients.

Evidence:
- `fetch_from_redis_for_partition()` uses `zrangebyscore_limit` then loops `conn.get(...)`: `crates/dashflow-observability/src/bin/websocket_server.rs:837`

Fix direction:
- Use pipelining / `MGET` for message keys returned by the ZSET page.
- Also add replay metrics: latency histogram for Redis reads, page sizes, and timeouts.

Acceptance:
- Replay of 1000 messages remains within the replay timeout budget on a moderately loaded Redis.

---

### M-701 (P2): Replay buffer retention is hardcoded (misconfiguration hazard)
**Problem**
- Redis retention and buffer sizes are fixed constants (e.g., `REDIS_MAX_SEQUENCES=10_000`, memory=1000), not environment-configurable.
- This makes correctness fragile: long disconnects or high-throughput topics will exceed retention and induce silent gaps.

Evidence:
- `REDIS_MAX_SEQUENCES` constant: `crates/dashflow-observability/src/bin/websocket_server.rs:460`
- Memory size hardcoded on init: `crates/dashflow-observability/src/bin/websocket_server.rs:2933`

Fix direction:
- Add env vars for max sequences and memory buffer size (with sane bounds).
- Document the tradeoffs and add alerts when approaching retention capacity.

Acceptance:
- Operators can tune retention without code changes and can detect when retention is insufficient.

---

### M-702 (P3): Connection rate limiting trusts `x-forwarded-for` without trusted proxy config
**Problem**
- Client IP extraction uses `x-forwarded-for` if present.
- Without a trusted-proxy allowlist, any client can spoof `x-forwarded-for`, bypass rate limits, or grief other clients by forcing collisions.

Evidence:
- `extract_client_ip(...)`: `crates/dashflow-observability/src/bin/websocket_server.rs:190`

Fix direction:
- Only honor `x-forwarded-for` when running behind a configured trusted proxy, otherwise use `SocketAddr`.
- Alternatively, use `Forwarded` header parsing with explicit proxy trust.

Acceptance:
- Spoofed headers do not alter rate limiting in non-proxy deployments; proxy deployments can opt-in safely.

---

### M-703 (P2): First-connect replay defaults to “replay everything retained”
**Problem**
- The UI always sends `lastOffsetsByPartition` (even empty), which forces partition replay mode.
- On a first connect (no offsets yet), the server will replay from the earliest retained offsets for every known partition, which can:
  - overwhelm the UI,
  - blow bandwidth,
  - and look like a production incident (“why did the dashboard load 1h of old runs?”).

Evidence:
- UI always sends resume with partition mode: `observability-ui/src/App.tsx:685`
- Server treats presence of `lastOffsetsByPartition` as “use partition mode”: `crates/dashflow-observability/src/bin/websocket_server.rs:4136`

Fix direction:
- Add explicit resume strategy: `{"type":"resume","mode":"partition","from":"latest|earliest|cursor"}`
- Default UI to `from:"latest"` unless user opts in to historical catch-up.

Acceptance:
- First load shows only live/new data by default; “catch up history” is an explicit user action.

---

### M-704 (P1): Patch apply failures do not mark runs corrupted or trigger resync
**Problem**
- If JSON Patch apply fails, the UI logs and continues, leaving state silently wrong until a future hash mismatch (if any).
- This breaks the “trust the UI state” contract and makes corruption detection non-actionable.

Evidence:
- Patch apply failure is only logged: `observability-ui/src/hooks/useRunStateStore.ts:723`

Fix direction:
- On patch apply failure:
  - mark store corrupted immediately with details (seq, error),
  - stop applying subsequent diffs unless a checkpoint/full snapshot arrives,
  - integrate with checkpoint-based resync (M-696).

Acceptance:
- Induce a bad patch: UI immediately flags corruption and recovers after the next checkpoint/full snapshot.

---

### M-705 (P2): Replay metrics still don’t cover “client-side apply lag”
**Problem**
- Server can measure “messages replayed”, but not whether clients are *keeping up applying them*.
- Without apply-lag telemetry, operators can’t distinguish “network slow” vs “UI CPU bound” vs “replay too big”.

Fix direction:
- UI emits periodic “apply progress” pings (e.g., last applied cursor) so server can export a gauge:
  - `websocket_client_apply_lag_messages` or `websocket_client_apply_lag_seconds`.

Acceptance:
- In a slow browser, dashboards show apply lag rising instead of incorrectly blaming Kafka/Redis.

---

### M-706 (P2): No explicit “replay reset” / “topic reset” protocol
**Problem**
- When Kafka topics are recreated or offsets reset, per-partition offsets can go backwards.
- The UI currently accepts backwards offsets as “stream reset” but there is no explicit protocol message explaining the reset, so operators can’t distinguish resets from bugs.

Evidence:
- Backwards offset acceptance in UI: `observability-ui/src/App.tsx:799`

Fix direction:
- Server emits explicit reset control message when it observes offset regression or topic epoch change:
  - `{"type":"cursor_reset","partition":p,"reason":"topic_recreated|retention|manual_reset",...}`
- UI clears local offsets for that namespace and shows a warning banner.

Acceptance:
- Topic recreation produces an explicit reset event; UI does not silently reuse stale offsets.

---

## Worker Priority (v19 additive)

1) P1 correctness: M-698, M-704.
2) P2 measurement truthfulness: M-697, M-699, M-705.
3) P2 operability: M-700, M-701, M-703, M-706.
4) P3 security: M-702.
