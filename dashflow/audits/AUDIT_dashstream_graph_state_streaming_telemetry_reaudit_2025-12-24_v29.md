# v29 Skeptical Source Code Audit - DashStream Graph State / Streaming / Telemetry

**Date:** 2025-12-24
**Auditor:** Worker #1703 (v29 deep source code audit)
**Prior:** v28 (M-786..M-795), v27 (M-776..M-785), v26 (M-775), v25 (M-774), v24 (M-762..M-773)
**Scope:** `observability-ui/`, `crates/dashflow-observability/`, DashStream graph-state pipeline, streaming metrics/measurement/telemetry
**Method:** Deep skeptical source code analysis — actual source, not docs

---

## Executive Summary

This v29 pass focuses on **protocol/decoder correctness** (seq=0, timestamp handling), **graph-state correctness** (what we allow to mutate state), and **telemetry correctness** (stale UI states and misleading compatibility fields).

Found **10 NEW issues** (M-796 to M-805), including **1 P1**.

### Design/Implementation Critique (cross-cutting)

- **We still treat “missing sequence” as “apply anyway” for state mutations**, which is correctness-hostile: if we can’t order a state update, we shouldn’t apply it to the authoritative state (M-798).
- **Decoder semantics and store semantics are inconsistent**: we claim to support `seq=0`, but the decoder drops it for most message types, forcing synthetic seqs (M-796).
- **UI telemetry failure modes are “silent stale”**: health fetch failures keep displaying the last good numbers with no explicit “unknown/stale” state (M-803/M-804).

---

## New Issues (M-796 to M-805)

### M-796 (P2): Decoder drops seq=0 for most message types (seq=0 support still incomplete)
**Category:** Protocol/UI Correctness
**Severity:** P2

**Problem:** `safePositiveSequenceString()` rejects `sequence <= 0`, so any message with `sequence=0` (valid in parts of the protocol evolution) gets `decoded.sequence = undefined`, forcing downstream synthetic seq generation.

**File:** `observability-ui/src/proto/dashstream.ts:32-35`

**Fix direction:** Replace `safePositiveSequenceString()` with an `safeNonNegativeSequenceString()` (accept `>= 0`) and use it consistently for message headers; keep synthetic negative seqs reserved for UI-only.

**Acceptance criteria:** A header `sequence=0` decodes to `decoded.sequence === "0"` for `event`, `state_diff`, `checkpoint`, etc.

---

### M-797 (P3): Decoder uses `tsUs ? ... : now` (timestampUs=0 treated as “missing”)
**Category:** Telemetry/Correctness
**Severity:** P3

**Problem:** Decoder uses truthiness (`tsUs ? tsUs/1000 : now`) across message types. If `timestampUs` is legitimately `0` (or intentionally used as sentinel), it is treated as missing and replaced with local `now`.

**File:** `observability-ui/src/proto/dashstream.ts:372-379` (and repeated for all message types)

**Fix direction:** Use explicit undefined checks: `timestamp: tsUs !== undefined ? tsUs / 1000 : now`.

**Acceptance criteria:** When `timestampUs=0`, decoded timestamp is `0` (not replaced by `Date.now()`), and when missing it uses `now`.

---

### M-798 (P1): State mutations can be applied with synthetic seq (missing real producer sequence)
**Category:** Graph State/Correctness
**Severity:** P1

**Problem:** `processMessage()` generates synthetic negative seqs when `decoded.sequence` is missing/not-real, but state-mutating messages (`state_diff` patches/snapshots, `checkpoint` state) can still mutate `latestState` under that synthetic seq. This creates “authoritative state” derived from unordered/unverifiable updates.

**Files:**
- `observability-ui/src/hooks/useRunStateStore.ts:729-735` (synthetic seq generation)
- `observability-ui/src/hooks/useRunStateStore.ts:887-922` (out-of-order detection excludes synthetic seqs; mutation proceeds)
- `observability-ui/src/hooks/useRunStateStore.ts:1154-1204` (checkpoint can overwrite latestState even with synthetic seq)

**Fix direction:** For any **state-mutating** message, require `isRealSeq(seq)`. If not real:
- Do **not** mutate `latestState` / checkpoints
- Mark `needsResync=true` + `corrupted=true`
- Still record the event for visibility, but treat it as non-authoritative

**Acceptance criteria:** No snapshot/patch/checkpoint updates `latestState` unless `seq` is real (`>= 0`). Synthetic seq state-mutating messages trigger resync instead.

---

### M-799 (P2): RunStateStore silently ignores non-(event/state_diff/checkpoint/event_batch) messages
**Category:** Telemetry/Correctness
**Severity:** P2

**Problem:** `useRunStateStore.processMessage()` only handles `event`, `state_diff`, `event_batch`, and `checkpoint`. It silently drops `token_chunk`, `tool_execution`, `metrics`, `error`, and `execution_trace`. This creates gaps where the UI “main timeline” knows something happened, but the run-state timeline/store does not.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:738-1155`

**Fix direction:** Add a default branch that records these message types as `StoredEvent` (e.g., `eventType=UNSPECIFIED` + `attributes={type, ...}`), and advance the store cursor for real sequences.

**Acceptance criteria:** These message types show up in the per-run event stream, and live cursor advances on them when they have a real seq.

---

### M-800 (P2): Event dedupe is keyed only by seq (can drop legitimate distinct messages)
**Category:** Graph State/Correctness
**Severity:** P2

**Problem:** `addEvent()` deduplicates solely on `seq`. If the protocol ever emits multiple messages with the same seq (different types or message IDs), later messages are dropped.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:571-576`

