# DashStream Graph State + Streaming Telemetry Re-Audit (v22) — 2025-12-24

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

This is an "AGAIN" pass extending:
- v21 audit section in `ROADMAP_CURRENT.md` (“v21 Skeptical Code Audit — NEW Issues (M-719 to M-738)”)
- Prior DashStream/streaming audits in `dashflow/audits/`

Scope: DashStream streaming telemetry and graph state correctness (DashStreamCallback → Kafka → websocket-server → observability-ui), with emphasis on correctness, bugs, configuration, and measurement truthfulness.

---

## Executive Summary (skeptical)

The codebase has accumulated a lot of hardening (resume namespace, cursor pairing, Redis replay buffer, backpressure disconnects, patch safety). This v22 pass focuses on:

1) **Silent corruption paths** (accepting malformed payloads as strings; hash validation that can’t be correct in JS for large ints).
2) **Protocol inconsistencies** (schema drift inside control messages).
3) **Operational recoverability gaps** (cursor_reset exists server-side but can’t be invoked from UI, and reset semantics don’t guarantee state correctness).
4) **Config hazards** (TTL values and replay caps are hard-coded / accept dangerous values).

This pass adds **10 more actionable gaps**: **M-739..M-748**.

---

## New v22 Findings (10 more actionable gaps)

### M-739 (P2): apply-lag `pendingCount` leaks on early-return paths (telemetry becomes wrong)
**Problem**
- `applyLagMetricsRef.current.pendingCount++` happens for every binary frame.
- Several early-return paths do not decrement it (decoder init failure, decoder not initialized, decode returns null).
- Result: the UI can report “pending” growth even when it’s just a decode/init failure, corrupting the apply-lag signal.

Evidence:
- Increment without guaranteed decrement: `observability-ui/src/App.tsx:1089-1113`

Fix direction:
- Wrap per-message processing in `try/finally` to guarantee decrement of `pendingCount`.
- Only record latency for successfully applied messages; keep separate counters for decode/init failures.

Acceptance:
- Force decoder init failure and decode null cases: `pendingCount` returns to baseline and does not drift upward.

---

### M-740 (P1): jsonPatch JSON decoding silently falls back to string on parse failure (state corruption)
**Problem**
- When `ValueEncoding.JSON` decode fails (invalid JSON bytes), the UI returns the raw decoded string instead of failing the patch.
- That can apply “garbage” strings into state while still committing offsets and continuing, making state correctness untrustworthy.

Evidence:
- Fallback-to-string behavior: `observability-ui/src/utils/jsonPatch.ts:327-336`

Fix direction:
- For `ValueEncoding.JSON`, **throw** on parse failure (do not coerce to string).
- Let the existing patch-failure handling quarantine state (corrupted + needsResync) instead of mutating state with an invalid value.

Acceptance:
- Inject invalid JSON bytes in a diff op: patch application fails, run is marked corrupted, and state is not mutated.

---

### M-741 (P1): state_hash verification cannot be correct in JS when state contains integers > 2^53
**Problem**
- The UI hash canonicalization treats numbers as JS `number` and serializes via `JSON.stringify`.
- If the producer state contains large integer fields (common with timestamps in ns, counters, IDs), JS may parse them imprecisely, producing a different canonical form and a false corruption signal.

Evidence:
- JS canonicalization on `typeof value === 'number'`: `observability-ui/src/utils/stateHash.ts:4-33`
- UI state comes from `JSON.parse` on snapshots/checkpoints, which already loses large integer precision.

Fix direction:
- Enforce an encoding policy: any integer fields that can exceed `2^53-1` must be represented as strings in graph state payloads (producer-side).
- Alternatively, adopt a BigInt-capable JSON parser and preserve numeric strings for hashing and patching.
- Add explicit “hash verification disabled due to unsafe numbers” mode if unsafe numbers are detected, to avoid false “corrupted” flags.

Acceptance:
- A snapshot with an integer value `> 9_007_199_254_740_991` does not produce a false hash mismatch (either by string encoding or by BigInt-safe parsing).

---

### M-742 (P2): `REDIS_MESSAGE_TTL_SECS` accepts unsafe values (0 / overflow), can break replay buffer
**Problem**
- TTL is parsed as `u64` without validation and used directly in Redis `SETEX`.
- The same TTL is cast to `i64` for `EXPIRE` inside the pipeline; extremely large values can overflow and become negative.
- TTL=0 is ambiguous: Redis `SETEX` requires a positive TTL; “0 means no TTL” is a common expectation but is not supported here.

Evidence:
- TTL parsing has no min/max clamp: `crates/dashflow-observability/src/bin/websocket_server.rs:628-634`
- TTL cast to i64: `crates/dashflow-observability/src/bin/websocket_server.rs:834-860`

Fix direction:
- Validate TTL:
  - If set to 0, either treat as “no TTL” (switch to `SET` and omit `EXPIRE`) or clamp to default with warning.
  - Clamp to a safe max (<= `i64::MAX`) before casting.
- Add a dedicated metric for replay-buffer write pipeline failures by reason (invalid TTL, connection error, etc.).

