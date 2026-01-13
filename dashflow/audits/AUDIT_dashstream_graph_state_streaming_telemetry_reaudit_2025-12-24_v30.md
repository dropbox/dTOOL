# v30 Skeptical Source Code Audit - DashStream Graph State / Streaming / Telemetry

**Date:** 2025-12-24
**Auditor:** Worker #1706 (v30 deep source code audit)
**Prior:** v29 (M-796..M-805), v28 (M-786..M-795), v27 (M-776..M-785), v26 (M-775)
**Scope:** `observability-ui/`, DashStream graph-state pipeline, streaming recovery logic
**Method:** Deep skeptical source code analysis focused on recovery/corruption flag handling

---

## Executive Summary

This v30 pass focuses on **recovery logic correctness** (corrupted flag clearing), **event timestamp accuracy** (batch vs inner event timestamps), and **clone failure handling**.

Found **3 NEW issues** (M-806 to M-808), including **1 P2**.

### Design/Implementation Critique (cross-cutting)

- **The `corrupted` flag is effectively sticky**: There are 12 code paths that set `corrupted=true` but only 1 path that sets `corrupted=false` (snapshotParseError recovery). This means runs marked corrupted due to out-of-order mutations, missing sequences, hash mismatches, or patch failures will remain visually marked as corrupted forever even after successful recovery (M-806).
- **Event batch processing loses inner-event timestamp precision**: Inner events inherit the batch timestamp rather than using their own header timestamp, which loses microsecond-level timing information (M-807).

---

## New Issues (M-806 to M-808)

### M-806 (P2): `corrupted` flag only cleared in snapshotParseError recovery path (sticky corruption)
**Category:** Graph State/UI Correctness
**Severity:** P2

**Problem:** The `corrupted` flag is set in 12 different error conditions but only cleared in ONE recovery path (line 1034: snapshotParseError recovery). This means:
- Runs corrupted due to missing real sequence (M-798) stay corrupted forever
- Runs corrupted due to out-of-order mutations (M-787) stay corrupted forever
- Runs corrupted due to patch apply failures stay corrupted forever
- Runs corrupted due to hash mismatches stay corrupted forever

Even when a valid full state snapshot arrives (needsResync recovery at lines 1019-1026), the `corrupted` flag is NOT cleared, only `needsResync` is.

**Files:**
- `observability-ui/src/hooks/useRunStateStore.ts:1019-1026` (needsResync recovery does NOT clear corrupted)
- `observability-ui/src/hooks/useRunStateStore.ts:1011-1018` (patchApplyFailed recovery does NOT clear corrupted)
- `observability-ui/src/hooks/useRunStateStore.ts:1027-1035` (snapshotParseError recovery DOES clear corrupted - only path)

**Evidence:**
```bash
# 12 places set corrupted=true
grep -n "store\.corrupted = true" observability-ui/src/hooks/useRunStateStore.ts | wc -l  # 12

# Only 1 place sets corrupted=false
grep -n "store\.corrupted = false" observability-ui/src/hooks/useRunStateStore.ts | wc -l  # 1
```

**Fix direction:** When a full state snapshot successfully applies (recovery from needsResync, patchApplyFailed, or checkpoint arrival), also clear `store.corrupted = false` and `store.corruptionDetails = undefined`. The recovery is complete and state is now consistent.

**Acceptance criteria:** After a successful full state snapshot applies, both `needsResync` and `corrupted` are false, and `corruptionDetails` is cleared.

---

### M-807 (P3): Event batch inner events use batch timestamp instead of their own header timestamp
**Category:** Telemetry/Correctness
**Severity:** P3

**Problem:** When processing `event_batch`, inner events are processed with `timestamp: decoded.timestamp` (the batch's timestamp) instead of extracting the inner event's own `header.timestampUs`. This loses per-event timing precision.

**File:** `observability-ui/src/hooks/useRunStateStore.ts:1206-1215`

```typescript
processMessage({
  type: 'event',
  message: { event },
  timestamp: decoded.timestamp,  // <-- Uses batch timestamp, not inner event's
  threadId: innerThreadId,
  sequence: innerSeq,
  ...
});
```

**Fix direction:** Extract `innerHeader.timestampUs` if present and convert to ms timestamp. Fall back to `decoded.timestamp` only if inner timestamp is missing.

```typescript
const innerTsUs = innerHeader?.['timestampUs'] as bigint | undefined;
const innerTimestamp = innerTsUs !== undefined
  ? Number(innerTsUs) / 1000
  : decoded.timestamp;
```

**Acceptance criteria:** Inner events with their own `header.timestampUs` are processed with their own timestamp, not the batch's.

---

### M-808 (P3): deepCloneJson/safeClone return original on failure (silent mutation aliasing)
**Category:** Robustness
**Severity:** P3

**Problem:** Both `deepCloneJson` (useRunStateStore.ts:49-69) and `safeClone` (jsonPatch.ts:24-39) fall back to returning the original value when both structuredClone and JSON round-trip fail. This is documented as "better than crashing", but creates mutation aliasing where modifications to the "clone" affect the original state.

This can cause subtle state corruption bugs that are hard to debug:
- Checkpoint state stored by ID could be mutated by subsequent operations
- Hash verification could compare wrong state if the clone was mutated

**Files:**
- `observability-ui/src/hooks/useRunStateStore.ts:68` - returns original on failure
- `observability-ui/src/utils/jsonPatch.ts:36` - returns original on failure

**Fix direction:**
1. When returning original, set a flag `store.cloneFailureWarned=true` and/or `store.needsResync=true`
2. Consider returning `{}` instead of original to fail loudly rather than silently corrupt
3. Log which state keys triggered the failure for debugging

**Acceptance criteria:** Clone failures either fail loudly (throw or mark corrupted) OR the aliasing risk is mitigated by making downstream code aware of the uncloned state.

---

## Verified Patterns (No Issues Found)

### Out-of-order detection correctly uses lastAppliedSeq
The M-787 fix correctly tracks `lastAppliedSeq` and compares incoming sequences. Out-of-order mutations are detected and blocked. The logic at lines 955-970 is sound.

### Checkpoint base_checkpoint_id verification is correct
The M-696/M-771 logic correctly verifies base checkpoint references and handles missing/invalid checkpoints by flagging needsResync.

### Sequence validation is comprehensive
The M-798 fix correctly requires `isRealSeq(seq)` before mutating state. All state-mutating paths check this.

### Hash verification respects stateApplied flag
The M-777 fix correctly skips hash verification when `stateApplied=false`. No false positives expected.

---

## Worker Priority (recommended order)

1. **M-806 (P2)** clear corrupted flag on all recovery paths (not just snapshotParseError)
2. **M-807 (P3)** use inner event timestamp in event_batch processing
3. **M-808 (P3)** mitigate clone failure aliasing risk

---

## Audit Methodology

1. Read v29 audit to understand prior issues and fixes
2. Traced all paths that set `corrupted=true` and verified recovery paths clear it
3. Analyzed event_batch processing for timestamp handling
4. Reviewed clone/copy functions for failure modes
5. Verified M-798 fixes are comprehensive and correctly implemented
