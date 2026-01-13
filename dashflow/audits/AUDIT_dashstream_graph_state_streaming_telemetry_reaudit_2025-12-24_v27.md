# v27 Skeptical Source Code Audit - DashStream Graph State / Streaming / Telemetry

> **⚠️ STALE FILE REFERENCES:** This audit references `websocket_server.rs` as a single file. The file was split into `websocket_server/` directory (Dec 2024) with: `main.rs`, `handlers.rs`, `state.rs`, `replay_buffer.rs`, `config.rs`, `client_ip.rs`, `dashstream.rs`, `kafka_util.rs`. Line numbers are historical and do not match current code.

**Date:** 2025-12-24
**Auditor:** Worker #1664 (v27 deep source code audit)
**Prior:** v26 (M-775), v25 (M-774), v24 (M-762..M-773), v23 (M-749..M-760), v22 (M-739..M-748), v21 (M-719..M-738), v20 (M-707..M-716)
**Scope:** `observability-ui/`, `crates/dashflow-observability/`, WebSocket/Kafka streaming pipeline, graph-state correctness + telemetry/metrics
**Method:** Deep skeptical source code analysis — actual source, not docs

---

## Executive Summary

This v27 pass targets **streaming correctness** and **measurement/telemetry reliability** in the DashStream graph-state pipeline. I found **10 NEW issues** (M-776 to M-785), including **1 P1**.

Top themes:
- The UI can keep applying patches even when it already knows state is untrustworthy (`needsResync`).
- Hash verification and metrics can become misleading when patches are skipped or verification cannot run.
- A few protocol/telemetry details still leak ambiguity (gap metrics semantics, legacy label mismatch, missing “replay complete” in thread mode).

---

## New Issues (M-776 to M-785)

### M-776 (P1): UI continues applying StateDiff patches even when `needsResync=true`
**Category:** Graph State/Correctness
**Severity:** P1

**Problem:** `useRunStateStore` sets `store.needsResync=true` when a StateDiff references a missing/invalid `base_checkpoint_id`, but patch application continues anyway. Once we know the base is missing/invalid, applying diffs is no longer well-defined and can drive the UI further away from truth.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:807-839` (sets `needsResync`) and `observability-ui/src/hooks/useRunStateStore.ts:912-931` (still applies patch)

**Impact:** Silent state corruption drift: the UI may “keep going” on a wrong base and present incorrect state while only showing a small `[resync needed]` hint.

**Fix direction:**
- Treat `needsResync` like `patchApplyFailed`: skip patch application when `store.needsResync` is true.
- Keep recording timeline events, but do not mutate `store.latestState` until a full snapshot arrives.
- Consider surfacing a stronger UI banner when patch application is being skipped due to resync requirement.

**Acceptance criteria:**
- When `store.needsResync=true`, incoming `stateDiff.operations` does not mutate `latestState` and logs a single warning explaining “skipping patches until fullState”.

---

### M-777 (P2): Hash verification runs even when state was not updated (patch skipped / snapshot parse failed)
**Category:** Telemetry/Correctness
**Severity:** P2

**Problem:** Hash verification runs whenever `stateDiff.stateHash` is present, even if:
- Patch apply was skipped due to `patchApplyFailed` (and `latestState` is stale), or
- FullState snapshot parse failed (and `latestState` is stale).

**File:** `observability-ui/src/hooks/useRunStateStore.ts:952-1005` (hash verify has no gating)

**Impact:**
- Guaranteed hash mismatches and inflated `hashMismatchCount` on runs where we intentionally did not apply the state update.
- Misleading “corruption” telemetry: it can look like state_hash is wrong, when the UI simply didn’t apply the diff/snapshot.

**Fix direction:**
- Gate hash verification on “we successfully applied this update”:
  - Snapshot path: only verify if snapshot parsed successfully.
  - Patch path: only verify if patch was applied (not skipped) and `needsResync` is false.
- If verification is skipped due to patch/snapshot failure, log once per run (similar to `hashVerificationSkipWarned`).

**Acceptance criteria:**
- If patch apply is skipped, the UI does not compare hashes for that diff and does not increment mismatch counters.

---

### M-778 (P2): Checkpoint JSON decode still uses non-fatal TextDecoder
**Category:** Data Integrity
**Severity:** P2

**Problem:** FullState snapshots and jsonPatch values use `TextDecoder('utf-8', { fatal: true })`, but checkpoint decoding still uses `new TextDecoder()` (non-fatal). Invalid UTF-8 will be silently replaced, possibly producing valid-but-corrupted JSON.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:1075`