Acceptance:
- Setting `REDIS_MESSAGE_TTL_SECS=0` does not silently break replay writes; behavior is explicit and documented.

---

### M-743 (P2): replay cap/timeouts are hard-coded (cannot tune per environment)
**Problem**
- `REPLAY_TIMEOUT_SECS` and `REPLAY_MAX_TOTAL` are constants.
- Deployments vary widely (local dev vs prod), and these should be tuneable without code changes.

Evidence:
- Timeout constant: `crates/dashflow-observability/src/bin/websocket_server.rs:4881`
- Cap constant: `crates/dashflow-observability/src/bin/websocket_server.rs:5010`

Fix direction:
- Introduce env vars:
  - `WEBSOCKET_REPLAY_TIMEOUT_SECS`
  - `WEBSOCKET_REPLAY_MAX_TOTAL`
- Echo configured values in `/version` or logs for easy debugging.

Acceptance:
- Operators can tune replay timeout/cap and verify via logs or /version.

---

### M-744 (P2): `cursor_reset` updates offsets but does not guarantee UI state recovery
**Problem**
- `cursor_reset_complete` updates stored offsets, but UI run state/history remains unchanged.
- If the UI is corrupted, simply changing offsets does not restore a consistent in-memory graph state; users can remain in a “corrupted but still rendering” limbo.

Evidence:
- Server reset response only provides offsets: `crates/dashflow-observability/src/bin/websocket_server.rs:5320-5356`
- UI handler only updates offsets; no state reset: `observability-ui/src/App.tsx:987-1014`

Fix direction:
- Define reset semantics:
  - client-side: clear all run stores/events/timeline and reconnect with `from:"latest"` or `from:"cursor"` using reset offsets
  - server-side: optionally provide a “reset_epoch” token for explicit discontinuities

Acceptance:
- After `cursor_reset`, UI returns to a known good baseline (no old corrupted state shown).

---

### M-745 (P2): UI has no way to invoke `cursor_reset` (recovery mechanism is unreachable)
**Problem**
- The server supports `{"type":"cursor_reset"}` requests, but the UI never sends them.
- Operators/users cannot trigger recovery without custom tooling.

Evidence:
- UI only handles `cursor_reset_complete`: `observability-ui/src/App.tsx:987`
- No `ws.send({type:"cursor_reset"})` present in UI code.

Fix direction:
- Add a UI control (button) gated behind an “advanced” toggle when `corrupted`/`needsResync` is set.
- Add confirmation text describing the consequences (clears local state and resumes from latest/cursor).

Acceptance:
- User can trigger `cursor_reset` from the UI and observe a clean reconnection.

---

### M-746 (P2): `cursor_reset` does not invalidate Redis replay buffer (stale history can reappear)
**Problem**
- `cursor_reset` does not clear/tombstone the replay buffer keys.
- If Kafka topic is recreated or offsets reset, Redis may still contain old messages until TTL expiry, causing confusing historical replays.

Evidence:
- Reset path only reads latest offsets; no deletion/tombstone: `crates/dashflow-observability/src/bin/websocket_server.rs:5320-5356`

Fix direction:
- Add an explicit invalidation mechanism:
  - bump a `replay_epoch` namespace component in Redis/localStorage, or
  - delete the relevant Redis keyspace for this resume_namespace (admin-only mode), or
  - store a “reset watermark” that suppresses replay of older offsets.

Acceptance:
- After reset, stale pre-reset messages do not replay even if Redis keys remain.

---

### M-747 (P2): resume_namespace can collide across clusters (no Kafka cluster ID)
**Problem**
- Namespace is derived from brokers/topic/group only.
- If broker hostnames are reused (DNS/LB) across environments or clusters, cursors and replay keys can collide, producing cross-environment corruption.

Evidence:
- Namespace inputs: topic, group, brokers: `crates/dashflow-observability/src/bin/websocket_server.rs:523-544`

Fix direction:
- Include Kafka cluster ID (from Kafka metadata API) in the namespace.
- If cluster ID fetch fails, include an explicit “unknown” marker and log a warning.

Acceptance:
- Two clusters with identical broker strings no longer share resume_namespace values.

---

### M-748 (P2): `replay_complete` schema inconsistent (`replayed` vs `totalReplayed`)
**Problem**
- `from:"latest"` emits `replay_complete` with `replayed`, but other paths use `totalReplayed`.
- Schema inconsistencies make clients brittle and encourage “best-effort” parsing that masks real bugs.

Evidence:
- from=latest: `crates/dashflow-observability/src/bin/websocket_server.rs:4988-4994`
- normal replay: `crates/dashflow-observability/src/bin/websocket_server.rs:5160`

Fix direction:
- Standardize on a single field name (`totalReplayed`) for all replay_complete variants.
- Add a `schema_version` field for forward-compat changes.

Acceptance:
- UI can reliably display replay stats for `from:"latest"` and `from:"cursor"` with the same code path.

---

## Worker Priority (v22 additive)

1) **P1 correctness:** M-740, M-741
2) **P2 operability/reliability:** M-739, M-742, M-743, M-744, M-745, M-746, M-747, M-748
