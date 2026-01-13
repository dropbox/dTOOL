# v28 Skeptical Source Code Audit - DashStream Graph State / Streaming / Telemetry

**Date:** 2025-12-24
**Auditor:** Worker #1669 (v28 deep source code audit)
**Prior:** v27 (M-776..M-785), v26 (M-775), v25 (M-774), v24 (M-762..M-773), v23 (M-749..M-760), v22 (M-739..M-748), v21 (M-719..M-738), v20 (M-707..M-716)
**Scope:** `observability-ui/`, `crates/dashflow-observability/`, DashStream graph-state pipeline, streaming metrics/measurement/telemetry
**Method:** Deep skeptical source code analysis — actual source, not docs

---

## Executive Summary

This v28 pass focuses on **measurement correctness** (what our charts/percentages actually mean), and **graph-state correctness under edge conditions** (seq=0, out-of-order delivery, resync semantics).

Found **10 NEW issues** (M-786 to M-795), including **1 P1**.

### Design/Implementation Critique (cross-cutting)

- **State application is not a pure function of ordered events**: the store has the machinery to insert events out-of-order, but state mutation is still performed eagerly on arrival, which is a correctness footgun under any out-of-order scenario (M-787).
- **Several “metrics” are derived from counters without an explicit contract**: UI labels imply “current” rates, but computations are lifetime ratios or assume a fixed poll interval (M-789/M-790).
- **Telemetry plumbing is missing basic robustness**: periodic health polling can overlap/hang and parsing assumes always-OK JSON responses (M-792/M-795).

---

## New Issues (M-786 to M-795)

### M-786 (P2): UI discards real seq=0 and replaces it with synthetic negative seq
**Category:** Graph State/Correctness
**Severity:** P2

**Problem:** The run-state store treats any non-positive sequence as “missing” and replaces it with a synthetic negative sequence. But seq=0 is now treated as valid in parts of the pipeline (e.g., event batches / protocol evolution), so replacing it can corrupt ordering/dedup assumptions.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:698-704`
```ts
if (decoded.sequence && isPositiveSeq(decoded.sequence)) {
  seq = decoded.sequence;
} else {
  seq = nextSyntheticSeqRef.current.toString();
}
```

**Impact:** A real message with `sequence="0"` becomes indistinguishable from “missing sequence” synthetic events, potentially breaking ordering and any cursor logic based on seq.

**Fix direction:** Accept `seq >= 0` as “real” (or explicitly accept `0`), and reserve synthetic negatives strictly for missing sequences.

**Acceptance criteria:** When `decoded.sequence === "0"`, `processMessage()` stores `seq="0"` (not a synthetic negative).

---

### M-787 (P1): Store supports out-of-order insertion but applies state mutations in arrival order (can corrupt state)
**Category:** Graph State/Correctness
**Severity:** P1

**Problem:** `addEvent()` explicitly supports out-of-order insertion (binary search insert) and logs when it happens, but state mutations (applying patches / replacing latestState) happen immediately during message processing, in arrival order.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:539-572` (out-of-order insertion support)

**Impact:** If the UI ever receives out-of-order StateDiff/Checkpoint messages (bug, multi-producer, or protocol edge), it can apply an older update after a newer one, silently corrupting state.

**Fix direction (pick one):**
- Hard correctness: buffer out-of-order updates and apply strictly in seq order per thread, or
- Defensive: if an out-of-order **state-mutating** message is detected (seq < latest applied), set `needsResync=true`, stop applying further patches, and wait for fullState snapshot recovery.

**Acceptance criteria:** When a state-mutating message arrives with `seq < lastAppliedSeq`, UI does not mutate state and flags resync.

---

### M-788 (P2): Gap/stale-cursor resync marking ignores non-running runs (can leave incorrect state “trusted”)
**Category:** Telemetry/Correctness
**Severity:** P2