**Impact:** Silent corruption in checkpoint-derived state, which then becomes a base for subsequent diffs and can suppress/complicate resync detection.

**Fix direction:**
- Use `new TextDecoder('utf-8', { fatal: true })` for checkpoint state decode.
- On decode error, treat it equivalently to JSON.parse failure (`stateValid=false`, trigger resync on reference).

**Acceptance criteria:**
- Invalid UTF-8 in checkpoint state produces a hard error path (no replacement characters).

---

### M-779 (P2): UI only handles WebSocket binary frames as Blob; ArrayBuffer frames are ignored
**Category:** Protocol/Browser Compatibility
**Severity:** P2

**Problem:** `App.tsx` only processes binary messages if `event.data instanceof Blob`. Some browsers/environments can provide `ArrayBuffer` (especially if `ws.binaryType = 'arraybuffer'` is set or defaults differ).

**File:** `observability-ui/src/App.tsx:1157-1160` (`else if (event.data instanceof Blob)`)

**Impact:** In those environments, streaming silently stops (binary frames are ignored), breaking graph-state updates and telemetry.

**Fix direction:**
- Explicitly set `ws.binaryType = 'arraybuffer'` (recommended) and handle `event.data` as `ArrayBuffer`.
- Alternatively, support both:
  - `event.data instanceof Blob` → `await blob.arrayBuffer()`
  - `event.data instanceof ArrayBuffer` → use directly

**Acceptance criteria:**
- UI correctly decodes and applies a binary frame delivered as `ArrayBuffer`.

---

### M-780 (P2): Server stale-cursor detection ignores requested_offset == 0 (can miss staleness)
**Category:** Replay/Correctness
**Severity:** P2

**Problem:** `check_for_stale_cursors` skips offsets `<= 0` as “requesting from beginning”. But in Kafka, the earliest retained offset for a partition can be > 0; requesting offset 0 should be considered stale in that case.

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:1833-1837`

**Impact:** Server can fail to notify clients they are stale when they request offset 0 and retention has advanced beyond 0. UI may think it has complete history when it doesn’t.

**Fix direction:**
- Change the guard to only skip negative sentinel offsets (e.g., `-1` for “earliest”), not `0`.
- Suggested: `if requested_offset < 0 { continue; }`.

**Acceptance criteria:**
- If `requested_offset=0` and `oldest_retained=500`, server emits `cursor_stale` and UI marks resync needed.

---

### M-781 (P3): `websocket_replay_messages_total` claims “legacy” label value but no code emits it
**Category:** Metrics Contract/Docs
**Severity:** P3

**Problem:** Metric help/labels suggest modes `(partition, thread, legacy)`, but replay increment sites only emit `"partition"` and `"thread"`. No `"legacy"` series is ever produced.

**Files:**
- Definition: `crates/dashflow-observability/src/bin/websocket_server.rs:3795-3801`
- Increments: `crates/dashflow-observability/src/bin/websocket_server.rs:5600-5603`, `crates/dashflow-observability/src/bin/websocket_server.rs:5803-5806`

**Impact:** Grafana dashboards and operators may expect a legacy mode series that never appears; confusing during incident debugging.

**Fix direction:**
- Either implement legacy replay accounting (if legacy path exists), or remove “legacy” from help text and comments to match reality.

**Acceptance criteria:**
- Metric docs and produced label values match; no phantom label values.

---

### M-782 (P2): replay gap metric loses “how many messages were missed” (counts only gap events)
**Category:** Metrics Semantics
**Severity:** P2

**Problem:** `websocket_replay_gaps_total` increments once per gap event, but does not record `gap_size` (missed message count). The UI receives `count`, but Prometheus cannot alert on actual missing volume.

**File:** `crates/dashflow-observability/src/bin/websocket_server.rs:5519-5523` and `crates/dashflow-observability/src/bin/websocket_server.rs:5736-5740`

**Impact:** Operators can’t distinguish “1 message missed” vs “100k missed” based on metrics; alerting is weak.

**Fix direction:**
- Keep existing `websocket_replay_gaps_total` as “gap events”.
- Add `websocket_replay_gap_messages_total{mode}` and `inc_by(gap_size as u64)`.

**Acceptance criteria:**
- Prometheus can graph both “gap events” and “gap messages” per mode.

---

### M-783 (P2): FullState snapshot has no size limit (browser OOM / long GC pauses)
**Category:** DoS/Robustness
**Severity:** P2

**Problem:** Checkpoints enforce `maxCheckpointStateSizeBytes`, but `stateDiff.fullState` snapshots are decoded/parsed without a size guard.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:848-911`

