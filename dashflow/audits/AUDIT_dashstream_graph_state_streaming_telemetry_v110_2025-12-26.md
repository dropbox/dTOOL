# DashFlow v110 Skeptical Audit: DashStream Streaming Metrics/Telemetry + Graph State (AGAIN)

**Date:** 2025-12-26
**Auditor:** Worker #1856
**Scope:** DashStream UI decode + graph-state reconstruction (`observability-ui/`), websocket-server streaming metrics/telemetry (`crates/dashflow-observability/.../websocket_server/`), and sequencing/measurement correctness (`crates/dashflow-streaming/consumer`).
**Prior:** v108/v109 — ✅ marked COMPLETE (see `WORKER_DIRECTIVE.md`, `ROADMAP_CURRENT.md`).

This re-audit focuses on **new** gaps after the v108/v109 fix wave: unsafe “debug/metadata” paths, precision/ordering drift, and sequence-loss metrics that can be **misleading on restarts**.

---

## Status (v110)

**0 P0 | 0 P1 | 0 P2 | 0 P3** remaining (10 fixed)

| ID | Priority | Category | Summary | Status |
|----|----------|----------|---------|--------|
| **M-1108** | **P1** | UI/Resource Safety + Privacy | Quarantine stores full `DecodedMessage` unbounded (1000 max by count only); can OOM and retain sensitive payloads | ✅ FIXED (#1857) |
| **M-1109** | **P1** | UI/Correctness | `event_batch` sequence parsing uses `Long.toNumber()` (precision loss) → wrong ordering/cursor/state updates for large sequences | ✅ FIXED (#1857) |
| **M-1110** | **P1** | UI/Resource Safety | `bytesToHex()` converts unbounded `Uint8Array` (messageId/checkpointId/stateHash) to huge strings; messageId is also used as dedupe key | ✅ FIXED (#1857) |
| **M-1114** | **P1** | Metrics/Correctness | Sequence gaps are misreported after websocket-server restart: new thread baseline expects seq=1 and flags "gap" for midstream first-seen messages | ✅ FIXED (#1857) |
| **M-1111** | P2 | UI/Resource Safety | GraphStart reads `schema_id`/`graph_name` from *raw* attributes before bounding; oversized strings can still bloat memory/UI | ✅ FIXED (#1858) |
| **M-1112** | P2 | UI/Performance | `event_batch` is processed synchronously in a tight loop; large batches can freeze the main thread despite worker decode | ✅ FIXED (#1859) |
| **M-1113** | P2 | UI/Resource Safety | JSON Patch `test` failure builds error via `JSON.stringify` of actual/expected; can allocate huge strings in failure mode | ✅ FIXED (#1858) |
| **M-1115** | P2 | Metrics/Signal Quality | websocket-server increments `dashstream_sequence_gaps_total` by 1 regardless of `gap_size`; missing severity signal (add histogram / counter-by-gap_size) | ✅ FIXED (#1858) |
| **M-1116** | P3 | Server/Operability | websocket-server `thread_id` is cloned/logged with no max length; huge `thread_id` can DoS logs/memory | ✅ FIXED (#1858) |
| **M-1117** | P3 | Server/Operability | resume parsing collects `parse_errors` strings containing unbounded user-provided text; can amplify logs/memory on malformed inputs | ✅ FIXED (#1858) |

---

## New Issues (M-1108 to M-1117)

### M-1108 (P1): Quarantine stores full unbounded `DecodedMessage` (OOM + sensitive retention)

**Where**
- QuarantinedMessage stores full `DecodedMessage`:
  - `observability-ui/src/hooks/useRunStateStore.ts:365-372`
- Missing `threadId` path pushes full decoded message into quarantine:
  - `observability-ui/src/hooks/useRunStateStore.ts:748-768`

**Why it matters**
- This is an “error path”, but it can still be hit at high volume (bad producer, schema mismatch, corrupt headers).
- It retains **raw decoded payloads** (including attributes/tags/state/checkpoints) without any of the usual bounding/truncation logic.
- `MAX_QUARANTINE` caps count (1000), not total memory; 1000 × large messages can still OOM a tab.

**Fix direction**
- Store a **bounded quarantine summary** instead of the full decoded message:
  - `type`, `timestamp`, `partition`, `offset`, `schemaVersion`, `sizeBytes`, `messageIdPrefix`, `reason`.
  - For any attributes/tags: store bounded keys only (reuse `boundAttributes`/safe allowlists).
- Add a byte-budget cap for quarantine (e.g., 1–5MB) and evict oldest by size.

**Acceptance**
- Quarantine cannot retain unbounded decoded payloads; bounded memory even under repeated malformed messages.
- Quarantine view remains useful for debugging (has offsets + type + sizes) without secrets.

---

### M-1109 (P1): `event_batch` sequences can lose precision (`Long.toNumber`) → wrong ordering/cursor

**Where**
- `coerceU64ToString` uses `toNumber()` for Long-like objects:
  - `observability-ui/src/hooks/useRunStateStore.ts:67-93`
- `event_batch` uses this helper for inner sequence extraction:
  - `observability-ui/src/hooks/useRunStateStore.ts:1368-1379`

**Why it matters**
- `toNumber()` loses precision for values > 2^53. DashStream sequences are u64 and can exceed that in long-running streams.
- If the inner sequence becomes wrong/undefined, the UI may treat it as synthetic (unordered), skipping authoritative state mutation (GraphStart/StateDiff guards), causing incorrect state.

**Fix direction**
- Delete or deprecate this local helper and use the canonical lossless converter:
  - import `coerceU64ToStr` from `observability-ui/src/proto/dashstream.ts` (it prefers Long.toString()).
- Add a strict “digits-only” validation before `BigInt` comparisons.

**Acceptance**
- Inner sequences from `event_batch` remain exact for u64 values > 2^53.
- Live cursor and ordering remain correct after long runtimes.

---

### M-1110 (P1): `bytesToHex()` can allocate huge strings from untrusted bytes fields

**Where**
- `bytesToHex` converts any `Uint8Array` to full hex string:
  - `observability-ui/src/hooks/useRunStateStore.ts:114-118`
- Used for messageId and checkpoint IDs (also feeds dedupe keys):
  - `observability-ui/src/hooks/useRunStateStore.ts:791-803` (event messageId)

**Why it matters**
- `header.messageId` and other bytes fields are untrusted and size-unbounded within the message.
- Hex conversion is O(n) allocation and creates a string ~2× the bytes length; a large bytes field can freeze/OOM the UI.
- messageId-as-dedupe-key means a single large messageId also bloats the `dedupeKeys` set.

**Fix direction**
- Enforce a hard max byte length for hex conversion (e.g., 32 bytes for UUID-like fields):
  - if longer: store `{len, sha256, prefixHex}` rather than full hex.
- Make dedupe key use bounded identity (`sha256` or first N bytes + len) when messageId is oversized.

**Acceptance**
- UI never creates multi-megabyte hex strings from bytes fields.
- Deduplication remains stable and bounded.

---

### M-1111 (P2): GraphStart reads `schema_id`/`graph_name` from raw attributes before bounding

**Where**
- Raw attribute reads happen before `boundAttributes` is the stored representation:
  - `observability-ui/src/hooks/useRunStateStore.ts:808-815` (`schema_id`, `graph_name`)

**Why it matters**
- `extractSchema` now has size caps, but other GraphStart metadata strings can still be huge and become the run label/name in UI.
- This is a common path (GraphStart), so it’s a high-leverage memory/UX risk.

**Fix direction**
- Read GraphStart metadata from bounded attributes:
  - use `storedEvent.attributes` or apply `boundAttributes` before calling `getStringAttribute`.
- Add max-length caps for `graph_name`, `schema_id`, and `threadId` (UI-side).

**Acceptance**
- `graph_name`/`schema_id` cannot exceed a bounded size in UI state/labels.

---

### M-1112 (P2): `event_batch` is processed synchronously; large batches can freeze the UI

**Where**
- `observability-ui/src/hooks/useRunStateStore.ts` — `event_batch` expansion and apply now runs via a chunked drain loop.

**Why it matters**
- Decode is off-main-thread, but *apply* is on main thread. A large batch produces a long uninterrupted JS loop (plus per-event inserts/checkpointing).
- This can cause frame drops and “tab unresponsive” even when payload sizes are within limits.

**Fix direction**
- Use chunked processing (yield between chunks) while preserving message ordering.

**Fix implemented**
- Added a message queue and a chunked drain loop that expands `event_batch` into inner events and yields via `setTimeout(0)` once the time budget is exceeded.

**Acceptance**
- Large `event_batch` inputs do not freeze the UI; processing yields between chunks.

---

### M-1113 (P2): JSON Patch `test` failure uses unbounded `JSON.stringify` in error message

**Where**
- `test` operation failure message includes full JSON.stringify for expected/actual:
  - `observability-ui/src/utils/jsonPatch.ts:531-537`

**Why it matters**
- This is a failure-mode path that can be triggered by malformed/malicious diffs.
- Building the error string itself can allocate huge strings and exacerbate the incident (OOM/log spam).

**Fix direction**
- Replace `JSON.stringify` in error message with a capped preview helper:
  - include type + length + first N chars/keys only.
- Include a “path + sizes” diagnostic, not full content.

**Acceptance**
- Patch failure errors remain bounded in size and cannot allocate huge strings.

---

### M-1114 (P1): Sequence gap metrics are misleading after websocket-server restart (baseline expects seq=1)

**Where**
- SequenceValidator initializes expected sequence for unseen thread to 1:
  - `crates/dashflow-streaming/src/consumer/mod.rs:255-256`
- websocket-server creates a fresh validator on start:
  - `crates/dashflow-observability/src/bin/websocket_server/main.rs:1727-1734`

**Why it matters**
- After restart, the first message for many active threads will be midstream (seq ≫ 1). This currently emits a “gap” warning and increments gap counters even if Kafka delivery is perfect.
- Operators will interpret spikes of `dashstream_sequence_gaps_total` as data loss when it may just be “validator cold-start”.

**Fix direction**
- Treat the first-seen sequence for a thread as a baseline:
  - on missing state, set `expected_next = sequence + 1` and return `Ok(())` with a *distinct metric* like `dashstream_sequence_tracking_initialized_total`.
- Optionally: reset baseline when Kafka consumer group rebalance/restart occurs (explicit “epoch”).

**Acceptance**
- websocket-server restart does not produce “false loss” spikes for every active thread.
- True gaps (within an active tracked sequence) are still detected and surfaced.

---

### M-1115 (P2): Sequence gap metric lacks severity (gap_size ignored)

**Where**
- gap_size is available but metric increments by 1:
  - `crates/dashflow-observability/src/bin/websocket_server/dashstream.rs:45-60`

**Why it matters**
- A gap of 1 and a gap of 10,000 both show as “1 gap”. Alerts/dashboards can’t distinguish “minor blip” vs “mass loss”.

**Fix direction**
- Add a Prometheus histogram for gap sizes (match library naming: `dashstream_sequence_gap_size`) or increment a counter by `gap_size`.
- Update Grafana panels to show both rate(gaps_total) and quantiles/avg gap size.

**Acceptance**
- Operators can see both “gap frequency” and “gap magnitude”.

---

### M-1116 (P3): websocket-server `thread_id` has no max length (log/memory DoS)

**Where**
- thread_id is cloned from header with no cap:
  - `crates/dashflow-observability/src/bin/websocket_server/dashstream.rs:35-37`
- thread_id is logged on warnings:
  - `crates/dashflow-observability/src/bin/websocket_server/dashstream.rs:51-56`

**Why it matters**
- A malicious producer can emit enormous thread_id strings that are cloned into heap allocations and emitted to logs/tracing.

**Fix direction**
- Enforce max thread_id length (e.g., 128/256) and replace oversized IDs with a hash+prefix.
- Keep full ID out of logs by default; use truncated for tracing fields.

**Acceptance**
- Oversized thread IDs cannot cause large allocations or log spam.

---

### M-1117 (P3): resume `parse_errors` can amplify unbounded attacker-controlled strings

**Where**
- parse_errors collects strings containing raw user-provided values:
  - `crates/dashflow-observability/src/bin/websocket_server/handlers.rs:853-909`

**Why it matters**
- Even with partition count caps, a single invalid offset string can be extremely long; storing it into `parse_errors` and logging can amplify memory/log volume.

**Fix direction**
- Truncate any string captured into parse_errors (e.g., 128–256 chars + length metadata).
- Log only aggregate counts + a few examples, not the full vector.

**Acceptance**
- Malformed resume requests can’t cause unbounded log/heap growth via parse_errors.

---

## Worker Priority (Recommended)

1. **P1:** M-1108, M-1109, M-1110, M-1114 (highest correctness + OOM risk)
2. **P2:** M-1111, M-1112, M-1113, M-1115 (hardening + signal quality)
3. **P3:** M-1116, M-1117 (operability robustness)