**Problem:** `markActiveRunsNeedResync()` only marks runs with `status === 'running'`. But a gap/cursor_stale signal indicates the stream is untrustworthy globally; completed/error runs can still be incomplete or corrupted and should be visibly flagged.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:1536-1543`

**Impact:** UI can present completed runs as “clean” even though the stream had gaps; this is especially misleading for debugging/forensics.

**Fix direction:**
- Mark all runs that have received any events in the current session, not only `running`, or
- Mark the currently active thread run + any run with `endTime` after the gap time.

**Acceptance criteria:** After a gap/stale signal, completed runs that overlap the affected window show `needsResync/corrupted`.

---

### M-789 (P2): Messages/sec computation assumes fixed 5s sampling interval and uses locale time strings
**Category:** Measurement Correctness
**Severity:** P2

**Problem:** `messagesPerSecond` divides by a hard-coded 5 seconds and `throughputData` stores a locale time string (no monotonic timestamp). Real sampling is jittery and can overlap; counters can reset on server restart.

**Files:**
- `observability-ui/src/App.tsx:742-751` (stores `{ time: toLocaleTimeString(), messages, errors }`)
- `observability-ui/src/App.tsx:1482-1487` (diff/5 assumption)

**Impact:** The displayed MPS can be wrong by large factors (jitter), and restarts/counter resets are silently flattened to 0.

**Fix direction:** Store numeric `tMs = Date.now()` alongside counters, compute rate as `delta / (dtMs/1000)`, and detect counter resets (clear history + annotate).

**Acceptance criteria:** MPS uses actual wall-clock dt between samples, not a hard-coded constant.

---

### M-790 (P3): Error rate is an overall lifetime ratio and can go stale (misleading “current error rate”)
**Category:** Measurement Semantics
**Severity:** P3

**Problem:** Error rate uses `kafka_errors / kafka_messages_received` (cumulative) and is only updated when messages_received > 0. If messages drop to 0 or counters reset, UI can keep showing a stale errorRate.

**File:** `observability-ui/src/App.tsx:736-740`

**Impact:** Operators may interpret this as a current %; it’s really lifetime ratio since server boot, and can display stale data after resets.

**Fix direction:** Either rename UI label to “lifetime error % since boot” or compute a rolling/windowed error rate based on deltas and timestamps; reset to 0 when messages_received is 0.

**Acceptance criteria:** UI error rate label matches the computation, and value does not remain stale on no-data.

---

### M-791 (P2): Latency chart clamps negative latencies to 0, hiding clock skew / timestamp bugs
**Category:** Measurement Correctness
**Severity:** P2

**Problem:** Latency is computed as `now - decoded.timestamp` and clamped with `Math.max(0, latency)`.

**File:** `observability-ui/src/App.tsx:1373-1379`

**Impact:** Clock skew (or producer timestamp bugs) becomes invisible and looks “perfect” (0ms), masking real telemetry integrity problems.

**Fix direction:** Preserve negative values (display separately as “clock skew”) or clamp but also emit an explicit warning/metric when `latency < 0`.

**Acceptance criteria:** Negative latency becomes visible (UI warning or dedicated skew series), not silently flattened to 0.

---

### M-792 (P2): Health polling can overlap and hang indefinitely (no timeout, no in-flight guard)
**Category:** Telemetry/Robustness
**Severity:** P2

**Problem:** `fetchHealth()` uses `fetch('/health')` with no timeout and is scheduled on a fixed `setInterval`. If `/health` stalls, requests can pile up and keep memory/CPU pinned.

**Files:**
- `observability-ui/src/App.tsx:730-755` (`fetch('/health')` without timeout)
- `observability-ui/src/App.tsx:1490-1495` (`setInterval(fetchHealth, 5000)`)

**Fix direction:** Add an in-flight guard + `AbortController` timeout (e.g., 3–5s), and skip scheduling a new request when one is outstanding.

**Acceptance criteria:** At most 1 in-flight `/health` request at a time, and requests time out.

---

### M-793 (P3): normalizeIntegerString accepts negative values; can collide with synthetic negative seq conventions
**Category:** Data Model Hardening
**Severity:** P3

**Problem:** `normalizeIntegerString()` accepts `^-?\\d+$`, allowing negative strings. But negative sequences are reserved for synthetic seqs in the UI. Accepting negative producer values risks collisions and confusing ordering.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:59-66`

**Fix direction:** Split helpers: one for signed integers (if ever needed), and a strict u64 string normalizer for sequences (digits only).

**Acceptance criteria:** Sequence normalization rejects negative string values from producers.

---

### M-794 (P2): deepCloneJson fallback can throw and crash if JSON round-trip fails (e.g., BigInt)
**Category:** Robustness
**Severity:** P2

**Problem:** `deepCloneJson()` catches structuredClone failures and then does `JSON.parse(JSON.stringify(value))` without a second safety net. If stringify fails (e.g., BigInt), this throws and can crash the UI.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:48-56`

**Fix direction:** Wrap the JSON round-trip in try/catch too; if it fails, return a safe fallback (e.g., `{}`) and set `needsResync/corrupted` for that run, or implement a custom serializer that can drop non-JSON values without throwing.

**Acceptance criteria:** A structuredClone failure never crashes the app; worst case it flags resync/corruption.

---

### M-795 (P3): Health/version fetch does not check HTTP status codes (can throw on non-JSON error bodies)
**Category:** Telemetry/Robustness
**Severity:** P3

**Problem:** `fetchHealth()` and `fetchVersion()` call `response.json()` without checking `response.ok`. A 503/HTML error body will throw and silently leave stale UI state.

**File:** `observability-ui/src/App.tsx:731-734`, `observability-ui/src/App.tsx:759-763`

**Fix direction:** Check `response.ok`, and on failure set an explicit “unhealthy/unavailable” UI state with the status code; optionally still parse JSON if content-type is JSON.

**Acceptance criteria:** Non-200 responses produce visible error state, not just a console log.

---

## Worker Priority (recommended order)

1. **M-787 (P1)** out-of-order state mutation protection
2. **M-786 (P2)** accept seq=0 as real (don’t synthesize)
3. **M-792 (P2)** health polling timeout + in-flight guard
4. **M-789 (P2)** messages/sec compute using real dt + reset handling
5. **M-791 (P2)** expose negative latency / skew instead of clamping to 0
6. **M-788 (P2)** resync marking should include non-running runs as appropriate
7. **M-794 (P2)** deepCloneJson fallback safety net
8. **M-790 (P3)** clarify/rename error rate semantics or compute rolling
9. **M-793 (P3)** reject negative integer strings for seq normalization
10. **M-795 (P3)** check response.ok before parsing JSON