**Impact:** A single oversized fullState snapshot can freeze the UI tab or crash it, turning telemetry into a DoS vector.

**Fix direction:**
- Add `maxFullStateSizeBytes` (or `maxSnapshotStateSizeBytes`) to merged config with a conservative default (e.g., 10–25MB).
- If exceeded, set `corrupted/needsResync` and require user/operator recovery (cursor_reset) or wait for a smaller snapshot.

**Acceptance criteria:**
- Oversized fullState snapshot is rejected with a clear warning and resync flag, without freezing the UI.

---

### M-784 (P3): Hash verification errors (e.g., no WebCrypto) warn every diff; should disable verification per-run after first failure
**Category:** Telemetry/Noise
**Severity:** P3

**Problem:** `computeStateHash()` throws when WebCrypto is unavailable; the caller logs a warning on every diff. This can spam logs and waste cycles while still leaving the run in an ambiguous “verification attempted” state.

**Files:**
- Throw: `observability-ui/src/utils/stateHash.ts:71-74`
- Warn loop: `observability-ui/src/hooks/useRunStateStore.ts:998-1005`

**Fix direction:**
- Add per-run flag like `hashVerificationDisabledReason`:
  - On first compute failure, set flag and warn once.
  - Skip future verification attempts for that run (similar to `hashVerificationSkipWarned`).

**Acceptance criteria:**
- In a non-WebCrypto environment, each run logs at most one warning and then stops retrying hash verification.

---

### M-785 (P2): UI accepts backward-moving Kafka offsets without forcing recovery/reset
**Category:** Resume/Correctness
**Severity:** P2

**Problem:** `commitKafkaCursor` updates persisted offsets whenever `BigInt(offset) !== BigInt(prev)` and explicitly allows offsets to move backwards (“treat it as a stream reset”). But it does not force a UI state reset or mark resync, so state can become inconsistent if offsets go backwards for any reason other than an intentional cursor_reset.

**File:** `observability-ui/src/App.tsx:1168-1173`

**Impact:** If offsets ever regress (topic recreation, partition rewinds, server bug), the UI can silently continue and persist a cursor that will cause replay duplicates and state drift.

**Fix direction:**
- If offset decreases (BigInt(offset) < BigInt(prev)):
  - mark active runs `needsResync` and surface a visible warning, and/or
  - automatically trigger `cursor_reset` flow (or prompt user) and clear stored offsets.
- Only allow backwards offsets as part of an explicit reset protocol.

**Acceptance criteria:**
- Backward offset movement triggers an explicit recovery path (resync flag + UX prompt) instead of silent acceptance.

---

## Worker Priority (recommended order)

1. **M-776 (P1)** stop patch apply when `needsResync=true` (prevents known-wrong state drift)
2. **M-777 (P2)** gate hash verification on successful apply (fixes misleading corruption telemetry)
3. **M-783 (P2)** add fullState snapshot size limit (prevents browser DoS)
4. **M-779 (P2)** handle ArrayBuffer frames / set `binaryType` (browser robustness)
5. **M-780 (P2)** treat requested_offset=0 as stale-check eligible
6. **M-782 (P2)** add gap-messages metric (improves alerting)
7. **M-778 (P2)** fatal TextDecoder for checkpoint state
8. **M-785 (P2)** backward offset should trigger recovery, not silent accept
9. **M-784 (P3)** disable per-run hash verification after first compute failure
10. **M-781 (P3)** remove/implement “legacy” label value in replay_messages_total