**Fix direction:** Extend `StoredEvent` to include `messageId` (hex) and/or `kind`, and dedupe by a stable unique key (`messageId` preferred). If `messageId` is unavailable, allow multiple events per seq and use a secondary ordering key.

**Acceptance criteria:** Two distinct messages with same seq are both preserved and visible, and state reconstruction uses the correct ordered subset for state diffs.

---

### M-801 (P2): jsonPatch safeClone fallback can throw (BigInt/cycles) and crash patch apply
**Category:** Robustness
**Severity:** P2

**Problem:** `jsonPatch.ts` has its own `safeClone()` which falls back to JSON round-trip without a second try/catch. If `JSON.stringify` fails (e.g., BigInt), patch application can throw and break the UI.

**File:** `observability-ui/src/utils/jsonPatch.ts:24-30`

**Fix direction:** Mirror the hardened pattern used in `deepCloneJson`: wrap the JSON round-trip in a nested try/catch, and fall back to returning the original (with a loud warning) or a safe empty object.

**Acceptance criteria:** Patch application never crashes due to clone failures; worst-case it flags the run `needsResync/corrupted`.

---

### M-802 (P3): App per-thread sequence persistence ignores seq=0
**Category:** Resume/Correctness
**Severity:** P3

**Problem:** `lastSequencesByThreadRef` only updates when `BigInt(decoded.sequence) > 0`. If seq=0 is valid (and once M-796 is fixed, it will be observable for more message types), seq=0 will never be persisted.

**File:** `observability-ui/src/App.tsx:1380-1383`

**Fix direction:** Accept `>= 0` and use a safe parse helper (avoid raw `BigInt()` on untrusted strings).

**Acceptance criteria:** When `decoded.sequence === "0"`, it is eligible for persistence if it’s the max seen.

---

### M-803 (P2): Health fetch failures leave stale “healthy” UI state (no explicit unhealthy/stale state)
**Category:** Telemetry/UI Correctness
**Severity:** P2

**Problem:** On non-OK responses, `fetchHealth()` warns and returns without updating UI state. The last successful `health` stays rendered, so operators see stale “healthy” metrics during an outage.

**File:** `observability-ui/src/App.tsx:748-756`

**Fix direction:** Introduce explicit health-fetch state (`healthLastOkAt`, `healthError`, `healthStale=true`), and on failure set `health=null` (or mark stale) so the UI clearly shows “unavailable”.

**Acceptance criteria:** When `/health` returns 503/timeouts, UI visibly transitions to “unhealthy/unavailable” and derived metrics are not presented as current.

---

### M-804 (P2): Derived metrics remain stale when health samples stop arriving
**Category:** Measurement Correctness
**Severity:** P2

**Problem:** `messagesPerSecond` and `errorRate` only change when new health data arrives. When health polling fails or is skipped, the UI continues showing the last computed values as if current.

**Files:**
- `observability-ui/src/App.tsx:736-788` (health fetch updates)
- `observability-ui/src/App.tsx:1541-1566` (messages/sec derived from history)

**Fix direction:** Add an “unknown” state for derived metrics when `health` is stale; optionally set them to `null` and render as “—”.

**Acceptance criteria:** If no successful `/health` response for >N seconds, UI shows derived metrics as stale/unknown (not frozen “current”).

---

### M-805 (P2): Legacy `lastSequence` can be unsafe but is still sent in resume (risking wrong replay on old servers)
**Category:** Resume/Correctness
**Severity:** P2

**Problem:** UI warns when `maxSeqBigInt > MAX_SAFE_INTEGER`, but still converts to `Number` and sends `lastSequence` in the resume message. This can be wrong and can cause old servers to resume from the wrong point.

**File:** `observability-ui/src/App.tsx:953-986`

**Fix direction:** If `maxSeqBigInt > MAX_SAFE_INTEGER`, omit `lastSequence` entirely (or set it to 0 and include an explicit `legacy_resume_unsupported=true` flag). Prefer `lastSequencesByThread` (string) and `lastOffsetsByPartition` (string) for correctness.

**Acceptance criteria:** When max seq exceeds MAX_SAFE_INTEGER, UI never sends an unsafe numeric `lastSequence`.

---

## Worker Priority (recommended order)

1. **M-798 (P1)** refuse state mutations without real seq
2. **M-796 (P2)** decode seq=0 for non-batch messages
3. **M-799 (P2)** record non-core message types in RunStateStore
4. **M-800 (P2)** dedupe by messageId (not seq)
5. **M-803 (P2)** explicit stale/unhealthy UI state on health failures
6. **M-804 (P2)** derived metrics become unknown when stale
7. **M-805 (P2)** don’t send unsafe legacy lastSequence
8. **M-801 (P2)** harden jsonPatch safeClone fallback
9. **M-802 (P3)** accept seq=0 in per-thread persistence
10. **M-797 (P3)** use explicit undefined check for timestampUs
